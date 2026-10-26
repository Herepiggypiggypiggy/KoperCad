use lopdf::Document;
use novo_layout::drill::{DeviceFeatures, Hole, Surface};
use novo_layout::state::CabinetLayout;
use novo_layout::workpack::{Pack, WireCut, WireGroup, build};
use novo_layout::{Margins, Placement, Plate, Rail, Size};
use novo_pdf::workpack::{export_workpack_pdf, page_count};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn pack(cabinet: &str) -> Pack {
    let mut l = CabinetLayout::new(Plate {
        width: 800.0,
        height: 600.0,
        margin: 10.0,
    });
    l.rails = vec![Rail {
        id: 1,
        x: 50.0,
        y: 100.0,
        length: 400.0,
        vertical: false,
    }];
    let mk = |id: u64, x: f64, rail: Option<u64>| Placement {
        id,
        x,
        y: 100.0,
        rotated: false,
        size: Size {
            width: 45.0,
            height: 90.0,
            guessed: false,
        },
        clearance: None,
        margins: Margins::default(),
        rail,
    };
    l.placements = vec![mk(1, 100.0, Some(1)), mk(2, 300.0, None)];
    let f = vec![DeviceFeatures {
        id: 2,
        label: "-Q2".into(),
        surface: Surface::Plate,
        holes: vec![
            Hole {
                dx: 5.0,
                dy: 5.0,
                dia: 4.5,
            },
            Hole {
                dx: 40.0,
                dy: 85.0,
                dia: 4.5,
            },
        ],
        cutout: None,
        problems: Vec::new(),
    }];
    let g = vec![WireGroup {
        title: "Power".into(),
        wires: (0..11)
            .map(|i| WireCut {
                number: format!("W{i}"),
                from: "-Q1:1".into(),
                to: "-Q2:2".into(),
                cut_mm: 300.0 - i as f64,
                kind: "1.5 mm2 black".into(),
                estimated: false,
                end_mm: Some(10.0),
                fitting: "ferrule",
                part: None,
                strip_from_mm: Some(10.0),
                strip_to_mm: Some(10.0),
                strip_basis: "typical",
                ferrule: String::new(),
            })
            .collect(),
    }];
    build(cabinet, &l, &f, &|id| format!("-Q{id}"), &g)
}

fn contents(bytes: &[u8]) -> String {
    let doc = Document::load_mem(bytes).unwrap();
    doc.get_pages()
        .values()
        .map(|id| String::from_utf8_lossy(&doc.get_page_content(*id)).into_owned())
        .collect()
}

fn has_rect(content: &str, w: f64, h: f64) -> bool {
    content.lines().any(|l| {
        let n: Vec<f64> = l
            .strip_suffix(" re")
            .map(|r| {
                r.split_whitespace()
                    .filter_map(|t| t.parse().ok())
                    .collect()
            })
            .unwrap_or_default();
        n.len() == 4 && (n[2] - w).abs() < 0.01 && (n[3] - h).abs() < 0.01
    })
}

#[test]
fn pages_are_a4_and_match_the_count() {
    let packs = [pack("+CAB1")];
    let pdf = export_workpack_pdf(&packs, "Crane").unwrap();
    let doc = Document::load_mem(&pdf).unwrap();
    assert_eq!(doc.get_pages().len(), page_count(&packs));
    for id in doc.get_pages().values() {
        let page = doc.get_dictionary(*id).unwrap();
        let b = page.get(b"MediaBox").unwrap().as_array().unwrap();
        let n = |i: usize| match &b[i] {
            lopdf::Object::Integer(v) => *v as f64,
            lopdf::Object::Real(v) => f64::from(*v),
            o => panic!("{o:?}"),
        };
        assert!(((n(2) - n(0)) / PT_PER_MM - 210.0).abs() < 0.01);
        assert!(((n(3) - n(1)) / PT_PER_MM - 297.0).abs() < 0.01);
    }
}

#[test]
fn a_device_footprint_is_drawn_true_size() {
    let pdf = export_workpack_pdf(&[pack("+CAB1")], "Crane").unwrap();
    let text = contents(&pdf);
    assert!(has_rect(&text, 45.0, 90.0), "45 x 90 mm footprint missing");
}

#[test]
fn a_long_rail_is_drawn_at_a_stated_smaller_scale_not_true_size() {
    let pdf = export_workpack_pdf(&[pack("+CAB1")], "Crane").unwrap();
    let text = contents(&pdf);
    // 400 x 35 mm rail at 1:2.5 is 160 x 14 mm.
    assert!(has_rect(&text, 160.0, 14.0), "scaled rail strip missing");
    assert!(!has_rect(&text, 400.0, 35.0), "an unlabelled 400 mm strip");
}

#[test]
fn a_dump_for_looking_at() {
    // Set WORKPACK_DUMP to a path to write the sample pack for a visual check.
    if let Ok(path) = std::env::var("WORKPACK_DUMP") {
        std::fs::write(
            path,
            export_workpack_pdf(&[pack("+CAB1")], "Crane").unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn output_is_deterministic_and_empty_cabinets_give_nothing() {
    let a = export_workpack_pdf(&[pack("+CAB1")], "Crane").unwrap();
    let b = export_workpack_pdf(&[pack("+CAB1")], "Crane").unwrap();
    assert_eq!(a, b);
    let empty = build(
        "+CAB2",
        &CabinetLayout::new(Plate {
            width: 400.0,
            height: 300.0,
            margin: 10.0,
        }),
        &[],
        &|id| format!("-Q{id}"),
        &[],
    );
    assert!(export_workpack_pdf(&[empty], "Crane").is_none());
}

#[test]
fn one_card_is_one_a4_page_with_that_steps_template() {
    use novo_pdf::workpack::export_card_pdf;
    let p = pack("+CAB1");
    let index = p.cards.iter().position(|c| c.mark == "-Q2").unwrap();
    let pdf = export_card_pdf(&p, index, "Crane").unwrap();
    let doc = Document::load_mem(&pdf).unwrap();
    assert_eq!(doc.get_pages().len(), 1);
    assert!(has_rect(&contents(&pdf), 45.0, 90.0), "true size footprint");
    assert!(export_card_pdf(&p, 999, "Crane").is_none());
}
