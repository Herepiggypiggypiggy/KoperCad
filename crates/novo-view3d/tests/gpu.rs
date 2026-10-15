//! Headless GPU tests: render to an offscreen target and read pixels back.
//! They skip (pass with a note) on machines without any wgpu adapter.

use glam::Vec3;
use novo_view3d::render::{Frame, Mode, Quality, Renderer, Tier, ViewTarget, read_rgba};
use novo_view3d::{
    Camera, Item, ItemId, ItemKind, Look, MeshLibrary, PlateFrame, Pose, Scene, Shape, ViewPreset,
    ViewState,
};

struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
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
    println!("adapter: {}", adapter.get_info().name);
    Some(Gpu { device, queue })
}

const SIZE: (u32, u32) = (256, 192);

fn render(gpu: &Gpu, scene: &Scene, state: &ViewState, camera: &Camera, mode: Mode) -> Vec<u8> {
    let look = Look::dark();
    let library = MeshLibrary::new();
    let mut renderer = Renderer::new(&gpu.device);
    let mut target = ViewTarget::new(&renderer, &gpu.device);
    let mut quality = Quality::new(mode, Tier::Discrete);
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
            look: &look,
            size_px: SIZE,
            quality,
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
