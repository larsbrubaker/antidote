//! `RotateOverlay` — full-screen "rotate your device" prompt.
//!
//! Antidote's playfield is 4:3; on a phone held portrait the game would be a
//! small letterboxed strip, so instead this overlay covers everything and asks
//! the player to rotate. Browsers can't force landscape outside of fullscreen
//! (and never on iOS Safari), which makes this prompt the universal fallback —
//! the wasm shell additionally tries `screen.orientation.lock('landscape')`
//! where supported.
//!
//! Shown only when the platform shell reported a mobile environment
//! (`GameModel::is_mobile`, set from a coarse-pointer media query) AND the
//! canvas is taller than it is wide. Desktop windows of any shape never see
//! it, and the native shell never sets `is_mobile` at all.
//!
//! While visible during active play the overlay force-pauses the game. It
//! deliberately does not auto-resume on rotating back — the regular
//! `PauseOverlay` is revealed instead so the player re-enters with a tap,
//! not with a virus already an inch from their bubble.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::SharedModel;

pub struct RotateOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
}

impl RotateOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
        }
    }

    fn should_show(&self, w: f64, h: f64) -> bool {
        self.model.borrow().is_mobile && w < h
    }
}

impl Widget for RotateOverlay {
    fn type_name(&self) -> &'static str {
        "RotateOverlay"
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

    fn is_visible(&self) -> bool {
        self.should_show(self.bounds.width, self.bounds.height)
    }

    fn layout(&mut self, available: Size) -> Size {
        // Force-pause active play while the prompt covers the screen. Runs
        // every layout pass; guarding on `Playing` keeps it from fighting the
        // player once they're on the pause screen (or any other phase).
        if self.model.borrow().is_mobile && available.width < available.height {
            let mut m = self.model.borrow_mut();
            if m.world.phase == Phase::Playing {
                m.world.phase = Phase::Paused;
            }
        }
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;

        // Opaque backdrop — unlike menu backdrops, nothing underneath is
        // meant to read through a prompt that says "you can't play yet";
        // the pause screen bleeding through just looks broken.
        ctx.set_fill_color(Color::rgba(0.04, 0.06, 0.10, 1.0));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        let cx = w * 0.5;
        let cy = h * 0.5;
        let text_color = Color::rgba(0.92, 0.95, 1.0, 1.0);
        let dim_color = Color::rgba(0.55, 0.62, 0.75, 1.0);

        // Phone glyph in the target (landscape) orientation, above the text.
        let glyph_cy = cy + 70.0;
        let phone_w = 96.0;
        let phone_h = 56.0;
        ctx.set_stroke_color(text_color);
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.rounded_rect(
            cx - phone_w * 0.5,
            glyph_cy - phone_h * 0.5,
            phone_w,
            phone_h,
            10.0,
        );
        ctx.stroke();

        // Rotation hint: a circular arrow sweeping over the phone. Y-up, so
        // the arc's high-y half is the visually-upper half.
        let arc_r = 82.0;
        let start = 0.35_f64; // radians
        let end = std::f64::consts::PI - 0.35;
        ctx.set_stroke_color(dim_color);
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.arc_to(cx, glyph_cy, arc_r, start, end, false);
        ctx.stroke();

        // Arrowhead at the arc's end, aligned to the sweep tangent.
        let tip_x = cx + arc_r * end.cos();
        let tip_y = glyph_cy + arc_r * end.sin();
        let tangent = end + std::f64::consts::FRAC_PI_2;
        let head = 10.0;
        ctx.set_fill_color(dim_color);
        ctx.begin_path();
        ctx.move_to(
            tip_x + head * tangent.cos(),
            tip_y + head * tangent.sin(),
        );
        ctx.line_to(
            tip_x + head * 0.6 * (end + std::f64::consts::PI).cos(),
            tip_y + head * 0.6 * (end + std::f64::consts::PI).sin(),
        );
        ctx.line_to(
            tip_x + head * 0.6 * end.cos(),
            tip_y + head * 0.6 * end.sin(),
        );
        ctx.close_path();
        ctx.fill();

        // Centered text below the glyph (Y-up: lower on screen = smaller y).
        ctx.set_font(self.font.clone());
        ctx.set_font_size(20.0);
        let title = "Rotate your device";
        let title_w = ctx.measure_text(title).map(|m| m.width).unwrap_or(200.0);
        ctx.set_fill_color(text_color);
        ctx.fill_text(title, cx - title_w * 0.5, cy - 40.0);

        ctx.set_font_size(15.0);
        let sub = "Antidote plays in landscape";
        let sub_w = ctx.measure_text(sub).map(|m| m.width).unwrap_or(180.0);
        ctx.set_fill_color(dim_color);
        ctx.fill_text(sub, cx - sub_w * 0.5, cy - 70.0);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        // Swallow pointer input — nothing behind the prompt should react to
        // stray taps. Keys bubble so Esc/P shortcuts stay live for testing.
        match event {
            Event::MouseMove { .. }
            | Event::MouseDown { .. }
            | Event::MouseUp { .. }
            | Event::MouseWheel { .. } => EventResult::Consumed,
            _ => EventResult::Ignored,
        }
    }

    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
