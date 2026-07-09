use std::sync::{Arc, Mutex};

use super::AudioShared;

pub struct AudioCallback {
    pub shared: Arc<Mutex<AudioShared>>,
}

impl sdl2::audio::AudioCallback for AudioCallback {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        let mut s = self.shared.lock().unwrap();
        if !s.playing {
            // Bug #3 fix: output silence when paused instead of draining buffer
            for sample in out.iter_mut() { *sample = 0.0; }
            return;
        }
        let vol = s.volume;
        for sample in out.iter_mut() {
            *sample = s.buffer.pop_front().unwrap_or(0.0) * vol;
        }
    }
}
