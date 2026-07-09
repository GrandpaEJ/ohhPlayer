mod app;
mod audio;
mod decoder;
mod ui_state;

slint::include_modules!();

use std::rc::Rc;

extern "C" fn sigint(_: i32) {
    unsafe { libc::_exit(0); }
}

fn get_history_path() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home).join(".ohhplayer_history")
}

fn load_history() -> Vec<String> {
    if let Ok(content) = std::fs::read_to_string(get_history_path()) {
        content.lines().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    }
}

fn save_to_history(path: &str) {
    let mut history = load_history();
    history.retain(|p| p != path);
    history.insert(0, path.to_string());
    history.truncate(15); // keep last 15
    let _ = std::fs::write(get_history_path(), history.join("\n"));
}

fn main() {
    unsafe { libc::signal(libc::SIGINT, sigint as *const () as libc::sighandler_t); }
    let app      = AppWindow::new().unwrap();
    let app_weak = app.as_weak();

    let args: Vec<String> = std::env::args().collect();
    let path = if args.len() >= 2 { Some(args[1].clone()) } else { None };

    let decoder = decoder::Decoder::new();
    let mut audio_out = audio::AudioOutput::new();
    let _ = audio_out.init_sdl();

    if let Some(p) = path {
        save_to_history(&p);
        decoder.start(&p, 800, 424);
        audio_out.start(&p);
    }

    let history: Vec<slint::SharedString> = load_history().into_iter().map(slint::SharedString::from).collect();
    app.set_recent_files(slint::ModelRc::new(slint::VecModel::from(history)));

    // Single shared state blob for audio (volume, playing, seek)
    let audio_shared = audio_out.shared.clone();

    let cmd   = decoder.command();
    let state = decoder.state();
    let frame = decoder.frame();
    let ui    = Rc::new(ui_state::UiState::new());

    let _timer = app::setup_callbacks(&app, app_weak, cmd, state, frame, audio_shared, ui);

    app.run().unwrap();
    decoder.command().lock().unwrap().quit = true;
}
