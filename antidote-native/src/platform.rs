//! Native `SettingsStore` impl — JSON file under
//! `dirs::data_dir()/antidote/settings.json`.

use antidote_core::platform::{in_memory_settings_store, Settings, SettingsStore};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FileSettingsStore {
    path: PathBuf,
}

impl FileSettingsStore {
    pub fn new() -> Option<Self> {
        let mut p = dirs::data_dir()?;
        p.push("antidote");
        let _ = fs::create_dir_all(&p);
        p.push("settings.json");
        Some(Self { path: p })
    }

    /// Construct an `Arc<dyn SettingsStore>` — falls back to an in-memory
    /// store if the data dir can't be resolved (e.g. no `$HOME`). Callers
    /// shouldn't have to care which they got.
    pub fn into_shared() -> Arc<dyn SettingsStore> {
        match Self::new() {
            Some(store) => Arc::new(store),
            None => in_memory_settings_store(),
        }
    }
}

impl SettingsStore for FileSettingsStore {
    fn load(&self) -> Settings {
        let Ok(bytes) = fs::read(&self.path) else {
            return Settings::default();
        };
        serde_json::from_slice::<Settings>(&bytes).unwrap_or_default()
    }
    fn save(&self, settings: &Settings) {
        if let Ok(bytes) = serde_json::to_vec_pretty(settings) {
            let _ = fs::write(&self.path, bytes);
        }
    }
}
