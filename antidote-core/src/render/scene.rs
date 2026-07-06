//! Scene painter — the Petri Pop reskin of the JS Canvas rendering at
//! `gfg/public/games/antidote/antidote-rendering.js`. Geometry, animation
//! phases, and draw order still mirror the JS reference; colors and the dish
//! chrome follow `docs/New Design/Antidote Frame.dc.html`.
//!
//! Coordinate convention for the entity helpers: JS-style Y-down. The
//! widget's `paint` method applies a single transform that maps the
//! (0..VIRTUAL_WIDTH, 0..VIRTUAL_HEIGHT) JS-down logical box into the
//! widget's letterboxed Y-up pixel area. The dish-panel helpers
//! ([`paint_dish_panel`], [`paint_arena_stroke`]) are the exception — they
//! paint in widget Y-up coordinates *before* that transform, because the
//! dish and grid extend beyond the live arena.

use agg_gui::draw_ctx::RadialGradientPaint;
use agg_gui::{Color, DrawCtx, Rect};

use crate::consts::VIRTUAL_HEIGHT;
use crate::game::state::{
    Bubble, DeadVirus, DyingVirus, GrowingBubble, PopAnimation, Virus, World,
};
use crate::theme;

#[inline]
pub fn flip_y(y: f32) -> f32 {
    VIRTUAL_HEIGHT - y
}

/// Top-level scene paint. Mirrors `render(ctx, state)` in the JS reference:
/// background → grid → border → solid bubbles → dead viruses → dying viruses →
/// growing bubble → viruses → pop animations.
pub fn paint_scene(world: &World, _time_seconds: f32) {
    // Routed via `GameWidget::paint` (which has the DrawCtx). This top-level
    // function is a placeholder so the module compiles standalone in tests.
    let _ = world;
}

/// The dish: panel background + 40-unit grid, in widget Y-up coordinates.
/// Grid lines align to the panel origin so they meet the rails cleanly.
pub fn paint_dish_panel(ctx: &mut dyn DrawCtx, r: Rect) {
    ctx.set_fill_color(theme::INK_800);
    ctx.begin_path();
    ctx.rect(r.x, r.y, r.width, r.height);
    ctx.fill();

    ctx.set_stroke_color(theme::GRID_LINE);
    ctx.set_line_width(1.0);
    ctx.begin_path();
    let mut x = r.x;
    while x <= r.x + r.width + 0.0001 {
        ctx.move_to(x, r.y);
        ctx.line_to(x, r.y + r.height);
        x += theme::GRID_CELL;
    }
    let mut y = r.y;
    while y <= r.y + r.height + 0.0001 {
        ctx.move_to(r.x, y);
        ctx.line_to(r.x + r.width, y);
        y += theme::GRID_CELL;
    }
    ctx.stroke();
}

/// Arena boundary: rounded violet stroke on the live-area rect (widget Y-up
/// coordinates). The corner rounding is cosmetic — physics walls are square.
pub fn paint_arena_stroke(ctx: &mut dyn DrawCtx, r: Rect) {
    ctx.set_stroke_color(theme::ARENA_STROKE);
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.rounded_rect(r.x, r.y, r.width, r.height, theme::ARENA_RADIUS);
    ctx.stroke();
}

// ---- per-entity helpers ----

pub fn paint_bubble(ctx: &mut dyn DrawCtx, b: &Bubble, is_growing: bool) {
    paint_bubble_at(ctx, b.x as f64, b.y as f64, b.radius as f64, is_growing);
}

pub fn paint_growing_bubble(ctx: &mut dyn DrawCtx, g: &GrowingBubble) {
    paint_bubble_at(ctx, g.x as f64, g.y as f64, g.radius as f64, true);
}

/// The Petri Pop bubble: lime radial fill (bright core fading to near-clear
/// at 72%), 2px pale-lime stroke, and a rotated specular ellipse upper-left.
/// The growing bubble is brighter than a settled one so "this one is yours,
/// keep holding" reads at a glance.
fn paint_bubble_at(ctx: &mut dyn DrawCtx, x: f64, y: f64, radius: f64, is_growing: bool) {
    let lime = theme::LIME_500;
    let (a_core, a_edge, stroke_color, stroke_w) = if is_growing {
        (0.28_f32, 0.06_f32, lime.with_alpha(0.95), 2.0_f64)
    } else {
        (0.20, 0.03, theme::LIME_STROKE, 2.0)
    };

    // Body — radial gradient fill, slightly off-center toward the light.
    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x - radius * 0.15,
        y - radius * 0.2,
        radius * 1.15,
        &[
            (0.0, lime.with_alpha(a_core)),
            (0.72, lime.with_alpha(a_edge)),
            (1.0, lime.with_alpha(0.0)),
        ],
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

    // Specular highlight: rotated ellipse upper-left (JS Y-down: -y is up).
    ctx.save();
    ctx.translate(x - radius * 0.42, y - radius * 0.55);
    ctx.rotate(-0.56); // ≈ -32°
    ctx.scale(1.0, 0.45);
    ctx.set_fill_color(Color::from_rgb8(255, 255, 255).with_alpha(0.4));
    ctx.begin_path();
    ctx.circle(0.0, 0.0, radius * 0.18);
    ctx.fill();
    ctx.restore();
}

/// The virus: same geometry and animation as the JS reference (wobble, 8
/// orbiting spikes, eyes — the character survives the redesign), recolored
/// to the coral gradient from the mockups.
pub fn paint_virus(ctx: &mut dyn DrawCtx, v: &Virus, time_seconds: f32) {
    let x = v.x as f64;
    let y = v.y as f64;
    let radius = crate::consts::VIRUS_RADIUS as f64;
    let wobble = ((time_seconds * 5.0 + v.phase).sin() * 2.0) as f64;

    // Spikes first so the body overlaps their inner half (mockup look).
    ctx.set_fill_color(theme::CORAL_500);
    let orbit_r = radius + 4.0 + wobble;
    for i in 0..8 {
        let angle = (i as f64 / 8.0) * std::f64::consts::TAU + (time_seconds as f64) * 2.0;
        let sx = x + angle.cos() * orbit_r;
        let sy = y + angle.sin() * orbit_r;
        ctx.begin_path();
        ctx.circle(sx, sy, 4.0);
        ctx.fill();
    }

    // Body — coral radial: (255,140,110) → (255,92,72) @45% → (150,28,40) @95%.
    ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
        x - radius * 0.3,
        y - radius * 0.36,
        radius * 1.35,
        &[
            (0.0, theme::CORAL_300),
            (0.45, theme::CORAL_500),
            (0.95, theme::CORAL_800),
        ],
    ));
    ctx.begin_path();
    ctx.circle(x, y, radius + wobble);
    ctx.fill();

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

    // Body gradient — lerp alive coral → dead gray-lavender stops.
    let t = progress as f32;
    let center = theme::CORAL_300.lerp(Color::from_rgb8(80, 80, 100), t);
    let mid = theme::CORAL_500.lerp(Color::from_rgb8(60, 60, 80), t);
    let edge = theme::CORAL_800.lerp(Color::from_rgb8(40, 40, 60), t);

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
    let spike_color = theme::CORAL_500.lerp(Color::from_rgb8(100, 100, 120), t);
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

    ctx.set_stroke_color(theme::LIME_500.with_alpha((ring_alpha * 0.8) as f32));
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

        ctx.set_stroke_color(theme::LIME_400.with_alpha((inner_alpha * 0.6) as f32));
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

    // Alternate lime / white fragments, per the mockup's pop debris.
    for i in 0..num_particles {
        let color = if i % 2 == 0 {
            theme::LIME_500.with_alpha(particle_alpha)
        } else {
            Color::from_rgb8(255, 255, 255).with_alpha(particle_alpha * 0.85)
        };
        ctx.set_fill_color(color);
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
