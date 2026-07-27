use std::sync::{Arc, Mutex};

use super::AudioShared;

pub struct AudioCallback {
    pub shared: Arc<Mutex<AudioShared>>,
}

impl sdl2::audio::AudioCallback for AudioCallback {
    type Channel = f32;
    fn callback(&mut self, out: &mut [f32]) {
        let mut s = match self.shared.try_lock() {
            Ok(s) => s,
            Err(_) => {
                for sample in out.iter_mut() { *sample = 0.0; }
                return;
            }
        };
        if !s.playing {
            for sample in out.iter_mut() { *sample = 0.0; }
            return;
        }
        let vol = s.volume;
        let mut samples_played = 0;
        for sample in out.iter_mut() {
            if let Some(val) = s.buffer.pop_front() {
                *sample = val * vol;
                samples_played += 1;
            } else {
                *sample = 0.0;
            }
        }
        // Track audio playback position (master clock for A/V sync)
        s.audio_position_secs += samples_played as f64 / (2.0 * 44100.0);
    }
}
