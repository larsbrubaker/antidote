//! Platform-injected services. Both shells (native, wasm) implement these.

use std::sync::Arc;

use serde::{Deserialize, Serialize};

/// Persistent per-device player state. Native saves a JSON file under
/// `dirs::data_dir()/antidote/settings.json`; wasm uses
/// `localStorage["antidote_settings"]`. Tests use [`InMemorySettingsStore`].
pub trait SettingsStore: Send + Sync {
    fn load(&self) -> Settings;
    fn save(&self, settings: &Settings);
}

/// Everything we persist about a player on this device. Versioned so we can
/// migrate the on-disk format later without losing data — `version: 1` is the
/// initial schema; bump on any breaking change and add a `From<SettingsVN>`
/// for the previous version.
///
/// Fits inside one localStorage / JSON-file blob; the whole struct is
/// (de)serialized as one unit by the platform store.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_version")]
    pub version: u32,
    /// Highest total score ever recorded on this device.
    #[serde(default)]
    pub best_score: u64,
    /// Most-recent first. Capped at [`MAX_RECENT_SCORES`].
    #[serde(default)]
    pub recent_scores: Vec<ScoreEntry>,
    /// If `Some`, the main menu shows a "Resume" button that drops the
    /// player back into this level at this score with this many lives.
    /// Cleared on `Game over` and on `New game`.
    #[serde(default)]
    pub saved_session: Option<SavedSession>,
}

const CURRENT_VERSION: u32 = 1;
const MAX_RECENT_SCORES: usize = 8;

fn default_version() -> u32 {
    CURRENT_VERSION
}

/// One row in the local "recent scores" list — written when a session ends
/// (either GameOver or by completing a level and returning to the menu).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScoreEntry {
    pub score: u64,
    /// Highest level the player reached during this session.
    pub level: u32,
}

/// Snapshot of a session that the player can come back to from the main menu.
/// Coarse-grained on purpose: only the level header, not mid-level world
/// state. Resuming starts the saved level fresh from its initial spawn, with
/// the persisted lives / score carried over.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedSession {
    pub level: u32,
    pub total_score: u64,
    pub lives: u8,
}

impl Settings {
    /// Record a fresh score in the recent-scores list and bump `best_score`
    /// if it's a new high. Drops the oldest entry once we exceed
    /// [`MAX_RECENT_SCORES`].
    pub fn record_finished_session(&mut self, score: u64, level: u32) {
        if score > self.best_score {
            self.best_score = score;
        }
        self.recent_scores.insert(0, ScoreEntry { score, level });
        if self.recent_scores.len() > MAX_RECENT_SCORES {
            self.recent_scores.truncate(MAX_RECENT_SCORES);
        }
    }
}

/// Test-friendly fallback that holds settings in memory only.
#[derive(Default)]
pub struct InMemorySettingsStore {
    settings: std::sync::Mutex<Settings>,
}

impl SettingsStore for InMemorySettingsStore {
    fn load(&self) -> Settings {
        self.settings.lock().expect("settings mutex").clone()
    }
    fn save(&self, settings: &Settings) {
        *self.settings.lock().expect("settings mutex") = settings.clone();
    }
}

/// Convenience: an `Arc<dyn SettingsStore>` backed by an in-memory store.
pub fn in_memory_settings_store() -> Arc<dyn SettingsStore> {
    Arc::new(InMemorySettingsStore::default())
}
