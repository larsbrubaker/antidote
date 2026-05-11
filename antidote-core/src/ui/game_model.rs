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
