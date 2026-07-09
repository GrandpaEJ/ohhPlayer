use std::collections::VecDeque;

/// State shared between main thread and the audio decode/SDL threads.
pub struct AudioShared {
    pub buffer:   VecDeque<f32>,
    pub volume:   f32,
    pub playing:  bool,
    /// When Some(t), the decode loop should seek to `t` seconds.
    pub seek_to:  Option<f64>,
    pub quit:     bool,
}

impl AudioShared {
    pub(crate) fn new() -> Self {
        Self {
            buffer:  VecDeque::new(),
            volume:  0.8,
            playing: true,
            seek_to: None,
            quit:    false,
        }
    }
}
