use crate::game::physics::PhysicsWorld;
use crate::game::state::World;

/// One simulation tick. M2 fills in:
/// - drain antidote per ANTIDOTE_DRAIN_RATE
/// - grow_bubble (port of `growBubble` in antidote-core.js): walls, slide-out
/// - physics.step
/// - sync rigid bodies → World coordinates
/// - maintain virus speeds per level
/// - update trap timers; transition trapped viruses → dying_viruses
/// - advance dying_viruses death_progress; on completion → dead_viruses + push pop_animation
/// - advance pop_animations; remove when progress >= 1
/// - advance solid_bubbles float and dead_viruses sink (forces applied via physics)
/// - check life-loss / level-complete transitions
pub fn tick(_world: &mut World, _physics: &mut PhysicsWorld, _dt: f32) {
    // see module doc for the M2 step list.
}
