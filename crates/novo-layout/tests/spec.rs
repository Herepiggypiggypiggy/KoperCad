use novo_core::Properties;
use novo_layout::spec::*;

fn catalogue() -> EnclosureSpec {
    let (rows, errors) = from_csv(
        "article;series;width_mm;height_mm;depth_mm;wall_mm;plate_article;source;terms\n\
         TEST.1;Test;600;1800;500;1,5;PLATE.1;own test file;test only\n",
    );
    assert!(errors.is_empty());
    rows[0].spec()
}

#[test]
fn record_round_trips_through_properties_with_sources() {
    let mut spec = catalogue();
    spec.plate_setback = Some(Field::new(30.0, Source::Step));
    spec.step_name = "x.stp".into();
    spec.step_sha256 = "ab".into();
    let mut props = Properties::new();
    props.insert("depth_mm".into(), 250.0.into());
    spec.write(&mut props);
    assert!(props.contains_key("depth_mm"), "other keys stay");
    assert_eq!(EnclosureSpec::read(&props), spec);
}

#[test]
fn writing_again_replaces_the_old_record() {
    let mut props = Properties::new();
    catalogue().write(&mut props);
    let other = EnclosureSpec {
        width: Some(Field::new(800.0, Source::Typed)),
        ..EnclosureSpec::default()
    };
    other.write(&mut props);
    let read = EnclosureSpec::read(&props);
    assert_eq!(read.article, "");
    assert_eq!(read.height, None);
}

#[test]
fn interior_subtracts_both_walls_and_names_the_weakest_source() {
    let mut spec = catalogue();
    spec.wall = Some(Field::new(2.0, Source::Typed));
    let i = spec.interior().unwrap();
    assert_eq!((i.width, i.height, i.depth), (596.0, 1796.0, 496.0));
    assert_eq!(i.weakest, Source::Typed);
}

#[test]
fn a_missing_field_is_named_never_defaulted() {
    let mut spec = catalogue();
    spec.depth = None;
    assert_eq!(spec.interior(), Err(Missing("outside depth")));
}

#[test]
fn notice_shows_for_catalogue_or_default_only() {
    let mut spec = catalogue();
    assert_eq!(spec.notice(), Some(ESTIMATE_NOTICE));
    for f in [
        &mut spec.width,
        &mut spec.height,
        &mut spec.depth,
        &mut spec.wall,
    ] {
        *f = f.map(|f| Field::new(f.value, Source::Measured));
    }
    assert_eq!(spec.notice(), None);
}

#[test]
fn step_wins_and_a_conflict_beyond_tolerance_shows_both() {
    let r = reconcile(Some(600.0), Some(601.5)).unwrap();
    assert_eq!(r.field, Field::new(600.0, Source::Step));
    assert_eq!(r.conflict, None);
    let r = reconcile(Some(1150.5), Some(600.0)).unwrap();
    assert_eq!(r.conflict, Some((1150.5, 600.0)));
    assert_eq!(
        reconcile(None, Some(5.0)).unwrap().field.source,
        Source::Catalogue
    );
    assert_eq!(reconcile(None, None), None);
    // 1 % of 1800 is 18 mm
    assert!(!disagree(1800.0, 1815.0));
    assert!(disagree(1800.0, 1820.0));
}

#[test]
fn unreadable_source_reads_as_typed() {
    let mut props = Properties::new();
    props.insert("encl.width_mm".into(), 600.0.into());
    assert_eq!(
        EnclosureSpec::read(&props).width.unwrap().source,
        Source::Typed
    );
}

#[test]
fn csv_refuses_rows_without_source_or_terms() {
    let (rows, errors) = from_csv(
        "A;S;600;1800;500;1.5;P;;\nB;S;600;1800;500;1.5;P;src;terms\nC;S;x;1;1;1;P;s;t\nD;S\n",
    );
    assert_eq!(rows.len(), 1);
    assert_eq!(
        errors.iter().map(|e| e.0).collect::<Vec<_>>(),
        vec![1, 3, 4]
    );
}
