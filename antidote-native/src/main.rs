//! Native shell for the Antidote game.
//!
//! - winit 0.30 owns the window + event loop.
//! - agg-gui paints into an in-memory `Framebuffer` (RGBA8 bottom-up).
//! - wgpu presents that framebuffer as a fullscreen RGBA8 texture.

#![allow(deprecated)] // matches the agg-gui demo-native winit 0.30 idiom

use std::sync::Arc;
use std::time::Instant;

use agg_gui::{winit_adapter, App, Framebuffer, GfxCtx, Modifiers, Size};
use antidote_core::ui::game_widget::GameWidget;
use wgpu::util::DeviceExt;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, Event, MouseScrollDelta, WindowEvent};
use winit::event_loop::EventLoop;
use winit::window::{Window, WindowAttributes};

mod platform;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct BlitVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

const FULLSCREEN_QUAD: [BlitVertex; 6] = [
    BlitVertex {
        position: [-1.0, -1.0],
        uv: [0.0, 0.0],
    },
    BlitVertex {
        position: [1.0, -1.0],
        uv: [1.0, 0.0],
    },
    BlitVertex {
        position: [1.0, 1.0],
        uv: [1.0, 1.0],
    },
    BlitVertex {
        position: [-1.0, -1.0],
        uv: [0.0, 0.0],
    },
    BlitVertex {
        position: [1.0, 1.0],
        uv: [1.0, 1.0],
    },
    BlitVertex {
        position: [-1.0, 1.0],
        uv: [0.0, 1.0],
    },
];

struct FrameTexture {
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

struct Gpu {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vertex_buffer: wgpu::Buffer,
    frame_texture: Option<FrameTexture>,
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

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("antidote-frame-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("antidote-frame-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("antidote-frame-blit-shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) uv: vec2<f32>) -> VsOut {
    var out: VsOut;
    out.position = vec4<f32>(position, 0.0, 1.0);
    out.uv = uv;
    return out;
}

@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(frame_texture, frame_sampler, in.uv);
}
"#
                .into(),
            ),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("antidote-frame-pipeline-layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("antidote-frame-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<BlitVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                    ],
                }],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("antidote-frame-quad"),
            contents: bytemuck::cast_slice(&FULLSCREEN_QUAD),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self {
            device: Arc::new(device),
            queue: Arc::new(queue),
            surface,
            config,
            pipeline,
            bind_group_layout,
            sampler,
            vertex_buffer,
            frame_texture: None,
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

    fn upload_framebuffer(&mut self, framebuffer: &Framebuffer) {
        let width = framebuffer.width();
        let height = framebuffer.height();
        if width == 0 || height == 0 {
            return;
        }
        let needs_texture = match self.frame_texture.as_ref() {
            Some(tex) => tex.width != width || tex.height != height,
            None => true,
        };
        if needs_texture {
            self.frame_texture = Some(self.create_frame_texture(width, height));
        }
        let Some(frame_texture) = self.frame_texture.as_ref() else {
            return;
        };
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &frame_texture.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            framebuffer.pixels(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn create_frame_texture(&self, width: u32, height: u32) -> FrameTexture {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("antidote-frame-texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("antidote-frame-bind-group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        FrameTexture {
            texture,
            bind_group,
            width,
            height,
        }
    }

    fn present(&self) {
        let Some(frame_texture) = self.frame_texture.as_ref() else {
            return;
        };
        let Some(surface_frame) = acquire_frame(self) else {
            return;
        };
        let view = surface_frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("antidote-frame-encoder"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("antidote-frame-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &frame_texture.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.draw(0..FULLSCREEN_QUAD.len() as u32, 0..1);
        }
        self.queue.submit(std::iter::once(encoder.finish()));
        surface_frame.present();
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

    let mut gpu = Gpu::new(window.clone());

    let mut framebuffer = Framebuffer::new(800, 600);
    let mut app = App::new(Box::new(GameWidget::new()));

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
                framebuffer.resize(win_w, win_h);
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
                paint_frame(&mut framebuffer, &mut app, &mut gpu, win_w, win_h);
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

fn paint_frame(
    framebuffer: &mut Framebuffer,
    app: &mut App,
    gpu: &mut Gpu,
    win_w: u32,
    win_h: u32,
) {
    if win_w == 0 || win_h == 0 {
        return;
    }
    if framebuffer.width() != win_w || framebuffer.height() != win_h {
        framebuffer.resize(win_w, win_h);
    }
    // No explicit clear — `paint_background_and_grid` covers the full
    // letterboxed game area, and the letterbox bars are left untouched
    // (they stay whatever the previous frame had, which is fine because
    // the framebuffer was zeroed at allocation time).

    {
        let mut ctx = GfxCtx::new(framebuffer);
        app.layout(Size::new(win_w as f64, win_h as f64));
        app.paint(&mut ctx);
    }

    gpu.upload_framebuffer(framebuffer);
    gpu.present();
}

fn acquire_frame(gpu: &Gpu) -> Option<wgpu::SurfaceTexture> {
    match gpu.surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(f) | wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
            Some(f)
        }
        _ => None,
    }
}
