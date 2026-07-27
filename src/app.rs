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
    settings: Rc<RefCell<crate::settings::AppSettings>>,
    initial_path: Option<String>,
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

    // ── Seek (Dragging - Throttled Video Preview) ─────────────────────────
    {
        let cmd_s   = cmd.clone();
        let state_s = state.clone();
        let cd      = ui.seek_cooldown.clone();
        let act     = ui.last_activity.clone();
        let last_seek = Rc::new(RefCell::new(std::time::Instant::now()));
        app.on_seek_dragging(move |frac| {
            *cd.borrow_mut()  = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            
            let mut last = last_seek.borrow_mut();
            if last.elapsed().as_millis() > 150 {
                *last = std::time::Instant::now();
                if let Ok(st) = state_s.lock() {
                    if st.duration > 0.0 {
                        let target = frac as f64 * st.duration;
                        // ONLY seek video thread during dragging for fast preview
                        cmd_s.lock().unwrap().seek_target = Some(target);
                    }
                }
            }
        });
    }

    // ── Seek (Ended - Final Full A/V Sync) ────────────────────────────────
    {
        let cmd_s   = cmd.clone();
        let audio_s = audio_shared.clone();
        let state_s = state.clone();
        let cd      = ui.seek_cooldown.clone();
        let act     = ui.last_activity.clone();
        app.on_seek_ended(move |frac| {
            *cd.borrow_mut()  = std::time::Instant::now();
            *act.borrow_mut() = std::time::Instant::now();
            if let Ok(st) = state_s.lock() {
                if st.duration > 0.0 {
                    let target = frac as f64 * st.duration;
                    cmd_s.lock().unwrap().seek_target  = Some(target);
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
        let weak    = app_weak.clone();

        // Stores: (target_position, last_event_time, hold_start_time)
        let pending_seek: Rc<RefCell<Option<(f64, std::time::Instant, std::time::Instant)>>> = Rc::new(RefCell::new(None));

        app.on_seek_relative(move |delta| {
            let now = std::time::Instant::now();
            *cd.borrow_mut()  = now;
            *act.borrow_mut() = now;

            if let Ok(st) = state_r.lock() {
                let (base_pos, start_time) = if let Some((prev_target, prev_time, first_start)) = *pending_seek.borrow() {
                    if now.duration_since(prev_time).as_millis() < 600 {
                        (prev_target, first_start)
                    } else {
                        (st.position, now)
                    }
                } else {
                    (st.position, now)
                };

                let hold_duration_secs = now.duration_since(start_time).as_secs_f64();
                let delta_f64 = delta as f64;
                let dir = if delta_f64 >= 0.0 { 1.0 } else { -1.0 };
                let abs_delta = delta_f64.abs();

                // Guaranteed full step (5s/10s) on every click. Accelerates to 10s/15s on long hold.
                let step_val = if hold_duration_secs >= 5.0 {
                    abs_delta.max(15.0)
                } else if hold_duration_secs >= 2.0 {
                    abs_delta.max(10.0)
                } else {
                    abs_delta // Always full 5s / 10s per click!
                };

                let step = step_val * dir;
                let target = (base_pos + step).max(0.0).min(st.duration);
                *pending_seek.borrow_mut() = Some((target, now, start_time));

                cmd_r.lock().unwrap().seek_target  = Some(target);
                audio_r.lock().unwrap().seek_to    = Some(target);

                if let Some(w) = weak.upgrade() {
                    let time_str = crate::ui_state::format_time(target, st.duration);
                    let sign     = if step >= 0.0 { "+" } else { "" };
                    let mode_tag = if hold_duration_secs >= 5.0 {
                        " (15s Boost)"
                    } else if hold_duration_secs >= 2.0 {
                        " (10s Fast)"
                    } else {
                        ""
                    };

                    w.set_osd_text(slint::SharedString::from(format!("Seek: {} ({}{}s{})", time_str, sign, step as i32, mode_tag)));
                    w.set_osd_opacity(1.0);
                }
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
    let current_file: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(initial_path));
    {
        let cmd_q = cmd.clone();
        let audio_q = audio_shared.clone();
        let set_q = settings.clone();
        let cur_file_q = current_file.clone();
        let state_q = state.clone();
        app.on_close_window(move || {
            // Save final position
            if let Some(ref cf) = *cur_file_q.borrow() {
                let pos = state_q.lock().unwrap().position;
                let dur = state_q.lock().unwrap().duration;
                let pos_to_save = if dur > 0.0 && pos >= dur - 2.0 { 0.0 } else { pos };
                set_q.borrow_mut().save_position(cf, pos_to_save);
            }
            
            cmd_q.lock().unwrap().quit = true;
            audio_q.lock().unwrap().quit = true;
            std::process::exit(0);
        });
    }

    // ── Open File & Recent ───────────────────────────────────────────────
    {
        let cmd_o = cmd.clone();
        let audio_o = audio_shared.clone();
        let state_o = state.clone();
        let weak = app_weak.clone();
        let set = settings.clone();
        let cur_file = current_file.clone();
        app.on_open_file(move || {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Video", &["mp4", "mkv", "avi", "webm", "mov"])
                .pick_file() 
            {
                let p = path.to_string_lossy().to_string();
                
                // Save current file's position before switching
                if let Some(ref cf) = *cur_file.borrow() {
                    let pos = state_o.lock().unwrap().position;
                    let dur = state_o.lock().unwrap().duration;
                    let pos_to_save = if dur > 0.0 && pos >= dur - 2.0 { 0.0 } else { pos };
                    set.borrow_mut().save_position(cf, pos_to_save);
                }
                *cur_file.borrow_mut() = Some(p.clone());

                crate::save_to_history(&p);
                if let Some(a) = weak.upgrade() {
                    a.set_recent_files(crate::get_history_model());
                }

                let resume_pos = set.borrow().get_position(&p);

                let mut c = cmd_o.lock().unwrap();
                let mut au = audio_o.lock().unwrap();
                c.load_file = Some(p.clone());
                au.load_file = Some(p);
                
                if resume_pos > 0.0 {
                    c.seek_target = Some(resume_pos);
                    au.seek_to = Some(resume_pos);
                }
            }
        });

        let cmd_r = cmd.clone();
        let audio_r = audio_shared.clone();
        let state_r = state.clone();
        let weak2 = app_weak.clone();
        let set2 = settings.clone();
        let cur_file2 = current_file.clone();
        app.on_open_recent_file(move |path| {
            let p = path.as_str().to_string();
            
            // Save current file's position before switching
            if let Some(ref cf) = *cur_file2.borrow() {
                let pos = state_r.lock().unwrap().position;
                let dur = state_r.lock().unwrap().duration;
                let pos_to_save = if dur > 0.0 && pos >= dur - 2.0 { 0.0 } else { pos };
                set2.borrow_mut().save_position(cf, pos_to_save);
            }
            *cur_file2.borrow_mut() = Some(p.clone());

            crate::save_to_history(&p);
            if let Some(a) = weak2.upgrade() {
                a.set_recent_files(crate::get_history_model());
            }

            let resume_pos = set2.borrow().get_position(&p);

            let mut c = cmd_r.lock().unwrap();
            let mut au = audio_r.lock().unwrap();
            c.load_file = Some(p.clone());
            au.load_file = Some(p);

            if resume_pos > 0.0 {
                c.seek_target = Some(resume_pos);
                au.seek_to = Some(resume_pos);
            }
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
        let weak    = app_weak.clone();
        let set     = settings.clone();
        app.on_volume_changed(move |new_vol| {
            *act.borrow_mut() = std::time::Instant::now();
            audio_v.lock().unwrap().volume = new_vol;
            set.borrow_mut().volume = new_vol;
            set.borrow().save();
            
            if let Some(w) = weak.upgrade() {
                w.set_osd_text(slint::SharedString::from(format!("Volume: {}%", (new_vol * 100.0) as i32)));
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Playback speed ────────────────────────────────────────────────────
    {
        let cmd_spd = cmd.clone();
        let weak    = app_weak.clone();
        let act     = ui.last_activity.clone();
        let set     = settings.clone();
        app.on_speed_changed(move |spd| {
            *act.borrow_mut() = std::time::Instant::now();
            cmd_spd.lock().unwrap().speed = spd;
            set.borrow_mut().speed = spd;
            set.borrow().save();
            if let Some(w) = weak.upgrade() {
                w.set_speed(spd);
                w.set_osd_text(slint::SharedString::from(format!("Speed: {:.1}x", spd)));
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Always On Top ─────────────────────────────────────────────────────
    {
        let weak = app_weak.clone();
        let set  = settings.clone();
        app.on_always_on_top_toggled(move || {
            let mut s = set.borrow_mut();
            s.always_on_top = !s.always_on_top;
            s.save();
            if let Some(w) = weak.upgrade() {
                w.set_my_always_on_top(s.always_on_top);
                w.set_osd_text(slint::SharedString::from(if s.always_on_top { "Pinned" } else { "Unpinned" }));
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Picture-in-Picture (PiP) Mode ─────────────────────────────────────
    {
        let weak = app_weak.clone();
        let set  = settings.clone();
        let act_pip = ui.last_activity.clone();
        let ctrl_op_pip = ui.controls_opacity.clone();
        let center_op_pip = ui.center_opacity.clone();
        let saved_size = Rc::new(RefCell::new((900u32, 520u32)));
        app.on_pip_toggled(move || {
            if let Some(w) = weak.upgrade() {
                let is_pip = !w.get_is_pip();
                w.set_is_pip(is_pip);
                
                if is_pip {
                    // Enter PiP mode: shrink window, pin on top
                    let size = w.window().size();
                    if size.width > 360 && size.height > 200 {
                        *saved_size.borrow_mut() = (size.width, size.height);
                    }
                    w.window().set_size(slint::PhysicalSize::new(360, 202));
                    w.set_my_always_on_top(true);
                    w.set_osd_text(slint::SharedString::from("PiP Mode Enabled"));
                    
                    // Hide controls immediately on entering PiP mode unless mouse moves
                    *act_pip.borrow_mut() = std::time::Instant::now() - std::time::Duration::from_secs(10);
                    *ctrl_op_pip.borrow_mut() = 0.0;
                    *center_op_pip.borrow_mut() = 0.0;
                    w.set_controls_opacity(0.0);
                    w.set_center_btn_opacity(0.0);

                    // Dispatch IPC command for tiling WMs (Niri, Hyprland, Sway, i3) to force floating
                    toggle_wm_floating(true);
                } else {
                    // Exit PiP mode: restore saved size & original always-on-top state
                    let (sw, sh) = *saved_size.borrow();
                    w.window().set_size(slint::PhysicalSize::new(sw, sh));
                    let restore_aot = set.borrow().always_on_top;
                    w.set_my_always_on_top(restore_aot);
                    w.set_osd_text(slint::SharedString::from("PiP Mode Disabled"));

                    // Dispatch IPC command to return window to tiled mode if applicable
                    toggle_wm_floating(false);
                }
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Scale Mode ────────────────────────────────────────────────────────
    {
        let set = settings.clone();
        let weak = app_weak.clone();
        app.on_scale_mode_changed(move |mode| {
            set.borrow_mut().scale_mode = mode;
            set.borrow().save();
            let label = match mode {
                0 => "Fit (Letterbox)",
                1 => "Stretch",
                2 => "Zoom (Crop)",
                3 => "100%",
                4 => "1:1",
                5 => "16:9",
                _ => "9:16",
            };
            if let Some(w) = weak.upgrade() {
                w.set_osd_text(slint::SharedString::from(format!("Scale: {}", label)));
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Audio Delay Adjustment ────────────────────────────────────────────
    {
        let set = settings.clone();
        let weak = app_weak.clone();
        let audio = audio_shared.clone();
        app.on_audio_delay_changed(move |step_ms| {
            let mut s = set.borrow_mut();
            s.audio_delay_ms += step_ms;
            let current_ms = s.audio_delay_ms;
            s.save();

            audio.lock().unwrap().audio_delay_secs = current_ms as f64 / 1000.0;

            if let Some(w) = weak.upgrade() {
                let osd_str = if current_ms == 0 {
                    "Audio Delay: 0 ms (Synced)".to_string()
                } else if current_ms > 0 {
                    format!("Audio Delay: +{} ms", current_ms)
                } else {
                    format!("Audio Delay: {} ms", current_ms)
                };
                w.set_osd_text(slint::SharedString::from(osd_str));
                w.set_osd_opacity(1.0);
            }
        });
    }

    {
        let set = settings.clone();
        let weak = app_weak.clone();
        let audio = audio_shared.clone();
        app.on_audio_delay_reset(move || {
            let mut s = set.borrow_mut();
            s.audio_delay_ms = 0;
            s.save();

            audio.lock().unwrap().audio_delay_secs = 0.0;

            if let Some(w) = weak.upgrade() {
                w.set_osd_text(slint::SharedString::from("Audio Delay Reset (0 ms)"));
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── Sleep Timer ───────────────────────────────────────────────────────
    let sleep_target: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));
    {
        let st = sleep_target.clone();
        let weak = app_weak.clone();
        app.on_sleep_timer_changed(move |mins| {
            if mins > 0 {
                *st.borrow_mut() = Some(std::time::Instant::now() + std::time::Duration::from_secs(mins as u64 * 60));
            } else {
                *st.borrow_mut() = None;
            }
            if let Some(w) = weak.upgrade() {
                if mins > 0 {
                    w.set_osd_text(slint::SharedString::from(format!("Sleep timer: {}m", mins)));
                } else {
                    w.set_osd_text(slint::SharedString::from("Sleep timer: Off"));
                }
                w.set_osd_opacity(1.0);
            }
        });
    }

    // ── UI refresh timer (16 ms ≈ 60 fps) ────────────────────────────────
    let debug_state = Rc::new(RefCell::new((
        std::time::Instant::now(),
        0_u32,
        slint::SharedString::from(""),
    )));

    let mut last_w = 0_u32;
    let mut last_h = 0_u32;
    let cmd_timer = cmd.clone();
    let mut last_save_time = std::time::Instant::now();
    let set_timer = settings.clone();

    let mut last_ui_time_str = String::new();
    let mut last_ui_playing = !state.lock().unwrap().playing;
    let mut last_ui_pos = -10.0_f32;
    let mut last_ui_dur = -10.0_f32;
    let mut last_ui_vol = -10.0_f32;
    let mut last_ctrl_op = -1.0_f32;
    let mut last_center_op = -1.0_f32;

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let a = match app_weak.upgrade() {
                Some(a) => a,
                None    => return,
            };

            // ── OSD Fading ───────────────────────────────────────────────
            let current_osd_op = a.get_osd_opacity();
            if current_osd_op > 0.0 {
                let new_op = (current_osd_op - 0.015).max(0.0);
                a.set_osd_opacity(new_op);
            }

            // ── Sleep Timer Check ────────────────────────────────────────
            if let Some(target) = *sleep_target.borrow() {
                if std::time::Instant::now() >= target {
                    *sleep_target.borrow_mut() = None;
                    cmd_timer.lock().unwrap().quit = true;
                    std::process::exit(0);
                }
            }

            if let Some(f) = frame.lock().unwrap().take() {
                a.set_frame(slint::Image::from_rgb8(f.buffer));
            }

            let (pos, dur, playing, vw, vh) = {
                let st = state.lock().unwrap();
                (st.position, st.duration, st.playing, st.video_width, st.video_height)
            };

            // Periodically save file position (every 5 seconds)
            if playing && last_save_time.elapsed().as_secs() >= 5 {
                last_save_time = std::time::Instant::now();
                if let Some(ref cf) = *current_file.borrow() {
                    let pos_to_save = if dur > 0.0 && pos >= dur - 2.0 { 0.0 } else { pos };
                    set_timer.borrow_mut().save_position(cf, pos_to_save);
                }
            }

            // Auto-scale window to video aspect ratio when dimensions change
            if vw > 0 && vh > 0 && (vw != last_w || vh != last_h) {
                last_w = vw;
                last_h = vh;
                
                if !a.window().is_fullscreen() {
                    let mut win_w = vw as f32;
                    let mut win_h = vh as f32;
                    
                    let max_w = 1280.0;
                    let max_h = 720.0;
                    let min_w = 400.0;
                    
                    if win_w > max_w || win_h > max_h {
                        let scale_w = max_w / win_w;
                        let scale_h = max_h / win_h;
                        let scale = scale_w.min(scale_h);
                        win_w *= scale;
                        win_h *= scale;
                    }
                    if win_w < min_w {
                        let scale = min_w / win_w;
                        win_w *= scale;
                        win_h *= scale;
                    }
                    
                    a.window().set_size(slint::PhysicalSize::new(win_w as u32, win_h as u32));
                }
            }

            let time_str = ui_state::format_time(pos, dur);
            if time_str != last_ui_time_str {
                last_ui_time_str = time_str.clone();
                a.set_time_text(slint::SharedString::from(time_str));
            }

            let idle = ui.last_activity.borrow().elapsed().as_secs_f32();
            let is_pip = a.get_is_pip();
            let op   = ui_state::compute_opacity(
                playing,
                is_pip,
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

            if (op.controls_target - last_ctrl_op).abs() > 0.001 {
                last_ctrl_op = op.controls_target;
                a.set_controls_opacity(op.controls_target);
            }
            *ui.controls_opacity.borrow_mut() = op.controls_target;

            if (op.center_target - last_center_op).abs() > 0.001 {
                last_center_op = op.center_target;
                a.set_center_btn_opacity(op.center_target);
            }
            *ui.center_opacity.borrow_mut() = op.center_target;

            if playing != last_ui_playing {
                last_ui_playing = playing;
                a.set_playing(playing);
            }
            let pos_f32 = pos as f32;
            if (pos_f32 - last_ui_pos).abs() > 0.01 {
                last_ui_pos = pos_f32;
                a.set_position(pos_f32);
            }
            let dur_f32 = dur as f32;
            if (dur_f32 - last_ui_dur).abs() > 0.01 {
                last_ui_dur = dur_f32;
                a.set_duration(dur_f32);
            }
            let vol = audio_shared.lock().unwrap().volume;
            if (vol - last_ui_vol).abs() > 0.001 {
                last_ui_vol = vol;
                a.set_volume_level(vol);
            }

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

            // ── Update Logs if Overlay is open ───────────────────────────────
            if a.get_logs_open() {
                let logs = crate::logger::global_logs().lock().unwrap();
                let mut lines = Vec::new();
                for log in logs.iter() {
                    lines.push(slint::SharedString::from(log));
                }
                a.set_log_lines(slint::ModelRc::new(slint::VecModel::from(lines)));
            }
        },
    );

    timer
}

fn toggle_wm_floating(enable: bool) {
    if std::env::var_os("NIRI_SOCKET").is_some() {
        let _ = std::process::Command::new("niri")
            .args(["msg", "action", "toggle-window-floating"])
            .spawn();
    } else if std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        let _ = std::process::Command::new("hyprctl")
            .args(["dispatch", "togglefloating"])
            .spawn();
    } else if std::env::var_os("SWAYSOCK").is_some() {
        let arg = if enable { "floating enable" } else { "floating disable" };
        let _ = std::process::Command::new("swaymsg")
            .arg(arg)
            .spawn();
    } else if std::env::var_os("I3SOCK").is_some() {
        let arg = if enable { "floating enable" } else { "floating disable" };
        let _ = std::process::Command::new("i3-msg")
            .arg(arg)
            .spawn();
    }
}
