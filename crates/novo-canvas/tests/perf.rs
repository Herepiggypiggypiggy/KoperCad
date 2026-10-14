//! CPU-side timings on the 10,000-object stress page. Run with
//! `cargo test --release -p novo-canvas --test perf -- --nocapture` for
//! meaningful numbers; in debug builds the bounds are deliberately loose.

use novo_canvas::{
    Aabb, BoxMode, Grid, Item, ItemId, ItemMeshCache, LayerId, Primitive, Shape, SnapSettings,
    Stroke, Tessellator, Vec2, Viewport, demo, snap,
};
use std::time::{Duration, Instant};

fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) -> Duration {
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    let per = start.elapsed() / runs;
    eprintln!("{label}: {:.3} ms", per.as_secs_f64() * 1000.0);
    per
}

#[test]
fn stress_page_cpu_budget() {
    let mut scene = demo::stress_scene(10_000);
    let t = Tessellator::default();
    let mut view = Viewport::new(Vec2::new(1920.0, 1080.0));
    view.fit(&scene.bounds(), 10.0);

    let full = time("tessellate full page", 3, || t.scene(&scene));
    time("  of which draw order", 3, || scene.draw_order().len());
    time("  of which items only", 3, || {
        let mut m = novo_canvas::Mesh::default();
        for (_, it) in scene.iter() {
            t.item(&mut m, it);
        }
        m.vertices.len()
    });

    let mut cache = ItemMeshCache::default();
    t.scene_cached(&scene, &mut cache);
    let mut k = 0u64;
    let edit = time("edit one item, rebuild page mesh", 5, || {
        k += 1;
        scene.insert(
            ItemId(7),
            Item::new(LayerId(0)).with(Primitive::stroked(
                Shape::line(Vec2::ZERO, Vec2::new(k as f32, 10.0)),
                Stroke::default(),
            )),
        );
        t.scene_cached(&scene, &mut cache)
    });

    let b = scene.bounds();
    let mut i = 0;
    let pick = time("pick under cursor", 1000, || {
        i += 1;
        let p = b.min + (b.size() * ((i % 97) as f32 / 97.0));
        scene.pick(p, 2.0, view.zoom)
    });
    let snap_t = time("snap cursor", 1000, || {
        i += 1;
        let p = b.min + (b.size() * ((i % 89) as f32 / 89.0));
        snap(
            &scene,
            &view,
            Some(&Grid::default()),
            &SnapSettings::default(),
            p,
            &[],
        )
    });
    let window = Aabb::from_corners(b.min, b.center());
    let boxed = time("box select a quarter of the page", 20, || {
        scene.box_select(&window, BoxMode::Crossing, view.zoom)
    });

    // Generous bounds so this guards against regressions by orders of
    // magnitude without being flaky on slow CI runners or debug builds.
    let slack = if cfg!(debug_assertions) { 20 } else { 1 };
    assert!(full < Duration::from_millis(250) * slack);
    assert!(edit < Duration::from_millis(100) * slack);
    assert!(pick < Duration::from_millis(2) * slack);
    assert!(snap_t < Duration::from_millis(2) * slack);
    assert!(boxed < Duration::from_millis(100) * slack);
}
