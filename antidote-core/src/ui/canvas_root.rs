//! `CanvasRoot` — hosts the fixed 1280×720 design canvas.
//!
//! The Petri Pop design is authored at one virtual resolution
//! ([`theme::APP_W`] × [`theme::APP_H`]) and scaled uniformly to the window.
//! Two pieces cooperate:
//!
//! - Platform shells call [`fixed_canvas_ux_scale`] on every resize and pass
//!   the result to `agg_gui::ux_scale::set_ux_scale`. That makes the
//!   framework's logical viewport exactly fit the canvas along one axis, so
//!   every widget lays out, paints, and hit-tests in design units with no
//!   per-widget scaling code.
//! - `CanvasRoot` centers its single child (the app's `OverlayStack`) at the
//!   fixed canvas size and paints the residual letterbox bars in the canvas
//!   background color, so off-canvas slivers never show stale pixels.
//!
//! Hit-testing outside the canvas falls to `CanvasRoot` itself, which
//! ignores events.

use agg_gui::geometry::Size;
use agg_gui::{Color, DrawCtx, Event, EventResult, Rect, Widget};

use crate::theme::{APP_H, APP_W, INK_900};

/// UX-scale value that makes the logical viewport letterbox-fit the fixed
/// canvas. `phys_w`/`phys_h` are physical pixels (what the shell's window /
/// canvas reports). Returns a value for `agg_gui::ux_scale::set_ux_scale`.
pub fn fixed_canvas_ux_scale(phys_w: f64, phys_h: f64) -> f64 {
    let fit = (phys_w / APP_W).min(phys_h / APP_H);
    // Guard startup races where the viewport briefly reports zero.
    let fit = if fit.is_finite() && fit > 0.0 {
        fit
    } else {
        1.0
    };
    fit / agg_gui::device_scale::device_scale().max(1e-6)
}

pub struct CanvasRoot {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
}

impl CanvasRoot {
    /// `canvas` is the 1280×720 widget tree (the app's overlay stack).
    pub fn new(canvas: Box<dyn Widget>) -> Self {
        Self {
            bounds: Rect::default(),
            children: vec![canvas],
        }
    }
}

impl Widget for CanvasRoot {
    fn type_name(&self) -> &'static str {
        "CanvasRoot"
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
        // With the shell driving ux_scale, `available` equals the canvas
        // size along one axis and exceeds it along the other; the canvas is
        // centered in the slack. Any drift (first frame, tests laying out at
        // arbitrary sizes) still yields a centered canvas.
        let x = (available.width - APP_W) * 0.5;
        let y = (available.height - APP_H) * 0.5;
        if let Some(canvas) = self.children.first_mut() {
            canvas.set_bounds(Rect::new(x, y, APP_W, APP_H));
            canvas.layout(Size::new(APP_W, APP_H));
        }
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        // Letterbox bars around the canvas.
        ctx.set_fill_color(Color::rgba(INK_900.r, INK_900.g, INK_900.b, 1.0));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, self.bounds.width, self.bounds.height);
        ctx.fill();
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        self.children.iter().any(|c| c.needs_draw())
    }
}
