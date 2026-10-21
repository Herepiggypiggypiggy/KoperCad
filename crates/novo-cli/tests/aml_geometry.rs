//! AutomationML import, looked at as geometry: the pins of an imported page
//! sit where the file puts them, every wire starts and ends exactly on a pin
//! or junction, and no wire runs across a symbol it does not connect to.
//!
//! The synthetic file is written by hand (invented tags). A real export is
//! checked the same way when `KOPERCAD_CRANE_AML` names one (opt in, never
//! in the repository).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use novo_canvas::model::{PlaceholderArt, SymbolArt};
use novo_core::{Document, Endpoint, PageId, Point, Project, SymbolInstance};
use novo_editor::art::LibraryArt;
use novo_interop::aml::{plan_import, read_aml};

type Pins<'a> = &'a [(&'a str, &'a str, f64, f64, &'a str)];

fn sym(id: &str, page: &str, kind: &str, x: f64, y: f64, extra: &str, pins: Pins) -> String {
    let mut s = format!(
        r#"<InternalElement ID="s-{id}" Name="{id}">
  <Attribute Name="Page" AttributeDataType="xs:string"><Value>{page}</Value></Attribute>
  <Attribute Name="Kind" AttributeDataType="xs:string"><Value>{kind}</Value></Attribute>
  {extra}
  <Attribute Name="Frame"><Attribute Name="x" AttributeDataType="xs:double"><Value>{x}</Value></Attribute><Attribute Name="y" AttributeDataType="xs:double"><Value>{y}</Value></Attribute></Attribute>
"#
    );
    for (pid, name, dx, dy, faces) in pins {
        let _ = write!(
            s,
            r#"  <ExternalInterface ID="{pid}" Name="{name}">
    <Attribute Name="X" AttributeDataType="xs:double"><Value>{dx}</Value></Attribute>
    <Attribute Name="Y" AttributeDataType="xs:double"><Value>{dy}</Value></Attribute>
    <Attribute Name="Faces" AttributeDataType="xs:string"><Value>{faces}</Value></Attribute>
  </ExternalInterface>
"#
        );
    }
    s + "  <RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Symbol\"/>\n</InternalElement>\n"
}

/// A net with the runs the file draws for it (`x1,y1 x2,y2` pairs) and the
/// pins it joins.
fn net(k: usize, runs: &[&str], pins: &[&str]) -> (String, String) {
    let mut e = format!("<InternalElement ID=\"n{k}\" Name=\"N{k}\">");
    let mut links = String::new();
    for (j, pin) in pins.iter().enumerate() {
        let _ = write!(e, "<ExternalInterface ID=\"np{k}-{j}\" Name=\"{pin}\"/>");
        let _ = write!(
            links,
            "<InternalLink Name=\"pin\" RefPartnerSideA=\"{pin}\" RefPartnerSideB=\"np{k}-{j}\"/>"
        );
    }
    for (j, run) in runs.iter().enumerate() {
        let n: Vec<f64> = run.split([',', ' ']).map(|v| v.parse().unwrap()).collect();
        let _ = write!(
            e,
            "<InternalElement ID=\"w{k}-{j}\" Name=\"W{j}\"><Attribute Name=\"Page\" AttributeDataType=\"xs:string\"><Value>{PAGE}</Value></Attribute>"
        );
        for (name, v) in ["X1", "Y1", "X2", "Y2"].iter().zip(&n) {
            let _ = write!(
                e,
                "<Attribute Name=\"{name}\" AttributeDataType=\"xs:double\"><Value>{v}</Value></Attribute>"
            );
        }
        e += "<RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Wire\"/></InternalElement>";
    }
    e += "<RoleRequirements RefBaseRoleClassPath=\"KoperCADRoleClassLib/Net\"/></InternalElement>";
    (e, links)
}

const PAGE: &str = "=A1+CAB1/01";

/// A page with a terminal strip feeding a relay coil and a lamp, drawn on
/// EPLAN's 4 and 8 mm pin pitch (the library symbols are 8 and 16 mm), a
/// junction and a bent branch, and a lamp that no wire may touch.
fn file() -> String {
    let down = "down";
    let up = "up";
    let x1 = sym(
        "x1s",
        PAGE,
        "terminal",
        50.0,
        240.0,
        "<Attribute Name=\"Terminal\" AttributeDataType=\"xs:string\"><Value>1</Value></Attribute>",
        &[("x1a", "1", 0.0, 0.0, up), ("x1b", "2", 0.0, -4.0, down)],
    );
    let k1 = sym(
        "k1c",
        PAGE,
        "coil",
        50.0,
        200.0,
        "",
        &[
            ("k1a1", "A1", 0.0, 0.0, up),
            ("k1a2", "A2", 0.0, -8.0, down),
        ],
    );
    let k1k = sym(
        "k1k",
        PAGE,
        "contact",
        90.0,
        200.0,
        "",
        &[
            ("k1c13", "13", 0.0, 0.0, up),
            ("k1c14", "14", 0.0, -8.0, down),
        ],
    );
    let h1 = sym(
        "h1l",
        PAGE,
        "lamp",
        130.0,
        200.0,
        "",
        &[
            ("h1x1", "X1", 0.0, 0.0, up),
            ("h1x2", "X2", 0.0, -8.0, down),
        ],
    );
    // A box that stands beside the wires.
    let u1 = sym(
        "u1b",
        PAGE,
        "generic",
        70.0,
        190.0,
        "",
        &[("u1p", "1", 0.0, 0.0, up), ("u1q", "2", 8.0, 0.0, up)],
    );
    let wires = [
        // terminal 2 down to a junction, on to the coil, and a branch with a
        // bend to the contact
        net(
            1,
            &[
                "50,236 50,220",
                "50,220 50,200",
                "50,220 90,220",
                "90,220 90,200",
            ],
            &["x1b", "k1a1", "k1c13"],
        ),
        // contact and coil lower ends to the lamp
        net(
            2,
            &["90,192 90,180", "90,180 130,180", "130,180 130,200"],
            &["k1c14", "h1x1"],
        ),
        net(
            3,
            &["50,192 50,170", "50,170 130,170", "130,170 130,192"],
            &["k1a2", "h1x2"],
        ),
    ];
    let (w, l): (String, String) = wires
        .iter()
        .fold(Default::default(), |(w, l), (e, k)| (w + e, l + k));
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<CAEXFile FileName="t.aml" SchemaVersion="2.15"><InstanceHierarchy Name="Test">
<InternalElement ID="proj" Name="Test"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Project"/>
<InternalElement ID="pg" Name="Pages"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/PageGroup"/>
<InternalElement ID="pg1" Name="=A1+CAB1/01"><Attribute Name="Title" AttributeDataType="xs:string"><Value>Control</Value></Attribute><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Page"/></InternalElement>
</InternalElement>
<InternalElement ID="k1" Name="=A1+CAB1-K1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-K1</Value></Attribute>{k1}{k1k}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Device"/></InternalElement>
<InternalElement ID="x1" Name="=A1+CAB1-X1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-X1</Value></Attribute><ExternalInterface ID="t1" Name="1" RefBaseClassPath="KoperCADInterfaceClassLib/Terminal"/>{x1}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/TerminalStrip"/></InternalElement>
<InternalElement ID="h1" Name="=A1+CAB1-H1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-H1</Value></Attribute>{h1}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Device"/></InternalElement>
<InternalElement ID="u1" Name="=A1+CAB1-U1"><Attribute Name="Tag" AttributeDataType="xs:string"><Value>=A1+CAB1-U1</Value></Attribute>{u1}<RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/Device"/></InternalElement>
<InternalElement ID="ng" Name="Nets"><RoleRequirements RefBaseRoleClassPath="KoperCADRoleClassLib/NetGroup"/>{w}</InternalElement>
{l}</InternalElement></InstanceHierarchy></CAEXFile>"#
    )
}

fn import(text: &str) -> Project {
    let plant = read_aml(text).unwrap();
    let mut doc = Document::new(Project::new("t"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    doc.project().clone()
}

/// What a symbol draws, as segments in page coordinates.
fn strokes(art: &LibraryArt, s: &SymbolInstance) -> Vec<(Point, Point)> {
    let primitives = art.artwork(s).unwrap_or_else(|| {
        PlaceholderArt::draw(
            s,
            novo_canvas::Stroke::world(0.35, novo_canvas::Color::BLACK),
        )
    });
    let mut out = Vec::new();
    for p in &primitives {
        for run in p.shape.outline(0.2) {
            for w in run.windows(2) {
                let at = |v: novo_canvas::Vec2| {
                    s.placement
                        .apply(Point::new(f64::from(v.x), f64::from(v.y)))
                };
                out.push((at(w[0]), at(w[1])));
            }
        }
    }
    out
}

/// Do two segments cross or run along each other (touching ends do not count)?
fn collide(a: (Point, Point), b: (Point, Point)) -> bool {
    let cross =
        |o: Point, p: Point, q: Point| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let (d1, d2) = (cross(b.0, b.1, a.0), cross(b.0, b.1, a.1));
    let (d3, d4) = (cross(a.0, a.1, b.0), cross(a.0, a.1, b.1));
    let eps = 0.05;
    if d1.abs() < eps && d2.abs() < eps {
        // Parallel on one line: overlap longer than a hair.
        let proj = |p: Point| {
            if (a.1.x - a.0.x).abs() > (a.1.y - a.0.y).abs() {
                p.x
            } else {
                p.y
            }
        };
        let (a0, a1) = (proj(a.0).min(proj(a.1)), proj(a.0).max(proj(a.1)));
        let (b0, b1) = (proj(b.0).min(proj(b.1)), proj(b.0).max(proj(b.1)));
        return a1.min(b1) - a0.max(b0) > 0.3;
    }
    (d1 * d2 < -eps) && (d3 * d4 < -eps)
}

/// Problems found on an imported project: a wire end that is not where its
/// pin or junction is, and a wire segment that crosses a foreign symbol.
fn problems(project: &Project, art: &LibraryArt) -> Vec<String> {
    let mut bad = Vec::new();
    let mut by_page: BTreeMap<PageId, Vec<(&SymbolInstance, Strokes)>> = BTreeMap::new();
    for s in project.symbols() {
        by_page
            .entry(s.page)
            .or_default()
            .push((s, strokes(art, s)));
    }
    for w in project.wires() {
        let (Some(a), Some(b)) = (
            project.endpoint_position(w.from),
            project.endpoint_position(w.to),
        ) else {
            continue;
        };
        let mut line = vec![a];
        line.extend(w.route.iter().copied());
        line.push(b);
        let own: Vec<_> = [w.from, w.to]
            .iter()
            .filter_map(|e| match e {
                Endpoint::Pin { symbol, .. } => Some(*symbol),
                _ => None,
            })
            .collect();
        for seg in line.windows(2) {
            for (s, art) in by_page.get(&w.page).into_iter().flatten() {
                if own.contains(&s.id) {
                    continue;
                }
                if art.iter().any(|&b| collide((seg[0], seg[1]), b)) {
                    let pins: Vec<_> = s
                        .pins
                        .iter()
                        .filter_map(|x| {
                            s.pin_position(
                                s.pins.iter().position(|y| y.name == x.name).unwrap() as u16
                            )
                        })
                        .collect();
                    bad.push(format!(
                        "wire {:?} (ends {a:?} {b:?}) runs across {:?} pins {pins:?} at ({:.1}, {:.1})-({:.1}, {:.1})",
                        w.id, s.symbol.0, seg[0].x, seg[0].y, seg[1].x, seg[1].y
                    ));
                }
            }
        }
    }
    bad
}

type Strokes = Vec<(Point, Point)>;

fn art() -> LibraryArt {
    LibraryArt::new(novo_py::library().unwrap(), novo_pdf::print_style().symbol)
}

fn pin_at(p: &Project, tag_pin: &str) -> Point {
    p.symbols()
        .find_map(|s| {
            s.pins
                .iter()
                .position(|x| x.name == tag_pin)
                .and_then(|i| s.pin_position(i as u16))
        })
        .unwrap_or_else(|| panic!("no pin {tag_pin}"))
}

#[test]
fn pins_land_where_the_file_puts_them() {
    let p = import(&file());
    let h = p.pages().next().unwrap().format.size_mm().1;
    // The file is y up; the page is y down. EPLAN's 8 mm coil, 4 mm terminal.
    for (pin, x, y) in [
        ("A1", 50.0, 200.0),
        ("A2", 50.0, 192.0),
        ("13", 90.0, 200.0),
        ("14", 90.0, 192.0),
        ("X1", 130.0, 200.0),
        ("X2", 130.0, 192.0),
    ] {
        let at = pin_at(&p, pin);
        assert!(
            (at.x - x).abs() < 1e-6 && (at.y - (h - y)).abs() < 1e-6,
            "pin {pin} is at {at:?}, the file has ({x}, {y})"
        );
    }
}

#[test]
fn every_wire_is_drawn_between_its_pins_on_its_route() {
    let p = import(&file());
    let h = p.pages().next().unwrap().format.size_mm().1;
    assert_eq!(p.wires().count(), 5, "the five wires of the file");
    assert_eq!(p.junctions().count(), 1);
    // The branch to the contact keeps its bend.
    let branch = p
        .wires()
        .find(|w| w.route.len() == 1)
        .expect("the wire with a corner");
    assert_eq!(branch.route[0], Point::new(90.0, h - 220.0));
    for w in p.wires() {
        for e in [w.from, w.to] {
            assert!(p.endpoint_position(e).is_some(), "{w:?} ends nowhere");
        }
    }
    let a = pin_at(&p, "A1");
    let junction = p.junctions().next().unwrap().position;
    assert_eq!(junction, Point::new(50.0, h - 220.0));
    assert!(p.wires().any(|w| {
        p.endpoint_position(w.from) == Some(junction) && p.endpoint_position(w.to) == Some(a)
    }));
}

#[test]
fn no_wire_runs_across_a_symbol_it_does_not_connect_to() {
    let p = import(&file());
    let bad = problems(&p, &art());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}

#[test]
fn the_nets_of_the_page_follow_the_wires() {
    let p = import(&file());
    let net_of = |name: &str| {
        let (sid, i) = p
            .symbols()
            .find_map(|s| {
                s.pins
                    .iter()
                    .position(|x| x.name == name)
                    .map(|i| (s.id, i as u16))
            })
            .unwrap();
        p.netlist()
            .nets()
            .iter()
            .find(|n| n.endpoints.contains(&Endpoint::pin(sid, i)))
            .map(|n| n.id)
    };
    assert_eq!(
        net_of("2"),
        net_of("A1"),
        "terminal 2, junction and coil are one net"
    );
    assert_eq!(
        net_of("A1"),
        net_of("13"),
        "and the contact, through the branch"
    );
    assert_eq!(net_of("14"), net_of("X1"));
    assert_eq!(net_of("A2"), net_of("X2"));
    assert_ne!(net_of("A1"), net_of("A2"));
}

/// Opt-in: a real crane export. Every wire of the file must end on a pin or
/// junction of the page, and none may run across a foreign symbol.
#[test]
fn a_real_crane_export_has_wires_on_pins_and_clear_of_symbols() {
    let Some(path) = std::env::var_os("KOPERCAD_CRANE_AML") else {
        return;
    };
    let text = std::fs::read_to_string(path).unwrap();
    let plant = read_aml(&text).unwrap();
    let p = import(&text);
    assert!(p.wires().count() > 0);
    // No wire may end in the air: every end is a pin or a junction.
    let open_ends: Vec<_> = p
        .wires()
        .flat_map(|w| [(w.page, w.from), (w.page, w.to)])
        .filter(|(_, e)| matches!(e, Endpoint::Free { .. }))
        .collect();
    for (page, e) in &open_ends {
        eprintln!(
            "open end {e:?} on {:?}",
            p.page(*page).map(|x| x.name.clone())
        );
    }
    let open = open_ends.len();
    // The file itself leaves a run in the air where EPLAN has a node with a
    // leg that goes nowhere; count those from the file, not from the import.
    let key = |page: &str, p: (f64, f64)| {
        (
            page.to_string(),
            (p.0 * 100.0).round() as i64,
            (p.1 * 100.0).round() as i64,
        )
    };
    let mut ends: BTreeMap<_, usize> = BTreeMap::new();
    for s in &plant.segments {
        *ends.entry(key(&s.page, s.from)).or_default() += 1;
        *ends.entry(key(&s.page, s.to)).or_default() += 1;
    }
    for sym in &plant.symbols {
        for pin in &sym.pins {
            let at = (sym.at.0 + pin.offset.0, sym.at.1 + pin.offset.1);
            if let Some(n) = ends.get_mut(&key(&sym.page, at)) {
                *n += 2;
            }
        }
    }
    let in_the_air = ends.values().filter(|n| **n == 1).count();
    eprintln!(
        "{} runs in the file, {in_the_air} end in the air, {open} open wire ends",
        plant.segments.len()
    );
    assert_eq!(
        open, in_the_air,
        "a wire ends in the air that the file closes"
    );
    let bad = problems(&p, &art());
    for b in bad.iter().take(25) {
        eprintln!("{b}");
    }
    eprintln!("{} wires, {} crossings", p.wires().count(), bad.len());
    assert!(bad.is_empty());
}

/// An invented hoist feeder page, written in the configurator's export
/// format (supply terminals, a breaker and contactor on EPLAN's 8 mm pitch,
/// a coil, a contact, a lamp, a junction and bends). Its pictures are in
/// the pull request that introduced this test.
#[test]
fn an_invented_hoist_page_imports_clean() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../novo-interop/tests/fixtures/aml/hoist-feeder.aml"
    );
    let text = std::fs::read_to_string(path).unwrap();
    let plant = read_aml(&text).unwrap();
    assert!(plant.segments.len() > 20, "the file draws its wires");
    let p = import(&text);
    let h = p.pages().next().unwrap().format.size_mm().1;
    // Every pin the file names is where the file puts it.
    for sym in &plant.symbols {
        let placed = p
            .symbols()
            .filter(|s| s.pins.len() == sym.pins.len())
            .any(|s| {
                sym.pins.iter().all(|pin| {
                    let want = Point::new(sym.at.0 + pin.offset.0, h - (sym.at.1 + pin.offset.1));
                    (0..s.pins.len()).any(|i| {
                        s.pin_position(i as u16).is_some_and(|at| {
                            (at.x - want.x).abs() < 1e-6 && (at.y - want.y).abs() < 1e-6
                        })
                    })
                })
            });
        assert!(placed, "a {} symbol is not on the file's pins", sym.kind);
    }
    let open = p
        .wires()
        .flat_map(|w| [w.from, w.to])
        .filter(|e| matches!(e, Endpoint::Free { .. }))
        .count();
    assert_eq!(open, 0, "no wire ends in the air");
    assert!(p.junctions().count() >= 3, "the file's three junctions");
    let bad = problems(&p, &art());
    assert!(bad.is_empty(), "{}", bad.join("\n"));
    // The coil's A1 and the supply terminal L3 are one net, through a junction.
    let net_of = |name: &str, nth: usize| {
        let (sid, i) = p
            .symbols()
            .filter_map(|s| {
                s.pins
                    .iter()
                    .position(|x| x.name == name)
                    .map(|i| (s.id, i as u16))
            })
            .nth(nth)
            .unwrap();
        p.netlist()
            .nets()
            .iter()
            .find(|n| n.endpoints.contains(&Endpoint::pin(sid, i)))
            .map(|n| n.id)
    };
    assert_eq!(net_of("A1", 0), net_of("13", 0));
    assert_ne!(net_of("A1", 0), net_of("A2", 0));
    assert_eq!(net_of("A2", 0), net_of("X2", 0));
}
