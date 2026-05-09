//! Scene painter — viruses + bubbles + growing bubble. Stub; M2 fills it in.
//!
//! Coordinate note: agg-gui is Y-up. The JS reference is Y-down.
//! `paint_scene` flips Y exactly once when reading `World` coordinates.

use crate::consts::VIRTUAL_HEIGHT;
use crate::game::state::World;

pub fn flip_y(y: f32) -> f32 {
    VIRTUAL_HEIGHT - y
}

pub fn paint_scene(_world: &World) {
    // M2: drive agg_gui::GfxCtx via the GameWidget's paint method.
}
