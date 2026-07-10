mod decode;
mod types;

pub use types::{DecodedFrame, DecoderCommand, DecoderState};

use std::sync::{Arc, Mutex};

use crate::audio::AudioShared;

pub struct Decoder {
    command: Arc<Mutex<DecoderCommand>>,
    state:   Arc<Mutex<DecoderState>>,
    frame:   Arc<Mutex<Option<DecodedFrame>>>,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            command: Arc::new(Mutex::new(DecoderCommand::default())),
            state:   Arc::new(Mutex::new(DecoderState::default())),
            frame:   Arc::new(Mutex::new(None)),
        }
    }

    pub fn command(&self) -> Arc<Mutex<DecoderCommand>> { self.command.clone() }
    pub fn state(&self)   -> Arc<Mutex<DecoderState>>   { self.state.clone()   }
    pub fn frame(&self)   -> Arc<Mutex<Option<DecodedFrame>>> { self.frame.clone() }

    pub fn start(&self, path: &str, target_w: u32, target_h: u32, audio_shared: Arc<Mutex<AudioShared>>) {
        let cmd   = self.command.clone();
        let state = self.state.clone();
        let frame = self.frame.clone();
        let path  = path.to_owned();
        std::thread::spawn(move || {
            decode::decode_video(&path, target_w, target_h, cmd, state, frame, audio_shared);
        });
    }
}
