//! The comparison of two projects: equal means equal, and every kind of
//! difference is named.

use novo_core::{
    CableCore, DeviceTag, Direction, Document, Pin, PinRef, Placement, Point, Project, SymbolRole,
};
use novo_interop::compare::compare_projects;
use novo_interop::eplan::pxf::{PxfOptions, PxfWriteOptions, plan_pxf, read_pxf_str, write_pxf};

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

fn drawn() -> Project {
    let mut p = Project::new("Compare");
    p.transact("setup", |tx| {
        tx.add_structure(tag("=A1"), "Pump section", false)?;
        tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
        let part = tx.add_part("ACME", "C-100")?;
        let k1 = tx.add_device(tag("=A1+CAB1-K1"), "Main contactor")?;
        tx.assign_part(k1, part)?;
        let x1 = tx.add_terminal_strip(tag("=A1+CAB1-X1"))?;
        let t1 = tx.add_terminal(x1, "1")?;
        tx.add_cable(
            tag("=A1-W1"),
            vec![CableCore::new("1"), CableCore::new("BK")],
        )?;
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

#[test]
fn a_project_equals_itself() {
    let p = drawn();
    let c = compare_projects(&p, &p);
    assert!(c.is_equal(), "{}", c.report.text());
    assert!(c.report.count_of("devices kept") >= 2);
    assert!(c.report.count_of("nets kept") >= 1);
}

#[test]
fn every_kind_of_difference_is_named() {
    let before = drawn();
    let mut after = drawn();
    after
        .transact("edit", |tx| {
            let k1 = tx.project().devices_by_tag(&tag("=A1+CAB1-K1"))[0];
            tx.update::<novo_core::Device>(k1, |d| d.description = "Edited".to_string())?;
            let page = tx.project().pages().next().unwrap().id;
            tx.update::<novo_core::Page>(page, |x| x.title = "Other".to_string())?;
            tx.add_device(tag("=A1+CAB1-F1"), "New")?;
            Ok(())
        })
        .unwrap();
    let c = compare_projects(&before, &after);
    assert!(!c.is_equal());
    let t = c.report.text();
    assert!(t.contains("devices missing or changed"), "{t}");
    assert!(t.contains("Main contactor"), "{t}");
    assert!(t.contains("devices added or changed"), "{t}");
    assert!(t.contains("pages missing or changed"), "{t}");
    // What did not change is not reported.
    assert!(!t.contains("nets missing"), "{t}");
    assert!(!t.contains("drawn symbols missing"), "{t}");
}

#[test]
fn a_cut_wire_shows_as_a_changed_net() {
    let before = drawn();
    let mut after = drawn();
    after
        .transact("cut", |tx| {
            let w = tx.project().wires().next().unwrap().id;
            tx.remove(w)?;
            Ok(())
        })
        .unwrap();
    let c = compare_projects(&before, &after);
    assert!(
        c.report.text().contains("nets missing or changed"),
        "{}",
        c.report.text()
    );
}

/// A drawn project through PXF and back into an empty project. The file
/// carries terminals, cables and devices, and what it does not carry shows up
/// here by name: the EPLAN part number comes back whole as the order number,
/// the page title is part of the page name, and without an EPLAN symbol table
/// nothing is drawn, so no symbols and no nets come back. A new difference, or
/// one that goes away, changes this list and has to be looked at.
#[test]
fn a_pxf_round_trip_differs_only_where_the_file_cannot_carry() {
    let before = drawn();
    let out = write_pxf(&before, &PxfWriteOptions::default());
    let file = read_pxf_str(&out.text).unwrap();
    let mut doc = Document::default();
    plan_pxf(doc.project(), &file, &PxfOptions::default())
        .apply(&mut doc)
        .unwrap();
    let c = compare_projects(&before, doc.project());
    let mut what: Vec<&str> = c.report.lossy.iter().map(|l| l.what.as_str()).collect();
    what.sort_unstable();
    assert_eq!(
        what,
        [
            "devices added or changed",
            "devices missing or changed",
            "drawn symbols missing or changed",
            "nets missing or changed",
            "pages added or changed",
            "pages missing or changed",
            "parts added or changed",
            "parts missing or changed",
        ],
        "{}",
        c.report.text()
    );
    // Terminals and cables survive exactly.
    assert!(c.report.count_of("terminals kept") >= 1);
    assert!(c.report.count_of("cables kept") >= 1);
}
