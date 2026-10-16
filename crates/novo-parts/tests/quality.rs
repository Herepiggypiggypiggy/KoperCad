//! Parts quality: completeness scores and the missing-data report.

use novo_core::Value;
use novo_parts::{
    Asset, AssetKind, Catalog, CatalogPart, ConnectionKind, Criterion, DataSource, Dimensions,
    Fill, Grade, PartTerminal, Price, Provenance, assess, keys, quality_report,
};

fn complete() -> CatalogPart {
    let mut p = CatalogPart::new("WAGO", "2002-1201");
    p.description = "feed-through terminal block".into();
    p.dimensions = Some(Dimensions {
        width_mm: 5.2,
        height_mm: 74.0,
        depth_mm: 32.0,
    });
    p.terminals = vec![PartTerminal {
        name: "1".into(),
        kind: ConnectionKind::PushIn,
        wire_mm2: Some((0.08, 4.0)),
        ..Default::default()
    }];
    p.data.insert(keys::POWER_LOSS_W.into(), Value::Float(0.4));
    p.sources
        .insert(keys::POWER_LOSS_W.into(), DataSource::MakerDatasheet);
    p.assets = vec![Asset {
        kind: AssetKind::Step,
        path: "Sources/wago/2002-1201.stp".into(),
        sha256: String::new(),
    }];
    p.price = Some(Price {
        amount: 0.5,
        currency: "EUR".into(),
        per: 1,
        date: "2026-10-15".into(),
    });
    p.provenance = Some(Provenance {
        source: "import:bmecat".into(),
        file: "wago.xml".into(),
        ..Default::default()
    });
    p
}

#[test]
fn a_fully_described_part_scores_100() {
    let c = assess(&complete());
    assert_eq!(c.percent, 100);
    assert!(c.is_complete());
    assert_eq!(c.gaps().count(), 0);
    assert_eq!(c.grade(), Grade::Good);
}

#[test]
fn weights_add_up_to_100() {
    let sum: u32 = Criterion::ALL.iter().map(|c| c.weight()).sum();
    assert_eq!(sum, 100);
}

#[test]
fn an_empty_part_scores_zero_and_every_line_is_missing() {
    let c = assess(&CatalogPart::new("X", "1"));
    assert_eq!(c.percent, 0);
    assert_eq!(c.grade(), Grade::Poor);
    for criterion in Criterion::ALL {
        assert_eq!(c.fill(criterion), Fill::Missing, "{criterion:?}");
        assert!(!c.finding(criterion).note.is_empty());
    }
}

#[test]
fn partial_lines_count_half() {
    let mut p = complete();
    p.price.as_mut().unwrap().date.clear();
    let c = assess(&p);
    assert_eq!(c.fill(Criterion::Price), Fill::Partial);
    assert_eq!(
        c.finding(Criterion::Price).note,
        "price list date not recorded"
    );
    // 100 - 15 / 2 = 92.5, rounds to 93.
    assert_eq!(c.percent, 93);
}

#[test]
fn loose_size_values_in_data_are_partial_not_complete() {
    let mut p = complete();
    p.dimensions = None;
    p.data.insert(keys::WIDTH_MM.into(), Value::Float(5.2));
    p.data.insert(keys::HEIGHT_MM.into(), Value::Int(74));
    let f = assess(&p).finding(Criterion::Dimensions).clone();
    assert_eq!(f.fill, Fill::Partial);
    assert_eq!(f.note, "only width and height given");
}

#[test]
fn zero_or_nan_sizes_are_flagged_by_name() {
    let mut p = complete();
    p.dimensions = Some(Dimensions {
        width_mm: 5.0,
        height_mm: 0.0,
        depth_mm: f64::NAN,
    });
    let f = assess(&p).finding(Criterion::Dimensions).clone();
    assert_eq!(f.fill, Fill::Partial);
    assert_eq!(f.note, "height, depth is zero or not a number");
}

#[test]
fn terminals_need_a_type_and_a_wire_range() {
    let mut p = complete();
    p.terminals.push(PartTerminal {
        name: "2".into(),
        ..Default::default()
    });
    let f = assess(&p).finding(Criterion::Terminals).clone();
    assert_eq!(f.fill, Fill::Partial);
    assert_eq!(
        f.note,
        "1 of 2 without connection type, 1 of 2 without wire range"
    );
}

#[test]
fn power_loss_depends_on_its_source() {
    let mut p = complete();
    let fill = |p: &CatalogPart| assess(p).fill(Criterion::PowerLoss);
    assert_eq!(fill(&p), Fill::Complete);
    p.sources
        .insert(keys::POWER_LOSS_W.into(), DataSource::Typed);
    assert_eq!(fill(&p), Fill::Complete);
    p.sources
        .insert(keys::POWER_LOSS_W.into(), DataSource::Estimated);
    assert_eq!(fill(&p), Fill::Partial);
    p.sources.remove(keys::POWER_LOSS_W);
    assert_eq!(fill(&p), Fill::Partial);
    p.data
        .insert(keys::POWER_LOSS_W.into(), Value::Text("0.4 W".into()));
    assert_eq!(fill(&p), Fill::Partial);
    p.data.remove(keys::POWER_LOSS_W);
    assert_eq!(fill(&p), Fill::Missing);
}

#[test]
fn geometry_wants_a_model_not_just_a_picture() {
    let mut p = complete();
    let fill = |p: &CatalogPart| assess(p).fill(Criterion::Geometry);
    p.assets[0].kind = AssetKind::Mesh;
    assert_eq!(fill(&p), Fill::Complete);
    p.assets[0].kind = AssetKind::Dxf2d;
    assert_eq!(fill(&p), Fill::Partial);
    p.assets[0].kind = AssetKind::Image;
    assert_eq!(fill(&p), Fill::Missing);
    p.assets.clear();
    assert_eq!(fill(&p), Fill::Missing);
}

#[test]
fn provenance_of_an_import_names_the_file() {
    let mut p = complete();
    p.provenance.as_mut().unwrap().file.clear();
    assert_eq!(assess(&p).fill(Criterion::Provenance), Fill::Partial);
    p.provenance.as_mut().unwrap().source = "authored".into();
    assert_eq!(assess(&p).fill(Criterion::Provenance), Fill::Complete);
    p.provenance = None;
    assert_eq!(assess(&p).fill(Criterion::Provenance), Fill::Missing);
}

fn catalog() -> Catalog {
    let mut c = Catalog::new();
    c.upsert(complete());
    let mut half = CatalogPart::new("Siemens", "3RT2016-1BB41");
    half.description = "contactor, 24 V".into();
    half.dimensions = Some(Dimensions {
        width_mm: 45.0,
        height_mm: 58.0,
        depth_mm: 73.0,
    });
    c.upsert(half);
    c.upsert(CatalogPart::new("ABB", "S201-B16"));
    c
}

#[test]
fn the_report_counts_grades_and_gaps() {
    let r = quality_report(&catalog());
    assert_eq!(r.len(), 3);
    assert_eq!(r.count_grade(Grade::Good), 1);
    assert_eq!(r.count_grade(Grade::Fair), 0);
    assert_eq!(r.count_grade(Grade::Poor), 2);
    assert_eq!(r.count_gaps(Criterion::Dimensions), 1);
    assert_eq!(r.count_gaps(Criterion::Price), 2);
    assert_eq!(r.count_fill(Criterion::Price, Fill::Missing), 2);
    let avg = r.average_percent().unwrap();
    assert!((avg - (100.0 + 20.0 + 0.0) / 3.0).abs() < 1e-9);
    assert!(quality_report(&Catalog::new()).average_percent().is_none());
}

#[test]
fn lists_put_the_worst_part_first_and_leave_complete_parts_out() {
    let r = quality_report(&catalog());
    let rows = r.incomplete();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].order_number, "S201-B16");
    assert_eq!(rows[1].order_number, "3RT2016-1BB41");
    let gap = r.with_gap(Criterion::Dimensions);
    assert_eq!(gap.len(), 1);
    assert_eq!(gap[0].manufacturer, "ABB");
}

#[test]
fn the_csv_lists_incomplete_parts_with_notes() {
    let csv = quality_report(&catalog()).missing_csv();
    let mut lines = csv.lines();
    assert_eq!(
        lines.next().unwrap(),
        "Manufacturer,Order number,Description,Complete %,Dimensions,Terminals,Power loss,3D model,Price,Origin,Notes"
    );
    let abb = lines.next().unwrap();
    assert!(abb.starts_with("ABB,S201-B16,,0,missing,missing,"), "{abb}");
    let siemens = lines.next().unwrap();
    assert!(
        siemens.starts_with("Siemens,3RT2016-1BB41,\"contactor, 24 V\",20,complete,missing,"),
        "{siemens}"
    );
    assert!(siemens.contains("Terminals: no connection points; "));
    assert!(lines.next().is_none());
}

#[test]
fn csv_cells_cannot_start_a_formula() {
    let mut c = Catalog::new();
    c.upsert(CatalogPart::new("=cmd", "1"));
    let csv = quality_report(&c).missing_csv();
    assert!(csv.lines().nth(1).unwrap().starts_with("\"'=cmd\",1,"));
}
