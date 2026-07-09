pub struct OpacityState {
    pub controls_target: f32,
    pub center_target: f32,
    pub needs_slider_update: bool,
    pub slider_val: f32,
}

pub fn compute_opacity(
    playing: bool,
    idle_secs: f32,
    cur_ctrl: f32,
    cur_center: f32,
    dur: f64,
    pos: f64,
    last_slider_set: f64,
) -> OpacityState {
    let controls_target = if playing && idle_secs > 3.0 { 0.25 } else { 1.0 };
    let center_target = if playing && idle_secs > 3.0 { 0.0 } else if playing { 0.0 } else { 1.0 };

    let new_ctrl = (cur_ctrl + (controls_target - cur_ctrl) * 0.06).clamp(0.0, 1.0);
    let new_center = (cur_center + (center_target - cur_center) * 0.05).clamp(0.0, 1.0);

    let mut needs_slider = false;
    let mut slider_val = 0.0;

    if dur > 0.0 {
        let val = (pos / dur) as f32;
        if (val as f64 - last_slider_set).abs() > 0.005 {
            needs_slider = true;
            slider_val = val;
        }
    }

    OpacityState {
        controls_target: new_ctrl,
        center_target: new_center,
        needs_slider_update: needs_slider,
        slider_val,
    }
}
