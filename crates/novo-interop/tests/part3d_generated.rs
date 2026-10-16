//! Stored part rotation and generated DIN rail / duct shapes (3D view).

use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use novo_interop::part3d::cache::{
    MeshCache, cache_key, cache_key_rotated, cached_part_geometry_rotated,
};
use novo_interop::part3d::decimate::{edge_health, weld};
use novo_interop::part3d::generated::{DuctSpec, RailProfile, RailSpec, din_rail, duct};
use novo_interop::part3d::service::{GeometryJob, GeometryService, GeometryState};
use novo_interop::part3d::{
    PartGeometry, PartRotation, Quality, geometry_from_step, geometry_from_step_rotated,
};

const MM: &str = "#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#8))REPRESENTATION_CONTEXT('',''));\n#8=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n";

/// A cylinder of radius `r` and height `h` along z, written the way CAD
/// programs do: circles, one seam line, planar caps and a cylindrical wall.
fn cylinder_step(r: f64, h: f64) -> String {
    let mut s = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
    s.push_str(MM);
    s.push_str(&format!(
        "#100=CARTESIAN_POINT('',(0.,0.,0.));\n#101=CARTESIAN_POINT('',(0.,0.,{h}));\n\
#102=DIRECTION('',(0.,0.,1.));\n#103=DIRECTION('',(0.,0.,-1.));\n#104=DIRECTION('',(1.,0.,0.));\n\
#105=AXIS2_PLACEMENT_3D('',#100,#102,#104);\n#106=AXIS2_PLACEMENT_3D('',#101,#102,#104);\n\
#107=AXIS2_PLACEMENT_3D('',#100,#103,#104);\n\
#110=CARTESIAN_POINT('',({r},0.,0.));\n#111=CARTESIAN_POINT('',({r},0.,{h}));\n\
#112=VERTEX_POINT('',#110);\n#113=VERTEX_POINT('',#111);\n\
#120=CIRCLE('',#105,{r});\n#121=CIRCLE('',#106,{r});\n\
#122=DIRECTION('',(0.,0.,1.));\n#123=VECTOR('',#122,1.);\n#124=LINE('',#110,#123);\n\
#130=EDGE_CURVE('',#112,#112,#120,.T.);\n#131=EDGE_CURVE('',#113,#113,#121,.T.);\n\
#132=EDGE_CURVE('',#112,#113,#124,.T.);\n\
#140=ORIENTED_EDGE('',*,*,#130,.T.);\n#141=ORIENTED_EDGE('',*,*,#131,.T.);\n\
#142=ORIENTED_EDGE('',*,*,#132,.T.);\n#143=ORIENTED_EDGE('',*,*,#132,.F.);\n\
#144=ORIENTED_EDGE('',*,*,#131,.F.);\n#145=ORIENTED_EDGE('',*,*,#130,.F.);\n\
#150=EDGE_LOOP('',(#145));\n#151=EDGE_LOOP('',(#141));\n\
#152=EDGE_LOOP('',(#140,#142,#144,#143));\n\
#160=FACE_OUTER_BOUND('',#150,.T.);\n#161=FACE_OUTER_BOUND('',#151,.T.);\n#162=FACE_OUTER_BOUND('',#152,.T.);\n\
#170=PLANE('',#107);\n#171=PLANE('',#106);\n#172=CYLINDRICAL_SURFACE('',#105,{r});\n\
#180=ADVANCED_FACE('',(#160),#170,.T.);\n#181=ADVANCED_FACE('',(#161),#171,.T.);\n\
#182=ADVANCED_FACE('',(#162),#172,.T.);\n\
#190=CLOSED_SHELL('',(#180,#181,#182));\n#191=MANIFOLD_SOLID_BREP('',#190);\n\
#192=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#191),#9);\nENDSEC;\nEND-ISO-10303-21;\n"
    ));
    s
}

fn volume(g: &PartGeometry) -> f64 {
    let m = g.detail();
    let mut v = 0.0;
    for t in m.indices.as_chunks::<3>().0.iter() {
        let p = |i: usize| m.positions[t[i] as usize].map(f64::from);
        let (a, b, c) = (p(0), p(1), p(2));
        v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    v
}

/// Edges used once or more than twice, and neighbours wound against each
/// other, on the welded detailed level.
fn health(g: &PartGeometry) -> (usize, usize, usize) {
    let m = g.detail();
    let pos: Vec<[f64; 3]> = m.positions.iter().map(|p| p.map(f64::from)).collect();
    let tris: Vec<[u32; 3]> = m.indices.as_chunks::<3>().0.to_vec();
    let (pos, tris) = weld(&pos, &tris, 1e-4);
    let _ = pos;
    let (open, multi) = edge_health(&tris);
    (
        open,
        multi,
        novo_interop::part3d::decimate::winding_conflicts(&tris),
    )
}

#[test]
fn a_rotation_turns_the_part_and_seats_it_again() {
    let step = cylinder_step(5.0, 20.0);
    let cancel = AtomicBool::new(false);
    let plain = geometry_from_step(step.as_bytes(), None, Quality::Normal, &cancel, None).unwrap();
    let s = plain.size_mm();
    let turned = geometry_from_step_rotated(
        step.as_bytes(),
        None,
        Quality::Normal,
        &cancel,
        None,
        PartRotation::new(1, 0, 0),
    )
    .unwrap();
    let t = turned.size_mm();
    // A quarter turn about x swaps height and depth.
    assert!((t[0] - s[0]).abs() < 1e-3 && (t[1] - s[2]).abs() < 1e-3 && (t[2] - s[1]).abs() < 1e-3);
    // Seated: x and y centred, the back at z = 0, and still a closed outward solid.
    let b = turned.detail().bounds;
    assert!((b.min[0] + b.max[0]).abs() < 1e-3 && (b.min[1] + b.max[1]).abs() < 1e-3);
    assert!(b.min[2].abs() < 1e-3);
    assert!((volume(&turned) - volume(&plain)).abs() < 0.01 * volume(&plain));
    // No rotation is exactly the plain build.
    let none = geometry_from_step_rotated(
        step.as_bytes(),
        None,
        Quality::Normal,
        &cancel,
        None,
        PartRotation::NONE,
    )
    .unwrap();
    assert_eq!(none, plain);
}

#[test]
fn a_rotated_part_has_its_own_cache_entry_and_the_old_keys_stay() {
    let dir = tempfile::tempdir().unwrap();
    let cache = MeshCache::at(dir.path());
    let step = cylinder_step(4.0, 12.0);
    let reads = Cell::new(0);
    let load = || {
        reads.set(reads.get() + 1);
        Ok(step.clone().into_bytes())
    };
    let cancel = AtomicBool::new(false);
    let get = |rot| {
        cached_part_geometry_rotated(
            &cache,
            "beef",
            None,
            Quality::Normal,
            rot,
            load,
            &cancel,
            None,
        )
    };
    let a = get(PartRotation::NONE);
    let b = get(PartRotation::new(0, 0, 1));
    let b2 = get(PartRotation::new(0, 0, 1));
    assert_eq!(reads.get(), 2, "one read per rotation");
    assert_eq!(b, b2);
    assert_ne!(a.size_mm(), b.size_mm());
    assert_eq!(
        cache_key_rotated("beef", None, Quality::Normal, PartRotation::NONE),
        cache_key("beef", None, Quality::Normal)
    );
}

#[test]
fn the_service_keeps_a_geometry_per_rotation() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.step");
    std::fs::write(&path, cylinder_step(4.0, 12.0)).unwrap();
    let svc = GeometryService::new(None, Quality::Normal, Duration::from_secs(10), || {});
    let job = GeometryJob {
        part_id: "p".into(),
        content_id: "abc123".into(),
        step_path: path,
        size_mm: None,
    };
    let k0 = svc.request(job.clone());
    let k1 = svc.request_rotated(job.clone(), PartRotation::new(1, 0, 0));
    assert_ne!(k0, k1);
    assert_eq!(k0, svc.request_rotated(job, PartRotation::NONE));
    let ready = |k| {
        for _ in 0..400 {
            if let GeometryState::Ready(g) = svc.geometry(k) {
                return g;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        panic!("not ready");
    };
    let (a, b) = (ready(k0), ready(k1));
    assert_ne!(a.size_mm(), b.size_mm());
}

/// Area of the TS35 cross-section: feet, legs and top web.
fn rail_area(profile: RailProfile) -> f64 {
    let (h, t) = profile.height_and_thickness();
    8.0 * t + 2.0 * t * (h - t) + 27.0 * t
}

#[test]
fn a_plain_din_rail_is_a_closed_top_hat_of_the_right_size_and_volume() {
    for (profile, len) in [
        (RailProfile::Ts35x7_5, 500.0),
        (RailProfile::Ts35x15, 250.0),
    ] {
        let g = din_rail(
            &RailSpec {
                profile,
                length_mm: len,
                slotted: false,
            },
            Quality::Normal,
        );
        let (h, _) = profile.height_and_thickness();
        let s = g.size_mm();
        assert!((f64::from(s[0]) - len).abs() < 1e-3, "{s:?}");
        assert!((f64::from(s[1]) - 35.0).abs() < 1e-3);
        assert!((f64::from(s[2]) - h).abs() < 1e-3);
        assert_eq!(health(&g), (0, 0, 0), "watertight, one winding");
        let want = rail_area(profile) * len;
        assert!(
            (volume(&g) - want).abs() < 1e-3 * want,
            "{} vs {want}",
            volume(&g)
        );
        assert!(g.is_box(), "a generated part reports itself like a box");
        assert_eq!(*g.lods.last().unwrap(), g.detail().bounding_box());
    }
}

#[test]
fn a_perforated_rail_loses_one_slot_volume_per_slot() {
    let spec = |slotted| RailSpec {
        profile: RailProfile::Ts35x7_5,
        length_mm: 500.0,
        slotted,
    };
    let solid = din_rail(&spec(false), Quality::Normal);
    let slotted = din_rail(&spec(true), Quality::Normal);
    assert_eq!(health(&slotted), (0, 0, 0));
    // A slot is 5.2 long and cuts the 25 mm wide, 1 mm thick top web.
    let each = 5.2 * 25.0 * 1.0;
    let n = (volume(&solid) - volume(&slotted)) / each;
    assert!((n - 19.0).abs() < 0.01, "19 slots, got {n}");
    assert!(slotted.detail().triangle_count() <= 2000);
}

#[test]
fn a_rail_too_long_for_the_budget_drops_its_slots_not_its_shape() {
    let g = din_rail(
        &RailSpec {
            profile: RailProfile::Ts35x7_5,
            length_mm: 2000.0,
            slotted: true,
        },
        Quality::Normal,
    );
    assert!(g.detail().triangle_count() <= 2000);
    assert_eq!(health(&g), (0, 0, 0));
    let want = rail_area(RailProfile::Ts35x7_5) * 2000.0;
    assert!((volume(&g) - want).abs() < 1e-3 * want);
}

#[test]
fn a_duct_is_an_open_channel_whose_fingers_are_cut_down_to_the_floor() {
    let spec = |fingers| DuctSpec {
        width_mm: 60.0,
        height_mm: 60.0,
        length_mm: 200.0,
        fingers,
    };
    let solid = duct(&spec(false), Quality::Normal);
    let comb = duct(&spec(true), Quality::Normal);
    let s = comb.size_mm();
    assert!((f64::from(s[0]) - 200.0).abs() < 1e-3);
    assert!((f64::from(s[1]) - 60.0).abs() < 1e-3);
    assert!((f64::from(s[2]) - 60.0).abs() < 1e-3);
    assert_eq!(health(&solid), (0, 0, 0));
    assert_eq!(health(&comb), (0, 0, 0));
    // U section: floor 60 x 2 plus two walls 2 x 58.
    let want = (60.0 * 2.0 + 2.0 * 2.0 * 58.0) * 200.0;
    assert!((volume(&solid) - want).abs() < 1e-3 * want);
    // 19 slots of 5 mm cut both walls (2 x 2 x 58 each).
    let n = (volume(&solid) - volume(&comb)) / (5.0 * 2.0 * 2.0 * 58.0);
    assert!((n - 19.0).abs() < 0.01, "19 slots, got {n}");
}
