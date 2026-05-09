//! Game-design constants ported verbatim from
//! `reference/GFG/public/games/antidote/antidote-core.js` and `antidote-physics.js`.
//! All values match the JS reference exactly.

pub const VIRTUAL_WIDTH: f32 = 800.0;
pub const VIRTUAL_HEIGHT: f32 = 600.0;

// Virus
pub const VIRUS_RADIUS: f32 = 12.0;
pub const VIRUS_BASE_SPEED: f32 = 100.0;
pub const VIRUS_TRAP_DISTANCE: f32 = 30.0;
pub const VIRUS_TRAP_TIME: f32 = 3.0;

// Bubble
pub const BUBBLE_GROW_RATE: f32 = 80.0;
pub const BUBBLE_FLOAT_SPEED: f32 = 60.0;
pub const BUBBLE_PUSH_FORCE: f32 = 25.0;
pub const BUBBLE_FRICTION: f32 = 0.95;
pub const MIN_VALID_RADIUS: f32 = 11.0;
pub const MAX_BUBBLE_COUNT: usize = 50;
pub const SLIDE_OUT_SPEED: f32 = 200.0;

// Dead virus
pub const DEAD_VIRUS_SINK_SPEED: f32 = 30.0;

// Lives & antidote
pub const BASE_LIVES: u8 = 3;
pub const BASE_ANTIDOTE_TIME: f32 = 7.5;
pub const ANTIDOTE_DRAIN_RATE: f32 = 0.5;

// Wall-slide correction (used by `correct_wall_slide_velocity`).
pub const WALL_PROXIMITY_THRESHOLD: f32 = 20.0;
pub const MIN_PERPENDICULAR_RATIO: f32 = 0.3;

// Physics scaling — Box2D/Rapier work best with object sizes in 0.1..10 m.
pub const PIXELS_PER_METER: f32 = 30.0;

#[inline]
pub fn to_meters(pixels: f32) -> f32 {
    pixels / PIXELS_PER_METER
}

#[inline]
pub fn to_pixels(meters: f32) -> f32 {
    meters * PIXELS_PER_METER
}

#[inline]
pub fn min_antidote_cost() -> f32 {
    1.0 / MAX_BUBBLE_COUNT as f32
}
