//! Shared immediate-mode painting helpers for the Petri Pop widgets.
//!
//! These wrap `DrawCtx` for the design language's recurring elements: hard
//! shadows (a solid offset rounded rect — never a blur), tracked uppercase
//! labels, and thousands-separated numbers. Letter-spacing is applied here
//! by advancing per character rather than through
//! `font_settings::set_interval`, which is global and bumps the typography
//! epoch (invalidating every cached Label backbuffer) each time it changes.

use agg_gui::{Color, DrawCtx, Rect};

/// `12450` → `"12,450"`.
pub fn fmt_thousands(n: u64) -> String {
    let digits = n.to_string();
    let len = digits.len();
    let mut out = String::with_capacity(len + len / 3);
    for (i, c) in digits.chars().enumerate() {
        if i != 0 && (len - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// Width of `text` at the current font/size with `tracking` px added
/// between characters.
pub fn measure_tracked(ctx: &mut dyn DrawCtx, text: &str, tracking: f64) -> f64 {
    let base = ctx.measure_text(text).map(|m| m.width).unwrap_or(0.0);
    let gaps = text.chars().count().saturating_sub(1) as f64;
    base + gaps * tracking
}

/// Draw `text` left-aligned at `(x, baseline_y)` with `tracking` px between
/// characters. Zero tracking falls through to a single `fill_text` call.
pub fn fill_text_tracked(ctx: &mut dyn DrawCtx, text: &str, x: f64, y: f64, tracking: f64) {
    if tracking.abs() < 1e-6 {
        ctx.fill_text(text, x, y);
        return;
    }
    let mut pen_x = x;
    let mut buf = [0u8; 4];
    for c in text.chars() {
        let s = c.encode_utf8(&mut buf);
        ctx.fill_text(s, pen_x, y);
        let advance = ctx.measure_text(s).map(|m| m.width).unwrap_or(0.0);
        pen_x += advance + tracking;
    }
}

/// Draw `text` centered on `cx` at `baseline_y` with tracking.
pub fn fill_text_centered(ctx: &mut dyn DrawCtx, text: &str, cx: f64, y: f64, tracking: f64) {
    let w = measure_tracked(ctx, text, tracking);
    fill_text_tracked(ctx, text, cx - w * 0.5, y, tracking);
}

/// Draw `text` right-aligned so it ends at `right_x`.
pub fn fill_text_right(ctx: &mut dyn DrawCtx, text: &str, right_x: f64, y: f64, tracking: f64) {
    let w = measure_tracked(ctx, text, tracking);
    fill_text_tracked(ctx, text, right_x - w, y, tracking);
}

/// A raised surface in the design language: solid hard-shadow rect offset
/// `drop` px straight down (Y-up: below means lower y), then the fill on
/// top, then an optional 1px border stroke.
pub fn raised_rect(
    ctx: &mut dyn DrawCtx,
    r: Rect,
    radius: f64,
    drop: f64,
    fill: Color,
    shadow: Color,
    border: Option<Color>,
) {
    if drop > 0.0 {
        ctx.set_fill_color(shadow);
        ctx.begin_path();
        ctx.rounded_rect(r.x, r.y - drop, r.width, r.height, radius);
        ctx.fill();
    }
    ctx.set_fill_color(fill);
    ctx.begin_path();
    ctx.rounded_rect(r.x, r.y, r.width, r.height, radius);
    ctx.fill();
    if let Some(b) = border {
        ctx.set_stroke_color(b);
        ctx.set_line_width(1.0);
        ctx.begin_path();
        ctx.rounded_rect(r.x + 0.5, r.y + 0.5, r.width - 1.0, r.height - 1.0, radius);
        ctx.stroke();
    }
}

#[cfg(test)]
mod tests {
    use super::fmt_thousands;

    #[test]
    fn thousands_grouping() {
        assert_eq!(fmt_thousands(0), "0");
        assert_eq!(fmt_thousands(999), "999");
        assert_eq!(fmt_thousands(1000), "1,000");
        assert_eq!(fmt_thousands(12450), "12,450");
        assert_eq!(fmt_thousands(1234567), "1,234,567");
    }
}
