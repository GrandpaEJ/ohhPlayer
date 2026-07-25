pub fn format_time(pos: f64, dur: f64) -> String {
    let pos_s = pos as u32;
    let dur_s = dur as u32;
    format!(
        "{:02}:{:02}:{:02} / {:02}:{:02}:{:02}",
        pos_s / 3600,
        (pos_s % 3600) / 60,
        pos_s % 60,
        dur_s / 3600,
        (dur_s % 3600) / 60,
        dur_s % 60,
    )
}
