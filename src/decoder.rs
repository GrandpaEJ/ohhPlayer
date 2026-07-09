use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct DecodedFrame {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub struct DecoderCommand {
    pub seek_target: Option<f64>,
    pub playing: bool,
    pub quit: bool,
}

impl Default for DecoderCommand {
    fn default() -> Self {
        Self {
            seek_target: None,
            playing: true,
            quit: false,
        }
    }
}

#[derive(Clone, Default)]
pub struct DecoderState {
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
}

pub struct Decoder {
    command: Arc<Mutex<DecoderCommand>>,
    state: Arc<Mutex<DecoderState>>,
    frame: Arc<Mutex<Option<DecodedFrame>>>,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            command: Arc::new(Mutex::new(DecoderCommand::default())),
            state: Arc::new(Mutex::new(DecoderState::default())),
            frame: Arc::new(Mutex::new(None)),
        }
    }

    pub fn command(&self) -> Arc<Mutex<DecoderCommand>> {
        self.command.clone()
    }

    pub fn state(&self) -> Arc<Mutex<DecoderState>> {
        self.state.clone()
    }

    pub fn frame(&self) -> Arc<Mutex<Option<DecodedFrame>>> {
        self.frame.clone()
    }

    pub fn start(&self, path: String, target_w: u32, target_h: u32) {
        let cmd = self.command.clone();
        let state = self.state.clone();
        let frame = self.frame.clone();
        std::thread::spawn(move || {
            decode_loop(path, target_w, target_h, cmd, state, frame);
        });
    }
}

fn decode_loop(
    path: String,
    target_w: u32,
    target_h: u32,
    command: Arc<Mutex<DecoderCommand>>,
    state: Arc<Mutex<DecoderState>>,
    frame_out: Arc<Mutex<Option<DecodedFrame>>>,
) {
    use ffmpeg_next as ff;

    ff::init().ok();

    let mut ictx = match ff::format::input(&path) {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("decoder: cannot open input '{}': {}", path, e);
            return;
        }
    };

    let stream = match ictx.streams().best(ff::media::Type::Video) {
        Some(s) => s,
        None => {
            eprintln!("decoder: no video stream found");
            return;
        }
    };

    let stream_idx = stream.index();
    let tb = stream.time_base();
    let duration_secs = stream.duration() as f64 * f64::from(tb);

    {
        let mut st = state.lock().unwrap();
        st.duration = duration_secs;
    }

    let mut decoder = match ff::codec::context::Context::from_parameters(stream.parameters()) {
        Ok(ctx) => ctx.decoder().video().unwrap(),
        Err(e) => {
            eprintln!("decoder: cannot create decoder: {}", e);
            return;
        }
    };

    let mut scaler = ff::software::scaling::context::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ff::format::Pixel::RGB24,
        target_w,
        target_h,
        ff::software::scaling::flag::Flags::BILINEAR,
    )
    .unwrap();

    let mut decoded = ff::util::frame::Video::empty();
    let mut rgb_frame = ff::util::frame::Video::empty();
    let mut do_seek = false;
    let mut seek_ts: i64 = 0;

    loop {
        let (packet_stream, packet) = match ictx.packets().next() {
            Some(p) => p,
            None => break,
        };

        if packet_stream.index() != stream_idx {
            continue;
        }

        {
            let mut cmd = command.lock().unwrap();
            if cmd.quit {
                break;
            }
            if let Some(target) = cmd.seek_target.take() {
                do_seek = true;
                seek_ts = (target * f64::from(tb.1) as f64 / f64::from(tb.0) as f64) as i64;
            }
            let playing = cmd.playing;
            state.lock().unwrap().playing = playing;
            if !playing {
                drop(cmd);
                std::thread::sleep(std::time::Duration::from_millis(5));
                continue;
            }
        }

        if do_seek {
            ictx.seek(seek_ts, seek_ts..(seek_ts + 1)).ok();
            decoder.flush();
            do_seek = false;
        }

        decoder.send_packet(&packet).ok();
        while decoder.receive_frame(&mut decoded).is_ok() {
            scaler.run(&decoded, &mut rgb_frame).ok();

            let mut buf: Vec<u8> = vec![0; (target_w * target_h * 3) as usize];
            let src_data = rgb_frame.data(0);
            let src_stride = rgb_frame.stride(0);
            let dst_stride = (target_w * 3) as usize;

            for y in 0..target_h as usize {
                let src_row = y * src_stride;
                let dst_row = y * dst_stride;
                let copy_len = dst_stride.min(src_stride);
                buf[dst_row..dst_row + copy_len].copy_from_slice(&src_data[src_row..src_row + copy_len]);
            }

            if let Some(pts) = decoded.pts() {
                let pos = pts as f64 * f64::from(tb);
                state.lock().unwrap().position = pos;
            }

            *frame_out.lock().unwrap() = Some(DecodedFrame {
                data: buf,
                width: target_w,
                height: target_h,
            });
        }
    }
}
