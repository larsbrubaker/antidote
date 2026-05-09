//! `OverlayStack` — z-stacks every child at full bounds.
//!
//! agg-gui's [`paint_subtree`] paints children front-to-back, and
//! [`hit_test_subtree`] iterates children back-to-front — so adding overlays
//! in declaration order is enough to get the right z-order. Each overlay
//! widget controls its own visibility through [`Widget::is_visible`], reading
//! the current [`Phase`](crate::game::state::Phase) from the shared model.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

pub struct OverlayStack {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
}

impl OverlayStack {
    pub fn new() -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn add(mut self, child: Box<dyn Widget>) -> Self {
        self.children.push(child);
        self
    }
}

impl Default for OverlayStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for OverlayStack {
    fn type_name(&self) -> &'static str {
        "OverlayStack"
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
        for child in self.children.iter_mut() {
            child.set_bounds(Rect::new(0.0, 0.0, available.width, available.height));
            child.layout(available);
        }
        available
    }

    fn paint(&mut self, _ctx: &mut dyn DrawCtx) {
        // No own content; children paint themselves on top.
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        // Game canvas requests continuous redraw via its own needs_draw;
        // bubble that up.
        self.children.iter().any(|c| c.needs_draw())
    }
}
