use lopdf::Document;
use novo_core::WireId;
use novo_pdf::wireit::{Line, export_wireit_pdf, layout, page_count};
use novo_reports::{DuctStep, WireClass, WireItPlan, WireItStep};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn step(n: usize, class: WireClass, duct_fill: f64) -> WireItStep {
    WireItStep {
        number: n,
        key: format!("{n}.0>{n}.1"),
        wires: vec![WireId::from(n as u64)],
        wire_number: format!("W{n}"),
        class,
        colour: "black".into(),
        cross_section_mm2: Some(1.5),
        from: format!("=A1+CAB-K{n}:13"),
        to: "=A1+CAB-X1:5".into(),
        cut_mm: 300.0 + n as f64,
        length_mm: 280.0 + n as f64,
        allowance_mm: 10.0,
        strip_from_mm: Some(10.0),
        strip_to_mm: Some(10.0),
        fitting_from: Some("A1.5-10".into()),
        fitting_to: Some("A1.5-10".into()),
        label_from: format!("W{n} to X1:5"),
        label_to: format!("W{n} to K{n}:13"),
        basis: "routed".into(),
        track: "D1 > D2".into(),
        ducts: vec![DuctStep {
            duct: 7,
            name: "D1".into(),
            fill_before: Some(duct_fill - 0.01),
            fill_after: Some(duct_fill),
            wires_after: n,
        }],
        cable_core: false,
        path: None,
    }
}

fn plan(n_power: usize, n_signal: usize) -> WireItPlan {
    let mut steps = Vec::new();
    for i in 0..n_power {
        steps.push(step(
            steps.len() + 1,
            WireClass::Power,
            0.1 + i as f64 * 0.01,
        ));
    }
    for i in 0..n_signal {
        steps.push(step(
            steps.len() + 1,
            WireClass::Signal,
            0.5 + i as f64 * 0.01,
        ));
    }
    WireItPlan { steps }
}

#[test]
fn every_step_is_on_exactly_one_page_in_order() {
    let p = plan(30, 25);
    let pages = layout(&p);
    let seen: Vec<usize> = pages
        .iter()
        .flatten()
        .filter_map(|l| match l {
            Line::Step(i) => Some(*i),
            Line::Heading(_) => None,
        })
        .collect();
    assert_eq!(seen, (0..55).collect::<Vec<_>>());
    assert!(pages.len() > 1);
}

#[test]
fn a_heading_is_never_alone_at_the_foot_of_a_page_and_continues_on_the_next() {
    for power in 1..60 {
        let p = plan(power, 7);
        let pages = layout(&p);
        for page in &pages {
            assert!(
                !matches!(page.last(), Some(Line::Heading(_))),
                "heading last with {power} power wires"
            );
            assert!(
                matches!(page.first(), Some(Line::Heading(_))),
                "every page starts with a heading so the pass is known"
            );
        }
    }
    let pages = layout(&plan(40, 3));
    let Some(Line::Heading(h)) = pages[1].first() else {
        panic!("no heading");
    };
    assert!(h.contains("continued"), "{h}");
}

#[test]
fn the_pdf_is_landscape_a4_with_the_counted_pages() {
    let p = plan(30, 25);
    let pdf = export_wireit_pdf(&p, &[false; 55], "Crane").unwrap();
    let doc = Document::load_mem(&pdf).unwrap();
    assert_eq!(doc.get_pages().len(), page_count(&p));
    for id in doc.get_pages().values() {
        let page = doc.get_dictionary(*id).unwrap();
        let b = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let n = |i: usize| match &b[i] {
            lopdf::Object::Integer(v) => *v as f64,
            lopdf::Object::Real(v) => f64::from(*v),
            o => panic!("{o:?}"),
        };
        assert!(((n(2) - n(0)) / PT_PER_MM - 297.0).abs() < 0.01);
        assert!(((n(3) - n(1)) / PT_PER_MM - 210.0).abs() < 0.01);
    }
}

#[test]
fn nothing_to_wire_gives_no_file() {
    assert!(export_wireit_pdf(&WireItPlan::default(), &[], "Crane").is_none());
    assert_eq!(page_count(&WireItPlan::default()), 0);
}

#[test]
fn output_is_deterministic_and_a_ticked_row_is_crossed() {
    let p = plan(5, 3);
    let none = vec![false; 8];
    let mut some = none.clone();
    some[2] = true;
    let a = export_wireit_pdf(&p, &none, "Crane").unwrap();
    let b = export_wireit_pdf(&p, &none, "Crane").unwrap();
    assert_eq!(a, b);
    let c = export_wireit_pdf(&p, &some, "Crane").unwrap();
    assert_ne!(a, c, "the tick changes the page");
    let strokes = |pdf: &[u8]| {
        let doc = Document::load_mem(pdf).unwrap();
        let id = *doc.get_pages().values().next().unwrap();
        String::from_utf8_lossy(&doc.get_page_content(id))
            .matches(" l\n")
            .count()
    };
    assert_eq!(strokes(&c), strokes(&a) + 2, "one cross is two strokes");
}

#[test]
fn a_dump_for_looking_at() {
    if let Ok(path) = std::env::var("WIREIT_DUMP") {
        let mut p = plan(22, 20);
        p.steps[3].basis = "estimate".into();
        p.steps[4].ducts[0].fill_after = Some(1.12);
        let mut done = vec![false; 42];
        done[0] = true;
        done[1] = true;
        std::fs::write(path, export_wireit_pdf(&p, &done, "Crane").unwrap()).unwrap();
    }
}
