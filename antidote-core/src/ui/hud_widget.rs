//! `HudWidget` — top bar with lives, level, antidote bar, and score.
//!
//! Reads game state through [`SharedModel`] each paint. The widget bounds span
//! the full window; only the top-bar rect ([`HUD_HEIGHT`] tall) actually paints
//! and only that rect hits — clicks below pass through to the game canvas
//! beneath in the [`OverlayStack`](super::overlay_stack::OverlayStack).

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Point, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::SharedModel;

/// Top bar height in logical pixels.
pub const HUD_HEIGHT: f64 = 40.0;
/// Inner padding on the left and right edges of the top bar.
const HUD_PAD_X: f64 = 16.0;
/// Body font size for HUD text.
const HUD_FONT_SIZE: f64 = 16.0;
/// Width of the antidote bar (px).
const ANTIDOTE_BAR_W: f64 = 200.0;
/// Height of the antidote bar.
const ANTIDOTE_BAR_H: f64 = 14.0;

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

    /// Limit hit-testing to the top-bar zone so clicks on the play area below
    /// fall through to the game canvas underneath in the OverlayStack.
    fn hit_test(&self, local_pos: Point) -> bool {
        let h = self.bounds.height;
        local_pos.y >= h - HUD_HEIGHT
            && local_pos.y <= h
            && local_pos.x >= 0.0
            && local_pos.x <= self.bounds.width
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
        let bar_y = h - HUD_HEIGHT;

        // Translucent dark backdrop for the top bar.
        ctx.set_fill_color(Color::rgba(0.05, 0.07, 0.12, 0.78));
        ctx.begin_path();
        ctx.rect(0.0, bar_y, w, HUD_HEIGHT);
        ctx.fill();

        // Subtle bottom edge highlight.
        ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.10));
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(0.0, bar_y);
        ctx.line_to(w, bar_y);
        ctx.stroke();

        ctx.set_font(self.font.clone());
        ctx.set_font_size(HUD_FONT_SIZE);

        let text_color = Color::rgba(0.92, 0.95, 1.0, 1.0);
        let dim_color = Color::rgba(0.55, 0.62, 0.75, 1.0);
        let baseline_y = bar_y + (HUD_HEIGHT - HUD_FONT_SIZE) * 0.5 + 2.0;

        // Left: "Lives: N"
        let lives_text = format!("Lives: {}", world.lives);
        ctx.set_fill_color(text_color);
        ctx.fill_text(&lives_text, HUD_PAD_X, baseline_y);

        // Center-left: "Level N"
        let level_text = format!("Level {}", world.level);
        let level_x = (w * 0.30).max(HUD_PAD_X + 120.0);
        ctx.set_fill_color(dim_color);
        ctx.fill_text(&level_text, level_x, baseline_y);

        // Right side: "Score: N"
        let score_text = format!("Score: {}", world.total_score);
        let score_w = ctx
            .measure_text(&score_text)
            .map(|m| m.width)
            .unwrap_or(80.0);
        let score_x = w - HUD_PAD_X - score_w;
        ctx.set_fill_color(text_color);
        ctx.fill_text(&score_text, score_x, baseline_y);

        // Antidote bar — between Level and Score.
        let bar_x = (level_x + 80.0).min(score_x - ANTIDOTE_BAR_W - 16.0);
        let bar_y_inner = bar_y + (HUD_HEIGHT - ANTIDOTE_BAR_H) * 0.5;
        ctx.set_fill_color(Color::rgba(0.15, 0.18, 0.25, 1.0));
        ctx.begin_path();
        ctx.rect(bar_x, bar_y_inner, ANTIDOTE_BAR_W, ANTIDOTE_BAR_H);
        ctx.fill();

        let fill_w = ANTIDOTE_BAR_W * world.antidote.clamp(0.0, 1.0) as f64;
        // Color shifts from green (full) to red (empty).
        let t = world.antidote.clamp(0.0, 1.0);
        let r = 1.0 - t * 0.85;
        let g = 0.20 + t * 0.70;
        let b = 0.35 + t * 0.10;
        ctx.set_fill_color(Color::rgba(r, g, b, 1.0));
        ctx.begin_path();
        ctx.rect(bar_x, bar_y_inner, fill_w, ANTIDOTE_BAR_H);
        ctx.fill();

        // Antidote-bar border.
        ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.20));
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.rect(bar_x, bar_y_inner, ANTIDOTE_BAR_W, ANTIDOTE_BAR_H);
        ctx.stroke();
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        // Live HUD numbers — keep redrawing while visible.
        true
    }
}
