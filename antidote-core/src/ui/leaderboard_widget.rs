//! `LeaderboardOverlay` — top-N scores for the current game.
//!
//! Shown when [`MenuView`](crate::ui::game_model::MenuView) is `Leaderboard`.
//! On each layout pass, if no top scores have been fetched yet, kicks off a
//! fetch through [`PostgrestClient`](crate::db::client::PostgrestClient)
//! against the `games` row whose `slug` matches `services.config.game_slug`.
//! The drain hook in [`crate::ui::drain_db_inbox`] writes the result into
//! `menu_caches.top_scores`, and the next layout pass renders rows.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::widgets::label::{Label, LabelAlign};
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::db::models::LeaderboardEntry;
use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::menu_widget::{
    body_label, header_label, layout_centered_column, paint_backdrop, secondary_button, COL_W,
};

const MAX_ROWS: usize = 10;

pub struct LeaderboardOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    /// Snapshot of which `top_scores` slot we last rendered (None means
    /// "loading", Some(n) means n rows). Used to know when to rebuild
    /// children rather than repainting the same labels.
    rendered_count: Option<usize>,
}

impl LeaderboardOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let back_model = model.clone();
        let back_btn = secondary_button("Back", font.clone(), move || {
            back_model.borrow_mut().menu_view = MenuView::Main;
        });
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Leaderboard", font.clone(), 30.0),
            body_label("Loading…", font.clone(), None),
            back_btn,
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
            font,
            rendered_count: None,
        }
    }

    /// Called from `layout` — kicks off a fetch if none is in flight and we
    /// don't yet have data, then rebuilds the row labels from cache.
    fn refresh(&mut self) {
        let (need_fetch, snapshot, error) = {
            let m = self.model.borrow();
            let need_fetch = m.menu_caches.top_scores.is_none()
                && !m.menu_caches.top_scores_pending
                && m.menu_caches.top_scores_error.is_none();
            (
                need_fetch,
                m.menu_caches.top_scores.clone(),
                m.menu_caches.top_scores_error.clone(),
            )
        };

        if need_fetch {
            self.dispatch_fetch();
        }

        let target_count = snapshot.as_ref().map(|v| v.len()).unwrap_or(0);
        let need_rebuild = self.rendered_count != Some(target_count) || error.is_some();
        if need_rebuild {
            self.rebuild_rows(snapshot.as_deref(), error.as_deref());
            self.rendered_count = snapshot.as_ref().map(|v| v.len());
        }
    }

    fn dispatch_fetch(&self) {
        let mut m = self.model.borrow_mut();
        m.menu_caches.top_scores_pending = true;
        m.menu_caches.top_scores_error = None;
        let slug = m.services.config.game_slug.clone();
        m.services
            .postgrest
            .top_leaderboard_async(&slug, MAX_ROWS as u32, &m.services.inbox);
    }

    fn rebuild_rows(&mut self, scores: Option<&[LeaderboardEntry]>, error: Option<&str>) {
        let back_model = self.model.clone();
        let mut new_children: Vec<Box<dyn Widget>> = Vec::with_capacity(MAX_ROWS + 4);
        new_children.push(header_label("Leaderboard", self.font.clone(), 30.0));

        match (scores, error) {
            (_, Some(err)) => {
                new_children.push(body_label(
                    &format!("Error: {err}"),
                    self.font.clone(),
                    Some(Color::rgba(1.0, 0.45, 0.45, 1.0)),
                ));
            }
            (Some([]), _) => {
                new_children.push(body_label(
                    "No scores yet — be the first.",
                    self.font.clone(),
                    Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
                ));
            }
            (Some(rows), _) => {
                for (i, row) in rows.iter().take(MAX_ROWS).enumerate() {
                    let line = format!("{:>2}. {:<20} {}", i + 1, row.handle, row.high_score);
                    new_children.push(Box::new(
                        Label::new(line, self.font.clone())
                            .with_font_size(15.0)
                            .with_align(LabelAlign::Center)
                            .with_has_backbuffer(false)
                            .with_min_size(Size::new(COL_W, 22.0)),
                    ));
                }
            }
            (None, _) => {
                new_children.push(body_label("Loading…", self.font.clone(), None));
            }
        }

        new_children.push(secondary_button("Back", self.font.clone(), move || {
            back_model.borrow_mut().menu_view = MenuView::Main;
        }));

        self.children = new_children;
    }
}

impl Widget for LeaderboardOverlay {
    fn type_name(&self) -> &'static str {
        "LeaderboardOverlay"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::Leaderboard
    }
    fn layout(&mut self, available: Size) -> Size {
        if self.is_visible() {
            self.refresh();
        }
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
