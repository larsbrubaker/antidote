//! Per-frame tick + pointer-input handlers. Mirrors the gameplay surface of
//! `gfg/public/games/antidote/antidote.js`.

use crate::consts::{
    min_antidote_cost, ANTIDOTE_DRAIN_RATE, BUBBLE_FLOAT_SPEED, BUBBLE_GROW_RATE,
    DEAD_VIRUS_SINK_SPEED, MIN_VALID_RADIUS, SLIDE_OUT_SPEED, VIRTUAL_HEIGHT, VIRTUAL_WIDTH,
    VIRUS_RADIUS, VIRUS_TRAP_DISTANCE, VIRUS_TRAP_TIME,
};
use crate::game::level::{init_level, virus_speed_for_level};
use crate::game::physics::PhysicsWorld;
use crate::game::state::{
    Bubble, DeadVirus, DyingVirus, GrowingBubble, InitialOverlap, Phase, PopAnimation, World,
};

/// Pop animation runs from `progress = 0` to `1` over this many seconds.
pub const POP_ANIMATION_DURATION: f32 = 0.3;
/// Dying virus death_progress runs from 0 to 1 over this many seconds.
pub const DYING_VIRUS_DURATION: f32 = 0.8;
/// The JS reference holds the life-lost state for this long before resuming.
pub const LIFE_LOST_DURATION: f32 = 1.2;

/// One simulation tick. Called from `GameWidget::paint` once per frame
/// (or from a dedicated game-tick path if/when one is split out).
pub fn tick(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    // The JS reference advances pop rings regardless of game state so a bubble
    // popped by life loss or level completion can finish animating under overlays.
    advance_pop_animations(world, dt);

    if world.phase == Phase::LifeLost {
        advance_life_lost(world, physics, dt);
        return;
    }

    if world.phase != Phase::Playing {
        return;
    }

    grow_bubble(world, physics, dt);

    // JS-faithful constant force (`BUBBLE_FLOAT_SPEED * 2`) with an
    // additional upward-speed cap of `BUBBLE_FLOAT_SPEED * 1.5` so small
    // bubbles don't rocket up at ~240 px/s. Both ingredients matter:
    // constant force keeps the cluster jittery enough for viruses to
    // escape, the cap keeps the small bubbles from feeling unhinged.
    physics.apply_bubble_float(world, BUBBLE_FLOAT_SPEED * 2.0, BUBBLE_FLOAT_SPEED * 1.5);
    physics.apply_dead_virus_gravity(world, DEAD_VIRUS_SINK_SPEED);

    physics.step(dt);
    // Hard guarantee the user explicitly asked for: no body ever crosses the
    // window frame. Box2D's contact solver handles body↔body resting contact
    // natively (the old rapier port needed a manual Gauss-Seidel separation
    // pass here); the playfield clamp stays as a cheap safety net for
    // extreme-impulse frames where the integrator briefly resolves outside
    // a wall.
    physics.clamp_to_playfield(world);
    physics.sync_to_world(world);

    let target_speed = virus_speed_for_level(world.level);
    physics.maintain_virus_speeds(world, target_speed);

    check_virus_growing_bubble_collision(world, physics);
    if world.phase != Phase::Playing {
        return;
    }

    update_trap_timers(world, physics, dt);
    advance_dying_viruses(world, physics, dt);
    check_level_complete(world);
}

// ---- pointer handlers (drive growing-bubble lifecycle) ----

pub fn on_pointer_down(world: &mut World, physics: &mut PhysicsWorld, x: f32, y: f32) {
    if world.phase != Phase::Playing {
        return;
    }
    world.pointer_down = true;
    world.pointer_x = x;
    world.pointer_y = y;

    if !is_valid_bubble_start(world, x, y) {
        return;
    }

    let initial_overlaps = compute_initial_overlaps(world, x, y, MIN_VALID_RADIUS);
    let mut g = GrowingBubble {
        x,
        y,
        radius: 0.0,
        growing: true,
        frozen: false,
        initial_overlaps,
        body: None,
    };
    physics.spawn_growing_bubble_body(&mut g);
    world.growing = Some(g);
    world.slide_out_charged = false;
}

pub fn on_pointer_move(world: &mut World, x: f32, y: f32) {
    world.pointer_x = x;
    world.pointer_y = y;
}

pub fn on_pointer_up(world: &mut World, physics: &mut PhysicsWorld) {
    world.pointer_down = false;
    solidify_bubble(world, physics);
}

// ---- helpers ----

fn is_valid_bubble_start(world: &World, x: f32, y: f32) -> bool {
    for v in &world.viruses {
        let dx = x - v.x;
        let dy = y - v.y;
        if (dx * dx + dy * dy).sqrt() < VIRUS_RADIUS + 20.0 {
            return false;
        }
    }
    true
}

fn compute_initial_overlaps(world: &World, x: f32, y: f32, radius: f32) -> Vec<InitialOverlap> {
    let mut overlaps = Vec::new();
    for (i, b) in world.solid_bubbles.iter().enumerate() {
        let dx = x - b.x;
        let dy = y - b.y;
        if (dx * dx + dy * dy).sqrt() < b.radius + radius {
            overlaps.push(InitialOverlap::Bubble(i));
        }
    }
    for (i, d) in world.dead_viruses.iter().enumerate() {
        let dx = x - d.x;
        let dy = y - d.y;
        if (dx * dx + dy * dy).sqrt() < d.radius + radius {
            overlaps.push(InitialOverlap::DeadVirus(i));
        }
    }
    overlaps
}

/// Port of `growBubble(dt)` in antidote.js. Combines slide-out, wall pinning,
/// solid-bubble / dead-virus collision-solidification, and antidote drain.
fn grow_bubble(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    let Some(g) = world.growing.as_mut() else {
        return;
    };
    if !g.growing {
        return;
    }

    let had_overlaps = !g.initial_overlaps.is_empty();

    if had_overlaps && !world.slide_out_charged {
        world.antidote = (world.antidote - min_antidote_cost()).max(0.0);
        world.slide_out_charged = true;
    }

    if !had_overlaps && world.pointer_down && !g.frozen {
        if let Some((cx, cy)) = constrain_bubble_position(
            world.pointer_x,
            world.pointer_y,
            g.radius,
            &world.solid_bubbles,
            &world.dead_viruses,
        ) {
            g.x = cx;
            g.y = cy;
            if let Some(h) = g.body {
                physics.set_body_position(h, cx, cy);
            }
        }
    }

    if had_overlaps {
        // Slide out gradually.
        g.radius = MIN_VALID_RADIUS;
        let mut still_overlapping = false;
        for overlap in g.initial_overlaps.clone() {
            let (ox, oy, or_) = match overlap {
                InitialOverlap::Bubble(i) => {
                    let b = &world.solid_bubbles[i];
                    (b.x, b.y, b.radius)
                }
                InitialOverlap::DeadVirus(i) => {
                    let d = &world.dead_viruses[i];
                    (d.x, d.y, d.radius)
                }
            };
            let dx = g.x - ox;
            let dy = g.y - oy;
            let dist = (dx * dx + dy * dy).sqrt();
            let min_dist = g.radius + or_;
            if dist < min_dist {
                still_overlapping = true;
                if dist > 0.01 {
                    let nx = dx / dist;
                    let ny = dy / dist;
                    g.x += nx * SLIDE_OUT_SPEED * dt;
                    g.y += ny * SLIDE_OUT_SPEED * dt;
                } else {
                    g.x += SLIDE_OUT_SPEED * dt;
                }
            }
        }
        if !still_overlapping {
            solidify_bubble(world, physics);
        }
        return;
    }

    // Normal growth, capped at the playfield's shorter axis. The moment the
    // bubble would span top-to-bottom (or left-to-right on a hypothetical
    // narrower playfield) we freeze it — bigger than the playfield is
    // unplayable and looks broken. Pin radius first, then position, so the
    // wall checks below don't run with an oversized radius.
    let max_radius = 0.5 * VIRTUAL_WIDTH.min(VIRTUAL_HEIGHT);
    g.radius += BUBBLE_GROW_RATE * dt;
    if g.radius >= max_radius {
        g.radius = max_radius;
        g.growing = false;
        g.frozen = true;
    }

    // Pin to walls (don't freeze on these). Use per-side checks rather than
    // `f32::clamp(g.radius, W - g.radius)`: with a square playfield the clamp
    // would hit its `min > max` failure mode the instant the bubble reaches
    // the cap above, and rust panics. This shape mirrors
    // antidote-core.js:222-233.
    if g.x - g.radius < 0.0 {
        g.x = g.radius;
    }
    if g.x + g.radius > VIRTUAL_WIDTH {
        g.x = VIRTUAL_WIDTH - g.radius;
    }
    if g.y - g.radius < 0.0 {
        g.y = g.radius;
    }
    if g.y + g.radius > VIRTUAL_HEIGHT {
        g.y = VIRTUAL_HEIGHT - g.radius;
    }

    // Resize the physics shape only when the radius has moved meaningfully —
    // tearing down + recreating a shape every frame is expensive. 0.5 px
    // is well below the visual difference but still tracks the bubble's
    // collision behaviour with viruses correctly.
    if let Some(h) = g.body {
        let last = world.last_grown_collider_radius;
        if (g.radius - last).abs() >= 0.5 {
            physics.resize_growing_bubble_collider(h, g.radius);
            world.last_grown_collider_radius = g.radius;
        }
    }

    // Collision-solidify with solid bubbles.
    let g_x = g.x;
    let g_y = g.y;
    let g_r = g.radius;
    let mut solidify_now = false;
    for b in &world.solid_bubbles {
        let dx = g_x - b.x;
        let dy = g_y - b.y;
        let dist = (dx * dx + dy * dy).sqrt();
        let min_dist = g_r + b.radius;
        if dist < min_dist {
            solidify_now = true;
            break;
        }
    }
    if !solidify_now {
        for d in &world.dead_viruses {
            let dx = g_x - d.x;
            let dy = g_y - d.y;
            let dist = (dx * dx + dy * dy).sqrt();
            let min_dist = g_r + d.radius;
            if dist < min_dist {
                solidify_now = true;
                break;
            }
        }
    }
    if solidify_now {
        solidify_bubble(world, physics);
        return;
    }

    // Antidote drain.
    let drain = (ANTIDOTE_DRAIN_RATE / world.total_antidote_time) * dt;
    world.antidote = (world.antidote - drain).max(0.0);
    if world.antidote <= 0.0 {
        if let Some(g) = world.growing.as_mut() {
            g.growing = false;
            g.frozen = true;
        }
    }
}

/// Constrain a desired position to the playfield bounds and away from solid
/// bubbles / dead viruses. Returns `None` if no valid position exists.
/// Mirrors `getConstrainedBubblePosition` in antidote-core.js.
fn constrain_bubble_position(
    target_x: f32,
    target_y: f32,
    radius: f32,
    solid_bubbles: &[Bubble],
    dead_viruses: &[DeadVirus],
) -> Option<(f32, f32)> {
    // Bubble doesn't fit on either axis — no valid position.
    if radius * 2.0 > VIRTUAL_WIDTH || radius * 2.0 > VIRTUAL_HEIGHT {
        return None;
    }
    let x = target_x.clamp(radius, VIRTUAL_WIDTH - radius);
    let y = target_y.clamp(radius, VIRTUAL_HEIGHT - radius);
    for b in solid_bubbles {
        let dx = x - b.x;
        let dy = y - b.y;
        if (dx * dx + dy * dy).sqrt() < radius + b.radius {
            return None;
        }
    }
    for d in dead_viruses {
        let dx = x - d.x;
        let dy = y - d.y;
        if (dx * dx + dy * dy).sqrt() < radius + d.radius {
            return None;
        }
    }
    Some((x, y))
}

/// Convert the current growing bubble into a solid bubble. Mirrors
/// `solidifyBubble` in the JS reference: instant clicks are bumped up to
/// `MIN_VALID_RADIUS`, not discarded, and pay the minimum antidote cost.
fn solidify_bubble(world: &mut World, physics: &mut PhysicsWorld) {
    let Some(g) = world.growing.take() else {
        return;
    };

    // Destroy the growing-bubble physics body — we'll create a fresh solid one.
    if let Some(h) = g.body {
        physics.destroy_body(h);
    }

    if g.radius < MIN_VALID_RADIUS {
        world.antidote = (world.antidote - min_antidote_cost()).max(0.0);
    }

    let mut bubble = Bubble {
        x: g.x,
        y: g.y,
        radius: g.radius.max(MIN_VALID_RADIUS),
        vx: 0.0,
        vy: -BUBBLE_FLOAT_SPEED,
        body: None,
    };
    physics.spawn_bubble_body(&mut bubble);
    world.solid_bubbles.push(bubble);
    world.slide_out_charged = false;
}

/// Per-frame trap-timer update. If a virus stays within `VIRUS_TRAP_DISTANCE`
/// of its `last_unstuck` reference for `VIRUS_TRAP_TIME` seconds, transition
/// it into the dying-virus list (with its physics body destroyed).
fn update_trap_timers(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    let mut to_remove: Vec<usize> = Vec::new();
    let total_alive = world.viruses.len();
    for (i, v) in world.viruses.iter_mut().enumerate() {
        let dx = v.x - v.last_unstuck_x;
        let dy = v.y - v.last_unstuck_y;
        let moved = (dx * dx + dy * dy).sqrt();
        if moved >= VIRUS_TRAP_DISTANCE {
            v.last_unstuck_x = v.x;
            v.last_unstuck_y = v.y;
            v.stuck_time = 0.0;
        } else {
            v.stuck_time += dt;
            if v.stuck_time >= VIRUS_TRAP_TIME {
                to_remove.push(i);
            }
        }
    }
    for &i in to_remove.iter().rev() {
        let v = world.viruses[i];
        if let Some(h) = v.body {
            physics.destroy_body(h);
        }
        world.dying_viruses.push(DyingVirus {
            x: v.x,
            y: v.y,
            radius: VIRUS_RADIUS,
            phase: v.phase,
            death_progress: 0.0,
            is_last_virus: total_alive == 1 && world.dying_viruses.is_empty(),
        });
        world.viruses.swap_remove(i);
        world.total_score += 100;
    }
}

/// Advance dying-virus death_progress. Completed ones become dead viruses
/// (with sink-prone physics body) plus a pop animation.
fn advance_dying_viruses(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    let mut to_remove: Vec<usize> = Vec::new();
    for (i, dv) in world.dying_viruses.iter_mut().enumerate() {
        dv.death_progress += dt / DYING_VIRUS_DURATION;
        if dv.death_progress >= 1.0 {
            to_remove.push(i);
            let mut dead = DeadVirus {
                x: dv.x,
                y: dv.y,
                radius: VIRUS_RADIUS + 2.0,
                vy: DEAD_VIRUS_SINK_SPEED,
                body: None,
            };
            physics.spawn_dead_virus_body(&mut dead);
            world.dead_viruses.push(dead);
            world.pop_animations.push(PopAnimation {
                x: dv.x,
                y: dv.y,
                radius: dv.radius,
                progress: 0.0,
            });
        }
    }
    for &i in to_remove.iter().rev() {
        world.dying_viruses.swap_remove(i);
    }
}

/// Advance pop_animations; remove when progress >= 1.
fn advance_pop_animations(world: &mut World, dt: f32) {
    world.pop_animations.retain_mut(|p| {
        p.progress += dt / POP_ANIMATION_DURATION;
        p.progress < 1.0
    });
}

/// Detect and react to a moving virus touching the growing bubble. Mirrors the
/// virus<->growing-bubble check at the top of `updateViruses` in antidote.js:
/// pop the bubble, lose a life.
fn check_virus_growing_bubble_collision(world: &mut World, physics: &mut PhysicsWorld) {
    let Some(g) = world.growing.as_ref() else {
        return;
    };
    // Bubble sliding out of overlap is immune.
    if !g.initial_overlaps.is_empty() {
        return;
    }
    let g_x = g.x;
    let g_y = g.y;
    let g_r = g.radius;
    let hit = world.viruses.iter().any(|v| {
        let dx = v.x - g_x;
        let dy = v.y - g_y;
        (dx * dx + dy * dy).sqrt() <= VIRUS_RADIUS + g_r
    });
    if hit {
        // Pop the bubble (no solidify) and lose a life.
        let mut death_xy = (g_x, g_y);
        if let Some(g) = world.growing.take() {
            if let Some(h) = g.body {
                physics.destroy_body(h);
            }
            death_xy = (g.x, g.y);
            world.pop_animations.push(PopAnimation {
                x: g.x,
                y: g.y,
                radius: g.radius.max(1.0),
                progress: 0.0,
            });
        }
        if world.lives > 0 {
            world.lives -= 1;
        }
        world.phase = Phase::LifeLost;
        world.phase_elapsed = 0.0;
        world.last_life_lost_at = Some(death_xy);
    }
}

fn advance_life_lost(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    world.phase_elapsed += dt;
    if world.phase_elapsed < LIFE_LOST_DURATION {
        return;
    }

    world.phase_elapsed = 0.0;
    if world.lives == 0 {
        world.phase = Phase::GameOver;
    } else {
        init_level(world, physics);
    }
}

/// Promote to LevelComplete once all viruses (alive and dying) are gone.
fn check_level_complete(world: &mut World) {
    if world.viruses.is_empty() && world.dying_viruses.is_empty() {
        world.phase = Phase::LevelComplete;
    }
}

/// Public entry from menus: advance to next level.
pub fn advance_to_next_level(world: &mut World, physics: &mut PhysicsWorld) {
    world.level = world.level.saturating_add(1);
    init_level(world, physics);
}

/// Public entry: start a new game from level 1.
pub fn start_new_game(world: &mut World, physics: &mut PhysicsWorld) {
    *world = World::new();
    init_level(world, physics);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::level::init_level as level_init;

    #[test]
    fn solidify_instant_click_creates_minimum_bubble() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        on_pointer_down(&mut world, &mut physics, 100.0, 100.0);
        // Phase isn't Playing yet — nothing should happen.
        assert!(world.growing.is_none());
        world.phase = Phase::Playing;
        on_pointer_down(&mut world, &mut physics, 100.0, 100.0);
        // Growing bubble exists but radius=0; the JS reference bumps it up to
        // the minimum valid radius and charges the minimum antidote cost.
        assert!(world.growing.is_some());
        on_pointer_up(&mut world, &mut physics);
        assert!(world.growing.is_none());
        assert_eq!(world.solid_bubbles.len(), 1);
        assert_eq!(world.solid_bubbles[0].radius, MIN_VALID_RADIUS);
        assert!(world.pop_animations.is_empty());
        assert_eq!(world.antidote, 1.0 - min_antidote_cost());
    }

    #[test]
    fn solidify_creates_bubble_after_growth() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        // Real level so check_level_complete doesn't fire on tick 1.
        level_init(&mut world, &mut physics);
        // Push viruses out of the way of the bubble we're about to grow.
        // The world struct fields alone aren't enough — sync_to_world rewrites
        // them from the physics body every tick, so we have to move the body
        // too AND zero its velocity so it doesn't drift back into the bubble.
        for v in world.viruses.iter_mut() {
            v.x = 50.0;
            v.y = 50.0;
            v.vx = 0.0;
            v.vy = 0.0;
            v.last_unstuck_x = 50.0;
            v.last_unstuck_y = 50.0;
            if let Some(handle) = v.body {
                physics.set_body_position(handle, 50.0, 50.0);
                physics.zero_body_velocity(handle);
            }
        }
        on_pointer_down(&mut world, &mut physics, 600.0, 400.0);
        // Grow for 0.25 s — radius becomes 20 px (BUBBLE_GROW_RATE * 0.25).
        for _ in 0..15 {
            tick(&mut world, &mut physics, 1.0 / 60.0);
        }
        // Ensure we're still mid-grow (bubble didn't pop).
        assert!(world.growing.is_some(), "growing bubble was lost mid-grow");
        on_pointer_up(&mut world, &mut physics);
        assert!(world.growing.is_none());
        assert_eq!(world.solid_bubbles.len(), 1);
        assert!(world.solid_bubbles[0].radius >= MIN_VALID_RADIUS);
    }

    #[test]
    fn level_complete_when_all_viruses_die() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        world.level = 1;
        level_init(&mut world, &mut physics);
        assert_eq!(world.viruses.len(), 1);
        // Manually transition the only virus into dying then dead.
        world.dying_viruses.push(DyingVirus {
            x: 100.0,
            y: 100.0,
            radius: VIRUS_RADIUS,
            phase: 0.0,
            death_progress: 0.99,
            is_last_virus: true,
        });
        let v = world.viruses.pop().unwrap();
        if let Some(h) = v.body {
            physics.destroy_body(h);
        }
        // Tick once to advance death_progress past 1.
        tick(&mut world, &mut physics, DYING_VIRUS_DURATION);
        assert_eq!(world.phase, Phase::LevelComplete);
    }

    #[test]
    fn pop_animations_advance_while_not_playing() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        world.phase = Phase::LifeLost;
        world.pop_animations.push(PopAnimation {
            x: 100.0,
            y: 100.0,
            radius: 12.0,
            progress: 0.0,
        });

        tick(&mut world, &mut physics, POP_ANIMATION_DURATION * 0.5);
        assert_eq!(world.pop_animations.len(), 1);
        assert!(world.pop_animations[0].progress > 0.49);

        tick(&mut world, &mut physics, POP_ANIMATION_DURATION * 0.6);
        assert!(world.pop_animations.is_empty());
        assert_eq!(world.phase, Phase::LifeLost);
    }

    #[test]
    fn life_lost_restarts_level_after_transition() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        world.phase = Phase::LifeLost;
        world.lives = 2;
        world.phase_elapsed = LIFE_LOST_DURATION - 0.01;
        world.solid_bubbles.push(Bubble {
            x: 200.0,
            y: 200.0,
            radius: 20.0,
            vx: 0.0,
            vy: 0.0,
            body: None,
        });

        tick(&mut world, &mut physics, 0.02);

        assert_eq!(world.phase, Phase::Playing);
        assert_eq!(world.lives, 2);
        assert_eq!(world.level, 1);
        assert!(world.solid_bubbles.is_empty());
        assert_eq!(world.phase_elapsed, 0.0);
    }

    #[test]
    fn final_life_lost_transitions_to_game_over_after_delay() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        world.phase = Phase::LifeLost;
        world.lives = 0;

        tick(&mut world, &mut physics, LIFE_LOST_DURATION);

        assert_eq!(world.phase, Phase::GameOver);
        assert_eq!(world.phase_elapsed, 0.0);
    }

    /// The growing bubble must freeze the instant it spans the playfield's
    /// shorter axis (2*radius == VIRTUAL_HEIGHT).
    /// Growing past that looks broken — the bubble is bigger than the box
    /// containing it. Also a regression guard for the old `f32::clamp(min,
    /// max)` panic when `min > max`: with the cap in place that branch is
    /// never reached.
    #[test]
    fn growing_bubble_freezes_at_playfield_height() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        level_init(&mut world, &mut physics);
        // Keep every virus in the far corner, but hop between two spots more
        // than VIRUS_TRAP_DISTANCE apart: a virus that sits still for
        // VIRUS_TRAP_TIME is trap-killed by the anti-stuck rule, the level
        // completes, and growth freezes below the cap this test is about.
        let park_viruses = |world: &mut World, physics: &mut PhysicsWorld, hop: bool| {
            let x = if hop { 130.0 } else { 60.0 };
            for v in world.viruses.iter_mut() {
                v.x = x;
                v.y = 60.0;
                v.vx = 0.0;
                v.vy = 0.0;
                if let Some(handle) = v.body {
                    physics.set_body_position(handle, x, 60.0);
                    physics.zero_body_velocity(handle);
                }
            }
        };
        park_viruses(&mut world, &mut physics, false);
        let cap = 0.5 * VIRTUAL_WIDTH.min(VIRTUAL_HEIGHT);
        // Make the antidote budget effectively unlimited so the bubble hits
        // the geometric cap (the subject of this test) instead of freezing
        // early when the meter empties — on the redesigned 1016×696 field
        // the level-1 budget runs out below the cap radius.
        world.total_antidote_time = 1e6;
        on_pointer_down(&mut world, &mut physics, 400.0, 300.0);

        // 20 simulated seconds is far more than the ~4.4 s the bubble needs
        // at 80 px/s to reach the 348-radius cap.
        for i in 0..1200 {
            park_viruses(&mut world, &mut physics, (i / 30) % 2 == 0);
            tick(&mut world, &mut physics, 1.0 / 60.0);
            if let Some(g) = world.growing.as_ref() {
                assert!(
                    g.radius <= cap + 1e-3,
                    "radius {} exceeded cap {}",
                    g.radius,
                    cap
                );
            }
        }

        let g = world.growing.as_ref().expect("bubble should still exist");
        assert_eq!(g.radius, cap, "bubble must sit exactly at the cap");
        assert!(!g.growing, "bubble must be frozen at the cap");
        assert!(g.frozen, "bubble's `frozen` flag must be set at the cap");
        assert_ne!(world.phase, Phase::GameOver);
    }
}
