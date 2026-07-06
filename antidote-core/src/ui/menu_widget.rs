//! Menu-style overlays: main menu, pause, level-complete, game-over.
//!
//! Every overlay shares the same shape:
//! - Reads the current [`Phase`](crate::game::state::Phase) from
//!   [`SharedModel`] to decide if it should paint and accept events.
//! - Draws a semi-transparent backdrop over the play area.
//! - Lays out a vertical column of agg-gui widgets (title, body labels, buttons)
//!   centered on the screen.
//! - Buttons mutate the shared model on click — `init_level`, set phase, etc.

use std::sync::Arc;

use agg_gui::color::Color;
use agg_gui::geometry::Size;
use agg_gui::text::Font;
use agg_gui::widgets::button::{Button, ButtonTheme};
use agg_gui::widgets::label::{Label, LabelAlign};
use agg_gui::{DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::{Phase, World};
use crate::game::update;
use crate::ui::game_model::{MenuView, SharedModel};

/// Width of the centered column that holds the menu widgets.
pub const COL_W: f64 = 360.0;
/// Vertical gap between adjacent items in the column.
pub const COL_GAP: f64 = 12.0;

// ─── Backdrop helpers ────────────────────────────────────────────────────────

pub fn paint_backdrop(ctx: &mut dyn DrawCtx, w: f64, h: f64) {
    ctx.set_fill_color(Color::rgba(0.04, 0.06, 0.10, 0.66));
    ctx.begin_path();
    ctx.rect(0.0, 0.0, w, h);
    ctx.fill();
}

/// Lay out children top-down in a centered column inside `(width, height)`.
/// Each child gets `COL_W` of horizontal space; vertical gap between each is
/// `COL_GAP`. The column is vertically centered.
pub fn layout_centered_column(children: &mut [Box<dyn Widget>], width: f64, height: f64) {
    let mut sizes: Vec<Size> = Vec::with_capacity(children.len());
    let mut total_h = 0.0;
    for (i, c) in children.iter_mut().enumerate() {
        let s = c.layout(Size::new(COL_W, height));
        if i > 0 {
            total_h += COL_GAP;
        }
        total_h += s.height;
        sizes.push(s);
    }
    let mut cursor_y = ((height + total_h) * 0.5).min(height);
    for (c, s) in children.iter_mut().zip(sizes.iter()) {
        let x = ((width - s.width) * 0.5).max(0.0);
        cursor_y -= s.height;
        c.set_bounds(Rect::new(x, cursor_y, s.width, s.height));
        cursor_y -= COL_GAP;
    }
}

pub fn header_label(text: &str, font: Arc<Font>, size: f64) -> Box<dyn Widget> {
    Box::new(
        Label::new(text, font)
            .with_font_size(size)
            .with_align(LabelAlign::Center)
            .with_has_backbuffer(false)
            .with_min_size(Size::new(COL_W, size * 1.4)),
    )
}

pub fn body_label(text: &str, font: Arc<Font>, color: Option<Color>) -> Box<dyn Widget> {
    // Wrap so longer body strings reflow onto multiple lines instead of
    // getting clipped at the column edge. `Label::layout` with `wrap = false`
    // returns `min(natural_width, available_width)` and `paint` clips to its
    // own bounds, so a too-wide string becomes ":o grow a bubble. Trap..."
    // chopped on both sides — exactly the artifact the user reported.
    let mut lbl = Label::new(text, font)
        .with_font_size(16.0)
        .with_align(LabelAlign::Center)
        .with_has_backbuffer(false)
        .with_wrap(true)
        .with_min_size(Size::new(COL_W, 22.0));
    if let Some(c) = color {
        lbl = lbl.with_color(c);
    }
    Box::new(lbl)
}

pub fn primary_button(
    text: &str,
    font: Arc<Font>,
    on_click: impl FnMut() + 'static,
) -> Box<dyn Widget> {
    Box::new(
        Button::new(text, font)
            .with_font_size(18.0)
            .with_min_size(Size::new(COL_W, 44.0))
            .on_click(on_click),
    )
}

pub fn secondary_button(
    text: &str,
    font: Arc<Font>,
    on_click: impl FnMut() + 'static,
) -> Box<dyn Widget> {
    let theme = ButtonTheme {
        background: Color::rgba(0.18, 0.22, 0.30, 1.0),
        background_hovered: Color::rgba(0.24, 0.28, 0.38, 1.0),
        background_pressed: Color::rgba(0.14, 0.17, 0.24, 1.0),
        ..ButtonTheme::default()
    };
    Box::new(
        Button::new(text, font)
            .with_font_size(16.0)
            .with_theme(theme)
            .with_min_size(Size::new(COL_W, 38.0))
            .on_click(on_click),
    )
}

/// Reset world to a fresh "level 1, full lives, zero score" state without
/// leaking physics bodies.
fn reset_to_start(model: &SharedModel) {
    let mut m = model.borrow_mut();
    let m = &mut *m;
    m.physics = crate::game::physics::PhysicsWorld::new(
        crate::consts::VIRTUAL_WIDTH,
        crate::consts::VIRTUAL_HEIGHT,
    );
    m.world = World::new();
    m.world.phase = Phase::Start;
}

// ─── Main menu ──────────────────────────────────────────────────────────────

pub struct MainMenuOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    font: Arc<Font>,
    /// Cached `(saved_session.level/score/lives, recent_scores.len, best_score)`
    /// from the last `rebuild_children` call. Used to skip rebuilds when
    /// nothing the menu shows has actually changed.
    cache_key: MenuCacheKey,
}

#[derive(Default, Clone, PartialEq, Eq)]
struct MenuCacheKey {
    saved: Option<(u32, u64, u8)>,
    scores_len: usize,
    best: u64,
}

impl MainMenuOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let mut s = Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            font,
            cache_key: MenuCacheKey::default(),
        };
        s.rebuild_children();
        s
    }

    /// Build the children vector from the current `Settings`. Buttons capture
    /// closures that mutate the model; cheap enough to redo each time
    /// settings change (which happens at most once per level transition).
    fn rebuild_children(&mut self) {
        let font = self.font.clone();
        let settings = self.model.borrow().settings.clone();

        let mut children: Vec<Box<dyn Widget>> = vec![
            header_label("Antidote", font.clone(), 36.0),
            body_label(
                "Hold to grow a bubble. Trap viruses for 3 seconds.",
                font.clone(),
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            body_label(
                &format!("Best: {}", settings.best_score),
                font.clone(),
                Some(Color::rgba(0.95, 0.85, 0.45, 1.0)),
            ),
        ];

        // Resume button: shown when there's a non-trivial saved session
        // (anything past the start of level 1 from a freshly-pressed Play).
        if let Some(saved) = settings
            .saved_session
            .as_ref()
            .filter(|s| s.level > 1 || s.total_score > 0)
        {
            let level = saved.level;
            let total_score = saved.total_score;
            let lives = saved.lives;
            let resume_model = self.model.clone();
            children.push(primary_button(
                &format!("Resume Level {level} ({total_score})"),
                font.clone(),
                move || {
                    let mut m = resume_model.borrow_mut();
                    let m = &mut *m;
                    m.physics = crate::game::physics::PhysicsWorld::new(
                        crate::consts::VIRTUAL_WIDTH,
                        crate::consts::VIRTUAL_HEIGHT,
                    );
                    m.world = World::new();
                    m.world.level = level;
                    m.world.total_score = total_score;
                    m.world.lives = lives;
                    crate::game::level::init_level(&mut m.world, &mut m.physics);
                },
            ));
        }

        let play_model = self.model.clone();
        children.push(primary_button("Play", font.clone(), move || {
            let mut m = play_model.borrow_mut();
            m.clear_saved_session();
            let m = &mut *m;
            update::start_new_game(&mut m.world, &mut m.physics);
        }));

        // Recent scores. Show up to 5 entries — anything older is in the
        // settings JSON but doesn't clutter the home screen.
        if !settings.recent_scores.is_empty() {
            children.push(body_label(
                "Recent",
                font.clone(),
                Some(Color::rgba(0.65, 0.72, 0.85, 1.0)),
            ));
            for entry in settings.recent_scores.iter().take(5) {
                children.push(body_label(
                    &format!("Lvl {} — {}", entry.level, entry.score),
                    font.clone(),
                    None,
                ));
            }
        }

        self.cache_key = MenuCacheKey {
            saved: settings
                .saved_session
                .as_ref()
                .map(|s| (s.level, s.total_score, s.lives)),
            scores_len: settings.recent_scores.len(),
            best: settings.best_score,
        };
        self.children = children;
    }

    /// Check the cached key against the current settings; if anything we
    /// surface in the menu has changed, rebuild. Called from `layout`.
    fn rebuild_if_stale(&mut self) {
        let key = {
            let s = &self.model.borrow().settings;
            MenuCacheKey {
                saved: s
                    .saved_session
                    .as_ref()
                    .map(|sn| (sn.level, sn.total_score, sn.lives)),
                scores_len: s.recent_scores.len(),
                best: s.best_score,
            }
        };
        if key != self.cache_key {
            self.rebuild_children();
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
        self.rebuild_if_stale();
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        // Backdrop swallows mouse events so the play area underneath
        // doesn't react while a menu is up — but lets keys bubble to the
        // global handler so Esc/P can still toggle pause.
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

// ─── Pause ──────────────────────────────────────────────────────────────────

pub struct PauseOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl PauseOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let resume_model = model.clone();
        let menu_model = model.clone();
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Paused", font.clone(), 32.0),
            body_label(
                "Press Esc or P to resume.",
                font.clone(),
                Some(Color::rgba(0.75, 0.82, 0.95, 1.0)),
            ),
            primary_button("Resume", font.clone(), move || {
                resume_model.borrow_mut().world.phase = Phase::Playing;
            }),
            secondary_button("Back to menu", font, move || {
                reset_to_start(&menu_model);
            }),
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }
}

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
    fn layout(&mut self, available: Size) -> Size {
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        // Backdrop swallows mouse events so the play area underneath
        // doesn't react while a menu is up — but lets keys bubble to the
        // global handler so Esc/P can still toggle pause.
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

// ─── Level complete ────────────────────────────────────────────────────────

pub struct LevelCompleteOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl LevelCompleteOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let next_model = model.clone();
        let menu_model = model.clone();
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Level complete", font.clone(), 32.0),
            body_label("Score this level: 0", font.clone(), None),
            primary_button("Next level", font.clone(), move || {
                let mut m = next_model.borrow_mut();
                let m = &mut *m;
                update::advance_to_next_level(&mut m.world, &mut m.physics);
            }),
            secondary_button("Back to menu", font, move || {
                reset_to_start(&menu_model);
            }),
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }

    fn refresh_dynamic_text(&mut self) {
        let (level, level_score) = {
            let world = &self.model.borrow().world;
            (world.level, world.current_level_score())
        };
        let title = format!("Level {} complete", level);
        self.children[0].set_label_text(&title);
        let body = format!("Score this level: {}", level_score);
        self.children[1].set_label_text(&body);
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
    fn layout(&mut self, available: Size) -> Size {
        self.refresh_dynamic_text();
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        // Backdrop swallows mouse events so the play area underneath
        // doesn't react while a menu is up — but lets keys bubble to the
        // global handler so Esc/P can still toggle pause.
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

// ─── Game over ─────────────────────────────────────────────────────────────

pub struct GameOverOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl GameOverOverlay {
    pub fn new(model: SharedModel, font: Arc<Font>) -> Self {
        let again_model = model.clone();
        let menu_model = model.clone();
        let children: Vec<Box<dyn Widget>> = vec![
            header_label("Game over", font.clone(), 36.0),
            body_label("Final score: 0", font.clone(), None),
            primary_button("Play again", font.clone(), move || {
                let mut m = again_model.borrow_mut();
                let m = &mut *m;
                update::start_new_game(&mut m.world, &mut m.physics);
            }),
            secondary_button("Back to menu", font, move || {
                reset_to_start(&menu_model);
            }),
        ];
        Self {
            bounds: Rect::default(),
            children,
            model,
        }
    }

    fn refresh_dynamic_text(&mut self) {
        let score = self.model.borrow().world.total_score;
        let body = format!("Final score: {}", score);
        self.children[1].set_label_text(&body);
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
    fn layout(&mut self, available: Size) -> Size {
        self.refresh_dynamic_text();
        layout_centered_column(&mut self.children, available.width, available.height);
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        paint_backdrop(ctx, self.bounds.width, self.bounds.height);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        // Backdrop swallows mouse events so the play area underneath
        // doesn't react while a menu is up — but lets keys bubble to the
        // global handler so Esc/P can still toggle pause.
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
