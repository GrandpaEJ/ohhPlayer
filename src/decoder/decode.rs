use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};

use super::{DecodedFrame, DecoderCommand, DecoderState};
use crate::audio::AudioShared;

pub(crate) fn decode_video(
    path: &str,
    _target_w: u32,
    _target_h: u32,
    command:   Arc<Mutex<DecoderCommand>>,
    state:     Arc<Mutex<DecoderState>>,
    frame_out: Arc<Mutex<Option<DecodedFrame>>>,
    audio_shared: Arc<Mutex<AudioShared>>,
) {
    use ffmpeg_sys_next::*;
    unsafe {
        let mut current_path = path.to_owned();
        loop {
            let path_c = CString::new(current_path.clone()).unwrap();
            let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();

            if avformat_open_input(&mut fmt_ctx, path_c.as_ptr(), ptr::null_mut(), ptr::null_mut()) < 0 {
                eprintln!("decoder: cannot open '{}'", current_path);
                // Wait for a new file command
                loop {
                    let mut c = command.lock().unwrap();
                    if c.quit { return; }
                    if let Some(new_file) = c.load_file.take() {
                        current_path = new_file;
                        break;
                    }
                    drop(c);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                continue;
            }
            if avformat_find_stream_info(fmt_ctx, ptr::null_mut()) < 0 {
                eprintln!("decoder: cannot find stream info");
                avformat_close_input(&mut fmt_ctx);
                // Wait for a new file command
                loop {
                    let mut c = command.lock().unwrap();
                    if c.quit { return; }
                    if let Some(new_file) = c.load_file.take() {
                        current_path = new_file;
                        break;
                    }
                    drop(c);
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                continue;
            }

        let nb      = (*fmt_ctx).nb_streams as usize;
        let streams = std::slice::from_raw_parts((*fmt_ctx).streams, nb);
        let mut video_idx = -1i32;
        for (i, &s) in streams.iter().enumerate() {
            if (*(*s).codecpar).codec_type == AVMediaType::AVMEDIA_TYPE_VIDEO {
                video_idx = i as i32;
                break;
            }
        }
        if video_idx < 0 {
            eprintln!("decoder: no video stream");
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut c = command.lock().unwrap();
                if c.quit { return; }
                if let Some(new_file) = c.load_file.take() {
                    current_path = new_file;
                    break;
                }
                drop(c);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let vs  = *streams[video_idx as usize];
        let tb  = vs.time_base;
        let dur = if vs.duration > 0 {
            vs.duration as f64 * tb.num as f64 / tb.den as f64
        } else {
            (*fmt_ctx).duration as f64 / 1_000_000.0
        };
        state.lock().unwrap().duration = dur;

        let codec = avcodec_find_decoder((*vs.codecpar).codec_id);
        if codec.is_null() {
            eprintln!("decoder: codec not found");
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut c = command.lock().unwrap();
                if c.quit { return; }
                if let Some(new_file) = c.load_file.take() { current_path = new_file; break; }
                drop(c);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let mut codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            eprintln!("decoder: cannot alloc codec context");
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut c = command.lock().unwrap();
                if c.quit { return; }
                if let Some(new_file) = c.load_file.take() { current_path = new_file; break; }
                drop(c);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }
        avcodec_parameters_to_context(codec_ctx, vs.codecpar);
        if avcodec_open2(codec_ctx, codec, ptr::null_mut()) < 0 {
            eprintln!("decoder: cannot open codec");
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut c = command.lock().unwrap();
                if c.quit { return; }
                if let Some(new_file) = c.load_file.take() { current_path = new_file; break; }
                drop(c);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        let native_w = (*codec_ctx).width as u32;
        let native_h = (*codec_ctx).height as u32;

        {
            let mut st = state.lock().unwrap();
            st.video_width = native_w;
            st.video_height = native_h;
        }

        let sws_ctx = sws_getContext(
            (*codec_ctx).width,
            (*codec_ctx).height,
            (*codec_ctx).pix_fmt,
            native_w as i32,
            native_h as i32,
            AVPixelFormat::AV_PIX_FMT_RGB24,
            2,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        );
        if sws_ctx.is_null() {
            eprintln!("decoder: cannot create scaler");
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            loop {
                let mut c = command.lock().unwrap();
                if c.quit { return; }
                if let Some(new_file) = c.load_file.take() { current_path = new_file; break; }
                drop(c);
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            continue;
        }

        // Intermediate rgb_buf and rgb_frame removed. 
        // We will scale directly into Slint's SharedPixelBuffer inside the loop.

        let mut pkt      = av_packet_alloc();
        let mut frame    = av_frame_alloc();
        let mut do_seek  = false;
        let mut seek_ts: i64 = 0;

        let mut skip_to_pts:   Option<f64> = None;

        loop {
            // ── Process commands before every packet ─────────────────────
            {
                let mut c = command.lock().unwrap();
                if c.quit { break; }

                if let Some(new_file) = c.load_file.take() {
                    current_path = new_file;
                    break; // breaks inner packet loop, proceeds to cleanup, then restarts outer loop
                }

                // Bug #6 fix: seek is checked every iteration, not only for video pkts
                if let Some(target) = c.seek_target.take() {
                    do_seek     = true;
                    seek_ts     = (target * tb.den as f64 / tb.num as f64) as i64;
                    skip_to_pts = Some(target);
                }

                let playing = c.playing;
                drop(c);
                state.lock().unwrap().playing = playing;

                if !playing {
                    std::thread::sleep(std::time::Duration::from_millis(8));
                    continue;
                }
            }

            // ── Seek ─────────────────────────────────────────────────────
            if do_seek {
                av_seek_frame(fmt_ctx, video_idx, seek_ts, AVSEEK_FLAG_BACKWARD);
                avcodec_flush_buffers(codec_ctx);
                do_seek = false;
            }

            let ret = av_read_frame(fmt_ctx, pkt);
            if ret < 0 { 
                command.lock().unwrap().playing = false;
                continue; 
            }

            if (*pkt).stream_index != video_idx {
                av_packet_unref(pkt);
                continue;
            }

            avcodec_send_packet(codec_ctx, pkt);
            av_packet_unref(pkt);

            while avcodec_receive_frame(codec_ctx, frame) >= 0 {
                // ── Get PTS ───────────────────────────────────────────────
                let frame_pts = if (*frame).pts != i64::MIN && (*frame).pts != i64::MAX {
                    (*frame).pts as f64 * tb.num as f64 / tb.den as f64
                } else {
                    av_frame_unref(frame);
                    continue; // Skip frames without a valid timestamp
                };

                // Skip frames until we hit the precise seek target
                if let Some(target) = skip_to_pts {
                    if frame_pts < target {
                        av_frame_unref(frame);
                        continue;
                    }
                    skip_to_pts = None;
                }

                state.lock().unwrap().position = frame_pts;

                // ── A/V sync: use audio position as master clock ──────────
                let (audio_pos, spd) = {
                    let a = audio_shared.lock().unwrap();
                    let c = command.lock().unwrap();
                    (a.audio_position_secs, c.speed as f64)
                };
                let effective_pos = if spd > 0.0 { audio_pos * spd } else { audio_pos };

                if frame_pts < effective_pos - 0.3 {
                    // Video is far behind audio — skip this frame to catch up
                    av_frame_unref(frame);
                    continue;
                }

                if frame_pts > effective_pos + 0.005 {
                    // Video is ahead of audio — sleep until audio catches up
                    let mut sleep_ms = ((frame_pts - effective_pos) * 1000.0) as i64;
                    while sleep_ms > 0 {
                        let chunk = if sleep_ms > 20 { 20 } else { sleep_ms };
                        std::thread::sleep(std::time::Duration::from_millis(chunk as u64));
                        sleep_ms -= chunk;
                        let c = command.lock().unwrap();
                        if c.quit || c.load_file.is_some() || c.seek_target.is_some() {
                            break;
                        }
                    }
                }

                // ── Scale frame to RGB directly into Slint's buffer ───────
                let mut buffer = slint::SharedPixelBuffer::<slint::Rgb8Pixel>::new(native_w, native_h);
                let slice = buffer.make_mut_slice();
                
                let dst_data: [*mut u8; 4] = [slice.as_mut_ptr() as *mut u8, ptr::null_mut(), ptr::null_mut(), ptr::null_mut()];
                let dst_linesize: [i32; 4] = [native_w as i32 * 3, 0, 0, 0];

                sws_scale(
                    sws_ctx,
                    (*frame).data.as_ptr() as *const *const u8,
                    (*frame).linesize.as_ptr(),
                    0,
                    (*codec_ctx).height,
                    dst_data.as_ptr(),
                    dst_linesize.as_ptr(),
                );

                *frame_out.lock().unwrap() = Some(DecodedFrame {
                    buffer,
                });

                av_frame_unref(frame);
            }
        }

        av_frame_free(&mut frame);
        av_packet_free(&mut pkt);
        sws_freeContext(sws_ctx);
        avcodec_free_context(&mut codec_ctx);
        avformat_close_input(&mut fmt_ctx);
        
        if command.lock().unwrap().quit {
            break; // Exit the outer loop and thread
        }
        } // end of outer loop
    }
}
