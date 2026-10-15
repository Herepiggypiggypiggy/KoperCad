// Copies the finished 3D image into the host's render pass.

struct Blit {
    // x: 1 when the target is an sRGB format (the hardware encodes on write,
    // so the gamma-encoded image is decoded first).
    srgb_target: vec4<f32>,
}

@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var<uniform> u: Blit;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs(@builtin(vertex_index) idx: u32) -> Out {
    var o: Out;
    let p = vec2<f32>(f32((idx << 1u) & 2u), f32(idx & 2u));
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let c = textureSample(image, samp, i.uv);
    if (u.srgb_target.x > 0.5) {
        return vec4<f32>(pow(c.rgb, vec3<f32>(2.2)), 1.0);
    }
    return vec4<f32>(c.rgb, 1.0);
}
