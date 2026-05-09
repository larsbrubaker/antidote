//! `GameWidget` — the 800×600 letterboxed play area. M2 implements `agg_gui::Widget`.

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::physics::PhysicsWorld;
use crate::game::state::World;

pub struct GameWidget {
    pub world: World,
    pub physics: PhysicsWorld,
}

impl GameWidget {
    pub fn new() -> Self {
        Self {
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
        }
    }
}

impl Default for GameWidget {
    fn default() -> Self {
        Self::new()
    }
}
