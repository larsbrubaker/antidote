//! Wasm `Storage` impl — `localStorage["antidote_session"]`.
#![allow(dead_code)] // wired up in M5

use antidote_core::db::inbox::Session;
use antidote_core::platform::Storage;

const KEY: &str = "antidote_session";

pub struct LocalStorage;

impl LocalStorage {
    pub fn new() -> Self {
        Self
    }

    fn ls() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }
}

impl Default for LocalStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for LocalStorage {
    fn load_session(&self) -> Option<Session> {
        let s = Self::ls()?.get_item(KEY).ok().flatten()?;
        serde_json::from_str(&s).ok()
    }
    fn save_session(&self, session: &Session) {
        if let Some(ls) = Self::ls() {
            if let Ok(s) = serde_json::to_string(session) {
                let _ = ls.set_item(KEY, &s);
            }
        }
    }
    fn clear_session(&self) {
        if let Some(ls) = Self::ls() {
            let _ = ls.remove_item(KEY);
        }
    }
}
