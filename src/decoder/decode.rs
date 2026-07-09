use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};

use super::{DecodedFrame, DecoderCommand, DecoderState};

pub(crate) fn decode_video(
    path: &str,
    target_w: u32,
    target_h: u32,
    command:   Arc<Mutex<DecoderCommand>>,
    state:     Arc<Mutex<DecoderState>>,
    frame_out: Arc<Mutex<Option<DecodedFrame>>>,
) {
    use ffmpeg_sys_next::*;
    unsafe {
        let path_c = CString::new(path).unwrap();
        let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();

        if avformat_open_input(&mut fmt_ctx, path_c.as_ptr(), ptr::null_mut(), ptr::null_mut()) < 0 {
            eprintln!("decoder: cannot open '{}'", path);
            return;
        }
        if avformat_find_stream_info(fmt_ctx, ptr::null_mut()) < 0 {
            eprintln!("decoder: cannot find stream info");
            avformat_close_input(&mut fmt_ctx);
            return;
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
            return;
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
            return;
        }

        let mut codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            eprintln!("decoder: cannot alloc codec context");
            avformat_close_input(&mut fmt_ctx);
            return;
        }
        avcodec_parameters_to_context(codec_ctx, vs.codecpar);
        if avcodec_open2(codec_ctx, codec, ptr::null_mut()) < 0 {
            eprintln!("decoder: cannot open codec");
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            return;
        }

        let sws_ctx = sws_getContext(
            (*codec_ctx).width,
            (*codec_ctx).height,
            (*codec_ctx).pix_fmt,
            target_w as i32,
            target_h as i32,
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
            return;
        }

        let rgb_size = av_image_get_buffer_size(AVPixelFormat::AV_PIX_FMT_RGB24, target_w as i32, target_h as i32, 1);
        let rgb_buf  = av_malloc(rgb_size as usize) as *mut u8;
        let mut rgb_frame = av_frame_alloc();
        av_image_fill_arrays(
            (*rgb_frame).data.as_mut_ptr(),
            (*rgb_frame).linesize.as_mut_ptr(),
            rgb_buf,
            AVPixelFormat::AV_PIX_FMT_RGB24,
            target_w as i32,
            target_h as i32,
            1,
        );

        let mut pkt      = av_packet_alloc();
        let mut frame    = av_frame_alloc();
        let mut do_seek  = false;
        let mut seek_ts: i64 = 0;

        // ── Frame-pacing state ─────────────────────────────────────────────
        // wall_start / pts_start track when playback began so we can sleep
        // the correct amount before presenting each frame.
        let mut wall_start:    Option<std::time::Instant> = None;
        let mut pts_start:     f64 = 0.0;
        let mut pause_elapsed: f64 = 0.0;   // accumulated seconds spent paused
        let mut pause_since:   Option<std::time::Instant> = None;
        let mut skip_to_pts:   Option<f64> = None;

        loop {
            // ── Process commands before every packet ─────────────────────
            {
                let mut c = command.lock().unwrap();
                if c.quit { break; }

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
                    // Bug #3 fix: properly track pause duration for frame-pacing
                    if pause_since.is_none() {
                        pause_since = Some(std::time::Instant::now());
                    }
                    std::thread::sleep(std::time::Duration::from_millis(8));
                    continue;
                } else if let Some(ps) = pause_since.take() {
                    // Resumed — accumulate the pause gap so timing stays correct
                    pause_elapsed += ps.elapsed().as_secs_f64();
                }
            }

            // ── Seek ─────────────────────────────────────────────────────
            if do_seek {
                av_seek_frame(fmt_ctx, video_idx, seek_ts, AVSEEK_FLAG_BACKWARD);
                avcodec_flush_buffers(codec_ctx);
                do_seek = false;
                // Bug #2 fix: reset frame-pacing anchor so we don't sleep forever
                wall_start    = None;
                pause_elapsed = 0.0;
                pause_since   = None;
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
                    continue; // Skip frames without a valid timestamp
                };

                // Skip frames until we hit the precise seek target
                if let Some(target) = skip_to_pts {
                    if frame_pts < target {
                        continue;
                    }
                    skip_to_pts = None;
                }

                state.lock().unwrap().position = frame_pts;

                // ── Bug #2 fix: PTS-based frame pacing ────────────────────
                // Anchor on the first frame presented after a start/seek.
                if wall_start.is_none() {
                    wall_start = Some(std::time::Instant::now());
                    pts_start  = frame_pts;
                }
                let wall = wall_start.unwrap();
                // How long since playback anchor (minus paused time)
                let real_elapsed = wall.elapsed().as_secs_f64() - pause_elapsed;
                // How far into the stream this frame lives
                let pts_elapsed  = frame_pts - pts_start;

                // Speed factor from command (read fresh for accuracy)
                let spd = command.lock().unwrap().speed as f64;
                let adjusted_pts = if spd > 0.0 { pts_elapsed / spd } else { pts_elapsed };

                if adjusted_pts > real_elapsed {
                    let sleep_ms = ((adjusted_pts - real_elapsed) * 1000.0) as u64;
                    // Guard: never sleep more than 1 s (catches edge-cases after seek)
                    if sleep_ms < 1000 {
                        std::thread::sleep(std::time::Duration::from_millis(sleep_ms));
                    }
                }

                // ── Scale frame to RGB ────────────────────────────────────
                sws_scale(
                    sws_ctx,
                    (*frame).data.as_ptr() as *const *const u8,
                    (*frame).linesize.as_ptr(),
                    0,
                    (*codec_ctx).height,
                    (*rgb_frame).data.as_mut_ptr(),
                    (*rgb_frame).linesize.as_mut_ptr(),
                );

                let row_bytes = target_w as usize * 3;
                let mut buf   = vec![0u8; (target_h as usize) * row_bytes];
                for y in 0..target_h as usize {
                    let src = (*rgb_frame).data[0].offset((y * (*rgb_frame).linesize[0] as usize) as isize);
                    let dst = &mut buf[y * row_bytes..(y + 1) * row_bytes];
                    dst.copy_from_slice(std::slice::from_raw_parts(src, row_bytes));
                }

                *frame_out.lock().unwrap() = Some(DecodedFrame {
                    data:   buf,
                    width:  target_w,
                    height: target_h,
                });
            }
        }

        av_frame_free(&mut frame);
        av_frame_free(&mut rgb_frame);
        av_free(rgb_buf as *mut libc::c_void);
        av_packet_free(&mut pkt);
        sws_freeContext(sws_ctx);
        avcodec_free_context(&mut codec_ctx);
        avformat_close_input(&mut fmt_ctx);
    }
}
