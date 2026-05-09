//! Scene painter. Goal: pixel-faithful reproduction of the JS Canvas rendering.
//!
//! The exact draw order, gradients, alpha values, line widths, eye positions,
//! spike orbits, and pop-animation easing all match
//! `reference/GFG/public/games/antidote/antidote-rendering.js` so the visual
//! feel of the original is preserved.
//!
//! M2 fills in each helper. The function signatures here are the contract.
//!
//! Coordinate note: agg-gui is Y-up. The JS reference is Y-down. The flip
//! happens **once** at the GameWidget boundary — every helper below works in
//! JS-style Y-down coordinates.

use crate::consts::VIRTUAL_HEIGHT;
use crate::game::state::{Bubble, DeadVirus, DyingVirus, GrowingBubble, PopAnimation, Virus, World};

#[inline]
pub fn flip_y(y: f32) -> f32 {
    VIRTUAL_HEIGHT - y
}

/// Top-level scene paint. Mirrors `render(ctx, state)` in the JS reference:
/// background → grid → border → solid bubbles → dead viruses → dying viruses →
/// growing bubble → viruses → pop animations.
pub fn paint_scene(_world: &World, _time_seconds: f32) {
    // M2: drives agg_gui::GfxCtx via the GameWidget's paint method.
    // Order:
    //   paint_background_and_grid(ctx)
    //   paint_border(ctx)
    //   for b in solid_bubbles { paint_bubble(ctx, b.x, b.y, b.radius, false) }
    //   for d in dead_viruses { paint_dead_virus(ctx, d.x, d.y, d.radius) }
    //   for d in dying_viruses { paint_dying_virus(ctx, d, time) }
    //   if let Some(g) = growing { paint_bubble(ctx, g.x, g.y, g.radius, true) }
    //   for v in viruses { paint_virus(ctx, v, time) }
    //   for p in pop_animations { paint_pop_animation(ctx, p) }
}

// Each function below is a 1:1 port of the corresponding JS function in
// antidote-rendering.js. M2 implements them.

pub fn paint_background_and_grid() {
    // fill #050508; 30px grid lines at rgba(0,255,242,0.05); lineWidth 1.
}

pub fn paint_border() {
    // strokeRect(1.5, 1.5, w-3, h-3) at rgba(0,255,242,0.6) lineWidth 3.
}

pub fn paint_bubble(_b: &Bubble, _is_growing: bool) {
    // Radial gradient (cx,cy → cx,cy r=radius):
    //   growing: 0 → 'rgba(0,255,242,0.4)', 0.7 → '...0.2)', 1 → '...0.1)'
    //   solid:   0 → 'rgba(0,200,220,0.6)', 0.7 → 'rgba(0,150,180,0.4)', 1 → 'rgba(0,100,140,0.2)'
    // Stroke: growing rgba(0,255,242,0.8) lineWidth 2 / solid rgba(0,200,220,0.5) lineWidth 1.
    // Highlight: arc at (x-r*0.3, y-r*0.3) radius r*0.2 fill rgba(255,255,255,0.3).
}

pub fn paint_growing_bubble(_g: &GrowingBubble) {
    // Same as paint_bubble with isGrowing=true.
}

pub fn paint_virus(_v: &Virus, _time_seconds: f32) {
    // wobble = sin(time*5 + phase) * 2
    // Body: radial gradient #ff4d6d → #c9184a → #800f2f at radius VIRUS_RADIUS+wobble.
    // 8 spikes: orbit angle = (i/8)*tau + time*2; spike center at (radius+4+wobble) outward; spike radius 4 fill #ff758f.
    // Eyes: white circles at (x-4,y-2) (x+4,y-2) radius 3; black pupils radius 1.5.
}

pub fn paint_dying_virus(_dv: &DyingVirus, _time_seconds: f32) {
    // Lerp gradient colors alive→dead by death_progress.
    // Wobble amount = 2 * (1-progress); spike orbit slows by (1-progress); spikes fade and shrink.
    // Eyes cross-fade: alive eyes alpha = 1-progress; X eyes alpha = progress.
}

pub fn paint_dead_virus(_d: &DeadVirus) {
    // Gray radial gradient 'rgba(80,80,100,0.8)' → 'rgba(40,40,60,0.6)'.
    // Stroke 'rgba(100,100,120,0.5)' lineWidth 1.
    // X eyes: stroke #666 lineWidth 2 — diagonal lines (-5,-4)→(-2,-1), (-2,-4)→(-5,-1), (+2,-4)→(+5,-1), (+5,-4)→(+2,-1).
}

pub fn paint_pop_animation(_p: &PopAnimation) {
    // eased = 1 - (1-progress)^2.
    // Outer ring: radius = r + eased*r*0.8, alpha = (1-eased)*0.8, lineWidth = max(1, (1-eased)*4), color rgba(0,255,242,..).
    // Inner ring (progress > 0.1): inner = (progress-0.1)/0.9, easedI = 1-(1-inner)^2. radius = r*0.7 + easedI*r*0.6.
    //   alpha = (1-easedI)*0.6, lineWidth = max(1, (1-easedI)*3), color rgba(0,200,220,..).
    // Particles: n = min(12, floor(r/5)+4); each at angle (i/n)*tau, distance r*0.5 + eased*r*1.2; size max(1, (1-eased)*3); fill rgba(0,255,242, (1-eased)*0.7).
}
