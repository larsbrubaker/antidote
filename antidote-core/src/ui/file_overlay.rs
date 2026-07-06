//! `FileOverlay` — the "SAVE DATA" panel (design screen 08), reached from
//! the main menu's FILE chip.
//!
//! Two option cards signal the platform shell to drive the actual file IO:
//!
//! - **EXPORT SAVE…** sets `model.pending_export = true`. The wasm shell
//!   drains the flag each frame, calls `model.export_settings_json()`, and
//!   offers the result as an `antidote-save.json` download.
//! - **IMPORT SAVE…** sets `model.pending_import = true`. The wasm shell
//!   drains the flag, opens a file picker, and feeds the selected JSON into
//!   `model.apply_settings_json`.

use agg_gui::geometry::Size;
use agg_gui::{Color, DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::theme::{self, Fonts};
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::paint_util::{fill_text_centered, fill_text_tracked};
use crate::ui::petri_kit::{
    paint_menu_backdrop, paint_panel, swallow_mouse, ButtonKind, ButtonSet, KitButton,
};

const PANEL_W: f64 = 540.0;
const PANEL_H: f64 = 460.0;
const CARD_H: f64 = 84.0;

pub struct FileOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

impl FileOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }

    fn panel(&self) -> Rect {
        Rect::new(
            (self.bounds.width - PANEL_W) * 0.5,
            (self.bounds.height - PANEL_H) * 0.5,
            PANEL_W,
            PANEL_H,
        )
    }
}

impl Widget for FileOverlay {
    fn type_name(&self) -> &'static str {
        "FileOverlay"
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
        let m = self.model.borrow();
        m.world.phase == Phase::Start && m.menu_view == MenuView::File
    }
    fn layout(&mut self, available: Size) -> Size {
        let panel = Rect::new(
            (available.width - PANEL_W) * 0.5,
            (available.height - PANEL_H) * 0.5,
            PANEL_W,
            PANEL_H,
        );
        let card_x = panel.x + 48.0;
        let card_w = PANEL_W - 96.0;
        self.buttons.clear();
        // Cards behave as big secondary buttons; custom icon/text painted
        // over them in `paint`.
        self.buttons.push(KitButton {
            id: "export",
            rect: Rect::new(card_x, panel.y + PANEL_H - 118.0 - CARD_H, card_w, CARD_H),
            label: String::new(),
            kind: ButtonKind::Secondary,
            font_size: 22.0,
        });
        self.buttons.push(KitButton {
            id: "import",
            rect: Rect::new(
                card_x,
                panel.y + PANEL_H - 118.0 - CARD_H - 18.0 - CARD_H,
                card_w,
                CARD_H,
            ),
            label: String::new(),
            kind: ButtonKind::Secondary,
            font_size: 22.0,
        });
        self.buttons.push(KitButton {
            id: "back",
            rect: Rect::new(
                panel.x + (PANEL_W - 200.0) * 0.5,
                panel.y + 32.0,
                200.0,
                56.0,
            ),
            label: "BACK".into(),
            kind: ButtonKind::Secondary,
            font_size: 19.0,
        });
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;
        paint_menu_backdrop(ctx, w, h);
        ctx.set_fill_color(Color::from_rgb8(10, 7, 20).with_alpha(0.6));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        let panel = self.panel();
        paint_panel(ctx, panel);

        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(40.0);
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_tracked(ctx, "SAVE DATA", panel.x + 48.0, panel.y + PANEL_H - 78.0, 0.0);

        self.buttons.paint(ctx, &self.fonts);

        // Card contents on top of the two blank secondary buttons.
        for (i, (title, sub, up)) in [
            ("EXPORT SAVE\u{2026}", "copy your progress to a file", true),
            ("IMPORT SAVE\u{2026}", "load progress from a file", false),
        ]
        .iter()
        .enumerate()
        {
            let r = self.buttons.buttons[i].rect;
            let icon_cx = r.x + 22.0 + 22.0;
            let icon_cy = r.y + r.height * 0.5;
            ctx.set_stroke_color(theme::LIME_500);
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.circle(icon_cx, icon_cy, 22.0);
            ctx.stroke();
            // Arrow glyph (Y-up: export points up, import down).
            let dir = if *up { 1.0 } else { -1.0 };
            ctx.set_line_width(3.0);
            ctx.begin_path();
            ctx.move_to(icon_cx, icon_cy - dir * 9.0);
            ctx.line_to(icon_cx, icon_cy + dir * 9.0);
            ctx.move_to(icon_cx - 6.0, icon_cy + dir * 3.0);
            ctx.line_to(icon_cx, icon_cy + dir * 9.0);
            ctx.line_to(icon_cx + 6.0, icon_cy + dir * 3.0);
            ctx.stroke();

            let text_x = r.x + 84.0;
            ctx.set_font(self.fonts.extrabold.clone());
            ctx.set_font_size(22.0);
            ctx.set_fill_color(theme::TEXT_HI);
            fill_text_tracked(ctx, title, text_x, icon_cy + 4.0, 1.0);
            ctx.set_font(self.fonts.semibold.clone());
            ctx.set_font_size(15.0);
            ctx.set_fill_color(theme::TEXT_MID);
            fill_text_tracked(ctx, sub, text_x, icon_cy - 20.0, 0.0);
        }

        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(15.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_centered(
            ctx,
            "saves live on this device only — no accounts, no cloud",
            panel.x + PANEL_W * 0.5,
            panel.y + 108.0,
            0.0,
        );
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some(id) = self.buttons.on_event(event) {
            match id {
                "export" => self.model.borrow_mut().pending_export = true,
                "import" => self.model.borrow_mut().pending_import = true,
                "back" => self.model.borrow_mut().menu_view = MenuView::Main,
                _ => {}
            }
            return EventResult::Consumed;
        }
        swallow_mouse(event)
    }
    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
