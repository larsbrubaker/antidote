//! `RotateOverlay` — the "TURN ME SIDEWAYS!" prompt (design screen 09).
//!
//! On a phone held portrait the landscape canvas would be a tiny letterboxed
//! strip, so this overlay covers the whole viewport and asks the player to
//! rotate. Browsers can't force landscape outside of fullscreen (and never
//! on iOS Safari), which makes the prompt the universal fallback — the wasm
//! shell additionally tries `screen.orientation.lock('landscape')` where
//! supported.
//!
//! Shown only when the platform shell reported a mobile environment
//! (`GameModel::is_mobile`, from a coarse-pointer media query) AND the
//! viewport is taller than wide. It lives as a full-viewport sibling of the
//! fixed canvas inside [`CanvasRoot`](crate::ui::canvas_root::CanvasRoot) —
//! inside the canvas its bounds would always be landscape and the portrait
//! check could never fire.
//!
//! While visible during active play the overlay force-pauses the game. It
//! deliberately does not auto-resume on rotating back — the regular
//! `PauseOverlay` is revealed instead so the player re-enters with a tap,
//! not with a virus already an inch from their bubble.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::theme::{self, Fonts};
use crate::ui::game_model::SharedModel;
use crate::ui::paint_util::fill_text_centered;
use crate::ui::petri_kit::{paint_logo_bubble, paint_menu_backdrop, paint_mini_virus, swallow_mouse};

pub struct RotateOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
}

impl RotateOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
        }
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
        self.model.borrow().is_mobile && self.bounds.width < self.bounds.height
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
        // Opaque — nothing behind a "you can't play yet" prompt should
        // read through.
        paint_menu_backdrop(ctx, w, h);

        let cx = w * 0.5;
        let cy = h * 0.5;

        // Phone glyph tilted mid-rotation, with a bubble + virus on its
        // screen. Glyph block sits above the text.
        let glyph_cy = cy + h * 0.14;
        ctx.save();
        ctx.translate(cx, glyph_cy);
        ctx.rotate(-18.0 * std::f64::consts::PI / 180.0);
        ctx.set_stroke_color(theme::TEXT_HI);
        ctx.set_line_width(5.0);
        ctx.begin_path();
        ctx.rounded_rect(-75.0, -135.0, 150.0, 270.0, 30.0);
        ctx.stroke();
        // Speaker slot near the top edge.
        ctx.set_fill_color(theme::TEXT_LOW.with_alpha(0.6));
        ctx.begin_path();
        ctx.rounded_rect(-22.0, 111.0, 44.0, 8.0, 4.0);
        ctx.fill();
        // Bubble + trapped virus on the phone screen.
        paint_logo_bubble(ctx, -6.0, 4.0, 33.0, 3.0);
        paint_mini_virus(ctx, -6.0, 4.0, 13.0);
        ctx.restore();

        // Rotation sweep arc + arrowhead (lime), around the glyph.
        let arc_r = 150.0;
        let start = 0.5_f64;
        let end = 1.9_f64;
        ctx.set_stroke_color(theme::LIME_500);
        ctx.set_line_width(4.0);
        ctx.begin_path();
        ctx.arc_to(cx, glyph_cy, arc_r, start, end, false);
        ctx.stroke();
        let tip_x = cx + arc_r * end.cos();
        let tip_y = glyph_cy + arc_r * end.sin();
        let tangent = end + std::f64::consts::FRAC_PI_2;
        ctx.set_fill_color(theme::LIME_500);
        ctx.begin_path();
        ctx.move_to(
            tip_x + 14.0 * tangent.cos(),
            tip_y + 14.0 * tangent.sin(),
        );
        ctx.line_to(
            tip_x + 9.0 * (end + std::f64::consts::PI).cos(),
            tip_y + 9.0 * (end + std::f64::consts::PI).sin(),
        );
        ctx.line_to(tip_x + 9.0 * end.cos(), tip_y + 9.0 * end.sin());
        ctx.close_path();
        ctx.fill();

        // Text block below the glyph.
        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(46.0);
        ctx.set_fill_color(theme::LIME_500);
        fill_text_centered(ctx, "TURN ME SIDEWAYS!", cx, cy - h * 0.13, 0.0);

        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(22.0);
        ctx.set_fill_color(theme::TEXT_MID);
        fill_text_centered(ctx, "Antidote plays in landscape.", cx, cy - h * 0.13 - 42.0, 0.0);

        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(17.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_centered(
            ctx,
            "the viruses need the elbow room",
            cx,
            cy - h * 0.13 - 76.0,
            0.0,
        );
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        // Swallow pointer input — nothing behind the prompt should react to
        // stray taps. Keys bubble so Esc/P shortcuts stay live for testing.
        swallow_mouse(event)
    }

    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
