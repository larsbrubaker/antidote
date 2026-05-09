//! World state. Plain structs; no ECS. Mirrors the JS reference's globals
//! in `gfg/public/games/antidote/antidote.js`.

use rapier2d::prelude::RigidBodyHandle;

use crate::consts::{BASE_ANTIDOTE_TIME, BASE_LIVES};

/// Game-level constants: per-level antidote time = `BASE_ANTIDOTE_TIME + level * 2`.
/// Stored on `World` because pause/resume needs it preserved.
pub fn total_antidote_time_for(level: u32) -> f32 {
    BASE_ANTIDOTE_TIME + level as f32 * 2.0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Start,
    Playing,
    Paused,
    LevelComplete,
    LifeLost,
    GameOver,
}

#[derive(Debug, Clone, Copy)]
pub struct Virus {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    /// Animation phase offset for spike orbit + radius wobble (radians).
    pub phase: f32,
    /// Position when `stuck_time` was last reset; if the virus stays within
    /// `VIRUS_TRAP_DISTANCE` of this for `VIRUS_TRAP_TIME` seconds it dies.
    pub last_unstuck_x: f32,
    pub last_unstuck_y: f32,
    pub stuck_time: f32,
    pub speed: f32,
    pub body: Option<RigidBodyHandle>,
}

/// A virus in the dying-animation transition (alive → dead).
#[derive(Debug, Clone, Copy)]
pub struct DyingVirus {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub phase: f32,
    pub death_progress: f32,
    pub is_last_virus: bool,
}

/// A finalized solid bubble that floats upward.
#[derive(Debug, Clone, Copy)]
pub struct Bubble {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub vx: f32,
    pub vy: f32,
    pub body: Option<RigidBodyHandle>,
}

/// A dead virus that sinks downward.
#[derive(Debug, Clone, Copy)]
pub struct DeadVirus {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub vy: f32,
    pub body: Option<RigidBodyHandle>,
}

/// The bubble the player is currently growing.
#[derive(Debug, Clone)]
pub struct GrowingBubble {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub growing: bool,
    pub frozen: bool,
    /// Indices into `World::solid_bubbles` / `World::dead_viruses` that this
    /// bubble was started inside of; used to slide out gradually before growing.
    pub initial_overlaps: Vec<InitialOverlap>,
    pub body: Option<RigidBodyHandle>,
}

#[derive(Debug, Clone, Copy)]
pub enum InitialOverlap {
    Bubble(usize),
    DeadVirus(usize),
}

/// Pop ring + particle animation (post-virus-trap) — see `drawPopAnimation`.
#[derive(Debug, Clone, Copy)]
pub struct PopAnimation {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub progress: f32,
}

pub struct World {
    pub phase: Phase,
    pub level: u32,
    pub lives: u8,
    /// Antidote remaining as a fraction of `total_antidote_time` (0.0..1.0).
    pub antidote: f32,
    pub total_antidote_time: f32,
    pub total_score: u64,
    pub viruses: Vec<Virus>,
    pub solid_bubbles: Vec<Bubble>,
    pub dead_viruses: Vec<DeadVirus>,
    pub dying_viruses: Vec<DyingVirus>,
    pub pop_animations: Vec<PopAnimation>,
    pub growing: Option<GrowingBubble>,
    /// Latest pointer position in JS-style logical coords.
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub pointer_down: bool,
    /// Seconds spent in the current non-playing transition phase.
    pub phase_elapsed: f32,
    /// Set true once the slide-out min antidote cost has been charged for the
    /// current growing bubble. Mirrors `slideOutCharged` in the JS reference.
    pub slide_out_charged: bool,
    /// Radius at the time of the last `resize_growing_bubble_collider` call.
    /// Used so we don't tear down + rebuild the rapier collider every frame
    /// for a tiny radius bump.
    pub last_grown_collider_radius: f32,
    /// `total_score` snapshot at the start of the current level. The
    /// LevelComplete overlay subtracts this from `total_score` to show how
    /// many points were earned this level.
    pub level_start_score: u64,
    /// Where the growing bubble was popped when the player last lost a life
    /// (JS-style logical coords). The LifeLost overlay anchors its float-up
    /// animation here.
    pub last_life_lost_at: Option<(f32, f32)>,
}

impl World {
    pub fn new() -> Self {
        Self {
            phase: Phase::Start,
            level: 1,
            lives: BASE_LIVES,
            antidote: 1.0,
            total_antidote_time: BASE_ANTIDOTE_TIME,
            total_score: 0,
            viruses: Vec::new(),
            solid_bubbles: Vec::new(),
            dead_viruses: Vec::new(),
            dying_viruses: Vec::new(),
            pop_animations: Vec::new(),
            growing: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
            pointer_down: false,
            phase_elapsed: 0.0,
            slide_out_charged: false,
            last_grown_collider_radius: 0.0,
            level_start_score: 0,
            last_life_lost_at: None,
        }
    }

    /// Score earned in the level currently in progress (or just completed).
    pub fn current_level_score(&self) -> u64 {
        self.total_score.saturating_sub(self.level_start_score)
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
