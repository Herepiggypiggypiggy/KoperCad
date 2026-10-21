//! The PXF writer: structure of the file, and a round trip through the
//! reader and the import plan. Passing proves the file agrees with this
//! reader, not that EPLAN accepts it.

use novo_core::{CableCore, DeviceTag, Document, Pin, Placement, Point, Project, SymbolRole};
use novo_interop::eplan::pxf::{
    PxfOptions, PxfWriteOptions, is_kopercad_pxf, plan_pxf, read_pxf_str, write_pxf,
};
use novo_interop::eplan::symbols::{SymbolKey, UserTable};

fn tag(s: &str) -> DeviceTag {
    DeviceTag::parse(s).unwrap()
}

fn project() -> Project {
    let mut p = Project::new("Pump station");
    p.transact("setup", |tx| {
        tx.add_structure(tag("=A1"), "Pump section", false)?;
        tx.add_structure(tag("+CAB1"), "Cabinet", true)?;
        let part = tx.add_part("ACME", "C-100")?;
        let k1 = tx.add_device(tag("=A1+CAB1-K1"), "Main contactor; 25 A")?;
        tx.assign_part(k1, part)?;
        let x1 = tx.add_terminal_strip(tag("=A1+CAB1-X1"))?;
        let t1 = tx.add_terminal(x1, "1")?;
        tx.add_terminal(x1, "PE")?;
        tx.add_device(tag("=A1+CAB1-F1"), "Never drawn")?;
        tx.add_cable(
            tag("=A1-W1"),
            vec![
                CableCore::new("1"),
                CableCore::new("BK"),
                CableCore::new(""),
            ],
        )?;
        let page = tx.add_page("1")?;
        tx.place_symbol(
            page,
            "iec/coils/relay",
            Placement::at(Point::new(42.5, 97.0)),
            vec![Pin::new("A1", 0.0, 0.0), Pin::new("A2", 0.0, 10.0)],
            SymbolRole::Device {
                device: k1,
                main: true,
            },
        )?;
        tx.place_symbol(
            page,
            "iec/terminal",
            Placement::at(Point::new(84.0, 50.0)),
            vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)],
            SymbolRole::Terminal {
                terminals: vec![t1],
            },
        )?;
        Ok(())
    })
    .unwrap();
    p
}

#[test]
fn the_file_has_the_root_pages_devices_terminals_and_cable() {
    let out = write_pxf(&project(), &PxfWriteOptions::default());
    let t = &out.text;
    assert!(t.starts_with("<?xml"));
    assert!(is_kopercad_pxf(t.as_bytes()));
    assert!(t.contains("PxfVersion=\"1.23\""));
    assert!(t.contains("<ESymLib Id=\"1\" Name=\"IEC_symbol\"/>"));
    assert!(t.contains("<O4 "));
    assert!(t.contains("P20100_1=\"ACME.C-100\""));
    assert!(t.contains("P20030=\"1\""));
    assert!(t.contains("P31004=\"BK\""));
    // The `;` in the description would end the language entry.
    assert!(t.contains("en_US@Main contactor, 25 A;"), "{t}");
    // Ids are unique.
    let ids: Vec<&str> = t
        .match_indices("A1=\"")
        .map(|(i, _)| &t[i + 4..i + 16])
        .collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len());
    // Deterministic.
    assert_eq!(t, &write_pxf(&project(), &PxfWriteOptions::default()).text);
}

#[test]
fn the_reader_reads_back_what_was_written() {
    let out = write_pxf(&project(), &PxfWriteOptions::default());
    let f = read_pxf_str(&out.text).unwrap();
    assert_eq!(f.pages.len(), 1);
    assert!(f.duplicate_ids.is_empty());
    let tags: Vec<String> = f.devices.iter().map(|d| d.tag.full()).collect();
    assert!(tags.contains(&"=A1+CAB1-K1".to_string()), "{tags:?}");
    let k1 = f
        .devices
        .iter()
        .find(|d| d.tag.full() == "=A1+CAB1-K1")
        .unwrap();
    assert_eq!(k1.part_number, "ACME.C-100");
    assert_eq!(k1.type_text.get("en_US"), "Main contactor, 25 A");
    // KoperCAD's page is A4 landscape by default or A3; y is flipped to the
    // sheet's bottom-left origin.
    let (x, y) = k1.position.unwrap();
    assert_eq!(x, 42.5);
    assert!(y > 0.0 && y != 97.0);
    assert_eq!(f.cores.len(), 2, "the unnamed core is not written");
    let cable = f.devices.iter().find(|d| d.tag.full() == "=A1-W1").unwrap();
    assert!(!cable.cable_text.is_empty());
}

#[test]
fn the_report_counts_what_was_left_out_and_says_it_was_never_opened_in_eplan() {
    let r = write_pxf(&project(), &PxfWriteOptions::default()).report;
    assert_eq!(r.count_of("pages"), 1);
    assert_eq!(r.count_of("cables"), 1);
    assert_eq!(r.count_of("cable cores"), 2);
    let lost = |what: &str| {
        r.lossy
            .iter()
            .find(|l| l.what == what)
            .map_or(0, |l| l.count)
    };
    // Undrawn devices and terminals are written on the first page, with no
    // symbol or position, rather than dropped.
    assert_eq!(
        r.count_of("devices and terminals without a drawing, on the first page"),
        2
    );
    assert_eq!(lost("devices without a drawn symbol"), 0);
    assert_eq!(lost("terminals without a drawn symbol"), 0);
    let text = r.text();
    assert!(text.contains("not been opened in a real EPLAN"), "{text}");
}

#[test]
fn a_symbol_key_in_the_users_table_becomes_the_symbol_reference() {
    let mut symbols = UserTable::new();
    symbols.insert(
        SymbolKey {
            set: 1,
            number: 77,
            variant: 2,
        },
        "iec/coils/relay",
    );
    let with = write_pxf(
        &project(),
        &PxfWriteOptions {
            symbols,
            ..PxfWriteOptions::default()
        },
    );
    assert!(
        with.text.contains("A1381=\"77\" A1382=\"2\" A1383=\"1\""),
        "{}",
        with.text
    );
    let without = write_pxf(&project(), &PxfWriteOptions::default());
    assert!(!without.text.contains("A1381="));
}

#[test]
fn a_written_file_imports_into_an_empty_project() {
    let out = write_pxf(&project(), &PxfWriteOptions::default());
    let f = read_pxf_str(&out.text).unwrap();
    let mut doc = Document::default();
    let plan = plan_pxf(doc.project(), &f, &PxfOptions::default());
    plan.apply(&mut doc).unwrap();
    let p = doc.project();
    assert_eq!(p.pages().count(), 1);
    assert_eq!(p.cables().count(), 1);
    assert_eq!(p.devices_by_tag(&tag("=A1+CAB1-K1")).len(), 1);
    assert_eq!(p.devices_by_tag(&tag("=A1+CAB1-X1")).len(), 1);
}

#[test]
fn a_project_without_pages_writes_a_valid_empty_file_and_says_so() {
    let out = write_pxf(&Project::new("Empty"), &PxfWriteOptions::default());
    let f = read_pxf_str(&out.text).unwrap();
    assert!(f.pages.is_empty());
    assert!(out.report.text().contains("no pages"));
}
