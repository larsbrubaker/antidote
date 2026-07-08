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

/// Font size + line height for [`HelpOverlay::paint_rich`], bundled so the
/// method stays under clippy's argument-count limit.
struct RichTextStyle {
    font_size: f64,
    line_h: f64,
}

pub struct HelpOverlay {
    bounds: Rect,
    children: Vec<Box<dyn Widget>>,
    model: SharedModel,
    fonts: Fonts,
    buttons: ButtonSet,
    /// Hit rect of the SOURCE repo link, in widget-local Y-up coords.
    /// Updated each paint (the link's baseline depends on how the
    /// paragraphs above it wrapped, which only paint knows).
    link_rect: Rect,
    link_hover: bool,
}

impl HelpOverlay {
    pub fn new(model: SharedModel, fonts: Fonts) -> Self {
        Self {
            bounds: Rect::default(),
            children: Vec::new(),
            model,
            fonts,
            buttons: ButtonSet::default(),
            link_rect: Rect::default(),
            link_hover: false,
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
        style: RichTextStyle,
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
            ctx.set_font_size(style.font_size);
            ctx.set_fill_color(color);
            for word in seg.0.split_inclusive(' ') {
                let w = ctx.measure_text(word).map(|m| m.width).unwrap_or(0.0);
                if pen_x + w > x + max_w && pen_x > x {
                    pen_x = x;
                    baseline -= style.line_h;
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
            // Y-up: low offset = near the panel's bottom edge, clear of the
            // SOURCE line above it.
            rect: Rect::new(
                panel.x + (PANEL_W - 200.0) * 0.5,
                panel.y + 12.0,
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
        let after_p1 = self.paint_rich(
            ctx,
            &p1,
            x,
            panel.y + PANEL_H - 120.0,
            text_w,
            RichTextStyle {
                font_size: 20.0,
                line_h: 32.0,
            },
        );

        let p2 = [
            Seg("Every bubble drains your ", SegStyle::Body),
            Seg("antidote", SegStyle::Lime),
            Seg(
                ", and a virus touching a growing bubble costs a ",
                SegStyle::Body,
            ),
            Seg("life", SegStyle::Coral),
            Seg(
                ". Clear the dish to advance. That\u{2019}s it — go save the petri dish.",
                SegStyle::Body,
            ),
        ];
        let after_p2 = self.paint_rich(
            ctx,
            &p2,
            x,
            after_p1 - 44.0,
            text_w,
            RichTextStyle {
                font_size: 20.0,
                line_h: 32.0,
            },
        );

        let src_baseline = after_p2 - 40.0;
        ctx.set_font(self.fonts.bold.clone());
        ctx.set_font_size(13.0);
        ctx.set_fill_color(theme::TEXT_LOW);
        fill_text_tracked(ctx, "SOURCE", x, src_baseline, 2.5);

        // Repo link — hot: brighter + underlined on hover, click opens it.
        let link_x = x + 80.0;
        ctx.set_font(self.fonts.semibold.clone());
        ctx.set_font_size(18.0);
        let link_w = ctx.measure_text(REPO_URL).map(|m| m.width).unwrap_or(0.0);
        // Hit rect wraps the text line: baseline - descender up to cap height.
        self.link_rect = Rect::new(link_x, src_baseline - 6.0, link_w, 26.0);
        let link_color = if self.link_hover {
            theme::LIME_400
        } else {
            theme::LIME_500
        };
        ctx.set_fill_color(link_color);
        fill_text_tracked(ctx, REPO_URL, link_x, src_baseline, 0.0);
        if self.link_hover {
            ctx.set_stroke_color(link_color);
            ctx.set_line_width(1.5);
            ctx.begin_path();
            ctx.move_to(link_x, src_baseline - 4.0);
            ctx.line_to(link_x + link_w, src_baseline - 4.0);
            ctx.stroke();
        }

        self.buttons.paint(ctx, &self.fonts);
    }
    fn on_event(&mut self, event: &Event) -> EventResult {
        if let Some("back") = self.buttons.on_event(event) {
            self.model.borrow_mut().menu_view = MenuView::Main;
            return EventResult::Consumed;
        }
        match event {
            Event::MouseMove { pos } => {
                self.link_hover = self.link_rect.contains(*pos);
            }
            Event::MouseUp {
                pos,
                button: agg_gui::MouseButton::Left,
                ..
            } if self.link_rect.contains(*pos) => {
                self.model.borrow_mut().pending_open_url = Some(format!("https://{REPO_URL}"));
                return EventResult::Consumed;
            }
            _ => {}
        }
        swallow_mouse(event)
    }
    fn needs_draw(&self) -> bool {
        self.is_visible()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agg_gui::geometry::Size;
    use agg_gui::{Framebuffer, GfxCtx, Modifiers, MouseButton, Point};

    /// Full click-through: lay out + paint (which measures the link and sets
    /// its hit rect), then release the mouse on the link. The model must end
    /// up with `pending_open_url` set to the https repo URL — that's the
    /// contract the platform shells' drains rely on.
    #[test]
    fn clicking_source_link_requests_url_open() {
        let model = crate::ui::game_model::shared();
        let mut overlay = HelpOverlay::new(model.clone(), Fonts::load());
        let size = Size::new(theme::APP_W, theme::APP_H);
        overlay.set_bounds(Rect::new(0.0, 0.0, theme::APP_W, theme::APP_H));
        overlay.layout(size);
        let mut fb = Framebuffer::new(theme::APP_W as u32, theme::APP_H as u32);
        let mut ctx = GfxCtx::new(&mut fb);
        overlay.paint(&mut ctx);
        assert!(
            overlay.link_rect.width > 0.0,
            "paint must measure the link and set its hit rect"
        );

        let p = Point::new(
            overlay.link_rect.x + overlay.link_rect.width * 0.5,
            overlay.link_rect.y + overlay.link_rect.height * 0.5,
        );
        overlay.on_event(&Event::MouseMove { pos: p });
        assert!(overlay.link_hover, "hover must arm on the link");
        let result = overlay.on_event(&Event::MouseUp {
            pos: p,
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        });
        assert_eq!(result, EventResult::Consumed);
        assert_eq!(
            model.borrow().pending_open_url.as_deref(),
            Some("https://github.com/larsbrubaker/antidote")
        );
    }

    /// A release outside the link must not request an open.
    #[test]
    fn release_off_link_does_not_open() {
        let model = crate::ui::game_model::shared();
        let mut overlay = HelpOverlay::new(model.clone(), Fonts::load());
        let size = Size::new(theme::APP_W, theme::APP_H);
        overlay.set_bounds(Rect::new(0.0, 0.0, theme::APP_W, theme::APP_H));
        overlay.layout(size);
        let mut fb = Framebuffer::new(theme::APP_W as u32, theme::APP_H as u32);
        let mut ctx = GfxCtx::new(&mut fb);
        overlay.paint(&mut ctx);

        overlay.on_event(&Event::MouseUp {
            pos: Point::new(10.0, 10.0),
            button: MouseButton::Left,
            modifiers: Modifiers::default(),
        });
        assert_eq!(model.borrow().pending_open_url, None);
    }
}
