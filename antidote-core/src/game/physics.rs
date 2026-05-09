//! Rapier2d-driven physics. Mirrors the body/collider parameters from the JS
//! reference (`reference/GFG/public/games/antidote/antidote-physics.js`) so
//! gameplay feel is preserved.
//!
//! Body parameters (from the JS reference):
//! - Wall:           static, friction 0.1, restitution 0.3
//! - Bubble:         dynamic, density 0.5, friction 0.1, restitution 0.4, linear_damping 0.5, rotation locked
//! - Virus:          dynamic, density 1.0, friction 0.0, restitution 1.0, ccd, rotation locked
//! - Dead virus:     dynamic, density 2.0, friction 0.5, restitution 0.1, linear_damping 2.0, rotation locked
//! - Growing bubble: dynamic, density 0.1, friction 0.1, restitution 0.0, rotation locked
//!
//! Step: 8 velocity iterations, 3 position iterations.

use crate::consts::{MIN_PERPENDICULAR_RATIO, WALL_PROXIMITY_THRESHOLD};

/// Collision-filter categories. Match the bitmasks used in the JS reference.
pub mod category {
    pub const WALL: u32 = 0x0001;
    pub const BUBBLE: u32 = 0x0002;
    pub const VIRUS: u32 = 0x0004;
    pub const DEAD_VIRUS: u32 = 0x0008;
    pub const GROWING_BUBBLE: u32 = 0x0010;
}

/// `correctWallSlideVelocity` from `antidote-core.js`. When a virus is near a
/// wall and moving mostly parallel to it, push the perpendicular component up
/// so the next step bounces away cleanly. Total speed is preserved.
pub fn correct_wall_slide_velocity(
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    game_width: f32,
    game_height: f32,
) -> (f32, f32) {
    let speed = (vx * vx + vy * vy).sqrt();
    if speed < 0.01 {
        return (vx, vy);
    }

    let mut cvx = vx;
    let mut cvy = vy;
    let mut corrected = false;

    // perp = ratio * parallel / sqrt(1 - ratio^2)
    let factor =
        MIN_PERPENDICULAR_RATIO / (1.0 - MIN_PERPENDICULAR_RATIO * MIN_PERPENDICULAR_RATIO).sqrt();

    if x < WALL_PROXIMITY_THRESHOLD && (vx / speed) < MIN_PERPENDICULAR_RATIO {
        cvx = cvy.abs() * factor;
        corrected = true;
    }
    if x > game_width - WALL_PROXIMITY_THRESHOLD && ((-vx) / speed) < MIN_PERPENDICULAR_RATIO {
        cvx = -cvy.abs() * factor;
        corrected = true;
    }
    if y < WALL_PROXIMITY_THRESHOLD && (vy / speed) < MIN_PERPENDICULAR_RATIO {
        cvy = cvx.abs() * factor;
        corrected = true;
    }
    if y > game_height - WALL_PROXIMITY_THRESHOLD && ((-vy) / speed) < MIN_PERPENDICULAR_RATIO {
        cvy = -cvx.abs() * factor;
        corrected = true;
    }

    if corrected {
        let new_speed = (cvx * cvx + cvy * cvy).sqrt();
        if new_speed > 0.01 {
            let scale = speed / new_speed;
            cvx *= scale;
            cvy *= scale;
        }
    }

    (cvx, cvy)
}

/// Wraps the rapier2d pipeline + body sets for the antidote game.
/// M2 fills in the implementation; this stub keeps the workspace compiling.
pub struct PhysicsWorld {
    // M2: rapier_pipeline, broad_phase, narrow_phase, island_manager, ccd_solver,
    //     rigid_body_set, collider_set, impulse_joint_set, multibody_joint_set,
    //     plus our HashMap<EntityId, RigidBodyHandle>.
}

impl PhysicsWorld {
    pub fn new(_game_width: f32, _game_height: f32) -> Self {
        // M2: build IntegrationParameters with dt=1/60, set gravity (0,0),
        // create static walls (4 boxes), wire CollisionEvent / ContactForceEvent
        // channels for the virus<->growing-bubble callback.
        Self {}
    }

    pub fn step(&mut self, _dt: f32) {
        // M2: PhysicsPipeline::step with 8 velocity iters, 3 position iters,
        // then drain CollisionEvent::Started and trigger
        // `on_virus_hit_growing_bubble` for any (virus, growing_bubble) pair.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correct_wall_slide_no_op_far_from_walls() {
        let (vx, vy) = correct_wall_slide_velocity(400.0, 300.0, 100.0, 50.0, 800.0, 600.0);
        assert!((vx - 100.0).abs() < 1e-4);
        assert!((vy - 50.0).abs() < 1e-4);
    }

    #[test]
    fn correct_wall_slide_preserves_speed_when_correcting() {
        // Virus near left wall, moving almost vertically (small vx).
        let (vx, vy) = correct_wall_slide_velocity(5.0, 300.0, 1.0, 100.0, 800.0, 600.0);
        let new_speed = (vx * vx + vy * vy).sqrt();
        let original_speed = (1.0_f32 * 1.0 + 100.0_f32 * 100.0).sqrt();
        assert!((new_speed - original_speed).abs() < 1e-3);
        // Perpendicular component should now be positive (bouncing away from left wall).
        assert!(vx > 0.0);
    }
}
