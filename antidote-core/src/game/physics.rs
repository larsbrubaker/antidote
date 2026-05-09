//! Rapier2d-driven physics. Mirrors the body/collider parameters from the JS
//! reference (`gfg/public/games/antidote/antidote-physics.js`) so
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
    to_meters, to_pixels, MIN_PERPENDICULAR_RATIO, PIXELS_PER_METER, WALL_PROXIMITY_THRESHOLD,
};
use crate::game::state::{Bubble, DeadVirus, GrowingBubble, Virus, World};
use rapier2d::prelude::*;
use std::num::NonZeroUsize;

/// Collision-filter categories. Match the JS reference's bitmasks exactly.
pub mod category {
    use rapier2d::prelude::Group;

    pub const WALL: Group = Group::GROUP_1;
    pub const BUBBLE: Group = Group::GROUP_2;
    pub const VIRUS: Group = Group::GROUP_3;
    pub const DEAD_VIRUS: Group = Group::GROUP_4;
    pub const GROWING_BUBBLE: Group = Group::GROUP_5;
}

/// Free-standing helper for [`PhysicsWorld::clamp_to_playfield`]. Snaps a
/// rapier body's centre back into the playfield rectangle (in pixels) and
/// kills the outward-pointing velocity component so the next step doesn't
/// immediately re-escape.
fn clamp_body_inside(
    bodies: &mut RigidBodySet,
    handle: RigidBodyHandle,
    radius_px: f32,
    game_width_px: f32,
    game_height_px: f32,
) {
    let Some(rb) = bodies.get_mut(handle) else {
        return;
    };
    let r_m = to_meters(radius_px);
    let w_m = to_meters(game_width_px);
    let h_m = to_meters(game_height_px);
    let p = *rb.translation();
    let v = *rb.linvel();
    let mut new_x = p.x;
    let mut new_y = p.y;
    let mut new_vx = v.x;
    let mut new_vy = v.y;
    let mut changed = false;
    if p.x < r_m {
        new_x = r_m;
        if v.x < 0.0 {
            new_vx = 0.0;
        }
        changed = true;
    } else if p.x > w_m - r_m {
        new_x = w_m - r_m;
        if v.x > 0.0 {
            new_vx = 0.0;
        }
        changed = true;
    }
    if p.y < r_m {
        new_y = r_m;
        if v.y < 0.0 {
            new_vy = 0.0;
        }
        changed = true;
    } else if p.y > h_m - r_m {
        new_y = h_m - r_m;
        if v.y > 0.0 {
            new_vy = 0.0;
        }
        changed = true;
    }
    if changed {
        rb.set_translation(vector![new_x, new_y], true);
        rb.set_linvel(vector![new_vx, new_vy], true);
    }
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
        // Per-step `dt` is overridden in `step()`. Match Box2D's 8 velocity
        // iterations as closely as Rapier's unified solver exposes.
        let integration_parameters = IntegrationParameters {
            num_solver_iterations: NonZeroUsize::new(8).expect("non-zero solver iterations"),
            ..IntegrationParameters::default()
        };

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
                .friction_combine_rule(CoefficientCombineRule::Min)
                .restitution_combine_rule(CoefficientCombineRule::Max)
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

    // ---- body creation (params match `antidote-physics.js`) ----

    /// `createVirusBody` — dynamic, density 1.0, friction 0.0, restitution 1.0,
    /// CCD enabled, rotation locked.
    pub fn spawn_virus_body(&mut self, virus: &mut Virus, radius: f32) -> RigidBodyHandle {
        let body = RigidBodyBuilder::dynamic()
            .translation(vector![to_meters(virus.x), to_meters(virus.y)])
            .linvel(vector![to_meters(virus.vx), to_meters(virus.vy)])
            .lock_rotations()
            .ccd_enabled(true)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::ball(to_meters(radius))
            .density(1.0)
            .friction(0.0)
            .restitution(1.0)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                category::VIRUS,
                category::WALL
                    | category::BUBBLE
                    | category::VIRUS
                    | category::DEAD_VIRUS
                    | category::GROWING_BUBBLE,
            ))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        virus.body = Some(handle);
        handle
    }

    /// `createBubbleBody` — dynamic, density 0.5, friction 0.1, restitution 0.4,
    /// linear damping 0.5, rotation locked.
    pub fn spawn_bubble_body(&mut self, bubble: &mut Bubble) -> RigidBodyHandle {
        let body = RigidBodyBuilder::dynamic()
            .translation(vector![to_meters(bubble.x), to_meters(bubble.y)])
            .linvel(vector![to_meters(bubble.vx), to_meters(bubble.vy)])
            .linear_damping(0.5)
            .lock_rotations()
            // CCD on bubbles too — without it a virus impact can give a
            // bubble enough velocity to tunnel through the 0.5 m wall in a
            // single fixed-step tick. Walls would still nudge the body back
            // on the next step, but for one frame the bubble's centre has
            // already moved past the playfield. The hard `clamp_to_playfield`
            // post-step is the ultimate safety net (see below); CCD prevents
            // the visible flicker.
            .ccd_enabled(true)
            // Don't let bubbles fall asleep — once they wedge against the
            // top wall, rapier deactivates them and the per-step
            // `add_force` (and incoming virus collisions) stop registering,
            // producing the "bubble locked in place" symptom. Planck/JS
            // does not have this sleep behavior, so we opt out here.
            .can_sleep(false)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::ball(to_meters(bubble.radius))
            .density(0.5)
            .friction(0.1)
            .restitution(0.4)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                category::BUBBLE,
                category::WALL | category::BUBBLE | category::VIRUS | category::DEAD_VIRUS,
            ))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        bubble.body = Some(handle);
        handle
    }

    /// `createDeadVirusBody` — dynamic, density 2.0, friction 0.5, restitution 0.1,
    /// linear damping 2.0, rotation locked.
    pub fn spawn_dead_virus_body(&mut self, dv: &mut DeadVirus) -> RigidBodyHandle {
        let body = RigidBodyBuilder::dynamic()
            .translation(vector![to_meters(dv.x), to_meters(dv.y)])
            .linvel(vector![0.0, to_meters(dv.vy)])
            .linear_damping(2.0)
            .lock_rotations()
            // CCD here too — a stack of dead viruses sinking with continuous
            // gravity force occasionally builds enough downward velocity to
            // poke through the bottom wall.
            .ccd_enabled(true)
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::ball(to_meters(dv.radius))
            .density(2.0)
            .friction(0.5)
            .restitution(0.1)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                category::DEAD_VIRUS,
                category::WALL | category::BUBBLE | category::VIRUS | category::DEAD_VIRUS,
            ))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        dv.body = Some(handle);
        handle
    }

    /// `createGrowingBubbleBody` — dynamic, density 0.1, friction 0.1,
    /// restitution 0.0, rotation locked. Only collides with walls + viruses.
    pub fn spawn_growing_bubble_body(&mut self, g: &mut GrowingBubble) -> RigidBodyHandle {
        let body = RigidBodyBuilder::dynamic()
            .translation(vector![to_meters(g.x), to_meters(g.y)])
            .lock_rotations()
            .build();
        let handle = self.bodies.insert(body);
        let collider = ColliderBuilder::ball(to_meters(g.radius))
            .density(0.1)
            .friction(0.1)
            .restitution(0.0)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                category::GROWING_BUBBLE,
                category::WALL | category::VIRUS,
            ))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
        g.body = Some(handle);
        handle
    }

    /// Force a body's translation in pixels. Used to keep the growing-bubble
    /// physics body at the pointer position each frame.
    pub fn set_body_position(&mut self, handle: RigidBodyHandle, x: f32, y: f32) {
        if let Some(rb) = self.bodies.get_mut(handle) {
            rb.set_translation(vector![to_meters(x), to_meters(y)], true);
        }
    }

    /// Reset a body's linear velocity to zero. Useful in tests that teleport
    /// a body to a known location and don't want stale velocity carrying it
    /// back into something on the next step.
    pub fn zero_body_velocity(&mut self, handle: RigidBodyHandle) {
        if let Some(rb) = self.bodies.get_mut(handle) {
            rb.set_linvel(vector![0.0, 0.0], true);
        }
    }

    /// Replace the growing-bubble's collider with a new one of the given radius.
    /// Mirrors `updateGrowingBubbleRadius` in the JS reference.
    pub fn resize_growing_bubble_collider(&mut self, handle: RigidBodyHandle, new_radius: f32) {
        // Remove the existing collider(s) on this body, then add a new one.
        let collider_handles: Vec<_> = self.bodies[handle].colliders().to_vec();
        for ch in collider_handles {
            self.colliders
                .remove(ch, &mut self.island_manager, &mut self.bodies, false);
        }
        let collider = ColliderBuilder::ball(to_meters(new_radius))
            .density(0.1)
            .friction(0.1)
            .restitution(0.0)
            .friction_combine_rule(CoefficientCombineRule::Min)
            .restitution_combine_rule(CoefficientCombineRule::Max)
            .collision_groups(InteractionGroups::new(
                category::GROWING_BUBBLE,
                category::WALL | category::VIRUS,
            ))
            .build();
        self.colliders
            .insert_with_parent(collider, handle, &mut self.bodies);
    }

    /// Remove a body + its colliders from the world.
    pub fn destroy_body(&mut self, handle: RigidBodyHandle) {
        self.bodies.remove(
            handle,
            &mut self.island_manager,
            &mut self.colliders,
            &mut self.impulse_joints,
            &mut self.multibody_joints,
            true,
        );
    }

    /// Hard playfield-boundary guarantee. The user-visible promise is that
    /// **no body ever crosses the window frame**, period. Rapier's velocity
    /// solver normally enforces this via the four wall colliders, but in
    /// extreme cases (high-impulse virus → bubble collision combined with
    /// the per-frame upward float force) the position integrator can
    /// briefly resolve outside the wall before the next contact step pulls
    /// it back. CCD eliminates most of that — this method is the safety
    /// net for the rest.
    ///
    /// For every dynamic body (virus, solid bubble, dead virus, growing
    /// bubble): if the centre lies outside `[radius, W-radius] ×
    /// [radius, H-radius]`, snap it back inside and zero the
    /// outward-pointing velocity component so the next step doesn't immediately
    /// shoot back through.
    ///
    /// Call after `step` and before `sync_to_world` so the world view
    /// observes the corrected positions.
    pub fn clamp_to_playfield(&mut self, world: &World) {
        let w = self.game_width;
        let h = self.game_height;
        for v in &world.viruses {
            if let Some(handle) = v.body {
                clamp_body_inside(&mut self.bodies, handle, crate::consts::VIRUS_RADIUS, w, h);
            }
        }
        for b in &world.solid_bubbles {
            if let Some(handle) = b.body {
                clamp_body_inside(&mut self.bodies, handle, b.radius, w, h);
            }
        }
        for d in &world.dead_viruses {
            if let Some(handle) = d.body {
                clamp_body_inside(&mut self.bodies, handle, d.radius, w, h);
            }
        }
        if let Some(g) = &world.growing {
            if let Some(handle) = g.body {
                clamp_body_inside(&mut self.bodies, handle, g.radius, w, h);
            }
        }
    }

    /// Hard non-interpenetration guarantee for body↔body pairs. Like
    /// [`Self::clamp_to_playfield`] but for circle-on-circle overlap.
    ///
    /// Why this is needed: rapier's velocity-based solver allows a small
    /// amount of contact penetration as solver slack. Under the continuous
    /// upward float force, a bubble that's pinned between another bubble
    /// and the top wall can never fully resolve that slack — every step the
    /// solver lets it drift a tiny bit further into its neighbours, and
    /// over many seconds the drift accumulates into the visible "creeping
    /// inside another bubble" symptom (and eventually past the wall edge).
    ///
    /// We run a Gauss-Seidel separation pass: for every pair of dynamic
    /// circles whose centre distance is less than the sum of their radii,
    /// push them apart along the contact normal by half the depth each and
    /// remove the velocity component that's still pushing them together.
    /// Then re-clamp to the playfield. Iterating this with [`Self::clamp_to_playfield`]
    /// handles cluster cases (a bubble pushed off another bubble may then
    /// poke through a wall, which the next clamp catches).
    ///
    /// Call sequence in [`crate::game::update::tick`] is:
    /// `step → enforce_no_interpenetration → sync_to_world`.
    pub fn enforce_no_interpenetration(&mut self, world: &World) {
        // Snapshot every dynamic-circle handle + its radius once. Walls are
        // static and aren't dynamic bodies in our world, so they're naturally
        // skipped — `clamp_to_playfield` handles wall containment for us.
        //
        // The **growing bubble is intentionally excluded** from pair
        // separation. Its position is teleported every frame by
        // `set_body_position` (driven by the pointer), and its game-logic
        // collisions (virus → life-loss, solid bubble / dead virus →
        // solidify) are handled by `check_virus_growing_bubble_collision`
        // and `grow_bubble`. Forcibly separating viruses from the growing
        // bubble here would defeat both checks because the virus would be
        // shoved out of overlap before the polling distance comparison fires.
        let mut entities: Vec<(RigidBodyHandle, f32)> = Vec::with_capacity(
            world.viruses.len() + world.solid_bubbles.len() + world.dead_viruses.len(),
        );
        for v in &world.viruses {
            if let Some(handle) = v.body {
                entities.push((handle, crate::consts::VIRUS_RADIUS));
            }
        }
        for b in &world.solid_bubbles {
            if let Some(handle) = b.body {
                entities.push((handle, b.radius));
            }
        }
        for d in &world.dead_viruses {
            if let Some(handle) = d.body {
                entities.push((handle, d.radius));
            }
        }

        // Iterate Gauss-Seidel sweeps: separate every pair, then clamp every
        // body back inside the playfield, then repeat. A long chain of
        // bubbles (A pushes B, B pushes C, …) needs multiple sweeps because
        // each pair only resolves itself, and re-clamping pushes wall-pinned
        // bubbles back into their neighbours. 16 passes is overkill for the
        // body counts we have but cheap enough at our scale (≤ ~30 bodies)
        // and gives the chain plenty of time to converge.
        const PASSES: usize = 16;
        for _ in 0..PASSES {
            for i in 0..entities.len() {
                for j in (i + 1)..entities.len() {
                    self.separate_pair(entities[i], entities[j]);
                }
            }
            self.clamp_to_playfield(world);
        }
    }

    /// Separate two circles if they overlap. Half the depth is pushed into
    /// each body, and the relative velocity along the contact normal is
    /// zeroed if it's still closing.
    fn separate_pair(&mut self, a: (RigidBodyHandle, f32), b: (RigidBodyHandle, f32)) {
        let (h_a, r_a_px) = a;
        let (h_b, r_b_px) = b;

        // Read both bodies (immutably) into locals so we can drop the borrows
        // before mutating.
        let (a_x, a_y, a_vx, a_vy) = match self.bodies.get(h_a) {
            Some(rb) => {
                let p = rb.translation();
                let v = rb.linvel();
                (p.x, p.y, v.x, v.y)
            }
            None => return,
        };
        let (b_x, b_y, b_vx, b_vy) = match self.bodies.get(h_b) {
            Some(rb) => {
                let p = rb.translation();
                let v = rb.linvel();
                (p.x, p.y, v.x, v.y)
            }
            None => return,
        };

        let r_sum = to_meters(r_a_px) + to_meters(r_b_px);
        let dx = b_x - a_x;
        let dy = b_y - a_y;
        let d_sq = dx * dx + dy * dy;
        if d_sq >= r_sum * r_sum {
            return; // Not overlapping.
        }

        let d = d_sq.sqrt();
        // Pick a stable normal even if centres coincide.
        let (nx, ny) = if d > 1e-6 {
            (dx / d, dy / d)
        } else {
            (1.0, 0.0)
        };
        let depth = r_sum - d;
        let half = depth * 0.5;

        let new_a_x = a_x - nx * half;
        let new_a_y = a_y - ny * half;
        let new_b_x = b_x + nx * half;
        let new_b_y = b_y + ny * half;

        // Remove the velocity component along the normal that's still
        // closing (negative relative-velocity-along-normal means approach).
        let vrel_n = (b_vx - a_vx) * nx + (b_vy - a_vy) * ny;
        let (new_a_vx, new_a_vy, new_b_vx, new_b_vy) = if vrel_n < 0.0 {
            let half_dvn = -vrel_n * 0.5;
            (
                a_vx - nx * half_dvn,
                a_vy - ny * half_dvn,
                b_vx + nx * half_dvn,
                b_vy + ny * half_dvn,
            )
        } else {
            (a_vx, a_vy, b_vx, b_vy)
        };

        if let Some(rb) = self.bodies.get_mut(h_a) {
            rb.set_translation(vector![new_a_x, new_a_y], true);
            rb.set_linvel(vector![new_a_vx, new_a_vy], true);
        }
        if let Some(rb) = self.bodies.get_mut(h_b) {
            rb.set_translation(vector![new_b_x, new_b_y], true);
            rb.set_linvel(vector![new_b_vx, new_b_vy], true);
        }
    }

    /// Push physics-world body positions back into the game-state entities.
    /// Mirrors `syncBodiesToGameObjects`.
    pub fn sync_to_world(&self, world: &mut World) {
        for v in world.viruses.iter_mut() {
            let Some(h) = v.body else { continue };
            let Some(rb) = self.bodies.get(h) else {
                continue;
            };
            let p = rb.translation();
            let lv = rb.linvel();
            v.x = to_pixels(p.x);
            v.y = to_pixels(p.y);
            v.vx = to_pixels(lv.x);
            v.vy = to_pixels(lv.y);
        }
        for b in world.solid_bubbles.iter_mut() {
            let Some(h) = b.body else { continue };
            let Some(rb) = self.bodies.get(h) else {
                continue;
            };
            let p = rb.translation();
            let lv = rb.linvel();
            b.x = to_pixels(p.x);
            b.y = to_pixels(p.y);
            b.vx = to_pixels(lv.x);
            b.vy = to_pixels(lv.y);
        }
        for d in world.dead_viruses.iter_mut() {
            let Some(h) = d.body else { continue };
            let Some(rb) = self.bodies.get(h) else {
                continue;
            };
            let p = rb.translation();
            let lv = rb.linvel();
            d.x = to_pixels(p.x);
            d.y = to_pixels(p.y);
            d.vy = to_pixels(lv.y);
        }
        if let Some(g) = world.growing.as_mut() {
            if let Some(h) = g.body {
                if let Some(rb) = self.bodies.get(h) {
                    let p = rb.translation();
                    g.x = to_pixels(p.x);
                    g.y = to_pixels(p.y);
                }
            }
        }
    }

    /// Apply the per-frame upward float force to every solid bubble.
    /// `force` is in pixels/s²; converted internally.
    /// Apply the JS reference's constant upward float force to every bubble,
    /// suppressed once the bubble is already moving up faster than `max_speed_px`
    /// so small bubbles don't rocket past it.
    ///
    /// Why a cap rather than mass-proportional force: a clean
    /// "every bubble's terminal velocity = target" model leaves bubbles too
    /// calm — they pile against the top wall in a stable cluster and any
    /// virus that gets wedged inside has no escape route, so its 3-second
    /// trap timer fires and it dies. The constant-force model creates the
    /// natural agitation that breaks clusters open. The cap keeps the small
    /// bubbles from feeling "way too fast."
    pub fn apply_bubble_float(&mut self, world: &World, force_px: f32, max_speed_px: f32) {
        let force = vector![0.0, -to_meters(force_px)];
        // Y-down convention: moving up = negative Y velocity. Cap the upward
        // motion at -max_speed (more negative = faster up).
        let max_v_up_m = -to_meters(max_speed_px);
        for b in &world.solid_bubbles {
            let Some(h) = b.body else { continue };
            let Some(rb) = self.bodies.get_mut(h) else {
                continue;
            };
            if rb.linvel().y > max_v_up_m {
                rb.add_force(force, true);
            }
        }
    }

    /// Apply downward gravity to dead viruses (so they sink despite damping).
    pub fn apply_dead_virus_gravity(&mut self, world: &World, sink_speed: f32) {
        let force = vector![0.0, to_meters(sink_speed * 2.0)];
        for d in &world.dead_viruses {
            let Some(h) = d.body else { continue };
            let Some(rb) = self.bodies.get_mut(h) else {
                continue;
            };
            rb.add_force(force, true);
        }
    }

    /// Re-scale virus velocities to `target_speed` after running wall-slide
    /// correction. Mirrors `maintainVirusSpeeds` in the JS reference.
    pub fn maintain_virus_speeds(&mut self, world: &mut World, target_speed: f32) {
        for v in world.viruses.iter_mut() {
            let Some(h) = v.body else {
                continue;
            };
            let Some(rb) = self.bodies.get_mut(h) else {
                continue;
            };
            let p = rb.translation();
            let lv = rb.linvel();
            let speed = (lv.x * lv.x + lv.y * lv.y).sqrt();
            if speed <= 0.01 {
                continue;
            }

            let pixel_x = to_pixels(p.x);
            let pixel_y = to_pixels(p.y);
            let pixel_vx = to_pixels(lv.x);
            let pixel_vy = to_pixels(lv.y);

            let (cvx, cvy) = correct_wall_slide_velocity(
                pixel_x,
                pixel_y,
                pixel_vx,
                pixel_vy,
                self.game_width,
                self.game_height,
            );
            let corrected_speed = (cvx * cvx + cvy * cvy).sqrt();
            if corrected_speed <= 0.01 {
                continue;
            }
            let scale = target_speed / corrected_speed;
            rb.set_linvel(
                vector![to_meters(cvx * scale), to_meters(cvy * scale)],
                true,
            );
        }
    }
}
