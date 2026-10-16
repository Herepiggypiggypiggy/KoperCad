use super::*;

const SMALL: &str = include_str!("../../../tests/fixtures/pxf/synthetic-small.epj");

#[test]
fn reads_pages_with_labels_and_titles() {
    let f = read_pxf_str(SMALL).unwrap();
    assert_eq!(f.header.kind, PxfKind::Project);
    assert_eq!(f.header.pxf_version, "1.23");
    assert_eq!(f.pages.len(), 2);
    assert_eq!(f.pages[0].label(), "=A1+CAB1/1");
    assert_eq!(f.pages[0].title.get("de_DE"), "Hauptversorgung");
    assert_eq!(f.pages[1].title.get("en_US"), "Motor feeder");
    assert_eq!(f.pages[0].id, Some(ObjId { kind: 4, number: 1 }));
    assert_eq!(f.pages[0].objects, 9);
}

#[test]
fn devices_terminals_and_cables_keep_tag_part_and_place() {
    let f = read_pxf_str(SMALL).unwrap();
    let q = &f.devices[0];
    assert_eq!(q.role, DeviceRole::Device);
    assert_eq!(q.tag.full(), "=A1+CAB1-Q1");
    assert_eq!(q.tag.display, "=A1+CAB1-Q1");
    assert_eq!(split_part_number(&q.part_number), ("DEMO", "BRK3P16"));
    assert_eq!(q.type_text.get("en_US"), "Breaker 3P");
    assert_eq!(q.position, Some((42.5, 200.0)));
    assert_eq!(
        q.symbol,
        Some(SymbolRef {
            number: 5,
            variant: 0,
            library: 1
        })
    );
    assert_eq!(q.page, Some(0));

    let x = &f.devices[1];
    assert_eq!(x.role, DeviceRole::Terminal);
    assert_eq!(x.terminal_text, "1");
    assert_eq!(f.devices[2].role, DeviceRole::Cable);
    assert_eq!(f.devices[2].cable_text, "en_US@Motor cable;");
    // sub index is written after a dot
    assert_eq!(f.devices[3].tag.full(), "=A1+CAB1-M1.2");
    assert_eq!(f.devices[3].page, Some(1));
}

#[test]
fn cores_texts_and_interruption_points() {
    let f = read_pxf_str(SMALL).unwrap();
    assert_eq!(f.cores.len(), 1);
    assert_eq!(f.cores[0].number, "1");
    assert_eq!(f.cores[0].cable.full(), "=A1+CAB1-W1");
    assert_eq!(f.texts.len(), 1);
    assert_eq!(f.texts[0].text.get("en_US"), "Sheet note");
    assert_eq!(f.texts[0].position, Some((10.0, 10.0)));
    assert_eq!(f.interruptions.len(), 3);
    let (pairs, alone) = pair_interruptions(&f.interruptions);
    assert_eq!(pairs, vec![(0, 1)]);
    assert_eq!(alone, vec![2]);
}

#[test]
fn the_report_counts_what_is_not_carried() {
    let r = read_pxf_str(SMALL).unwrap().report();
    assert_eq!(r.count_of("pages"), 2);
    assert_eq!(r.count_of("devices"), 2);
    assert_eq!(r.count_of("terminals"), 1);
    assert_eq!(r.count_of("cables"), 1);
    assert_eq!(r.count_of("cable cores"), 1);
    let left = |w: &str| r.lossy.iter().find(|l| l.what == w).map(|l| l.count);
    assert_eq!(left("lines"), Some(2));
    assert_eq!(left("connection nodes"), Some(1));
    assert_eq!(left("other objects"), Some(1));
    assert!(r.text().contains("no partner"));
    // the declared count (2) matches the pages found: no note about it
    assert!(!r.text().contains("declares"));
}

#[test]
fn a_page_macro_is_recognised_and_other_files_are_refused() {
    let emp = r#"<EplanPxfRoot Type="PageMacro" PxfVersion="1.23"><O4 A1="4/1"><P49 P11012="1"/></O4></EplanPxfRoot>"#;
    let f = read_pxf_str(emp).unwrap();
    assert_eq!(f.header.kind, PxfKind::PageMacro);
    assert_eq!(f.pages.len(), 1);
    assert!(matches!(
        read_pxf_str("<project/>"),
        Err(PxfError::NotPxf(n)) if n == "project"
    ));
    assert_eq!(read_pxf_str(""), Err(PxfError::Empty));
    assert!(matches!(
        read_pxf_str("<EplanPxfRoot><O4>"),
        Err(PxfError::Truncated(_))
    ));
}

#[test]
fn duplicate_ids_and_other_versions_are_noted() {
    let x = r#"<EplanPxfRoot Type="Project" PxfVersion="9.9">
        <O4 A1="4/1"><P49 P11012="1"/></O4><O4 A1="4/1"><P49 P11012="2"/></O4>
    </EplanPxfRoot>"#;
    let f = read_pxf_str(x).unwrap();
    assert_eq!(f.duplicate_ids, vec![ObjId { kind: 4, number: 1 }]);
    let t = f.report().text();
    assert!(t.contains("PXF version 9.9"));
    assert!(t.contains("occur twice"));
}

#[test]
fn objects_after_a_closed_page_belong_to_the_last_page() {
    let x = r#"<EplanPxfRoot Type="Project" PxfVersion="1.23">
        <O4 A1="4/1"><P49 P11012="7"/></O4>
        <O17 A1="17/2"><P150 P20013="K" P20014="1"/></O17>
    </EplanPxfRoot>"#;
    let f = read_pxf_str(x).unwrap();
    assert_eq!(f.devices[0].page, Some(0));
    assert_eq!(f.devices[0].tag.full(), "-K1");
}

#[test]
fn a_large_synthetic_project_reads_from_a_stream() {
    let mut s = String::from(r#"<EplanPxfRoot Type="Project" PxfVersion="1.23">"#);
    for p in 1..=200 {
        s.push_str(&format!(r#"<O4 A1="4/{p}"><P49 P1100="A1" P11012="{p}"/>"#));
        for d in 1..=50 {
            s.push_str(&format!(
                r#"<O17 A1="17/{}"><P150 P20013="K" P20014="{d}"/><P11 P20100_1="DEMO.R{d}"/></O17>"#,
                1000 + p * 100 + d
            ));
        }
        s.push_str("</O4>");
    }
    s.push_str("</EplanPxfRoot>");
    let f = read_pxf(std::io::BufReader::with_capacity(4096, s.as_bytes())).unwrap();
    assert_eq!(f.pages.len(), 200);
    assert_eq!(f.devices.len(), 10_000);
    assert!(f.duplicate_ids.is_empty());
}
