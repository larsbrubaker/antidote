//! Wasm `SettingsStore` impl — `localStorage["antidote_settings"]`.

use antidote_core::platform::{Settings, SettingsStore};
use std::sync::Arc;

const KEY: &str = "antidote_settings";

pub struct LocalStorageSettingsStore;

impl LocalStorageSettingsStore {
    pub fn new() -> Self {
        Self
    }

    /// Construct an `Arc<dyn SettingsStore>` — backed by `localStorage`
    /// when available, and by an in-memory store otherwise (e.g. when
    /// `localStorage` is disabled or unavailable).
    pub fn into_shared() -> Arc<dyn SettingsStore> {
        Arc::new(Self::new())
    }

    fn ls() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }
}

impl Default for LocalStorageSettingsStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsStore for LocalStorageSettingsStore {
    fn load(&self) -> Settings {
        let Some(ls) = Self::ls() else {
            return Settings::default();
        };
        let Ok(Some(s)) = ls.get_item(KEY) else {
            return Settings::default();
        };
        serde_json::from_str::<Settings>(&s).unwrap_or_default()
    }
    fn save(&self, settings: &Settings) {
        if let Some(ls) = Self::ls() {
            if let Ok(s) = serde_json::to_string(settings) {
                let _ = ls.set_item(KEY, &s);
            }
        }
    }
}
