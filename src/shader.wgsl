struct Uniforms {
    scale: f32,
    escape_threshold: f32,
    centre: vec2<f32>,
    iterations: i32,
    flags: u32,
    initial_value: vec2<f32>,
}

const JULIA_SET = 1u;
const SMOOTHEN = 2u;
const INTERNAL_BLACK = 4u;
const INITIAL_C = 8u;

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

// deprecated: use length(z) instead
fn cabs(z: vec2<f32>) -> f32 {
    return length(z);
}

fn cpow(z: vec2<f32>, p: f32) -> vec2<f32> {
    let r: f32 = length(z);
    let arg: f32 = atan2(z.y, z.x);
    return vec2<f32>(pow(r, p) * cos(p * arg), pow(r, p) * sin(p * arg));
}

fn ccpow(z: vec2<f32>, w: vec2<f32>) -> vec2<f32> {
    let r: f32 = length(z);
    var len: f32 = pow(r, w.x);
    let arg: f32 = atan2(z.y, z.x);
    var phase: f32 = arg * w.x;
    if (w.y != 0.0) {
        len /= exp(arg * w.y);
        phase += w.y * log(r);
    }
    return vec2<f32>(len * cos(phase), len * sin(phase));
}

fn cdiv(w: vec2<f32>, z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(w.x * z.x + w.y * z.y, w.y * z.x - w.x * z.y) / (z.x * z.x + z.y * z.y);
}

fn cmul(w: vec2<f32>, z: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(z.x * w.x - z.y * w.y, z.x * w.y + z.y * w.x);
}

fn csquare(z: vec2<f32>) -> vec2<f32> {
    return cmul(z, z);
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

fn get_fragment_colour(c: vec2<f32>) -> vec4<f32> {
    var i: i32 = 0;
    var z: vec2<f32>;

    if ((uniforms.flags & JULIA_SET) == 0u) {
        if ((uniforms.flags & INITIAL_C) != 0u) {
            z = c;
            i++;
        }

        for (
            z += uniforms.initial_value;
            cabs_squared(z) < uniforms.escape_threshold * uniforms.escape_threshold;
            z = REPLACE_FRACTAL_EQN // gets replaced by user-defined expression
        ) {
            i++;
            if (i == uniforms.iterations) {
                if ((uniforms.flags & INTERNAL_BLACK) != 0u) {
                    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
                } else {
                    break;
                }
            }
        }
    } else {
        z = c;
        var c: vec2<f32> = uniforms.initial_value;
        for (;
            cabs_squared(z) < uniforms.escape_threshold * uniforms.escape_threshold;
            z = REPLACE_FRACTAL_EQN // gets replaced by user-defined expression
        ) {
            i++;
            if (i == uniforms.iterations) {
                if ((uniforms.flags & INTERNAL_BLACK) != 0u) {
                    return vec4<f32>(0.0, 0.0, 0.0, 1.0);
                } else {
                    break;
                }
            }
        }
    }

    var n = f32(i);

    if ((uniforms.flags & SMOOTHEN) != 0u && i > 0) {
        z = REPLACE_FRACTAL_EQN;
        z = REPLACE_FRACTAL_EQN;

        n += 2.0 - log2(log(length(z)));
    }

    return vec4(REPLACE_COLOR, 1.0); // gets replaced by user-defined expression
}

@fragment
fn fs_main(@builtin(position) in: vec4<f32>) -> @location(0) vec4<f32> {
    return get_fragment_colour(in.xy * uniforms.scale - uniforms.centre);
}