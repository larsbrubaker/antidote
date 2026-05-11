//! `MenuBar` — top "File / Help" strip visible only on the start screen.
//!
//! Renders two small text buttons in a strip at the top of the canvas. Click
//! `File` to flip [`MenuView::File`], `Help` to flip [`MenuView::Help`]. The
//! respective overlays in `file_overlay.rs` / `help_overlay.rs` then take
//! over while their view is active.
//!
//! Like [`crate::ui::hud_widget::HudWidget`], the widget bounds span the full
//! canvas but only the top strip paints and only that strip hit-tests, so
//! clicks below pass through to the main-menu overlay.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::widgets::button::{Button, ButtonTheme};
use agg_gui::{DrawCtx, Event, EventResult, Point, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};

/// Height of the menu strip in logical pixels.
pub const MENU_BAR_HEIGHT: f64 = 36.0;
/// Inner padding on the left edge of the strip.
const PAD_X: f64 = 12.0;
/// Gap between adjacent menu-bar buttons.
const ITEM_GAP: f64 = 4.0;
/// Width of each top-level menu button.
const ITEM_W: f64 = 72.0;
/// Font size for the menu-bar text.
const FONT_SIZE: f64 = 15.0;

pub struct MenuBar {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl MenuBar {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let file_model = model.clone();
        let help_model = model.clone();
        let children: Vec<Box<dyn Widget>> = vec![
            menu_bar_button("File", font.clone(), move || {
                file_model.borrow_mut().menu_view = MenuView::File;
            }),
            menu_bar_button("Help", font, move || {
                help_model.borrow_mut().menu_view = MenuView::Help;
            }),
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }
}

fn menu_bar_button(
    text: &str,
    font: Arc<Font>,
    on_click: impl FnMut() + 'static,
) -> Box<dyn Widget> {
    let theme = ButtonTheme {
        background: Color::rgba(0.0, 0.0, 0.0, 0.0),
        background_hovered: Color::rgba(1.0, 1.0, 1.0, 0.08),
        background_pressed: Color::rgba(1.0, 1.0, 1.0, 0.16),
        label_color: Color::rgba(0.92, 0.95, 1.0, 1.0),
        ..ButtonTheme::default()
    };
    Box::new(
        Button::new(text, font)
            .with_font_size(FONT_SIZE)
            .with_theme(theme)
            .with_min_size(Size::new(ITEM_W, MENU_BAR_HEIGHT - 6.0))
            .on_click(on_click),
    )
}

impl Widget for MenuBar {
    fn type_name(&self) -> &'static str {
        "MenuBar"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::Main
    }
    /// Limit hit-testing to the top-bar zone (high y in agg-gui Y-up) so
    /// clicks elsewhere fall through to the main-menu overlay underneath.
    fn hit_test(&self, local_pos: Point) -> bool {
        let h = self.bounds.height;
        local_pos.y >= h - MENU_BAR_HEIGHT
            && local_pos.y <= h
            && local_pos.x >= 0.0
            && local_pos.x <= self.bounds.width
    }
    fn layout(&mut self, available: Size) -> Size {
        let h = available.height;
        let bar_y = h - MENU_BAR_HEIGHT;
        let pad_y = (MENU_BAR_HEIGHT - (MENU_BAR_HEIGHT - 6.0)) * 0.5;
        let item_h = MENU_BAR_HEIGHT - 6.0;
        let mut cursor_x = PAD_X;
        for child in self.children.iter_mut() {
            let s = child.layout(Size::new(ITEM_W, item_h));
            child.set_bounds(Rect::new(cursor_x, bar_y + pad_y, s.width, s.height));
            cursor_x += s.width + ITEM_GAP;
        }
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;
        let bar_y = h - MENU_BAR_HEIGHT;
        // Translucent dark strip so it reads as chrome over the menu backdrop.
        ctx.set_fill_color(Color::rgba(0.05, 0.07, 0.12, 0.78));
        ctx.begin_path();
        ctx.rect(0.0, bar_y, w, MENU_BAR_HEIGHT);
        ctx.fill();
        // Hairline under the strip.
        ctx.set_stroke_color(Color::rgba(1.0, 1.0, 1.0, 0.10));
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(0.0, bar_y);
        ctx.line_to(w, bar_y);
        ctx.stroke();
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        // Swallow mouse inside the strip so the main-menu overlay below
        // doesn't react to the same click; let keys bubble up.
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
