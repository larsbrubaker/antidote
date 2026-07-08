//! `HudWidget` — the two fixed rails flanking the playfield.
//!
//! Petri Pop authors the whole app at a fixed 1280×720 canvas, so the HUD is
//! one layout, always: a 120-unit rail on each side of the 1040-unit
//! playfield panel (see `docs/New Design/Antidote Frame.dc.html`).
//!
//! - **Left rail**: pause button, LEVEL number, LIVES pips, vertical
//!   ANTIDOTE pill meter with % readout.
//! - **Right rail**: fullscreen + mute buttons, SCORE, BEST (gold).
//!
//! The rail buttons are painted + hit-tested here rather than using
//! `agg_gui::widgets::Button` — they're icon buttons with hard shadows and
//! active states the stock button theme doesn't model. Only the two rail
//! rects claim pointer events; the playfield in between falls through to
//! the game canvas underneath.

use agg_gui::geometry::Size;
use agg_gui::{Color, DrawCtx, Event, EventResult, MouseButton, Point, Rect, Widget};

use crate::game::state::Phase;
use crate::theme::{self, Fonts};
use crate::ui::game_model::SharedModel;
use crate::ui::paint_util::{fill_text_centered, fmt_thousands, measure_tracked, raised_rect};

/// Vertical positions (screen-space, top-down) of the rail blocks,
/// converted to Y-up at paint time. All values in design units.
const PAD_TOP: f64 = 24.0;
const BTN: f64 = theme::RAIL_BUTTON;
const BTN_GAP: f64 = 14.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailButton {
    Pause,
    Fullscreen,
    Mute,
}

pub struct HudWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    hovered: Option<RailButton>,
    pressed: Option<RailButton>,
}

impl HudWidget {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            hovered: None,
            pressed: None,
        }
    }

    fn right_rail_x(&self) -> f64 {
        self.bounds.width - theme::RAIL_W
    }

    /// Button rects in local Y-up coordinates.
    fn button_rect(&self, b: RailButton) -> Rect {
        let h = self.bounds.height;
        let y_top = |screen_y: f64| h - screen_y - BTN;
        match b {
            RailButton::Pause => Rect::new((theme::RAIL_W - BTN) * 0.5, y_top(PAD_TOP), BTN, BTN),
            RailButton::Fullscreen => Rect::new(
                self.right_rail_x() + (theme::RAIL_W - BTN) * 0.5,
                y_top(PAD_TOP),
                BTN,
                BTN,
            ),
            RailButton::Mute => Rect::new(
                self.right_rail_x() + (theme::RAIL_W - BTN) * 0.5,
                y_top(PAD_TOP + BTN + BTN_GAP),
                BTN,
                BTN,
            ),
        }
    }

    fn button_at(&self, p: Point) -> Option<RailButton> {
        [RailButton::Pause, RailButton::Fullscreen, RailButton::Mute]
            .into_iter()
            .find(|b| contains(self.button_rect(*b), p))
    }

    fn activate(&mut self, b: RailButton) {
        let mut m = self.model.borrow_mut();
        match b {
            RailButton::Pause => match m.world.phase {
                Phase::Playing => m.world.phase = Phase::Paused,
                Phase::Paused => m.world.phase = Phase::Playing,
                _ => {}
            },
            RailButton::Fullscreen => m.pending_fullscreen_toggle = true,
            RailButton::Mute => {
                m.settings.muted = !m.settings.muted;
                m.save_settings();
            }
        }
    }
}

fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.x && p.x <= r.x + r.width && p.y >= r.y && p.y <= r.y + r.height
}

impl Widget for HudWidget {
    fn type_name(&self) -> &'static str {
        "HudWidget"
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
        available
    }

    /// Only the rails claim the pointer; the playfield falls through.
    fn hit_test(&self, local_pos: Point) -> bool {
        let in_y = local_pos.y >= 0.0 && local_pos.y <= self.bounds.height;
        in_y && (local_pos.x <= theme::RAIL_W || local_pos.x >= self.right_rail_x())
    }

    fn is_visible(&self) -> bool {
        let phase = self.model.borrow().world.phase;
        matches!(
            phase,
            Phase::Playing
                | Phase::Paused
                | Phase::LevelComplete
                | Phase::LifeLost
                | Phase::GameOver
        )
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let (level, lives, score, best, antidote, paused, muted) = {
            let m = self.model.borrow();
            (
                m.world.level,
                m.world.lives,
                m.world.total_score,
                m.settings.best_score.max(m.world.total_score),
                m.world.antidote.clamp(0.0, 1.0),
                m.world.phase == Phase::Paused,
                m.settings.muted,
            )
        };
        let h = self.bounds.height;
        let right_x = self.right_rail_x();

        // Rail panels + inner-edge hairlines framing the playfield.
        ctx.set_fill_color(theme::INK_700);
        ctx.begin_path();
        ctx.rect(0.0, 0.0, theme::RAIL_W, h);
        ctx.rect(right_x, 0.0, theme::RAIL_W, h);
        ctx.fill();
        ctx.set_stroke_color(theme::HAIRLINE);
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(theme::RAIL_W - 0.5, 0.0);
        ctx.line_to(theme::RAIL_W - 0.5, h);
        ctx.move_to(right_x + 0.5, 0.0);
        ctx.line_to(right_x + 0.5, h);
        ctx.stroke();

        // --- Buttons -----------------------------------------------------
        self.paint_button(ctx, RailButton::Pause, paused, |ctx, r| {
            paint_pause_icon(ctx, r)
        });
        self.paint_button(ctx, RailButton::Fullscreen, false, |ctx, r| {
            paint_fullscreen_icon(ctx, r)
        });
        self.paint_button(ctx, RailButton::Mute, muted, |ctx, r| {
            paint_speaker_icon(ctx, r, muted)
        });

        // --- Left rail blocks (screen-space y, converted per call) -------
        let cx_l = theme::RAIL_W * 0.5;
        self.label(ctx, "LEVEL", cx_l, PAD_TOP + BTN + 22.0, theme::TEXT_LOW);
        self.big_number(ctx, &level.to_string(), cx_l, PAD_TOP + BTN + 64.0, 40.0);

        self.label(ctx, "LIVES", cx_l, PAD_TOP + BTN + 100.0, theme::TEXT_LOW);
        paint_lives_pips(ctx, cx_l, h - (PAD_TOP + BTN + 122.0), lives);

        // Antidote block fills the remaining rail height.
        let meter_label_y = PAD_TOP + BTN + 156.0;
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(13.0);
        ctx.set_fill_color(theme::LIME_500);
        fill_text_centered(ctx, "ANTIDOTE", cx_l, h - meter_label_y, 2.0);

        let pct_baseline_screen = h - 24.0; // 24 up from the bottom edge
        let meter_top_screen = meter_label_y + 14.0;
        let meter_bottom_screen = pct_baseline_screen - 26.0;
        let meter_h = (meter_bottom_screen - meter_top_screen).max(0.0);
        let meter = Rect::new(cx_l - 15.0, h - meter_bottom_screen, 30.0, meter_h);
        paint_meter(ctx, meter, antidote);

        ctx.set_font(self.fonts.extrabold.clone());
        ctx.set_font_size(18.0);
        ctx.set_fill_color(theme::meter_color(antidote));
        let pct = format!("{}%", (antidote * 100.0).round() as u32);
        fill_text_centered(ctx, &pct, cx_l, h - pct_baseline_screen, 0.0);

        // --- Right rail blocks -------------------------------------------
        let cx_r = right_x + theme::RAIL_W * 0.5;
        let score_label_y = PAD_TOP + 2.0 * BTN + BTN_GAP + 30.0;
        self.label(ctx, "SCORE", cx_r, score_label_y, theme::TEXT_LOW);
        self.big_number(ctx, &fmt_thousands(score), cx_r, score_label_y + 34.0, 30.0);

        let best_label_y = score_label_y + 70.0;
        self.label(ctx, "BEST", cx_r, best_label_y, theme::TEXT_LOW);
        ctx.set_fill_color(theme::GOLD_400);
        self.number(ctx, &fmt_thousands(best), cx_r, best_label_y + 26.0, 20.0);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseMove { pos } => {
                let over = self.button_at(*pos);
                if over != self.hovered {
                    self.hovered = over;
                }
                EventResult::Ignored
            }
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(b) = self.button_at(*pos) {
                    self.pressed = Some(b);
                    return EventResult::Consumed;
                }
                // Swallow rail clicks so they never reach the playfield.
                EventResult::Consumed
            }
            Event::MouseUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let was = self.pressed.take();
                if let (Some(down), Some(up)) = (was, self.button_at(*pos)) {
                    if down == up {
                        self.activate(up);
                    }
                }
                EventResult::Consumed
            }
            _ => EventResult::Ignored,
        }
    }

    fn needs_draw(&self) -> bool {
        // Live HUD numbers — keep redrawing while visible.
        true
    }
}

impl HudWidget {
    /// Uppercase tracked label centered on `cx`, `screen_y` from the top.
    fn label(&self, ctx: &mut dyn DrawCtx, text: &str, cx: f64, screen_y: f64, color: Color) {
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(theme::FS_LABEL);
        ctx.set_fill_color(color);
        fill_text_centered(ctx, text, cx, self.bounds.height - screen_y, 2.5);
    }

    /// Big extrabold number centered on `cx`, shrinking to fit the rail.
    fn big_number(&self, ctx: &mut dyn DrawCtx, text: &str, cx: f64, screen_y: f64, size: f64) {
        ctx.set_fill_color(theme::TEXT_HI);
        self.number(ctx, text, cx, screen_y, size);
    }

    fn number(&self, ctx: &mut dyn DrawCtx, text: &str, cx: f64, screen_y: f64, size: f64) {
        ctx.set_font(self.fonts.extrabold.clone());
        let max_w = theme::RAIL_W - 12.0;
        let mut size = size;
        ctx.set_font_size(size);
        let w = measure_tracked(ctx, text, 0.0);
        if w > max_w {
            size *= max_w / w;
            ctx.set_font_size(size);
        }
        fill_text_centered(ctx, text, cx, self.bounds.height - screen_y, 0.0);
    }

    fn paint_button(
        &self,
        ctx: &mut dyn DrawCtx,
        b: RailButton,
        active: bool,
        icon: impl Fn(&mut dyn DrawCtx, Rect),
    ) {
        let mut r = self.button_rect(b);
        let mut drop = theme::SHADOW_DROP;
        let mut fill = theme::INK_600;
        if self.pressed == Some(b) {
            r.y -= theme::SHADOW_DROP - 1.0; // sink into the shadow
            drop = 1.0;
        } else if self.hovered == Some(b) {
            fill = Color::from_rgb8(48, 37, 80);
        }
        let border = if active {
            Color::rgba(theme::LIME_500.r, theme::LIME_500.g, theme::LIME_500.b, 0.9)
        } else {
            theme::HAIRLINE
        };
        raised_rect(
            ctx,
            r,
            theme::RADIUS_BUTTON,
            drop,
            fill,
            theme::EDGE_950,
            Some(border),
        );
        icon(ctx, r);
    }
}

fn paint_lives_pips(ctx: &mut dyn DrawCtx, cx: f64, y_up_center: f64, lives: u8) {
    const PIP_D: f64 = 18.0;
    const GAP: f64 = 8.0;
    let total = crate::consts::BASE_LIVES as f64;
    let span = total * PIP_D + (total - 1.0) * GAP;
    for i in 0..crate::consts::BASE_LIVES {
        let x = cx - span * 0.5 + i as f64 * (PIP_D + GAP) + PIP_D * 0.5;
        ctx.begin_path();
        ctx.circle(x, y_up_center, PIP_D * 0.5);
        if i < lives {
            ctx.set_fill_color(theme::LIME_500);
            ctx.fill();
        } else {
            ctx.set_stroke_color(Color::rgba(
                theme::TEXT_LOW.r,
                theme::TEXT_LOW.g,
                theme::TEXT_LOW.b,
                0.7,
            ));
            ctx.set_line_width(2.0);
            ctx.stroke();
        }
    }
}

/// Vertical pill meter: ink track, hairline border, bottom-up fill in the
/// threshold color, quarter tick lines.
fn paint_meter(ctx: &mut dyn DrawCtx, r: Rect, t: f32) {
    let radius = r.width * 0.5;
    ctx.set_fill_color(theme::INK_600);
    ctx.begin_path();
    ctx.rounded_rect(r.x, r.y, r.width, r.height, radius);
    ctx.fill();
    ctx.set_stroke_color(theme::HAIRLINE);
    ctx.set_line_width(2.0);
    ctx.begin_path();
    ctx.rounded_rect(
        r.x + 1.0,
        r.y + 1.0,
        r.width - 2.0,
        r.height - 2.0,
        radius - 1.0,
    );
    ctx.stroke();

    let fill_h = r.height * t as f64;
    if fill_h > 1.0 {
        ctx.set_fill_color(theme::meter_color(t));
        ctx.begin_path();
        // Pill-cap the fill; short fills just render as a stubby pill.
        ctx.rounded_rect(
            r.x + 2.0,
            r.y + 2.0,
            r.width - 4.0,
            (fill_h - 4.0).max(2.0),
            radius - 2.0,
        );
        ctx.fill();
    }

    ctx.set_stroke_color(Color::rgba(
        theme::INK_900.r,
        theme::INK_900.g,
        theme::INK_900.b,
        0.5,
    ));
    ctx.set_line_width(1.0);
    ctx.begin_path();
    for q in [0.25, 0.5, 0.75] {
        let y = r.y + r.height * q;
        ctx.move_to(r.x + 2.0, y);
        ctx.line_to(r.x + r.width - 2.0, y);
    }
    ctx.stroke();
}

fn paint_pause_icon(ctx: &mut dyn DrawCtx, r: Rect) {
    let cx = r.x + r.width * 0.5;
    let cy = r.y + r.height * 0.5;
    ctx.set_fill_color(theme::TEXT_HI);
    ctx.begin_path();
    ctx.rounded_rect(cx - 10.0, cy - 11.0, 7.0, 22.0, 3.0);
    ctx.rounded_rect(cx + 3.0, cy - 11.0, 7.0, 22.0, 3.0);
    ctx.fill();
}

/// Four corner brackets — the classic expand glyph.
fn paint_fullscreen_icon(ctx: &mut dyn DrawCtx, r: Rect) {
    let cx = r.x + r.width * 0.5;
    let cy = r.y + r.height * 0.5;
    let half = 12.0;
    let arm = 8.0;
    ctx.set_stroke_color(theme::TEXT_HI);
    ctx.set_line_width(3.0);
    ctx.begin_path();
    for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
        let corner_x = cx + sx * half;
        let corner_y = cy + sy * half;
        ctx.move_to(corner_x - sx * arm, corner_y);
        ctx.line_to(corner_x, corner_y);
        ctx.line_to(corner_x, corner_y - sy * arm);
    }
    ctx.stroke();
}

fn paint_speaker_icon(ctx: &mut dyn DrawCtx, r: Rect, muted: bool) {
    let cx = r.x + r.width * 0.5;
    let cy = r.y + r.height * 0.5;
    ctx.set_fill_color(theme::TEXT_HI);
    // Speaker body: box + cone, mouth at x = cx - 3 opening right. Shifted
    // 2px left of center so body + waves read centered as a unit.
    ctx.begin_path();
    ctx.move_to(cx - 14.0, cy - 4.0);
    ctx.line_to(cx - 8.0, cy - 4.0);
    ctx.line_to(cx - 3.0, cy - 10.0);
    ctx.line_to(cx - 3.0, cy + 10.0);
    ctx.line_to(cx - 8.0, cy + 4.0);
    ctx.line_to(cx - 14.0, cy + 4.0);
    ctx.close_path();
    ctx.fill();
    if muted {
        // Slash across the whole glyph, matching the reference mute icon.
        ctx.set_stroke_color(theme::TEXT_HI);
        ctx.set_line_width(3.0);
        ctx.begin_path();
        ctx.move_to(cx - 13.0, cy + 12.0);
        ctx.line_to(cx + 12.0, cy - 12.0);
        ctx.stroke();
    } else {
        // Two sound-wave arcs off the cone mouth. `ccw: true` sweeps the
        // short way from -0.8 up through 0 to 0.8 (Y-up positive angles are
        // counterclockwise); `false` here wrapped the long way around and
        // drew a big "C" over the body.
        ctx.set_stroke_color(theme::TEXT_HI);
        ctx.set_line_width(2.5);
        ctx.begin_path();
        ctx.arc_to(cx - 3.0, cy, 7.0, -0.8, 0.8, true);
        ctx.stroke();
        ctx.begin_path();
        ctx.arc_to(cx - 3.0, cy, 12.0, -0.8, 0.8, true);
        ctx.stroke();
    }
}
