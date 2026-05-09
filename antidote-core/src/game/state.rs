//! World state. Plain structs; no ECS. Mirrors the JS reference's globals
//! in `reference/GFG/public/games/antidote/antidote.js`.

use crate::consts::{BASE_ANTIDOTE_TIME, BASE_LIVES};

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
}

/// A dead virus that sinks downward.
#[derive(Debug, Clone, Copy)]
pub struct DeadVirus {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub vy: f32,
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
    pub antidote: f32,
    pub total_score: u64,
    pub viruses: Vec<Virus>,
    pub solid_bubbles: Vec<Bubble>,
    pub dead_viruses: Vec<DeadVirus>,
    pub dying_viruses: Vec<DyingVirus>,
    pub pop_animations: Vec<PopAnimation>,
    pub growing: Option<GrowingBubble>,
}

impl World {
    pub fn new() -> Self {
        Self {
            phase: Phase::Start,
            level: 1,
            lives: BASE_LIVES,
            antidote: BASE_ANTIDOTE_TIME,
            total_score: 0,
            viruses: Vec::new(),
            solid_bubbles: Vec::new(),
            dead_viruses: Vec::new(),
            dying_viruses: Vec::new(),
            pop_animations: Vec::new(),
            growing: None,
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
