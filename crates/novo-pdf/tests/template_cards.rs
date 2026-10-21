use lopdf::Document;
use novo_layout::drill::{CutoutRow, CutoutShape, Surface};
use novo_layout::plate_drawing::Marks;
use novo_layout::template::TemplateSheet;
use novo_pdf::template_cards::{export_cutout_cards_pdf, export_scale_check_pdf};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn door(cutouts: Vec<(CutoutShape, f64, f64)>) -> TemplateSheet {
    TemplateSheet {
        title: "Cab".into(),
        surface: Surface::Door,
        width: 600.0,
        height: 800.0,
        marks: Marks::default(),
        holes: Vec::new(),
        cutouts: cutouts
            .into_iter()
            .enumerate()
            .map(|(i, (shape, x, y))| CutoutRow {
                no: i + 1,
                surface: Surface::Door,
                shape,
                x,
                y,
                source: format!("-H{}", i + 1),
            })
            .collect(),
        without_mounting_data: Vec::new(),
        clip_on: Vec::new(),
        problems: Vec::new(),
    }
}

fn page_sizes_mm(bytes: &[u8]) -> Vec<(f64, f64)> {
    let doc = Document::load_mem(bytes).unwrap();
    doc.get_pages()
        .values()
        .map(|id| {
            let page = doc.get_dictionary(*id).unwrap();
            let b = page.get(b"MediaBox").unwrap().as_array().unwrap();
            let n = |i: usize| match &b[i] {
                lopdf::Object::Integer(v) => *v as f64,
                lopdf::Object::Real(v) => f64::from(*v),
                o => panic!("{o:?}"),
            };
            ((n(2) - n(0)) / PT_PER_MM, (n(3) - n(1)) / PT_PER_MM)
        })
        .collect()
}

fn contents(bytes: &[u8]) -> String {
    let doc = Document::load_mem(bytes).unwrap();
    doc.get_pages()
        .keys()
        .map(|n| String::from_utf8_lossy(&doc.get_page_content(doc.get_pages()[n])).into_owned())
        .collect()
}

/// Page content is in millimetres; a rectangle is `x y w h re`.
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
fn scale_check_is_one_a4_page_with_the_square() {
    let pdf = export_scale_check_pdf("Crane");
    let sizes = page_sizes_mm(&pdf);
    assert_eq!(sizes.len(), 1);
    assert!((sizes[0].0 - 210.0).abs() < 0.01 && (sizes[0].1 - 297.0).abs() < 0.01);
    assert_eq!(pdf, export_scale_check_pdf("Crane"), "deterministic");
}

#[test]
fn the_check_square_is_exactly_100_mm_on_the_page() {
    // The page content is in millimetres, a rectangle is `x y w h re`.
    let text = contents(&export_scale_check_pdf("Crane"));
    assert!(
        has_rect(&text, 100.0, 100.0),
        "no 100 x 100 mm rectangle in the page content"
    );
}

#[test]
fn one_card_per_shape_and_a_page_each() {
    let r = CutoutShape::Round { dia: 22.5 };
    let sheet = door(vec![
        (r, 100.0, 150.0),
        (CutoutShape::Rect { w: 92.0, h: 92.0 }, 300.0, 400.0),
        (r, 200.0, 150.0),
    ]);
    let cards = export_cutout_cards_pdf(&[sheet], "Crane").unwrap();
    assert_eq!(cards.pages, 2);
    assert_eq!(cards.cutouts, 3);
    assert!(cards.too_large.is_empty());
    assert!(
        page_sizes_mm(&cards.pdf)
            .iter()
            .all(|s| (s.0 - 210.0).abs() < 0.01 && (s.1 - 297.0).abs() < 0.01)
    );
}

#[test]
fn the_outline_is_true_size() {
    let sheet = door(vec![(CutoutShape::Rect { w: 92.0, h: 70.0 }, 300.0, 400.0)]);
    let text = contents(&export_cutout_cards_pdf(&[sheet], "Crane").unwrap().pdf);
    assert!(has_rect(&text, 92.0, 70.0), "92 x 70 mm outline missing");
}

#[test]
fn a_cutout_too_big_for_a_card_is_named_not_dropped() {
    let sheet = door(vec![
        (CutoutShape::Rect { w: 200.0, h: 120.0 }, 300.0, 400.0),
        (CutoutShape::Round { dia: 30.5 }, 100.0, 100.0),
    ]);
    let cards = export_cutout_cards_pdf(&[sheet], "Crane").unwrap();
    assert_eq!(cards.pages, 1);
    assert_eq!(cards.too_large, vec!["200 x 120 on Door".to_owned()]);
}

#[test]
fn many_positions_continue_on_more_pages() {
    let sheet = door(
        (0..80)
            .map(|i| {
                (
                    CutoutShape::Round { dia: 22.5 },
                    30.0 + f64::from(i) * 6.0,
                    100.0,
                )
            })
            .collect(),
    );
    let cards = export_cutout_cards_pdf(&[sheet], "Crane").unwrap();
    assert!(cards.pages >= 2, "{} pages", cards.pages);
    assert_eq!(cards.cutouts, 80);
    assert_eq!(page_sizes_mm(&cards.pdf).len(), cards.pages);
}

#[test]
fn no_cutouts_no_cards() {
    assert!(export_cutout_cards_pdf(&[door(Vec::new())], "Crane").is_none());
}

#[test]
fn a_project_with_a_door_lamp_gets_a_card_for_its_cutout() {
    use novo_core::{DeviceTag, MountingPlate, Project, Value};
    use novo_layout::model::{apply_plan, candidates, load, set_plate, template_documents};
    let mut p = Project::new("t");
    let (cab, _) = p
        .transact("setup", |tx| {
            let cab = tx.add_structure(DeviceTag::parse("+CAB1").unwrap(), "Cabinet", true)?;
            let lamp = tx.add_part("ACME", "D1")?;
            tx.set_property(lamp, "width_mm", Some(Value::Float(72.0)))?;
            tx.set_property(lamp, "height_mm", Some(Value::Float(72.0)))?;
            tx.set_property(lamp, "mounting", Some(Value::Text("door_cutout".into())))?;
            tx.set_property(lamp, "cutout", Some(Value::Text("round 22.5".into())))?;
            let h = tx.add_device(DeviceTag::parse("=A1+CAB1-P1").unwrap(), "")?;
            tx.assign_part(h, lamp)?;
            set_plate(
                tx,
                cab,
                MountingPlate {
                    width: 400.0,
                    height: 400.0,
                    margin: 10.0,
                },
            )?;
            Ok(cab)
        })
        .unwrap();
    let plan = load(&p, cab).unwrap().preview(&candidates(&p, cab));
    p.transact("place", |tx| apply_plan(tx, cab, &plan))
        .unwrap();
    let (sheets, table) = template_documents(&p, cab).unwrap();
    assert_eq!(table.cutouts.len(), 1);
    let cards = export_cutout_cards_pdf(&sheets, "Plant").unwrap();
    assert_eq!((cards.pages, cards.cutouts), (1, 1));
    assert!(cards.too_large.is_empty());
}
