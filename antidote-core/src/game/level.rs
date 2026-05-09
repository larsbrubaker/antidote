use rand::Rng;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH, VIRUS_BASE_SPEED, VIRUS_RADIUS};
use crate::game::physics::PhysicsWorld;
use crate::game::state::{total_antidote_time_for, Phase, Virus, World};

pub fn virus_count_for_level(level: u32) -> u32 {
    1 + level / 2
}

pub fn virus_speed_for_level(level: u32) -> f32 {
    VIRUS_BASE_SPEED + (level as f32 - 1.0) * 10.0
}

/// Initialize the world and physics for the start of a level. Mirrors
/// `initLevel()` in the JS reference: clear all entities, set antidote
/// budget, spawn viruses.
pub fn init_level(world: &mut World, physics: &mut PhysicsWorld) {
    // Destroy every rapier body the world references BEFORE wiping the Vecs;
    // otherwise the previous level's bubbles, dead viruses, and any
    // in-progress growing bubble would persist as invisible colliders that
    // newly-spawned viruses can pin themselves against (manifests as "virus
    // stuck dead-center on level start").
    for v in &world.viruses {
        if let Some(h) = v.body {
            physics.destroy_body(h);
        }
    }
    for b in &world.solid_bubbles {
        if let Some(h) = b.body {
            physics.destroy_body(h);
        }
    }
    for d in &world.dead_viruses {
        if let Some(h) = d.body {
            physics.destroy_body(h);
        }
    }
    if let Some(g) = world.growing.as_ref() {
        if let Some(h) = g.body {
            physics.destroy_body(h);
        }
    }

    world.viruses.clear();
    world.solid_bubbles.clear();
    world.dead_viruses.clear();
    world.dying_viruses.clear();
    world.pop_animations.clear();
    world.growing = None;
    world.pointer_down = false;
    world.phase_elapsed = 0.0;
    world.slide_out_charged = false;

    world.total_antidote_time = total_antidote_time_for(world.level);
    world.antidote = 1.0;
    world.phase = Phase::Playing;
    world.level_start_score = world.total_score;
    world.last_life_lost_at = None;

    spawn_viruses(world, physics, virus_count_for_level(world.level));
}

/// Spawn `count` viruses into random positions in the central 60% of the
/// playfield, each with a random velocity direction at the level's target
/// speed.
pub fn spawn_viruses(world: &mut World, physics: &mut PhysicsWorld, count: u32) {
    let speed = virus_speed_for_level(world.level);
    let mut rng = rand::thread_rng();

    for _ in 0..count {
        let angle: f32 = rng.gen_range(0.0..std::f32::consts::TAU);
        let start_x = VIRTUAL_WIDTH * 0.2 + rng.gen_range(0.0..1.0_f32) * VIRTUAL_WIDTH * 0.6;
        let start_y = VIRTUAL_HEIGHT * 0.2 + rng.gen_range(0.0..1.0_f32) * VIRTUAL_HEIGHT * 0.6;
        let phase = rng.gen_range(0.0..std::f32::consts::TAU);
        let mut virus = Virus {
            x: start_x,
            y: start_y,
            vx: angle.cos() * speed,
            vy: angle.sin() * speed,
            phase,
            last_unstuck_x: start_x,
            last_unstuck_y: start_y,
            stuck_time: 0.0,
            speed,
            body: None,
        };
        physics.spawn_virus_body(&mut virus, VIRUS_RADIUS);
        world.viruses.push(virus);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_1_has_one_virus() {
        assert_eq!(virus_count_for_level(1), 1);
    }

    #[test]
    fn level_4_has_three_viruses() {
        assert_eq!(virus_count_for_level(4), 3);
    }

    #[test]
    fn speed_grows_linearly() {
        assert_eq!(virus_speed_for_level(1), VIRUS_BASE_SPEED);
        assert_eq!(virus_speed_for_level(2), VIRUS_BASE_SPEED + 10.0);
    }

    #[test]
    fn init_level_spawns_correct_count_with_bodies() {
        let mut world = World::new();
        world.level = 4;
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);
        init_level(&mut world, &mut physics);
        assert_eq!(world.viruses.len(), 3);
        for v in &world.viruses {
            assert!(v.body.is_some());
            assert!(v.x >= VIRTUAL_WIDTH * 0.2 && v.x <= VIRTUAL_WIDTH * 0.8);
            assert!(v.y >= VIRTUAL_HEIGHT * 0.2 && v.y <= VIRTUAL_HEIGHT * 0.8);
        }
    }

    /// Re-initializing a level must not leak rapier bodies from the previous
    /// level. Without the destroy-bodies pass at the top of `init_level`,
    /// each level transition would accumulate ghost colliders that pin newly
    /// spawned viruses ("virus stuck dead-center on level start").
    #[test]
    fn init_level_does_not_leak_physics_bodies() {
        use crate::game::state::{Bubble, DeadVirus};
        let mut world = World::new();
        let mut physics = PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT);

        world.level = 5;
        init_level(&mut world, &mut physics);
        // Walls (4) + 3 viruses for level 5.
        let baseline = physics.bodies.len();
        assert_eq!(world.viruses.len(), 3);
        assert_eq!(baseline, 7);

        // Stage a bubble and a dead virus, both with rapier bodies.
        let mut bubble = Bubble {
            x: 200.0,
            y: 200.0,
            radius: 20.0,
            vx: 0.0,
            vy: 0.0,
            body: None,
        };
        physics.spawn_bubble_body(&mut bubble);
        world.solid_bubbles.push(bubble);

        let mut dead = DeadVirus {
            x: 300.0,
            y: 300.0,
            radius: 14.0,
            vy: 0.0,
            body: None,
        };
        physics.spawn_dead_virus_body(&mut dead);
        world.dead_viruses.push(dead);

        assert_eq!(physics.bodies.len(), baseline + 2);

        // Advance to the next level. New viruses spawn fresh; bubble +
        // dead virus from previous level must be destroyed in physics.
        world.level += 1;
        init_level(&mut world, &mut physics);

        let virus_count = virus_count_for_level(world.level) as usize;
        // Walls (4) + viruses; nothing else lingers.
        assert_eq!(physics.bodies.len(), 4 + virus_count);
    }
}
