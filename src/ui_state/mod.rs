mod opacity;
mod time;

pub use opacity::compute_opacity;
pub use time::format_time;

use std::cell::RefCell;
use std::rc::Rc;

pub struct UiState {
    pub last_activity: Rc<RefCell<std::time::Instant>>,
    pub controls_opacity: Rc<RefCell<f32>>,
    pub center_opacity: Rc<RefCell<f32>>,
    pub last_slider_set: Rc<RefCell<f64>>,
    pub seek_cooldown: Rc<RefCell<std::time::Instant>>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            last_activity: Rc::new(RefCell::new(std::time::Instant::now())),
            controls_opacity: Rc::new(RefCell::new(1.0)),
            center_opacity: Rc::new(RefCell::new(1.0)),
            last_slider_set: Rc::new(RefCell::new(-0.01)),
            seek_cooldown: Rc::new(RefCell::new(std::time::Instant::now())),
        }
    }

}
