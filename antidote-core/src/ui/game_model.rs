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
use crate::platform::{in_memory_best_score_store, BestScoreStore};
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
    /// Highest `world.total_score` ever recorded on this device. Loaded from
    /// the platform store at construction and rewritten whenever
    /// `total_score` climbs past it.
    pub best_score: u64,
    best_score_store: Arc<dyn BestScoreStore>,
}

impl GameModel {
    pub fn new(best_score_store: Arc<dyn BestScoreStore>) -> Self {
        let best_score = best_score_store.load();
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
            epoch: Instant::now(),
            last_paint: None,
            timestep: FixedTimestep::new(),
            best_score,
            best_score_store,
        }
    }

    /// Persist a new best score if the current session has beaten the
    /// previous record. Cheap when called every frame — the comparison
    /// short-circuits the common case.
    pub fn maybe_record_best_score(&mut self) {
        if self.world.total_score > self.best_score {
            self.best_score = self.world.total_score;
            self.best_score_store.save(self.best_score);
        }
    }
}

impl Default for GameModel {
    fn default() -> Self {
        Self::new(in_memory_best_score_store())
    }
}

/// Reference-counted handle that every widget holds a clone of.
pub type SharedModel = Rc<RefCell<GameModel>>;

/// Convenience constructor with an in-memory best-score store. Tests use this;
/// production shells pass a real `Arc<dyn BestScoreStore>` via
/// [`shared_with_store`].
pub fn shared() -> SharedModel {
    Rc::new(RefCell::new(GameModel::default()))
}

/// Constructor that accepts a real best-score store from a platform shell.
pub fn shared_with_store(store: Arc<dyn BestScoreStore>) -> SharedModel {
    Rc::new(RefCell::new(GameModel::new(store)))
}
