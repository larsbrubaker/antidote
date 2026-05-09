//! Native shell — winit + wgpu event loop wrapping `agg_gui::App` and the antidote `GameWidget`.
//! Stub; M2 wires up the real event loop.

use antidote_core::ui::game_widget::GameWidget;

fn main() {
    let _ = dotenvy::dotenv();
    let _widget = GameWidget::new();
    println!("antidote-native: M1 stub — full winit/wgpu loop arrives in M2.");
}
