mod audio;
mod decoder;
mod ui_state;

slint::include_modules!();

use std::cell::RefCell;
use std::rc::Rc;

fn main() {
    let app = AppWindow::new().unwrap();
    let app_weak = app.as_weak();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_file>", args[0]);
        std::process::exit(1);
    }
    let path = &args[1];

    let decoder = decoder::Decoder::new();
    decoder.start(path, 800, 424);

    let audio_out = audio::AudioOutput::new();
    audio_out.start(path);
    let _audio_device = audio_out.init_sdl().ok();

    let cmd = decoder.command();
    let state = decoder.state();
    let frame = decoder.frame();
    let ui = Rc::new(ui_state::UiState::new());
    let vol = Rc::new(RefCell::new(0.8f32));

    let cmd_p = cmd.clone();
    let state_p = state.clone();
    let act_p = ui.last_activity.clone();
    app.on_play_paused(move || {
        let mut c = cmd_p.lock().unwrap();
        let st = state_p.lock().unwrap();
        c.playing = !st.playing;
        *act_p.borrow_mut() = std::time::Instant::now();
    });

    {
        let cmd_s = cmd.clone();
        let state_s = state.clone();
        let cd = ui.seek_cooldown.clone();
        let act = ui.last_activity.clone();
        app.on_seeked(move |val| {
            *cd.borrow_mut() = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            if let Ok(st) = state_s.lock() {
                if st.duration > 0.0 {
                    cmd_s.lock().unwrap().seek_target = Some(val as f64 * st.duration);
                }
            }
        });
    }

    {
        let cmd_r = cmd.clone();
        let state_r = state.clone();
        let cd = ui.seek_cooldown.clone();
        let act = ui.last_activity.clone();
        app.on_seek_relative(move |delta| {
            *cd.borrow_mut() = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            if let Ok(st) = state_r.lock() {
                let new_pos = (st.position + delta as f64).max(0.0).min(st.duration);
                cmd_r.lock().unwrap().seek_target = Some(new_pos);
            }
        });
    }

    {
        let act = ui.last_activity.clone();
        app.on_controls_moved(move || {
            *act.borrow_mut() = std::time::Instant::now();
        });
    }

    {
        let weak = app_weak.clone();
        let act = ui.last_activity.clone();
        app.on_fullscreen_toggled(move || {
            *act.borrow_mut() = std::time::Instant::now();
            if let Some(w) = weak.upgrade() {
                let fs = !w.window().is_fullscreen();
                w.window().set_fullscreen(fs);
                w.set_is_fullscreen(fs);
            }
        });
    }

    {
        let act = ui.last_activity.clone();
        app.on_close_window(move || {
            *act.borrow_mut() = std::time::Instant::now();
            std::process::exit(0);
        });
    }

    {
        let v = vol.clone();
        let act = ui.last_activity.clone();
        app.on_volume_toggled(move || {
            *act.borrow_mut() = std::time::Instant::now();
            let mut vv = v.borrow_mut();
            *vv = if *vv > 0.0 { 0.0 } else { 0.8 };
        });
    }

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

            a.set_time_text(slint::SharedString::from(ui_state::format_time(pos, dur)));

            let idle = ui.last_activity.borrow().elapsed().as_secs_f32();
            let op = ui_state::compute_opacity(
                playing,
                idle,
                *ui.controls_opacity.borrow(),
                *ui.center_opacity.borrow(),
                dur,
                pos,
                *ui.last_slider_set.borrow(),
            );

            if op.needs_slider_update {
                let in_cd = ui.seek_cooldown.borrow().elapsed()
                    < std::time::Duration::from_millis(250);
                if !in_cd {
                    a.set_block_slider_update(true);
                    a.set_slider_value(op.slider_val);
                    a.set_block_slider_update(false);
                }
                *ui.last_slider_set.borrow_mut() = op.slider_val as f64;
            }

            a.set_controls_opacity(op.controls_target);
            *ui.controls_opacity.borrow_mut() = op.controls_target;

            a.set_center_btn_opacity(op.center_target);
            *ui.center_opacity.borrow_mut() = op.center_target;

            a.set_playing(playing);
            a.set_position(pos as f32);
            a.set_duration(dur as f32);
            a.set_volume_level(*vol.borrow());
        },
    );

    app.run().unwrap();
    decoder.command().lock().unwrap().quit = true;
}
