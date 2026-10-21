//! AutomationML round trip of a drawn project: KoperCAD writes its pages,
//! symbols and nets; reading the file back into an empty project gives the
//! same pages, the same artwork at the same places and the same nets.

use novo_core::{
    DeviceTag, Direction, Document, Endpoint, Pin, PinRef, Placement, Point, Project, SymbolRole,
};
use novo_interop::aml::{AmlOptions, export_project, plan_import, read_aml};

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

fn drawn() -> Project {
    let mut p = Project::new("Round trip");
    p.transact("setup", |tx| {
        tx.add_structure(tag("=A1"), "Pump section", false)?;
        tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
        let k1 = tx.add_device(tag("=A1+CAB1-K1"), "Main contactor")?;
        let x1 = tx.add_terminal_strip(tag("=A1+CAB1-X1"))?;
        let t1 = tx.add_terminal(x1, "1")?;
        let page = tx.add_page("1")?;
        tx.update::<novo_core::Page>(page, |x| {
            x.structure = tag("=A1+CAB1");
            x.title = "Control".to_string();
        })?;
        let coil = tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(42.5, 97.0)),
            vec![
                Pin::new("A1", 0.0, 0.0).with_direction(Direction::Up),
                Pin::new("A2", 0.0, 10.0).with_direction(Direction::Down),
            ],
            SymbolRole::Device {
                device: k1,
                main: true,
            },
        )?;
        let term = tx.place_symbol(
            page,
            "iec/terminals/terminal",
            Placement::at(Point::new(84.0, 50.0)),
            vec![
                Pin::new("1", 0.0, 0.0).with_direction(Direction::Up),
                Pin::new("2", 0.0, 10.0).with_direction(Direction::Down),
            ],
            SymbolRole::Terminal {
                terminals: vec![t1],
            },
        )?;
        tx.connect(PinRef::new(coil, 0), PinRef::new(term, 1))?;
        Ok(())
    })
    .unwrap();
    p
}

fn back(p: &Project) -> Project {
    let out = export_project(p, &AmlOptions::default());
    let plant = read_aml(&out.text).unwrap();
    let mut doc = Document::new(Project::new("Round trip"));
    plan_import(doc.project(), &plant).apply(&mut doc).unwrap();
    doc.project().clone()
}

#[test]
fn the_pages_and_their_symbols_come_back() {
    let before = drawn();
    let after = back(&before);
    let (pa, pb): (Vec<_>, Vec<_>) = (before.pages().collect(), after.pages().collect());
    assert_eq!(pa.len(), pb.len());
    assert_eq!(pa[0].name, pb[0].name);
    assert_eq!(pa[0].title, pb[0].title);
    assert_eq!(pa[0].structure, pb[0].structure);
    let keys = |p: &Project| {
        let mut v: Vec<String> = p.symbols().map(|s| s.symbol.0.clone()).collect();
        v.sort();
        v
    };
    assert_eq!(keys(&before), keys(&after));
    // Each pin lies where it did.
    let spots = |p: &Project| {
        let mut v: Vec<(String, i64, i64)> = Vec::new();
        for s in p.symbols() {
            for (i, pin) in s.pins.iter().enumerate() {
                let at = s.pin_position(i as u16).unwrap();
                v.push((
                    format!("{}:{}", s.symbol.0, pin.name),
                    (at.x * 100.0).round() as i64,
                    (at.y * 100.0).round() as i64,
                ));
            }
        }
        v.sort();
        v
    };
    assert_eq!(spots(&before), spots(&after));
}

#[test]
fn the_wire_between_the_pins_comes_back_as_one_net() {
    let after = back(&drawn());
    let find = |name: &str, key: &str| {
        let s = after
            .symbols()
            .find(|s| s.symbol.0 == key && s.pins.iter().any(|x| x.name == name))
            .unwrap();
        Endpoint::pin(
            s.id,
            s.pins.iter().position(|x| x.name == name).unwrap() as u16,
        )
    };
    let (a1, t2, a2) = (
        find("A1", "iec/coils/relay"),
        find("2", "iec/terminals/terminal"),
        find("A2", "iec/coils/relay"),
    );
    let nets = after.netlist();
    let net_of = |e: Endpoint| {
        nets.nets()
            .iter()
            .find(|n| n.endpoints.contains(&e))
            .map(|n| n.id)
    };
    assert!(net_of(a1).is_some());
    assert_eq!(net_of(a1), net_of(t2));
    assert_ne!(net_of(a1), net_of(a2));
}

#[test]
fn the_roles_come_back_too() {
    let after = back(&drawn());
    assert!(
        after
            .symbols()
            .any(|s| matches!(&s.role, SymbolRole::Device { main: true, .. }))
    );
    assert!(
        after
            .symbols()
            .any(|s| matches!(&s.role, SymbolRole::Terminal { terminals } if terminals.len() == 1))
    );
}
