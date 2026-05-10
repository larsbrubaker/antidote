//! Native `BestScoreStore` impl — JSON file under
//! `dirs::data_dir()/antidote/best_score.json`.

use antidote_core::platform::BestScoreStore;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

pub struct FileBestScoreStore {
    path: PathBuf,
}

impl FileBestScoreStore {
    pub fn new() -> Option<Self> {
        let mut p = dirs::data_dir()?;
        p.push("antidote");
        let _ = fs::create_dir_all(&p);
        p.push("best_score.json");
        Some(Self { path: p })
    }

    /// Construct an `Arc<dyn BestScoreStore>` — falls back to an in-memory
    /// store if the data dir can't be resolved (e.g. no `$HOME`). Callers
    /// shouldn't have to care which they got.
    pub fn into_shared() -> Arc<dyn BestScoreStore> {
        match Self::new() {
            Some(store) => Arc::new(store),
            None => antidote_core::platform::in_memory_best_score_store(),
        }
    }
}

impl BestScoreStore for FileBestScoreStore {
    fn load(&self) -> u64 {
        let Ok(bytes) = fs::read(&self.path) else {
            return 0;
        };
        serde_json::from_slice::<u64>(&bytes).unwrap_or(0)
    }
    fn save(&self, score: u64) {
        if let Ok(bytes) = serde_json::to_vec(&score) {
            let _ = fs::write(&self.path, bytes);
        }
    }
}
