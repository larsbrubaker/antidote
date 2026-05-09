//! Scene painter. Pixel-faithful reproduction of the JS Canvas rendering at
//! `gfg/public/games/antidote/antidote-rendering.js`.
//!
//! Coordinate convention inside this module: JS-style Y-down. The widget's
//! `paint` method applies a single transform that maps the (0..VIRTUAL_WIDTH,
//! 0..VIRTUAL_HEIGHT) JS-down logical box into the widget's letterboxed Y-up
//! pixel area, so every helper below works in JS coordinates.

use agg_gui::draw_ctx::RadialGradientPaint;
use agg_gui::{Color, DrawCtx};

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::state::{
    Bubble, DeadVirus, DyingVirus, GrowingBubble, PopAnimation, Virus, World,
};

#[inline]
pub fn flip_y(y: f32) -> f32 {
    VIRTUAL_HEIGHT - y
}

const GAME_W: f64 = VIRTUAL_WIDTH as f64;
const GAME_H: f64 = VIRTUAL_HEIGHT as f64;

/// Top-level scene paint. Mirrors `render(ctx, state)` in the JS reference:
/// background → grid → border → solid bubbles → dead viruses → dying viruses →
/// growing bubble → viruses → pop animations.
pub fn paint_scene(world: &World, _time_seconds: f32) {
    // Routed via `GameWidget::paint` (which has the DrawCtx). M2-D+ implements
    // the per-entity helpers below; this top-level function is currently a
    // placeholder so the module compiles standalone in tests.
    let _ = world;
}

/// `ctx.fillRect(0, 0, w, h)` with #050508, then a 30 px grid stroke at
/// `rgba(0, 255, 242, 0.05)` lineWidth 1.
pub fn paint_background_and_grid(ctx: &mut dyn DrawCtx) {
    // Background
    ctx.set_fill_color(Color::from_rgb8(5, 5, 8));
    ctx.begin_path();
    ctx.rect(0.0, 0.0, GAME_W, GAME_H);
    ctx.fill();

    // Grid lines (cyan, 5% alpha, 1 px)
    ctx.set_stroke_color(Color::from_rgb8(0, 255, 242).with_alpha(0.05));
    ctx.set_line_width(1.0);
    let step = 30.0_f64;

    let mut x = 0.0_f64;
    while x <= GAME_W + 0.0001 {
        ctx.begin_path();
        ctx.move_to(x, 0.0);
        ctx.line_to(x, GAME_H);
        ctx.stroke();
        x += step;
    }
    let mut y = 0.0_f64;
    while y <= GAME_H + 0.0001 {
        ctx.begin_path();
        ctx.move_to(0.0, y);
        ctx.line_to(GAME_W, y);
        ctx.stroke();
        y += step;
    }
}

/// `strokeRect(1.5, 1.5, w-3, h-3)` cyan @ 60%, lineWidth 3.
pub fn paint_border(ctx: &mut dyn DrawCtx) {
    ctx.set_stroke_color(Color::from_rgb8(0, 255, 242).with_alpha(0.6));
    ctx.set_line_width(3.0);
    ctx.begin_path();
    ctx.rect(1.5, 1.5, GAME_W - 3.0, GAME_H - 3.0);
    ctx.stroke();
}

// ---- per-entity helpers ----

pub fn paint_bubble(ctx: &mut dyn DrawCtx, b: &Bubble, is_growing: bool) {
    paint_bubble_at(ctx, b.x as f64, b.y as f64, b.radius as f64, is_growing);
}

pub fn paint_growing_bubble(ctx: &mut dyn DrawCtx, g: &GrowingBubble) {
    paint_bubble_at(ctx, g.x as f64, g.y as f64, g.radius as f64, true);
}

/// Pixel-faithful port of `drawBubble(ctx, x, y, radius, isGrowing)`.
fn paint_bubble_at(ctx: &mut dyn DrawCtx, x: f64, y: f64, radius: f64, is_growing: bool) {
    let (c0, c1, c2, stroke_color, stroke_w) = if is_growing {
        (
            Color::from_rgb8(0, 255, 242).with_alpha(0.4),
            Color::from_rgb8(0, 255, 242).with_alpha(0.2),
            Color::from_rgb8(0, 255, 242).with_alpha(0.1),
            Color::from_rgb8(0, 255, 242).with_alpha(0.8),
            2.0_f64,
        )
    } else {
        (
            Color::from_rgb8(0, 200, 220).with_alpha(0.6),
            Color::from_rgb8(0, 150, 180).with_alpha(0.4),
            Color::from_rgb8(0, 100, 140).with_alpha(0.2),
            Color::from_rgb8(0, 200, 220).with_alpha(0.5),
            1.0_f64,
        )
    };

    // Body — radial gradient fill.
    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x,
        y,
        radius,
        &[(0.0, c0), (0.7, c1), (1.0, c2)],
    ));
    ctx.begin_path();
    ctx.circle(x, y, radius);
    ctx.fill();

    // Outline.
    ctx.set_stroke_color(stroke_color);
    ctx.set_line_width(stroke_w);
    ctx.begin_path();
    ctx.circle(x, y, radius);
    ctx.stroke();

    // Highlight at upper-left (in JS Y-down, "upper" is smaller y).
    ctx.set_fill_color(Color::from_rgb8(255, 255, 255).with_alpha(0.3));
    ctx.begin_path();
    ctx.circle(x - radius * 0.3, y - radius * 0.3, radius * 0.2);
    ctx.fill();
}

/// Pixel-faithful port of `drawVirus(ctx, virus)`.
pub fn paint_virus(ctx: &mut dyn DrawCtx, v: &Virus, time_seconds: f32) {
    let x = v.x as f64;
    let y = v.y as f64;
    let radius = crate::consts::VIRUS_RADIUS as f64;
    let wobble = ((time_seconds * 5.0 + v.phase).sin() * 2.0) as f64;

    // Body — radial gradient #ff4d6d → #c9184a → #800f2f.
    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x,
        y,
        radius,
        &[
            (0.0, Color::from_rgb8(0xff, 0x4d, 0x6d)),
            (0.6, Color::from_rgb8(0xc9, 0x18, 0x4a)),
            (1.0, Color::from_rgb8(0x80, 0x0f, 0x2f)),
        ],
    ));
    ctx.begin_path();
    ctx.circle(x, y, radius + wobble);
    ctx.fill();

    // 8 spikes orbiting at radius+4+wobble; per-spike radius 4 fill #ff758f.
    ctx.set_fill_color(Color::from_rgb8(0xff, 0x75, 0x8f));
    let orbit_r = radius + 4.0 + wobble;
    for i in 0..8 {
        let angle = (i as f64 / 8.0) * std::f64::consts::TAU + (time_seconds as f64) * 2.0;
        let sx = x + angle.cos() * orbit_r;
        let sy = y + angle.sin() * orbit_r;
        ctx.begin_path();
        ctx.circle(sx, sy, 4.0);
        ctx.fill();
    }

    // Eyes — white circles + black pupils.
    paint_eyes_alive(ctx, x, y, 1.0);
}

/// Pixel-faithful port of `drawDeadVirus(ctx, x, y, radius)`.
pub fn paint_dead_virus(ctx: &mut dyn DrawCtx, d: &DeadVirus) {
    let x = d.x as f64;
    let y = d.y as f64;
    let radius = d.radius as f64;

    // Gray radial gradient fill.
    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x,
        y,
        radius,
        &[
            (0.0, Color::from_rgb8(80, 80, 100).with_alpha(0.8)),
            (1.0, Color::from_rgb8(40, 40, 60).with_alpha(0.6)),
        ],
    ));
    ctx.begin_path();
    ctx.circle(x, y, radius);
    ctx.fill();

    // Outline.
    ctx.set_stroke_color(Color::from_rgb8(100, 100, 120).with_alpha(0.5));
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.circle(x, y, radius);
    ctx.stroke();

    // X eyes — #666 lineWidth 2.
    paint_eyes_x(ctx, x, y, 1.0);
}

/// Pixel-faithful port of `drawDyingVirus(ctx, dv)`. Cross-fades virus body,
/// spikes, and eyes from alive (#ff4d6d) to dead (rgba(80,80,100)) over
/// `death_progress ∈ [0,1]`.
pub fn paint_dying_virus(ctx: &mut dyn DrawCtx, dv: &DyingVirus, time_seconds: f32) {
    let x = dv.x as f64;
    let y = dv.y as f64;
    let progress = dv.death_progress.clamp(0.0, 1.0) as f64;
    let wobble_amount = 2.0 * (1.0 - progress);
    let wobble = ((time_seconds * 5.0 + dv.phase).sin() as f64) * wobble_amount;

    // Body gradient — lerp alive→dead color stops.
    let t = progress as f32;
    let center = Color::from_rgb8(255, 77, 109).lerp(Color::from_rgb8(80, 80, 100), t);
    let mid = Color::from_rgb8(201, 24, 74).lerp(Color::from_rgb8(60, 60, 80), t);
    let edge = Color::from_rgb8(128, 15, 47).lerp(Color::from_rgb8(40, 40, 60), t);

    let radius = (crate::consts::VIRUS_RADIUS as f64) * (1.0 - progress * 0.1);

    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x,
        y,
        radius,
        &[(0.0, center), (0.6, mid), (1.0, edge)],
    ));
    ctx.begin_path();
    ctx.circle(x, y, radius + wobble);
    ctx.fill();

    // Spikes — fade and shrink, orbit slows by (1 - progress).
    let spike_color = Color::from_rgb8(255, 117, 143).lerp(Color::from_rgb8(100, 100, 120), t);
    let spike_alpha = (1.0 - progress * 0.5) as f32;
    let spike_size = 4.0 * (1.0 - progress * 0.5);
    let orbit_r = radius + 4.0 + wobble;

    ctx.set_fill_color(spike_color.with_alpha(spike_alpha));
    for i in 0..8 {
        let angle = (i as f64 / 8.0) * std::f64::consts::TAU
            + (time_seconds as f64) * 2.0 * (1.0 - progress);
        let sx = x + angle.cos() * orbit_r;
        let sy = y + angle.sin() * orbit_r;
        ctx.begin_path();
        ctx.circle(sx, sy, spike_size);
        ctx.fill();
    }

    // Eyes — alive fades out, X fades in.
    let alive_alpha = 1.0 - progress as f32;
    let x_alpha = progress as f32;
    paint_eyes_alive(ctx, x, y, alive_alpha);
    paint_eyes_x(ctx, x, y, x_alpha);
}

/// Pixel-faithful port of `drawPopAnimation(ctx, pop)`. Outer ring + inner
/// ring (delayed) + N particle fragments radiating outward.
pub fn paint_pop_animation(ctx: &mut dyn DrawCtx, p: &PopAnimation) {
    let x = p.x as f64;
    let y = p.y as f64;
    let radius = p.radius as f64;
    let progress = p.progress.clamp(0.0, 1.0) as f64;
    let eased = 1.0 - (1.0 - progress).powi(2);

    // Outer ring.
    let ring_radius = radius + eased * radius * 0.8;
    let ring_alpha = 1.0 - eased;
    let ring_width = ((1.0 - eased) * 4.0).max(1.0);

    ctx.set_stroke_color(Color::from_rgb8(0, 255, 242).with_alpha((ring_alpha * 0.8) as f32));
    ctx.set_line_width(ring_width);
    ctx.begin_path();
    ctx.circle(x, y, ring_radius);
    ctx.stroke();

    // Second (inner) ring, delayed until progress > 0.1.
    if progress > 0.1 {
        let inner_progress = (progress - 0.1) / 0.9;
        let inner_eased = 1.0 - (1.0 - inner_progress).powi(2);
        let inner_radius = radius * 0.7 + inner_eased * radius * 0.6;
        let inner_alpha = 1.0 - inner_eased;
        let inner_width = ((1.0 - inner_eased) * 3.0).max(1.0);

        ctx.set_stroke_color(Color::from_rgb8(0, 200, 220).with_alpha((inner_alpha * 0.6) as f32));
        ctx.set_line_width(inner_width);
        ctx.begin_path();
        ctx.circle(x, y, inner_radius);
        ctx.stroke();
    }

    // Particles radiating outward.
    let num_particles = ((radius / 5.0).floor() as i32 + 4).clamp(1, 12);
    let particle_alpha = ((1.0 - eased) * 0.7) as f32;
    let particle_size = ((1.0 - eased) * 3.0).max(1.0);
    let dist = radius * 0.5 + eased * radius * 1.2;

    ctx.set_fill_color(Color::from_rgb8(0, 255, 242).with_alpha(particle_alpha));
    for i in 0..num_particles {
        let angle = (i as f64 / num_particles as f64) * std::f64::consts::TAU;
        let px = x + angle.cos() * dist;
        let py = y + angle.sin() * dist;
        ctx.begin_path();
        ctx.circle(px, py, particle_size);
        ctx.fill();
    }
}

// ---- shared sub-helpers ----

/// Two white eyes with black pupils at (-4,-2) / (+4,-2) relative to (x,y).
/// `alpha` scales both white and black opacity (used by the dying-virus morph).
fn paint_eyes_alive(ctx: &mut dyn DrawCtx, x: f64, y: f64, alpha: f32) {
    if alpha < 0.01 {
        return;
    }
    ctx.set_fill_color(Color::from_rgb8(255, 255, 255).with_alpha(alpha));
    ctx.begin_path();
    ctx.circle(x - 4.0, y - 2.0, 3.0);
    ctx.fill();
    ctx.begin_path();
    ctx.circle(x + 4.0, y - 2.0, 3.0);
    ctx.fill();
    ctx.set_fill_color(Color::from_rgb8(0, 0, 0).with_alpha(alpha));
    ctx.begin_path();
    ctx.circle(x - 4.0, y - 2.0, 1.5);
    ctx.fill();
    ctx.begin_path();
    ctx.circle(x + 4.0, y - 2.0, 1.5);
    ctx.fill();
}

/// X eyes — 4 short diagonal strokes around (x,y). `alpha` scales opacity.
fn paint_eyes_x(ctx: &mut dyn DrawCtx, x: f64, y: f64, alpha: f32) {
    if alpha < 0.01 {
        return;
    }
    ctx.set_stroke_color(Color::from_rgb8(0x66, 0x66, 0x66).with_alpha(alpha));
    ctx.set_line_width(2.0);
    // Left X
    ctx.begin_path();
    ctx.move_to(x - 5.0, y - 4.0);
    ctx.line_to(x - 2.0, y - 1.0);
    ctx.move_to(x - 2.0, y - 4.0);
    ctx.line_to(x - 5.0, y - 1.0);
    ctx.stroke();
    // Right X
    ctx.begin_path();
    ctx.move_to(x + 2.0, y - 4.0);
    ctx.line_to(x + 5.0, y - 1.0);
    ctx.move_to(x + 5.0, y - 4.0);
    ctx.line_to(x + 2.0, y - 1.0);
    ctx.stroke();
}
