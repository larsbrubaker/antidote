//! Native `Storage` impl — JSON file under `dirs::config_dir()/antidote/session.json`.

use antidote_core::db::auth::Session;
use antidote_core::platform::Storage;
use std::fs;
use std::path::PathBuf;

pub struct FileStorage {
    path: PathBuf,
}

impl FileStorage {
    pub fn new() -> Option<Self> {
        let mut p = dirs::config_dir()?;
        p.push("antidote");
        let _ = fs::create_dir_all(&p);
        p.push("session.json");
        Some(Self { path: p })
    }
}

impl Storage for FileStorage {
    fn load_session(&self) -> Option<Session> {
        let bytes = fs::read(&self.path).ok()?;
        serde_json::from_slice(&bytes).ok()
    }
    fn save_session(&self, session: &Session) {
        if let Ok(bytes) = serde_json::to_vec_pretty(session) {
            let _ = fs::write(&self.path, bytes);
        }
    }
    fn clear_session(&self) {
        let _ = fs::remove_file(&self.path);
    }
}
