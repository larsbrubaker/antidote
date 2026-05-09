use crate::game::physics;
use crate::game::state::World;

pub fn tick(world: &mut World, dt: f32) {
    physics::step(world, dt);
    // M2: antidote drain, life loss, level-complete check.
}
