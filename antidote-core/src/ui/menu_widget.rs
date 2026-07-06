//! Petri Pop main menu overlay.
//!
//! Pause / level-complete / game-over overlays share the same look and
//! button kit but live in [`crate::ui::session_overlays`] to keep both
//! files under the project's line-count guardrail.
//!
//! Every overlay is custom-painted (see [`crate::ui::petri_kit`]) to match
//! the mockups in `docs/New Design/Antidote Redesign.dc.html`. Buttons are
//! immediate-mode [`ButtonSet`]s; clicks map to model mutations here.
//!
//! Coordinates: widget-local Y-up. Design-space "y from top" values from
//! the mockups convert as `y_up = height - y_top`.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::{Phase, World};
use crate::game::update;
use crate::theme::{self, Fonts};
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::paint_util::{
    fill_text_centered, fill_text_right, fill_text_tracked, fmt_thousands,
};
use crate::ui::petri_kit::{
    paint_logo_bubble, paint_menu_backdrop, paint_mini_virus, paint_panel, swallow_mouse,
    ButtonKind, ButtonSet, ChipIcon, KitButton,
};

/// Reset world to a fresh "level 1, full lives, zero score" start state
/// without leaking physics bodies. Shared with `session_overlays`, whose
/// pause/level-complete/game-over "back to menu" buttons call this too.
pub(crate) fn reset_to_start(model: &SharedModel) {
    let mut m = model.borrow_mut();
    let m = &mut *m;
    m.physics = crate::game::physics::PhysicsWorld::new(
        crate::consts::VIRTUAL_WIDTH,
        crate::consts::VIRTUAL_HEIGHT,
    );
    m.world = World::new();
    m.world.phase = Phase::Start;
}

fn start_new_run(model: &SharedModel) {
    let mut m = model.borrow_mut();
    m.clear_saved_session();
    if m.is_mobile {
        m.pending_enter_fullscreen = true;
    }
    m.session_start_best = m.settings.best_score;
    let m = &mut *m;
    update::start_new_game(&mut m.world, &mut m.physics);
}

// ─── Main menu ──────────────────────────────────────────────────────────────

pub struct MainMenuOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

impl MainMenuOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }

    /// Rects only depend on whether a resume button exists, so rebuilding
    /// each layout pass is cheap and always current.
    fn rebuild_buttons(&mut self, has_resume: bool, resume_level: u32) {
        let h = self.bounds.height;
        let cx = self.bounds.width * 0.5;
        self.buttons.clear();
        // PLAY at design y 330 (top), 300×68.
        self.buttons.push(KitButton {
            id: "play",
            rect: Rect::new(cx - 150.0, h - 330.0 - 68.0, 300.0, 68.0),
            label: "PLAY".into(),
            kind: ButtonKind::Primary,
            font_size: 26.0,
        });
        if has_resume {
            self.buttons.push(KitButton {
                id: "resume",
                rect: Rect::new(cx - 150.0, h - 416.0 - 56.0, 300.0, 56.0),
                label: format!("RESUME · LV {resume_level}"),
                kind: ButtonKind::Secondary,
                font_size: 20.0,
            });
        }
        // Bottom chip row, centered, 48 tall at design bottom 40.
        let chip_y = 40.0 + 48.0;
        let chips: [(&'static str, &str, ChipIcon); 3] = [
            ("help", "HELP", ChipIcon::Question),
            ("file", "FILE", ChipIcon::Save),
            ("fullscreen", "FULLSCREEN", ChipIcon::Expand),
        ];
        // Widths measured at paint time are unavailable here (no ctx), so
        // use generous fixed widths per label length; measured centering
        // differences are invisible at chip scale.
        let widths = [104.0, 96.0, 168.0];
        let gap = 20.0;
        let total: f64 = widths.iter().sum::<f64>() + gap * 2.0;
        let mut x = cx - total * 0.5;
        for ((id, label, icon), w) in chips.into_iter().zip(widths) {
            self.buttons.push(KitButton {
                id,
                rect: Rect::new(x, h - chip_y, w, 48.0),
                label: label.into(),
                kind: ButtonKind::Chip(icon),
                font_size: theme::FS_CHIP,
            });
            x += w + gap;
        }
    }
}

impl Widget for MainMenuOverlay {
    fn type_name(&self) -> &'static str {
        "MainMenuOverlay"
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
    fn layout(&mut self, available: Size) -> Size {
        let (has_resume, resume_level) = {
            let m = self.model.borrow();
            match m
                .settings
                .saved_session
                .as_ref()
                .filter(|s| s.level > 1 || s.total_score > 0)
            {
                Some(s) => (true, s.level),
                None => (false, 0),
            }
        };
        self.rebuild_buttons(has_resume, resume_level);
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;
        paint_menu_backdrop(ctx, w, h);

        // Big faint dish ring behind the title (design: 440Ø at x 420,
        // y -40 — partially off-canvas top).
        let ring_cx = w * 0.5;
        let ring_cy = h + 40.0 - 220.0;
        ctx.set_stroke_color(theme::LIME_500.with_alpha(0.10));
        ctx.set_line_width(2.0);
        ctx.begin_path();
        ctx.circle(ring_cx, ring_cy, 220.0);
        ctx.stroke();

        // Two ambient viruses drifting in the backdrop.
        ctx.set_global_alpha(0.5);
        paint_mini_virus(ctx, 155.0, h - 515.0, 30.0);
        ctx.set_global_alpha(0.4);
        paint_mini_virus(ctx, w - 223.0, h - 583.0, 20.0);
        ctx.set_global_alpha(1.0);

        // ── Logo: ANTID⦿TE with the bubble-O ─────────────────────────────
        let logo_baseline = h - 110.0 - 78.0; // cap-top at 110 design-y
        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(theme::FS_LOGO);
        ctx.set_fill_color(theme::TEXT_HI);
        let ls = theme::LS_LOGO * theme::FS_LOGO;
        let left = "ANTID";
        let right = "TE";
        let left_w = crate::ui::paint_util::measure_tracked(ctx, left, ls);
        let right_w = crate::ui::paint_util::measure_tracked(ctx, right, ls);
        let o_d = 78.0;
        let o_pad = 6.0;
        let total = left_w + o_pad + o_d + o_pad + right_w;
        let x0 = (w - total) * 0.5;
        fill_text_tracked(ctx, left, x0, logo_baseline, ls);
        fill_text_tracked(
            ctx,
            right,
            x0 + left_w + o_pad * 2.0 + o_d,
            logo_baseline,
            ls,
        );
        let o_cx = x0 + left_w + o_pad + o_d * 0.5;
        let o_cy = logo_baseline + 34.0;
        paint_logo_bubble(ctx, o_cx, o_cy, o_d * 0.5, 5.0);
        paint_mini_virus(ctx, o_cx, o_cy, 13.0);

        // Tagline.
        ctx.set_font(self.fonts.extrabold.clone());
        ctx.set_font_size(17.0);
        ctx.set_fill_color(theme::LIME_500);
        fill_text_centered(
            ctx,
            "TRAP \u{2019}EM · CURE \u{2019}EM",
            w * 0.5,
            h - 232.0,
            5.0,
        );

        // ── Best / recent card (right side) ──────────────────────────────
        let (best, recent) = {
            let m = self.model.borrow();
            (
                m.settings.best_score,
                m.settings
                    .recent_scores
                    .iter()
                    .take(5)
                    .map(|e| e.score)
                    .collect::<Vec<_>>(),
            )
        };
        if best > 0 || !recent.is_empty() {
            let card_w = 280.0;
            let rows = recent.len() as f64;
            let card_h = 96.0 + 13.0 + 10.0 + rows * 30.0 + 16.0;
            let card = Rect::new(w - 72.0 - card_w, h - 150.0 - card_h, card_w, card_h);
            paint_panel(ctx, card);
            let pad_x = card.x + 24.0;
            let mut y_top = 150.0 + 22.0 + 13.0; // design-y of BEST label baseline area

            ctx.set_font(self.fonts.bold.clone());
            ctx.set_font_size(13.0);
            ctx.set_fill_color(theme::TEXT_LOW);
            fill_text_tracked(ctx, "BEST", pad_x, h - y_top, 2.5);
            y_top += 38.0;
            ctx.set_font(self.fonts.extrabold.clone());
            ctx.set_font_size(34.0);
            ctx.set_fill_color(theme::GOLD_400);
            fill_text_tracked(
                ctx,
                &format!("{} \u{2605}", fmt_thousands(best)),
                pad_x,
                h - y_top,
                0.0,
            );
            y_top += 20.0;
            ctx.set_stroke_color(theme::HAIRLINE);
            ctx.set_line_width(1.0);
            ctx.begin_path();
            ctx.move_to(pad_x, h - y_top);
            ctx.line_to(card.x + card_w - 24.0, h - y_top);
            ctx.stroke();
            if !recent.is_empty() {
                y_top += 24.0;
                ctx.set_font(self.fonts.bold.clone());
                ctx.set_font_size(13.0);
                ctx.set_fill_color(theme::TEXT_LOW);
                fill_text_tracked(ctx, "RECENT", pad_x, h - y_top, 2.5);
                for (i, score) in recent.iter().enumerate() {
                    y_top += 30.0;
                    ctx.set_font(self.fonts.extrabold.clone());
                    ctx.set_font_size(14.0);
                    ctx.set_fill_color(if i == 0 {
                        theme::LIME_500
                    } else {
                        theme::TEXT_LOW
                    });
                    fill_text_tracked(ctx, &format!("{:02}", i + 1), pad_x, h - y_top, 0.0);
                    ctx.set_font_size(20.0);
                    ctx.set_fill_color(if i == 0 {
                        theme::TEXT_HI
                    } else {
                        theme::TEXT_MID
                    });
                    fill_text_right(
                        ctx,
                        &fmt_thousands(*score),
                        card.x + card_w - 24.0,
                        h - y_top,
                        0.0,
                    );
                }
            }
        }

        // Footer caption.
        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(13.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_tracked(ctx, "v2.0 · saves stay on this device", 24.0, 20.0, 0.0);

        self.buttons.paint(ctx, &self.fonts);
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some(id) = self.buttons.on_event(event) {
            match id {
                "play" => start_new_run(&self.model),
                "resume" => {
                    let saved = self.model.borrow().settings.saved_session.clone();
                    if let Some(s) = saved {
                        let mut m = self.model.borrow_mut();
                        if m.is_mobile {
                            m.pending_enter_fullscreen = true;
                        }
                        m.session_start_best = m.settings.best_score;
                        let m = &mut *m;
                        m.physics = crate::game::physics::PhysicsWorld::new(
                            crate::consts::VIRTUAL_WIDTH,
                            crate::consts::VIRTUAL_HEIGHT,
                        );
                        m.world = World::new();
                        m.world.level = s.level;
                        m.world.total_score = s.total_score;
                        m.world.lives = s.lives;
                        crate::game::level::init_level(&mut m.world, &mut m.physics);
                    }
                }
                "help" => self.model.borrow_mut().menu_view = MenuView::Help,
                "file" => self.model.borrow_mut().menu_view = MenuView::File,
                "fullscreen" => self.model.borrow_mut().pending_fullscreen_toggle = true,
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
