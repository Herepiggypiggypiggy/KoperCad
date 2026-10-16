use novo_layout::collide::*;

fn cube(min: [f64; 3], size: f64) -> Body {
    Body::from_mesh(Mesh::boxed(
        min,
        [min[0] + size, min[1] + size, min[2] + size],
    ))
    .unwrap()
}

#[test]
fn overlapping_boxes_collide() {
    let a = cube([0.0; 3], 10.0);
    let b = cube([5.0, 5.0, 5.0], 10.0);
    assert!(collide(&a, &b).is_some());
    assert_eq!(clearance(&a, &b).distance, 0.0);
}

#[test]
fn touching_boxes_do_not_collide() {
    let a = cube([0.0; 3], 10.0);
    let b = cube([10.0, 0.0, 0.0], 10.0);
    assert!(collide(&a, &b).is_none());
    assert!(clearance(&a, &b).distance < 1e-9);
    assert!(keeps_clearance(&a, &b, 0.0));
    assert!(!keeps_clearance(&a, &b, 5.0));
}

#[test]
fn separated_boxes_report_the_gap() {
    let a = cube([0.0; 3], 10.0);
    let face = cube([25.0, 0.0, 0.0], 10.0);
    assert!((clearance(&a, &face).distance - 15.0).abs() < 1e-9);
    // Corner to corner: 3-4-12 gives 13 per... (3,4,12) -> 13.
    let corner = cube([13.0, 14.0, 22.0], 5.0);
    assert!((clearance(&a, &corner).distance - 13.0).abs() < 1e-9);
    assert!(keeps_clearance(&a, &face, 15.0));
    assert!(!keeps_clearance(&a, &face, 15.5));
}

#[test]
fn a_body_inside_another_collides_without_surfaces_crossing() {
    let big = cube([0.0; 3], 100.0);
    let small = cube([40.0, 40.0, 40.0], 10.0);
    let c = collide(&big, &small).unwrap();
    assert!(c.contained && c.triangles.is_none());
    assert!(collide(&small, &big).unwrap().contained);
    assert_eq!(clearance(&big, &small).distance, 0.0);
    // Same footprint, one end buried in the other: only side faces touch.
    let post = Body::from_mesh(Mesh::boxed([10.0, 10.0, 0.0], [20.0, 20.0, 60.0])).unwrap();
    let cap = Body::from_mesh(Mesh::boxed([10.0, 10.0, 40.0], [20.0, 20.0, 100.0])).unwrap();
    assert!(collide(&post, &cap).is_some());
    let beyond = cube([200.0, 0.0, 0.0], 10.0);
    assert!(collide(&big, &beyond).is_none());
}

#[test]
fn crossing_bars_collide_without_any_vertex_inside() {
    let a = Body::from_mesh(Mesh::boxed([-50.0, -1.0, -1.0], [50.0, 1.0, 1.0])).unwrap();
    let b = Body::from_mesh(Mesh::boxed([-1.0, -50.0, -1.0], [1.0, 50.0, 1.0])).unwrap();
    assert!(collide(&a, &b).is_some());
    let lifted = b.translated([0.0, 0.0, 3.0]);
    assert!(collide(&a, &lifted).is_none());
    assert!((clearance(&a, &lifted).distance - 1.0).abs() < 1e-9);
}

#[test]
fn coplanar_faces_touch_but_do_not_penetrate() {
    let flat = |off: f64| {
        Body::from_mesh(Mesh {
            vertices: vec![[off, 0.0, 0.0], [off + 10.0, 0.0, 0.0], [off, 10.0, 0.0]],
            triangles: vec![[0, 1, 2]],
        })
        .unwrap()
    };
    assert!(collide(&flat(0.0), &flat(2.0)).is_none());
    assert!(clearance(&flat(0.0), &flat(2.0)).distance < 1e-9);
    assert!(collide(&flat(0.0), &flat(20.0)).is_none());
    assert!((clearance(&flat(0.0), &flat(20.0)).distance - 10.0).abs() < 1e-9);
}

#[test]
fn fallback_boxes_are_flagged() {
    let real = cube([0.0; 3], 10.0);
    let fb = Body::fallback_box([5.0, 5.0, 5.0], [15.0, 15.0, 15.0]);
    assert!(fb.is_fallback() && !real.is_fallback());
    assert!(collide(&real, &fb).unwrap().approximate);
    assert!(
        !collide(&real, &cube([5.0, 5.0, 5.0], 10.0))
            .unwrap()
            .approximate
    );
    assert!(clearance(&real, &fb).approximate);
    // No mesh, or a broken one, becomes a fallback box.
    let broken = Mesh {
        vertices: vec![[0.0; 3]],
        triangles: vec![[0, 1, 2]],
    };
    assert!(Body::from_mesh(broken.clone()).is_none());
    assert!(Body::from_mesh_or_box(Some(broken), [0.0; 3], [1.0; 3]).is_fallback());
    assert!(Body::from_mesh_or_box(None, [0.0; 3], [1.0; 3]).is_fallback());
}

#[test]
fn a_large_mesh_agrees_with_brute_force() {
    // A 20x20 grid of unit cubes with gaps against a probe cube.
    let mut vertices = Vec::new();
    let mut triangles = Vec::new();
    for i in 0..20 {
        for j in 0..20 {
            let m = Mesh::boxed(
                [i as f64 * 3.0, j as f64 * 3.0, 0.0],
                [i as f64 * 3.0 + 2.0, j as f64 * 3.0 + 2.0, 2.0],
            );
            let base = vertices.len() as u32;
            vertices.extend(m.vertices);
            triangles.extend(
                m.triangles
                    .iter()
                    .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
            );
        }
    }
    let grid = Body::from_mesh(Mesh {
        vertices,
        triangles,
    })
    .unwrap();
    // Probe sits in the 1 mm gap column between cubes, high above: clear of the nearest cube corner.
    let probe = Body::from_mesh(Mesh::boxed([29.2, 29.2, 5.0], [29.8, 29.8, 6.0])).unwrap();
    assert!(collide(&grid, &probe).is_none());
    let corner = (0.2f64 * 0.2 * 2.0 + 9.0).sqrt();
    assert!((clearance(&grid, &probe).distance - corner).abs() < 1e-9);
    let low = Body::from_mesh(Mesh::boxed([29.2, 29.2, 0.5], [29.8, 29.8, 1.5])).unwrap();
    assert!(collide(&grid, &low).is_none());
    let d = clearance(&grid, &low).distance;
    assert!((d - 0.2 * 2f64.sqrt()).abs() < 1e-9, "{d}");
    let hit = Body::from_mesh(Mesh::boxed([29.0, 29.0, 0.5], [31.0, 31.0, 1.5])).unwrap();
    assert!(collide(&grid, &hit).is_some());
}

#[test]
fn rotating_about_the_hinge_turns_the_body() {
    // A 100 mm bar along x from the hinge at (0, 0); after 90 degrees it
    // lies along z.
    let bar = Body::from_mesh(Mesh::boxed([0.0, 0.0, -1.0], [100.0, 10.0, 1.0])).unwrap();
    let turned = bar.rotated_y(0.0, 0.0, std::f64::consts::FRAC_PI_2);
    let b = turned.bounds();
    assert!(b.max[0] < 1.0 + 1e-9 && b.max[2] > 99.0 - 1e-9);
    assert!(b.min[0] > -1.0 - 1e-9);
}
