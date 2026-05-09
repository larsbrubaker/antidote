//! `GameWidget` — the 800×600 letterboxed play area.

use agg_gui::geometry::Size;
use agg_gui::{DrawCtx, Event, EventResult, Point, Rect, Widget};
use web_time::Instant;

use crate::consts::{VIRTUAL_HEIGHT, VIRTUAL_WIDTH};
use crate::game::physics::PhysicsWorld;
use crate::game::state::World;
use crate::render::scene;

pub struct GameWidget {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    pub world: World,
    pub physics: PhysicsWorld,
    /// Wall-clock start; used to compute a monotonic time for animations.
    epoch: Instant,
}

impl GameWidget {
    pub fn new() -> Self {
        Self {
            bounds: Rect::new(0.0, 0.0, VIRTUAL_WIDTH as f64, VIRTUAL_HEIGHT as f64),
            children: Vec::new(),
            world: World::new(),
            physics: PhysicsWorld::new(VIRTUAL_WIDTH, VIRTUAL_HEIGHT),
            epoch: Instant::now(),
        }
    }

    /// Compute a centered letterbox: scale that fits 800×600 into the widget
    /// bounds, plus the centered offset.
    fn letterbox(&self) -> Letterbox {
        let w = self.bounds.width as f32;
        let h = self.bounds.height as f32;
        let target = VIRTUAL_WIDTH / VIRTUAL_HEIGHT;
        let widget_aspect = if h > 0.0 { w / h } else { target };
        let scale = if widget_aspect >= target {
            h / VIRTUAL_HEIGHT
        } else {
            w / VIRTUAL_WIDTH
        };
        let game_w = VIRTUAL_WIDTH * scale;
        let game_h = VIRTUAL_HEIGHT * scale;
        Letterbox {
            scale,
            offset_x: (w - game_w) * 0.5,
            offset_y: (h - game_h) * 0.5,
            game_h,
        }
    }

    /// Map an event point (widget-local Y-up pixels) to JS-style logical
    /// coordinates (0..VIRTUAL_WIDTH, 0..VIRTUAL_HEIGHT, Y-down).
    #[allow(dead_code)] // M2-G wires this up
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

impl Default for GameWidget {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for GameWidget {
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
        let lb = self.letterbox();
        let time_seconds = self.epoch.elapsed().as_secs_f32();

        ctx.save();
        // Map JS Y-down logical (0..W, 0..H) → widget Y-up letterboxed pixels.
        ctx.translate(lb.offset_x as f64, (lb.offset_y + lb.game_h) as f64);
        ctx.scale(lb.scale as f64, -(lb.scale as f64));

        // Draw order matches the JS `render(ctx, state)` function exactly.
        scene::paint_background_and_grid(ctx);
        scene::paint_border(ctx);

        for b in &self.world.solid_bubbles {
            scene::paint_bubble(ctx, b, false);
        }
        for d in &self.world.dead_viruses {
            scene::paint_dead_virus(ctx, d);
        }
        for d in &self.world.dying_viruses {
            scene::paint_dying_virus(ctx, d, time_seconds);
        }
        if let Some(g) = &self.world.growing {
            scene::paint_growing_bubble(ctx, g);
        }
        for v in &self.world.viruses {
            scene::paint_virus(ctx, v, time_seconds);
        }
        for p in &self.world.pop_animations {
            scene::paint_pop_animation(ctx, p);
        }

        ctx.restore();
    }

    fn on_event(&mut self, _event: &Event) -> EventResult {
        // M2-G wires growBubble + pointer handlers here.
        EventResult::Ignored
    }
}
