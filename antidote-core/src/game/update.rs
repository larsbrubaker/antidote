//! Per-frame tick + pointer-input handlers. Mirrors the gameplay surface of
//! `reference/GFG/public/games/antidote/antidote.js`.

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

/// One simulation tick. Called from `GameWidget::paint` once per frame
/// (or from a dedicated game-tick path if/when one is split out).
pub fn tick(world: &mut World, physics: &mut PhysicsWorld, dt: f32) {
    if world.phase != Phase::Playing {
        return;
    }

    grow_bubble(world, physics, dt);

    physics.apply_bubble_float(world, BUBBLE_FLOAT_SPEED * 2.0);
    physics.apply_dead_virus_gravity(world, DEAD_VIRUS_SINK_SPEED);

    physics.step(dt);
    physics.sync_to_world(world);

    let target_speed = virus_speed_for_level(world.level);
    physics.maintain_virus_speeds(world, target_speed);

    check_virus_growing_bubble_collision(world, physics);
    if world.phase != Phase::Playing {
        return;
    }

    update_trap_timers(world, physics, dt);
    advance_dying_viruses(world, physics, dt);
    advance_pop_animations(world, dt);
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

    // Normal growth.
    g.radius += BUBBLE_GROW_RATE * dt;

    // Pin to walls (don't freeze).
    g.x = g.x.clamp(g.radius, VIRTUAL_WIDTH - g.radius);
    g.y = g.y.clamp(g.radius, VIRTUAL_HEIGHT - g.radius);

    // Resize the rapier collider to match.
    if let Some(h) = g.body {
        physics.resize_growing_bubble_collider(h, g.radius);
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

/// Convert the current growing bubble into a solid bubble (or pop it if too
/// small). Mirrors `solidifyBubble` + the parts of the JS reference that
/// promote the new bubble to a physics-managed body. Always pushes either a
/// new solid bubble or a pop animation.
fn solidify_bubble(world: &mut World, physics: &mut PhysicsWorld) {
    let Some(g) = world.growing.take() else {
        return;
    };

    // Destroy the growing-bubble physics body — we'll create a fresh solid one.
    if let Some(h) = g.body {
        physics.destroy_body(h);
    }

    if g.radius < MIN_VALID_RADIUS {
        // Pop animation only — too small to be a real bubble.
        world.pop_animations.push(PopAnimation {
            x: g.x,
            y: g.y,
            radius: g.radius.max(1.0),
            progress: 0.0,
        });
        world.slide_out_charged = false;
        return;
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
/// (with sink-prone rapier body) plus a pop animation.
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
        if let Some(g) = world.growing.take() {
            if let Some(h) = g.body {
                physics.destroy_body(h);
            }
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
        if world.lives == 0 {
            world.phase = Phase::GameOver;
        } else {
            world.phase = Phase::LifeLost;
        }
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
    fn solidify_pop_for_too_small_bubble() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        on_pointer_down(&mut world, &mut physics, 100.0, 100.0);
        // Phase isn't Playing yet — nothing should happen.
        assert!(world.growing.is_none());
        world.phase = Phase::Playing;
        on_pointer_down(&mut world, &mut physics, 100.0, 100.0);
        // Growing bubble exists but radius=0; release immediately should pop.
        assert!(world.growing.is_some());
        on_pointer_up(&mut world, &mut physics);
        assert!(world.growing.is_none());
        assert!(world.solid_bubbles.is_empty());
        assert_eq!(world.pop_animations.len(), 1);
    }

    #[test]
    fn solidify_creates_bubble_after_growth() {
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        // Real level so check_level_complete doesn't fire on tick 1.
        level_init(&mut world, &mut physics);
        // Push viruses out of the way of the bubble we're about to grow.
        for v in world.viruses.iter_mut() {
            v.x = 50.0;
            v.y = 50.0;
            v.last_unstuck_x = 50.0;
            v.last_unstuck_y = 50.0;
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
}
