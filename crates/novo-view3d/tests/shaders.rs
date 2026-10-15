//! The shaders parse and validate without a GPU.

fn check(name: &str, source: &str) {
    let module = naga::front::wgsl::parse_str(source).unwrap_or_else(|e| panic!("{name}: {e}"));
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap_or_else(|e| panic!("{name}: {e:?}"));
}

#[test]
fn scene_shader_validates() {
    check("scene.wgsl", include_str!("../src/render/scene.wgsl"));
}

#[test]
fn blit_shader_validates() {
    check("blit.wgsl", include_str!("../src/render/blit.wgsl"));
}
