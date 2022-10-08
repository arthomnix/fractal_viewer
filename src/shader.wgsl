struct Uniforms {
    scale: f32,
    centre: vec2<f32>,
    iterations: i32,
    _pad_wasm: vec2<u32>,
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4<f32> {
    var vertex_positions: array<vec4<f32>, 6> = array<vec4<f32>, 6>(
        vec4<f32>(-1.0, -1.0, 0.0, 1.0),
        vec4<f32>(1.0, -1.0, 0.0, 1.0),
        vec4<f32>(-1.0, 1.0, 0.0, 1.0),
        vec4<f32>(1.0, -1.0, 0.0, 1.0),
        vec4<f32>(1.0, 1.0, 0.0, 1.0),
        vec4<f32>(-1.0, 1.0, 0.0, 1.0),
    );
    return vertex_positions[in_vertex_index];
}

fn cabs_squared(z: vec2<f32>) -> f32 {
    return (z.x * z.x + z.y * z.y);
}

fn cabs(z: vec2<f32>) -> f32 {
    return sqrt(cabs_squared(z));
}

fn cpow(z: vec2<f32>, p: f32) -> vec2<f32> {
    let r: f32 = cabs(z);
    let arg: f32 = atan2(z.y, z.x);
    return vec2<f32>(pow(r, p) * cos(p * arg), pow(r, p) * sin(p * arg));
}

fn hsv_rgb(hsv: vec3<f32>) -> vec3<f32> {
    if (hsv.y == 0.0) {
        return vec3<f32>(hsv.z, hsv.z, hsv.z);
    } else {
        var hp: f32 = hsv.x * 6.0;
        if (hp == 6.0) {
            hp = 0.0;
        }
        let hpi: i32 = i32(hp);
        let v1: f32 = hsv.z * (1.0 - hsv.y);
        let v2: f32 = hsv.z * (1.0 - hsv.y * (hp - f32(hpi)));
        let v3: f32 = hsv.z * (1.0 - hsv.y * (1.0 - (hp - f32(hpi))));
        switch (hpi) {
            case 0: {
                return vec3<f32>(hsv.z, v3, v1);
            }
            case 1: {
                return vec3<f32>(v2, hsv.z, v1);
            }
            case 2: {
                return vec3<f32>(v1, hsv.z, v3);
            }
            case 3: {
                return vec3<f32>(v1, v2, hsv.z);
            }
            case 4: {
                return vec3<f32>(v3, v1, hsv.z);
            }
            default: {
                return vec3<f32>(hsv.z, v1, v2);
            }
        }
    }
}

fn get_fragment_colour(c: vec2<f32>, iterations: i32) -> vec4<f32> {
    var i: i32 = 0;

    for (
        var z: vec2<f32> = vec2<f32>(0.0, 0.0);
        cabs_squared(z) < 4.0;
        z = REPLACE_FRACTAL_EQN // gets replaced by user-defined expression
    ) {
        i++;
        if (i == iterations) {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
    }

    return vec4<f32>(hsv_rgb(vec3<f32>(log(f32(i) + 1.0) / log(f32(iterations) + 1.0), 0.8, 0.8)), 1.0);
}

@fragment
fn fs_main(@builtin(position) in: vec4<f32>) -> @location(0) vec4<f32> {
    return get_fragment_colour(in.xy * uniforms.scale - uniforms.centre, uniforms.iterations);
}