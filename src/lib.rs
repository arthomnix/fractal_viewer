mod settings;
mod uniforms;
#[cfg(target_arch = "wasm32")]
mod web;

use crate::settings::UserSettings;
use crate::uniforms::{calculate_scale, Uniforms};
#[allow(unused_imports)] // eframe::egui::ViewportCommand used on native but not web
use eframe::egui::{
    Color32, Context, Key, PaintCallbackInfo, PointerButton, TextEdit, ViewportCommand,
};
use eframe::{egui, Frame};
use egui_wgpu::{CallbackResources, ScreenDescriptor};
use instant::Instant;
use naga::valid::{Capabilities, ValidationFlags};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    Backend, BindGroup, BindGroupDescriptor, BindGroupLayout, BindGroupLayoutDescriptor, Buffer,
    ColorTargetState, CommandBuffer, CommandEncoder, Device, PipelineLayoutDescriptor, Queue,
    RenderPass, RenderPipeline, RenderPipelineDescriptor, ShaderModuleDescriptor, ShaderSource,
};

static SHADER: &str = include_str!("shader.wgsl");

fn validate_shader(equation: &str, colour: &str) -> Result<(), String> {
    let shader_src = SHADER
        .replace("REPLACE_FRACTAL_EQN", equation)
        .replace("REPLACE_COLOR", colour);

    let module = naga::front::wgsl::Frontend::new()
        .parse(&shader_src)
        .map_err(|e| e.to_string())?;
    naga::valid::Validator::new(ValidationFlags::all(), Capabilities::empty())
        .validate(&module)
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub struct FractalViewerApp {
    settings: UserSettings,
    last_frame: Instant,
    prev_frame_time: Duration,
    backend: &'static str,
    driver_info: String,
    show_ui: bool,
    recompile_shader: bool,
    shader_error: Option<String>,
    import_error: Option<String>,
    fps_samples: VecDeque<f32>,
    last_title_update: Option<Instant>,
    #[cfg(not(target_arch = "wasm32"))]
    clipboard: arboard::Clipboard,
}

impl FractalViewerApp {
    pub fn new<'a>(cc: &'a eframe::CreationContext<'a>) -> Option<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let settings = UserSettings::default();
        #[cfg(not(target_arch = "wasm32"))]
        let import_error = None;

        #[cfg(target_arch = "wasm32")]
        let (settings, import_error) = match web_sys::window()
            .and_then(|w| match w.location().href().ok() {
                Some(s) if s.contains('?') => Some(s),
                _ => None,
            })
            .map(|url| UserSettings::import_string(&url))
            .unwrap_or_else(|| Ok(UserSettings::default()))
        {
            Ok(settings) => (settings, None),
            Err(e) => (UserSettings::default(), Some(e.to_string())),
        };

        let wgpu_render_state = cc.wgpu_render_state.as_ref()?;
        let device = &wgpu_render_state.device;

        let size = cc.egui_ctx.screen_rect().size();

        let uniform_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("fv_uniform_buffer"),
            contents: bytemuck::cast_slice(&[Uniforms::new(size, &settings)]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("fv_uniform_bind_group_layout"),
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
            });

        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("fv_uniform_bind_group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("fv_shader"),
            source: ShaderSource::Wgsl(
                SHADER
                    .replace("REPLACE_FRACTAL_EQN", &settings.equation)
                    .replace("REPLACE_COLOR", &settings.colour)
                    .into(),
            ),
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("fv_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("fv_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu_render_state.target_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(FvRenderer {
                device: Arc::clone(device),
                pipeline,
                target_format: wgpu_render_state.target_format.into(),
                bind_group: uniform_bind_group,
                bind_group_layout: uniform_bind_group_layout,
                uniform_buffer,
            });

        let adapter_info = wgpu_render_state.adapter.get_info();
        let backend = match adapter_info.backend {
            Backend::Empty => "Empty",
            Backend::Vulkan => "Vulkan",
            Backend::Metal => "Metal",
            Backend::Dx12 => "DirectX 12",
            Backend::Gl => "WebGL/OpenGL",
            Backend::BrowserWebGpu => "WebGPU",
        };
        let driver_info = adapter_info.driver_info.clone();

        Some(Self {
            settings,
            last_frame: Instant::now(),
            prev_frame_time: Duration::from_secs(0),
            backend,
            driver_info,
            show_ui: true,
            recompile_shader: false,
            shader_error: None,
            import_error,
            fps_samples: VecDeque::new(),
            last_title_update: None,
            #[cfg(not(target_arch = "wasm32"))]
            clipboard: arboard::Clipboard::new().unwrap(),
        })
    }

    pub fn paint_fractal(&mut self, ui: &mut egui::Ui) {
        let size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        let scale = calculate_scale(size, &self.settings);
        if response.dragged_by(PointerButton::Primary) {
            let drag_motion = response.drag_delta();
            self.settings.centre[0] -= drag_motion.x * scale;
            self.settings.centre[1] -= drag_motion.y * scale;
        } else if response.clicked_by(PointerButton::Secondary)
            || response.dragged_by(PointerButton::Secondary)
        {
            let pointer_pos = response.interact_pointer_pos().unwrap();
            self.settings.initial_value[0] =
                (pointer_pos.x - size.x / 2.0) * scale + self.settings.centre[0];
            self.settings.initial_value[1] =
                (pointer_pos.y - size.y / 2.0) * scale + self.settings.centre[1];
        }

        let scroll = ui.input(|i| i.raw_scroll_delta);
        self.settings.zoom += self.settings.zoom * (scroll.y / 300.0).max(-0.9);

        let uniforms = Uniforms::new(size, &self.settings);

        let callback = FvRenderCallback {
            uniforms,
            shader_recompilation_options: if self.recompile_shader {
                Some((self.settings.equation.clone(), self.settings.colour.clone()))
            } else {
                None
            },
        };

        ui.painter()
            .add(egui_wgpu::Callback::new_paint_callback(rect, callback));
    }
}

impl eframe::App for FractalViewerApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        let fps = self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32;
        if self.last_title_update.is_none()
            || self
                .last_title_update
                .is_some_and(|i| i.elapsed() >= Duration::from_secs(1))
        {
            let title = format!(
                "{} {} [{} | {} | {:.0} FPS]",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION"),
                self.backend,
                std::env::consts::ARCH,
                fps
            );

            #[cfg(not(target_arch = "wasm32"))]
            ctx.send_viewport_cmd(ViewportCommand::Title(title));

            #[cfg(target_arch = "wasm32")]
            if let Some(title_element) = web_sys::window()
                .and_then(|window| window.document())
                .and_then(|document| document.get_element_by_id("title"))
            {
                title_element.set_inner_html(&title);
            }

            self.last_title_update = Some(Instant::now());
        }

        #[cfg(not(target_arch = "wasm32"))]
        if ctx.input(|i| i.key_pressed(Key::F11)) {
            let current_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap());
            ctx.send_viewport_cmd(ViewportCommand::Fullscreen(!current_fullscreen));
        }

        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.show_ui = !self.show_ui;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame::default().inner_margin(0.0))
            .show(ctx, |ui| self.paint_fractal(ui));

        egui::Window::new(env!("CARGO_PKG_NAME"))
            .title_bar(true)
            .open(&mut self.show_ui)
            .show(ctx, |ui| {
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

                if self.driver_info.is_empty() {
                    ui.label(format!("Render backend: {}", self.backend));
                } else {
                    ui.label(format!("Render backend: {} ({})", self.backend, &self.driver_info));
                }

                ui.label(format!(
                    "Last frame: {:.1}ms (smoothed FPS: {:.0})",
                    self.prev_frame_time.as_micros() as f64 / 1000.0,
                    self.fps_samples.iter().sum::<f32>() / self.fps_samples.len() as f32
                ));
                #[cfg(not(target_arch = "wasm32"))]
                ui.label("Fullscreen: [F11]");

                ui.label("Toggle UI: [F1]");
                ui.separator();

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
                            1.0..=f32::MAX,
                        )
                            .logarithmic(true),
                    );
                });
                ui.separator();
                ui.collapsing("Centre [Click and drag to pan]", |ui| {
                    ui.label("Centre");
                    ui.add(
                        egui::DragValue::new(&mut self.settings.centre[0])
                            .speed(0.1 / self.settings.zoom),
                    );
                    ui.add(
                        egui::DragValue::new(&mut self.settings.centre[1])
                            .speed(0.1 / self.settings.zoom)
                            .suffix("i"),
                    );
                    if ui.button("Reset").clicked() {
                        self.settings.centre = [0.0, 0.0];
                    }
                });
                ui.separator();
                ui.checkbox(&mut self.settings.julia_set, "Julia set");
                ui.separator();
                ui.collapsing("Initial value [Hold right click and drag]", |ui| {
                    ui.label("Initial value of z");
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
                    ui.checkbox(&mut self.settings.initial_c, "Add c to initial value");
                });
                ui.separator();
                ui.collapsing("Equation", |ui| {
                    ui.label("Iterative function (WGSL expression)");
                    egui::ComboBox::from_label("Iterative function")
                        .selected_text("Select default equation")
                        .show_ui(ui, |ui| {
                            if ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(z) + c".to_string(),
                                "Mandelbrot set",
                            ).clicked() || ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(abs(z)) + c".to_string(),
                                "Burning ship fractal",
                            ).clicked() || ui.selectable_value(
                                &mut self.settings.equation,
                                "cdiv(cmul(csquare(z), z), vec2<f32>(1.0, 0.0) + z * z) + c"
                                    .to_string(),
                                "Feather fractal",
                            ).clicked() || ui.selectable_value(
                                &mut self.settings.equation,
                                "csquare(vec2<f32>(z.x, -z.y)) + c".to_string(),
                                "Tricorn fractal",
                            ).clicked() {
                                self.recompile_shader = true;
                            }
                        });
                    ui.label("...Or edit it yourself!");
                    if ui.add(TextEdit::singleline(&mut self.settings.equation).desired_width(ui.max_rect().width())).changed() {
                        self.recompile_shader = true;
                    };
                    ui.label("Colour expression:");
                    ui.horizontal(|ui| {
                        if ui.text_edit_singleline(&mut self.settings.colour).changed() {
                            self.recompile_shader = true;
                        };
                        if ui.button("Reset").clicked() {
                            self.settings.colour = "hsv_rgb(vec3(log(n + 1.0) / log(f32(uniforms.iterations) + 1.0), 0.8, 0.8))".to_string();
                            self.recompile_shader = true;
                        }
                    });
                    ui.checkbox(&mut self.settings.internal_black, "Always colour inside of set black");

                    if let Some(e) = &self.shader_error {
                        ui.colored_label(Color32::RED, format!("Invalid expression: {e}"));
                    }
                });

                {
                    ui.separator();
                    ui.checkbox(&mut self.settings.smoothen, "Smoothen (warning: only produces correct results on a normal Mandelbrot set!)");
                }
                {
                    ui.separator();
                    egui::CollapsingHeader::new("Export and import options")
                        .default_open(self.import_error.is_some())
                        .show(ui, |ui| {
                            if ui.button("Export to clipboard").clicked() {
                                ui.output_mut(|o| o.copied_text = self.settings.export_string());
                            }
                            if ui.button("Export link to clipboard").clicked() {
                                ui.output_mut(|o| o.copied_text = format!("{}?{}", option_env!("SITE_LINK").unwrap_or("https://arthomnix.dev/fractal/"), self.settings.export_string()));
                            }
                            // Reading clipboard doesn't work in Firefox, so we only support importing from link on web
                            #[cfg(not(target_arch = "wasm32"))]
                            if ui.button("Import from clipboard").clicked() {
                                let text = self.clipboard.get_text().unwrap_or_default();
                                match UserSettings::import_string(&text) {
                                    Ok(settings) => {
                                        self.settings = settings;
                                        self.import_error = None;
                                        self.recompile_shader = true;
                                    }
                                    Err(e) => self.import_error = Some(e.to_string()),
                                };
                            }
                            if let Some(e) = &self.import_error {
                                ui.colored_label(Color32::RED, format!("Import failed: {e}"));
                            }
                            #[cfg(target_arch = "wasm32")]
                            ui.label("To import a settings string on web, add '?<string>' to the end of this page's URL.")
                        });
                }

                #[cfg(target_arch = "wasm32")]
                {
                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.hyperlink_to("Source code", option_env!("SOURCE_LINK").unwrap_or("https://github.com/arthomnix/fractal_viewer"));
                        ui.label("|");
                        ui.hyperlink_to("Download desktop version", option_env!("DL_LINK").unwrap_or("https://github.com/arthomnix/fractal_viewer/releases/latest"));
                    })
                }
            });

        // Validate custom expressions
        if self.recompile_shader {
            if let Err(e) = validate_shader(&self.settings.equation, &self.settings.colour) {
                self.shader_error = Some(e);
                self.recompile_shader = false;
            } else {
                self.shader_error = None;
            }
        }

        self.prev_frame_time = self.last_frame.elapsed();
        let new_fps = self.prev_frame_time.as_secs_f32().recip();
        self.fps_samples.push_back(new_fps);
        if self.fps_samples.len() > 200 {
            self.fps_samples.pop_front();
        }
        self.last_frame = Instant::now();
    }
}

struct FvRenderer {
    device: Arc<Device>,
    pipeline: RenderPipeline,
    target_format: ColorTargetState,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    uniform_buffer: Buffer,
}

impl FvRenderer {
    fn prepare(&mut self, queue: &Queue, callback: &FvRenderCallback) {
        if let Some((equation, colour)) = &callback.shader_recompilation_options {
            let shader = self.device.create_shader_module(ShaderModuleDescriptor {
                label: Some("fv_shader"),
                source: ShaderSource::Wgsl(
                    SHADER
                        .replace("REPLACE_FRACTAL_EQN", &equation)
                        .replace("REPLACE_COLOR", &colour)
                        .into(),
                ),
            });

            let pipeline_layout = self
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("fv_pipeline_layout"),
                    bind_group_layouts: &[&self.bind_group_layout],
                    push_constant_ranges: &[],
                });

            let pipeline = self
                .device
                .create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some("fv_pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[Some(self.target_format.clone())],
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                });

            self.pipeline = pipeline;
        }

        queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[callback.uniforms]),
        );
    }

    fn paint<'rp>(&'rp self, render_pass: &mut RenderPass<'rp>) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

struct FvRenderCallback {
    uniforms: Uniforms,
    shader_recompilation_options: Option<(String, String)>,
}

impl egui_wgpu::CallbackTrait for FvRenderCallback {
    fn prepare(
        &self,
        _device: &Device,
        queue: &Queue,
        _screen_descriptor: &ScreenDescriptor,
        _egui_encoder: &mut CommandEncoder,
        callback_resources: &mut CallbackResources,
    ) -> Vec<CommandBuffer> {
        let renderer: &mut FvRenderer = callback_resources.get_mut().unwrap();
        renderer.prepare(queue, self);
        vec![]
    }

    fn paint<'a>(
        &'a self,
        _info: PaintCallbackInfo,
        render_pass: &mut RenderPass<'a>,
        callback_resources: &'a CallbackResources,
    ) {
        let renderer: &FvRenderer = callback_resources.get().unwrap();
        renderer.paint(render_pass);
    }
}
