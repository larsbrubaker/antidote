//! Tests for [`crate::game::physics`]. Split out into a sibling file so the
//! production module stays under the project-wide 800-line cap enforced by
//! `tests/file_line_count.rs`.

use super::physics::{correct_wall_slide_velocity, PhysicsWorld};
use crate::game::state::{Bubble, Virus, World};

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
    assert_eq!(world.bodies.len(), 4);
    assert_eq!(world.colliders.len(), 4);
}

#[test]
fn step_runs_without_panic() {
    let mut world = PhysicsWorld::new(800.0, 600.0);
    world.step(1.0 / 60.0);
    world.step(1.0 / 60.0);
}

/// Regression: a stack of bubbles under continuous upward float force must
/// never interpenetrate, even when pinned against the top wall for many
/// seconds. Mirrors the screenshot Lars sent where one bubble had crept
/// inside another bubble (and through the playfield's top edge) after the
/// cluster sat at the top wall under constant upward pressure. The fix is
/// the post-step `enforce_no_interpenetration` pass; without it, after a
/// few seconds two adjacent bubbles' centres approach each other by 4+ px
/// past the sum of radii.
#[test]
fn stacked_bubbles_under_float_force_dont_interpenetrate() {
    use crate::consts::BUBBLE_FLOAT_SPEED;

    let mut phys = PhysicsWorld::new(800.0, 600.0);
    let mut world = World::new();

    // Six bubbles in a vertical column near the top of the playfield, already
    // touching, all primed with upward velocity. The top of the column starts
    // only 100 px below the wall, so the float force will pin the cluster
    // against the ceiling within a couple seconds.
    let radius = 25.0;
    let column_x = 300.0;
    for i in 0..6 {
        let y = 100.0 + (i as f32) * radius * 2.0;
        world.solid_bubbles.push(Bubble {
            x: column_x,
            y,
            radius,
            vx: 0.0,
            vy: -BUBBLE_FLOAT_SPEED,
            body: None,
        });
    }
    for b in world.solid_bubbles.iter_mut() {
        phys.spawn_bubble_body(b);
    }

    for _ in 0..(8 * 60) {
        phys.apply_bubble_float(&world, BUBBLE_FLOAT_SPEED * 2.0, BUBBLE_FLOAT_SPEED * 1.5);
        phys.step(1.0 / 60.0);
        phys.enforce_no_interpenetration(&world);
        phys.sync_to_world(&mut world);
    }

    const SLOP_PX: f32 = 1.0;
    for i in 0..world.solid_bubbles.len() {
        for j in (i + 1)..world.solid_bubbles.len() {
            let a = &world.solid_bubbles[i];
            let b = &world.solid_bubbles[j];
            let d = ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
            let min_d = a.radius + b.radius - SLOP_PX;
            assert!(
                d >= min_d,
                "bubbles {i} and {j} interpenetrating: d={d:.2}, min={min_d:.2}"
            );
        }
        let b = &world.solid_bubbles[i];
        assert!(
            b.x >= b.radius - SLOP_PX && b.x <= 800.0 - b.radius + SLOP_PX,
            "bubble {i} escaped horizontally to x={}",
            b.x
        );
        assert!(
            b.y >= b.radius - SLOP_PX && b.y <= 600.0 - b.radius + SLOP_PX,
            "bubble {i} escaped vertically to y={}",
            b.y
        );
    }
}

/// Regression: a bubble launched at ten times its terminal speed straight up
/// at the wall must never exit the playfield. Mirrors the "throws bubbles
/// above the top of the window" symptom Lars reported — the safety net is
/// `clamp_to_playfield`. We sim 5 seconds at 60 Hz.
#[test]
fn bubble_never_escapes_playfield_under_extreme_velocity() {
    let mut phys = PhysicsWorld::new(800.0, 600.0);
    let mut world = World::new();
    world.solid_bubbles.push(Bubble {
        x: 400.0,
        y: 30.0,
        radius: 25.0,
        vx: 0.0,
        vy: -1500.0,
        body: None,
    });
    phys.spawn_bubble_body(&mut world.solid_bubbles[0]);

    for _ in 0..(5 * 60) {
        phys.apply_bubble_float(
            &world,
            crate::consts::BUBBLE_FLOAT_SPEED * 2.0,
            crate::consts::BUBBLE_FLOAT_SPEED * 1.5,
        );
        phys.step(1.0 / 60.0);
        phys.clamp_to_playfield(&world);
        phys.sync_to_world(&mut world);
        let b = &world.solid_bubbles[0];
        assert!(
            b.x >= b.radius - 0.001 && b.x <= 800.0 - b.radius + 0.001,
            "bubble left horizontally to x={}",
            b.x
        );
        assert!(
            b.y >= b.radius - 0.001 && b.y <= 600.0 - b.radius + 0.001,
            "bubble escaped to y={}",
            b.y
        );
    }
}

#[test]
fn virus_spawn_step_sync_round_trip() {
    let mut phys = PhysicsWorld::new(800.0, 600.0);
    let mut world = World::new();
    world.viruses.push(Virus {
        x: 200.0,
        y: 300.0,
        vx: 100.0,
        vy: 0.0,
        phase: 0.0,
        last_unstuck_x: 200.0,
        last_unstuck_y: 300.0,
        stuck_time: 0.0,
        speed: 100.0,
        body: None,
    });
    let radius = crate::consts::VIRUS_RADIUS;
    phys.spawn_virus_body(&mut world.viruses[0], radius);
    assert!(world.viruses[0].body.is_some());

    for _ in 0..30 {
        phys.step(1.0 / 60.0);
    }
    phys.sync_to_world(&mut world);
    assert!(world.viruses[0].x > 200.0, "virus did not move right");
}

#[test]
fn virus_collision_pushes_solid_bubble() {
    let mut phys = PhysicsWorld::new(800.0, 600.0);
    let mut world = World::new();

    world.solid_bubbles.push(Bubble {
        x: 300.0,
        y: 300.0,
        radius: 30.0,
        vx: 0.0,
        vy: 0.0,
        body: None,
    });
    phys.spawn_bubble_body(&mut world.solid_bubbles[0]);

    world.viruses.push(Virus {
        x: 240.0,
        y: 300.0,
        vx: 120.0,
        vy: 0.0,
        phase: 0.0,
        last_unstuck_x: 240.0,
        last_unstuck_y: 300.0,
        stuck_time: 0.0,
        speed: 120.0,
        body: None,
    });
    phys.spawn_virus_body(&mut world.viruses[0], crate::consts::VIRUS_RADIUS);

    for _ in 0..45 {
        phys.step(1.0 / 60.0);
        phys.sync_to_world(&mut world);
        phys.maintain_virus_speeds(&mut world, 120.0);
    }

    assert!(
        world.solid_bubbles[0].x > 302.0,
        "virus did not transfer visible momentum to bubble: x={}",
        world.solid_bubbles[0].x
    );
}
