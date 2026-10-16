//! Net reconstruction of imported EPLAN geometry against KoperCAD's own net
//! resolution, on a synthetic fixture (written for this test, not taken from
//! any EPLAN project).

use std::collections::HashMap;

use novo_core::*;
use novo_interop::eplan::nets::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

/// What a symbol of the fixture is.
enum Kind {
    Device,
    Terminal,
    Interruption,
}

/// The fixture's symbols as the project draws them: kind, name, pin names.
/// A pin's key is `name:pin`.
fn symbols() -> Vec<(Kind, &'static str, Vec<&'static str>)> {
    vec![
        (Kind::Terminal, "-X1:1", vec!["a", "b"]),
        (Kind::Device, "-Q1", vec!["1", "2"]),
        (Kind::Device, "-K1", vec!["13"]),
        (Kind::Device, "-H1", vec!["X1", "X2"]),
        (Kind::Interruption, "=A-W1", vec!["1"]),
        (Kind::Interruption, "=A-W1", vec!["1"]),
        (Kind::Device, "-H2", vec!["X1"]),
    ]
}

/// The fixture as EPLAN stores it: facing pins, a node with three legs, a
/// drawn line, interruption points on two pages, a terminal conductor.
fn graph() -> ConnGraph {
    let mut g = ConnGraph::default();
    let pin = |g: &mut ConnGraph, key: &str, page, x, y, d| {
        g.points.push(ConnPoint::pin(key, page, x, y, d));
    };
    // Page 0: feed terminal, breaker, a node, contact, lamp.
    g.points
        .push(ConnPoint::pin("-X1:1:a", 0, 0.0, 120.0, Dir::Down).with_conductor("-X1:1"));
    g.points
        .push(ConnPoint::pin("-X1:1:b", 0, 0.0, 130.0, Dir::Up).with_conductor("-X1:1"));
    pin(&mut g, "-Q1:1", 0, 0.0, 80.0, Dir::Up);
    pin(&mut g, "-Q1:2", 0, 0.0, 60.0, Dir::Down);
    g.points.push(ConnPoint::node(
        "n1",
        0,
        0.0,
        30.0,
        &[Dir::Up, Dir::Down, Dir::Right],
    ));
    pin(&mut g, "-K1:13", 0, 0.0, 10.0, Dir::Up);
    pin(&mut g, "-H1:X1", 0, 40.0, 30.0, Dir::Left);
    pin(&mut g, "-H1:X2", 0, 40.0, 50.0, Dir::Up);
    pin(&mut g, "=A-W1:p0", 0, 40.0, 70.0, Dir::Down);
    // Page 1: the other interruption point and a lamp.
    pin(&mut g, "=A-W1:p1", 1, 20.0, 100.0, Dir::Down);
    pin(&mut g, "-H2:X1", 1, 20.0, 60.0, Dir::Up);
    // Terminal pin facing the terminal's partner pin is also on the page: the
    // terminal's second side meets the breaker's top pin on the same column.
    g.blockers.push(Blocker {
        page: 0,
        min: Pt::new(-6.0, 61.0),
        max: Pt::new(6.0, 79.0),
    });
    g.lines.push(ConnLine {
        page: 0,
        from: Pt::new(0.0, 30.0),
        to: Pt::new(40.0, 30.0),
    });
    // H1:X2 up to the interruption point, H1's top is a body edge: a short line.
    g.lines.push(ConnLine {
        page: 0,
        from: Pt::new(40.0, 50.0),
        to: Pt::new(40.0, 70.0),
    });
    g.interruptions.push(Interruption {
        point: "=A-W1:p0".into(),
        name: "=A-W1".into(),
        seq: 0,
    });
    g.interruptions.push(Interruption {
        point: "=A-W1:p1".into(),
        name: "=A-W1".into(),
        seq: 1,
    });
    g
}

/// Maps the graph's keys to the project's pins.
fn project_for(links: &[(String, String)]) -> (Project, HashMap<String, Endpoint>) {
    let mut project = Project::default();
    let mut map = HashMap::new();
    let mut interruptions = 0;
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            for (i, (kind, name, pins)) in symbols().into_iter().enumerate() {
                let at = Point::new(10.0 * i as f64, 0.0);
                let pin_defs: Vec<Pin> = pins
                    .iter()
                    .enumerate()
                    .map(|(k, p)| Pin::new(*p, 0.0, 8.0 * k as f64))
                    .collect();
                let role = match kind {
                    Kind::Device => {
                        let d = tx.add_device(tag(name), "")?;
                        SymbolRole::Device {
                            device: d,
                            main: true,
                        }
                    }
                    Kind::Terminal => {
                        let strip = tx.add_terminal_strip(tag("-X1"))?;
                        let t = tx.add_terminal(strip, "1")?;
                        SymbolRole::Terminal { terminals: vec![t] }
                    }
                    Kind::Interruption => SymbolRole::Interruption { name: name.into() },
                };
                let id = tx.place_symbol(page, "test/sym", Placement::at(at), pin_defs, role)?;
                for (k, p) in pins.iter().enumerate() {
                    let key = match kind {
                        Kind::Terminal => format!("{name}:{p}"),
                        Kind::Interruption => {
                            interruptions += 1;
                            format!("{name}:p{}", interruptions - 1)
                        }
                        Kind::Device => format!("{name}:{p}"),
                    };
                    map.insert(key, Endpoint::pin(id, u16::try_from(k).unwrap()));
                }
            }
            for (a, b) in links {
                tx.connect(map[a], map[b])?;
            }
            Ok(())
        })
        .unwrap();
    (project, map)
}

/// The wires a draftsman would draw for the fixture.
fn drawn_links() -> Vec<(String, String)> {
    [
        ("-X1:1:b", "-Q1:1"),
        ("-Q1:2", "-K1:13"),
        ("-Q1:2", "-H1:X1"),
        ("-H1:X2", "=A-W1:p0"),
        ("=A-W1:p1", "-H2:X1"),
    ]
    .iter()
    .map(|(a, b)| (a.to_string(), b.to_string()))
    .collect()
}

fn compare_with(
    project: &Project,
    map: &HashMap<String, Endpoint>,
    g: &ConnGraph,
) -> NetComparison {
    let nets = rebuild_nets(g);
    compare(&nets, &project.netlist(), |k| map.get(&k.0).copied())
}

#[test]
fn the_fixture_rebuilds_the_nets_a_draftsman_would_draw() {
    let nets = rebuild_nets(&graph());
    assert!(nets.issues.is_empty(), "{:?}", nets.issues);
    // Feed net: terminal both sides, breaker top.
    assert!(nets.connected(&"-X1:1:a".into(), &"-Q1:1".into()));
    // The node joins breaker bottom, contact and lamp H1 X1.
    assert!(nets.connected(&"-Q1:2".into(), &"-K1:13".into()));
    assert!(nets.connected(&"-Q1:2".into(), &"-H1:X1".into()));
    // The breaker's two sides are not one net (a breaker is not a wire).
    assert!(!nets.connected(&"-Q1:1".into(), &"-Q1:2".into()));
    // The interruption pair crosses the pages.
    assert!(nets.connected(&"-H1:X2".into(), &"-H2:X1".into()));
    assert_eq!(nets.connected_nets(), 3);
}

#[test]
fn kopercad_nets_agree_with_the_rebuild() {
    let (project, map) = project_for(&drawn_links());
    let cmp = compare_with(&project, &map, &graph());
    assert!(cmp.agrees(), "{cmp:#?}");
    assert_eq!(cmp.compared, 10);
    assert!(cmp.unmapped.is_empty());
}

#[test]
fn a_missing_wire_shows_as_joined_by_the_rebuild_only() {
    // Drop the contact wire only.
    let links: Vec<_> = drawn_links()
        .into_iter()
        .filter(|(_, b)| b != "-K1:13")
        .collect();
    let (project, map) = project_for(&links);
    let cmp = compare_with(&project, &map, &graph());
    assert!(!cmp.agrees());
    assert_eq!(cmp.import_only.len(), 1);
    assert!(cmp.project_only.is_empty());
    let (a, b) = &cmp.import_only[0];
    assert!([&a.0, &b.0].contains(&&"-K1:13".to_owned()));
}

#[test]
fn an_extra_wire_shows_as_joined_by_kopercad_only() {
    let mut links = drawn_links();
    links.push(("-Q1:1".into(), "-H2:X1".into()));
    let (project, map) = project_for(&links);
    let cmp = compare_with(&project, &map, &graph());
    assert!(cmp.import_only.is_empty());
    assert!(!cmp.project_only.is_empty());
}

#[test]
fn spanning_links_make_kopercad_agree_by_construction() {
    let g = graph();
    let nets = rebuild_nets(&g);
    let (project, map) = project_for(
        &nets
            .spanning_links()
            .iter()
            .map(|(a, b)| (a.0.clone(), b.0.clone()))
            .collect::<Vec<_>>(),
    );
    let cmp = compare(&nets, &project.netlist(), |k| map.get(&k.0).copied());
    assert!(cmp.agrees(), "{cmp:#?}");
}

#[test]
fn three_points_of_one_name_differ_between_eplan_and_kopercad() {
    // EPLAN pairs interruption points two by two; KoperCAD joins every point
    // of one name. With three, the third is the case where they disagree.
    let mut g = ConnGraph::default();
    let mut project = Project::default();
    let mut map: HashMap<String, Endpoint> = HashMap::new();
    project
        .transact("build", |tx| {
            let page = tx.add_page("1")?;
            for seq in 0..3u32 {
                let key = format!("=A-W1:p{seq}");
                let id = tx.place_symbol(
                    page,
                    "test/sym",
                    Placement::at(Point::new(0.0, 0.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Interruption {
                        name: "=A-W1".into(),
                    },
                )?;
                map.insert(key.clone(), Endpoint::pin(id, 0));
                g.points
                    .push(ConnPoint::pin(&key, seq, 0.0, 0.0, Dir::Down));
                g.interruptions.push(Interruption {
                    point: key.as_str().into(),
                    name: "=A-W1".into(),
                    seq,
                });
            }
            Ok(())
        })
        .unwrap();
    let nets = rebuild_nets(&g);
    assert!(nets.issues.contains(&NetIssue::AmbiguousInterruption {
        name: "=A-W1".into(),
        count: 3
    }));
    assert!(nets.issues.contains(&NetIssue::UnpairedInterruption {
        name: "=A-W1".into(),
        point: "=A-W1:p2".into()
    }));
    let cmp = compare(&nets, &project.netlist(), |k| map.get(&k.0).copied());
    assert!(cmp.import_only.is_empty());
    assert_eq!(cmp.project_only.len(), 1, "{cmp:#?}");
}
