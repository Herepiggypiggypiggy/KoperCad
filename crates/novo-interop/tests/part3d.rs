//! STEP to part geometry, end to end (WS-24 / 3D view).

use std::cell::Cell;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use novo_interop::part3d::cache::{MeshCache, cache_key, cached_part_geometry};
use novo_interop::part3d::{
    Fallback, Fit, MeshOrigin, MeshRequest, Orientation, PartGeometry, Quality, fit_to_footprint,
    geometry_from_step, part_geometry,
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
    for t in m.indices.chunks_exact(3) {
        let p = |i: usize| m.positions[t[i] as usize].map(f64::from);
        let (a, b, c) = (p(0), p(1), p(2));
        v += (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
            + a[2] * (b[0] * c[1] - b[1] * c[0]))
            / 6.0;
    }
    v
}

fn build(step: &str, size: Option<[f64; 3]>, q: Quality) -> PartGeometry {
    geometry_from_step(step.as_bytes(), size, q, &AtomicBool::new(false), None).unwrap()
}

#[test]
fn a_cylinder_is_a_closed_outward_mesh_of_the_right_size() {
    let g = build(
        &cylinder_step(5.0, 20.0),
        Some([10.0, 10.0, 20.0]),
        Quality::Normal,
    );
    let m = g.detail();
    // Axes: height 20 is the catalog depth, so the model's z stays z.
    let s = g.size_mm();
    assert!((s[2] - 20.0).abs() < 1e-3);
    assert!((s[0] - 10.0).abs() < 0.2 && (s[1] - 10.0).abs() < 0.2);
    let exact = std::f64::consts::PI * 25.0 * 20.0;
    let v = volume(&g);
    assert!(
        v > exact * 0.95 && v <= exact * 1.001,
        "volume {v} vs {exact}"
    );
    assert!(m.positions.iter().all(|p| p[2] > -1e-4 && p[2] < 20.0001));
    match g.origin {
        MeshOrigin::Step {
            stats, orientation, ..
        } => {
            assert_eq!(
                (stats.faces, stats.approximated_faces, stats.skipped_faces),
                (3, 0, 0)
            );
            assert_eq!(orientation, Orientation::MatchedToCatalog);
        }
        o => panic!("{o:?}"),
    }
    // Within the footprint tolerance of the stored size.
    assert_eq!(fit_to_footprint(&g, [10.0, 10.0, 20.0]), Fit::Fits);
    assert!(matches!(
        fit_to_footprint(&g, [10.0, 10.0, 40.0]),
        Fit::Mismatch { .. }
    ));
}

#[test]
fn levels_of_detail_follow_the_design_budgets() {
    // A fine cylinder is far above the 500 triangle budget of a small part.
    let step = cylinder_step(10.0, 15.0);
    let normal = build(&step, None, Quality::Normal);
    let counts: Vec<usize> = normal.lods.iter().map(|l| l.triangle_count()).collect();
    assert!(counts[0] <= 500, "{counts:?}");
    assert!(
        counts.windows(2).all(|w| w[0] > w[1]),
        "levels shrink: {counts:?}"
    );
    assert_eq!(*counts.last().unwrap(), 12, "the last level is the box");
    if counts.len() == 3 {
        assert!(counts[1] <= 300, "{counts:?}");
    }
    let draft = build(&step, None, Quality::Draft);
    assert!(draft.detail().triangle_count() <= 200);
    let exact = std::f64::consts::PI * 100.0 * 15.0;
    let v = volume(&draft);
    assert!(v > exact * 0.8 && v < exact * 1.05, "volume {v} vs {exact}");
    // Axes without a catalog size: smallest extent is depth.
    assert!((draft.size_mm()[2] - 15.0).abs() < 0.1);
    // The box level has the bounds of the model.
    let last = normal.lods.last().unwrap();
    assert_eq!(last.bounds, normal.detail().bounds);
}

#[test]
fn feature_edges_follow_creases_not_the_smooth_wall() {
    let g = build(
        &cylinder_step(5.0, 20.0),
        Some([10.0, 10.0, 20.0]),
        Quality::Normal,
    );
    let m = g.detail();
    // Two rims of a polygonal wall (each segment of the wall is within the
    // 30 degree smoothing limit, so only the cap rims are creases).
    let on_rim = |p: [f32; 3]| p[2] < 1e-3 || p[2] > 19.999;
    assert!(!m.edges.is_empty());
    // After decimation a few wall diagonals can survive as faint creases;
    // the rims and the seam must still dominate.
    let wanted = m
        .edges
        .iter()
        .filter(|e| {
            let (a, b) = (m.positions[e[0] as usize], m.positions[e[1] as usize]);
            (on_rim(a) && on_rim(b)) || (a[0] == b[0] && a[1] == b[1])
        })
        .count();
    assert!(
        wanted * 10 >= m.edges.len() * 7,
        "{wanted} of {} edges are rims or seam",
        m.edges.len()
    );
}

#[test]
fn parts_without_a_usable_model_are_boxes_that_say_why() {
    let cancel = AtomicBool::new(false);
    let req = |step, size| MeshRequest {
        step,
        size_mm: size,
        quality: Quality::Normal,
    };
    let none = part_geometry(&req(None, Some([17.5, 90.0, 60.0])), &cancel);
    assert_eq!(
        (none.origin, none.size_mm()),
        (MeshOrigin::Box, [17.5, 90.0, 60.0])
    );
    assert_eq!(none.lods.len(), 1);
    let junk = part_geometry(&req(Some(b"hello"), Some([17.5, 90.0, 60.0])), &cancel);
    assert_eq!(junk.origin, MeshOrigin::Fallback(Fallback::Unreadable));
    assert_eq!(junk.detail().triangle_count(), 12);
    let empty = part_geometry(&req(Some(b"ISO-10303-21;\nDATA;\nENDSEC;"), None), &cancel);
    assert_eq!(empty.origin, MeshOrigin::Fallback(Fallback::NoGeometry));
}

#[test]
fn the_cache_reads_the_step_file_once() {
    let dir = tempfile::tempdir().unwrap();
    let cache = MeshCache::at(dir.path());
    let step = cylinder_step(4.0, 12.0);
    let reads = Cell::new(0);
    let load = || {
        reads.set(reads.get() + 1);
        Ok(step.clone().into_bytes())
    };
    let cancel = AtomicBool::new(false);
    let get = |id: &str, size, load: &dyn Fn() -> std::io::Result<Vec<u8>>| {
        cached_part_geometry(&cache, id, size, Quality::Normal, load, &cancel, None)
    };
    let a = get("deadbeef", None, &load);
    let b = get("deadbeef", None, &load);
    assert_eq!(reads.get(), 1);
    assert_eq!(a, b);
    assert!(!a.is_box());
    // A model that cannot be read is remembered as failed, not re-parsed.
    let bad = || {
        reads.set(reads.get() + 1);
        Ok(b"nonsense".to_vec())
    };
    let f1 = get("cafe", Some([5.0, 5.0, 5.0]), &bad);
    let f2 = get("cafe", Some([5.0, 5.0, 5.0]), &bad);
    assert_eq!(reads.get(), 2);
    assert_eq!(f1.origin, MeshOrigin::Fallback(Fallback::Unreadable));
    assert_eq!(f1, f2);
    // The cache folder holds one small file per model.
    let key = cache_key("deadbeef", None, Quality::Normal);
    assert!(dir.path().join(format!("{key}.kmsh")).exists());
}

#[test]
fn a_cancelled_or_timed_out_build_is_not_a_model() {
    let dir = tempfile::tempdir().unwrap();
    let cache = MeshCache::at(dir.path());
    let step = cylinder_step(4.0, 12.0);
    let read = || Ok(step.clone().into_bytes());
    let cancelled = AtomicBool::new(true);
    let m = cached_part_geometry(&cache, "abc", None, Quality::Normal, read, &cancelled, None);
    assert_eq!(m.origin, MeshOrigin::Fallback(Fallback::Cancelled));
    // Cancelling is not remembered: the next ask builds it.
    let m = cached_part_geometry(
        &cache,
        "abc",
        None,
        Quality::Normal,
        read,
        &AtomicBool::new(false),
        None,
    );
    assert!(!m.is_box());
    // A deadline in the past is remembered as TimedOut for that model.
    let past = Some(Instant::now());
    let m = cached_part_geometry(
        &cache,
        "slow",
        None,
        Quality::Normal,
        read,
        &AtomicBool::new(false),
        past,
    );
    assert_eq!(m.origin, MeshOrigin::Fallback(Fallback::TimedOut));
}

/// Load time: a part with many placements of one pin (a perforated plate,
/// a connector with 400 contacts) must come out within its budget in well
/// under a second in a release build.
#[test]
fn a_heavy_file_loads_fast_and_ends_within_budget() {
    let mut text = String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n");
    text.push_str(MM);
    let pin = cylinder_step(1.0, 6.0);
    let body: String = pin
        .lines()
        .skip_while(|l| !l.starts_with("#100"))
        .take_while(|l| !l.starts_with("ENDSEC"))
        .filter(|l| !l.starts_with("#192"))
        .collect::<Vec<_>>()
        .join("\n");
    text.push_str(&body);
    text.push_str("\n#300=ADVANCED_BREP_SHAPE_REPRESENTATION('pin',(#191),#9);\n#301=REPRESENTATION_MAP(#105,#300);\n");
    let mut items = Vec::new();
    for i in 0..400 {
        let (x, y) = ((i % 20) as f64 * 5.0, (i / 20) as f64 * 5.0);
        let id = 1000 + i * 3;
        text.push_str(&format!(
            "#{id}=CARTESIAN_POINT('',({x}.,{y}.,0.));\n#{}=AXIS2_PLACEMENT_3D('',#{id},#102,#104);\n#{}=MAPPED_ITEM('',#301,#{});\n",
            id + 1,
            id + 2,
            id + 1
        ));
        items.push(format!("#{}", id + 2));
    }
    text.push_str(&format!(
        "#9000=SHAPE_REPRESENTATION('asm',({}),#9);\nENDSEC;\nEND-ISO-10303-21;\n",
        items.join(",")
    ));
    let t = Instant::now();
    let g = build(&text, None, Quality::Normal);
    let dt = t.elapsed();
    match g.origin {
        MeshOrigin::Step {
            stats,
            before_simplify,
            ..
        } => {
            assert_eq!((stats.solids, stats.placements), (1, 400));
            assert!(before_simplify > 5_000, "{before_simplify}");
        }
        o => panic!("{o:?}"),
    }
    assert!(
        g.detail().triangle_count() <= 2_000,
        "{}",
        g.detail().triangle_count()
    );
    assert!(dt.as_secs_f64() < 20.0, "took {dt:?}");
    eprintln!(
        "400 pins: {:?}, {} triangles",
        dt,
        g.detail().triangle_count()
    );
}
