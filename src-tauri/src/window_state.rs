use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowState {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    #[serde(rename = "isMaximized")]
    pub is_maximized: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        WindowState {
            x: -1,
            y: -1,
            width: 1280,
            height: 800,
            is_maximized: false,
        }
    }
}

pub struct WindowStateManager {
    file_path: PathBuf,
}

impl WindowStateManager {
    pub fn new(data_dir: PathBuf) -> Self {
        WindowStateManager {
            file_path: data_dir.join("window-state.json"),
        }
    }

    pub fn load(&self) -> WindowState {
        if let Ok(data) = std::fs::read_to_string(&self.file_path) {
            if let Ok(state) = serde_json::from_str::<WindowState>(&data) {
                return state;
            }
        }
        WindowState::default()
    }

    pub fn save(&self, x: i32, y: i32, width: u32, height: u32, is_maximized: bool) {
        let state = WindowState {
            x,
            y,
            width,
            height,
            is_maximized,
        };
        if let Ok(json) = serde_json::to_string(&state) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }
}
