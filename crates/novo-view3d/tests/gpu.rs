//! Headless GPU tests: render to an offscreen target and read pixels back.
//! They skip (pass with a note) on machines without any wgpu adapter.

use glam::Vec3;
use novo_view3d::render::{Frame, Mode, Quality, Renderer, Tier, ViewTarget, read_rgba};
use novo_view3d::{
    Camera, Item, ItemId, ItemKind, Look, MeshLibrary, Overlay, OverlayLines, OverlayMesh,
    PlateFrame, Pose, Scene, Shape, ViewPreset, ViewState,
};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    adapter_name: String,
}

fn gpu() -> Option<Gpu> {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    let adapter_name = adapter.get_info().name;
    println!("adapter: {adapter_name}");
    Some(Gpu {
        device,
        queue,
        adapter_name,
    })
}

/// The adapter for the wire colour checks. Windows' software fallback
/// ("Microsoft Basic Render Driver", all a GitHub Windows runner has) draws the
/// translucent door and the wire tubes differently, so the pixel counts below
/// do not hold there; these checks run on every real GPU and on Linux.
fn colour_gpu() -> Option<Gpu> {
    let gpu = gpu()?;
    if gpu.adapter_name.contains("Basic Render Driver") {
        println!(
            "software adapter {}, wire colours skipped",
            gpu.adapter_name
        );
        return None;
    }
    Some(gpu)
}

const SIZE: (u32, u32) = (256, 192);

fn render(gpu: &Gpu, scene: &Scene, state: &ViewState, camera: &Camera, mode: Mode) -> Vec<u8> {
    render_with(gpu, scene, state, camera, mode, &[])
}

fn render_with(
    gpu: &Gpu,
    scene: &Scene,
    state: &ViewState,
    camera: &Camera,
    mode: Mode,
    overlays: &[Overlay],
) -> Vec<u8> {
    render_look(gpu, scene, state, camera, mode, overlays, &Look::dark())
}

fn render_look(
    gpu: &Gpu,
    scene: &Scene,
    state: &ViewState,
    camera: &Camera,
    mode: Mode,
    overlays: &[Overlay],
    look: &Look,
) -> Vec<u8> {
    render_full(gpu, scene, state, camera, mode, overlays, look, false)
}

fn render_ssao(
    gpu: &Gpu,
    scene: &Scene,
    state: &ViewState,
    camera: &Camera,
    mode: Mode,
    overlays: &[Overlay],
    ssao: bool,
) -> Vec<u8> {
    render_full(
        gpu,
        scene,
        state,
        camera,
        mode,
        overlays,
        &Look::dark(),
        ssao,
    )
}

#[allow(clippy::too_many_arguments)]
fn render_full(
    gpu: &Gpu,
    scene: &Scene,
    state: &ViewState,
    camera: &Camera,
    mode: Mode,
    overlays: &[Overlay],
    look: &Look,
    ssao: bool,
) -> Vec<u8> {
    let library = MeshLibrary::new();
    let mut renderer = Renderer::new(&gpu.device);
    let mut target = ViewTarget::new(&renderer, &gpu.device);
    let mut quality = Quality::new(mode, Tier::Discrete).with_ssao(ssao);
    quality.scale = 1.0;
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    renderer.render(
        &gpu.device,
        &gpu.queue,
        &mut encoder,
        &mut target,
        &Frame {
            scene,
            state,
            camera,
            library: &library,
            look,
            size_px: SIZE,
            quality,
            overlays,
            overlay_revision: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    assert_eq!(target.size(), SIZE);
    read_rgba(&gpu.device, &gpu.queue, &target)
}

fn px(img: &[u8], x: u32, y: u32) -> [u8; 3] {
    let i = ((y * SIZE.0 + x) * 4) as usize;
    [img[i], img[i + 1], img[i + 2]]
}

fn boxed(raw: u64, centre: Vec3, size: Vec3, tint: [u8; 3]) -> Item {
    let mut i = Item::new(
        ItemId::new(ItemKind::Device, raw),
        Shape::Box,
        Pose::at(centre),
        size,
    );
    i.tint = Some(tint);
    i
}

fn front_camera(scene: &Scene) -> Camera {
    let mut c = Camera::default();
    c.set_preset(ViewPreset::Front);
    c.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    c
}

#[test]
fn a_box_covers_the_centre_and_leaves_the_corners_as_background() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(100.0, 100.0, 40.0),
        [200, 40, 40],
    ));
    let img = render(
        &gpu,
        &scene,
        &ViewState::default(),
        &front_camera(&scene),
        Mode::Interactive,
    );
    let centre = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(
        centre[0] > centre[1] + 40,
        "red face expected, got {centre:?}"
    );
    let corner = px(&img, 2, 2);
    assert!(
        corner[0] < 80 && corner[1] < 80 && corner[2] < 80,
        "background expected, got {corner:?}"
    );
}

#[test]
fn nearer_items_hide_farther_ones_whatever_the_draw_order() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    for near_first in [true, false] {
        let mut scene = Scene::new(PlateFrame {
            width: 100.0,
            height: 100.0,
        });
        let far = boxed(
            1,
            Vec3::new(50.0, 50.0, 0.0),
            Vec3::new(100.0, 100.0, 10.0),
            [40, 40, 220],
        );
        let near = boxed(
            2,
            Vec3::new(50.0, 50.0, 60.0),
            Vec3::new(30.0, 30.0, 10.0),
            [220, 40, 40],
        );
        if near_first {
            scene.push(near);
            scene.push(far);
        } else {
            scene.push(far);
            scene.push(near);
        }
        let img = render(
            &gpu,
            &scene,
            &ViewState::default(),
            &front_camera(&scene),
            Mode::Interactive,
        );
        let centre = px(&img, SIZE.0 / 2, SIZE.1 / 2);
        assert!(
            centre[0] > centre[2],
            "near box (red) must win, got {centre:?} (near_first {near_first})"
        );
        let beside = px(&img, SIZE.0 / 2 + 50, SIZE.1 / 2);
        assert!(
            beside[2] > beside[0],
            "far box (blue) beside it, got {beside:?}"
        );
    }
}

#[test]
fn selection_changes_the_picture() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(60.0, 60.0, 20.0),
        [200, 200, 200],
    ));
    let camera = front_camera(&scene);
    let plain = render(
        &gpu,
        &scene,
        &ViewState::default(),
        &camera,
        Mode::Interactive,
    );
    let mut state = ViewState::default();
    state.selection.insert(ItemId::new(ItemKind::Device, 1));
    let selected = render(&gpu, &scene, &state, &camera, Mode::Interactive);
    assert_ne!(plain, selected);
    // And hidden kinds vanish: only the background is left.
    let mut hidden = ViewState::default();
    hidden.hidden.insert(ItemKind::Device);
    let img = render(&gpu, &scene, &hidden, &camera, Mode::Interactive);
    assert!(px(&img, SIZE.0 / 2, SIZE.1 / 2)[0] < 80);
}

#[test]
fn settled_quality_renders_with_msaa() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (scene, _) = novo_view3d::demo::cabinet(&Default::default());
    let mut camera = Camera::default();
    camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    let img = render(&gpu, &scene, &ViewState::default(), &camera, Mode::Settled);
    // Not a flat image: the cabinet and the background differ.
    let distinct: std::collections::HashSet<[u8; 3]> = (0..SIZE.0)
        .step_by(7)
        .flat_map(|x| (0..SIZE.1).step_by(7).map(move |y| (x, y)))
        .map(|(x, y)| px(&img, x, y))
        .collect();
    assert!(distinct.len() > 20, "only {} colours", distinct.len());
}

#[test]
fn orthographic_view_renders_too() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(100.0, 100.0, 40.0),
        [200, 40, 40],
    ));
    let mut camera = front_camera(&scene);
    camera.projection = novo_view3d::Projection::Orthographic;
    camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    let img = render(
        &gpu,
        &scene,
        &ViewState::default(),
        &camera,
        Mode::Interactive,
    );
    let centre = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(centre[0] > centre[1] + 40, "got {centre:?}");
}

#[test]
fn a_wire_is_drawn_as_a_tube_in_front_of_the_device_it_crosses() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(100.0, 100.0, 40.0),
        [200, 40, 40],
    ));
    scene.wires.push(novo_view3d::Wire {
        id: ItemId::new(ItemKind::Wire, 2),
        points: vec![Vec3::new(0.0, 50.0, 60.0), Vec3::new(100.0, 50.0, 60.0)],
        radius: 6.0,
        tint: Some([40, 220, 60]),
        stripe: None,
        mark: novo_view3d::Mark::None,
        label: None,
    });
    let camera = front_camera(&scene);
    let img = render(
        &gpu,
        &scene,
        &ViewState::default(),
        &camera,
        Mode::Interactive,
    );
    let centre = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(
        centre[1] > centre[0] + 40,
        "green wire expected over the red box, got {centre:?}"
    );
    let mut state = ViewState::default();
    state.hidden.insert(ItemKind::Wire);
    let img = render(&gpu, &scene, &state, &camera, Mode::Interactive);
    let centre = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(centre[0] > centre[1] + 40, "wires hidden, got {centre:?}");
}

fn quad(z: f32, rgba: [u8; 4], translucent: bool, order: i32) -> Overlay {
    Overlay::Mesh(OverlayMesh {
        positions: vec![
            Vec3::new(25.0, 25.0, z),
            Vec3::new(75.0, 25.0, z),
            Vec3::new(75.0, 75.0, z),
            Vec3::new(25.0, 75.0, z),
        ],
        colors: vec![rgba; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        translucent,
        order,
    })
}

fn overlay_scene() -> (Scene, Camera) {
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(100.0, 100.0, 40.0),
        [200, 40, 40],
    ));
    let camera = front_camera(&scene);
    (scene, camera)
}

#[test]
fn an_opaque_overlay_mesh_covers_the_scene_and_clearing_it_brings_the_scene_back() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (scene, camera) = overlay_scene();
    let state = ViewState::default();
    let with = render_with(
        &gpu,
        &scene,
        &state,
        &camera,
        Mode::Interactive,
        &[quad(80.0, [30, 200, 60, 255], false, 0)],
    );
    let c = px(&with, SIZE.0 / 2, SIZE.1 / 2);
    assert!(c[1] > c[0] + 40, "green overlay expected, got {c:?}");
    let without = render_with(&gpu, &scene, &state, &camera, Mode::Interactive, &[]);
    let c = px(&without, SIZE.0 / 2, SIZE.1 / 2);
    assert!(c[0] > c[1] + 40, "red box expected, got {c:?}");
}

#[test]
fn a_translucent_overlay_blends_with_what_is_behind_it_and_hides_behind_nearer_things() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (scene, camera) = overlay_scene();
    let state = ViewState::default();
    let img = render_with(
        &gpu,
        &scene,
        &state,
        &camera,
        Mode::Interactive,
        &[quad(80.0, [30, 200, 60, 128], true, 0)],
    );
    let c = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(
        c[0] > 60 && c[1] > 60,
        "red box and green overlay should mix, got {c:?}"
    );
    // Inside the box (z = 20 is below its front face at 40): the depth test
    // hides the overlay.
    let img = render_with(
        &gpu,
        &scene,
        &state,
        &camera,
        Mode::Interactive,
        &[quad(20.0, [30, 200, 60, 128], true, 0)],
    );
    let c = px(&img, SIZE.0 / 2, SIZE.1 / 2);
    assert!(c[0] > c[1] + 40, "overlay inside the box is hidden: {c:?}");
}

#[test]
fn overlay_lines_have_the_requested_pixel_width() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (scene, camera) = overlay_scene();
    let state = ViewState::default();
    let line = Overlay::Lines(OverlayLines {
        positions: vec![Vec3::new(0.0, 50.0, 80.0), Vec3::new(100.0, 50.0, 80.0)],
        colors: vec![[20, 60, 255, 255]; 2],
        starts: vec![],
        width_px: 10.0,
        order: 0,
    });
    let img = render_with(&gpu, &scene, &state, &camera, Mode::Interactive, &[line]);
    let (cx, cy) = (SIZE.0 / 2, SIZE.1 / 2);
    let on = px(&img, cx, cy);
    assert!(on[2] > on[0] + 80, "blue ribbon on the middle row: {on:?}");
    let near = px(&img, cx, cy + 4);
    assert!(near[2] > near[0] + 80, "still inside five pixels: {near:?}");
    let off = px(&img, cx, cy + 12);
    assert!(off[0] > off[2], "red box again beyond the ribbon: {off:?}");
}

/// A plate with one duct and a wire in it, behind the ghost shell and the
/// closed door.
fn cabinet_with_wire(colour: Option<novo_view3d::build::WireColour>) -> Scene {
    use novo_layout::Plate;
    use novo_layout::state::CabinetLayout;
    use novo_view3d::build::{DuctShape, Inputs, Options, WireInput, WireKind, from_layout};
    let layout = CabinetLayout::new(Plate {
        width: 400.0,
        height: 300.0,
        margin: 10.0,
    });
    let mut inputs = Inputs::default();
    inputs.ducts.push(DuctShape {
        id: 1,
        x: 20.0,
        y: 150.0,
        length: 360.0,
        vertical: false,
        width: 40.0,
        height: 60.0,
    });
    let wires = vec![WireInput {
        id: 5,
        kind: WireKind::Ducted,
        points: vec![(30.0, 150.0), (370.0, 150.0)],
        ducts: vec![1],
        diameter_mm: Some(6.0),
        colour,
    }];
    from_layout(
        &layout,
        &inputs,
        &Options {
            wires,
            ..Options::default()
        },
    )
}

/// How many pixels of an RGBA image satisfy `test`.
fn count(img: &[u8], test: impl Fn(i32, i32, i32) -> bool) -> usize {
    img.chunks(4)
        .filter(|p| test(p[0] as i32, p[1] as i32, p[2] as i32))
        .count()
}

/// The blue of the wire in the test: clearly more blue than red or green.
fn bluish(r: i32, g: i32, b: i32) -> bool {
    b > r + 50 && b > g + 20
}

#[test]
fn a_coloured_wire_shows_through_the_closed_door() {
    use novo_view3d::build::WireColour;
    let Some(gpu) = colour_gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let blue = [0x2F, 0x6F, 0xD0];
    let with = cabinet_with_wire(Some(WireColour {
        body: blue,
        stripe: None,
    }));
    assert_eq!(
        with.group(novo_view3d::build::DOOR_HINGE).unwrap().angle,
        0.0
    );
    let camera = front_camera(&with);
    let img = render(&gpu, &with, &ViewState::default(), &camera, Mode::Settled);
    assert!(
        count(&img, bluish) > 40,
        "a blue wire is visible through the closed door"
    );
    let mut none = with.clone();
    none.wires.clear();
    let img = render(&gpu, &none, &ViewState::default(), &camera, Mode::Settled);
    assert_eq!(count(&img, bluish), 0, "no blue without the wire");
}

#[test]
fn a_bi_colour_wire_shows_both_its_colours() {
    use novo_view3d::build::WireColour;
    let Some(gpu) = colour_gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (body, stripe) = ([0x7F, 0xB0, 0x20], [0xE3, 0xD3, 0x4A]);
    let scene = cabinet_with_wire(Some(WireColour {
        body,
        stripe: Some(stripe),
    }));
    let camera = front_camera(&scene);
    let img = render(&gpu, &scene, &ViewState::default(), &camera, Mode::Settled);
    // Green: green clearly above blue and not yellow; yellow: red as high as
    // green and both well above blue.
    let greenish = |r: i32, g: i32, b: i32| g > r + 15 && g > b + 40;
    let yellowish = |r: i32, g: i32, b: i32| r > 140 && g > 140 && b + 50 < r.min(g);
    assert!(count(&img, greenish) > 10, "the green body shows");
    assert!(count(&img, yellowish) > 10, "and so does the yellow stripe");
}

/// A tall box on a plate, seen from the front right above, with the key light
/// shadows on or off.
fn shadow_picture(gpu: &Gpu, shadows: bool) -> Vec<u8> {
    let mut scene = Scene::new(PlateFrame {
        width: 300.0,
        height: 300.0,
    });
    let mut plate = Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(Vec3::new(150.0, 150.0, -3.0)),
        Vec3::new(300.0, 300.0, 3.0),
    );
    plate.tint = Some([200, 200, 200]);
    scene.push(plate);
    scene.push(boxed(
        1,
        Vec3::new(150.0, 150.0, 0.0),
        Vec3::new(40.0, 40.0, 100.0),
        [180, 120, 90],
    ));
    let mut camera = Camera::default();
    camera.set_preset(ViewPreset::Iso);
    camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    let mut state = ViewState::default();
    state.display.shadows = shadows;
    state.display.edges = false;
    render(gpu, &scene, &state, &camera, Mode::Settled)
}

#[test]
fn the_key_light_casts_a_shadow_of_a_tall_box_on_the_plate() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let (lit, shaded) = (shadow_picture(&gpu, false), shadow_picture(&gpu, true));
    if let Ok(dir) = std::env::var("NOVO_DUMP_DIR") {
        for (name, img) in [("shadow_off", &lit), ("shadow_on", &shaded)] {
            let path = std::path::Path::new(&dir).join(format!("{name}.ppm"));
            let mut out = format!("P6\n{} {}\n255\n", SIZE.0, SIZE.1).into_bytes();
            out.extend(img.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
            std::fs::write(path, out).unwrap();
        }
    }
    let sum = |img: &[u8]| -> u64 {
        img.chunks(4)
            .map(|p| u64::from(p[0]) + u64::from(p[1]))
            .sum()
    };
    let (darker, brighter) = lit
        .chunks(4)
        .zip(shaded.chunks(4))
        .fold((0, 0), |(d, b), (a, s)| {
            let (a, s) = (i32::from(a[0]), i32::from(s[0]));
            (d + usize::from(s < a - 8), b + usize::from(s > a + 2))
        });
    assert!(sum(&shaded) < sum(&lit), "shadows darken the picture");
    assert!(
        darker > 100,
        "a patch of the plate is in shadow ({darker} px)"
    );
    assert_eq!(brighter, 0, "shadows never brighten anything");
}

#[test]
fn a_caster_outside_the_picture_still_shadows_what_is_in_it() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let frame = PlateFrame {
        width: 400.0,
        height: 300.0,
    };
    let mut scene = Scene::new(frame);
    let mut plate = Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(Vec3::new(200.0, 150.0, -3.0)),
        Vec3::new(400.0, 300.0, 3.0),
    );
    plate.tint = Some([200, 200, 200]);
    scene.push(plate);
    // The tall box stands left of the part of the plate the camera looks at.
    scene.push(boxed(
        1,
        Vec3::new(100.0, 150.0, 0.0),
        Vec3::new(40.0, 40.0, 120.0),
        [180, 120, 90],
    ));
    let mut camera = Camera::default();
    camera.set_preset(ViewPreset::Front);
    let view = novo_view3d::Aabb::new(Vec3::new(150.0, 60.0, 0.0), Vec3::new(260.0, 240.0, 0.0));
    camera.fit(&view, SIZE.0 as f32 / SIZE.1 as f32);
    let picture = |shadows: bool| {
        let mut state = ViewState::default();
        state.display.shadows = shadows;
        state.display.edges = false;
        render(&gpu, &scene, &state, &camera, Mode::Settled)
    };
    let (lit, shaded) = (picture(false), picture(true));
    let darker = lit
        .chunks(4)
        .zip(shaded.chunks(4))
        .filter(|(a, s)| i32::from(s[0]) < i32::from(a[0]) - 8)
        .count();
    assert!(
        darker > 200,
        "the shadow reaches into the picture ({darker} px)"
    );
}

#[test]
fn the_plate_is_darker_around_a_device_than_far_from_it() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 300.0,
        height: 300.0,
    });
    let mut plate = Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(Vec3::new(150.0, 150.0, -3.0)),
        Vec3::new(300.0, 300.0, 3.0),
    );
    plate.tint = Some([200, 200, 200]);
    scene.push(plate);
    scene.push(boxed(
        1,
        Vec3::new(150.0, 150.0, 0.0),
        Vec3::new(40.0, 40.0, 20.0),
        [40, 90, 200],
    ));
    let mut camera = Camera::default();
    camera.set_preset(ViewPreset::Iso);
    camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    let mut state = ViewState::default();
    state.display.shadows = false;
    state.display.edges = false;
    let (mut open, mut occluded) = (Look::light(), Look::light());
    open.contact_shade = 0.0;
    occluded.contact_shade = 0.8;
    let picture =
        |look: &Look| render_look(&gpu, &scene, &state, &camera, Mode::Settled, &[], look);
    let (a, b) = (picture(&open), picture(&occluded));
    // The open plate's grey: the most common red value of the open picture,
    // which is the plate (the background gradient is a different value per row).
    let mut hist = [0usize; 256];
    for x in a.chunks(4) {
        hist[usize::from(x[0])] += 1;
    }
    let plate_grey = (0..256).max_by_key(|&v| hist[v]).unwrap() as i32;
    // Only plain plate counts: pixels that are plate grey when open and have
    // no non-plate neighbour, so the device, its edge and its sides (which
    // the contact shade darkens too) cannot pass for occlusion on the plate.
    let (w, h) = (SIZE.0 as usize, SIZE.1 as usize);
    let plate = |x: usize, y: usize| (i32::from(a[(y * w + x) * 4]) - plate_grey).abs() <= 1;
    let (mut darker, mut brighter) = (0, 0);
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let (o, c) = (i32::from(a[(y * w + x) * 4]), i32::from(b[(y * w + x) * 4]));
            brighter += usize::from(c > o + 2);
            let clear = [(-1, 0), (1, 0), (0, -1), (0, 1)]
                .iter()
                .all(|&(dx, dy)| plate((x as i32 + dx) as usize, (y as i32 + dy) as usize));
            if plate(x, y) && clear {
                darker += usize::from(c < o - 4);
            }
        }
    }
    assert!(
        darker > 20,
        "a ring of plate around the device is darker ({darker} px)"
    );
    assert_eq!(brighter, 0, "occlusion never brightens anything");
    assert_eq!(px(&a, 2, 2), px(&b, 2, 2), "the far corner is untouched");
}

#[test]
fn screen_space_occlusion_darkens_the_corner_between_a_wall_and_the_plate() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 300.0,
        height: 300.0,
    });
    let mut plate = Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(Vec3::new(150.0, 150.0, -3.0)),
        Vec3::new(300.0, 300.0, 3.0),
    );
    plate.tint = Some([200, 200, 200]);
    scene.push(plate);
    scene.push(boxed(
        1,
        Vec3::new(150.0, 150.0, 0.0),
        Vec3::new(60.0, 60.0, 80.0),
        [180, 120, 90],
    ));
    let mut state = ViewState::default();
    state.display.shadows = false;
    state.display.edges = false;
    // The radius must not depend on where the camera looks from: the world-y
    // row of the combined matrix (a naive scale) shrinks with cos(pitch) and
    // is zero when the camera looks straight along y.
    for (yaw_deg, pitch) in [
        (-45.0_f32, 0.3_f32),
        (0.0, 0.3),
        (45.0, 0.3),
        (-45.0, 0.9),
        (0.0, 0.9),
        (45.0, 0.9),
        (0.0, 1.45),
    ] {
        let preset = format!("yaw {yaw_deg} pitch {pitch}");
        let least = 25;
        let mut camera = Camera {
            yaw: yaw_deg.to_radians(),
            pitch,
            ..Camera::default()
        };
        camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
        let picture = |ssao| render_ssao(&gpu, &scene, &state, &camera, Mode::Settled, &[], ssao);
        let (flat, occluded) = (picture(false), picture(true));
        if let Ok(dir) = std::env::var("NOVO_DUMP_DIR") {
            for (name, img) in [("ssao_off", &flat), ("ssao_on", &occluded)] {
                let path = std::path::Path::new(&dir).join(format!("{name}_{preset}.ppm"));
                let mut out = format!("P6\n{} {}\n255\n", SIZE.0, SIZE.1).into_bytes();
                out.extend(img.chunks(4).flat_map(|p| [p[0], p[1], p[2]]));
                std::fs::write(path, out).unwrap();
            }
        }
        let (mut darker, mut brighter) = (0, 0);
        for (a, b) in flat.chunks(4).zip(occluded.chunks(4)) {
            let (a, b) = (i32::from(a[0]), i32::from(b[0]));
            darker += usize::from(b < a - 10);
            brighter += usize::from(b > a + 3);
        }
        assert!(
            darker > least,
            "{preset}: the foot of the box is darker ({darker} px)"
        );
        assert!(
            brighter < 20,
            "{preset}: occlusion does not brighten ({brighter} px)"
        );
        assert_eq!(
            px(&flat, 2, 2),
            px(&occluded, 2, 2),
            "{preset}: background untouched"
        );
    }
}

#[test]
fn a_section_plane_cuts_the_near_box_away_and_shows_the_far_one() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 100.0,
        height: 100.0,
    });
    scene.push(boxed(
        1,
        Vec3::new(50.0, 50.0, 0.0),
        Vec3::new(100.0, 100.0, 10.0),
        [40, 40, 220],
    ));
    scene.push(boxed(
        2,
        Vec3::new(50.0, 50.0, 60.0),
        Vec3::new(30.0, 30.0, 10.0),
        [220, 40, 40],
    ));
    let cam = front_camera(&scene);
    let whole = render(&gpu, &scene, &ViewState::default(), &cam, Mode::Interactive);
    let c = px(&whole, SIZE.0 / 2, SIZE.1 / 2);
    assert!(c[0] > c[2], "the near box is red, got {c:?}");
    let state = ViewState {
        section: Some(novo_view3d::Section::default()),
        ..ViewState::default()
    };
    let cut = render(&gpu, &scene, &state, &cam, Mode::Interactive);
    let c = px(&cut, SIZE.0 / 2, SIZE.1 / 2);
    assert!(c[2] > c[0], "the plane hides the near box, got {c:?}");
}

#[test]
fn a_section_plane_cuts_the_depth_the_occlusion_reads() {
    let Some(gpu) = gpu() else {
        println!("no adapter, skipped");
        return;
    };
    let mut scene = Scene::new(PlateFrame {
        width: 300.0,
        height: 300.0,
    });
    let mut plate = Item::new(
        ItemId::new(ItemKind::Plate, 0),
        Shape::Box,
        Pose::at(Vec3::new(150.0, 150.0, -3.0)),
        Vec3::new(300.0, 300.0, 3.0),
    );
    plate.tint = Some([200, 200, 200]);
    scene.push(plate);
    // A tall box cut through the middle of the scene's depth: from above, the
    // plate shows where the cut-away lid would be, ringed by the lower walls.
    scene.push(boxed(
        1,
        Vec3::new(150.0, 150.0, 0.0),
        Vec3::new(60.0, 60.0, 80.0),
        [180, 120, 90],
    ));
    let mut state = ViewState::default();
    state.display.shadows = false;
    state.display.edges = false;
    state.section = Some(novo_view3d::Section::default());
    let mut camera = Camera {
        yaw: 0.0,
        pitch: 1.3,
        ..Camera::default()
    };
    camera.fit(&scene.bounds(), SIZE.0 as f32 / SIZE.1 as f32);
    let picture = |ssao| render_ssao(&gpu, &scene, &state, &camera, Mode::Settled, &[], ssao);
    let (flat, occluded) = (picture(false), picture(true));
    let darker = flat
        .chunks(4)
        .zip(occluded.chunks(4))
        .filter(|(a, b)| i32::from(b[0]) < i32::from(a[0]) - 10)
        .count();
    assert!(
        darker > 20,
        "the plate inside the cut box is occluded by its lower walls ({darker} px)"
    );
}
