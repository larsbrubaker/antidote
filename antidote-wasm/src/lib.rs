#![cfg(target_arch = "wasm32")]
//! WASM shell for Antidote — browser canvas + wgpu/WebGL rendering.
//!
//! # Platform-split policy (kept identical across `antidote-native`, `antidote-wasm`)
//!
//! This crate is a **platform shell only** — canvas, browser events,
//! `localStorage` persistence, and wasm-bindgen exports. It contains **no game
//! or UI content**: every game rule, widget tree, menu, layout, and interface
//! the user sees is shared via `antidote-core` (game logic + widget tree) and
//! `demo-wgpu` (the wgpu rendering library shared with agg-gui).
//!
//! - **Game / widget / layout code** → `antidote-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `demo-wgpu`
//! - **Platform shell (canvas + event forwarding + persistence backend)** →
//!   here and `antidote-native`
//!
//! `demo-wgpu` targets WebGL2 via wgpu on `wasm32-unknown-unknown` (no WebGPU
//! dependency), so the game runs on every modern browser with WebGL2 support
//! once the WASM render loop is wired.
//!
//! WASM exports planned for Phase 2:
//! - `start()` — initialize panic hooks and browser state
//! - `render(width, height, frame_ms)` — full-frame render
//! - `on_pointer_down/move/up/cancel` — pointer events
//! - `request_redraw()` / `needs_draw()` — JS animation loop coordination

mod platform;

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    web_sys::console::log_1(
        &"antidote-wasm: platform shell ready; shared UI lives in antidote-core.".into(),
    );
}
