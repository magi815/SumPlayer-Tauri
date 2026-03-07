use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: String,
    pub url: String,
    pub title: String,
    pub favicon: String,
    #[serde(rename = "folderId")]
    pub folder_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookmarkFolder {
    pub id: String,
    pub name: String,
    #[serde(rename = "parentId")]
    pub parent_id: String,
    #[serde(rename = "createdAt")]
    pub created_at: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct BookmarkData {
    bookmarks: Vec<Bookmark>,
    folders: Vec<BookmarkFolder>,
    #[serde(rename = "nextId")]
    next_id: u64,
}

pub struct BookmarkManager {
    bookmarks: Vec<Bookmark>,
    folders: Vec<BookmarkFolder>,
    next_id: u64,
    file_path: PathBuf,
}

impl BookmarkManager {
    pub fn new(data_dir: PathBuf) -> Self {
        let file_path = data_dir.join("bookmarks.json");
        let first_run = !file_path.exists();
        let mut mgr = BookmarkManager {
            bookmarks: Vec::new(),
            folders: Vec::new(),
            next_id: 1,
            file_path,
        };
        mgr.load();
        if first_run && mgr.bookmarks.is_empty() {
            mgr.add_bookmark("https://www.google.com", "Google", None, None);
            mgr.add_bookmark("https://www.youtube.com", "YouTube", None, None);
        }
        mgr
    }

    fn load(&mut self) {
        if let Ok(data) = std::fs::read_to_string(&self.file_path) {
            if let Ok(parsed) = serde_json::from_str::<BookmarkData>(&data) {
                self.bookmarks = parsed.bookmarks;
                self.folders = parsed.folders;
                self.next_id = parsed.next_id;
            }
        }
    }

    fn save(&self) {
        let data = BookmarkData {
            bookmarks: self.bookmarks.clone(),
            folders: self.folders.clone(),
            next_id: self.next_id,
        };
        if let Ok(json) = serde_json::to_string_pretty(&data) {
            let _ = std::fs::write(&self.file_path, json);
        }
    }

    pub fn add_bookmark(
        &mut self,
        url: &str,
        title: &str,
        folder_id: Option<&str>,
        favicon: Option<&str>,
    ) -> Bookmark {
        let id = self.next_id.to_string();
        self.next_id += 1;

        let bookmark = Bookmark {
            id: id.clone(),
            url: url.to_string(),
            title: if title.is_empty() {
                url.to_string()
            } else {
                title.to_string()
            },
            favicon: favicon.unwrap_or("").to_string(),
            folder_id: folder_id.unwrap_or("").to_string(),
            created_at: chrono::Utc::now().timestamp_millis() as u64,
        };

        self.bookmarks.push(bookmark.clone());
        self.save();
        bookmark
    }

    pub fn remove_bookmark(&mut self, id: &str) {
        self.bookmarks.retain(|b| b.id != id);
        self.save();
    }

    pub fn update_bookmark(
        &mut self,
        id: &str,
        title: Option<&str>,
        url: Option<&str>,
        folder_id: Option<&str>,
    ) {
        if let Some(bookmark) = self.bookmarks.iter_mut().find(|b| b.id == id) {
            if let Some(t) = title {
                bookmark.title = t.to_string();
            }
            if let Some(u) = url {
                bookmark.url = u.to_string();
            }
            if let Some(f) = folder_id {
                bookmark.folder_id = f.to_string();
            }
            self.save();
        }
    }

    pub fn is_bookmarked(&self, url: &str) -> Option<&Bookmark> {
        self.bookmarks.iter().find(|b| b.url == url)
    }

    pub fn get_bookmarks(&self, folder_id: &str) -> Vec<&Bookmark> {
        self.bookmarks
            .iter()
            .filter(|b| b.folder_id == folder_id)
            .collect()
    }

    pub fn get_all_bookmarks(&self) -> &Vec<Bookmark> {
        &self.bookmarks
    }

    pub fn get_all_folders(&self) -> &Vec<BookmarkFolder> {
        &self.folders
    }

    pub fn move_bookmark(&mut self, bookmark_id: &str, before_bookmark_id: Option<&str>) {
        let from_idx = self.bookmarks.iter().position(|b| b.id == bookmark_id);
        if let Some(from) = from_idx {
            let moved = self.bookmarks.remove(from);
            if let Some(before_id) = before_bookmark_id {
                if let Some(to) = self.bookmarks.iter().position(|b| b.id == before_id) {
                    self.bookmarks.insert(to, moved);
                } else {
                    self.bookmarks.push(moved);
                }
            } else {
                self.bookmarks.push(moved);
            }
            self.save();
        }
    }
}
