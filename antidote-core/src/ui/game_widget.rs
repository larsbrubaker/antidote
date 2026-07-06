//! `GameWidget` — the letterboxed play area. Reads + mutates the
//! shared [`GameModel`] each frame; never owns the world directly.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, MouseButton, Point, Rect, Widget};
use web_time::Instant;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::state::Phase;
use crate::game::update;
use crate::render::scene;
use crate::theme;
use crate::ui::game_model::SharedModel;
use agg_gui::timestep::FIXED_DT;

pub struct GameWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    /// `world.phase` from the previous paint pass — used to detect
    /// transitions exactly once for session persistence (save the resume
    /// snapshot on entering `LevelComplete`; record the finished session +
    /// clear the snapshot on entering `GameOver`).
    last_phase: Option<Phase>,
}

impl GameWidget {
    pub fn new(model: SharedModel) -> Self {
        Self {
            bounds: Rect::new(0.0, 0.0, VIRTUAL_WIDTH as f64, VIRTUAL_HEIGHT as f64),
            children: Vec::new(),
            model,
            last_phase: None,
        }
    }

    fn letterbox(&self) -> Letterbox {
        arena_letterbox(self.bounds.width, self.bounds.height)
    }

    /// Map an event point (widget-local Y-up pixels) to JS-style logical
    /// coordinates (0..VIRTUAL_WIDTH, 0..VIRTUAL_HEIGHT, Y-down).
    fn event_to_logical(&self, p: Point) -> Option<(f32, f32)> {
        let lb = self.letterbox();
        if lb.scale <= 0.0 {
            return None;
        }
        let local_x = p.x as f32 - lb.offset_x;
        let local_y_up = p.y as f32 - lb.offset_y;
        let logical_x = local_x / lb.scale;
        let logical_y_jsdown = (lb.game_h - local_y_up) / lb.scale;
        if !(0.0..=VIRTUAL_WIDTH).contains(&logical_x)
            || !(0.0..=VIRTUAL_HEIGHT).contains(&logical_y_jsdown)
        {
            return None;
        }
        Some((logical_x, logical_y_jsdown))
    }
}

pub(crate) struct Letterbox {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub game_h: f32,
}

/// Map the live arena (the [`VIRTUAL_WIDTH`]×[`VIRTUAL_HEIGHT`] game space)
/// into a canvas of `(w, h)` local Y-up units. The arena sits inside the
/// playfield panel between the two rails, inset by [`theme::ARENA_INSET`];
/// at the design canvas size the scale is exactly 1.0, and the min-scale
/// letterbox below is a safety net for tests laying out at other sizes.
pub(crate) fn arena_letterbox(w: f64, h: f64) -> Letterbox {
    let play_x = (theme::PLAYFIELD_X + theme::ARENA_INSET) as f32;
    let play_y = theme::ARENA_INSET as f32;
    let play_w = ((w - 2.0 * theme::RAIL_W - 2.0 * theme::ARENA_INSET).max(0.0)) as f32;
    let play_h = ((h - 2.0 * theme::ARENA_INSET).max(0.0)) as f32;
    let target = VIRTUAL_WIDTH / VIRTUAL_HEIGHT;
    let widget_aspect = if play_h > 0.0 {
        play_w / play_h
    } else {
        target
    };
    let scale = if widget_aspect >= target {
        play_h / VIRTUAL_HEIGHT
    } else {
        play_w / VIRTUAL_WIDTH
    };
    let game_w = VIRTUAL_WIDTH * scale;
    let game_h = VIRTUAL_HEIGHT * scale;
    Letterbox {
        scale,
        offset_x: play_x + (play_w - game_w) * 0.5,
        offset_y: play_y + (play_h - game_h) * 0.5,
        game_h,
    }
}

impl Widget for GameWidget {
    fn type_name(&self) -> &'static str {
        "GameWidget"
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
        // Take the full available area; letterbox happens in paint.
        available
    }

    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let mut model = self.model.borrow_mut();
        let now = Instant::now();
        let elapsed = match model.last_paint {
            Some(prev) => (now - prev).as_secs_f32(),
            None => FIXED_DT,
        };
        model.last_paint = Some(now);

        let batch = model.timestep.advance(elapsed);
        for _ in 0..batch.steps {
            let dt = batch.dt;
            let m = &mut *model;
            update::tick(&mut m.world, &mut m.physics, dt);
        }
        // Persist a new best score if this tick beat the previous record.
        // Cheap when called every frame — the comparison short-circuits.
        model.maybe_record_best_score();

        // Detect phase transitions for session persistence. Runs at most
        // once per frame; the comparisons + occasional JSON write are
        // negligible next to physics + paint.
        let phase = model.world.phase;
        let prev = self.last_phase;
        self.last_phase = Some(phase);
        if prev != Some(phase) {
            match phase {
                // Player just cleared a level. Resume snapshot points at
                // the level they're *about to* play next, with the
                // current score + lives carried over. Captured here
                // rather than in `advance_to_next_level` so a player
                // backing out to the menu also resumes cleanly.
                Phase::LevelComplete => {
                    let m = &mut *model;
                    m.settings.saved_session = Some(crate::platform::SavedSession {
                        level: m.world.level + 1,
                        total_score: m.world.total_score,
                        lives: m.world.lives,
                    });
                    m.save_settings();
                }
                // Run is over — record the final score in `recent_scores`
                // and drop the snapshot so the next session starts fresh.
                Phase::GameOver => {
                    let score = model.world.total_score;
                    let level = model.world.level;
                    model.record_finished_session(score, level);
                    model.clear_saved_session();
                }
                _ => {}
            }
        }

        let lb = self.letterbox();
        let time_seconds = model.epoch.elapsed().as_secs_f32();

        // Dish panel + arena boundary paint in widget Y-up coordinates —
        // the dish extends past the live arena to meet the rails.
        let panel = agg_gui::Rect::new(
            theme::PLAYFIELD_X,
            0.0,
            (self.bounds.width - 2.0 * theme::RAIL_W).max(0.0),
            self.bounds.height,
        );
        scene::paint_dish_panel(ctx, panel);
        scene::paint_arena_stroke(
            ctx,
            agg_gui::Rect::new(
                lb.offset_x as f64,
                lb.offset_y as f64,
                (VIRTUAL_WIDTH * lb.scale) as f64,
                lb.game_h as f64,
            ),
        );

        ctx.save();
        // Map JS Y-down logical (0..W, 0..H) → widget Y-up letterboxed pixels.
        ctx.translate(lb.offset_x as f64, (lb.offset_y + lb.game_h) as f64);
        ctx.scale(lb.scale as f64, -(lb.scale as f64));

        // Entity draw order matches the JS `render(ctx, state)` function.

        let world = &model.world;
        for b in &world.solid_bubbles {
            scene::paint_bubble(ctx, b, false);
        }
        for d in &world.dead_viruses {
            scene::paint_dead_virus(ctx, d);
        }
        for d in &world.dying_viruses {
            scene::paint_dying_virus(ctx, d, time_seconds);
        }
        if let Some(g) = &world.growing {
            scene::paint_growing_bubble(ctx, g);
        }
        for v in &world.viruses {
            scene::paint_virus(ctx, v, time_seconds);
        }
        for p in &world.pop_animations {
            scene::paint_pop_animation(ctx, p);
        }

        ctx.restore();
    }

    fn on_event(&mut self, event: &Event) -> EventResult {
        match event {
            Event::MouseDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some((x, y)) = self.event_to_logical(*pos) {
                    let mut model = self.model.borrow_mut();
                    let m = &mut *model;
                    update::on_pointer_down(&mut m.world, &mut m.physics, x, y);
                    return EventResult::Consumed;
                }
            }
            Event::MouseMove { pos } => {
                if let Some((x, y)) = self.event_to_logical(*pos) {
                    let mut model = self.model.borrow_mut();
                    update::on_pointer_move(&mut model.world, x, y);
                }
            }
            Event::MouseUp {
                button: MouseButton::Left,
                ..
            } => {
                let mut model = self.model.borrow_mut();
                let m = &mut *model;
                update::on_pointer_up(&mut m.world, &mut m.physics);
                return EventResult::Consumed;
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn needs_draw(&self) -> bool {
        // Continuous animation — always request a redraw.
        true
    }
}
