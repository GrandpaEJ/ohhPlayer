use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub volume: f32,
    pub speed: f32,
    pub scale_mode: i32,
    pub always_on_top: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            speed: 1.0,
            scale_mode: 0,
            always_on_top: false,
        }
    }
}

impl AppSettings {
    pub fn load() -> Self {
        let path = Self::path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
        Self::default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), json);
        }
    }

    fn path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home).join(".ohhplayer_settings")
    }
}
