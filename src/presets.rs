/// List of preset fractals which can be selected in the UI.
#[rustfmt::skip] // rustfmt tries to split only some of these across multiple lines, which looks uglier than just having long lines
pub(crate) const FRACTAL_PRESETS: [(&str, &str); 4] = [
    ("Mandelbrot set", "csquare(z) + c"),
    ("Burning ship fractal", "csquare(abs(z)) + c"),
    ("Feather fractal", "cdiv(cmul(csquare(z), z), vec2<f32>(1.0, 0.0) + z * z) + c"),
    ("Tricorn fractal", "csquare(vec2<f32>(z.x, -z.y)) + c"),
];
