use std::collections::VecDeque;
use std::ffi::CString;
use std::ptr;
use std::sync::{Arc, Mutex};
use std::mem;

pub struct AudioOutput {
    pub buffer: Arc<Mutex<VecDeque<f32>>>,
    sample_rate: i32,
    channels: i32,
    _sdl: Option<sdl2::Sdl>,
    _device: Option<sdl2::audio::AudioDevice<AudioCallback>>,
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(VecDeque::new())),
            sample_rate: 44100,
            channels: 2,
            _sdl: None,
            _device: None,
        }
    }

    pub fn start(&self, path: &str) {
        let buf = self.buffer.clone();
        let path = path.to_owned();
        std::thread::spawn(move || {
            decode_audio(&path, buf);
        });
    }

    pub fn init_sdl(&mut self) -> Result<(), String> {
        let sdl = sdl2::init().map_err(|e| e.to_string())?;
        let audio = sdl.audio().map_err(|e| e.to_string())?;

        let desired = sdl2::audio::AudioSpecDesired {
            freq: Some(self.sample_rate),
            channels: Some(self.channels as u8),
            samples: None,
        };

        let buf = self.buffer.clone();
        let device = audio.open_playback(None, &desired, move |_| {
            AudioCallback { buffer: buf.clone() }
        })?;

        device.resume();
        self._sdl = Some(sdl);
        self._device = Some(device);
        Ok(())
    }
}

pub struct AudioCallback {
    buffer: Arc<Mutex<VecDeque<f32>>>,
}

impl sdl2::audio::AudioCallback for AudioCallback {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        let mut buf = self.buffer.lock().unwrap();
        for sample in out.iter_mut() {
            *sample = buf.pop_front().unwrap_or(0.0);
        }
    }
}

fn decode_audio(path: &str, out_buffer: Arc<Mutex<VecDeque<f32>>>) {
    use ffmpeg_sys_next::*;
    unsafe {
        let path_c = CString::new(path).unwrap();
        let mut fmt_ctx: *mut AVFormatContext = ptr::null_mut();

        if avformat_open_input(&mut fmt_ctx, path_c.as_ptr(), ptr::null_mut(), ptr::null_mut()) < 0 {
            eprintln!("audio: cannot open '{}'", path);
            return;
        }
        if avformat_find_stream_info(fmt_ctx, ptr::null_mut()) < 0 {
            eprintln!("audio: cannot find stream info");
            avformat_close_input(&mut fmt_ctx);
            return;
        }

        let nb = (*fmt_ctx).nb_streams as usize;
        let streams = std::slice::from_raw_parts((*fmt_ctx).streams, nb);
        let mut audio_idx = -1i32;
        for (i, &s) in streams.iter().enumerate() {
            if (*(*s).codecpar).codec_type == AVMediaType::AVMEDIA_TYPE_AUDIO {
                audio_idx = i as i32;
                break;
            }
        }
        if audio_idx < 0 {
            avformat_close_input(&mut fmt_ctx);
            return;
        }

        let as_ = *streams[audio_idx as usize];
        let codec = avcodec_find_decoder((*as_.codecpar).codec_id);
        if codec.is_null() {
            avformat_close_input(&mut fmt_ctx);
            return;
        }

        let mut codec_ctx = avcodec_alloc_context3(codec);
        if codec_ctx.is_null() {
            avformat_close_input(&mut fmt_ctx);
            return;
        }
        avcodec_parameters_to_context(codec_ctx, as_.codecpar);
        if avcodec_open2(codec_ctx, codec, ptr::null_mut()) < 0 {
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            return;
        }

        let src_rate = (*codec_ctx).sample_rate;
        let src_fmt = (*codec_ctx).sample_fmt;
        let dst_rate = 44100;
        let dst_fmt = AVSampleFormat::AV_SAMPLE_FMT_FLT;

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
            eprintln!("audio: cannot create resampler");
            avcodec_free_context(&mut codec_ctx);
            avformat_close_input(&mut fmt_ctx);
            return;
        }
        swr_init(swr);

        let mut pkt = av_packet_alloc();
        let mut frame = av_frame_alloc();

        loop {
            if av_read_frame(fmt_ctx, pkt) < 0 { break; }

            if (*pkt).stream_index != audio_idx {
                av_packet_unref(pkt);
                continue;
            }

            avcodec_send_packet(codec_ctx, pkt);
            av_packet_unref(pkt);

            while avcodec_receive_frame(codec_ctx, frame) >= 0 {
                let nb_samples = (*frame).nb_samples;
                let delay = swr_get_delay(swr, src_rate as i64);
                let dst_nb = av_rescale_rnd(
                    delay + nb_samples as i64,
                    dst_rate as i64,
                    src_rate as i64,
                    AVRounding::AV_ROUND_UP,
                ) as i32;

                let dst_buf_size = av_samples_get_buffer_size(
                    ptr::null_mut(),
                    2,
                    dst_nb,
                    dst_fmt,
                    1,
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
                    let total = (converted * 2) as usize;
                    let samples = std::slice::from_raw_parts(dst_buf as *const f32, total);
                    let mut buf = out_buffer.lock().unwrap();
                    if buf.len() < 44100 * 10 {
                        buf.extend(samples.iter());
                    }
                }

                av_free(dst_buf as *mut libc::c_void);
            }
        }

        swr_free(&mut swr);
        av_channel_layout_uninit(&mut dst_layout);
        av_frame_free(&mut frame);
        av_packet_free(&mut pkt);
        avcodec_free_context(&mut codec_ctx);
        avformat_close_input(&mut fmt_ctx);
    }
}
