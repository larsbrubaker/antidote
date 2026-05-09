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
    world.viruses.clear();
    world.solid_bubbles.clear();
    world.dead_viruses.clear();
    world.dying_viruses.clear();
    world.pop_animations.clear();
    world.growing = None;
    world.pointer_down = false;
    world.slide_out_charged = false;

    world.total_antidote_time = total_antidote_time_for(world.level);
    world.antidote = 1.0;
    world.phase = Phase::Playing;

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
}
