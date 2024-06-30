use crate::settings::UserSettings;
use eframe::egui::Vec2;

pub(crate) fn calculate_scale(size: Vec2, settings: &UserSettings) -> f32 {
    4.0 / settings.zoom / size.min_elem()
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct Uniforms {
    scale: f32,
    escape_threshold: f32,
    centre: [f32; 2],
    iterations: i32,
    flags: u32,
    initial_value: [f32; 2],
}

impl Uniforms {
    pub(crate) fn new(size: Vec2, settings: &UserSettings) -> Self {
        let scale = calculate_scale(size, settings);
        Uniforms {
            scale,
            centre: [
                size.x / 2.0 * scale - settings.centre[0],
                size.y / 2.0 * scale - settings.centre[1],
            ],
            iterations: settings.iterations,
            flags: (settings.initial_c as u32) << 3
                | (settings.internal_black as u32) << 2
                | (settings.smoothen as u32) << 1
                | (settings.julia_set as u32),
            initial_value: settings.initial_value,
            escape_threshold: settings.escape_threshold,
        }
    }
}
