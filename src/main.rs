mod app;
mod audio;
mod decoder;
mod ui_state;
mod settings;

slint::include_modules!();

use std::rc::Rc;
use std::cell::RefCell;

extern "C" fn sigint(_: i32) {
    unsafe { libc::_exit(0); }
}

fn get_history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".ohhplayer_history")
}

pub fn load_history() -> Vec<String> {
    if let Ok(content) = std::fs::read_to_string(get_history_path()) {
        content.lines().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    }
}

pub fn save_to_history(path: &str) {
    let mut history = load_history();
    history.retain(|p| p != path);
    history.insert(0, path.to_string());
    history.truncate(15);
    let _ = std::fs::write(get_history_path(), history.join("\n"));
}

pub fn get_history_model() -> slint::ModelRc<RecentFile> {
    let history = load_history();
    let mut models = Vec::new();
    for p in history {
        let name = std::path::Path::new(&p)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        models.push(RecentFile {
            path: slint::SharedString::from(p),
            name: slint::SharedString::from(name),
        });
    }
    slint::ModelRc::new(slint::VecModel::from(models))
}

fn main() {
    // Force the femtovg backend by default to drastically reduce RAM usage compared to Skia
    if std::env::var("SLINT_BACKEND").is_err() {
        std::env::set_var("SLINT_BACKEND", "winit-femtovg");
    }

    unsafe { libc::signal(libc::SIGINT, sigint as *const () as libc::sighandler_t); }
    let app      = AppWindow::new().unwrap();
    let app_weak = app.as_weak();

    let settings = Rc::new(RefCell::new(settings::AppSettings::load()));
    app.set_volume_level(settings.borrow().volume);
    app.set_speed(settings.borrow().speed);
    app.set_scale_mode(settings.borrow().scale_mode);
    app.set_my_always_on_top(settings.borrow().always_on_top);

    let args: Vec<String> = std::env::args().collect();
    let path = if args.len() >= 2 { Some(args[1].clone()) } else { None };

    let decoder = decoder::Decoder::new();
    let mut audio_out = audio::AudioOutput::new();
    let _ = audio_out.init_sdl();

    let start_path = path.clone().unwrap_or_else(|| "".to_string());
    if !start_path.is_empty() {
        save_to_history(&start_path);
    }
    decoder.start(&start_path, 800, 424, audio_out.shared.clone());
    audio_out.start(&start_path);

    // Apply initial resume position if applicable
    if !start_path.is_empty() {
        let resume_pos = settings.borrow().get_position(&start_path);
        if resume_pos > 0.0 {
            decoder.command().lock().unwrap().seek_target = Some(resume_pos);
            audio_out.shared.lock().unwrap().seek_to = Some(resume_pos);
        }
    }

    app.set_recent_files(get_history_model());

    // Single shared state blob for audio (volume, playing, seek)
    let audio_shared = audio_out.shared.clone();

    let cmd   = decoder.command();
    let state = decoder.state();
    let frame = decoder.frame();
    let ui    = Rc::new(ui_state::UiState::new());

    // Apply loaded settings to backend
    cmd.lock().unwrap().speed = settings.borrow().speed;
    audio_shared.lock().unwrap().volume = settings.borrow().volume;

    let _timer = app::setup_callbacks(&app, app_weak, cmd.clone(), state, frame, audio_shared.clone(), ui, settings.clone(), path.clone());

    app.run().unwrap();
    decoder.command().lock().unwrap().quit = true;
}
