#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;
#[cfg(target_arch = "wasm32")]
use std::cell::{Cell, RefCell};

#[cfg(not(target_arch = "wasm32"))]
use winit::window::Fullscreen;

use egui::Color32;
use egui_wgpu::renderer::{Renderer, ScreenDescriptor};
use instant::{Duration, Instant};
use naga::valid::{Capabilities, ValidationFlags};
use std::fmt::{Display, Formatter};
use base64::Engine;
use wgpu::util::DeviceExt;
use wgpu::{Backend, ShaderSource};
use winit::dpi::PhysicalPosition;
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

#[derive(Debug, serde::Deserialize)]
enum InvalidSettingsImportError {
    InvalidFormat,
    VersionMismatch,
    InvalidBase64,
    DeserialisationFailed,
}

impl InvalidSettingsImportError {
    fn to_str(&self) -> &str {
        match self {
            InvalidSettingsImportError::InvalidFormat => "Invalid settings string format",
            InvalidSettingsImportError::VersionMismatch => "Version mismatch or invalid format",
            InvalidSettingsImportError::InvalidBase64 => "Base64 decoding failed",
            InvalidSettingsImportError::DeserialisationFailed => "Deserialising data failed",
        }
    }
}

impl Display for InvalidSettingsImportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_str())
    }
}

impl std::error::Error for InvalidSettingsImportError {
    fn description(&self) -> &str {
        self.to_str()
    }
}

fn calculate_scale(size: &winit::dpi::PhysicalSize<u32>, settings: &UserSettings) -> f32 {
    4.0 / settings.zoom
        / (if size.width < size.height {
            size.width
        } else {
            size.height
        }) as f32
}

fn get_major_minor_version() -> String {
    let mut version_iterator = env!("CARGO_PKG_VERSION").split('.');
    format!(
        "{}.{}",
        version_iterator.next().unwrap(),
        version_iterator.next().unwrap()
    )
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    scale: f32,
    escape_threshold: f32,
    centre: [f32; 2],
    iterations: i32,
    julia_set: u32,
    initial_value: [f32; 2],
}

impl Uniforms {
    fn new(size: &winit::dpi::PhysicalSize<u32>, settings: &UserSettings) -> Self {
        let scale = calculate_scale(size, settings);
        Uniforms {
            scale,
            centre: [
                size.width as f32 / 2.0 * scale - settings.centre[0],
                size.height as f32 / 2.0 * scale - settings.centre[1],
            ],
            iterations: settings.iterations,
            julia_set: settings.julia_set as u32,
            initial_value: settings.initial_value,
            escape_threshold: settings.escape_threshold,
        }
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct UserSettings {
    zoom: f32,
    centre: [f32; 2],
    iterations: i32,
    equation: String,
    prev_equation: String,
    equation_valid: bool,
    julia_set: bool,
    initial_value: [f32; 2],
    escape_threshold: f32,
}

impl UserSettings {
    fn export_string(&self) -> String {
        let mut settings = self.clone();
        settings.prev_equation = String::new();
        let encoded = bincode::serialize(&settings).unwrap();
        format!("{};{}", get_major_minor_version(), base64::engine::general_purpose::STANDARD.encode(encoded))
    }

    fn import_string(string: &String) -> Result<Self, InvalidSettingsImportError> {
        let string = match url::Url::parse(string) {
            Ok(url) => url.query().unwrap_or_default().to_string(),
            Err(_) => string.to_string(),
        };

        if string.is_empty() {
            return Err(InvalidSettingsImportError::InvalidFormat);
        }

        let mut iterator = string.split(';');

        let major_minor_version = iterator.next().ok_or(InvalidSettingsImportError::InvalidFormat)?;

        if major_minor_version == get_major_minor_version() {
            let base64 = iterator.next().ok_or(InvalidSettingsImportError::InvalidFormat)?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(base64).map_err(|_| InvalidSettingsImportError::InvalidBase64)?;
            let mut result = bincode::deserialize::<'_, Self>(bytes.as_slice()).map_err(|_| InvalidSettingsImportError::DeserialisationFailed)?;
            result.prev_equation = String::new();
            Ok(result)
        } else {
            Err(InvalidSettingsImportError::VersionMismatch)
        }
    }
}

struct InputState {
    lmb_pressed: bool,
    rmb_pressed: bool,
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
    last_frame: Instant,
    prev_frame_time: Duration,
    backend: &'static str,
    settings: UserSettings,
    input_state: InputState,
    egui_state: egui_winit::State,
    context: egui::Context,
    rpass: Renderer,
    import_error: String,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: arboard::Clipboard,
    #[cfg(target_arch = "wasm32")]
    copy_event: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    cut_event: Rc<Cell<bool>>,
    #[cfg(target_arch = "wasm32")]
    paste_event: Rc<RefCell<String>>,
}

impl State {
    async fn new(window: &Window, ev_loop: &EventLoop<()>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(Default::default());
        let surface = unsafe { instance.create_surface(window).expect("Failed to create surface") };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let backend = match adapter.get_info().backend {
            Backend::Empty => "Empty",
            Backend::Vulkan => "Vulkan",
            Backend::Metal => "Metal",
            Backend::Dx12 => "DirectX 12",
            Backend::Dx11 => "DirectX 11",
            Backend::Gl => "WebGL",
            Backend::BrowserWebGpu => "WebGPU",
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::downlevel_webgl2_defaults(),
                    label: None,
                },
                None,
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface.get_capabilities(&adapter).formats[0],
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        let egui_state = egui_winit::State::new(ev_loop);
        let context = egui::Context::default();

        let rpass = Renderer::new(&device, config.format, None, 1);

        #[allow(unused_mut)]
        // variable is mutated in wasm but will cause a warning on non-wasm platforms
        let mut settings = UserSettings {
            zoom: 1.0,
            centre: [0.0, 0.0],
            iterations: 100,
            equation: "csquare(z) + c".to_string(),
            prev_equation: "csquare(z) + c".to_string(),
            equation_valid: true,
            julia_set: false,
            initial_value: [0.0, 0.0],
            escape_threshold: 2.0,
        };

        #[allow(unused_mut)]
        let mut import_error = String::new();

        #[cfg(target_arch = "wasm32")]
        {
            settings = UserSettings::import_string(
                &web_sys::window()
                    .and_then(|win| Some(win.location().href().unwrap()))
                    .unwrap(),
            )
            .unwrap_or(settings);
        }

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[Uniforms::new(&size, &settings)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WGSL Shader"),
            source: ShaderSource::Wgsl(
                include_str!("shader.wgsl")
                    .replace("REPLACE_FRACTAL_EQN", "cpow(z, 2.0) + c")
                    .into(),
            ),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
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

        let state = Self {
            surface,
            device,
            queue,
            config,
            size,
            render_pipeline,
            uniform_buffer,
            uniform_bind_group,
            uniform_bind_group_layout,
            last_frame: Instant::now(),
            prev_frame_time: Duration::from_secs(0),
            backend,
            settings,
            input_state: InputState {
                lmb_pressed: false,
                rmb_pressed: false,
                prev_cursor_pos: PhysicalPosition { x: 0.0, y: 0.0 },
            },
            egui_state,
            context,
            rpass,
            import_error,
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: arboard::Clipboard::new().unwrap(),
            #[cfg(target_arch = "wasm32")]
            copy_event: Rc::new(Cell::new(false)),
            #[cfg(target_arch = "wasm32")]
            cut_event: Rc::new(Cell::new(false)),
            #[cfg(target_arch = "wasm32")]
            paste_event: Rc::new(RefCell::new(Default::default())),
        };

        #[cfg(target_arch = "wasm32")]
        web_sys::window().and_then(|window| {
           window.document().and_then(|document| {
               {
                   let event = state.copy_event.clone();
                   let closure = Closure::<dyn FnMut(_)>::new(move |_: web_sys::ClipboardEvent| {
                       event.set(true);
                   });
                   document
                       .add_event_listener_with_callback("copy", closure.as_ref().unchecked_ref())
                       .expect("Failed to add copy event listener");
                   closure.forget();
               }
               {
                   let event = state.cut_event.clone();
                   let closure = Closure::<dyn FnMut(_)>::new(move |_: web_sys::ClipboardEvent| {
                        event.set(true);
                   });
                   document
                       .add_event_listener_with_callback("cut", closure.as_ref().unchecked_ref())
                       .expect("Failed to add cut event listener");
                   closure.forget();
               }
               {
                   let event = state.paste_event.clone();
                   let closure = Closure::<dyn FnMut(_)>::new(move |ev: web_sys::ClipboardEvent| {
                        if let Some(data) = ev.clipboard_data() {
                            if let Ok(text) = data.get_data("text/plain") {
                                if let Ok(mut b) = event.try_borrow_mut() {
                                    *b = text;
                                }
                            }
                        }
                   });
                   document
                       .add_event_listener_with_callback("paste", closure.as_ref().unchecked_ref())
                       .expect("Failed to add paste event listener");
                   closure.forget();
               }
               Some(())
           })
        });

        state
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
            WindowEvent::MouseWheel { delta, .. } => {
                self.settings.zoom += match delta {
                    MouseScrollDelta::LineDelta(_, vert_scroll) => vert_scroll / 5.0,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 300.0,
                }
                .max(-0.9)
                    * self.settings.zoom;
            }
            WindowEvent::MouseInput { state, button, .. } => match button {
                MouseButton::Left => match state {
                    ElementState::Pressed => self.input_state.lmb_pressed = true,
                    ElementState::Released => self.input_state.lmb_pressed = false,
                },
                MouseButton::Right => match state {
                    ElementState::Pressed => self.input_state.rmb_pressed = true,
                    ElementState::Released => self.input_state.rmb_pressed = false,
                },
                _ => {}
            },
            WindowEvent::CursorMoved { position, .. } => {
                if self.input_state.lmb_pressed {
                    self.settings.centre[0] -= (position.x - self.input_state.prev_cursor_pos.x)
                        as f32
                        * calculate_scale(&self.size, &self.settings);
                    self.settings.centre[1] -= (position.y - self.input_state.prev_cursor_pos.y)
                        as f32
                        * calculate_scale(&self.size, &self.settings);
                } else if self.input_state.rmb_pressed {
                    let scale = calculate_scale(&self.size, &self.settings);
                    self.settings.initial_value = [
                        (position.x as f32 - (self.size.width / 2) as f32) * scale
                            + self.settings.centre[0],
                        (position.y as f32 - (self.size.height / 2) as f32) * scale
                            + self.settings.centre[1],
                    ];
                }
                self.input_state.prev_cursor_pos = *position;
            }
            _ => return false,
        }
        true
    }

    fn update(&mut self) {
        if self.settings.equation != self.settings.prev_equation {
            let shader_src =
                include_str!("shader.wgsl").replace("REPLACE_FRACTAL_EQN", &self.settings.equation);
            match naga::front::wgsl::Frontend::new().parse(&shader_src) {
                Ok(module) => {
                    match naga::valid::Validator::new(ValidationFlags::all(), Capabilities::empty())
                        .validate(&module)
                    {
                        Ok(_) => {
                            let shader =
                                self.device
                                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                                        label: Some("shader.wgsl"),
                                        source: ShaderSource::Wgsl(shader_src.into()),
                                    });

                            let render_pipeline_layout = self.device.create_pipeline_layout(
                                &wgpu::PipelineLayoutDescriptor {
                                    label: Some("Render Pipeline Layout"),
                                    bind_group_layouts: &[&self.uniform_bind_group_layout],
                                    push_constant_ranges: &[],
                                },
                            );

                            self.render_pipeline = self.device.create_render_pipeline(
                                &wgpu::RenderPipelineDescriptor {
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
                                },
                            );
                            self.settings.equation_valid = true;
                        }
                        Err(_) => self.settings.equation_valid = false,
                    };
                }
                Err(_) => self.settings.equation_valid = false,
            };
        }

        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[Uniforms::new(&self.size, &self.settings)]),
        );

        self.prev_frame_time = self.last_frame.elapsed();
        self.last_frame = Instant::now();
    }

    fn render(&mut self, window: &Window) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        #[allow(unused_mut)]
        let mut input = self.egui_state.take_egui_input(window);
        #[cfg(target_arch = "wasm32")]
        {
            if self.copy_event.get() {
                input.events.push(egui::Event::Copy);
                self.copy_event.set(false);
            }
            if self.cut_event.get() {
                input.events.push(egui::Event::Cut);
                self.copy_event.set(false);
            }
            if let Ok(mut text) = self.paste_event.try_borrow_mut() {
                input.events.push(egui::Event::Paste(text.clone()));
                text.clear();
            }
        }
        self.context.begin_frame(input);

        egui::Window::new(env!("CARGO_PKG_NAME"))
            .title_bar(true)
            .show(&self.context, |ui| {
                egui::trace!(ui);

                ui.label(format!(
                    "Version {} ({}{}{})",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS,
                    if std::env::consts::OS.is_empty() {
                        ""
                    } else {
                        " "
                    },
                    std::env::consts::ARCH
                ));
                ui.label(format!("Render backend: {}", self.backend));
                ui.label(format!(
                    "Last frame: {:.1}ms ({:.0} FPS)",
                    self.prev_frame_time.as_micros() as f64 / 1000.0,
                    1.0 / self.prev_frame_time.as_secs_f64()
                ));
                #[cfg(not(target_arch = "wasm32"))]
                ui.label("Fullscreen: [F11]");
                ui.separator();

                let settings_clone = self.settings.clone();

                ui.collapsing("Zoom [Scroll]", |ui| {
                    ui.label("Zoom");
                    ui.add(
                        egui::Slider::new(&mut self.settings.zoom, 0.0..=100000.0)
                            .logarithmic(true),
                    );
                });
                ui.separator();
                ui.collapsing("Iterations", |ui| {
                    ui.label("Iterations");
                    ui.add(
                        egui::Slider::new(&mut self.settings.iterations, 1..=10000)
                            .logarithmic(true),
                    );
                    ui.label("Escape threshold");
                    ui.add(
                        egui::Slider::new(
                            &mut self.settings.escape_threshold,
                            1.0..=13043817825300000000.0,
                        ) // approximate square root of maximum f32
                        .logarithmic(true),
                    );
                });
                ui.separator();
                ui.collapsing("Centre [Click and drag to pan]", |ui| {
                    ui.label("Centre");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.centre[0])
                            .speed(0.1 / settings_clone.zoom),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.settings.centre[1])
                            .speed(0.1 / settings_clone.zoom)
                            .suffix("i"),
                    );
                    if ui.button("Reset").clicked() {
                        self.settings.centre = [0.0, 0.0];
                    }
                });
                ui.separator();
                ui.checkbox(&mut self.settings.julia_set, "Julia set");
                ui.separator();
                ui.collapsing("Initial value [Right click and drag]", |ui| {
                    ui.label("Initial value of z [Right click]");
                    ui.label("(or value of c for Julia sets)");
                    ui.add(egui::DragValue::new(&mut self.settings.initial_value[0]).speed(0.01));
                    ui.add(
                        egui::DragValue::new(&mut self.settings.initial_value[1])
                            .speed(0.01)
                            .suffix("i"),
                    );
                    if ui.button("Reset").clicked() {
                        self.settings.initial_value = [0.0, 0.0];
                    }
                });
                ui.separator();
                ui.collapsing("Equation", |ui| {
                    self.settings.prev_equation = settings_clone.equation;
                    ui.label("Iterative function (WGSL expression)");
                    egui::ComboBox::from_label("Iterative function")
                        .selected_text(&self.settings.equation)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(z) + c".to_string(),
                                "Mandelbrot set",
                            );
                            ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(abs(z)) + c".to_string(),
                                "Burning ship fractal",
                            );
                            ui.selectable_value(
                                &mut self.settings.equation,
                                "cdiv(cmul(csquare(z), z), vec2<f32>(1.0, 0.0) + z * z) + c"
                                    .to_string(),
                                "Feather fractal",
                            );
                            ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(vec2<f32>(z.x, -z.y)) + c".to_string(),
                                "Tricorn fractal",
                            );
                        });
                    ui.label("Custom");
                    ui.text_edit_singleline(&mut self.settings.equation);
                    if !settings_clone.equation_valid {
                        ui.colored_label(Color32::RED, "Invalid expression");
                    }
                });
                {
                    ui.separator();
                    egui::CollapsingHeader::new("Export and import options")
                        .default_open(!self.import_error.is_empty())
                        .show(ui, |ui| {
                        if ui.button("Export to clipboard").clicked() {
                            ui.output_mut(|o| o.copied_text = self.settings.export_string());
                        }
                        if ui.button("Export link to clipboard").clicked() {
                            ui.output_mut(|o| o.copied_text = format!("https://arthomnix.dev/fractal/?{}", self.settings.export_string()));
                        }
                        // Reading clipboard doesn't work in Firefox, so we only support importing from link on web
                        #[cfg(not(target_arch = "wasm32"))]
                        if ui.button("Import from clipboard").clicked() {
                            let text = self.clipboard.get_text().unwrap_or_default();
                            match UserSettings::import_string(&text) {
                                Ok(settings) => {
                                    self.settings = settings;
                                    self.import_error = String::new();
                                }
                                Err(e) => self.import_error = format!("{e}"),
                            };
                        }
                        if !&self.import_error.is_empty() {
                            ui.colored_label(Color32::RED, format!("Import failed: {}", self.import_error));
                        }
                        #[cfg(target_arch = "wasm32")]
                        ui.label("To import a settings string on web, add '?<string>' to the end of this page's URL.")
                    });
                }

                #[cfg(target_arch = "wasm32")]
                {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.hyperlink_to("Source code", "https://github.com/arthomnix/fractal_viewer");
                        ui.label("|");
                        ui.hyperlink_to("Download desktop version", "https://github.com/arthomnix/fractal_viewer/releases/latest");
                    })
                }
            });

        let full_output = self.context.end_frame();
        #[cfg(target_arch = "wasm32")]
        {
            if !full_output.platform_output.copied_text.is_empty() {
                web_set_clipboard_text(&full_output.platform_output.copied_text);
            }
        }
        self.egui_state.handle_platform_output(&window, &self.context, full_output.platform_output);
        let paint_jobs = self.context.tessellate(full_output.shapes);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        let screen_descriptor = ScreenDescriptor {
            size_in_pixels: [self.config.width, self.config.height],
            pixels_per_point: if cfg!(target_arch = "wasm32") {
                window.scale_factor() as f32
            } else {
                1.0
            },
        };

        let tdelta = full_output.textures_delta;

        for (id, delta) in tdelta.set.iter() {
            self.rpass.update_texture(&self.device, &self.queue, *id, delta);
        }
        self.rpass.update_buffers(&self.device, &self.queue, &mut encoder, &paint_jobs, &screen_descriptor);

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
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.draw(0..6, 0..1);

            self.rpass.render(&mut render_pass, &paint_jobs, &screen_descriptor);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
fn web_set_clipboard_text(s: &str) {
    if let Some(window) = web_sys::window() {
        if let Some(clipboard) = window.navigator().clipboard() {
            let promise = clipboard.write_text(s);
            let future = wasm_bindgen_futures::JsFuture::from(promise);
            let future = async move {
                if let Err(err) = future.await {
                    log::error!("Copy/cut action failed: {err:?}");
                }
            };
            wasm_bindgen_futures::spawn_local(future);
        }
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
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
    let builder = WindowBuilder::new();
    #[cfg(target_arch = "wasm32")]
        let builder = {
        use winit::platform::web::WindowBuilderExtWebSys;

        builder.with_prevent_default(false)
            .with_focusable(true)
    };
    let window = builder.build(&event_loop).unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        use winit::dpi::LogicalSize;
        use winit::platform::web::WindowExtWebSys;

        web_sys::window()
            .and_then(|win| {
                window.set_inner_size(LogicalSize::new(
                    win.inner_width().ok()?.as_f64()?,
                    win.inner_height().ok()?.as_f64()?,
                ));
                win.document()
            })
            .and_then(|doc| {
                let dst = doc.get_element_by_id("fractal-viewer")?;
                let canvas = web_sys::Element::from(window.canvas());
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document");
    }

    let mut state = State::new(&window, &event_loop).await;

    let mut last_title_update = Instant::now();

    event_loop.run(move |event, _, control_flow| match event {
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => {
            if !state.egui_state.on_event(&state.context, event).consumed && !state.input(event) {
                match event {
                    WindowEvent::CloseRequested
                    | WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => *control_flow = ControlFlow::Exit,
                    #[cfg(not(target_arch = "wasm32"))]
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                state: ElementState::Pressed,
                                virtual_keycode: Some(VirtualKeyCode::F11),
                                ..
                            },
                        ..
                    } => {
                        if window.fullscreen().is_some() {
                            window.set_fullscreen(None);
                        } else {
                            window.current_monitor().map(|monitor| {
                                monitor.video_modes().next().map(|mode| {
                                    window.set_fullscreen(Some(Fullscreen::Exclusive(mode)));
                                })
                            });
                        }
                    }
                    WindowEvent::Resized(physical_size) => state.resize(*physical_size),
                    WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                        state.resize(**new_inner_size)
                    }
                    _ => {}
                }
            }
        }
        Event::MainEventsCleared => window.request_redraw(),
        Event::RedrawRequested(window_id) => {
            if window_id == window.id() {
                #[cfg(target_arch = "wasm32")]
                {
                    use winit::dpi::LogicalSize;

                    web_sys::window()
                        .and_then(|win| {
                            window.set_inner_size(LogicalSize::new(
                                win.inner_width().ok()?.as_f64()?,
                                win.inner_height().ok()?.as_f64()?,
                            ));
                            Some(())
                        })
                        .expect("Couldn't resize window");
                }

                if last_title_update.elapsed() >= Duration::from_secs(1) {
                    let title = format!(
                        "{} {} [{} | {} | {:.0} FPS]",
                        env!("CARGO_PKG_NAME"),
                        env!("CARGO_PKG_VERSION"),
                        state.backend,
                        std::env::consts::ARCH,
                        (1.0 / state.prev_frame_time.as_secs_f64())
                    );
                    window.set_title(&*title);
                    #[cfg(target_arch = "wasm32")]
                    {
                        web_sys::window()
                            .and_then(|win| win.document())
                            .and_then(|doc| {
                                let title_element = doc.get_element_by_id("title")?;
                                title_element.set_inner_html(&title);
                                Some(())
                            });
                    }
                    last_title_update = Instant::now();
                }
                state.update();
                match state.render(&window) {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    Err(e) => eprintln!("{:?}", e),
                }
            }
        }
        _ => {}
    });
}
