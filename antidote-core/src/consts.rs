//! Game-design constants ported from `reference/GFG/public/games/antidote/antidote.js`.

pub const VIRTUAL_WIDTH: f32 = 800.0;
pub const VIRTUAL_HEIGHT: f32 = 600.0;

pub const VIRUS_RADIUS: f32 = 12.0;
pub const VIRUS_BASE_SPEED: f32 = 100.0;
pub const VIRUS_TRAP_DISTANCE: f32 = 30.0;
pub const VIRUS_TRAP_TIME: f32 = 3.0;

pub const BUBBLE_GROW_RATE: f32 = 80.0;
pub const BUBBLE_FLOAT_SPEED: f32 = 60.0;
pub const MIN_VALID_RADIUS: f32 = 11.0;
pub const MAX_BUBBLE_COUNT: usize = 50;

pub const BASE_LIVES: u8 = 3;
pub const BASE_ANTIDOTE_TIME: f32 = 7.5;
