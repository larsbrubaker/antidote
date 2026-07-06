//! `LifeLostOverlay` — the Petri Pop life-lost interstitial (design screen
//! 06): a coral vignette pulses over the playfield while "−1 LIFE" floats up
//! from the death point with a "N LEFT — BREATHE" caption. Purely visual;
//! input stays live.

use agg_gui::geometry::Size;
use agg_gui::paints::RadialGradientPaint;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::game::update::LIFE_LOST_DURATION;
use crate::theme::{self, Fonts};
use crate::ui::game_model::SharedModel;
use crate::ui::game_widget::arena_letterbox;
use crate::ui::paint_util::fill_text_centered;

/// How far the text block rises over the interstitial (design units).
const FLOAT_RISE: f64 = 60.0;

pub struct LifeLostOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
}

impl LifeLostOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
        }
    }
}

impl Widget for LifeLostOverlay {
    fn type_name(&self) -> &'static str {
        "LifeLostOverlay"
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
        self.model.borrow().world.phase == Phase::LifeLost
    }
    fn layout(&mut self, available: Size) -> Size {
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let (death, lives, elapsed) = {
            let m = self.model.borrow();
            (
                m.world.last_life_lost_at,
                m.world.lives,
                m.world.phase_elapsed,
            )
        };
        let Some((dx, dy)) = death else {
            return;
        };

        let lb = arena_letterbox(self.bounds.width, self.bounds.height);
        if lb.scale <= 0.0 {
            return;
        }
        // JS-style logical (Y-down) → widget Y-up pixels.
        let death_x = (lb.offset_x + dx * lb.scale) as f64;
        let death_y = (lb.offset_y + lb.game_h - dy * lb.scale) as f64;

        let t_raw = (elapsed / LIFE_LOST_DURATION).clamp(0.0, 1.0) as f64;

        // Coral vignette over the playfield panel: transparent at the death
        // point, coral at the panel edge. Alpha ramps 0→1→0 (in 20%,
        // out after 60%).
        let vignette_a = if t_raw < 0.2 {
            t_raw / 0.2
        } else if t_raw > 0.6 {
            ((1.0 - t_raw) / 0.4).max(0.0)
        } else {
            1.0
        };
        if vignette_a > 0.0 {
            let peak = 0.26 * vignette_a;
            let coral = theme::CORAL_500;
            let panel_w = self.bounds.width - 2.0 * theme::RAIL_W;
            let radius = (panel_w.powi(2) + self.bounds.height.powi(2)).sqrt() * 0.55;
            ctx.set_fill_radial_gradient(RadialGradientPaint::centered(
                death_x,
                death_y,
                radius,
                &[
                    (0.0, coral.with_alpha(0.0)),
                    (0.48, coral.with_alpha(0.0)),
                    (1.0, coral.with_alpha(peak as f32)),
                ],
            ));
            ctx.begin_path();
            ctx.rect(theme::PLAYFIELD_X, 0.0, panel_w, self.bounds.height);
            ctx.fill();
        }

        // Text block floats up from just above the death point and fades in
        // fast / out at the end. Quadratic ease-out on the rise.
        let rise = 1.0 - (1.0 - t_raw).powi(2);
        let text_a = if t_raw < 0.12 {
            t_raw / 0.12
        } else if t_raw > 0.8 {
            ((1.0 - t_raw) / 0.2).max(0.0)
        } else {
            1.0
        } as f32;
        let base_y = death_y + 90.0 + rise * FLOAT_RISE;
        // Keep the block inside the playfield panel.
        let cx = death_x.clamp(
            theme::PLAYFIELD_X + 140.0,
            self.bounds.width - theme::RAIL_W - 140.0,
        );
        let base_y = base_y.min(self.bounds.height - 80.0);

        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(46.0);
        ctx.set_fill_color(theme::CORAL_500.with_alpha(text_a));
        fill_text_centered(ctx, "−1 LIFE", cx, base_y, 1.0);

        let caption = match lives {
            0 => "NONE LEFT".to_string(),
            1 => "1 LEFT — BREATHE".to_string(),
            n => format!("{n} LEFT — BREATHE"),
        };
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(16.0);
        ctx.set_fill_color(theme::TEXT_MID.with_alpha(text_a));
        fill_text_centered(ctx, &caption, cx, base_y - 30.0, 3.0);
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
