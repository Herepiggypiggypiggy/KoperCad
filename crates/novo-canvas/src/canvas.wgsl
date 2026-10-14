// NovoElectric canvas shader.
//
// World positions go through a pan/zoom transform. Strokes are extruded here
// so a mesh stays valid at every zoom: world-unit widths scale with zoom,
// screen-unit widths do not, and nothing gets thinner than one pixel (thin
// lines fade, down to 60 %). A one-pixel fringe on both sides gives anti-aliasing.

struct Uniforms {
    center: vec2<f32>,
    // World to clip scale (y flipped).
    scale: vec2<f32>,
    // Physical pixels to clip scale (y flipped).
    px_to_clip: vec2<f32>,
    // Physical pixels per world unit.
    zoom_px: f32,
    // Physical pixels per screen point.
    pixels_per_point: f32,
    // 1 when the render target is an sRGB format.
    srgb_target: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) extrude: vec2<f32>,
    @location(2) width_world: f32,
    @location(3) width_screen: f32,
    @location(4) edge: f32,
    @location(5) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
    // Signed distance from the stroke centre, in physical pixels.
    @location(1) dist: f32,
    // Half the drawn stroke width in physical pixels.
    @location(2) half_px: f32,
    // Opacity factor for strokes thinner than a pixel.
    @location(3) coverage: f32,
};

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var out: VsOut;
    let base = (v.pos - u.center) * u.scale;
    out.color = v.color;
    if (v.width_screen < 0.0) {
        // Fill vertex: no extrusion, no edge fade.
        out.clip = vec4<f32>(base, 0.0, 1.0);
        out.dist = 0.0;
        out.half_px = 1.0e6;
        out.coverage = 1.0;
        return out;
    }
    var true_half = (v.width_world * u.zoom_px + v.width_screen * u.pixels_per_point) * 0.5;
    // Zero width means a one-pixel hairline.
    if (true_half <= 0.0) {
        true_half = 0.5;
    }
    let half = max(true_half, 0.5);
    let reach = half + 1.0;
    out.clip = vec4<f32>(base + v.extrude * reach * u.px_to_clip, 0.0, 1.0);
    out.dist = v.edge * reach;
    out.half_px = half;
    // Sub-pixel lines fade with their width, but never below 60 % so a
    // zoomed-out page stays readable.
    out.coverage = clamp(true_half * 2.0, 0.6, 1.0);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c < vec3<f32>(0.04045);
    let lower = c / vec3<f32>(12.92);
    let higher = pow((c + vec3<f32>(0.055)) / vec3<f32>(1.055), vec3<f32>(2.4));
    return select(higher, lower, cutoff);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let a = clamp(in.half_px + 0.5 - abs(in.dist), 0.0, 1.0) * in.coverage * in.color.a;
    var rgb = in.color.rgb;
    if (u.srgb_target == 1u) {
        rgb = srgb_to_linear(rgb);
    }
    // Premultiplied alpha, matching egui's blending.
    return vec4<f32>(rgb * a, a);
}
