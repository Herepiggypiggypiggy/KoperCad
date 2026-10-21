use lopdf::{Document, Object};
use novo_core::{DeviceTag, MountingPlate, Project, Value};
use novo_layout::model::{apply_plan, candidates, load, set_plate, template_documents};
use novo_layout::template::{Paper, TemplateSheet};
use novo_pdf::template::{export_plate_pdf, export_template_pdf, page_count};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn sheets(devices: usize, plate: (f64, f64)) -> Vec<TemplateSheet> {
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let part = tx.add_part("ACME", "C1")?;
            tx.set_property(part, "width_mm", Some(Value::Float(20.0)))?;
            tx.set_property(part, "height_mm", Some(Value::Int(60)))?;
            tx.set_property(
                part,
                "hole_pattern",
                Some(Value::Text("3,3,4.2;17,57,4.2".into())),
            )?;
            for n in 1..=devices {
                let d = tx.add_device(DeviceTag::parse(&format!("=A1+CAB1-K{n}")).unwrap(), "")?;
                tx.assign_part(d, part)?;
            }
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: plate.0,
                    height: plate.1,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    template_documents(&p, cab).unwrap().0
}

fn number(o: &Object) -> f64 {
    match o {
        Object::Integer(i) => *i as f64,
        Object::Real(r) => f64::from(*r),
        other => panic!("{other:?}"),
    }
}

/// Horizontal line segments (length in page mm) of a page's content, found
/// from the `m`/`l` operators; the page transform maps 1 unit to 1 mm.
fn horizontal_segments(pdf: &Document, page: u32) -> Vec<f64> {
    let id = pdf.get_pages()[&page];
    let content = pdf.get_and_decode_page_content(id).unwrap();
    let mut out = Vec::new();
    let mut from: Option<(f64, f64)> = None;
    for op in &content.operations {
        match op.operator.as_str() {
            "m" => from = Some((number(&op.operands[0]), number(&op.operands[1]))),
            "l" => {
                let to = (number(&op.operands[0]), number(&op.operands[1]));
                if let Some(f) = from.replace(to)
                    && (f.1 - to.1).abs() < 1e-3
                {
                    out.push((to.0 - f.0).abs());
                }
            }
            _ => {}
        }
    }
    out
}

#[test]
fn a_big_plate_is_tiled_with_overview_first() {
    let sheets = sheets(4, (600.0, 800.0));
    let a4 = export_template_pdf(&sheets, "Demo", Paper::A4);
    let pdf = Document::load_mem(&a4).unwrap();
    let expect = page_count(&sheets[0], Paper::A4);
    assert!(expect > 6, "{expect}");
    assert_eq!(pdf.get_pages().len(), expect);
    let first = pdf.extract_text(&[1]).unwrap();
    assert!(first.contains("Drilling template 1:1"), "{first}");
    assert!(first.contains("100%"), "{first}");
    let tile = pdf.extract_text(&[2]).unwrap();
    assert!(tile.contains("Tile A1"), "{tile}");
    // A3 needs fewer sheets for the same plate.
    let a3 = Document::load_mem(&export_template_pdf(&sheets, "Demo", Paper::A3)).unwrap();
    assert!(a3.get_pages().len() < pdf.get_pages().len());
}

#[test]
fn every_sheet_has_a_ruler_that_is_exactly_100_mm_on_a_page_of_the_right_size() {
    let sheets = sheets(2, (400.0, 500.0));
    let pdf = Document::load_mem(&export_template_pdf(&sheets, "Demo", Paper::A4)).unwrap();
    let pages = pdf.get_pages();
    for (n, id) in &pages {
        let page = pdf.get_dictionary(*id).unwrap();
        let media: Vec<f64> = page
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(number)
            .collect();
        let (w, h) = (media[2], media[3]);
        let short = w.min(h) / PT_PER_MM;
        let long = w.max(h) / PT_PER_MM;
        assert!(
            (short - 210.0).abs() < 0.01 && (long - 297.0).abs() < 0.01,
            "{short} x {long}"
        );
        let segs = horizontal_segments(&pdf, *n);
        assert!(
            segs.iter().any(|l| (l - 100.0).abs() < 0.01),
            "page {n}: no 100 mm ruler in {segs:?}"
        );
    }
    let tile = pdf.extract_text(&[2]).unwrap();
    assert!(tile.contains("Must measure 100 mm"), "{tile}");
}

#[test]
fn holes_are_drawn_at_their_true_size() {
    // A hole of 4.2 mm is a circle of radius 2.1 mm: its bezier arc
    // control points reach 2.1 mm from the centre on tile A1.
    let sheets = sheets(1, (150.0, 200.0));
    assert_eq!(page_count(&sheets[0], Paper::A4), 2);
    let a = export_template_pdf(&sheets, "Demo", Paper::A4);
    assert_eq!(a, export_template_pdf(&sheets, "Demo", Paper::A4));
    let pdf = Document::load_mem(&a).unwrap();
    let id = pdf.get_pages()[&2];
    let content = pdf.get_and_decode_page_content(id).unwrap();
    // Four curves per circle, start point on the x axis of the centre.
    let mut radii = Vec::new();
    let mut at: Option<(f64, f64)> = None;
    for op in &content.operations {
        match op.operator.as_str() {
            "m" => at = Some((number(&op.operands[0]), number(&op.operands[1]))),
            "c" => {
                let end = (number(&op.operands[4]), number(&op.operands[5]));
                if let Some(s) = at.replace(end) {
                    radii.push(((s.0 - end.0).powi(2) + (s.1 - end.1).powi(2)).sqrt());
                }
            }
            _ => {}
        }
    }
    // The chord of a quarter circle is r * sqrt(2).
    let hole = 2.1 * std::f64::consts::SQRT_2;
    assert!(
        radii.iter().any(|r| (r - hole).abs() < 0.01),
        "no 4.2 mm hole in {radii:?}"
    );
}

/// Writes a sample to `$TEMPLATE_DUMP` to look at it: `TEMPLATE_DUMP=/tmp/t.pdf
/// cargo test -p novo-pdf --test template -- --ignored dump`.
#[test]
#[ignore]
fn dump() {
    let Ok(path) = std::env::var("TEMPLATE_DUMP") else {
        return;
    };
    let bytes = export_template_pdf(&sheets(6, (600.0, 800.0)), "Demo", Paper::A4);
    std::fs::write(path, bytes).unwrap();
}

#[test]
fn the_single_sheet_plate_is_one_page_the_size_of_the_plate_with_a_100_mm_ruler() {
    let sheets = sheets(3, (450.0, 550.0));
    let bytes = export_plate_pdf(&sheets[0], "Demo").expect("a plate this size fits one page");
    assert_eq!(bytes, export_plate_pdf(&sheets[0], "Demo").unwrap());
    let pdf = Document::load_mem(&bytes).unwrap();
    let pages = pdf.get_pages();
    assert_eq!(pages.len(), 1);
    let page = pdf.get_dictionary(pages[&1]).unwrap();
    let media: Vec<f64> = page
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(number)
        .collect();
    assert!((media[2] / PT_PER_MM - 450.0).abs() < 0.01, "{media:?}");
    assert!((media[3] / PT_PER_MM - 550.0).abs() < 0.01, "{media:?}");
    let segs = horizontal_segments(&pdf, 1);
    assert!(
        segs.iter().any(|l| (l - 100.0).abs() < 0.01),
        "no 100 mm ruler in {segs:?}"
    );
    assert!(pdf.extract_text(&[1]).unwrap().contains("not fit to page"));

    // Too small for the ruler, or longer than a PDF page can be: no single sheet.
    for (w, h) in [(100.0, 100.0), (6000.0, 800.0), (f64::INFINITY, 800.0)] {
        let odd = TemplateSheet {
            width: w,
            height: h,
            ..sheets[0].clone()
        };
        assert!(export_plate_pdf(&odd, "Demo").is_none(), "{w} x {h}");
    }
}
