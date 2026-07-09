use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::audio::AudioShared;
use crate::decoder::{DecoderCommand, DecoderState, DecodedFrame};
use crate::ui_state;
use crate::AppWindow;
use slint::ComponentHandle;

pub fn setup_callbacks(
    app: &AppWindow,
    app_weak: slint::Weak<AppWindow>,
    cmd: Arc<Mutex<DecoderCommand>>,
    state: Arc<Mutex<DecoderState>>,
    frame: Arc<Mutex<Option<DecodedFrame>>>,
    audio_shared: Arc<Mutex<AudioShared>>,
    ui: Rc<ui_state::UiState>,
) -> slint::Timer {
    // ── Play / Pause ───────────────────────────────────────────────────────
    {
        let cmd_p     = cmd.clone();
        let state_p   = state.clone();
        let audio_p   = audio_shared.clone();
        let act_p     = ui.last_activity.clone();
        app.on_play_paused(move || {
            let st = state_p.lock().unwrap();
            let playing = !st.playing;
            let at_end = st.duration > 0.0 && st.position >= st.duration - 0.1;
            drop(st);
            
            if playing && at_end {
                cmd_p.lock().unwrap().seek_target = Some(0.0);
                audio_p.lock().unwrap().seek_to   = Some(0.0);
            }
            
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

    // ── Close window (ESC / Q / native X button) ─────────────────────────
    {
        let cmd_q = cmd.clone();
        let audio_q = audio_shared.clone();
        app.on_close_window(move || {
            cmd_q.lock().unwrap().quit = true;
            audio_q.lock().unwrap().quit = true;
            std::process::exit(0);
        });
    }

    // ── Open File & Recent ───────────────────────────────────────────────
    {
        app.on_open_file(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Video", &["mp4", "mkv", "avi", "webm", "mov"])
                .pick_file() 
            {
                if let Ok(exe) = std::env::current_exe() {
                    let _ = std::process::Command::new(exe).arg(path).spawn();
                    std::process::exit(0);
                }
            }
        });
        app.on_open_recent(move || {
            println!("Recent files feature coming soon!");
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
    let debug_state = Rc::new(RefCell::new((
        std::time::Instant::now(),
        0_u32,
        slint::SharedString::from(""),
    )));

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

            // ── Debug Overlay Update ─────────────────────────────────────────
            let mut ds = debug_state.borrow_mut();
            ds.1 += 1;
            let elapsed = ds.0.elapsed().as_secs_f32();
            if elapsed >= 1.0 {
                let fps = ds.1 as f32 / elapsed;
                ds.1 = 0;
                ds.0 = std::time::Instant::now();
                
                let mut ram_mb = 0.0;
                if let Ok(statm) = std::fs::read_to_string("/proc/self/statm") {
                    if let Some(res_pages) = statm.split_whitespace().nth(1) {
                        if let Ok(pages) = res_pages.parse::<u64>() {
                            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
                            ram_mb = (pages * page_size) as f32 / 1_048_576.0;
                        }
                    }
                }
                
                ds.2 = slint::SharedString::from(format!("{:>4.1} FPS  | {:>5.1} MB", fps, ram_mb));
            }
            a.set_debug_text(ds.2.clone());
        },
    );

    timer
}
