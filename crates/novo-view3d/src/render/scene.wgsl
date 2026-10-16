// Scene shader of novo-view3d: flat-shaded faces, edge lines and the
// background gradient. All colours arrive from `Look` through the uniform
// buffer and the instance data; there are no colour literals here.
// Depth is reversed (1 near, 0 far).

struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // xyz: direction towards the light, w: ambient share.
    light: vec4<f32>,
    bg_top: vec4<f32>,
    bg_bottom: vec4<f32>,
    // x: contact shade, y: height of the contact zone (mm), z: unused,
    // w: 1 when the camera is orthographic.
    params: vec4<f32>,
    // Edge depth nudge towards the camera: x relative (perspective, where
    // depth is 1/distance), y absolute in depth units (orthographic).
    nudge: vec4<f32>,
    // xyz: view direction (towards the scene), used for orthographic views.
    view_dir: vec4<f32>,
    // xy: size of the image being drawn, in pixels.
    viewport: vec4<f32>,
    // World to the key light's clip space.
    light_vp: mat4x4<f32>,
    // x: shadow strength (0 = off), y: depth bias, z: texel size in uv.
    shadow: vec4<f32>,
}

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_sampler: sampler_comparison;

struct Vertex {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

struct Instance {
    @location(2) r0: vec4<f32>,
    @location(3) r1: vec4<f32>,
    @location(4) r2: vec4<f32>,
    @location(5) scale: vec3<f32>,
    @location(6) color: vec4<f32>,
    @location(7) edge: vec4<f32>,
    @location(8) spec: vec4<f32>,
}

fn world_of(v: Vertex, i: Instance) -> vec3<f32> {
    let p = v.pos * i.scale;
    return vec3<f32>(
        dot(i.r0.xyz, p) + i.r0.w,
        dot(i.r1.xyz, p) + i.r1.w,
        dot(i.r2.xyz, p) + i.r2.w,
    );
}

struct FaceOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
    @location(3) spec: f32,
}

// Depth only, from the key light.
@vertex
fn vs_shadow(v: Vertex, i: Instance) -> @builtin(position) vec4<f32> {
    return g.light_vp * vec4<f32>(world_of(v, i), 1.0);
}

// Share of the key light that reaches `world` (1 = fully lit): 3x3 PCF over
// the shadow map, 1 outside it.
fn key_visibility(world: vec3<f32>, n: vec3<f32>) -> f32 {
    if (g.shadow.x <= 0.0) {
        return 1.0;
    }
    // Push the point off the surface along its normal to avoid acne.
    let p = g.light_vp * vec4<f32>(world + n * 1.5, 1.0);
    let uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    let ref_depth = p.z - g.shadow.y;
    var lit = 0.0;
    for (var dy = -1; dy <= 1; dy++) {
        for (var dx = -1; dx <= 1; dx++) {
            let o = vec2<f32>(f32(dx), f32(dy)) * g.shadow.z * 1.5;
            lit += textureSampleCompareLevel(shadow_map, shadow_sampler, uv + o, ref_depth);
        }
    }
    let inside = uv.x >= 0.0 && uv.x <= 1.0 && uv.y >= 0.0 && uv.y <= 1.0 && p.z <= 1.0;
    let v = select(1.0, lit / 9.0, inside);
    return mix(1.0, v, g.shadow.x);
}

@vertex
fn vs_face(v: Vertex, i: Instance) -> FaceOut {
    var o: FaceOut;
    let w = world_of(v, i);
    // Normals of a scaled mesh: the inverse transpose of R * S is R * S^-1.
    let n = v.normal / max(i.scale, vec3<f32>(1e-4));
    o.normal = normalize(vec3<f32>(dot(i.r0.xyz, n), dot(i.r1.xyz, n), dot(i.r2.xyz, n)));
    o.world = w;
    o.clip = g.view_proj * vec4<f32>(w, 1.0);
    o.color = i.color;
    o.spec = i.spec.x;
    return o;
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return pow(c, vec3<f32>(2.2));
}

fn to_srgb(c: vec3<f32>) -> vec3<f32> {
    return pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.2));
}

@fragment
fn fs_face(f: FaceOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
    var n = normalize(f.normal);
    if (!front) {
        n = -n;
    }
    let l = normalize(g.light.xyz);
    var to_eye = normalize(g.eye.xyz - f.world);
    if (g.params.w > 0.5) {
        to_eye = -g.view_dir.xyz;
    }
    let ambient = g.light.w;
    let hemi = mix(0.55, 1.0, n.y * 0.5 + 0.5);
    let diffuse = max(dot(n, l), 0.0);
    let head = max(dot(n, to_eye), 0.0);
    let vis = key_visibility(f.world, n);
    var shade = ambient * hemi + (1.0 - ambient) * (0.7 * diffuse * vis + 0.3 * head);
    // Contact shade: the lower part of things standing on the plate is darker.
    if (f.world.z > 0.5) {
        let k = smoothstep(0.0, g.params.y, f.world.z);
        shade = shade * (1.0 - g.params.x * (1.0 - k));
    }
    // Weak Blinn highlight (power 24) from the key light only.
    let half_vec = normalize(l + to_eye);
    let highlight = f.spec * pow(max(dot(n, half_vec), 0.0), 24.0) * (1.0 - ambient * 0.5) * vis;
    let rgb = to_srgb(to_linear(f.color.rgb) * shade + vec3<f32>(highlight));
    return vec4<f32>(rgb, f.color.a);
}

struct EdgeOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_edge(v: Vertex, i: Instance) -> EdgeOut {
    var o: EdgeOut;
    var c = g.view_proj * vec4<f32>(world_of(v, i), 1.0);
    // Pull lines towards the camera so they win against the faces they lie on.
    c.z = c.z * (1.0 + g.nudge.x) + g.nudge.y * c.w;
    o.clip = c;
    o.color = i.edge;
    return o;
}

@fragment
fn fs_edge(e: EdgeOut) -> @location(0) vec4<f32> {
    return e.color;
}

struct BgOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) t: f32,
}

@vertex
fn vs_bg(@builtin(vertex_index) idx: u32) -> BgOut {
    var o: BgOut;
    let p = vec2<f32>(f32((idx << 1u) & 2u), f32(idx & 2u));
    o.clip = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.t = 1.0 - p.y;
    return o;
}

@fragment
fn fs_bg(b: BgOut) -> @location(0) vec4<f32> {
    return mix(g.bg_top, g.bg_bottom, clamp(b.t, 0.0, 1.0));
}

// Overlays (heat layer and the like): vertex colours, flat shading from the
// triangle itself, no instancing.

struct OverlayVertex {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
}

struct OverlayOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_overlay(v: OverlayVertex) -> OverlayOut {
    var o: OverlayOut;
    o.clip = g.view_proj * vec4<f32>(v.pos, 1.0);
    o.world = v.pos;
    o.color = v.color;
    return o;
}

@fragment
fn fs_overlay(f: OverlayOut) -> @location(0) vec4<f32> {
    // The triangle's own normal, from the screen derivatives of its position.
    var n = normalize(cross(dpdx(f.world), dpdy(f.world)));
    var to_eye = normalize(g.eye.xyz - f.world);
    if (g.params.w > 0.5) {
        to_eye = -g.view_dir.xyz;
    }
    // Two-sided: turn the normal towards the viewer.
    if (dot(n, to_eye) < 0.0) {
        n = -n;
    }
    let l = normalize(g.light.xyz);
    let ambient = g.light.w;
    let hemi = mix(0.55, 1.0, n.y * 0.5 + 0.5);
    let diffuse = max(dot(n, l), 0.0);
    let head = max(dot(n, to_eye), 0.0);
    let shade = ambient * hemi + (1.0 - ambient) * (0.7 * diffuse + 0.3 * head);
    let rgb = to_srgb(to_linear(f.color.rgb) * shade);
    return vec4<f32>(rgb, f.color.a);
}

// Polylines as ribbons of constant pixel width. One instance is one segment;
// six vertices make its quad.

struct RibbonIn {
    @location(0) pa: vec3<f32>,
    @location(1) pb: vec3<f32>,
    @location(2) ca: vec4<f32>,
    @location(3) cb: vec4<f32>,
    @location(4) width: f32,
}

struct RibbonOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_ribbon(@builtin(vertex_index) idx: u32, s: RibbonIn) -> RibbonOut {
    var o: RibbonOut;
    var ts = array<f32, 6>(0.0, 0.0, 1.0, 0.0, 1.0, 1.0);
    var sides = array<f32, 6>(-1.0, 1.0, -1.0, 1.0, 1.0, -1.0);
    let t = ts[idx];
    let side = sides[idx];
    var a = g.view_proj * vec4<f32>(s.pa, 1.0);
    var b = g.view_proj * vec4<f32>(s.pb, 1.0);
    let eps = 1e-3;
    if (a.w < eps && b.w < eps) {
        // Wholly behind the camera: a degenerate quad outside the view.
        o.clip = vec4<f32>(2.0, 2.0, 2.0, 1.0);
        o.color = vec4<f32>(0.0);
        return o;
    }
    if (a.w < eps) {
        a = mix(a, b, (eps - a.w) / (b.w - a.w));
    }
    if (b.w < eps) {
        b = mix(b, a, (eps - b.w) / (a.w - b.w));
    }
    let half = g.viewport.xy * 0.5;
    let na = a.xy / a.w;
    let nb = b.xy / b.w;
    var d = (nb - na) * half;
    let len = length(d);
    if (len > 1e-4) {
        d = d / len;
    } else {
        d = vec2<f32>(1.0, 0.0);
    }
    let normal = vec2<f32>(-d.y, d.x);
    let offset = (normal * side + d * (t * 2.0 - 1.0)) * (s.width * 0.5) / half;
    let base = mix(a, b, t);
    let ndc = mix(na, nb, t) + offset;
    o.clip = vec4<f32>(ndc * base.w, base.z, base.w);
    o.color = mix(s.ca, s.cb, t);
    return o;
}

@fragment
fn fs_ribbon(f: RibbonOut) -> @location(0) vec4<f32> {
    return f.color;
}
