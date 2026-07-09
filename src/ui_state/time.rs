pub fn format_time(pos: f64, dur: f64) -> String {
    format!(
        "{:02}:{:02} / {:02}:{:02}",
        (pos as u32) / 60,
        (pos as u32) % 60,
        (dur as u32) / 60,
        (dur as u32) % 60,
    )
}
