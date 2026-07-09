mod decoder;

slint::include_modules!();

use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let app = AppWindow::new().unwrap();

    let decoder = decoder::Decoder::new();

    let target_w: u32 = 800;
    let target_h: u32 = 424;

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_file>", args[0]);
        std::process::exit(1);
    }
    let video_path = args[1].clone();

    decoder.start(video_path, target_w, target_h);

    let cmd = decoder.command();
    let state = decoder.state();
    let frame = decoder.frame();

    let last_slider_set = Rc::new(RefCell::new(-0.01f64));
    let seek_cooldown = Rc::new(RefCell::new(std::time::Instant::now()));
    let controls_opacity = Rc::new(RefCell::new(0.0f32));
    let center_opacity = Rc::new(RefCell::new(1.0f32));
    let last_activity = Rc::new(RefCell::new(std::time::Instant::now()));
    let la_play = last_activity.clone();

    let cmd_play = cmd.clone();
    let state_play = state.clone();
    app.on_play_paused(move || {
        let mut c = cmd_play.lock().unwrap();
        let st = state_play.lock().unwrap();
        c.playing = !st.playing;
        *la_play.borrow_mut() = std::time::Instant::now();
    });

    {
        let cmd_seek = cmd.clone();
        let state_seek = state.clone();
        let cd = seek_cooldown.clone();
        app.on_seeked(move |val| {
            *cd.borrow_mut() = std::time::Instant::now();
            let st = state_seek.lock().unwrap();
            if st.duration > 0.0 {
                let target = val as f64 * st.duration;
                cmd_seek.lock().unwrap().seek_target = Some(target);
            }
        });
    }

    {
        let cmd_rel = cmd.clone();
        let state_rel = state.clone();
        let cd = seek_cooldown.clone();
        app.on_seek_relative(move |delta| {
            *cd.borrow_mut() = std::time::Instant::now();
            let st = state_rel.lock().unwrap();
            let new_pos = (st.position + delta as f64).max(0.0).min(st.duration);
            cmd_rel.lock().unwrap().seek_target = Some(new_pos);
        });
    }

    let weak_fs = app.as_weak();
    app.on_fullscreen_toggled(move || {
        if let Some(w) = weak_fs.upgrade() {
            let fs = !w.window().is_fullscreen();
            w.window().set_fullscreen(fs);
        }
    });

    let vol = Rc::new(RefCell::new(0.8f32));
    let vol_state = vol.clone();
    app.on_volume_toggled(move || {
        let mut v = vol_state.borrow_mut();
        *v = if *v > 0.0 { 0.0 } else { 0.8 };
    });

    {
        let la = last_activity.clone();
        app.on_controls_moved(move || {
            *la.borrow_mut() = std::time::Instant::now();
        });
    }

    let app_weak = app.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let a = match app_weak.upgrade() {
                Some(a) => a,
                None => return,
            };

            if let Some(f) = frame.lock().unwrap().take() {
                let img = slint::Image::from_rgb8(
                    slint::SharedPixelBuffer::<slint::Rgb8Pixel>::clone_from_slice(
                        &f.data,
                        f.width,
                        f.height,
                    ),
                );
                a.set_frame(img);
            }

            let (pos, dur, playing) = {
                let st = state.lock().unwrap();
                (st.position, st.duration, st.playing)
            };

            a.set_time_text(slint::SharedString::from(format!(
                "{:02}:{:02} / {:02}:{:02}",
                pos as u32 / 60,
                pos as u32 % 60,
                dur as u32 / 60,
                dur as u32 % 60,
            )));

            if dur > 0.0 {
                let slider_val = pos / dur;
                if (slider_val - *last_slider_set.borrow()).abs() > 0.005 {
                    let in_cd = seek_cooldown.borrow().elapsed()
                        < std::time::Duration::from_millis(250);
                    if !in_cd {
                        a.set_block_slider_update(true);
                        a.set_slider_value(slider_val as f32);
                        a.set_block_slider_update(false);
                    }
                    *last_slider_set.borrow_mut() = slider_val;
                }
            }

            let idle = last_activity.borrow().elapsed().as_secs_f32();
            let controls_target = if playing && idle > 2.0 { 0.0 } else { 1.0 };
            let center_target = if playing { 0.0 } else { 1.0 };

            let cur_ctrl = *controls_opacity.borrow();
            let new_ctrl = cur_ctrl + (controls_target - cur_ctrl) * 0.08;
            let new_ctrl = new_ctrl.clamp(0.0, 1.0);
            a.set_controls_opacity(new_ctrl);
            *controls_opacity.borrow_mut() = new_ctrl;

            let cur_center = *center_opacity.borrow();
            let new_center = cur_center + (center_target - cur_center) * 0.06;
            let new_center = new_center.clamp(0.0, 1.0);
            a.set_center_btn_opacity(new_center);
            *center_opacity.borrow_mut() = new_center;

            a.set_playing(playing);
            a.set_position(pos as f32);
            a.set_duration(dur as f32);
            a.set_volume_level(*vol.borrow());
        },
    );

    app.run().unwrap();

    decoder.command().lock().unwrap().quit = true;
}
