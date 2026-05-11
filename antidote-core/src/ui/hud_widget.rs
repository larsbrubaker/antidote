//! `HudWidget` — lives, level, antidote bar, and score.
//!
//! Two layouts, picked per layout pass to maximize the playfield's scale on
//! the available canvas:
//!
//! - **Top strip** (default on portrait phones + tall windows): horizontal bar
//!   at the top, `HUD_HEIGHT` tall, full width.
//! - **Left strip** (landscape phones + wide windows): vertical bar down the
//!   left edge, `HUD_WIDTH` wide, full height. Lets a 16:9 phone in landscape
//!   put the playfield in the centre instead of squeezed under a top bar.
//!
//! Reads game state through [`SharedModel`] each paint. The widget bounds span
//! the full window; only the active strip rect actually paints and only that
//! rect hits — clicks elsewhere pass through to the game canvas beneath in the
//! [`OverlayStack`](super::overlay_stack::OverlayStack).

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Point, Rect, Widget};

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::state::Phase;
use crate::ui::game_model::SharedModel;

/// Top-strip thickness in logical pixels.
pub const HUD_HEIGHT: f64 = 40.0;
/// Left-strip thickness in logical pixels. Wide enough for "Lives: 2" /
/// "Level 1" / "Score: 999" at the body font size plus a vertical antidote bar.
pub const HUD_WIDTH: f64 = 92.0;
/// Inner padding on each edge of the strip.
const HUD_PAD: f64 = 16.0;
/// Body font size for HUD text.
const HUD_FONT_SIZE: f64 = 16.0;
/// Length of the antidote bar along its long axis.
const ANTIDOTE_BAR_LONG: f64 = 200.0;
/// Length of the antidote bar along its short axis.
const ANTIDOTE_BAR_SHORT: f64 = 14.0;

/// Which strip the HUD occupies — picked from available canvas size each layout
/// pass to maximize the resulting playfield scale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudLayout {
    TopStrip,
    LeftStrip,
}

impl HudLayout {
    /// Pick whichever strip lets the 4:3 playfield letterbox bigger. Compares
    /// the two `min(usable_w / VIRTUAL_WIDTH, usable_h / VIRTUAL_HEIGHT)`
    /// scales and chooses the larger; ties go to `TopStrip` for stability and
    /// because that's the long-standing default.
    pub fn for_available(w: f64, h: f64) -> Self {
        let pw = VIRTUAL_WIDTH as f64;
        let ph = VIRTUAL_HEIGHT as f64;
        let scale_top = (w / pw).min((h - HUD_HEIGHT).max(0.0) / ph).max(0.0);
        let scale_left = ((w - HUD_WIDTH).max(0.0) / pw).min(h / ph).max(0.0);
        if scale_left > scale_top {
            Self::LeftStrip
        } else {
            Self::TopStrip
        }
    }
}

/// Region the playfield occupies inside a canvas of `(w, h)` after the HUD
/// strip is carved off. Coordinates are agg-gui Y-up local pixels (origin at
/// bottom-left).
pub fn playfield_rect(layout: HudLayout, w: f64, h: f64) -> Rect {
    match layout {
        // HUD strip pinned to the top of the screen; in Y-up that's the
        // *high-y* slab. Playfield gets the low-y slab from 0..h-HUD_HEIGHT.
        HudLayout::TopStrip => Rect::new(0.0, 0.0, w, (h - HUD_HEIGHT).max(0.0)),
        // HUD strip pinned to the left edge. Playfield gets the right slab.
        HudLayout::LeftStrip => Rect::new(HUD_WIDTH, 0.0, (w - HUD_WIDTH).max(0.0), h),
    }
}

pub struct HudWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
}

impl HudWidget {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
        }
    }
}

impl Widget for HudWidget {
    fn type_name(&self) -> &'static str {
        "HudWidget"
    }

    fn bounds(&self) -> Rect {
        self.bounds
    }

    fn set_bounds(&mut self, bounds: Rect) {
        self.bounds = bounds;
    }

    fn children(&self) -> &[Box<dyn Widget>] {
        &self.children
    }

    fn children_mut(&mut self) -> &mut Vec<Box<dyn Widget>> {
        &mut self.children
    }

    fn layout(&mut self, available: Size) -> Size {
        available
    }

    /// Limit hit-testing to whichever strip we're currently painting so clicks
    /// on the play area below fall through to the game canvas underneath.
    fn hit_test(&self, local_pos: Point) -> bool {
        let w = self.bounds.width;
        let h = self.bounds.height;
        match HudLayout::for_available(w, h) {
            HudLayout::TopStrip => {
                // Top in screen-space = high-y in agg-gui Y-up local.
                local_pos.y >= h - HUD_HEIGHT
                    && local_pos.y <= h
                    && local_pos.x >= 0.0
                    && local_pos.x <= w
            }
            HudLayout::LeftStrip => {
                local_pos.x >= 0.0
                    && local_pos.x <= HUD_WIDTH
                    && local_pos.y >= 0.0
                    && local_pos.y <= h
            }
        }
    }

    fn is_visible(&self) -> bool {
        let phase = self.model.borrow().world.phase;
        matches!(
            phase,
            Phase::Playing | Phase::Paused | Phase::LevelComplete | Phase::LifeLost
        )
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let model = self.model.borrow();
        let world = &model.world;
        let w = self.bounds.width;
        let h = self.bounds.height;
        let layout = HudLayout::for_available(w, h);
        ctx.set_font(self.font.clone());
        ctx.set_font_size(HUD_FONT_SIZE);
        match layout {
            HudLayout::TopStrip => paint_top_strip(ctx, w, h, world),
            HudLayout::LeftStrip => paint_left_strip(ctx, h, world),
        }
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        // Live HUD numbers — keep redrawing while visible.
        true
    }
}

fn paint_top_strip(ctx: &mut dyn DrawCtx, w: f64, h: f64, world: &crate::game::state::World) {
    let bar_y = h - HUD_HEIGHT;

    ctx.set_fill_color(Color::rgba(0.05, 0.07, 0.12, 0.78));
    ctx.begin_path();
    ctx.rect(0.0, bar_y, w, HUD_HEIGHT);
    ctx.fill();

    ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.10));
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(0.0, bar_y);
    ctx.line_to(w, bar_y);
    ctx.stroke();

    let text_color = Color::rgba(0.92, 0.95, 1.0, 1.0);
    let dim_color = Color::rgba(0.55, 0.62, 0.75, 1.0);
    let baseline_y = bar_y + (HUD_HEIGHT - HUD_FONT_SIZE) * 0.5 + 2.0;

    // Left: "Lives: N"
    let lives_text = format!("Lives: {}", world.lives);
    ctx.set_fill_color(text_color);
    ctx.fill_text(&lives_text, HUD_PAD, baseline_y);

    // Center-left: "Level N"
    let level_text = format!("Level {}", world.level);
    let level_x = (w * 0.30).max(HUD_PAD + 120.0);
    ctx.set_fill_color(dim_color);
    ctx.fill_text(&level_text, level_x, baseline_y);

    // Right: "Score: N"
    let score_text = format!("Score: {}", world.total_score);
    let score_w = ctx
        .measure_text(&score_text)
        .map(|m| m.width)
        .unwrap_or(80.0);
    let score_x = w - HUD_PAD - score_w;
    ctx.set_fill_color(text_color);
    ctx.fill_text(&score_text, score_x, baseline_y);

    // Antidote bar — horizontal, between Level and Score.
    let bar_x = (level_x + 80.0).min(score_x - ANTIDOTE_BAR_LONG - 16.0);
    let bar_y_inner = bar_y + (HUD_HEIGHT - ANTIDOTE_BAR_SHORT) * 0.5;
    paint_antidote_bar(
        ctx,
        bar_x,
        bar_y_inner,
        ANTIDOTE_BAR_LONG,
        ANTIDOTE_BAR_SHORT,
        world.antidote,
        true,
    );
}

fn paint_left_strip(ctx: &mut dyn DrawCtx, h: f64, world: &crate::game::state::World) {
    ctx.set_fill_color(Color::rgba(0.05, 0.07, 0.12, 0.78));
    ctx.begin_path();
    ctx.rect(0.0, 0.0, HUD_WIDTH, h);
    ctx.fill();

    ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.10));
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.move_to(HUD_WIDTH, 0.0);
    ctx.line_to(HUD_WIDTH, h);
    ctx.stroke();

    let text_color = Color::rgba(0.92, 0.95, 1.0, 1.0);
    let dim_color = Color::rgba(0.55, 0.62, 0.75, 1.0);

    // Lay out four blocks top-down in screen order. Agg-gui is Y-up, so
    // "top of screen" is high y.
    let row_h = HUD_FONT_SIZE + 6.0;
    let lives_y = h - HUD_PAD - HUD_FONT_SIZE; // baseline of top label
    let level_y = lives_y - row_h;
    let score_y = HUD_PAD + 2.0; // baseline of bottom label

    let lives_text = format!("Lives: {}", world.lives);
    ctx.set_fill_color(text_color);
    ctx.fill_text(&lives_text, HUD_PAD * 0.5, lives_y);

    let level_text = format!("Level {}", world.level);
    ctx.set_fill_color(dim_color);
    ctx.fill_text(&level_text, HUD_PAD * 0.5, level_y);

    let score_text = format!("Score: {}", world.total_score);
    ctx.set_fill_color(text_color);
    ctx.fill_text(&score_text, HUD_PAD * 0.5, score_y);

    // Antidote bar — vertical, centred between the level and score blocks,
    // fills bottom-up like a thermometer.
    let bar_x = (HUD_WIDTH - ANTIDOTE_BAR_SHORT) * 0.5;
    let bar_top_y = level_y - HUD_FONT_SIZE - 10.0;
    let bar_bot_y = score_y + 10.0;
    let bar_h = (bar_top_y - bar_bot_y).max(0.0);
    let bar_h_long = bar_h.min(ANTIDOTE_BAR_LONG);
    let bar_y0 = bar_bot_y + (bar_h - bar_h_long) * 0.5;
    paint_antidote_bar(
        ctx,
        bar_x,
        bar_y0,
        ANTIDOTE_BAR_SHORT,
        bar_h_long,
        world.antidote,
        false,
    );
}

/// Antidote bar rectangle filled from one end to the other. `horizontal=true`
/// fills left-to-right; `horizontal=false` fills bottom-up.
fn paint_antidote_bar(
    ctx: &mut dyn DrawCtx,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    antidote: f32,
    horizontal: bool,
) {
    ctx.set_fill_color(Color::rgba(0.15, 0.18, 0.25, 1.0));
    ctx.begin_path();
    ctx.rect(x, y, w, h);
    ctx.fill();

    let t = antidote.clamp(0.0, 1.0);
    let r = 1.0 - t * 0.85;
    let g = 0.20 + t * 0.70;
    let b = 0.35 + t * 0.10;
    ctx.set_fill_color(Color::rgba(r, g, b, 1.0));
    ctx.begin_path();
    if horizontal {
        ctx.rect(x, y, w * t as f64, h);
    } else {
        ctx.rect(x, y, w, h * t as f64);
    }
    ctx.fill();

    ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.20));
    ctx.set_line_width(1.0);
    ctx.begin_path();
    ctx.rect(x, y, w, h);
    ctx.stroke();
}
