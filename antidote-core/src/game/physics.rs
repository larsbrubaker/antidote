//! box2d-rust-driven physics. Mirrors the body/fixture parameters from the JS
//! reference (`gfg/public/games/antidote/antidote-physics.js`) so
//! gameplay feel is preserved. box2d-rust is a pure-Rust port of Box2D v3 —
//! the same engine family as the Planck.js the reference ran on, so contact
//! mixing (friction = sqrt(f1*f2), restitution = max) matches the original
//! with no overrides.
//!
//! Body parameters (ported verbatim from the JS reference):
//! - Wall:           static, friction 0.1, restitution 0.3
//! - Bubble:         dynamic, density 0.5, friction 0.1, restitution 0.4, linear_damping 0.5, rotation locked
//! - Virus:          dynamic, density 1.0, friction 0.0, restitution 1.0, bullet (CCD), rotation locked
//! - Dead virus:     dynamic, density 2.0, friction 0.5, restitution 0.1, linear_damping 2.0, rotation locked
//! - Growing bubble: dynamic, density 0.1, friction 0.1, restitution 0.0, rotation locked, never sleeps
//!
//! Step: Box2D v3 sub-stepping, 4 sub-steps (replaces Planck's 8 velocity /
//! 3 position iterations — v3's soft-constraint solver uses sub-steps instead).

use crate::consts::{
    to_meters, to_pixels, MIN_PERPENDICULAR_RATIO, PIXELS_PER_METER, WALL_PROXIMITY_THRESHOLD,
};
use crate::game::state::{Bubble, DeadVirus, GrowingBubble, Virus, World};
use box2d_rust::body::{
    body_apply_force_to_center, body_get_linear_velocity, body_get_position, body_get_shape_count,
    body_get_shapes, body_is_valid, body_set_linear_velocity, body_set_transform, create_body,
    destroy_body,
};
use box2d_rust::collision::Circle;
use box2d_rust::geometry::make_box;
use box2d_rust::id::BodyId;
use box2d_rust::math_functions::{to_pos, Vec2, ROT_IDENTITY, VEC2_ZERO};
use box2d_rust::shape::{create_circle_shape, create_polygon_shape, destroy_shape};
use box2d_rust::types::{
    default_body_def, default_shape_def, default_world_def, BodyType, MotionLocks, ShapeDef,
};
use box2d_rust::world::{world_step, World as B2World};

/// Box2D v3 sub-step count. The v3 solver replaces v2's velocity/position
/// iteration pair (Planck used 8/3) with sub-stepping; 4 is the upstream
/// recommendation.
const SUB_STEP_COUNT: i32 = 4;

/// Collision-filter categories. Match the JS reference's bitmasks exactly.
pub mod category {
    pub const WALL: u64 = 1 << 0;
    pub const BUBBLE: u64 = 1 << 1;
    pub const VIRUS: u64 = 1 << 2;
    pub const DEAD_VIRUS: u64 = 1 << 3;
    pub const GROWING_BUBBLE: u64 = 1 << 4;
}

/// Free-standing helper for [`PhysicsWorld::clamp_to_playfield`]. Snaps a
/// body's centre back into the playfield rectangle (in pixels) and kills the
/// outward-pointing velocity component so the next step doesn't immediately
/// re-escape.
fn clamp_body_inside(
    world: &mut B2World,
    id: BodyId,
    radius_px: f32,
    game_width_px: f32,
    game_height_px: f32,
) {
    if !body_is_valid(world, id) {
        return;
    }
    let r_m = to_meters(radius_px);
    let w_m = to_meters(game_width_px);
    let h_m = to_meters(game_height_px);
    let p = body_get_position(world, id);
    let v = body_get_linear_velocity(world, id);
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
        body_set_transform(world, id, to_pos(Vec2 { x: new_x, y: new_y }), ROT_IDENTITY);
        body_set_linear_velocity(
            world,
            id,
            Vec2 {
                x: new_vx,
                y: new_vy,
            },
        );
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

/// box2d-rust-backed physics simulation. One owned Box2D world; entity
/// handles are generational [`BodyId`]s stored on the game-state structs.
pub struct PhysicsWorld {
    pub world: B2World,
    pub game_width: f32,
    pub game_height: f32,
    wall_bodies: Vec<BodyId>,
}

impl PhysicsWorld {
    pub fn new(game_width: f32, game_height: f32) -> Self {
        let mut def = default_world_def();
        // The JS reference runs a zero-gravity world: bubbles float and dead
        // viruses sink via explicit per-frame forces. box2d-rust's default is
        // earth gravity (0, -10) — with our Y-down convention that would pull
        // everything *up*, so it must be zeroed.
        def.gravity = VEC2_ZERO;
        let world = B2World::new(&def);

        let mut physics = Self {
            world,
            game_width,
            game_height,
            wall_bodies: Vec::new(),
        };

        physics.create_walls();
        physics
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

        for (cx, cy, hex, hey) in walls {
            let mut body_def = default_body_def(); // BodyType::Static
            body_def.position = to_pos(Vec2 { x: cx, y: cy });
            let id = create_body(&mut self.world, &body_def);

            let mut shape_def = default_shape_def();
            shape_def.material.friction = 0.1;
            shape_def.material.restitution = 0.3;
            shape_def.filter.category_bits = category::WALL;
            shape_def.filter.mask_bits = category::BUBBLE
                | category::VIRUS
                | category::DEAD_VIRUS
                | category::GROWING_BUBBLE;
            create_polygon_shape(&mut self.world, id, &shape_def, &make_box(hex, hey));
            self.wall_bodies.push(id);
        }
    }

    /// Step the simulation with Box2D v3 sub-stepping.
    pub fn step(&mut self, dt: f32) {
        world_step(&mut self.world, dt, SUB_STEP_COUNT);
    }

    /// Convert pixel-space coords to physics translation (meters).
    #[inline]
    pub fn pixels_to_meters(px: f32) -> f32 {
        px / PIXELS_PER_METER
    }

    /// Body count including the four walls. Exposed for tests.
    pub fn body_count(&self) -> i32 {
        box2d_rust::world::world_get_counters(&self.world).body_count
    }

    /// Shape count including the four wall boxes. Exposed for tests.
    pub fn shape_count(&self) -> i32 {
        box2d_rust::world::world_get_counters(&self.world).shape_count
    }

    /// Shared ShapeDef for the dynamic circles; per-body material params.
    fn circle_shape_def(
        density: f32,
        friction: f32,
        restitution: f32,
        category_bits: u64,
        mask_bits: u64,
    ) -> ShapeDef {
        let mut def = default_shape_def();
        def.density = density;
        def.material.friction = friction;
        def.material.restitution = restitution;
        def.filter.category_bits = category_bits;
        def.filter.mask_bits = mask_bits;
        def
    }

    // ---- body creation (params match `antidote-physics.js`) ----

    /// `createVirusBody` — dynamic, density 1.0, friction 0.0, restitution 1.0,
    /// bullet (CCD), rotation locked.
    pub fn spawn_virus_body(&mut self, virus: &mut Virus, radius: f32) -> BodyId {
        let mut body_def = default_body_def();
        body_def.type_ = BodyType::Dynamic;
        body_def.position = to_pos(Vec2 {
            x: to_meters(virus.x),
            y: to_meters(virus.y),
        });
        body_def.linear_velocity = Vec2 {
            x: to_meters(virus.vx),
            y: to_meters(virus.vy),
        };
        body_def.motion_locks = MotionLocks {
            angular_z: true,
            ..Default::default()
        };
        // Bullet CCD on viruses only, exactly like the JS reference — they
        // are the fast movers.
        body_def.is_bullet = true;
        let id = create_body(&mut self.world, &body_def);

        let shape_def = Self::circle_shape_def(
            1.0,
            0.0,
            1.0,
            category::VIRUS,
            category::WALL
                | category::BUBBLE
                | category::VIRUS
                | category::DEAD_VIRUS
                | category::GROWING_BUBBLE,
        );
        create_circle_shape(
            &mut self.world,
            id,
            &shape_def,
            &Circle {
                center: VEC2_ZERO,
                radius: to_meters(radius),
            },
        );
        virus.body = Some(id);
        id
    }

    /// `createBubbleBody` — dynamic, density 0.5, friction 0.1, restitution 0.4,
    /// linear damping 0.5, rotation locked. Sleeping stays enabled: the
    /// per-frame float force is applied with `wake = true`, which is exactly
    /// how the Planck reference kept bubbles live.
    pub fn spawn_bubble_body(&mut self, bubble: &mut Bubble) -> BodyId {
        let mut body_def = default_body_def();
        body_def.type_ = BodyType::Dynamic;
        body_def.position = to_pos(Vec2 {
            x: to_meters(bubble.x),
            y: to_meters(bubble.y),
        });
        body_def.linear_velocity = Vec2 {
            x: to_meters(bubble.vx),
            y: to_meters(bubble.vy),
        };
        body_def.linear_damping = 0.5;
        body_def.motion_locks = MotionLocks {
            angular_z: true,
            ..Default::default()
        };
        let id = create_body(&mut self.world, &body_def);

        let shape_def = Self::circle_shape_def(
            0.5,
            0.1,
            0.4,
            category::BUBBLE,
            category::WALL | category::BUBBLE | category::VIRUS | category::DEAD_VIRUS,
        );
        create_circle_shape(
            &mut self.world,
            id,
            &shape_def,
            &Circle {
                center: VEC2_ZERO,
                radius: to_meters(bubble.radius),
            },
        );
        bubble.body = Some(id);
        id
    }

    /// `createDeadVirusBody` — dynamic, density 2.0, friction 0.5, restitution 0.1,
    /// linear damping 2.0, rotation locked.
    pub fn spawn_dead_virus_body(&mut self, dv: &mut DeadVirus) -> BodyId {
        let mut body_def = default_body_def();
        body_def.type_ = BodyType::Dynamic;
        body_def.position = to_pos(Vec2 {
            x: to_meters(dv.x),
            y: to_meters(dv.y),
        });
        body_def.linear_velocity = Vec2 {
            x: 0.0,
            y: to_meters(dv.vy),
        };
        body_def.linear_damping = 2.0;
        body_def.motion_locks = MotionLocks {
            angular_z: true,
            ..Default::default()
        };
        let id = create_body(&mut self.world, &body_def);

        let shape_def = Self::circle_shape_def(
            2.0,
            0.5,
            0.1,
            category::DEAD_VIRUS,
            category::WALL | category::BUBBLE | category::VIRUS | category::DEAD_VIRUS,
        );
        create_circle_shape(
            &mut self.world,
            id,
            &shape_def,
            &Circle {
                center: VEC2_ZERO,
                radius: to_meters(dv.radius),
            },
        );
        dv.body = Some(id);
        id
    }

    /// `createGrowingBubbleBody` — dynamic, density 0.1, friction 0.1,
    /// restitution 0.0, rotation locked. Only collides with walls + viruses.
    pub fn spawn_growing_bubble_body(&mut self, g: &mut GrowingBubble) -> BodyId {
        let mut body_def = default_body_def();
        body_def.type_ = BodyType::Dynamic;
        body_def.position = to_pos(Vec2 {
            x: to_meters(g.x),
            y: to_meters(g.y),
        });
        body_def.motion_locks = MotionLocks {
            angular_z: true,
            ..Default::default()
        };
        // The growing bubble is teleported to the pointer every frame via
        // `body_set_transform`, which does NOT wake a sleeping body. It also
        // never has forces applied. If it dozed off it would stop pushing
        // viruses, so it opts out of sleeping entirely.
        body_def.enable_sleep = false;
        let id = create_body(&mut self.world, &body_def);

        let shape_def = Self::circle_shape_def(
            0.1,
            0.1,
            0.0,
            category::GROWING_BUBBLE,
            category::WALL | category::VIRUS,
        );
        create_circle_shape(
            &mut self.world,
            id,
            &shape_def,
            &Circle {
                center: VEC2_ZERO,
                radius: to_meters(g.radius),
            },
        );
        g.body = Some(id);
        id
    }

    /// Force a body's translation in pixels. Used to keep the growing-bubble
    /// physics body at the pointer position each frame.
    pub fn set_body_position(&mut self, id: BodyId, x: f32, y: f32) {
        if !body_is_valid(&self.world, id) {
            return;
        }
        body_set_transform(
            &mut self.world,
            id,
            to_pos(Vec2 {
                x: to_meters(x),
                y: to_meters(y),
            }),
            ROT_IDENTITY,
        );
    }

    /// Reset a body's linear velocity to zero. Useful in tests that teleport
    /// a body to a known location and don't want stale velocity carrying it
    /// back into something on the next step.
    pub fn zero_body_velocity(&mut self, id: BodyId) {
        if !body_is_valid(&self.world, id) {
            return;
        }
        body_set_linear_velocity(&mut self.world, id, VEC2_ZERO);
    }

    /// Replace the growing-bubble's circle shape with a new one of the given
    /// radius. Mirrors `updateGrowingBubbleRadius` in the JS reference
    /// (destroy fixture, create fixture).
    pub fn resize_growing_bubble_collider(&mut self, id: BodyId, new_radius: f32) {
        if !body_is_valid(&self.world, id) {
            return;
        }
        let count = body_get_shape_count(&self.world, id) as usize;
        for shape_id in body_get_shapes(&self.world, id, count) {
            destroy_shape(&mut self.world, shape_id, true);
        }
        let shape_def = Self::circle_shape_def(
            0.1,
            0.1,
            0.0,
            category::GROWING_BUBBLE,
            category::WALL | category::VIRUS,
        );
        create_circle_shape(
            &mut self.world,
            id,
            &shape_def,
            &Circle {
                center: VEC2_ZERO,
                radius: to_meters(new_radius),
            },
        );
    }

    /// Remove a body + its shapes from the world. Safe to call with a stale
    /// handle (checked via the generational id).
    pub fn destroy_body(&mut self, id: BodyId) {
        if !body_is_valid(&self.world, id) {
            return;
        }
        destroy_body(&mut self.world, id);
    }

    /// Hard playfield-boundary guarantee. The user-visible promise is that
    /// **no body ever crosses the window frame**, period. The four wall
    /// colliders enforce this in the normal case; in extreme ones (a
    /// high-impulse virus → bubble collision combined with the per-frame
    /// upward float force) the position integrator can briefly resolve
    /// outside the wall before the next contact step pulls it back. This
    /// method is the safety net for those.
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
            if let Some(id) = v.body {
                clamp_body_inside(&mut self.world, id, crate::consts::VIRUS_RADIUS, w, h);
            }
        }
        for b in &world.solid_bubbles {
            if let Some(id) = b.body {
                clamp_body_inside(&mut self.world, id, b.radius, w, h);
            }
        }
        for d in &world.dead_viruses {
            if let Some(id) = d.body {
                clamp_body_inside(&mut self.world, id, d.radius, w, h);
            }
        }
        if let Some(g) = &world.growing {
            if let Some(id) = g.body {
                clamp_body_inside(&mut self.world, id, g.radius, w, h);
            }
        }
    }

    /// Push physics-world body positions back into the game-state entities.
    /// Mirrors `syncBodiesToGameObjects`.
    pub fn sync_to_world(&self, world: &mut World) {
        for v in world.viruses.iter_mut() {
            let Some(id) = v.body else { continue };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            let p = body_get_position(&self.world, id);
            let lv = body_get_linear_velocity(&self.world, id);
            v.x = to_pixels(p.x);
            v.y = to_pixels(p.y);
            v.vx = to_pixels(lv.x);
            v.vy = to_pixels(lv.y);
        }
        for b in world.solid_bubbles.iter_mut() {
            let Some(id) = b.body else { continue };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            let p = body_get_position(&self.world, id);
            let lv = body_get_linear_velocity(&self.world, id);
            b.x = to_pixels(p.x);
            b.y = to_pixels(p.y);
            b.vx = to_pixels(lv.x);
            b.vy = to_pixels(lv.y);
        }
        for d in world.dead_viruses.iter_mut() {
            let Some(id) = d.body else { continue };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            let p = body_get_position(&self.world, id);
            let lv = body_get_linear_velocity(&self.world, id);
            d.x = to_pixels(p.x);
            d.y = to_pixels(p.y);
            d.vy = to_pixels(lv.y);
        }
        if let Some(g) = world.growing.as_mut() {
            if let Some(id) = g.body {
                if body_is_valid(&self.world, id) {
                    let p = body_get_position(&self.world, id);
                    g.x = to_pixels(p.x);
                    g.y = to_pixels(p.y);
                }
            }
        }
    }

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
    ///
    /// `wake = true` on the force keeps resting bubbles responsive — a
    /// sleeping bubble reads velocity 0, passes the cap test, and is woken by
    /// the force on the same frame.
    pub fn apply_bubble_float(&mut self, world: &World, force_px: f32, max_speed_px: f32) {
        let force = Vec2 {
            x: 0.0,
            y: -to_meters(force_px),
        };
        // Y-down convention: moving up = negative Y velocity. Cap the upward
        // motion at -max_speed (more negative = faster up).
        let max_v_up_m = -to_meters(max_speed_px);
        for b in &world.solid_bubbles {
            let Some(id) = b.body else { continue };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            if body_get_linear_velocity(&self.world, id).y > max_v_up_m {
                body_apply_force_to_center(&mut self.world, id, force, true);
            }
        }
    }

    /// Apply downward gravity to dead viruses (so they sink despite damping).
    pub fn apply_dead_virus_gravity(&mut self, world: &World, sink_speed: f32) {
        let force = Vec2 {
            x: 0.0,
            y: to_meters(sink_speed * 2.0),
        };
        for d in &world.dead_viruses {
            let Some(id) = d.body else { continue };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            body_apply_force_to_center(&mut self.world, id, force, true);
        }
    }

    /// Re-scale virus velocities to `target_speed` after running wall-slide
    /// correction. Mirrors `maintainVirusSpeeds` in the JS reference.
    pub fn maintain_virus_speeds(&mut self, world: &mut World, target_speed: f32) {
        for v in world.viruses.iter_mut() {
            let Some(id) = v.body else {
                continue;
            };
            if !body_is_valid(&self.world, id) {
                continue;
            }
            let p = body_get_position(&self.world, id);
            let lv = body_get_linear_velocity(&self.world, id);
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
            body_set_linear_velocity(
                &mut self.world,
                id,
                Vec2 {
                    x: to_meters(cvx * scale),
                    y: to_meters(cvy * scale),
                },
            );
        }
    }
}
