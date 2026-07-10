use std::collections::VecDeque;

/// State shared between main thread and the audio decode/SDL threads.
pub struct AudioShared {
    pub buffer:   VecDeque<f32>,
    pub volume:   f32,
    pub playing:  bool,
    /// When Some(t), the decode loop should seek to `t` seconds.
    pub seek_to:  Option<f64>,
    pub quit:     bool,
    pub load_file: Option<String>,
    /// Audio playback position in seconds (master clock for A/V sync).
    pub audio_position_secs: f64,
}

impl AudioShared {
    pub(crate) fn new() -> Self {
        Self {
            buffer:   VecDeque::new(),
            volume:   0.8,
            playing:  true,
            seek_to:  None,
            quit:     false,
            load_file: None,
            audio_position_secs: 0.0,
        }
    }
}
