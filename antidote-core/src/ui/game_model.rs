//! `GameModel` — the shared mutable state every Antidote widget reads from.
//!
//! `GameWidget` originally owned `World` + `PhysicsWorld` exclusively, which
//! worked when the entire UI was a single widget. Once we add a HUD, main
//! menu, pause overlay, level-complete screen, and game-over screen, multiple
//! widgets need to read the world (HUD reads `lives` / `total_score`) and a
//! few need to mutate it (Play button, Resume button, Next-level button). We
//! share via `Rc<RefCell<GameModel>>` so each widget can hold its own clone.
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
use crate::game::physics::PhysicsWorld;
use crate::game::state::World;
use crate::game::timestep::FixedTimestep;

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
}

impl GameModel {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
            epoch: Instant::now(),
            last_paint: None,
            timestep: FixedTimestep::new(),
        }
    }
}

impl Default for GameModel {
    fn default() -> Self {
        Self::new()
    }
}

/// Reference-counted handle that every widget holds a clone of.
pub type SharedModel = Rc<RefCell<GameModel>>;

/// Convenience constructor.
pub fn shared() -> SharedModel {
    Rc::new(RefCell::new(GameModel::new()))
}
