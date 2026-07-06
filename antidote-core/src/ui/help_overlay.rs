//! `HelpOverlay` — the "HOW TO PLAY" panel (design screen 07), reached from
//! the main menu's HELP chip. Two short paragraphs with lime/coral keyword
//! highlights, the repo URL, and a BACK button.

use agg_gui::geometry::Size;
use agg_gui::{Color, DrawCtx, Event, EventResult, Rect, Widget};

use crate::game::state::Phase;
use crate::theme::{self, Fonts};
use crate::ui::game_model::{MenuView, SharedModel};
use crate::ui::paint_util::fill_text_tracked;
use crate::ui::petri_kit::{
    paint_menu_backdrop, paint_panel, swallow_mouse, ButtonKind, ButtonSet, KitButton,
};

const REPO_URL: &str = "github.com/larsbrubaker/antidote";
const PANEL_W: f64 = 720.0;
const PANEL_H: f64 = 420.0;

/// One colored run inside a wrapped paragraph.
struct Seg(&'static str, SegStyle);
#[derive(Clone, Copy, PartialEq)]
enum SegStyle {
    Body,
    Lime,
    Coral,
}

pub struct HelpOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
}

impl HelpOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
        }
    }

    /// Word-wrap a segmented paragraph downward from `top_baseline` (Y-up),
    /// returning the last line's baseline. Splits on spaces within each
    /// segment; a segment switch never forces a break.
    fn paint_rich(
        &self,
        ctx: &mut dyn DrawCtx,
        segs: &[Seg],
        x: f64,
        top_baseline: f64,
        max_w: f64,
        font_size: f64,
        line_h: f64,
    ) -> f64 {
        let mut pen_x = x;
        let mut baseline = top_baseline;
        for seg in segs {
            let (color, font) = match seg.1 {
                SegStyle::Body => (theme::TEXT_MID, self.fonts.medium.clone()),
                SegStyle::Lime => (theme::LIME_500, self.fonts.bold.clone()),
                SegStyle::Coral => (theme::CORAL_500, self.fonts.bold.clone()),
            };
            ctx.set_font(font);
            ctx.set_font_size(font_size);
            ctx.set_fill_color(color);
            for word in seg.0.split_inclusive(' ') {
                let w = ctx.measure_text(word).map(|m| m.width).unwrap_or(0.0);
                if pen_x + w > x + max_w && pen_x > x {
                    pen_x = x;
                    baseline -= line_h;
                }
                ctx.fill_text(word, pen_x, baseline);
                pen_x += w;
            }
        }
        baseline
    }
}

impl Widget for HelpOverlay {
    fn type_name(&self) -> &'static str {
        "HelpOverlay"
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
        m.world.phase == Phase::Start && m.menu_view == MenuView::Help
    }
    fn layout(&mut self, available: Size) -> Size {
        let panel = Rect::new(
            (available.width - PANEL_W) * 0.5,
            (available.height - PANEL_H) * 0.5,
            PANEL_W,
            PANEL_H,
        );
        self.buttons.clear();
        self.buttons.push(KitButton {
            id: "back",
            rect: Rect::new(
                panel.x + (PANEL_W - 200.0) * 0.5,
                panel.y + 32.0,
                200.0,
                56.0,
            ),
            label: "BACK".into(),
            kind: ButtonKind::Secondary,
            font_size: 19.0,
        });
        available
    }
    fn paint(&mut self, ctx: &mut dyn DrawCtx) {
        let w = self.bounds.width;
        let h = self.bounds.height;
        paint_menu_backdrop(ctx, w, h);
        ctx.set_fill_color(Color::from_rgb8(10, 7, 20).with_alpha(0.6));
        ctx.begin_path();
        ctx.rect(0.0, 0.0, w, h);
        ctx.fill();

        let panel = Rect::new((w - PANEL_W) * 0.5, (h - PANEL_H) * 0.5, PANEL_W, PANEL_H);
        paint_panel(ctx, panel);
        let x = panel.x + 52.0;
        let text_w = PANEL_W - 104.0;

        ctx.set_font(self.fonts.extrabold_italic.clone());
        ctx.set_font_size(40.0);
        ctx.set_fill_color(theme::TEXT_HI);
        fill_text_tracked(ctx, "HOW TO PLAY", x, panel.y + PANEL_H - 74.0, 0.0);

        let p1 = [
            Seg("Viruses are loose in the dish! ", SegStyle::Body),
            Seg("Press and hold", SegStyle::Lime),
            Seg(
                " anywhere to blow a bubble — let go, and anything caught inside is stuck. Keep a virus bubbled for ",
                SegStyle::Body,
            ),
            Seg("3 seconds", SegStyle::Lime),
            Seg(" to cure it.", SegStyle::Body),
        ];
        let after_p1 = self.paint_rich(ctx, &p1, x, panel.y + PANEL_H - 120.0, text_w, 20.0, 32.0);

        let p2 = [
            Seg("Every bubble drains your ", SegStyle::Body),
            Seg("antidote", SegStyle::Lime),
            Seg(", and a virus touching a growing bubble costs a ", SegStyle::Body),
            Seg("life", SegStyle::Coral),
            Seg(
                ". Clear the dish to advance. That\u{2019}s it — go save the petri dish.",
                SegStyle::Body,
            ),
        ];
        let after_p2 = self.paint_rich(ctx, &p2, x, after_p1 - 44.0, text_w, 20.0, 32.0);

        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(13.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_tracked(ctx, "SOURCE", x, after_p2 - 40.0, 2.5);
        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(18.0);
        ctx.set_fill_color(theme::LIME_500);
        fill_text_tracked(ctx, REPO_URL, x + 80.0, after_p2 - 40.0, 0.0);

        self.buttons.paint(ctx, &self.fonts);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some("back") = self.buttons.on_event(event) {
            self.model.borrow_mut().menu_view = MenuView::Main;
            return EventResult::Consumed;
        }
        swallow_mouse(event)
    }
    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}
