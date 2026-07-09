pub struct DecodedFrame {
    pub buffer: slint::SharedPixelBuffer<slint::Rgb8Pixel>,
}

pub struct DecoderCommand {
    pub seek_target: Option<f64>,
    pub playing: bool,
    pub quit: bool,
    pub speed: f32,
}

impl Default for DecoderCommand {
    fn default() -> Self {
        Self { seek_target: None, playing: true, quit: false, speed: 1.0 }
    }
}

#[derive(Clone, Default)]
pub struct DecoderState {
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
}
