//! Cross-thread bridge between async REST callbacks and the synchronous
//! agg-gui frame loop.
//!
//! `ehttp::fetch` runs its callback on whatever thread/task the underlying
//! transport finishes on (an internal `std::thread` on native, the JS event
//! loop on wasm). Our [`SharedModel`](crate::ui::game_model::SharedModel) is
//! `Rc<RefCell<…>>`, which is **not** `Send` — touching it from a network
//! callback would either fail to compile (native) or trip a panic (wasm).
//!
//! Instead, every REST call captures a [`flume::Sender<DbInboxEvent>`] and
//! emits a single typed event into the inbox when the response (or error)
//! arrives. The receiver lives on the GameModel; widgets drain it once per
//! frame in [`crate::ui::drain_db_inbox`] and update the auth / menu state.

use flume::{Receiver, Sender};
use serde::{Deserialize, Serialize};

use crate::db::models::{Game, LeaderboardEntry};

/// One inbound event from the Supabase REST layer back to the UI.
///
/// The UI matches on the variant and updates the corresponding piece of
/// state (sign-in flow, games catalog, leaderboard, etc.). Errors come
/// through as `Err(String)` because the underlying transport returns
/// `Result<ehttp::Response, String>` already.
#[derive(Debug, Clone)]
pub enum DbInboxEvent {
    /// Result of [`crate::db::auth::AuthClient::sign_in_async`]. The string
    /// form of the error is what we display to the user — keep it readable.
    SignInResult(Result<Session, String>),
    /// Result of [`crate::db::auth::AuthClient::sign_up_async`].
    SignUpResult(Result<Session, String>),
    /// Catalog snapshot from `GET /rest/v1/games?select=*`.
    GamesList(Result<Vec<Game>, String>),
    /// Top-N rows from `GET /rest/v1/leaderboard?game_slug=eq.<slug>&order=high_score.desc`.
    /// Each row is a [`LeaderboardEntry`] with the user's `handle` instead
    /// of their `user_id` UUID — see migration 0004.
    TopScoresList(Result<Vec<LeaderboardEntry>, String>),
}

/// Persisted Supabase session — lives in [`SharedModel`] for as long as the
/// user is signed in, and is also serialized to disk (native) or
/// `localStorage` (wasm) by the platform-injected `Storage` impl so the
/// signed-in state survives an app restart.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds since UNIX epoch. Supabase returns `expires_in`; we add it
    /// to the current time at sign-in.
    pub expires_at: i64,
    pub user_id: String,
    pub email: Option<String>,
}

/// The Sender / Receiver pair that lives on `GameModel`. Widgets clone the
/// `tx` into their REST callbacks; the main paint loop drains the `rx`.
#[derive(Debug, Clone)]
pub struct DbInbox {
    pub tx: Sender<DbInboxEvent>,
    pub rx: Receiver<DbInboxEvent>,
}

impl DbInbox {
    /// Construct an unbounded channel pair.
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();
        Self { tx, rx }
    }
}

impl Default for DbInbox {
    fn default() -> Self {
        Self::new()
    }
}
