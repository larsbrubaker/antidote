//! `HelpOverlay` — reached from the start-screen menu bar's "Help" button.
//!
//! Static About panel: title + a few lines describing how to play and a
//! pointer to the repo. Back button returns to [`MenuView::Main`].

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::menu_widget::{
    body_label, header_label, layout_centered_column, paint_backdrop, secondary_button,
};

const REPO_URL: &str = "github.com/larsbrubaker/antidote";

pub struct HelpOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl HelpOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let back_model = model.clone();
        let body_color = Some(Color::rgba(0.75, 0.82, 0.95, 1.0));
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("About", font.clone(), 32.0),
            body_label(
                "Antidote is a bubble-trap puzzle game. Hold the pointer to grow an antidote bubble; trap viruses for 3 seconds to neutralize them.",
                font.clone(),
                body_color,
            ),
            body_label(
                "Lose a life when a virus pops your growing bubble; clear all viruses to advance to the next level.",
                font.clone(),
                body_color,
            ),
            body_label(REPO_URL, font.clone(), body_color),
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

impl Widget for HelpOverlay {
    fn type_name(&self) -> &'static str {
        "HelpOverlay"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::Help
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
