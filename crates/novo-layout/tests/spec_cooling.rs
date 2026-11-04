use novo_layout::sizing::{Installation, Material, SizingRequest};
use novo_layout::spec::*;
use novo_layout::spec_cooling::*;

fn spec(w: f64, h: f64, d: f64, src: Source) -> EnclosureSpec {
    EnclosureSpec {
        article: "TEST.1".into(),
        width: Some(Field::new(w, src)),
        height: Some(Field::new(h, src)),
        depth: Some(Field::new(d, src)),
        ..EnclosureSpec::default()
    }
}

#[test]
fn changing_the_spec_changes_surface_and_cooling_need() {
    let req = SizingRequest::default();
    let small = spec(600.0, 1000.0, 300.0, Source::Catalogue);
    let big = spec(800.0, 2000.0, 500.0, Source::Catalogue);
    let a = thermal_for(&small, Material::Steel, 300.0, &req).unwrap();
    let b = thermal_for(&big, Material::Steel, 300.0, &req).unwrap();
    assert!(b.thermal.surface > a.thermal.surface);
    assert!(b.thermal.rise < a.thermal.rise);
    assert!(b.thermal.cooling_needed <= a.thermal.cooling_needed);
    // Free standing 1.8 H (W + D) + 1.4 W D, IEC 60890 surface factors.
    let s = surface_m2(&small, Installation::FreeStanding).unwrap();
    assert!((s - (1.8 * 1.0 * 0.9 + 1.4 * 0.6 * 0.3)).abs() < 1e-9);
}

#[test]
fn a_missing_number_is_named_not_defaulted() {
    let mut s = spec(600.0, 1000.0, 300.0, Source::Typed);
    s.depth = None;
    let e = surface_m2(&s, Installation::Wall).unwrap_err();
    assert_eq!(e.to_string(), "The enclosure has no outside depth.");
}

#[test]
fn the_weakest_source_travels_with_the_answer() {
    let mut s = spec(600.0, 1000.0, 300.0, Source::Step);
    s.depth = Some(Field::new(300.0, Source::Default));
    let t = thermal_for(&s, Material::Steel, 100.0, &SizingRequest::default()).unwrap();
    assert_eq!(t.weakest, Source::Default);
    assert_eq!(provenance_line(t.weakest), "Enclosure size: default.");
}

#[test]
fn a_catalogue_size_with_a_typed_depth_still_shows_the_estimate_notice() {
    let mut s = spec(600.0, 1000.0, 300.0, Source::Catalogue);
    s.depth = Some(Field::new(300.0, Source::Typed));
    let lines = size_lines(&s);
    assert_eq!(lines[0], "Enclosure size: typed by you.");
    assert_eq!(lines[1], ESTIMATE_NOTICE);
    // All typed or from the STEP: no notice.
    assert_eq!(
        size_lines(&spec(600.0, 1000.0, 300.0, Source::Step)).len(),
        1
    );
    assert_eq!(
        size_lines(&spec(600.0, 1000.0, 300.0, Source::Typed)).len(),
        1
    );
    // No size: nothing to say here (the missing number is reported instead).
    let mut none = s.clone();
    none.depth = None;
    assert!(size_lines(&none).is_empty());
}
