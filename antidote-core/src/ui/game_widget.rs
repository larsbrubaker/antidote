//! `GameWidget` — the 800×600 letterboxed play area. Reads + mutates the
//! shared [`GameModel`] each frame; never owns the world directly.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, MouseButton, Point, Rect, Widget};
use web_time::Instant;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::timestep::FIXED_DT;
use crate::game::update;
use crate::render::scene;
use crate::ui::game_model::SharedModel;
use crate::ui::hud_widget::HUD_HEIGHT;

pub struct GameWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
}

impl GameWidget {
    pub fn new(model: SharedModel) -> Self {
        Self {
            bounds: Rect::new(0.0, 0.0, VIRTUAL_WIDTH as f64, VIRTUAL_HEIGHT as f64),
            children: Vec::new(),
            model,
        }
    }

    /// Compute a centered letterbox into the area BELOW the HUD bar so the top
    /// strip of the screen reads as chrome, not playfield.
    fn letterbox(&self) -> Letterbox {
        let w = self.bounds.width as f32;
        let h_full = self.bounds.height as f32;
        let hud = HUD_HEIGHT as f32;
        let h_play = (h_full - hud).max(0.0);
        let target = VIRTUAL_WIDTH / VIRTUAL_HEIGHT;
        let widget_aspect = if h_play > 0.0 { w / h_play } else { target };
        let scale = if widget_aspect >= target {
            h_play / VIRTUAL_HEIGHT
        } else {
            w / VIRTUAL_WIDTH
        };
        let game_w = VIRTUAL_WIDTH * scale;
        let game_h = VIRTUAL_HEIGHT * scale;
        Letterbox {
            scale,
            offset_x: (w - game_w) * 0.5,
            // Y-up: subtract HUD strip from the top of the available area.
            offset_y: (h_play - game_h) * 0.5,
            game_h,
        }
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

struct Letterbox {
    scale: f32,
    offset_x: f32,
    offset_y: f32,
    game_h: f32,
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

        let lb = self.letterbox();
        let time_seconds = model.epoch.elapsed().as_secs_f32();

        ctx.save();
        // Map JS Y-down logical (0..W, 0..H) → widget Y-up letterboxed pixels.
        ctx.translate(lb.offset_x as f64, (lb.offset_y + lb.game_h) as f64);
        ctx.scale(lb.scale as f64, -(lb.scale as f64));

        // Draw order matches the JS `render(ctx, state)` function exactly.
        scene::paint_background_and_grid(ctx);
        scene::paint_border(ctx);

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
