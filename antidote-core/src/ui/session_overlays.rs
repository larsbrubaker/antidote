//! Petri Pop pause / level-complete / game-over overlays.
//!
//! Split out of [`crate::ui::menu_widget`] (which keeps the main menu) so
//! both files stay under the project's line-count guardrail. Shares the
//! same custom-painted look (see [`crate::ui::petri_kit`]) and button kit.
//!
//! Coordinates: widget-local Y-up. Design-space "y from top" values from
//! the mockups convert as `y_up = height - y_top`.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::game::update;
use crate::theme::{self, Fonts};
use crate::ui::game_model::SharedModel;
use crate::ui::menu_widget::reset_to_start;
use crate::ui::paint_util::{
    fill_text_centered, fill_text_right, fill_text_tracked, fmt_thousands,
};
use crate::ui::petri_kit::{
    paint_panel, paint_playfield_scrim, swallow_mouse, ButtonKind, ButtonSet, KitButton,
};

/// Center panel geometry shared by pause / level-complete / game-over.
fn centered_panel(bounds: Rect, panel_w: f64, panel_h: f64) -> Rect {
    Rect::new(
        (bounds.width - panel_w) * 0.5,
        (bounds.height - panel_h) * 0.5,
        panel_w,
        panel_h,
    )
}

// ─── Pause ──────────────────────────────────────────────────────────────────

pub struct PauseOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

impl PauseOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }
}

const PAUSE_W: f64 = 420.0;
const PAUSE_H: f64 = 330.0;

impl Widget for PauseOverlay {
    fn type_name(&self) -> &'static str {
        "PauseOverlay"
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
        self.model.borrow().world.phase == Phase::Paused
    }
    /// The rails stay live while paused — only the playfield is claimed.
    fn hit_test(&self, p: agg_gui::Point) -> bool {
        p.x >= theme::PLAYFIELD_X
            && p.x <= self.bounds.width - theme::RAIL_W
            && p.y >= 0.0
            && p.y <= self.bounds.height
    }
    fn layout(&mut self, available: Size) -> Size {
        let panel = centered_panel(
            Rect::new(0.0, 0.0, available.width, available.height),
            PAUSE_W,
            PAUSE_H,
        );
        self.buttons.clear();
        self.buttons.push(KitButton {
            id: "resume",
            rect: Rect::new(
                panel.x + 44.0,
                panel.y + panel.height - 118.0 - 64.0,
                PAUSE_W - 88.0,
                64.0,
            ),
            label: "RESUME".into(),
            kind: ButtonKind::Primary,
            font_size: 24.0,
        });
        self.buttons.push(KitButton {
            id: "menu",
            rect: Rect::new(
                panel.x + 44.0,
                panel.y + panel.height - 198.0 - 56.0,
                PAUSE_W - 88.0,
                56.0,
            ),
            label: "BACK TO MENU".into(),
            kind: ButtonKind::Secondary,
            font_size: 19.0,
        });
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_playfield_scrim(ctx, self.bounds, 0.78);
        let panel = centered_panel(self.bounds, PAUSE_W, PAUSE_H);
        paint_panel(ctx, panel);
        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(theme::FS_OVERLAY_TITLE);
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_centered(
            ctx,
            "PAUSED",
            panel.x + panel.width * 0.5,
            panel.y + panel.height - 40.0 - 42.0,
            1.0,
        );
        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(15.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_centered(
            ctx,
            "tap the pause button anytime · Esc / P on desktop",
            panel.x + panel.width * 0.5,
            panel.y + 26.0,
            0.0,
        );
        self.buttons.paint(ctx, &self.fonts);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some(id) = self.buttons.on_event(event) {
            match id {
                "resume" => self.model.borrow_mut().world.phase = Phase::Playing,
                "menu" => reset_to_start(&self.model),
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

// ─── Level complete ────────────────────────────────────────────────────────

pub struct LevelCompleteOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

const LC_W: f64 = 480.0;
const LC_H: f64 = 380.0;

impl LevelCompleteOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }
}

impl Widget for LevelCompleteOverlay {
    fn type_name(&self) -> &'static str {
        "LevelCompleteOverlay"
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
        self.model.borrow().world.phase == Phase::LevelComplete
    }
    fn hit_test(&self, p: agg_gui::Point) -> bool {
        p.x >= theme::PLAYFIELD_X
            && p.x <= self.bounds.width - theme::RAIL_W
            && p.y >= 0.0
            && p.y <= self.bounds.height
    }
    fn layout(&mut self, available: Size) -> Size {
        let panel = centered_panel(
            Rect::new(0.0, 0.0, available.width, available.height),
            LC_W,
            LC_H,
        );
        self.buttons.clear();
        self.buttons.push(KitButton {
            id: "next",
            rect: Rect::new(panel.x + 48.0, panel.y + 92.0, LC_W - 96.0, 64.0),
            label: "NEXT LEVEL".into(),
            kind: ButtonKind::Primary,
            font_size: 24.0,
        });
        self.buttons.push(KitButton {
            id: "menu",
            rect: Rect::new(panel.x + 48.0, panel.y + 30.0, LC_W - 96.0, 40.0),
            label: "BACK TO MENU".into(),
            kind: ButtonKind::Ghost,
            font_size: 18.0,
        });
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_playfield_scrim(ctx, self.bounds, 0.78);
        // Static confetti accents around the panel (animated in the polish
        // pass): small rects + dots in lime / gold / violet / coral.
        let w = self.bounds.width;
        let h = self.bounds.height;
        for (x, y_top, ww, hh, rot, color) in [
            (500.0, 180.0, 12.0, 16.0, 24.0, theme::LIME_500),
            (820.0, 150.0, 10.0, 14.0, -30.0, theme::GOLD_400),
            (900.0, 540.0, 12.0, 16.0, 40.0, theme::CORAL_500),
            (930.0, 330.0, 11.0, 15.0, -18.0, theme::LIME_500),
        ] {
            ctx.save();
            ctx.translate(x, h - y_top);
            ctx.rotate(rot * std::f64::consts::PI / 180.0);
            ctx.set_fill_color(color);
            ctx.begin_path();
            ctx.rect(-ww * 0.5, -hh * 0.5, ww, hh);
            ctx.fill();
            ctx.restore();
        }
        ctx.set_fill_color(theme::VIOLET_400);
        ctx.begin_path();
        ctx.circle(400.0, h - 520.0, 5.0);
        ctx.circle(360.0, h - 300.0, 4.5);
        ctx.fill();
        let _ = w;

        let panel = centered_panel(self.bounds, LC_W, LC_H);
        paint_panel(ctx, panel);
        let cx = panel.x + panel.width * 0.5;
        let (level, level_score, total) = {
            let world = &self.model.borrow().world;
            (world.level, world.current_level_score(), world.total_score)
        };

        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(48.0);
        ctx.set_fill_color(theme::LIME_500);
        fill_text_centered(
            ctx,
            &format!("LEVEL {level} CLEAR!"),
            cx,
            panel.y + panel.height - 78.0,
            0.5,
        );

        let left = panel.x + 48.0;
        let right = panel.x + panel.width - 48.0;
        let mut row_y = panel.y + panel.height - 128.0;
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(theme::FS_LABEL);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_tracked(ctx, "LEVEL SCORE", left, row_y, 2.5);
        ctx.set_font(self.fonts.extrabold.clone());
        ctx.set_font_size(28.0);
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_right(
            ctx,
            &format!("+{}", fmt_thousands(level_score)),
            right,
            row_y,
            0.0,
        );

        row_y -= 22.0;
        ctx.set_stroke_color(theme::HAIRLINE);
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.move_to(left, row_y);
        ctx.line_to(right, row_y);
        ctx.stroke();

        row_y -= 42.0;
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(theme::FS_LABEL);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_tracked(ctx, "TOTAL", left, row_y, 2.5);
        ctx.set_font(self.fonts.extrabold.clone());
        ctx.set_font_size(40.0);
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_right(ctx, &fmt_thousands(total), right, row_y, 0.0);

        self.buttons.paint(ctx, &self.fonts);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some(id) = self.buttons.on_event(event) {
            match id {
                "next" => {
                    let mut m = self.model.borrow_mut();
                    let m = &mut *m;
                    update::advance_to_next_level(&mut m.world, &mut m.physics);
                }
                "menu" => reset_to_start(&self.model),
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

// ─── Game over ─────────────────────────────────────────────────────────────

pub struct GameOverOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

const GO_W: f64 = 480.0;
const GO_H: f64 = 420.0;

impl GameOverOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }

    fn is_new_best(&self) -> bool {
        let m = self.model.borrow();
        m.world.total_score > 0 && m.world.total_score > m.session_start_best
    }
}

impl Widget for GameOverOverlay {
    fn type_name(&self) -> &'static str {
        "GameOverOverlay"
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
        self.model.borrow().world.phase == Phase::GameOver
    }
    fn hit_test(&self, p: agg_gui::Point) -> bool {
        p.x >= theme::PLAYFIELD_X
            && p.x <= self.bounds.width - theme::RAIL_W
            && p.y >= 0.0
            && p.y <= self.bounds.height
    }
    fn layout(&mut self, available: Size) -> Size {
        let panel = centered_panel(
            Rect::new(0.0, 0.0, available.width, available.height),
            GO_W,
            GO_H,
        );
        self.buttons.clear();
        self.buttons.push(KitButton {
            id: "again",
            rect: Rect::new(panel.x + 48.0, panel.y + 96.0, GO_W - 96.0, 64.0),
            label: "PLAY AGAIN".into(),
            kind: ButtonKind::Primary,
            font_size: 24.0,
        });
        self.buttons.push(KitButton {
            id: "menu",
            rect: Rect::new(panel.x + 48.0, panel.y + 34.0, GO_W - 96.0, 40.0),
            label: "BACK TO MENU".into(),
            kind: ButtonKind::Ghost,
            font_size: 18.0,
        });
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_playfield_scrim(ctx, self.bounds, 0.82);
        let new_best = self.is_new_best();
        let panel = centered_panel(self.bounds, GO_W, GO_H);
        let cx = panel.x + panel.width * 0.5;
        let cy = panel.y + panel.height * 0.5;

        if new_best {
            // Gold celebration rings + confetti accents behind the panel.
            for (r, a) in [(280.0, 0.28), (350.0, 0.12)] {
                ctx.set_stroke_color(theme::GOLD_400.with_alpha(a));
                ctx.set_line_width(if a > 0.2 { 3.0 } else { 2.0 });
                ctx.begin_path();
                ctx.circle(cx, cy, r);
                ctx.stroke();
            }
            for (x, y_top, rot, a) in [
                (460.0, 140.0, 24.0, 1.0),
                (830.0, 170.0, -35.0, 0.7),
                (880.0, 540.0, 48.0, 1.0),
            ] {
                ctx.save();
                ctx.translate(x, self.bounds.height - y_top);
                ctx.rotate(rot * std::f64::consts::PI / 180.0);
                ctx.set_fill_color(theme::GOLD_400.with_alpha(a));
                ctx.begin_path();
                ctx.rect(-6.0, -8.0, 12.0, 16.0);
                ctx.fill();
                ctx.restore();
            }
        }

        // Panel — gold border on a new best.
        crate::ui::paint_util::raised_rect(
            ctx,
            panel,
            theme::RADIUS_PANEL,
            8.0,
            theme::INK_700,
            theme::EDGE_950,
            Some(if new_best {
                theme::GOLD_400.with_alpha(0.45)
            } else {
                theme::HAIRLINE
            }),
        );

        let (score, prev_best) = {
            let m = self.model.borrow();
            (m.world.total_score, m.session_start_best)
        };

        let mut y = panel.y + panel.height - 56.0;
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(18.0);
        ctx.set_fill_color(theme::TEXT_MID);
        fill_text_centered(ctx, "GAME OVER", cx, y, 4.0);

        if new_best {
            y -= 52.0;
            ctx.set_font(self.fonts.extrabold_italic.clone());
            ctx.set_font_size(54.0);
            ctx.set_fill_color(theme::GOLD_400);
            fill_text_centered(ctx, "NEW BEST! \u{2605}", cx, y, 0.0);
        }

        y -= 70.0;
        ctx.set_font(self.fonts.extrabold.clone());
        ctx.set_font_size(if new_best { 72.0 } else { 64.0 });
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_centered(ctx, &fmt_thousands(score), cx, y, 0.0);

        y -= 30.0;
        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(16.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        let caption = if new_best {
            format!("previous best {}", fmt_thousands(prev_best))
        } else {
            format!(
                "best {}",
                fmt_thousands(self.model.borrow().settings.best_score)
            )
        };
        fill_text_centered(ctx, &caption, cx, y, 0.0);

        self.buttons.paint(ctx, &self.fonts);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some(id) = self.buttons.on_event(event) {
            match id {
                "again" => {
                    let mut m = self.model.borrow_mut();
                    if m.is_mobile {
                        m.pending_enter_fullscreen = true;
                    }
                    m.session_start_best = m.settings.best_score;
                    let m = &mut *m;
                    update::start_new_game(&mut m.world, &mut m.physics);
                }
                "menu" => reset_to_start(&self.model),
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
