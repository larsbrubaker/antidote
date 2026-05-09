//! Native shell for the Antidote game.
//!
//! # Platform-split policy (kept identical across `antidote-native`, `antidote-wasm`)
//!
//! This crate is a **platform shell only** — it wires up the OS window
//! (winit + wgpu surface), the event loop, input forwarding, and native
//! persistence. It contains **no game or UI content**: every game rule, widget
//! tree, menu, layout, and interface the user sees is shared via
//! `antidote-core` (game logic + widget tree) and `demo-wgpu` (the wgpu
//! rendering library shared with agg-gui).
//!
//! - **Game / widget / layout code** → `antidote-core`
//! - **GPU renderers (WGSL shaders, geometry, draw calls)** → `demo-wgpu`
//! - **Platform shell (OS window + event forwarding + persistence backend)** →
//!   here and `antidote-wasm`
//!
//! # Scope
//!
//! Currently covers: window creation, wgpu device/surface init, per-frame flush
//! via `WgpuGfxCtx::end_frame`, resize, mouse/keyboard/wheel input forwarding,
//! and disk-backed state persistence. Future screens, menus, HUDs, dialogs,
//! leaderboards, and gameplay UI must be added to `antidote-core`, not here.

#![allow(deprecated)] // matches the agg-gui demo-native winit 0.30 idiom

use std::sync::Arc;
use std::time::Instant;

use agg_gui::{winit_adapter, App, Modifiers, Size};
use antidote_core::ui::build_antidote_app;
use demo_wgpu::{begin_frame, WgpuGfxCtx};
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes};

mod platform;

struct Gpu {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    config: wgpu::SurfaceConfiguration,
}

impl Gpu {
    fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::PRIMARY;
        let instance = wgpu::Instance::new(instance_desc);
        let surface = instance
            .create_surface(window.clone())
            .expect("create wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("request wgpu adapter");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("antidote-native-wgpu"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: wgpu::ExperimentalFeatures::default(),
            trace: wgpu::Trace::Off,
        }))
        .expect("request wgpu device");

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
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
        };
        surface.configure(&device, &config);

        Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface,
            surface_format,
            config,
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.device, &self.config);
    }
}

fn main() {
    let _ = dotenvy::dotenv();

    let event_loop = EventLoop::new().expect("create event loop");

    let window_attributes = WindowAttributes::default()
        .with_title("Antidote")
        .with_inner_size(LogicalSize::new(800, 600));

    let window = Arc::new(
        event_loop
            .create_window(window_attributes)
            .expect("create window"),
    );
    agg_gui::set_device_scale(window.scale_factor());
    // Enable LCD subpixel text rendering on standard-DPI displays (matches
    // agg-gui's demo default). Without this `set_lcd_mode(false)` is forced
    // every frame and `fill_text` falls back to the grayscale outline path,
    // which emits `DrawCommand::Solid` (no AA halo) — producing the chunky
    // text the user reported.
    agg_gui::font_settings::set_lcd_enabled(agg_gui::device_scale() <= 1.25);

    let mut gpu = Gpu::new(window.clone());

    let (mut app, model) = build_antidote_app();
    let mut wgpu_ctx = WgpuGfxCtx::new(
        Arc::clone(&gpu.device),
        Arc::clone(&gpu.queue),
        gpu.surface_format,
        gpu.config.width as f32,
        gpu.config.height as f32,
    );

    let mut win_w = window.inner_size().width.max(1);
    let mut win_h = window.inner_size().height.max(1);
    let mut cursor_x = 0.0_f64;
    let mut cursor_y = 0.0_f64;
    let mut current_mods = Modifiers::default();

    // Light-touch frame-time logger — averages over 30 frames, prints once per
    // ~half second so we can spot regressions without spamming stdout.
    let mut frame_count: u32 = 0;
    let mut frame_window_start = Instant::now();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),

            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } if size.width > 0 && size.height > 0 => {
                win_w = size.width;
                win_h = size.height;
                gpu.resize(win_w, win_h);
                window.request_redraw();
            }

            Event::WindowEvent {
                event: WindowEvent::ScaleFactorChanged { scale_factor, .. },
                ..
            } => {
                agg_gui::set_device_scale(scale_factor);
                window.request_redraw();
            }

            Event::WindowEvent {
                event: WindowEvent::CursorMoved { position, .. },
                ..
            } => {
                cursor_x = position.x;
                cursor_y = position.y;
                app.on_mouse_move(cursor_x, cursor_y);
                winit_adapter::apply_cursor(&window, agg_gui::current_cursor_icon());
            }

            Event::WindowEvent {
                event: WindowEvent::CursorLeft { .. },
                ..
            } => {
                app.on_mouse_leave();
            }

            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(mods_state),
                ..
            } => {
                current_mods = winit_adapter::modifiers(mods_state.state());
            }

            Event::WindowEvent {
                event: WindowEvent::MouseInput { state, button, .. },
                ..
            } => {
                let btn = winit_adapter::mouse_button(button);
                match state {
                    ElementState::Pressed => {
                        app.on_mouse_down(cursor_x, cursor_y, btn, current_mods);
                    }
                    ElementState::Released => {
                        app.on_mouse_up(cursor_x, cursor_y, btn, current_mods);
                    }
                }
            }

            Event::WindowEvent {
                event:
                    WindowEvent::MouseWheel {
                        delta: MouseScrollDelta::LineDelta(dx, dy),
                        ..
                    },
                ..
            } => {
                app.on_mouse_wheel_xy_mods(cursor_x, cursor_y, dx as f64, dy as f64, current_mods);
            }

            Event::WindowEvent {
                event:
                    WindowEvent::KeyboardInput {
                        event: key_event, ..
                    },
                ..
            } => {
                let Some(key) = winit_adapter::key_event(&key_event, current_mods) else {
                    return;
                };
                match key_event.state {
                    ElementState::Pressed => {
                        app.on_key_down(key, current_mods);
                    }
                    ElementState::Released => {
                        app.on_key_up(key, current_mods);
                    }
                }
            }

            Event::WindowEvent {
                event: WindowEvent::RedrawRequested,
                ..
            } => {
                // Drain pending OAuth click → URL into pending_open_url.
                // For native there's no localhost callback yet, so we use a
                // sentinel redirect that lands the user on a Supabase
                // success page; they manually return to the app and (for
                // now) sign in via email/password instead.
                antidote_core::ui::drain_pending_oauth(
                    &model,
                    "https://edupgibalgeqfujfkwmm.supabase.co/auth/v1/callback",
                );
                if let Some(url) = model.borrow_mut().pending_open_url.take() {
                    if let Err(err) = webbrowser::open(&url) {
                        eprintln!("antidote: failed to open browser for OAuth: {err}");
                    }
                }
                paint_frame(&gpu, &mut wgpu_ctx, &mut app, win_w, win_h);
                frame_count += 1;
                if frame_count >= 60 {
                    let avg_ms =
                        frame_window_start.elapsed().as_secs_f64() * 1000.0 / frame_count as f64;
                    eprintln!(
                        "antidote: {avg_ms:.1} ms/frame ({:.0} fps)",
                        1000.0 / avg_ms
                    );
                    frame_count = 0;
                    frame_window_start = Instant::now();
                }
            }

            Event::AboutToWait => {
                // Continuous animation — keep redrawing.
                window.request_redraw();
            }

            _ => {}
        })
        .expect("event loop");
}

fn paint_frame(gpu: &Gpu, ctx: &mut WgpuGfxCtx, app: &mut App, win_w: u32, win_h: u32) {
    if win_w == 0 || win_h == 0 {
        return;
    }
    let Some(frame) = acquire_frame(gpu) else {
        return;
    };
    let view = frame
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());
    ctx.set_surface_texture(frame.texture.clone());
    ctx.reset(win_w as f32, win_h as f32);
    ctx.set_lcd_mode(agg_gui::font_settings::lcd_enabled());
    begin_frame(ctx, view);
    app.layout(Size::new(win_w as f64, win_h as f64));
    app.paint(ctx);
    ctx.end_frame();
    frame.present();
}

fn acquire_frame(gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
    match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            Some(f)
        }
        _ => None,
    }
}
