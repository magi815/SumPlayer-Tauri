use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(rename = "visitedAt")]
    pub visited_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct HistoryData {
    entries: Vec<HistoryEntry>,
    #[serde(rename = "nextId")]
    next_id: u64,
}

pub struct HistoryManager {
    entries: Vec<HistoryEntry>,
    next_id: u64,
    max_entries: usize,
    file_path: PathBuf,
    dirty: bool,
}

impl HistoryManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let file_path = data_dir.join("history.json");
        let mut mgr = HistoryManager {
            entries: Vec::new(),
            next_id: 1,
            max_entries: 10000,
            file_path,
            dirty: false,
        };
        mgr.load();
        mgr
    }

    fn load(&mut self) {
        if let Ok(data) = std::fs::read_to_string(&self.file_path) {
            if let Ok(parsed) = serde_json::from_str::<HistoryData>(&data) {
                self.entries = parsed.entries;
                self.next_id = parsed.next_id;
            }
        }
    }

    fn save(&self) {
        let data = HistoryData {
            entries: self.entries.clone(),
            next_id: self.next_id,
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }

    pub fn flush_save(&self) {
        if self.dirty {
            self.save();
        }
    }

    pub fn add_entry(&mut self, url: &str, title: &str) {
        if url.is_empty()
            || url == "about:blank"
            || url.starts_with("file://")
            || url.starts_with("devtools://")
        {
            return;
        }

        let entry = HistoryEntry {
            id: self.next_id.to_string(),
            url: url.to_string(),
            title: if title.is_empty() {
                url.to_string()
            } else {
                title.to_string()
            },
            visited_at: chrono::Utc::now().timestamp_millis() as u64,
        };
        self.next_id += 1;
        self.entries.push(entry);

        if self.entries.len() > self.max_entries {
            let drain = self.entries.len() - self.max_entries;
            self.entries.drain(0..drain);
        }

        self.dirty = true;
        // Save periodically (every 50 entries)
        if self.entries.len() % 50 == 0 {
            self.save();
            // dirty is not reset since we might still have unsaved changes conceptually
        }
    }

    pub fn get_entries(
        &self,
        query: Option<&str>,
        limit: usize,
        offset: usize,
    ) -> Vec<&HistoryEntry> {
        let mut result: Vec<&HistoryEntry> = self.entries.iter().rev().collect();
        if let Some(q) = query {
            let q_lower = q.to_lowercase();
            result.retain(|e| {
                e.url.to_lowercase().contains(&q_lower)
                    || e.title.to_lowercase().contains(&q_lower)
            });
        }
        result.into_iter().skip(offset).take(limit).collect()
    }

    pub fn delete_entry(&mut self, id: &str) {
        self.entries.retain(|e| e.id != id);
        self.save();
    }

    pub fn clear_all(&mut self) {
        self.entries.clear();
        self.next_id = 1;
        self.save();
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<&HistoryEntry> {
        if query.is_empty() {
            return Vec::new();
        }
        let q_lower = query.to_lowercase();
        let mut seen = std::collections::HashSet::new();
        let mut results = Vec::new();

        for entry in self.entries.iter().rev() {
            if results.len() >= limit {
                break;
            }
            if !seen.contains(&entry.url)
                && (entry.url.to_lowercase().contains(&q_lower)
                    || entry.title.to_lowercase().contains(&q_lower))
            {
                seen.insert(entry.url.clone());
                results.push(entry);
            }
        }
        results
    }
}
