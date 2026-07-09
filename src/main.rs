mod app;
mod audio;
mod decoder;
mod ui_state;

slint::include_modules!();

use std::rc::Rc;

extern "C" fn sigint(_: i32) {
    unsafe { libc::_exit(0); }
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
        decoder.start(&p, 800, 424);
        audio_out.start(&p);
    }

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
