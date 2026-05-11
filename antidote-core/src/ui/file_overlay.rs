//! `FileOverlay` — reached from the start-screen menu bar's "File" button.
//!
//! Two buttons that signal the platform shell to drive the actual file IO:
//!
//! - **Export…** sets `model.pending_export = true`. The wasm shell drains
//!   the flag each frame, calls `model.export_settings_json()`, and offers
//!   the result as an `antidote-save.json` download. Native has no equivalent
//!   yet (no file dialog dep); the flag just stays unset on that side until
//!   we wire one in.
//! - **Import…** sets `model.pending_import = true`. The wasm shell drains
//!   the flag each frame, opens a file picker, and feeds the selected
//!   JSON into `model.apply_settings_json`.
//!
//! Back returns to [`MenuView::Main`].

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::menu_widget::{
    body_label, header_label, layout_centered_column, paint_backdrop, primary_button,
    secondary_button,
};

pub struct FileOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl FileOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let export_model = model.clone();
        let import_model = model.clone();
        let back_model = model.clone();
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("File", font.clone(), 32.0),
            body_label(
                "Save or load your game state as a JSON file.",
                font.clone(),
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            primary_button("Export\u{2026}", font.clone(), move || {
                export_model.borrow_mut().pending_export = true;
            }),
            primary_button("Import\u{2026}", font.clone(), move || {
                import_model.borrow_mut().pending_import = true;
            }),
            secondary_button("Back", font, move || {
                back_model.borrow_mut().menu_view = MenuView::Main;
            }),
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
        }
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
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
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
