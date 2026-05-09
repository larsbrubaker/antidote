//! World state. Plain structs; no ECS. See `reference/GFG/public/games/antidote/antidote.js` for the JS originals.

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
    pub last_unstuck_x: f32,
    pub last_unstuck_y: f32,
    pub stuck_time: f32,
    pub dead: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct Bubble {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub vy: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct GrowingBubble {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
}

pub struct World {
    pub phase: Phase,
    pub level: u32,
    pub lives: u8,
    pub antidote: f32,
    pub total_score: u64,
    pub viruses: Vec<Virus>,
    pub bubbles: Vec<Bubble>,
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
            bubbles: Vec::new(),
            growing: None,
        }
    }
}

impl Default for World {
    fn default() -> Self {
        Self::new()
    }
}
