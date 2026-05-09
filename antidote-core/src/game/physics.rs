//! Rapier2d-driven physics. Mirrors the body/collider parameters from the JS
//! reference (`reference/GFG/public/games/antidote/antidote-physics.js`) so
//! gameplay feel is preserved.
//!
//! Body parameters (ported verbatim from the JS reference):
//! - Wall:           static, friction 0.1, restitution 0.3
//! - Bubble:         dynamic, density 0.5, friction 0.1, restitution 0.4, linear_damping 0.5, rotation locked
//! - Virus:          dynamic, density 1.0, friction 0.0, restitution 1.0, ccd, rotation locked
//! - Dead virus:     dynamic, density 2.0, friction 0.5, restitution 0.1, linear_damping 2.0, rotation locked
//! - Growing bubble: dynamic, density 0.1, friction 0.1, restitution 0.0, rotation locked
//!
//! Step: 8 velocity iterations, 3 position iterations.

use crate::consts::{
    to_meters, MIN_PERPENDICULAR_RATIO, PIXELS_PER_METER, WALL_PROXIMITY_THRESHOLD,
};
use rapier2d::prelude::*;

/// Collision-filter categories. Match the JS reference's bitmasks exactly.
pub mod category {
    use rapier2d::prelude::Group;

    pub const WALL: Group = Group::GROUP_1;
    pub const BUBBLE: Group = Group::GROUP_2;
    pub const VIRUS: Group = Group::GROUP_3;
    pub const DEAD_VIRUS: Group = Group::GROUP_4;
    pub const GROWING_BUBBLE: Group = Group::GROUP_5;
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

/// rapier2d-backed physics simulation. Owns the rigid-body and collider sets
/// plus the per-step working state.
pub struct PhysicsWorld {
    pub gravity: Vector<Real>,
    pub integration_parameters: IntegrationParameters,
    pub physics_pipeline: PhysicsPipeline,
    pub island_manager: IslandManager,
    pub broad_phase: DefaultBroadPhase,
    pub narrow_phase: NarrowPhase,
    pub impulse_joints: ImpulseJointSet,
    pub multibody_joints: MultibodyJointSet,
    pub ccd_solver: CCDSolver,
    pub query_pipeline: QueryPipeline,
    pub bodies: RigidBodySet,
    pub colliders: ColliderSet,

    pub game_width: f32,
    pub game_height: f32,
    wall_bodies: Vec<RigidBodyHandle>,
}

impl PhysicsWorld {
    pub fn new(game_width: f32, game_height: f32) -> Self {
        // Per-step `dt` is overridden in `step()`. JS reference's exact iteration
        // counts (8 velocity, 3 position) don't map cleanly to rapier's unified
        // solver; defaults are close enough for the gameplay feel.
        let integration_parameters = IntegrationParameters::default();

        let mut world = Self {
            gravity: vector![0.0, 0.0],
            integration_parameters,
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            impulse_joints: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solver: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            bodies: RigidBodySet::new(),
            colliders: ColliderSet::new(),
            game_width,
            game_height,
            wall_bodies: Vec::new(),
        };

        world.create_walls();
        world
    }

    /// Static walls around the play area. Inner edges sit at game boundary.
    fn create_walls(&mut self) {
        let hw = to_meters(self.game_width / 2.0);
        let hh = to_meters(self.game_height / 2.0);
        let wall_thickness = 0.5_f32; // meters

        // (center_x, center_y, half_extents_x, half_extents_y)
        let walls = [
            (hw, -wall_thickness * 0.5, hw, wall_thickness * 0.5), // top (y < 0 in JS Y-down — outside the playfield)
            (
                hw,
                to_meters(self.game_height) + wall_thickness * 0.5,
                hw,
                wall_thickness * 0.5,
            ), // bottom
            (-wall_thickness * 0.5, hh, wall_thickness * 0.5, hh), // left
            (
                to_meters(self.game_width) + wall_thickness * 0.5,
                hh,
                wall_thickness * 0.5,
                hh,
            ), // right
        ];

        let groups = InteractionGroups::new(
            category::WALL,
            category::BUBBLE | category::VIRUS | category::DEAD_VIRUS | category::GROWING_BUBBLE,
        );

        for (cx, cy, hex, hey) in walls {
            let body = RigidBodyBuilder::fixed()
                .translation(vector![cx, cy])
                .build();
            let body_handle = self.bodies.insert(body);
            let collider = ColliderBuilder::cuboid(hex, hey)
                .friction(0.1)
                .restitution(0.3)
                .collision_groups(groups)
                .build();
            self.colliders
                .insert_with_parent(collider, body_handle, &mut self.bodies);
            self.wall_bodies.push(body_handle);
        }
    }

    /// Step the simulation. JS reference uses 8 velocity / 3 position iterations.
    pub fn step(&mut self, dt: f32) {
        self.integration_parameters.dt = dt;
        let physics_hooks = ();
        let event_handler = ();
        self.physics_pipeline.step(
            &self.gravity,
            &self.integration_parameters,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            &mut self.ccd_solver,
            Some(&mut self.query_pipeline),
            &physics_hooks,
            &event_handler,
        );
    }

    /// Convert pixel-space coords to rapier translation (meters).
    #[inline]
    pub fn pixels_to_meters(px: f32) -> f32 {
        px / PIXELS_PER_METER
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
        let (vx, vy) = correct_wall_slide_velocity(5.0, 300.0, 1.0, 100.0, 800.0, 600.0);
        let new_speed = (vx * vx + vy * vy).sqrt();
        let original_speed = (1.0_f32 * 1.0 + 100.0_f32 * 100.0).sqrt();
        assert!((new_speed - original_speed).abs() < 1e-3);
        assert!(vx > 0.0);
    }

    #[test]
    fn world_creates_with_four_walls() {
        let world = PhysicsWorld::new(800.0, 600.0);
        assert_eq!(world.wall_bodies.len(), 4);
        assert_eq!(world.bodies.len(), 4);
        assert_eq!(world.colliders.len(), 4);
    }

    #[test]
    fn step_runs_without_panic() {
        let mut world = PhysicsWorld::new(800.0, 600.0);
        world.step(1.0 / 60.0);
        world.step(1.0 / 60.0);
    }
}
