//! `GameModel` — the shared mutable state every Antidote widget reads from.
//!
//! `GameWidget` originally owned `World` + `PhysicsWorld` exclusively, which
//! worked when the entire UI was a single widget. Once we add a HUD, main
//! menu, pause overlay, level-complete screen, game-over screen, sign-in
//! dialog, leaderboard, and an "other games" panel, multiple widgets need
//! to read the world (HUD reads `lives` / `total_score`) and a few need to
//! mutate it (Play button, Resume button, Next-level button). We share via
//! `Rc<RefCell<GameModel>>` so each widget can hold its own clone.
//!
//! Borrow discipline: every widget takes `&mut self` for its tree-mutating
//! methods (paint, on_event), and only borrows `model.borrow()` /
//! `borrow_mut()` for the duration of one operation. There is no nesting
//! across widget boundaries — borrows are taken and released within a single
//! callback or paint frame.

use std::cell::RefCell;
use std::rc::Rc;

use web_time::Instant;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::db::auth::AuthClient;
use crate::db::client::PostgrestClient;
use crate::db::inbox::{DbInbox, Session};
use crate::db::models::{Game, LeaderboardEntry};
use crate::game::physics::PhysicsWorld;
use crate::game::state::{Phase, World};
use agg_gui::timestep::FixedTimestep;

/// Supabase project identity. Both shells construct this from environment /
/// runtime-config.json and hand it to [`build_antidote_app`].
///
/// Empty values are tolerated — the auth + REST calls will fail with a
/// "network" error which the sign-in / leaderboard overlays surface cleanly.
/// The shells can boot before the config is known and the game is still
/// playable offline.
#[derive(Clone, Debug, Default)]
pub struct SupabaseConfig {
    pub url: String,
    pub anon_key: String,
    /// `slug` of the row in `public.games` that represents this game. Used
    /// to scope leaderboard queries and `user_scores` upserts. Defaults to
    /// `"antidote"`.
    pub game_slug: String,
}

impl SupabaseConfig {
    /// Production Supabase project for the deployed Antidote game. The
    /// `sb_publishable_*` key is documented by Supabase as safe to ship in
    /// browser bundles — Row-Level Security is what actually guards data,
    /// not the key itself. See `db/migrations/0001_init.sql`.
    pub fn antidote_default() -> Self {
        Self {
            url: "https://edupgibalgeqfujfkwmm.supabase.co".to_owned(),
            anon_key: "sb_publishable_ZDEbV624BDkCgIHEZnOTrw_nZDtyoBt".to_owned(),
            game_slug: "antidote".to_owned(),
        }
    }
}

/// Which sub-view of the main menu is currently active. Drives which overlay
/// is `is_visible()` while the world is in `Phase::Start`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuView {
    #[default]
    Main,
    SignIn,
    Leaderboard,
    OtherGames,
}

/// State of the current sign-in flow. The sign-in widget reads this to
/// render its inputs, in-flight indicator, and last error message; menu
/// widgets read `session` to decide whether to show "Sign in" or the
/// signed-in email + "Sign out" button.
#[derive(Default)]
pub struct AuthState {
    pub session: Option<Session>,
    /// True between dispatching a sign-in / sign-up request and receiving
    /// its result. Disables the form submit buttons in the meantime.
    pub pending: bool,
    /// Last server-returned or transport error from a sign-in / sign-up
    /// attempt. Cleared on the next successful sign-in or whenever the
    /// user navigates away from the SignIn view.
    pub last_error: Option<String>,
}

impl AuthState {
    /// Once a sign-in / sign-up succeeds, drop pending + clear any prior
    /// error and remember the session (also persisted by the platform
    /// shell via `Storage`).
    pub fn record_session(&mut self, session: Session) {
        self.session = Some(session);
        self.pending = false;
        self.last_error = None;
    }

    /// Mark a failed REST call. Clears `pending` so the form re-enables.
    pub fn record_error(&mut self, message: String) {
        self.pending = false;
        self.last_error = Some(message);
    }
}

/// Cached read-only data the menu overlays display: the games catalog and
/// the leaderboard for the current game. Each is fetched on demand
/// (entering the corresponding MenuView triggers a request) and updated
/// when the response lands on the db inbox.
#[derive(Default)]
pub struct MenuCaches {
    pub games: Option<Vec<Game>>,
    pub games_error: Option<String>,
    pub games_pending: bool,
    pub top_scores: Option<Vec<LeaderboardEntry>>,
    pub top_scores_error: Option<String>,
    pub top_scores_pending: bool,
}

/// Holds the network-side services. Lives inside [`GameModel`] so widgets
/// can call REST methods directly: `model.services.auth.sign_in_async(…)`.
pub struct AppServices {
    pub config: SupabaseConfig,
    pub auth: AuthClient,
    pub postgrest: PostgrestClient,
    pub inbox: DbInbox,
}

impl AppServices {
    pub fn new(config: SupabaseConfig) -> Self {
        let auth = AuthClient::new(&config.url, &config.anon_key);
        let postgrest = PostgrestClient::new(&config.url, &config.anon_key);
        Self {
            config,
            auth,
            postgrest,
            inbox: DbInbox::new(),
        }
    }
}

/// Per-session state for the score-sync logic. Tracks how much of the
/// current play session has already been pushed to Supabase via the
/// `add_game_score` RPC, so a single tick of `LevelComplete` only fires
/// one network call. Reset on `start_new_game` (or any other phase reset
/// back to a fresh session).
#[derive(Default)]
pub struct ScoreSyncState {
    /// Cumulative session score we've already sent to the server. Used
    /// to compute the per-flush delta.
    pub last_synced_session_score: u64,
    /// `world.phase` from the previous frame. Used to detect the
    /// transition into `LevelComplete` / `GameOver` exactly once.
    pub last_phase: Option<Phase>,
}

/// Owning state for the running game. Held inside [`SharedModel`].
pub struct GameModel {
    pub world: World,
    pub physics: PhysicsWorld,
    /// Wall-clock start; used to compute a monotonic time for animations.
    pub epoch: Instant,
    /// Last `paint` time on `GameCanvasWidget`; used to feed elapsed wall-time
    /// into the fixed timestep accumulator.
    pub last_paint: Option<Instant>,
    pub timestep: FixedTimestep,

    pub services: AppServices,
    pub auth: AuthState,
    pub menu_view: MenuView,
    pub menu_caches: MenuCaches,
    pub score_sync: ScoreSyncState,
}

impl GameModel {
    pub fn new(config: SupabaseConfig) -> Self {
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
            epoch: Instant::now(),
            last_paint: None,
            timestep: FixedTimestep::new(),
            services: AppServices::new(config),
            auth: AuthState::default(),
            menu_view: MenuView::Main,
            menu_caches: MenuCaches::default(),
            score_sync: ScoreSyncState::default(),
        }
    }

    /// Look up the UUID of the games-table row that corresponds to this
    /// game (i.e. `slug == services.config.game_slug`). Returns `None` if
    /// the games catalog hasn't been fetched yet, or if the catalog doesn't
    /// contain a matching row. Cheap; the catalog is normally tiny.
    pub fn cached_game_id(&self) -> Option<String> {
        let slug = &self.services.config.game_slug;
        self.menu_caches
            .games
            .as_ref()?
            .iter()
            .find(|g| &g.slug == slug)
            .map(|g| g.id.clone())
    }

    /// Reset score-sync state at the start of a fresh play session so the
    /// next `LevelComplete` / `GameOver` records the full session score.
    /// Call from button handlers that reset the world (`Play`, `Play again`,
    /// `Back to menu`).
    pub fn reset_score_sync(&mut self) {
        self.score_sync = ScoreSyncState::default();
    }
}

impl Default for GameModel {
    fn default() -> Self {
        Self::new(SupabaseConfig::default())
    }
}

/// Reference-counted handle that every widget holds a clone of.
pub type SharedModel = Rc<RefCell<GameModel>>;

/// Convenience constructor with default (empty) Supabase config. Tests use
/// this; production shells pass real config via [`shared_with_config`].
pub fn shared() -> SharedModel {
    Rc::new(RefCell::new(GameModel::default()))
}

/// Constructor that accepts a real Supabase config from a platform shell.
pub fn shared_with_config(config: SupabaseConfig) -> SharedModel {
    Rc::new(RefCell::new(GameModel::new(config)))
}
