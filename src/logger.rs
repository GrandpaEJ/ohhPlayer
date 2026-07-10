use std::sync::{Mutex, OnceLock};
use std::ffi::CStr;
use libc::{c_char, c_int, c_void};

extern "C" {
    pub fn vsnprintf(
        s: *mut c_char,
        n: usize,
        format: *const c_char,
        arg: *mut ffmpeg_sys_next::__va_list_tag,
    ) -> c_int;
}

pub fn global_logs() -> &'static Mutex<Vec<String>> {
    static LOGS: OnceLock<Mutex<Vec<String>>> = OnceLock::new();
    LOGS.get_or_init(|| Mutex::new(Vec::new()))
}

#[macro_export]
macro_rules! app_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        println!("{}", msg); // Also print to terminal
        let mut logs = $crate::logger::global_logs().lock().unwrap();
        logs.push(msg);
        if logs.len() > 100 {
            logs.remove(0);
        }
    }};
}

unsafe extern "C" fn ffmpeg_log_callback(_ptr: *mut c_void, level: c_int, fmt: *const c_char, vl: *mut ffmpeg_sys_next::__va_list_tag) {
    if level > ffmpeg_sys_next::AV_LOG_INFO { return; }
    let mut buffer = [0u8; 1024];
    vsnprintf(buffer.as_mut_ptr() as *mut c_char, buffer.len(), fmt, vl);
    if let Ok(c_str) = CStr::from_ptr(buffer.as_ptr() as *const c_char).to_str() {
        let trimmed = c_str.trim();
        if !trimmed.is_empty() {
            let mut logs = global_logs().lock().unwrap();
            logs.push(format!("[ffmpeg] {}", trimmed));
            if logs.len() > 100 {
                logs.remove(0);
            }
        }
    }
}

pub fn init_ffmpeg_log() {
    unsafe {
        ffmpeg_sys_next::av_log_set_callback(Some(ffmpeg_log_callback));
    }
}
