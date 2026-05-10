//! Platform-injected services. Both shells (native, wasm) implement these.

use std::sync::Arc;

/// Persistent best-score store. Native saves a JSON file under
/// `dirs::data_dir()/antidote/best_score.json`; wasm uses
/// `localStorage["antidote_best_score"]`. Tests use [`InMemoryBestScoreStore`].
pub trait BestScoreStore: Send + Sync {
    fn load(&self) -> u64;
    fn save(&self, score: u64);
}

/// Test-friendly fallback that holds the score in memory only.
#[derive(Default)]
pub struct InMemoryBestScoreStore {
    score: std::sync::Mutex<u64>,
}

impl BestScoreStore for InMemoryBestScoreStore {
    fn load(&self) -> u64 {
        *self.score.lock().expect("best-score mutex")
    }
    fn save(&self, score: u64) {
        *self.score.lock().expect("best-score mutex") = score;
    }
}

/// Convenience: an `Arc<dyn BestScoreStore>` backed by an in-memory store.
pub fn in_memory_best_score_store() -> Arc<dyn BestScoreStore> {
    Arc::new(InMemoryBestScoreStore::default())
}
