//! `OtherGamesOverlay` — list of games hosted on the same Supabase project.
//!
//! Shown when [`MenuView`](crate::ui::game_model::MenuView) is `OtherGames`.
//! On layout, kicks off `GET /rest/v1/games` if no cache exists. The drain
//! hook writes the result into `menu_caches.games`. Each row renders the
//! display name + description; clicking a row's button takes the player to
//! that game's `deploy_url`.
//!
//! On native we surface deploy_url by writing it into a tiny status label
//! below the list rather than launching a browser — keeps the platform
//! shells responsibility-free of opening URLs. (Future: add a
//! `Platform::open_url` trait if we want one-click navigation.)

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::widgets::label::{Label, LabelAlign};
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::db::models::Game;
use crate::game::state::Phase;
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::menu_widget::{
    body_label, header_label, layout_centered_column, paint_backdrop, secondary_button, COL_W,
};

pub struct OtherGamesOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    rendered_count: Option<usize>,
}

impl OtherGamesOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let back_model = model.clone();
        let back_btn = secondary_button("Back", font.clone(), move || {
            back_model.borrow_mut().menu_view = MenuView::Main;
        });
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Other games", font.clone(), 30.0),
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

    fn refresh(&mut self) {
        let (need_fetch, snapshot, error) = {
            let m = self.model.borrow();
            let need_fetch = m.menu_caches.games.is_none()
                && !m.menu_caches.games_pending
                && m.menu_caches.games_error.is_none();
            (
                need_fetch,
                m.menu_caches.games.clone(),
                m.menu_caches.games_error.clone(),
            )
        };

        if need_fetch {
            let mut m = self.model.borrow_mut();
            m.menu_caches.games_pending = true;
            m.services.postgrest.list_games_async(&m.services.inbox);
        }

        let target_count = snapshot.as_ref().map(|v| v.len()).unwrap_or(0);
        let need_rebuild = self.rendered_count != Some(target_count) || error.is_some();
        if need_rebuild {
            self.rebuild_rows(snapshot.as_deref(), error.as_deref());
            self.rendered_count = snapshot.as_ref().map(|v| v.len());
        }
    }

    fn rebuild_rows(&mut self, games: Option<&[Game]>, error: Option<&str>) {
        let back_model = self.model.clone();
        let mut new_children: Vec<Box<dyn Widget>> = Vec::new();
        new_children.push(header_label("Other games", self.font.clone(), 30.0));

        match (games, error) {
            (_, Some(err)) => {
                new_children.push(body_label(
                    &format!("Error: {err}"),
                    self.font.clone(),
                    Some(Color::rgba(1.0, 0.45, 0.45, 1.0)),
                ));
            }
            (Some([]), _) => {
                new_children.push(body_label(
                    "No games registered yet.",
                    self.font.clone(),
                    Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
                ));
            }
            (Some(rows), _) => {
                let game_slug = self.model.borrow().services.config.game_slug.clone();
                for g in rows.iter() {
                    let mut line = g.display_name.clone();
                    if g.slug == game_slug {
                        line.push_str("  (you are here)");
                    } else if let Some(url) = g.deploy_url.as_ref() {
                        line.push('\n');
                        line.push_str(url);
                    }
                    new_children.push(Box::new(
                        Label::new(line, self.font.clone())
                            .with_font_size(15.0)
                            .with_align(LabelAlign::Center)
                            .with_has_backbuffer(false)
                            .with_wrap(true)
                            .with_min_size(Size::new(COL_W, 38.0)),
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

impl Widget for OtherGamesOverlay {
    fn type_name(&self) -> &'static str {
        "OtherGamesOverlay"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::OtherGames
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
