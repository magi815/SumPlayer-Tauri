use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadItem {
    pub id: String,
    pub filename: String,
    pub url: String,
    #[serde(rename = "savePath")]
    pub save_path: String,
    #[serde(rename = "totalBytes")]
    pub total_bytes: u64,
    #[serde(rename = "receivedBytes")]
    pub received_bytes: u64,
    pub state: String, // "progressing", "completed", "cancelled", "interrupted"
    #[serde(rename = "startedAt")]
    pub started_at: u64,
    #[serde(rename = "completedAt")]
    pub completed_at: Option<u64>,
    pub progress: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct DownloadData {
    history: Vec<DownloadItem>,
    #[serde(rename = "nextId")]
    next_id: u64,
}

pub struct DownloadManager {
    active_downloads: Vec<DownloadItem>,
    history: Vec<DownloadItem>,
    next_id: u64,
    file_path: PathBuf,
}

impl DownloadManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let file_path = data_dir.join("downloads.json");
        let mut mgr = DownloadManager {
            active_downloads: Vec::new(),
            history: Vec::new(),
            next_id: 1,
            file_path,
        };
        mgr.load();
        mgr
    }

    fn load(&mut self) {
        if let Ok(data) = std::fs::read_to_string(&self.file_path) {
            if let Ok(parsed) = serde_json::from_str::<DownloadData>(&data) {
                self.history = parsed.history;
                self.next_id = parsed.next_id;
            }
        }
    }

    fn save(&self) {
        let data = DownloadData {
            history: self.history.clone(),
            next_id: self.next_id,
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }

    pub fn get_active_downloads(&self) -> Vec<&DownloadItem> {
        self.active_downloads
            .iter()
            .filter(|d| d.state == "progressing")
            .collect()
    }

    pub fn get_history(&self) -> Vec<&DownloadItem> {
        self.history.iter().rev().collect()
    }

    pub fn cancel_download(&mut self, id: &str) {
        if let Some(dl) = self.active_downloads.iter_mut().find(|d| d.id == id) {
            dl.state = "cancelled".to_string();
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
        self.save();
    }

    pub fn add_completed_download(&mut self, filename: &str, save_path: &str, size: u64) {
        let id = self.next_id.to_string();
        self.next_id += 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let item = DownloadItem {
            id,
            filename: filename.to_string(),
            url: String::new(),
            save_path: save_path.to_string(),
            total_bytes: size,
            received_bytes: size,
            state: "completed".to_string(),
            started_at: now,
            completed_at: Some(now),
            progress: 100,
        };
        self.history.push(item);
        self.save();
    }

    pub fn get_tracked_files(&self) -> std::collections::HashSet<String> {
        self.history.iter().map(|d| d.save_path.clone()).collect()
    }
}
