//! Wasm `BestScoreStore` impl — `localStorage["antidote_best_score"]`.

use antidote_core::platform::BestScoreStore;
use std::sync::Arc;

const KEY: &str = "antidote_best_score";

pub struct LocalStorageBestScoreStore;

impl LocalStorageBestScoreStore {
    pub fn new() -> Self {
        Self
    }

    /// Construct an `Arc<dyn BestScoreStore>` — backed by `localStorage`
    /// when available, and by an in-memory store otherwise (e.g. when
    /// `localStorage` is disabled or unavailable).
    pub fn into_shared() -> Arc<dyn BestScoreStore> {
        Arc::new(Self::new())
    }

    fn ls() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok().flatten()
    }
}

impl Default for LocalStorageBestScoreStore {
    fn default() -> Self {
        Self::new()
    }
}

impl BestScoreStore for LocalStorageBestScoreStore {
    fn load(&self) -> u64 {
        let Some(ls) = Self::ls() else {
            return 0;
        };
        let Ok(Some(s)) = ls.get_item(KEY) else {
            return 0;
        };
        s.parse::<u64>().unwrap_or(0)
    }
    fn save(&self, score: u64) {
        if let Some(ls) = Self::ls() {
            let _ = ls.set_item(KEY, &score.to_string());
        }
    }
}
