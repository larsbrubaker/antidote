//! `LifeLostOverlay` — animates a small "−1" indicator floating from the
//! death point on the playfield up to the HUD's lives counter while the game
//! is in [`Phase::LifeLost`]. Purely visual; no event handling.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::state::Phase;
use crate::game::update::LIFE_LOST_DURATION;
use crate::ui::game_model::SharedModel;
use crate::ui::hud_widget::{playfield_rect, HudLayout, HUD_HEIGHT, HUD_WIDTH};

const HEART_RADIUS: f64 = 18.0;
const FONT_SIZE: f64 = 18.0;

pub struct LifeLostOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
}

impl LifeLostOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
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
        let model = self.model.borrow();
        let world = &model.world;
        let Some((dx, dy)) = world.last_life_lost_at else {
            return;
        };

        // Letterbox transform — keeps the start anchor lined up with the
        // exact pixel where the bubble popped. Mirrors GameWidget::letterbox,
        // including the HUD-strip carve-out so the "−1" rises from inside
        // the play area, not from inside the chrome.
        let w = self.bounds.width;
        let h = self.bounds.height;
        let layout = HudLayout::for_available(w, h);
        let play = playfield_rect(layout, w, h);
        let play_w = play.width as f32;
        let play_h = play.height as f32;
        let target = VIRTUAL_WIDTH / VIRTUAL_HEIGHT;
        let widget_aspect = if play_h > 0.0 {
            play_w / play_h
        } else {
            target
        };
        let scale = if widget_aspect >= target {
            play_h / VIRTUAL_HEIGHT
        } else {
            play_w / VIRTUAL_WIDTH
        };
        if scale <= 0.0 {
            return;
        }
        let game_w = VIRTUAL_WIDTH * scale;
        let game_h = VIRTUAL_HEIGHT * scale;
        let offset_x = play.x as f32 + (play_w - game_w) * 0.5;
        let offset_y = play.y as f32 + (play_h - game_h) * 0.5;

        // JS-style logical (Y-down) → widget Y-up pixels.
        let start_x = (offset_x + dx * scale) as f64;
        let start_y = (offset_y + game_h - dy * scale) as f64;

        // End anchor: under the "Lives:" label, wherever the HUD is sitting
        // this frame.
        let (end_x, end_y) = match layout {
            HudLayout::TopStrip => (50.0_f64, h - HUD_HEIGHT * 0.5),
            // Both left-panel layouts put "Lives:" at the same offsets from
            // the left edge, so they share an anchor.
            HudLayout::LeftStrip | HudLayout::SideColumns => (HUD_WIDTH * 0.5, h - 24.0),
        };

        // Eased upward-floating motion — quadratic ease-out lifts the
        // indicator quickly, then settles into the HUD slot.
        let t_raw = (world.phase_elapsed / LIFE_LOST_DURATION).clamp(0.0, 1.0);
        let t = 1.0 - (1.0 - t_raw).powf(2.0);
        let cx = start_x + (end_x - start_x) * t as f64;
        let cy = start_y + (end_y - start_y) * t as f64;

        // Fade in at the start, out at the end.
        let alpha: f32 = if t_raw < 0.15 {
            t_raw / 0.15
        } else if t_raw > 0.85 {
            (1.0 - t_raw) / 0.15
        } else {
            1.0
        }
        .clamp(0.0, 1.0);

        // Heart-ish red disc with a thin glow.
        ctx.set_fill_color(Color::rgba(1.0, 0.32, 0.36, alpha));
        ctx.begin_path();
        ctx.circle(cx, cy, HEART_RADIUS);
        ctx.fill();

        ctx.set_stroke_color(Color::rgba(1.0, 0.78, 0.82, 0.85 * alpha));
        ctx.set_line_width(1.5);
        ctx.begin_path();
        ctx.circle(cx, cy, HEART_RADIUS + 1.2);
        ctx.stroke();

        // "−1" label centered on the disc.
        ctx.set_font(self.font.clone());
        ctx.set_font_size(FONT_SIZE);
        ctx.set_fill_color(Color::rgba(1.0, 1.0, 1.0, alpha));
        let label = "-1";
        if let Some(m) = ctx.measure_text(label) {
            let tx = cx - m.width * 0.5;
            let ty = cy + FONT_SIZE * 0.32;
            ctx.fill_text(label, tx, ty);
        }
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
