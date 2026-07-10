use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::mem;

use super::AudioShared;

pub(crate) fn decode_audio(path: &str, shared: Arc<Mutex<AudioShared>>) {
    use ffmpeg_sys_next::*;
    unsafe {
        let mut current_path = path.to_owned();
        loop {
            let path_c = CString::new(current_path.clone()).unwrap();
            let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();

            let mut opts: *mut AVDictionary = ptr::null_mut();
            av_dict_set(&mut opts, CString::new("probesize").unwrap().as_ptr(), CString::new("32000").unwrap().as_ptr(), 0);
            av_dict_set(&mut opts, CString::new("analyzeduration").unwrap().as_ptr(), CString::new("0").unwrap().as_ptr(), 0);
            
            let ret = avformat_open_input(&mut fmt_ctx, path_c.as_ptr(), ptr::null_mut(), &mut opts);
            av_dict_free(&mut opts);
            
            if ret < 0 {
                crate::app_log!("audio: cannot open '{}'", current_path);
                loop {
                    let mut s = shared.lock().unwrap();
                    if s.quit { return; }
                    if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                    drop(s);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                continue;
            }
            if avformat_find_stream_info(fmt_ctx, ptr::null_mut()) < 0 {
                crate::app_log!("audio: cannot find stream info");
                avformat_close_input(&mut fmt_ctx);
                loop {
                    let mut s = shared.lock().unwrap();
                    if s.quit { return; }
                    if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                    drop(s);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                continue;
            }

        let nb      = (*fmt_ctx).nb_streams as usize;
        let streams = std::slice::from_raw_parts((*fmt_ctx).streams, nb);
        let mut audio_idx = -1i32;
        for (i, &s) in streams.iter().enumerate() {
            if (*(*s).codecpar).codec_type == AVMediaType::AVMEDIA_TYPE_AUDIO {
                if audio_idx < 0 { audio_idx = i as i32; }
            } else {
                (*s).discard = AVDiscard::AVDISCARD_ALL;
            }
        }
        if audio_idx < 0 {
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut s = shared.lock().unwrap();
                if s.quit { return; }
                if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                drop(s);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let as_       = *streams[audio_idx as usize];
        let audio_tb  = as_.time_base;
        let codec     = avcodec_find_decoder((*as_.codecpar).codec_id);
        if codec.is_null() {
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut s = shared.lock().unwrap();
                if s.quit { return; }
                if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                drop(s);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let mut codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut s = shared.lock().unwrap();
                if s.quit { return; }
                if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                drop(s);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }
        avcodec_parameters_to_context(codec_ctx, as_.codecpar);
        (*codec_ctx).thread_count = 1; // Limit threads to save RAM
        if avcodec_open2(codec_ctx, codec, ptr::null_mut()) < 0 {
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut s = shared.lock().unwrap();
                if s.quit { return; }
                if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                drop(s);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let src_rate = (*codec_ctx).sample_rate;
        let src_fmt  = (*codec_ctx).sample_fmt;
        let dst_rate = 44100i32;
        let dst_fmt  = AVSampleFormat::AV_SAMPLE_FMT_FLT;

        let mut dst_layout: AVChannelLayout = mem::zeroed();
        av_channel_layout_default(&mut dst_layout, 2);

        let mut swr: *mut SwrContext = ptr::null_mut();
        let ret = swr_alloc_set_opts2(
            &mut swr,
            &dst_layout as *const AVChannelLayout,
            dst_fmt,
            dst_rate,
            &(*codec_ctx).ch_layout as *const AVChannelLayout,
            src_fmt,
            src_rate,
            0,
            ptr::null_mut(),
        );
        if ret < 0 || swr.is_null() {
            crate::app_log!("audio: cannot create resampler");
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut s = shared.lock().unwrap();
                if s.quit { return; }
                if let Some(new_file) = s.load_file.take() { current_path = new_file; break; }
                drop(s);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }
        swr_init(swr);

        let mut pkt   = av_packet_alloc();
        let mut frame = av_frame_alloc();
        let mut skip_to_pts: Option<f64> = None;

        loop {
            // ── Bug #3 fix: pause — back-pressure by waiting ─────────────
            // ── Bug #4 fix: seek — flush buffer and jump ─────────────────
            {
                let mut s = shared.lock().unwrap();

                if s.quit { break; }

                if let Some(new_file) = s.load_file.take() {
                    current_path = new_file;
                    s.buffer.clear();
                    break;
                }

                if let Some(target) = s.seek_to.take() {
                    // Flush decode pipeline
                    avcodec_flush_buffers(codec_ctx);
                    // Clear audio buffer to eliminate stale samples causing A/V desync
                    s.buffer.clear();
                    // Reset audio position to seek target (master clock for A/V sync)
                    s.audio_position_secs = target;
                    let seek_ts = (target * audio_tb.den as f64 / audio_tb.num as f64) as i64;
                    drop(s);
                    av_seek_frame(fmt_ctx, audio_idx, seek_ts, AVSEEK_FLAG_BACKWARD);
                    skip_to_pts = Some(target);
                    continue;
                }

                if !s.playing {
                    drop(s);
                    std::thread::sleep(std::time::Duration::from_millis(8));
                    continue;
                }

                // Don't over-buffer — apply back-pressure so RAM stays bounded
                // 44100 samples/sec × 2 ch × 2 sec headroom
                if s.buffer.len() >= 44100 * 2 * 2 {
                    drop(s);
                    std::thread::sleep(std::time::Duration::from_millis(20));
                    continue;
                }
            }

            if av_read_frame(fmt_ctx, pkt) < 0 { 
                // Reached EOF for audio. Do not set playing = false, because the video thread
                // relies on the audio master clock to pace itself. If we pause the audio clock,
                // the video thread will freeze and cause severe lag.
                // Just sleep and wait for a seek or a new file.
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue; 
            }

            if (*pkt).stream_index != audio_idx {
                av_packet_unref(pkt);
                continue;
            }

            avcodec_send_packet(codec_ctx, pkt);
            av_packet_unref(pkt);

            while avcodec_receive_frame(codec_ctx, frame) >= 0 {
                let frame_pts = if (*frame).pts != i64::MIN && (*frame).pts != i64::MAX {
                    (*frame).pts as f64 * audio_tb.num as f64 / audio_tb.den as f64
                } else {
                    -1.0
                };

                // Skip frames until we hit the precise seek target
                if let Some(target) = skip_to_pts {
                    if frame_pts >= 0.0 && frame_pts < target {
                        av_frame_unref(frame);
                        continue;
                    }
                    skip_to_pts = None;
                }

                let nb_samples  = (*frame).nb_samples;
                let delay       = swr_get_delay(swr, src_rate as i64);
                let dst_nb      = av_rescale_rnd(
                    delay + nb_samples as i64,
                    dst_rate as i64,
                    src_rate as i64,
                    AVRounding::AV_ROUND_UP,
                ) as i32;

                let dst_buf_size = av_samples_get_buffer_size(
                    ptr::null_mut(), 2, dst_nb, dst_fmt, 1,
                );
                let mut dst_buf = av_malloc(dst_buf_size as usize) as *mut u8;

                let converted = swr_convert(
                    swr,
                    &mut dst_buf as *mut *mut u8,
                    dst_nb,
                    (*frame).extended_data as *const *const u8,
                    nb_samples,
                );

                if converted > 0 {
                    let total   = (converted * 2) as usize;
                    let samples = std::slice::from_raw_parts(dst_buf as *const f32, total);
                    let mut s = shared.lock().unwrap();
                    // Sync audio position to actual frame PTS when buffer was empty
                    // (after seek or buffer drain) to correct seek-keyframe misalignment
                    if s.buffer.is_empty() && frame_pts >= 0.0 && s.audio_position_secs < frame_pts {
                        s.audio_position_secs = frame_pts;
                    }
                    // Note: volume applied at playback time in AudioCallback, not here
                    s.buffer.extend(samples.iter());
                }

                av_free(dst_buf as *mut libc::c_void);
                av_frame_unref(frame);
            }
        }

        swr_free(&mut swr);
        av_channel_layout_uninit(&mut dst_layout);
        av_frame_free(&mut frame);
        av_packet_free(&mut pkt);
        avcodec_free_context(&mut codec_ctx);
        avformat_close_input(&mut fmt_ctx);

        if shared.lock().unwrap().quit {
            break;
        }
        } // end of outer loop
    }
}
