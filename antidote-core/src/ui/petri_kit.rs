//! Immediate-mode UI kit for the Petri Pop overlays.
//!
//! The redesigned menus are custom-painted panels, not stock agg-gui
//! widgets: buttons are rounded rects with solid hard-shadow edges, labels
//! are tracked uppercase, and text renders through the immediate path so it
//! rasterizes at the physical pixel grid. Overlays own a [`ButtonSet`],
//! rebuild its rects during `layout`, paint it during `paint`, and translate
//! clicks back into model mutations via the button ids returned from
//! [`ButtonSet::on_event`].

use agg_gui::{Color, DrawCtx, Event, EventResult, MouseButton, Point, Rect};

use crate::theme::{self, Fonts};
use crate::ui::paint_util::{fill_text_centered, measure_tracked, raised_rect};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    /// Lime fill, ink text — the one action we want pressed.
    Primary,
    /// Ink fill, hairline border — alternate actions.
    Secondary,
    /// Text only — de-emphasized escape hatches ("BACK TO MENU").
    Ghost,
    /// Pill chip with a small leading icon — the main menu's HELP/FILE/
    /// FULLSCREEN row.
    Chip(ChipIcon),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ChipIcon {
    Question,
    Save,
    Expand,
}

pub struct KitButton {
    pub id: &'static str,
    /// Local Y-up rect.
    pub rect: Rect,
    pub label: String,
    pub kind: ButtonKind,
    pub font_size: f64,
}

/// A group of immediate-mode buttons with shared hover/press tracking.
#[derive(Default)]
pub struct ButtonSet {
    pub buttons: Vec<KitButton>,
    hovered: Option<usize>,
    pressed: Option<usize>,
}

impl ButtonSet {
    pub fn clear(&mut self) {
        self.buttons.clear();
        self.hovered = None;
        self.pressed = None;
    }

    pub fn push(&mut self, b: KitButton) {
        self.buttons.push(b);
    }

    fn index_at(&self, p: Point) -> Option<usize> {
        self.buttons.iter().position(|b| {
            p.x >= b.rect.x
                && p.x <= b.rect.x + b.rect.width
                && p.y >= b.rect.y
                && p.y <= b.rect.y + b.rect.height
        })
    }

    /// Feed a widget-local event. Returns the id of a button that was
    /// clicked (press + release on the same button), if any.
    pub fn on_event(&mut self, event: &Event) -> Option<&'static str> {
        match event {
            Event::MouseMove { pos } => {
                self.hovered = self.index_at(*pos);
                None
            }
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = self.index_at(*pos);
                None
            }
            Event::MouseUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let was = self.pressed.take();
                let up = self.index_at(*pos);
                match (was, up) {
                    (Some(a), Some(b)) if a == b => Some(self.buttons[a].id),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    pub fn paint(&self, ctx: &mut dyn DrawCtx, fonts: &Fonts) {
        for (i, b) in self.buttons.iter().enumerate() {
            paint_kit_button(
                ctx,
                fonts,
                b,
                self.hovered == Some(i),
                self.pressed == Some(i),
            );
        }
    }
}

/// A consumed-mouse / bubbled-keys event policy shared by every overlay:
/// menus swallow pointer input so the playfield underneath doesn't react,
/// while keyboard events bubble to the global Esc/P pause handler.
pub fn swallow_mouse(event: &Event) -> EventResult {
    match event {
        Event::MouseMove { .. }
        | Event::MouseDown { .. }
        | Event::MouseUp { .. }
        | Event::MouseWheel { .. } => EventResult::Consumed,
        _ => EventResult::Ignored,
    }
}

fn paint_kit_button(ctx: &mut dyn DrawCtx, fonts: &Fonts, b: &KitButton, hover: bool, press: bool) {
    let mut r = b.rect;
    match b.kind {
        ButtonKind::Primary => {
            let mut drop = theme::SHADOW_DROP_LG;
            let mut fill = theme::LIME_500;
            if press {
                r.y -= theme::SHADOW_DROP_LG - 2.0;
                drop = 2.0;
                fill = theme::LIME_600;
            } else if hover {
                r.y += 1.0;
                drop += 1.0;
                fill = theme::LIME_400;
            }
            raised_rect(
                ctx,
                r,
                theme::RADIUS_BUTTON,
                drop,
                fill,
                theme::LIME_700,
                None,
            );
            ctx.set_font(fonts.extrabold.clone());
            ctx.set_font_size(b.font_size);
            ctx.set_fill_color(theme::INK_900);
            fill_text_centered(
                ctx,
                &b.label,
                r.x + r.width * 0.5,
                r.y + (r.height - b.font_size * 0.72) * 0.5,
                2.0,
            );
        }
        ButtonKind::Secondary => {
            let mut drop = theme::SHADOW_DROP_LG;
            let mut fill = theme::INK_600;
            if press {
                r.y -= theme::SHADOW_DROP_LG - 2.0;
                drop = 2.0;
            } else if hover {
                fill = Color::from_rgb8(48, 37, 80);
            }
            raised_rect(
                ctx,
                r,
                theme::RADIUS_BUTTON,
                drop,
                fill,
                theme::EDGE_950,
                Some(theme::HAIRLINE),
            );
            ctx.set_font(fonts.extrabold.clone());
            ctx.set_font_size(b.font_size);
            ctx.set_fill_color(theme::TEXT_HI);
            fill_text_centered(
                ctx,
                &b.label,
                r.x + r.width * 0.5,
                r.y + (r.height - b.font_size * 0.72) * 0.5,
                1.5,
            );
        }
        ButtonKind::Ghost => {
            ctx.set_font(fonts.extrabold.clone());
            ctx.set_font_size(b.font_size);
            ctx.set_fill_color(if hover || press {
                theme::TEXT_HI
            } else {
                theme::TEXT_MID
            });
            fill_text_centered(
                ctx,
                &b.label,
                r.x + r.width * 0.5,
                r.y + (r.height - b.font_size * 0.72) * 0.5,
                1.5,
            );
        }
        ButtonKind::Chip(icon) => {
            let mut drop = theme::SHADOW_DROP;
            let mut fill = theme::INK_600;
            if press {
                r.y -= theme::SHADOW_DROP - 1.0;
                drop = 1.0;
            } else if hover {
                fill = Color::from_rgb8(48, 37, 80);
            }
            raised_rect(
                ctx,
                r,
                theme::RADIUS_CHIP,
                drop,
                fill,
                theme::EDGE_950,
                Some(theme::HAIRLINE),
            );
            let cy = r.y + r.height * 0.5;
            let icon_cx = r.x + 24.0;
            paint_chip_icon(ctx, icon, icon_cx, cy);
            ctx.set_font(fonts.bold.clone());
            ctx.set_font_size(b.font_size);
            ctx.set_fill_color(theme::TEXT_HI);
            let text_x = r.x + 42.0;
            crate::ui::paint_util::fill_text_tracked(
                ctx,
                &b.label,
                text_x,
                cy - b.font_size * 0.36,
                1.0,
            );
        }
    }
}

/// Width a chip needs for its icon + tracked label at `font_size`.
pub fn chip_width(ctx: &mut dyn DrawCtx, fonts: &Fonts, label: &str, font_size: f64) -> f64 {
    ctx.set_font(fonts.bold.clone());
    ctx.set_font_size(font_size);
    42.0 + measure_tracked(ctx, label, 1.0) + 22.0
}

fn paint_chip_icon(ctx: &mut dyn DrawCtx, icon: ChipIcon, cx: f64, cy: f64) {
    let lime = theme::LIME_500;
    match icon {
        ChipIcon::Question => {
            ctx.set_stroke_color(lime);
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.circle(cx, cy, 11.0);
            ctx.stroke();
            // Painted glyph rather than text so it stays centered in the ring.
            ctx.set_fill_color(lime);
            ctx.begin_path();
            ctx.circle(cx, cy - 5.5, 1.6);
            ctx.fill();
            ctx.set_stroke_color(lime);
            ctx.begin_path();
            ctx.move_to(cx, cy - 2.0);
            ctx.line_to(cx, cy + 1.0);
            ctx.arc_to(cx, cy + 4.5, 4.0, -std::f64::consts::FRAC_PI_2, 0.6, false);
            ctx.stroke();
        }
        ChipIcon::Save => {
            ctx.set_stroke_color(lime);
            ctx.set_line_width(2.0);
            ctx.begin_path();
            ctx.rounded_rect(cx - 9.0, cy - 9.0, 18.0, 18.0, 4.0);
            ctx.stroke();
            ctx.set_fill_color(lime);
            ctx.begin_path();
            ctx.rect(cx - 5.0, cy - 7.0, 10.0, 6.0);
            ctx.fill();
        }
        ChipIcon::Expand => {
            ctx.set_stroke_color(lime);
            ctx.set_line_width(2.0);
            ctx.begin_path();
            for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let corner_x = cx + sx * 9.0;
                let corner_y = cy + sy * 9.0;
                ctx.move_to(corner_x - sx * 6.0, corner_y);
                ctx.line_to(corner_x, corner_y);
                ctx.line_to(corner_x, corner_y - sy * 6.0);
            }
            ctx.stroke();
        }
    }
}

/// Overlay panel: ink-700 rounded rect, hairline border, deep hard shadow.
pub fn paint_panel(ctx: &mut dyn DrawCtx, r: Rect) {
    raised_rect(
        ctx,
        r,
        theme::RADIUS_PANEL,
        8.0,
        theme::INK_700,
        theme::EDGE_950,
        Some(theme::HAIRLINE),
    );
}

/// Full-canvas menu backdrop: ink-900 with the 40-unit violet grid.
pub fn paint_menu_backdrop(ctx: &mut dyn DrawCtx, w: f64, h: f64) {
    ctx.set_fill_color(theme::INK_900);
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, h);
    ctx.fill();
    ctx.set_stroke_color(theme::GRID_LINE);
    ctx.set_line_width(1.0);
    ctx.begin_path();
    let mut x = 0.0;
    while x <= w + 0.0001 {
        ctx.move_to(x, 0.0);
        ctx.line_to(x, h);
        x += theme::GRID_CELL;
    }
    let mut y = 0.0;
    while y <= h + 0.0001 {
        ctx.move_to(0.0, y);
        ctx.line_to(w, y);
        y += theme::GRID_CELL;
    }
    ctx.stroke();
}

/// Scrim over the playfield panel only (rails stay visible + live). `alpha`
/// is the design scrim strength (0.45 first-run / 0.78 standard / 0.82
/// game over).
pub fn paint_playfield_scrim(ctx: &mut dyn DrawCtx, bounds: Rect, alpha: f32) {
    ctx.set_fill_color(Color::from_rgb8(10, 7, 20).with_alpha(alpha));
    ctx.begin_path();
    ctx.rect(
        theme::PLAYFIELD_X,
        0.0,
        (bounds.width - 2.0 * theme::RAIL_W).max(0.0),
        bounds.height,
    );
    ctx.fill();
}

/// A miniature virus (coral gradient + 4 spike dots), used by the logo's
/// bubble-O and the rotate prompt. `r` is the body radius.
pub fn paint_mini_virus(ctx: &mut dyn DrawCtx, cx: f64, cy: f64, r: f64) {
    ctx.set_fill_color(theme::CORAL_500);
    for (dx, dy) in [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)] {
        ctx.begin_path();
        ctx.circle(cx + dx * r, cy + dy * r, r * 0.28);
        ctx.fill();
    }
    ctx.set_fill_radial_gradient(agg_gui::draw_ctx::RadialGradientPaint::centered(
        cx - r * 0.3,
        cy + r * 0.35,
        r * 1.3,
        &[
            (0.0, theme::CORAL_300),
            (0.45, theme::CORAL_500),
            (0.95, theme::CORAL_800),
        ],
    ));
    ctx.begin_path();
    ctx.circle(cx, cy, r);
    ctx.fill();
}

/// The lime bubble ring used by the logo's O and the rotate prompt.
pub fn paint_logo_bubble(ctx: &mut dyn DrawCtx, cx: f64, cy: f64, r: f64, stroke_w: f64) {
    ctx.set_fill_radial_gradient(agg_gui::draw_ctx::RadialGradientPaint::centered(
        cx - r * 0.3,
        cy + r * 0.4,
        r * 1.2,
        &[
            (0.0, theme::LIME_500.with_alpha(0.25)),
            (0.72, theme::LIME_500.with_alpha(0.04)),
            (1.0, theme::LIME_500.with_alpha(0.0)),
        ],
    ));
    ctx.begin_path();
    ctx.circle(cx, cy, r);
    ctx.fill();
    ctx.set_stroke_color(theme::LIME_500);
    ctx.set_line_width(stroke_w);
    ctx.begin_path();
    ctx.circle(cx, cy, r);
    ctx.stroke();
    // Specular gleam upper-left.
    ctx.save();
    ctx.translate(cx - r * 0.4, cy + r * 0.45);
    ctx.rotate(0.5);
    ctx.scale(1.0, 0.5);
    ctx.set_fill_color(Color::from_rgb8(255, 255, 255).with_alpha(0.5));
    ctx.begin_path();
    ctx.circle(0.0, 0.0, r * 0.2);
    ctx.fill();
    ctx.restore();
}
