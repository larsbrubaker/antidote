//! "Sign in with Google" button — same dark color theme + size as the
//! other menu secondary buttons, with Google's multicolor "G" SVG mark
//! painted on top of the leading edge and "Sign in with Google" text
//! centered alongside it.
//!
//! Implemented as a custom widget rather than a themed [`Button`] because
//! Button's child slot is reserved for a single Label — it can't host the
//! SVG icon next to the label text. The widget mirrors Button's hover /
//! pressed state machine so it feels identical to interact with.
//!
//! Visually: identical background / hover / pressed colors to
//! [`crate::ui::menu_widget::secondary_button`] — the only difference is
//! the colored "G" leading the label. Google's brand guidelines allow
//! this dark-button variant alongside the all-white version.
//!
//! The SVG embedded below is Google's official 4-path "G" logo (viewBox
//! `0 0 48 48`) — same asset Google's own JS sign-in button uses. The
//! widget just renders it through `agg_gui::svg::render_svg_at_size` each
//! paint; usvg's parse step is fast enough at ~140 bytes of path data
//! that we don't bother caching the parse tree.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::{Point, Size};
use agg_gui::text::Font;
use agg_gui::{DrawCtx, Event, EventResult, MouseButton, Rect, Widget};

/// Google's official "Google G" logo — 4 paths, full color, viewBox 48×48.
/// Mirrors the asset Google publishes in its branding guidelines.
// Two `#`s on each side because the SVG attribute syntax `fill="#RRGGBB"`
// contains `"#`, which would terminate a single-hash raw string.
const GOOGLE_G_SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48">
<path fill="#4285F4" d="M45.12 24.5c0-1.56-.14-3.06-.4-4.5H24v8.51h11.84c-.51 2.75-2.06 5.08-4.39 6.64v5.52h7.11c4.16-3.83 6.56-9.47 6.56-16.17z"/>
<path fill="#34A853" d="M24 46c5.94 0 10.92-1.97 14.56-5.33l-7.11-5.52c-1.97 1.32-4.49 2.1-7.45 2.1-5.73 0-10.58-3.87-12.31-9.07H4.34v5.7C7.96 41.07 15.4 46 24 46z"/>
<path fill="#FBBC05" d="M11.69 28.18C11.25 26.86 11 25.45 11 24s.25-2.86.69-4.18v-5.7H4.34C2.85 17.09 2 20.45 2 24c0 3.55.85 6.91 2.34 9.88l7.35-5.7z"/>
<path fill="#EA4335" d="M24 10.75c3.23 0 6.13 1.11 8.41 3.29l6.31-6.31C34.91 4.18 29.93 2 24 2 15.4 2 7.96 6.93 4.34 14.12l7.35 5.7c1.73-5.2 6.58-9.07 12.31-9.07z"/>
</svg>"##;

/// White label, matching the other dark secondary buttons.
const LABEL_COLOR: Color = Color::rgb(1.0, 1.0, 1.0);

const ICON_SIZE: f64 = 18.0;
const ICON_TEXT_GAP: f64 = 10.0;
/// Match `menu_widget::secondary_button`'s `with_font_size(16.0)`.
const FONT_SIZE: f64 = 16.0;
const BORDER_RADIUS: f64 = 6.0;
/// Match `menu_widget::secondary_button`'s `with_min_size(.., 38.0)`.
const BUTTON_HEIGHT: f64 = 38.0;

pub struct GoogleSignInButton {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    font: Arc<Font>,
    on_click: Option<Box<dyn FnMut()>>,
    hovered: bool,
    pressed: bool,
}

impl GoogleSignInButton {
    pub fn new(font: Arc<Font>, on_click: impl FnMut() + 'static) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            font,
            on_click: Some(Box::new(on_click)),
            hovered: false,
            pressed: false,
        }
    }

    /// Background color for the current hover/pressed state. Matches the
    /// `secondary_button` theme in `menu_widget.rs` byte-for-byte so this
    /// button reads as a peer of "Create account" / "Back" rather than a
    /// stylistic outlier.
    fn background_color(&self) -> Color {
        if self.pressed {
            Color::rgba(0.14, 0.17, 0.24, 1.0)
        } else if self.hovered {
            Color::rgba(0.24, 0.28, 0.38, 1.0)
        } else {
            Color::rgba(0.18, 0.22, 0.30, 1.0)
        }
    }

    fn fire_click(&mut self) {
        if let Some(cb) = self.on_click.as_mut() {
            cb();
        }
    }
}

impl Widget for GoogleSignInButton {
    fn type_name(&self) -> &'static str {
        "GoogleSignInButton"
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
        // Same height as the rest of the menu's secondary buttons.
        Size::new(available.width, BUTTON_HEIGHT)
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;
        if w <= 0.0 || h <= 0.0 {
            return;
        }

        // Background — rounded pill matching the secondary buttons' theme.
        ctx.set_fill_color(self.background_color());
        ctx.begin_path();
        ctx.rounded_rect(0.0, 0.0, w, h, BORDER_RADIUS);
        ctx.fill();

        // Layout: icon + gap + text, the whole group horizontally centered.
        ctx.set_font(self.font.clone());
        ctx.set_font_size(FONT_SIZE);
        let label = "Sign in with Google";
        let text_width = ctx.measure_text(label).map(|m| m.width).unwrap_or(120.0);
        let group_w = ICON_SIZE + ICON_TEXT_GAP + text_width;
        let group_x = ((w - group_w) * 0.5).max(0.0);
        let icon_y = (h - ICON_SIZE) * 0.5;

        // Icon — Google G, rendered fresh each paint. Tiny SVG, cheap.
        ctx.save();
        ctx.translate(group_x, icon_y);
        let _ =
            agg_gui::svg::render_svg_at_size(GOOGLE_G_SVG, ctx, ICON_SIZE as u32, ICON_SIZE as u32);
        ctx.restore();

        // Label — vertically center the text on the button rect.
        ctx.set_fill_color(LABEL_COLOR);
        let text_x = group_x + ICON_SIZE + ICON_TEXT_GAP;
        // Approximate baseline: 0.7 of font size below the visual middle.
        let text_y = h * 0.5 + FONT_SIZE * 0.35;
        ctx.fill_text(label, text_x, text_y);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseMove { pos } => {
                let was_hovered = self.hovered;
                self.hovered = self.hit_test(*pos);
                if !self.hovered {
                    self.pressed = false;
                }
                if was_hovered != self.hovered {
                    agg_gui::animation::request_draw();
                    return EventResult::Consumed;
                }
                EventResult::Ignored
            }
            Event::MouseDown {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                agg_gui::animation::request_draw();
                EventResult::Consumed
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                let was_pressed = self.pressed;
                self.pressed = false;
                if was_pressed && self.hovered {
                    self.fire_click();
                    agg_gui::animation::request_draw();
                }
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn is_focusable(&self) -> bool {
        true
    }

    fn hit_test(&self, local_pos: Point) -> bool {
        local_pos.x >= 0.0
            && local_pos.x <= self.bounds.width
            && local_pos.y >= 0.0
            && local_pos.y <= self.bounds.height
    }
}
