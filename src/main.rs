mod audio;
mod decoder;
mod ui_state;

slint::include_modules!();

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

extern "C" fn sigint(_: i32) {
    unsafe { libc::_exit(0); }
}

fn main() {
    unsafe { libc::signal(libc::SIGINT, sigint as *const () as libc::sighandler_t); }
    let app      = AppWindow::new().unwrap();
    let app_weak = app.as_weak();

    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <video_file>", args[0]);
        std::process::exit(1);
    }
    let path = &args[1];

    let decoder = decoder::Decoder::new();
    decoder.start(path, 800, 424);

    let mut audio_out = audio::AudioOutput::new();
    audio_out.start(path);
    let _ = audio_out.init_sdl();

    // Single shared state blob for audio (volume, playing, seek)
    let audio_shared: Arc<Mutex<audio::AudioShared>> = audio_out.shared.clone();

    let cmd   = decoder.command();
    let state = decoder.state();
    let frame = decoder.frame();
    let ui    = Rc::new(ui_state::UiState::new());

    // ── Play / Pause ───────────────────────────────────────────────────────
    {
        let cmd_p     = cmd.clone();
        let state_p   = state.clone();
        let audio_p   = audio_shared.clone();
        let act_p     = ui.last_activity.clone();
        app.on_play_paused(move || {
            let st = state_p.lock().unwrap();
            let playing = !st.playing;
            drop(st);
            cmd_p.lock().unwrap().playing            = playing;
            audio_p.lock().unwrap().playing          = playing;
            *act_p.borrow_mut() = std::time::Instant::now();
        });
    }

    // ── Seek (absolute) ───────────────────────────────────────────────────
    {
        let cmd_s   = cmd.clone();
        let audio_s = audio_shared.clone();
        let state_s = state.clone();
        let cd      = ui.seek_cooldown.clone();
        let act     = ui.last_activity.clone();
        app.on_seeked(move |val| {
            *cd.borrow_mut()  = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            if let Ok(st) = state_s.lock() {
                if st.duration > 0.0 {
                    let target = val as f64 * st.duration;
                    cmd_s.lock().unwrap().seek_target  = Some(target);
                    // Bug #4 fix: tell audio decoder to seek too
                    audio_s.lock().unwrap().seek_to    = Some(target);
                }
            }
        });
    }

    // ── Seek relative (±seconds) ───────────────────────────────────────────
    {
        let cmd_r   = cmd.clone();
        let audio_r = audio_shared.clone();
        let state_r = state.clone();
        let cd      = ui.seek_cooldown.clone();
        let act     = ui.last_activity.clone();
        app.on_seek_relative(move |delta| {
            *cd.borrow_mut()  = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            if let Ok(st) = state_r.lock() {
                let target = (st.position + delta as f64).max(0.0).min(st.duration);
                cmd_r.lock().unwrap().seek_target  = Some(target);
                // Bug #4 fix: seek audio too
                audio_r.lock().unwrap().seek_to    = Some(target);
            }
        });
    }

    // ── Controls activity ─────────────────────────────────────────────────
    {
        let act = ui.last_activity.clone();
        app.on_controls_moved(move || {
            *act.borrow_mut() = std::time::Instant::now();
        });
    }

    // ── Fullscreen toggle ─────────────────────────────────────────────────
    {
        let weak = app_weak.clone();
        let act  = ui.last_activity.clone();
        app.on_fullscreen_toggled(move || {
            *act.borrow_mut() = std::time::Instant::now();
            if let Some(w) = weak.upgrade() {
                let fs = !w.window().is_fullscreen();
                w.window().set_fullscreen(fs);
                w.set_is_fullscreen(fs);
            }
        });
    }

    // ── Volume mute toggle ────────────────────────────────────────────────
    {
        let audio_v = audio_shared.clone();
        let act     = ui.last_activity.clone();
        let saved   = Rc::new(RefCell::new(0.8f32));
        app.on_volume_toggled(move || {
            *act.borrow_mut() = std::time::Instant::now();
            let mut s = audio_v.lock().unwrap();
            if s.volume > 0.0 {
                *saved.borrow_mut() = s.volume;
                s.volume = 0.0;
            } else {
                s.volume = *saved.borrow();
            }
        });
    }

    // ── Volume slider ─────────────────────────────────────────────────────
    {
        let audio_v = audio_shared.clone();
        let act     = ui.last_activity.clone();
        app.on_volume_changed(move |new_vol| {
            *act.borrow_mut() = std::time::Instant::now();
            audio_v.lock().unwrap().volume = new_vol;
        });
    }

    // ── Playback speed ────────────────────────────────────────────────────
    {
        let cmd_spd = cmd.clone();
        let weak    = app_weak.clone();
        let act     = ui.last_activity.clone();
        app.on_speed_changed(move |spd| {
            *act.borrow_mut() = std::time::Instant::now();
            cmd_spd.lock().unwrap().speed = spd;
            if let Some(w) = weak.upgrade() {
                w.set_speed(spd);
            }
        });
    }

    // ── UI refresh timer (16 ms ≈ 60 fps) ────────────────────────────────
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let a = match app_weak.upgrade() {
                Some(a) => a,
                None    => return,
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
            let op   = ui_state::compute_opacity(
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
            a.set_volume_level(audio_shared.lock().unwrap().volume);
        },
    );

    app.run().unwrap();
    decoder.command().lock().unwrap().quit = true;
}
