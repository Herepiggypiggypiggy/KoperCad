//! Connection points of a part from its STEP model (WS-24 / parts geometry).
//!
//! The models are synthetic boxes with holes and pins written the way CAD
//! programs write them; no maker file is used.

use std::sync::atomic::AtomicBool;

use novo_interop::part3d::pins::{
    CandidateKind, Fix, POSITIONS_SOURCE_KEY, apply_to_part, map_terminals, scan_step,
};
use novo_parts::{CatalogPart, ConnectionKind, DataSource, PartTerminal, Side};

const MM: &str = "#9=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNIT_ASSIGNED_CONTEXT((#8))REPRESENTATION_CONTEXT('',''));\n#8=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n";

#[derive(Clone, Copy)]
enum Hole {
    Round { u: f64, v: f64, r: f64 },
    Rect { u: f64, v: f64, w: f64, h: f64 },
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Face {
    Bottom,
    Top,
    Front,
    Back,
    Right,
    Left,
}

struct Pin {
    base: [f64; 3],
    axis: [f64; 3],
    length: f64,
    r: f64,
}

struct Writer {
    out: String,
    next: u32,
}

impl Writer {
    fn add(&mut self, body: String) -> u32 {
        let id = self.next;
        self.next += 1;
        self.out.push_str(&format!("#{id}={body};\n"));
        id
    }
    fn point(&mut self, p: [f64; 3]) -> u32 {
        self.add(format!(
            "CARTESIAN_POINT('',({:?},{:?},{:?}))",
            p[0], p[1], p[2]
        ))
    }
    fn dir(&mut self, d: [f64; 3]) -> u32 {
        self.add(format!("DIRECTION('',({:?},{:?},{:?}))", d[0], d[1], d[2]))
    }
    fn axis(&mut self, o: [f64; 3], z: [f64; 3], x: [f64; 3]) -> u32 {
        let (o, z, x) = (self.point(o), self.dir(z), self.dir(x));
        self.add(format!("AXIS2_PLACEMENT_3D('',#{o},#{z},#{x})"))
    }
    fn line_edge(&mut self, a: [f64; 3], b: [f64; 3]) -> u32 {
        let (pa, pb) = (self.point(a), self.point(b));
        let (va, vb) = (
            self.add(format!("VERTEX_POINT('',#{pa})")),
            self.add(format!("VERTEX_POINT('',#{pb})")),
        );
        let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        let dd = self.dir([d[0] / l, d[1] / l, d[2] / l]);
        let v = self.add(format!("VECTOR('',#{dd},1.)"));
        let line = self.add(format!("LINE('',#{pa},#{v})"));
        let e = self.add(format!("EDGE_CURVE('',#{va},#{vb},#{line},.T.)"));
        self.add(format!("ORIENTED_EDGE('',*,*,#{e},.T.)"))
    }
    fn circle_edge(&mut self, c: [f64; 3], n: [f64; 3], x: [f64; 3], r: f64) -> u32 {
        let ax = self.axis(c, n, x);
        let p = self.point([c[0] + x[0] * r, c[1] + x[1] * r, c[2] + x[2] * r]);
        let v = self.add(format!("VERTEX_POINT('',#{p})"));
        let circle = self.add(format!("CIRCLE('',#{ax},{r:?})"));
        let e = self.add(format!("EDGE_CURVE('',#{v},#{v},#{circle},.T.)"));
        self.add(format!("ORIENTED_EDGE('',*,*,#{e},.T.)"))
    }
    fn loop_of(&mut self, edges: &[u32]) -> u32 {
        let list: Vec<String> = edges.iter().map(|e| format!("#{e}")).collect();
        self.add(format!("EDGE_LOOP('',({}))", list.join(",")))
    }
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Origin, outward normal, x direction and (width, height) in face
/// coordinates of each side of a box `w` x `h` x `d`.
fn face_frame(f: Face, w: f64, h: f64, d: f64) -> ([f64; 3], [f64; 3], [f64; 3], (f64, f64)) {
    match f {
        Face::Bottom => ([0.0, 0.0, 0.0], [0., -1., 0.], [1., 0., 0.], (w, d)),
        Face::Top => ([0.0, h, d], [0., 1., 0.], [1., 0., 0.], (w, d)),
        Face::Front => ([0.0, 0.0, d], [0., 0., 1.], [1., 0., 0.], (w, h)),
        Face::Back => ([0.0, h, 0.0], [0., 0., -1.], [1., 0., 0.], (w, h)),
        Face::Right => ([w, 0.0, 0.0], [1., 0., 0.], [0., 1., 0.], (h, d)),
        Face::Left => ([0.0, 0.0, d], [-1., 0., 0.], [0., 1., 0.], (h, d)),
    }
}

/// A closed box `w` x `h` x `d` (corner at the origin) with holes in its
/// faces, and cylinders sticking out of it.
fn box_step(w: f64, h: f64, d: f64, holes: &[(Face, Hole)], pins: &[Pin]) -> String {
    let mut s = Writer {
        out: String::from("ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\n"),
        next: 1000,
    };
    s.out.push_str(MM);
    let mut faces = Vec::new();
    for f in [
        Face::Bottom,
        Face::Top,
        Face::Front,
        Face::Back,
        Face::Right,
        Face::Left,
    ] {
        let (o, n, x, (a, b)) = face_frame(f, w, h, d);
        let y = cross(n, x);
        let at = |u: f64, v: f64| {
            [
                o[0] + x[0] * u + y[0] * v,
                o[1] + x[1] * u + y[1] * v,
                o[2] + x[2] * u + y[2] * v,
            ]
        };
        let corners = [at(0.0, 0.0), at(a, 0.0), at(a, b), at(0.0, b)];
        let edges: Vec<u32> = (0..4)
            .map(|i| s.line_edge(corners[i], corners[(i + 1) % 4]))
            .collect();
        let outer_loop = s.loop_of(&edges);
        let mut bounds = vec![s.add(format!("FACE_OUTER_BOUND('',#{outer_loop},.T.)"))];
        for (hf, hole) in holes {
            if *hf != f {
                continue;
            }
            let l = match *hole {
                Hole::Round { u, v, r } => {
                    let e = s.circle_edge(at(u, v), n, x, r);
                    s.loop_of(&[e])
                }
                Hole::Rect { u, v, w, h } => {
                    let c = [at(u, v), at(u + w, v), at(u + w, v + h), at(u, v + h)];
                    let e: Vec<u32> = (0..4).map(|i| s.line_edge(c[i], c[(i + 1) % 4])).collect();
                    s.loop_of(&e)
                }
            };
            bounds.push(s.add(format!("FACE_BOUND('',#{l},.T.)")));
        }
        let ax = s.axis(o, n, x);
        let plane = s.add(format!("PLANE('',#{ax})"));
        let list: Vec<String> = bounds.iter().map(|b| format!("#{b}")).collect();
        faces.push(s.add(format!(
            "ADVANCED_FACE('',({}),#{plane},.T.)",
            list.join(",")
        )));
    }
    let list: Vec<String> = faces.iter().map(|f| format!("#{f}")).collect();
    let shell = s.add(format!("CLOSED_SHELL('',({}))", list.join(",")));
    let mut solids = vec![s.add(format!("MANIFOLD_SOLID_BREP('',#{shell})"))];
    for p in pins {
        solids.push(pin_solid(&mut s, p));
    }
    let list: Vec<String> = solids.iter().map(|f| format!("#{f}")).collect();
    s.add(format!(
        "ADVANCED_BREP_SHAPE_REPRESENTATION('',({}),#9)",
        list.join(",")
    ));
    s.out.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    s.out
}

/// A cylinder as a closed solid, outward wall.
fn pin_solid(s: &mut Writer, p: &Pin) -> u32 {
    let a = p.axis;
    let x = if a[0].abs() < 0.9 {
        cross(a, [1.0, 0.0, 0.0])
    } else {
        cross(a, [0.0, 1.0, 0.0])
    };
    let l = (x[0] * x[0] + x[1] * x[1] + x[2] * x[2]).sqrt();
    let x = [x[0] / l, x[1] / l, x[2] / l];
    let top = [
        p.base[0] + a[0] * p.length,
        p.base[1] + a[1] * p.length,
        p.base[2] + a[2] * p.length,
    ];
    let e0 = s.circle_edge(p.base, a, x, p.r);
    let e1 = s.circle_edge(top, a, x, p.r);
    let seam0 = [
        p.base[0] + x[0] * p.r,
        p.base[1] + x[1] * p.r,
        p.base[2] + x[2] * p.r,
    ];
    let seam1 = [
        top[0] + x[0] * p.r,
        top[1] + x[1] * p.r,
        top[2] + x[2] * p.r,
    ];
    let seam = s.line_edge(seam0, seam1);
    let l0 = s.loop_of(&[e0]);
    let l1 = s.loop_of(&[e1]);
    let lw = s.loop_of(&[e0, seam, e1, seam]);
    let neg = [-a[0], -a[1], -a[2]];
    let ax0 = s.axis(p.base, neg, x);
    let ax1 = s.axis(top, a, x);
    let axw = s.axis(p.base, a, x);
    let (pl0, pl1) = (
        s.add(format!("PLANE('',#{ax0})")),
        s.add(format!("PLANE('',#{ax1})")),
    );
    let cyl = s.add(format!("CYLINDRICAL_SURFACE('',#{axw},{:?})", p.r));
    let mut faces = Vec::new();
    for (l, surf) in [(l0, pl0), (l1, pl1), (lw, cyl)] {
        let b = s.add(format!("FACE_OUTER_BOUND('',#{l},.T.)"));
        faces.push(s.add(format!("ADVANCED_FACE('',(#{b}),#{surf},.T.)")));
    }
    let list: Vec<String> = faces.iter().map(|f| format!("#{f}")).collect();
    let shell = s.add(format!("CLOSED_SHELL('',({}))", list.join(",")));
    s.add(format!("MANIFOLD_SOLID_BREP('',#{shell})"))
}

fn scan(step: &str, size: [f64; 3]) -> novo_interop::part3d::pins::PinScan {
    scan_step(step.as_bytes(), Some(size), &AtomicBool::new(false), None).unwrap()
}

fn terminal(name: &str, side: Side, kind: ConnectionKind) -> PartTerminal {
    PartTerminal {
        name: name.into(),
        side,
        kind,
        ..Default::default()
    }
}

fn round(u: f64, v: f64) -> Hole {
    Hole::Round { u, v, r: 1.6 }
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|k| (a[k] - b[k]).abs() < 0.05)
}

#[test]
fn round_openings_on_two_sides_become_terminal_positions_in_reading_order() {
    // 40 wide, 60 high, 40 deep. Four wire entries on top, four on the bottom.
    let mut holes = Vec::new();
    for u in [5.0, 15.0, 25.0, 35.0] {
        holes.push((Face::Top, round(u, 20.0)));
        holes.push((Face::Bottom, round(u, 20.0)));
    }
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    assert_eq!(found.openings().count(), 8);
    assert_eq!(found.pins().count(), 0);
    assert!(
        found
            .candidates
            .iter()
            .all(|c| c.round && (c.size_mm - 3.2).abs() < 0.1)
    );
    let terminals: Vec<PartTerminal> = ["1", "3", "5", "7"]
        .iter()
        .map(|n| terminal(n, Side::Top, ConnectionKind::Screw))
        .chain(
            ["2", "4", "6", "8"]
                .iter()
                .map(|n| terminal(n, Side::Bottom, ConnectionKind::Screw)),
        )
        .collect();
    let map = map_terminals(&terminals, &found);
    assert_eq!(map.count(Fix::Measured), 8);
    assert_eq!(map.weakest(), Some(Fix::Measured));
    assert_eq!(map.unused_candidates, 0);
    let at = |n: &str| map.fixes.iter().find(|f| f.name == n).unwrap().position_mm;
    // Top face is y = 60; the model's z = 20 is the part's depth position.
    assert!(near(at("1"), [5.0, 60.0, 20.0]), "{:?}", at("1"));
    assert!(near(at("7"), [35.0, 60.0, 20.0]));
    assert!(near(at("2"), [5.0, 0.0, 20.0]));
    assert!(near(at("8"), [35.0, 0.0, 20.0]));
}

#[test]
fn a_model_with_other_axes_lands_in_the_frame_of_the_mesh() {
    // The maker's model has width along y, height along z and depth along x
    // (25 x 40 x 60 as written). The catalog size, 40 wide, 60 high and
    // 25 deep, makes the part axes (y, z, x).
    let holes = vec![
        (Face::Front, round(6.0, 10.0)),
        (Face::Front, round(19.0, 30.0)),
    ];
    let step = box_step(25.0, 40.0, 60.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 25.0]);
    assert_eq!(found.size_mm, [40.0, 60.0, 25.0]);
    let mut tops: Vec<[f64; 3]> = found
        .candidates
        .iter()
        .inspect(|c| assert_eq!(c.side, Side::Top))
        .map(|c| c.position_mm)
        .collect();
    tops.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(tops.len(), 2);
    assert!(near(tops[0], [10.0, 60.0, 6.0]), "{:?}", tops[0]);
    assert!(near(tops[1], [30.0, 60.0, 19.0]), "{:?}", tops[1]);
}

#[test]
fn rectangular_entries_in_rows_on_the_front_are_read_row_by_row() {
    // Spring-cage style slots on the front face: two rows of three.
    let mut holes = Vec::new();
    for (i, u) in [6.0, 20.0, 34.0].iter().enumerate() {
        let _ = i;
        holes.push((
            Face::Front,
            Hole::Rect {
                u: *u - 2.0,
                v: 40.0,
                w: 4.0,
                h: 3.0,
            },
        ));
        holes.push((
            Face::Front,
            Hole::Rect {
                u: *u - 2.0,
                v: 15.0,
                w: 4.0,
                h: 3.0,
            },
        ));
    }
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    assert_eq!(found.openings().count(), 6);
    assert!(
        found
            .candidates
            .iter()
            .all(|c| !c.round && c.side == Side::Front)
    );
    let terminals: Vec<PartTerminal> = ["A1", "A2", "A3", "B1", "B2", "B3"]
        .iter()
        .map(|n| terminal(n, Side::Front, ConnectionKind::PushIn))
        .collect();
    let map = map_terminals(&terminals, &found);
    assert_eq!(map.count(Fix::Measured), 6);
    let at = |n: &str| map.fixes.iter().find(|f| f.name == n).unwrap().position_mm;
    assert!(near(at("A1"), [6.0, 41.5, 40.0]), "{:?}", at("A1"));
    assert!(near(at("A3"), [34.0, 41.5, 40.0]));
    assert!(near(at("B1"), [6.0, 16.5, 40.0]));
}

#[test]
fn vents_and_tiny_holes_are_not_connections() {
    let holes = vec![
        (Face::Top, round(10.0, 20.0)),
        // A 12 x 1.5 vent slot and a 0.5 mm hole.
        (
            Face::Top,
            Hole::Rect {
                u: 15.0,
                v: 5.0,
                w: 12.0,
                h: 1.5,
            },
        ),
        (
            Face::Top,
            Hole::Round {
                u: 30.0,
                v: 20.0,
                r: 0.25,
            },
        ),
    ];
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    assert_eq!(found.openings().count(), 1);
}

#[test]
fn extra_openings_take_the_evenly_spaced_run() {
    // Five holes in a row, the last one off the pitch: four terminals.
    let holes: Vec<(Face, Hole)> = [4.0, 14.0, 24.0, 34.0, 39.0]
        .iter()
        .map(|&u| (Face::Top, round(u, 20.0)))
        .collect();
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    let terminals: Vec<PartTerminal> = ["1", "2", "3", "4"]
        .iter()
        .map(|n| terminal(n, Side::Top, ConnectionKind::Screw))
        .collect();
    let map = map_terminals(&terminals, &found);
    assert_eq!(map.count(Fix::Partial), 4);
    assert_eq!(map.weakest(), Some(Fix::Partial));
    assert_eq!(map.unused_candidates, 1);
    let x: Vec<f64> = map.fixes.iter().map(|f| f.position_mm[0]).collect();
    assert!(
        x.iter()
            .zip([4.0, 14.0, 24.0, 34.0])
            .all(|(a, b)| (a - b).abs() < 0.05),
        "{x:?}"
    );
}

#[test]
fn missing_openings_give_an_estimate_that_is_marked_and_optional() {
    // The model has two holes where the catalog lists three terminals.
    let holes = vec![
        (Face::Top, round(10.0, 20.0)),
        (Face::Top, round(30.0, 20.0)),
    ];
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    let mut part = CatalogPart::new("Test", "T-3");
    part.terminals = ["1", "2", "3"]
        .iter()
        .map(|n| terminal(n, Side::Top, ConnectionKind::Screw))
        .collect();
    let map = map_terminals(&part.terminals, &found);
    assert_eq!(map.count(Fix::Estimated), 3);
    assert_eq!(map.weakest(), Some(Fix::Estimated));
    // Evenly spaced: 1/6, 3/6, 5/6 of the width, on the top face.
    assert!(near(map.fixes[0].position_mm, [40.0 / 6.0, 60.0, 20.0]));
    assert!(near(
        map.fixes[2].position_mm,
        [40.0 * 5.0 / 6.0, 60.0, 20.0]
    ));
    // Without estimates nothing is written; the terminals stay unknown.
    assert_eq!(apply_to_part(&mut part, &map, false), 0);
    assert!(part.terminals.iter().all(|t| t.position_mm.is_none()));
    assert!(!part.sources.contains_key(POSITIONS_SOURCE_KEY));
    // With estimates they are written and the source says so.
    assert_eq!(apply_to_part(&mut part, &map, true), 3);
    assert_eq!(
        part.sources.get(POSITIONS_SOURCE_KEY),
        Some(&DataSource::Estimated)
    );
}

#[test]
fn measured_positions_are_written_and_sourced_from_the_makers_model() {
    let holes = vec![
        (Face::Top, round(10.0, 20.0)),
        (Face::Top, round(30.0, 20.0)),
    ];
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    let mut part = CatalogPart::new("Test", "T-2");
    part.terminals = ["L", "N"]
        .iter()
        .map(|n| terminal(n, Side::Top, ConnectionKind::Screw))
        .collect();
    let map = map_terminals(&part.terminals, &found);
    assert_eq!(apply_to_part(&mut part, &map, false), 2);
    assert!(near(
        part.terminals[0].position_mm.unwrap(),
        [10.0, 60.0, 20.0]
    ));
    assert_eq!(
        part.sources.get(POSITIONS_SOURCE_KEY),
        Some(&DataSource::MakerDatasheet)
    );
}

#[test]
fn pins_that_stick_out_are_found_at_their_tips() {
    // Two 1 mm pins out of the top, 8 mm long; the body is 40 x 50 x 40 so
    // the outline is 58 high with the pins.
    let pins = vec![
        Pin {
            base: [10.0, 50.0, 20.0],
            axis: [0.0, 1.0, 0.0],
            length: 8.0,
            r: 0.5,
        },
        Pin {
            base: [30.0, 50.0, 20.0],
            axis: [0.0, 1.0, 0.0],
            length: 8.0,
            r: 0.5,
        },
    ];
    let step = box_step(40.0, 50.0, 40.0, &[], &pins);
    let found = scan(&step, [40.0, 58.0, 40.0]);
    assert_eq!(found.pins().count(), 2, "{:?}", found.candidates);
    assert!(
        found
            .candidates
            .iter()
            .all(|c| c.kind == CandidateKind::Pin)
    );
    let terminals = vec![
        terminal("1", Side::Top, ConnectionKind::Pin),
        terminal("2", Side::Top, ConnectionKind::Pin),
    ];
    let map = map_terminals(&terminals, &found);
    assert_eq!(map.count(Fix::Measured), 2);
    assert!(
        near(map.fixes[0].position_mm, [10.0, 58.0, 20.0]),
        "{:?}",
        map.fixes[0]
    );
    assert!(near(map.fixes[1].position_mm, [30.0, 58.0, 20.0]));
}

#[test]
fn a_terminal_without_a_side_is_left_unplaced() {
    let step = box_step(40.0, 60.0, 40.0, &[(Face::Top, round(10.0, 20.0))], &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    let terminals = vec![terminal("1", Side::Unknown, ConnectionKind::Screw)];
    let map = map_terminals(&terminals, &found);
    assert!(map.fixes.is_empty());
    assert_eq!(map.unplaced, vec!["1".to_string()]);
    assert_eq!(map.unused_candidates, 1);
}

#[test]
fn a_model_turned_to_put_its_mounting_side_at_the_back_keeps_its_points() {
    // The holed face (model z = 0) is the smaller flat one, so the mesh is
    // turned about y: that face ends up at the front and x is mirrored.
    let holes = vec![
        (Face::Back, round(10.0, 20.0)),
        (Face::Back, round(30.0, 20.0)),
    ];
    let step = box_step(40.0, 60.0, 40.0, &holes, &[]);
    let found = scan(&step, [40.0, 60.0, 40.0]);
    let mut pts: Vec<[f64; 3]> = found
        .candidates
        .iter()
        .inspect(|c| assert_eq!(c.side, Side::Front))
        .map(|c| c.position_mm)
        .collect();
    pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
    assert_eq!(pts.len(), 2);
    assert!(near(pts[0], [10.0, 40.0, 40.0]), "{:?}", pts[0]);
    assert!(near(pts[1], [30.0, 40.0, 40.0]), "{:?}", pts[1]);
}

#[test]
fn bad_input_is_an_error_not_a_panic() {
    let r = scan_step(b"not a step file", None, &AtomicBool::new(false), None);
    assert!(r.is_err());
}
