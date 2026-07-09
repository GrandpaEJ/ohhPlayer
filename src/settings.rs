use std::collections::HashMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct AppSettings {
    pub volume: f32,
    pub speed: f32,
    pub scale_mode: i32,
    pub always_on_top: bool,
    #[serde(default)]
    pub file_positions: HashMap<String, f64>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            volume: 0.8,
            speed: 1.0,
            scale_mode: 0,
            always_on_top: false,
            file_positions: HashMap::new(),
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

    pub fn save_position(&mut self, path: &str, pos: f64) {
        if path.is_empty() { return; }
        self.file_positions.insert(path.to_string(), pos);
        // keep map small to prevent endless growth over years
        if self.file_positions.len() > 100 {
            let keys_to_remove: Vec<String> = self.file_positions.keys().take(20).cloned().collect();
            for k in keys_to_remove {
                self.file_positions.remove(&k);
            }
        }
        self.save();
    }

    pub fn get_position(&self, path: &str) -> f64 {
        *self.file_positions.get(path).unwrap_or(&0.0)
    }
}
