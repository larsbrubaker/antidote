#![cfg(target_arch = "wasm32")]
//! WASM shell for Antidote — browser canvas + wgpu/WebGL rendering.
//!
//! # Platform-split policy (kept identical across `antidote-native`, `antidote-wasm`)
//!
//! This crate is a **platform shell only** — canvas, browser events,
//! `localStorage` persistence, and wasm-bindgen exports. It contains **no game
//! or UI content**: every game rule, widget tree, menu, layout, and interface
//! the user sees is shared via `antidote-core` (game logic + widget tree) and
//! `agg-gui-wgpu` (agg-gui's wgpu rendering crate).
//!
//! - **Game / widget / layout code** → `antidote-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `agg-gui-wgpu`
//! - **Platform shell (canvas + event forwarding + persistence backend)** →
//!   here and `antidote-native`
//!
//! `agg-gui-wgpu` targets WebGL2 via wgpu on `wasm32-unknown-unknown` (no WebGPU
//! dependency), so the game runs on every modern browser with WebGL2 support
//! once the WASM render loop is wired.
//!
//! WASM exports planned for Phase 2:
//! - `start()` — initialize panic hooks and browser state
//! - `render(width, height, frame_ms)` — full-frame render
//! - `on_pointer_down/move/up/cancel` — pointer events
//! - `request_redraw()` / `needs_draw()` — JS animation loop coordination

mod platform;

use std::cell::{Cell, RefCell};
use std::sync::Arc;

use agg_gui::{App, Key, Modifiers, MouseButton};
use agg_gui_wgpu::WgpuGfxCtx;
use antidote_core::ui::{build_antidote_app_with_store, game_model::SharedModel};
use platform::LocalStorageSettingsStore;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    /// Top-level handle to the shared GameModel — needed by the export/import
    /// bindings that have to read/write `settings` outside the widget tree.
    /// The widgets each hold their own clone, so this is a cheap extra
    /// reference (single `Rc<RefCell<…>>` clone).
    static MODEL: RefCell<Option<SharedModel>> = const { RefCell::new(None) };
    static WGPU_INIT: RefCell<Option<WgpuInit>> = const { RefCell::new(None) };
    static WGPU_CTX: RefCell<Option<WgpuGfxCtx>> = const { RefCell::new(None) };
    static NEEDS_DRAW: Cell<bool> = const { Cell::new(true) };
    static MOUSE_BUTTONS_DOWN: Cell<u32> = const { Cell::new(0) };
}

struct WgpuInit {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    config: wgpu::SurfaceConfiguration,
}

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    // Build the App + SharedModel synchronously so the first render() finds
    // a populated MODEL. This doesn't need wgpu — `build_antidote_app_with_store`
    // only constructs widgets and the GameModel.
    ensure_app();
    wasm_bindgen_futures::spawn_local(async {
        match init_wgpu_async().await {
            Ok(init) => WGPU_INIT.with(|c| *c.borrow_mut() = Some(init)),
            Err(err) => {
                web_sys::console::error_1(&JsValue::from_str(&format!("wgpu init failed: {err}")));
            }
        }
        mark_dirty();
    });
}

#[derive(Debug)]
struct WebDisplay;

impl wgpu::rwh::HasDisplayHandle for WebDisplay {
    fn display_handle(&self) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
        Ok(wgpu::rwh::DisplayHandle::web())
    }
}

async fn init_wgpu_async() -> Result<WgpuInit, String> {
    let document = web_sys::window()
        .ok_or("no global window")?
        .document()
        .ok_or("no document")?;
    let canvas = document
        .get_element_by_id("antidote-canvas")
        .ok_or("#antidote-canvas element not found")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .map_err(|_| "#antidote-canvas is not a canvas")?;

    let mut instance_desc = wgpu::InstanceDescriptor::new_with_display_handle(Box::new(WebDisplay));
    instance_desc.backends = wgpu::Backends::GL;
    let instance = wgpu::Instance::new(instance_desc);
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
        .map_err(|err| format!("create_surface: {err:?}"))?;

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .await
        .map_err(|err| format!("request_adapter: {err:?}"))?;

    // Start from the WebGL2 baseline (max 2048 textures) and raise just the
    // texture-dimension / buffer-size limits to whatever the adapter actually
    // supports. Most browsers report 4096 or higher (Pixel + modern desktops
    // typically 8192–16384), so without this lift the surface configure
    // panics any time `viewport × DPR` exceeds 2048 on either axis.
    let adapter_limits = adapter.limits();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("antidote-wasm-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                .using_resolution(adapter_limits),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|err| format!("request_device: {err:?}"))?;

    let caps = surface.get_capabilities(&adapter);
    let surface_format = caps
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(caps.formats[0]);

    let config = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: canvas.width().max(1),
        height: canvas.height().max(1),
        present_mode: wgpu::PresentMode::AutoVsync,
        desired_maximum_frame_latency: 2,
        alpha_mode: caps.alpha_modes[0],
        view_formats: vec![],
    };
    surface.configure(&device, &config);

    Ok(WgpuInit {
        device: Arc::new(device),
        queue: Arc::new(queue),
        surface,
        surface_format,
        config,
    })
}

fn ensure_app() {
    APP.with(|cell| {
        if cell.borrow().is_some() {
            return;
        }
        let (app, model) = build_antidote_app_with_store(LocalStorageSettingsStore::into_shared());
        *cell.borrow_mut() = Some(app);
        MODEL.with(|m| *m.borrow_mut() = Some(model));
    });
}

/// Cheap clone of the shared model handle for use inside the export/import
/// bindings. Pulls the `Rc<RefCell<…>>` out of the `MODEL` thread_local so
/// we don't tangle nested `Ref` borrows with `borrow_mut()` on the inner
/// cell — that confuses the borrow checker on the wasm32 target.
fn shared_model_clone() -> Option<SharedModel> {
    MODEL.with(|cell| cell.borrow().clone())
}

/// Called by the JS shell each frame. If the File → Export menu set
/// `model.pending_export`, returns the JSON to download (and clears the
/// flag); otherwise returns `null`. The JS side wraps the string in a Blob
/// and offers it as `antidote-save.json`.
#[wasm_bindgen]
pub fn drain_pending_export() -> Option<String> {
    let model = shared_model_clone()?;
    let mut m = model.borrow_mut();
    if !m.pending_export {
        return None;
    }
    m.pending_export = false;
    Some(m.export_settings_json())
}

/// Called by the JS shell each frame. If the File → Import menu set
/// `model.pending_import`, returns `true` exactly once so the JS side
/// opens a file picker on the user-gesture frame the click happened on.
#[wasm_bindgen]
pub fn drain_pending_import() -> bool {
    let Some(model) = shared_model_clone() else {
        return false;
    };
    let mut m = model.borrow_mut();
    if !m.pending_import {
        return false;
    }
    m.pending_import = false;
    true
}

/// Replace the local settings with the JSON the JS side read from the
/// imported file. Returns `false` on parse failure so the shell can
/// surface an error.
#[wasm_bindgen]
pub fn apply_settings_json(json: String) -> bool {
    let Some(model) = shared_model_clone() else {
        return false;
    };
    let mut m = model.borrow_mut();
    m.apply_settings_json(&json)
}

/// Called by the JS shell each frame. If the menu bar's Fullscreen button
/// set `model.pending_fullscreen_toggle`, returns `true` exactly once so
/// the JS side can call `requestFullscreen` / `exitFullscreen` on the very
/// next frame — still inside the user-gesture window from the click.
#[wasm_bindgen]
pub fn drain_pending_fullscreen_toggle() -> bool {
    let Some(model) = shared_model_clone() else {
        return false;
    };
    let mut m = model.borrow_mut();
    if !m.pending_fullscreen_toggle {
        return false;
    }
    m.pending_fullscreen_toggle = false;
    true
}

/// Called by the JS shell each frame. If starting/resuming a game on mobile
/// set `model.pending_enter_fullscreen`, returns `true` exactly once so the
/// JS side can call `requestFullscreen` and then
/// `screen.orientation.lock('landscape')` — still inside the user-gesture
/// window from the tap. Enter-only, unlike the Fullscreen button's toggle.
#[wasm_bindgen]
pub fn drain_pending_enter_fullscreen() -> bool {
    let Some(model) = shared_model_clone() else {
        return false;
    };
    let mut m = model.borrow_mut();
    if !m.pending_enter_fullscreen {
        return false;
    }
    m.pending_enter_fullscreen = false;
    true
}

/// Called by the JS shell each frame. If a widget set
/// `model.pending_open_url` (currently the Help panel's SOURCE repo link),
/// returns the URL exactly once so the JS side can `window.open` it in a
/// new tab — still inside the user-gesture window from the click, which
/// popup blockers require.
#[wasm_bindgen]
pub fn drain_pending_open_url() -> Option<String> {
    let model = shared_model_clone()?;
    let mut m = model.borrow_mut();
    m.pending_open_url.take()
}

fn ensure_wgpu_ctx(width: f32, height: f32) {
    WGPU_CTX.with(|ctx_cell| {
        if ctx_cell.borrow().is_some() {
            return;
        }
        WGPU_INIT.with(|init_cell| {
            let init = init_cell.borrow();
            let Some(init) = init.as_ref() else {
                return;
            };
            *ctx_cell.borrow_mut() = Some(WgpuGfxCtx::new(
                Arc::clone(&init.device),
                Arc::clone(&init.queue),
                init.surface_format,
                width,
                height,
            ));
        });
    });
}

fn resize_surface_if_needed(width: u32, height: u32) {
    if width == 0 || height == 0 {
        return;
    }
    WGPU_INIT.with(|cell| {
        let mut init = cell.borrow_mut();
        let Some(init) = init.as_mut() else {
            return;
        };
        if init.config.width != width || init.config.height != height {
            init.config.width = width;
            init.config.height = height;
            init.surface.configure(&init.device, &init.config);
        }
    });
}

#[wasm_bindgen]
pub fn render(width: u32, height: u32, _frame_ms: f64) {
    if !WGPU_INIT.with(|cell| cell.borrow().is_some()) {
        return;
    }
    ensure_app();
    ensure_wgpu_ctx(width as f32, height as f32);
    resize_surface_if_needed(width, height);

    let frame = WGPU_INIT.with(|cell| {
        let init = cell.borrow();
        let init = init.as_ref()?;
        match init.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            _ => None,
        }
    });
    let Some(frame) = frame else {
        return;
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    WGPU_CTX.with(|ctx_cell| {
        let mut ctx = ctx_cell.borrow_mut();
        let Some(ctx) = ctx.as_mut() else {
            return;
        };
        ctx.set_surface_texture(frame.texture.clone());
        ctx.reset(width as f32, height as f32);
        ctx.set_lcd_mode(agg_gui::font_settings::lcd_enabled());
        ctx.begin_frame(view);
        APP.with(|app_cell| {
            let mut app = app_cell.borrow_mut();
            if let Some(app) = app.as_mut() {
                // Fixed 1280×720 design canvas: fit it to the backing-store
                // size every frame (CanvasRoot centers the slack).
                agg_gui::ux_scale::set_ux_scale(antidote_core::ui::fixed_canvas_ux_scale(
                    width as f64,
                    height as f64,
                ));
                app.layout(agg_gui::Size::new(width as f64, height as f64));
                app.paint(ctx);
            }
        });
        ctx.end_frame();
    });
    frame.present();
    NEEDS_DRAW.with(|cell| cell.set(false));
}

#[wasm_bindgen]
pub fn set_device_pixel_ratio(dpr: f64) {
    agg_gui::set_device_scale(dpr.max(0.5));
    // Mirror the native shell — LCD subpixel text rendering on standard-DPI
    // displays. Without this, the grayscale outline path emits non-AA text.
    agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);
    mark_dirty();
}

/// Called by the JS shell at startup (and if the primary pointer type ever
/// changes) with the result of `matchMedia("(pointer: coarse)")`. A coarse
/// pointer means a phone/tablet — the core UI shows the rotate-to-landscape
/// overlay in portrait and enters fullscreen when a game starts.
#[wasm_bindgen]
pub fn set_environment(is_mobile: bool) {
    ensure_app();
    let Some(model) = shared_model_clone() else {
        return;
    };
    model.borrow_mut().is_mobile = is_mobile;
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_move(x: f64, y: f64) {
    ensure_app();
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_move(x, y);
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_down(x: f64, y: f64, button: u8) {
    ensure_app();
    MOUSE_BUTTONS_DOWN.set(MOUSE_BUTTONS_DOWN.get().saturating_add(1));
    let btn = mouse_button(button);
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_down(x, y, btn, Modifiers::default());
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn on_mouse_up(x: f64, y: f64, button: u8) {
    ensure_app();
    MOUSE_BUTTONS_DOWN.set(MOUSE_BUTTONS_DOWN.get().saturating_sub(1));
    let btn = mouse_button(button);
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_up(x, y, btn, Modifiers::default());
        }
    });
    mark_dirty();
}

/// Browser keyboard input → agg-gui. Returns `true` if any focused
/// widget consumed the event so the TS shell can call `preventDefault()`
/// (otherwise Tab navigation, arrow-key scrolling, etc. would fight the
/// in-game text fields).
///
/// `key` is the raw `KeyboardEvent.key` string (`"a"`, `"Backspace"`,
/// `"ArrowLeft"`, …). Modifiers come through as four booleans, matching
/// the `KeyboardEvent` properties.
#[wasm_bindgen]
pub fn on_key_down(key: String, shift: bool, ctrl: bool, alt: bool, meta: bool) -> bool {
    ensure_app();
    let Some(parsed) = parse_browser_key(&key) else {
        return false;
    };
    let mods = Modifiers {
        shift,
        ctrl,
        alt,
        meta,
    };
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_key_down(parsed, mods);
        }
    });
    mark_dirty();
    // Always claim the event when we forwarded it — agg-gui's App may not
    // surface a per-event "consumed" bit yet, but every key we recognize
    // is one we want the browser to leave alone (text-field typing, Esc/P
    // pause, etc.).
    true
}

#[wasm_bindgen]
pub fn on_key_up(key: String, shift: bool, ctrl: bool, alt: bool, meta: bool) {
    let Some(parsed) = parse_browser_key(&key) else {
        return;
    };
    let mods = Modifiers {
        shift,
        ctrl,
        alt,
        meta,
    };
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_key_up(parsed, mods);
        }
    });
    mark_dirty();
}

/// Map a browser `KeyboardEvent.key` string to agg-gui's [`Key`] enum.
/// Returns `None` for keys we deliberately don't forward (Shift/Control
/// alone, F-keys, etc.) so the browser keeps its default behaviour.
fn parse_browser_key(key: &str) -> Option<Key> {
    match key {
        "Backspace" => Some(Key::Backspace),
        "Delete" => Some(Key::Delete),
        "Insert" => Some(Key::Insert),
        "ArrowLeft" => Some(Key::ArrowLeft),
        "ArrowRight" => Some(Key::ArrowRight),
        "ArrowUp" => Some(Key::ArrowUp),
        "ArrowDown" => Some(Key::ArrowDown),
        "Home" => Some(Key::Home),
        "End" => Some(Key::End),
        "Tab" => Some(Key::Tab),
        "Enter" => Some(Key::Enter),
        "Escape" => Some(Key::Escape),
        // Single visible character → Char(c). Anything multi-char and not
        // in the allow-list above is something we don't model (modifier-
        // only keys, F-keys, "PageUp", etc.) — let the browser handle it.
        s if s.chars().count() == 1 => s.chars().next().map(Key::Char),
        _ => None,
    }
}

#[wasm_bindgen]
pub fn on_mouse_leave() {
    APP.with(|cell| {
        if let Some(app) = cell.borrow_mut().as_mut() {
            app.on_mouse_leave();
        }
    });
    mark_dirty();
}

#[wasm_bindgen]
pub fn needs_draw() -> bool {
    if NEEDS_DRAW.with(|cell| cell.get()) {
        return true;
    }
    APP.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|app| app.wants_draw())
            .unwrap_or(true)
    })
}

fn mouse_button(button: u8) -> MouseButton {
    match button {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        other => MouseButton::Other(other),
    }
}

fn mark_dirty() {
    NEEDS_DRAW.with(|cell| cell.set(true));
}
