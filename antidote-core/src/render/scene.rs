//! Scene painter. Pixel-faithful reproduction of the JS Canvas rendering at
//! `reference/GFG/public/games/antidote/antidote-rendering.js`.
//!
//! Coordinate convention inside this module: JS-style Y-down. The widget's
//! `paint` method applies a single transform that maps the (0..VIRTUAL_WIDTH,
//! 0..VIRTUAL_HEIGHT) JS-down logical box into the widget's letterboxed Y-up
//! pixel area, so every helper below works in JS coordinates.

use agg_gui::draw_ctx::{GradientSpread, GradientStop, RadialGradientPaint};
use agg_gui::{Color, DrawCtx, TransAffine};

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::state::{
    Bubble, DeadVirus, DyingVirus, GrowingBubble, PopAnimation, Virus, World,
};

#[inline]
pub fn flip_y(y: f32) -> f32 {
    VIRTUAL_HEIGHT - y
}

/// `rgba(r, g, b, a)` from CSS-style 8-bit RGB + 0..1 alpha.
#[inline]
fn css_rgba(r: u8, g: u8, b: u8, a: f32) -> Color {
    Color::rgba(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
}

/// `#RRGGBB` literal at full alpha.
#[inline]
fn hex(r: u8, g: u8, b: u8) -> Color {
    css_rgba(r, g, b, 1.0)
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
    ctx.set_fill_color(hex(5, 5, 8));
    ctx.begin_path();
    ctx.rect(0.0, 0.0, GAME_W, GAME_H);
    ctx.fill();

    // Grid lines (cyan, 5% alpha, 1 px)
    ctx.set_stroke_color(css_rgba(0, 255, 242, 0.05));
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
    ctx.set_stroke_color(css_rgba(0, 255, 242, 0.6));
    ctx.set_line_width(3.0);
    ctx.begin_path();
    ctx.rect(1.5, 1.5, GAME_W - 3.0, GAME_H - 3.0);
    ctx.stroke();
}

// ---- per-entity helpers (M2-E+, currently stubs that match the JS contract) ----

pub fn paint_bubble(_ctx: &mut dyn DrawCtx, _b: &Bubble, _is_growing: bool) {
    // M2-E.
}

pub fn paint_growing_bubble(_ctx: &mut dyn DrawCtx, _g: &GrowingBubble) {
    // M2-E.
}

pub fn paint_virus(_ctx: &mut dyn DrawCtx, _v: &Virus, _time_seconds: f32) {
    // M2-E.
}

pub fn paint_dying_virus(_ctx: &mut dyn DrawCtx, _dv: &DyingVirus, _time_seconds: f32) {
    // M2-F.
}

pub fn paint_dead_virus(_ctx: &mut dyn DrawCtx, _d: &DeadVirus) {
    // M2-E.
}

pub fn paint_pop_animation(_ctx: &mut dyn DrawCtx, _p: &PopAnimation) {
    // M2-F.
}

/// Build a 3-stop radial gradient centered at (cx, cy) with outer radius r.
#[allow(dead_code)] // used by per-entity painters in M2-E
pub(crate) fn radial_3(
    cx: f64,
    cy: f64,
    r: f64,
    c0: Color,
    c1: Color,
    c2: Color,
) -> RadialGradientPaint {
    RadialGradientPaint {
        cx,
        cy,
        r,
        fx: cx,
        fy: cy,
        transform: TransAffine::default(),
        spread: GradientSpread::Pad,
        stops: vec![
            GradientStop {
                offset: 0.0,
                color: c0,
            },
            GradientStop {
                offset: 0.6,
                color: c1,
            },
            GradientStop {
                offset: 1.0,
                color: c2,
            },
        ],
    }
}
