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
}

@group(0) @binding(0) var<uniform> g: Globals;

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
    var shade = ambient * hemi + (1.0 - ambient) * (0.7 * diffuse + 0.3 * head);
    // Contact shade: the lower part of things standing on the plate is darker.
    if (f.world.z > 0.5) {
        let k = smoothstep(0.0, g.params.y, f.world.z);
        shade = shade * (1.0 - g.params.x * (1.0 - k));
    }
    let rgb = to_srgb(to_linear(f.color.rgb) * shade);
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
