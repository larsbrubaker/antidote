//! Native shell for the Antidote game.
//!
//! - winit 0.30 owns the window + event loop.
//! - agg-gui paints into an in-memory `Framebuffer` (RGBA8 bottom-up).
//! - softbuffer presents the framebuffer to the OS (BGRA8 top-down).
//!
//! This shell intentionally avoids wgpu: the game's draw cost is dominated by
//! rapier physics + scene paint, and a software path keeps native and wasm
//! shells close in shape (the wasm shell will use the same `Framebuffer` →
//! canvas `putImageData` flow).

#![allow(deprecated)] // matches the agg-gui demo-native winit 0.30 idiom

use std::num::NonZeroU32;
use std::rc::Rc;

use agg_gui::{winit_adapter, App, Framebuffer, GfxCtx, Modifiers, Size};
use antidote_core::ui::game_widget::GameWidget;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes};

mod platform;

fn main() {
    let _ = dotenvy::dotenv();

    let event_loop = EventLoop::new().expect("create event loop");

    let window_attributes = WindowAttributes::default()
        .with_title("Antidote")
        .with_inner_size(LogicalSize::new(800, 600));

    let window = Rc::new(
        event_loop
            .create_window(window_attributes)
            .expect("create window"),
    );
    agg_gui::set_device_scale(window.scale_factor());

    let context = softbuffer::Context::new(window.clone()).expect("softbuffer ctx");
    let mut surface =
        softbuffer::Surface::new(&context, window.clone()).expect("softbuffer surface");

    let mut framebuffer = Framebuffer::new(800, 600);
    let mut app = App::new(Box::new(GameWidget::new()));

    let mut win_w = window.inner_size().width.max(1);
    let mut win_h = window.inner_size().height.max(1);
    let mut cursor_x = 0.0_f64;
    let mut cursor_y = 0.0_f64;
    let mut current_mods = Modifiers::default();

    event_loop
        .run(move |event, elwt| match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => elwt.exit(),

            Event::WindowEvent {
                event: WindowEvent::Resized(size),
                ..
            } => {
                if size.width > 0 && size.height > 0 {
                    win_w = size.width;
                    win_h = size.height;
                    framebuffer.resize(win_w, win_h);
                    window.request_redraw();
                }
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
                paint_frame(&mut framebuffer, &mut app, &mut surface, win_w, win_h);
            }

            Event::AboutToWait => {
                // Continuous animation — keep redrawing.
                window.request_redraw();
            }

            _ => {}
        })
        .expect("event loop");
}

fn paint_frame(
    framebuffer: &mut Framebuffer,
    app: &mut App,
    surface: &mut softbuffer::Surface<Rc<Window>, Rc<Window>>,
    win_w: u32,
    win_h: u32,
) {
    if win_w == 0 || win_h == 0 {
        return;
    }
    if framebuffer.width() != win_w || framebuffer.height() != win_h {
        framebuffer.resize(win_w, win_h);
    }
    // Clear to opaque black before each frame; the scene paints over it.
    for chunk in framebuffer.pixels_mut().chunks_exact_mut(4) {
        chunk[0] = 0;
        chunk[1] = 0;
        chunk[2] = 0;
        chunk[3] = 255;
    }

    {
        let mut ctx = GfxCtx::new(framebuffer);
        app.layout(Size::new(win_w as f64, win_h as f64));
        app.paint(&mut ctx);
    }

    let Some(buf_w) = NonZeroU32::new(win_w) else {
        return;
    };
    let Some(buf_h) = NonZeroU32::new(win_h) else {
        return;
    };
    if surface.resize(buf_w, buf_h).is_err() {
        return;
    }
    let Ok(mut out) = surface.buffer_mut() else {
        return;
    };

    blit_rgba_y_up_to_bgra_y_down(
        framebuffer.pixels(),
        &mut out,
        win_w as usize,
        win_h as usize,
    );
    let _ = out.present();
}

/// Convert agg-gui's RGBA8 bottom-up framebuffer into softbuffer's BGRA8
/// top-down (alpha byte ignored / written as 0). Each pixel becomes a u32
/// in 0x00RRGGBB layout because softbuffer expects little-endian XRGB.
fn blit_rgba_y_up_to_bgra_y_down(src: &[u8], dst: &mut [u32], width: usize, height: usize) {
    debug_assert_eq!(src.len(), width * height * 4);
    debug_assert_eq!(dst.len(), width * height);
    for y in 0..height {
        let src_row = (height - 1 - y) * width * 4;
        let dst_row = y * width;
        for x in 0..width {
            let s = src_row + x * 4;
            let r = src[s] as u32;
            let g = src[s + 1] as u32;
            let b = src[s + 2] as u32;
            // softbuffer little-endian XRGB layout: 0x00RRGGBB.
            dst[dst_row + x] = (r << 16) | (g << 8) | b;
        }
    }
}
