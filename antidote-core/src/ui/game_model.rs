//! `GameModel` — the shared mutable state every Antidote widget reads from.
//!
//! `GameWidget` originally owned `World` + `PhysicsWorld` exclusively, which
//! worked when the entire UI was a single widget. Once a HUD, main menu, and
//! pause / level-complete / game-over overlays appeared too, multiple widgets
//! need to read the world (HUD reads `lives` / `total_score`) and a few need
//! to mutate it (Play button, Resume button, Next-level button). We share via
//! `Rc<RefCell<GameModel>>` so each widget can hold its own clone.
//!
//! Borrow discipline: every widget takes `&mut self` for its tree-mutating
//! methods (paint, on_event), and only borrows `model.borrow()` /
//! `borrow_mut()` for the duration of one operation. There is no nesting
//! across widget boundaries — borrows are taken and released within a single
//! callback or paint frame.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use web_time::Instant;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::physics::PhysicsWorld;
use crate::game::state::World;
use crate::platform::{in_memory_settings_store, SavedSession, Settings, SettingsStore};
use agg_gui::timestep::FixedTimestep;

/// Which sub-screen of the start-phase main menu is showing. Drives which
/// overlay paints when `world.phase == Phase::Start`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MenuView {
    #[default]
    Main,
    File,
    Help,
}

/// Owning state for the running game. Held inside [`SharedModel`].
pub struct GameModel {
    pub world: World,
    pub physics: PhysicsWorld,
    /// Wall-clock start; used to compute a monotonic time for animations.
    pub epoch: Instant,
    /// Last `paint` time on `GameWidget`; used to feed elapsed wall-time
    /// into the fixed timestep accumulator.
    pub last_paint: Option<Instant>,
    pub timestep: FixedTimestep,
    /// Local persistent player state — best score, recent scores, and the
    /// optional resume snapshot. Read on every layout pass by the main menu;
    /// rewritten whenever the player beats their best score, finishes a
    /// level / runs out of lives, or starts a fresh game.
    pub settings: Settings,
    settings_store: Arc<dyn SettingsStore>,
    /// Which sub-screen of the start menu is showing. `Main` is the default
    /// Play / Resume / Recent scores view; `File` and `Help` are the
    /// dropdown-overlays reached from the top menu bar.
    pub menu_view: MenuView,
    /// Set to `true` by the File → Export… menu item. The platform shell
    /// drains this each frame; on web it triggers a `Blob` download of the
    /// JSON returned by `export_settings_json`. Cleared by the drain.
    pub pending_export: bool,
    /// Set to `true` by the File → Import… menu item. The platform shell
    /// drains this each frame and opens a file picker; the selected file's
    /// JSON is fed back into `apply_settings_json`.
    pub pending_import: bool,
    /// Set to `true` by the menu bar's Fullscreen button. The platform shell
    /// drains this each frame and toggles browser fullscreen — entering if
    /// not currently fullscreen, exiting if so. Cleared by the drain.
    pub pending_fullscreen_toggle: bool,
    /// True when the platform shell reported a mobile/touch environment
    /// (coarse primary pointer). Drives the rotate-device overlay and
    /// enter-fullscreen-on-Play. The native shell never sets it.
    pub is_mobile: bool,
    /// Set on mobile when the player starts or resumes a game. The wasm
    /// shell drains it and calls `requestFullscreen()` +
    /// `screen.orientation.lock('landscape')`. Enter-only, unlike
    /// `pending_fullscreen_toggle`. Cleared by the drain.
    pub pending_enter_fullscreen: bool,
    /// `settings.best_score` as it stood when the current run started.
    /// `maybe_record_best_score` bumps the live best every frame, so this
    /// snapshot is the only way the game-over screen can tell "you beat
    /// your old best this run" (gold takeover + previous-best line).
    pub session_start_best: u64,
}

impl GameModel {
    pub fn new(settings_store: Arc<dyn SettingsStore>) -> Self {
        let settings = settings_store.load();
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
            epoch: Instant::now(),
            last_paint: None,
            timestep: FixedTimestep::new(),
            settings,
            settings_store,
            menu_view: MenuView::Main,
            pending_export: false,
            pending_import: false,
            pending_fullscreen_toggle: false,
            is_mobile: false,
            pending_enter_fullscreen: false,
            session_start_best: 0,
        }
    }

    /// Serialize the current `Settings` to a pretty JSON string. Used by
    /// the File → Export… flow; the platform shell wraps the result in a
    /// download.
    pub fn export_settings_json(&self) -> String {
        serde_json::to_string_pretty(&self.settings).unwrap_or_else(|_| "{}".to_owned())
    }

    /// Replace the current `Settings` with one parsed from `json` and
    /// persist. Returns `false` on parse failure; in that case nothing is
    /// touched so the player can retry with a different file.
    pub fn apply_settings_json(&mut self, json: &str) -> bool {
        match serde_json::from_str::<Settings>(json) {
            Ok(s) => {
                self.settings = s;
                self.save_settings();
                true
            }
            Err(_) => false,
        }
    }

    /// Flush `settings` to the platform store. Cheap (one JSON serialize +
    /// one filesystem / `localStorage` write); call from any code path that
    /// just mutated `settings`.
    pub fn save_settings(&self) {
        self.settings_store.save(&self.settings);
    }

    /// Persist a new best score if the current session has beaten the
    /// previous record. Cheap when called every frame — the comparison
    /// short-circuits the common case. Called from `GameWidget::paint`.
    pub fn maybe_record_best_score(&mut self) {
        if self.world.total_score > self.settings.best_score {
            self.settings.best_score = self.world.total_score;
            self.save_settings();
        }
    }

    /// Remember where the player is so the main menu's `Resume` button
    /// drops them back here. Call when a level finishes successfully or when
    /// the player pauses out to the menu — anywhere the current world state
    /// represents a useful place to come back to.
    pub fn record_saved_session(&mut self) {
        self.settings.saved_session = Some(SavedSession {
            level: self.world.level,
            total_score: self.world.total_score,
            lives: self.world.lives,
        });
        self.save_settings();
    }

    /// Drop any persisted resume snapshot. Called from `New game` and from
    /// the `GameOver` finalization so a fresh launch shows the start
    /// experience, not a stale "Resume from level 5" carry-over.
    pub fn clear_saved_session(&mut self) {
        if self.settings.saved_session.take().is_some() {
            self.save_settings();
        }
    }

    /// Append a finished session to `recent_scores` and bump `best_score`
    /// if applicable. Called when a session ends — either via `GameOver` or
    /// by the player explicitly returning to the menu mid-run.
    pub fn record_finished_session(&mut self, score: u64, level: u32) {
        if score == 0 {
            return; // Don't pollute the list with zero-score abandons.
        }
        self.settings.record_finished_session(score, level);
        self.save_settings();
    }
}

impl Default for GameModel {
    fn default() -> Self {
        Self::new(in_memory_settings_store())
    }
}

/// Reference-counted handle that every widget holds a clone of.
pub type SharedModel = Rc<RefCell<GameModel>>;

/// Convenience constructor with an in-memory settings store. Tests use this;
/// production shells pass a real `Arc<dyn SettingsStore>` via
/// [`shared_with_store`].
pub fn shared() -> SharedModel {
    Rc::new(RefCell::new(GameModel::default()))
}

/// Constructor that accepts a real settings store from a platform shell.
pub fn shared_with_store(store: Arc<dyn SettingsStore>) -> SharedModel {
    Rc::new(RefCell::new(GameModel::new(store)))
}
