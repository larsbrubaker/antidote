//! `GameWidget` — the 800×600 letterboxed play area. M2 implements `agg_gui::Widget`.

use crate::game::state::World;

pub struct GameWidget {
    pub world: World,
}

impl GameWidget {
    pub fn new() -> Self {
        Self {
            world: World::new(),
        }
    }
}

impl Default for GameWidget {
    fn default() -> Self {
        Self::new()
    }
}
