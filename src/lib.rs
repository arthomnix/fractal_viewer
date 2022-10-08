#[cfg(target_arch="wasm32")]
use wasm_bindgen::prelude::*;

use std::time::{Duration, Instant};
use imgui::{Condition, FontSource, MouseCursor};
use naga::valid::{Capabilities, ValidationFlags};
use wgpu::{Backend, ShaderSource};
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{WindowBuilder, Window},
};
use winit::dpi::PhysicalPosition;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    scale: f32,
    pad_1: u32,
    centre: [f32; 2],
    iterations: i32,
    pad_2: u32,
}

#[derive(Clone)]
struct UserSettings {
    zoom: f32,
    centre: [f32; 2],
    iterations: i32,
    equation: String,
    prev_equation: String,
    equation_valid: bool,
}

struct InputState {
    lmb_pressed: bool,
    prev_cursor_pos: PhysicalPosition<f64>,
}

struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    render_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    imgui: imgui::Context,
    imgui_platform: imgui_winit_support::WinitPlatform,
    imgui_renderer: imgui_wgpu::Renderer,
    last_frame: Instant,
    last_cursor: Option<MouseCursor>,
    backend: &'static str,
    settings: UserSettings,
    input_state: InputState,
}

fn calculate_scale(size: &winit::dpi::PhysicalSize<u32>, settings: &UserSettings) -> f32 {
    4.0 / settings.zoom / (if size.width < size.height { size.width } else { size.height }) as f32
}

fn calculate_uniforms(size: &winit::dpi::PhysicalSize<u32>, settings: &UserSettings) -> Uniforms {
    let scale = calculate_scale(&size, &settings);
    Uniforms {
        scale,
        pad_1: 0,
        centre: [size.width as f32 / 2.0 * scale - settings.centre[0], size.height as f32 / 2.0 * scale - settings.centre[1]],
        iterations: settings.iterations,
        pad_2: 0,
    }
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let backend = match adapter.get_info().backend {
            Backend::Empty => "Empty",
            Backend::Vulkan => "Vulkan",
            Backend::Metal => "Metal",
            Backend::Dx12 => "DirectX 12",
            Backend::Dx11 => "DirectX 11",
            Backend::Gl => "OpenGL",
            Backend::BrowserWebGpu => "WebGPU",
        };

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                limits: wgpu::Limits::downlevel_webgl2_defaults(),
                label: None
            },
            None,
        ).await.unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_supported_formats(&adapter)[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        let mut imgui = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), window, imgui_winit_support::HiDpiMode::Default);
        imgui.set_ini_filename(None);

        let hidpi_factor = window.scale_factor();
        let font_size = 11.0 * hidpi_factor as f32;
        imgui.io_mut().font_global_scale = 1.0 / hidpi_factor as f32;

        imgui.fonts().add_font(&[FontSource::DefaultFontData {
            config: Some(imgui::FontConfig {
                oversample_h: 1,
                pixel_snap_h: true,
                size_pixels: font_size,
                ..Default::default()
            }),
        }]);

        let renderer_config = imgui_wgpu::RendererConfig {
            texture_format: config.format,
            ..Default::default()
        };

        let renderer = imgui_wgpu::Renderer::new(&mut imgui, &device, &queue, renderer_config);

        let settings = UserSettings {
            zoom: 1.0,
            centre: [0.0, 0.0],
            iterations: 100,
            equation: "cpow(z, 2.0) + c".to_string(),
            prev_equation: "cpow(z, 2.0) + c".to_string(),
            equation_valid: true,
        };

        let uniform_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Window Resolution Uniform Buffer"),
                contents: bytemuck::cast_slice(&[calculate_uniforms(&size, &settings)]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
            }
        );

        let uniform_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("uniform_bind_group_layout")
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }
            ],
            label: Some("uniform_bind_group")
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WGSL Shader"),
            source: ShaderSource::Wgsl(include_str!("shader.wgsl").replace("REPLACE_FRACTAL_EQN", "cpow(z, 2.0) + c").into()),
        });
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            uniform_buffer,
            uniform_bind_group,
            uniform_bind_group_layout,
            imgui,
            imgui_platform: platform,
            imgui_renderer: renderer,
            last_frame: Instant::now(),
            last_cursor: None,
            backend,
            settings,
            input_state: InputState {
                lmb_pressed: false,
                prev_cursor_pos: PhysicalPosition {
                    x: 0.0,
                    y: 0.0,
                }
            }
        }
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::MouseWheel {delta, ..} => match delta {
                MouseScrollDelta::LineDelta(_, vert_scroll) => self.settings.zoom += vert_scroll / 5.0 * self.settings.zoom,
                MouseScrollDelta::PixelDelta(pos) => self.settings.zoom += pos.y as f32 / 10.0 * self.settings.zoom
            },
            WindowEvent::MouseInput {state, button, ..} => match button {
                MouseButton::Left => match state {
                    ElementState::Pressed => self.input_state.lmb_pressed = true,
                    ElementState::Released => self.input_state.lmb_pressed = false
                },
                _ => {}
            },
            WindowEvent::CursorMoved {position, ..} => {
                if self.input_state.lmb_pressed {
                    self.settings.centre[0] -= (position.x - self.input_state.prev_cursor_pos.x) as f32 * calculate_scale(&self.size, &self.settings);
                    self.settings.centre[1] -= (position.y - self.input_state.prev_cursor_pos.y) as f32 * calculate_scale(&self.size, &self.settings);
                }
                self.input_state.prev_cursor_pos = *position;
            }
            _ => { return false }
        }
        true
    }

    fn update(&mut self) {
        if self.settings.equation != self.settings.prev_equation {
            let shader_src = include_str!("shader.wgsl").replace("REPLACE_FRACTAL_EQN", &self.settings.equation);
            match naga::front::wgsl::Parser::new().parse(&*shader_src) {
                Ok(module) => {
                    match naga::valid::Validator::new(ValidationFlags::all(), Capabilities::empty()).validate(&module) {
                        Ok(_) => {
                            let shader = self.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                                label: Some("shader.wgsl"),
                                source: ShaderSource::Wgsl(shader_src.into()),
                            });

                            let render_pipeline_layout = self.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                                label: Some("Render Pipeline Layout"),
                                bind_group_layouts: &[&self.uniform_bind_group_layout],
                                push_constant_ranges: &[],
                            });

                            self.render_pipeline = self.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                                label: Some("Render Pipeline"),
                                layout: Some(&render_pipeline_layout),
                                vertex: wgpu::VertexState {
                                    module: &shader,
                                    entry_point: "vs_main",
                                    buffers: &[],
                                },
                                fragment: Some(wgpu::FragmentState {
                                    module: &shader,
                                    entry_point: "fs_main",
                                    targets: &[Some(wgpu::ColorTargetState {
                                        format: self.config.format,
                                        blend: Some(wgpu::BlendState::REPLACE),
                                        write_mask: wgpu::ColorWrites::ALL,
                                    })],
                                }),
                                primitive: wgpu::PrimitiveState {
                                    topology: wgpu::PrimitiveTopology::TriangleList,
                                    strip_index_format: None,
                                    front_face: wgpu::FrontFace::Ccw,
                                    cull_mode: None,
                                    polygon_mode: wgpu::PolygonMode::Fill,
                                    unclipped_depth: false,
                                    conservative: false,
                                },
                                depth_stencil: None,
                                multisample: wgpu::MultisampleState {
                                    count: 1,
                                    mask: !0,
                                    alpha_to_coverage_enabled: false,
                                },
                                multiview: None,
                            });
                            self.settings.equation_valid = true;
                        },
                        Err(_) => self.settings.equation_valid = false
                    };
                },
                Err(_) => self.settings.equation_valid = false
            };
        }

        self.queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[calculate_uniforms(&self.size, &self.settings)]));
        let now = Instant::now();
        self.imgui.io_mut().update_delta_time(now - self.last_frame);
        self.last_frame = now;
    }

    fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        self.imgui_platform.prepare_frame(self.imgui.io_mut(), &window).expect("Failed to prepare frame");
        let ui = self.imgui.frame();

        {
            let window = imgui::Window::new(format!("{} {} | {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), self.backend));
            window
                .size([250.0, 170.0], Condition::FirstUseEver)
                .build(&ui, || {
                    let settings_clone = self.settings.clone();
                    ui.input_float("Zoom", &mut self.settings.zoom).step(0.1 * settings_clone.zoom).build();
                    ui.separator();
                    ui.input_int("Iterations", &mut self.settings.iterations).step(1).build();
                    ui.text("Centre:");
                    ui.input_float("+", &mut self.settings.centre[0]).step(0.1 / settings_clone.zoom).build();
                    ui.input_float("i", &mut self.settings.centre[1]).step(0.1 / settings_clone.zoom).build();
                    ui.separator();
                    self.settings.prev_equation = settings_clone.equation;
                    ui.text("Iterative function (WGSL expression)");
                    ui.input_text("", &mut self.settings.equation).build();
                    if !settings_clone.equation_valid { ui.text_colored([1.0, 0.0, 0.0, 1.0], "Expression invalid"); }
                });
        }

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder")
        });

        if self.last_cursor != ui.mouse_cursor() {
            self.last_cursor = ui.mouse_cursor();
            self.imgui_platform.prepare_render(&ui, &window);
        }

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        }),
                        store: true,
                    }
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.draw(0..6, 0..1);

            self.imgui_renderer.render(ui.render(), &self.queue, &self.device, &mut render_pass).expect("UI rendering failed");
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch="wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch="wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Warn).expect("Couldn't initialise logger");
        } else {
            env_logger::init();
        }
    }
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .unwrap();

    #[cfg(target_arch="wasm32")]
    {
        use winit::dpi::PhysicalSize;
        window.set_inner_size(PhysicalSize::new(400, 400));

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("fractal-viewer")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document");
    }

    let mut state = State::new(&window).await;

    let mut last_title_update = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => if !state.input(event) {
                match event {
                    WindowEvent::CloseRequested | WindowEvent::KeyboardInput {
                        input:
                        KeyboardInput {
                            state: ElementState::Pressed,
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => state.resize(**new_inner_size),
                    _ => {}
                }
            },
            Event::MainEventsCleared => window.request_redraw(),
            Event::RedrawEventsCleared => {
                if last_title_update.elapsed() >= Duration::from_secs(1) {
                    window.set_title(&*format!("{} {} [{} | {:.0} FPS]", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"), state.backend, (1.0 / state.last_frame.elapsed().as_secs_f32())));
                    last_title_update = Instant::now();
                }
                state.update();
                match state.render(&window) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            },
            _ => {},
        }
        state.imgui_platform.handle_event(state.imgui.io_mut(), &window, &event);
    });
}