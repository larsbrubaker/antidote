//! Platform-injected services. Both shells (native, wasm) implement these.

use crate::db::inbox::Session;

/// Persists the auth session — JSON file on native, `localStorage` in wasm.
pub trait Storage: Send + Sync {
    fn load_session(&self) -> Option<Session>;
    fn save_session(&self, session: &Session);
    fn clear_session(&self);
}
