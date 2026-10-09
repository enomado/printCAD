// The cached scene, copied texel for texel onto the window under the UI.

@group(0) @binding(0) var scene: texture_2d<f32>;

// One triangle covering the whole target.
@vertex
fn vs_blit(@builtin(vertex_index) vertex: u32) -> @builtin(position) vec4<f32> {
    let x = f32((vertex << 1u) & 2u);
    let y = f32(vertex & 2u);
    return vec4<f32>(x * 2.0 - 1.0, y * 2.0 - 1.0, 0.0, 1.0);
}

// Onto an sRGB window, which encodes what it is given.
@fragment
fn fs_blit(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return textureLoad(scene, vec2<i32>(position.xy), 0);
}

fn srgb(linear: f32) -> f32 {
    if (linear <= 0.0031308) {
        return linear * 12.92;
    }
    return 1.055 * pow(linear, 1.0 / 2.4) - 0.055;
}

// Onto a plain window: the scene texture decodes on load, so encode again.
@fragment
fn fs_blit_encode(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let c = textureLoad(scene, vec2<i32>(position.xy), 0);
    return vec4<f32>(srgb(c.r), srgb(c.g), srgb(c.b), c.a);
}
