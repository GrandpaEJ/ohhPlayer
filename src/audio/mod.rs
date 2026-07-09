mod callback;
mod decode;
mod types;

pub use types::AudioShared;

use std::sync::{Arc, Mutex};

pub struct AudioOutput {
    pub shared: Arc<Mutex<AudioShared>>,
    _sdl:    Option<sdl2::Sdl>,
    _device: Option<sdl2::audio::AudioDevice<callback::AudioCallback>>,
}

impl AudioOutput {
    pub fn new() -> Self {
        Self {
            shared:  Arc::new(Mutex::new(AudioShared::new())),
            _sdl:    None,
            _device: None,
        }
    }

    pub fn start(&self, path: &str) {
        let shared = self.shared.clone();
        let path   = path.to_owned();
        std::thread::spawn(move || {
            decode::decode_audio(&path, shared);
        });
    }

    pub fn init_sdl(&mut self) -> Result<(), String> {
        let sdl   = sdl2::init().map_err(|e| e.to_string())?;
        let audio = sdl.audio().map_err(|e| e.to_string())?;

        let desired = sdl2::audio::AudioSpecDesired {
            freq:     Some(44100),
            channels: Some(2),
            samples:  None,
        };

        let shared  = self.shared.clone();
        let device  = audio.open_playback(None, &desired, move |_| {
            callback::AudioCallback { shared: shared.clone() }
        })?;

        device.resume();
        self._sdl    = Some(sdl);
        self._device = Some(device);
        Ok(())
    }
}
