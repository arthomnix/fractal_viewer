# fractal_viewer
A cross-platform, GPU-accelerated viewer for the Mandelbrot Set and related fractals.
#### [Try it online!](https://arthomnix.dev/fractal)

## Usage
Scroll wheel to zoom, click and drag to pan. Change the initial value of z or c by right clicking.

Custom functions should be valid WGSL expressions, with the following extra functions available:
* `cabs(vec2<f32>) -> f32`: absolute value of a complex number
* `cpow(vec2<f32>, f32) -> vec2<f32>`: real power of a complex number
* `cdiv(vec2<f32>, vec2<f32>) -> vec2<f32>`: divide two complex numbers
* `cmul(vec2<f32>, vec2<f32>) -> vec2<f32>`: multiply two complex numbers

All builtin WGSL functions are also available.