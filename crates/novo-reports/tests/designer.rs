//! The documentation designer engine (DOC-1): fields, the table builder,
//! rendering and paging, frame pixel diffs, validation and storage.

use novo_core::{Document, PageFormat, Value};
use novo_reports::designer::raster::Raster;
use novo_reports::designer::store::{self, Origin};
use novo_reports::designer::{
    ColumnSpec, DesignProblem, Element, Fields, Filter, FilterOp, SortKey, TableElement,
    TableError, TableSpec, Template, TemplateKind, build_table, builtin, expand, preview_data,
    render, validate,
};
use novo_reports::{Cell, Column, ReportOutput, Section, generate_all};
use novo_tests::motor_starter;

fn devices_like() -> ReportOutput {
    let mut s = Section::new(
        "all",
        "",
        vec![
            Column::new("tag", "Tag", 10.0),
            Column::new("kind", "Kind", 10.0),
            Column::new("qty", "Qty", 5.0).right(),
        ],
    );
    for (tag, kind, qty) in [
        ("-K10", "relay", 2),
        ("-K2", "relay", 1),
        ("-Q1", "breaker", 4),
    ] {
        s.push(vec![tag.into(), kind.into(), Cell::Int(qty)], Vec::new());
    }
    ReportOutput {
        report: "devices".into(),
        title: "Devices".into(),
        revision: 0,
        sections: vec![s],
    }
}

#[test]
fn fields_expand_known_unknown_and_escaped_braces() {
    let mut f = Fields::new();
    f.set("a", "1");
    f.set("empty", "");
    let (text, unknown) = expand("{a}-{ b }-{empty}|{{lit}}|{nope}|{nope}|{open", &f);
    assert_eq!(text, "1-{ b }-|{lit}|{nope}|{nope}|{open");
    assert_eq!(unknown, ["b", "nope"]);
}

#[test]
fn project_fields_come_from_the_project() {
    let mut doc = Document::default();
    doc.transact("props", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert("drawn_by".into(), Value::Text("Ann".into()));
        })
    })
    .expect("set");
    let f = Fields::from_project(doc.project());
    assert_eq!(f.get("drawn_by"), Some("Ann"));
    assert_eq!(f.get("page.name"), Some(""));
}

#[test]
fn table_builder_picks_renames_filters_and_sorts_naturally() {
    let mut spec = TableSpec::new("devices");
    spec.columns = vec![
        ColumnSpec {
            header: Some("Designation".into()),
            width: Some(30.0),
            ..ColumnSpec::new("tag")
        },
        ColumnSpec::new("qty"),
    ];
    spec.filters = vec![Filter {
        column: "kind".into(),
        op: FilterOp::Equals,
        value: "RELAY".into(),
    }];
    spec.sort = vec![SortKey {
        column: "tag".into(),
        descending: false,
    }];
    let (s, errors) = build_table(&spec, &[devices_like()]);
    assert!(errors.is_empty());
    assert_eq!(s.columns[0].header, "Designation");
    assert_eq!(s.columns[0].width, 30.0);
    assert_eq!(s.text_rows(), [["-K2", "1"], ["-K10", "2"]]);

    spec.sort[0].descending = true;
    spec.sort.insert(
        0,
        SortKey {
            column: "qty".into(),
            descending: true,
        },
    );
    let (s, _) = build_table(&spec, &[devices_like()]);
    assert_eq!(s.text_rows(), [["-K10", "2"], ["-K2", "1"]]);
}

#[test]
fn table_builder_lists_what_it_cannot_resolve() {
    let mut spec = TableSpec::new("devices");
    spec.columns = vec![ColumnSpec::new("tag"), ColumnSpec::new("colour")];
    let (s, errors) = build_table(&spec, &[devices_like()]);
    assert_eq!(s.columns.len(), 1);
    assert_eq!(
        errors,
        [TableError::MissingColumn {
            report: "devices".into(),
            column: "colour".into()
        }]
    );
    let (s, errors) = build_table(&TableSpec::new("nope"), &[devices_like()]);
    assert!(s.rows.is_empty());
    assert_eq!(errors, [TableError::MissingReport("nope".into())]);
    spec.section = Some("other".into());
    assert!(matches!(
        build_table(&spec, &[devices_like()]).1[0],
        TableError::MissingSection { .. }
    ));
}

#[test]
fn a_report_template_pages_and_repeats_header_and_title_block() {
    let template = builtin::report_a4("Device list", "devices");
    let data = preview_data(&template, 60);
    let out = render(&template, &data, &Fields::sample());
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    assert!(out.pages.len() >= 3);
    let mut seen = 0;
    for (i, page) in out.pages.iter().enumerate() {
        let texts: Vec<&str> = page.texts.iter().map(|t| t.text.as_str()).collect();
        assert!(texts.contains(&"Device list"), "heading on page {i}");
        assert!(texts.contains(&"Sample project"), "title block on page {i}");
        let sheet = format!("{} of {}", i + 1, out.pages.len());
        assert!(texts.contains(&sheet.as_str()), "sheet number on page {i}");
        assert!(texts.contains(&"Tag"), "header repeats on page {i}");
        seen += texts.iter().filter(|t| t.starts_with("tag-")).count();
    }
    assert_eq!(seen, 60, "every sample row is printed exactly once");
}

#[test]
fn a_template_renders_live_report_data() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let data = generate_all(doc.project());
    let template = builtin::report_a4("Device list", "devices");
    let out = render(&template, &data, &Fields::from_project(doc.project()));
    assert!(out.problems.is_empty(), "{:?}", out.problems);
    let devices = data.iter().find(|o| o.report == "devices").expect("report");
    let first = devices.sections[0].rows[0].cells[0].to_string();
    assert!(out.pages[0].texts.iter().any(|t| t.text == first));
}

#[test]
fn only_every_page_elements_repeat() {
    let mut t = builtin::report_a4("L", "devices");
    t.elements
        .push(Element::text(15.0, 15.0, 10.0, 4.0, "first", 2.0));
    let mut each = Element::text(30.0, 15.0, 10.0, 4.0, "each", 2.0);
    if let Element::Text { every_page, .. } = &mut each {
        *every_page = true;
    }
    t.elements.push(each);
    let out = render(&t, &preview_data(&t, 60), &Fields::sample());
    let count = |p: usize, s: &str| out.pages[p].texts.iter().filter(|t| t.text == s).count();
    assert_eq!((count(0, "first"), count(1, "first")), (1, 0));
    assert_eq!((count(0, "each"), count(1, "each")), (1, 1));
}

/// The frame of an A4 landscape sheet, drawn independently of the engine:
/// sheet edge, heavy frame line at 10 mm, column ticks in the margins.
fn expected_frame(columns: u16, margin: usize) -> Raster {
    let (w, h) = (297usize, 210usize);
    let mut px: Vec<(usize, usize)> = Vec::new();
    for x in 0..=w {
        px.extend([(x, 0), (x, h)]);
    }
    for y in 0..=h {
        px.extend([(0, y), (w, y)]);
    }
    let (x0, y0, x1, y1) = (margin, margin, w - margin, h - margin);
    for x in x0..=x1 {
        px.extend([(x, y0), (x, y0 + 1), (x, y1), (x, y1 + 1)]);
    }
    for y in y0..=y1 {
        px.extend([(x0, y), (x0 + 1, y), (x1, y), (x1 + 1, y)]);
    }
    let col_w = w as f64 / f64::from(columns);
    for c in 1..columns {
        let x = (f64::from(c) * col_w).round() as usize;
        for y in (0..=margin).chain(h - margin..=h) {
            px.push((x, y));
        }
    }
    let rows: Vec<String> = (0..h)
        .map(|y| {
            (0..w)
                .map(|x| if px.contains(&(x, y)) { '#' } else { '.' })
                .collect()
        })
        .collect();
    let refs: Vec<&str> = rows.iter().map(String::as_str).collect();
    Raster::from_rows(&refs)
}

fn frame_only(margin: f64) -> Template {
    let mut t = Template::new("F", TemplateKind::Sheet, Template::a4_landscape());
    t.frame = Some(novo_reports::designer::Frame {
        margin_mm: margin,
        column_numbers: false,
    });
    t
}

#[test]
fn the_frame_matches_an_independent_drawing_pixel_for_pixel() {
    let t = frame_only(10.0);
    let out = render(&t, &[], &Fields::new());
    let got = Raster::from_layout(&out.pages[0], 1.0);
    let want = expected_frame(t.format.columns, 10);
    assert_eq!(
        got.diff(&want),
        Some(0),
        "\n{}",
        got.to_ascii()
            .lines()
            .take(14)
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn a_moved_frame_shows_in_the_pixel_diff() {
    let out = render(&frame_only(12.0), &[], &Fields::new());
    let got = Raster::from_layout(&out.pages[0], 1.0);
    let want = expected_frame(10, 10);
    assert!(got.diff(&want).expect("same size") > 500);
    let small = Raster::blank(10, 10);
    assert_eq!(got.diff(&small), None);
}

#[test]
fn the_title_block_sits_in_the_frame_corner_like_the_canvas() {
    let t = builtin::sheet("S", PageFormat::default());
    // A3 landscape 420 x 297: block 140 x 36 against the 10 mm frame.
    assert_eq!(t.title_block_rect(), Some((270.0, 251.0, 140.0, 36.0)));
    let out = render(&t, &[], &Fields::sample());
    let texts: Vec<&str> = out.pages[0].texts.iter().map(|t| t.text.as_str()).collect();
    assert!(texts.contains(&"Sample project"));
    assert!(texts.contains(&"12  Motor starter"));
    assert!(texts.contains(&"B  2026-10-16"));
    assert!(texts.contains(&"A. Drafter / C. Checker"));
    assert!(texts.contains(&"1 of 1"));
    // Column numbers 1..=10 in both margins.
    assert_eq!(texts.iter().filter(|t| **t == "10").count(), 2);
}

#[test]
fn validate_reports_geometry_and_field_problems() {
    let mut t = builtin::report_a4("L", "devices");
    t.name = " ".into();
    t.elements
        .push(Element::text(15.0, 15.0, 10.0, 4.0, "{project.nmae}", 2.0));
    t.elements
        .push(Element::text(250.0, 190.0, 10.0, 4.0, "in block", 2.0));
    t.elements
        .push(Element::text(290.0, 15.0, 20.0, 4.0, "off sheet", 2.0));
    let Element::Table(first) = t.elements[0].clone() else {
        unreachable!()
    };
    t.elements.push(Element::Table(TableElement {
        height: 5.0,
        ..first
    }));
    let problems = validate(&t, &Fields::sample());
    for p in [
        DesignProblem::EmptyName,
        DesignProblem::UnknownField {
            place: "element 2".into(),
            field: "project.nmae".into(),
        },
        DesignProblem::OverlapsTitleBlock { index: 2 },
        DesignProblem::OutsideSheet { index: 3 },
        DesignProblem::TableTooSmall { index: 4 },
        DesignProblem::TablesOverlap { a: 0, b: 4 },
    ] {
        assert!(problems.contains(&p), "missing {p:?} in {problems:?}");
    }
    // The typo stays visible on the page.
    let out = render(&t, &preview_data(&t, 3), &Fields::sample());
    assert!(
        out.pages[0]
            .texts
            .iter()
            .any(|t| t.text == "{project.nmae}")
    );
}

#[test]
fn every_builtin_template_is_clean_and_renders() {
    for t in builtin::all() {
        let data = preview_data(&t, 5);
        let out = render(&t, &data, &Fields::sample());
        assert!(out.problems.is_empty(), "{}: {:?}", t.name, out.problems);
        assert!(!out.pages.is_empty());
    }
}

#[test]
fn templates_round_trip_through_json_and_the_project_property() {
    let all = builtin::all();
    for t in &all {
        assert_eq!(&store::from_json(&store::to_json(t)).expect("parse"), t);
    }
    assert_eq!(
        store::from_property(&store::to_property(&all)).expect("parse"),
        all
    );
    let mut doc = Document::default();
    doc.transact("templates", |tx| {
        tx.set_info(|i| {
            i.properties.insert(
                store::PROJECT_KEY.into(),
                Value::Text(store::to_property(&all)),
            );
        })
    })
    .expect("set");
    assert_eq!(store::project_templates(doc.project()).expect("read"), all);
    assert!(
        store::project_templates(Document::default().project())
            .expect("none")
            .is_empty()
    );
}

#[test]
fn newer_formats_and_broken_files_are_reported_not_skipped() {
    let mut newer = builtin::cover_form();
    newer.version = 99;
    assert!(matches!(
        store::from_json(&store::to_json(&newer)),
        Err(store::StoreError::TooNew { found: 99 })
    ));
    let dir = tempfile::tempdir().expect("dir");
    store::save_to_dir(dir.path(), &builtin::cover_form()).expect("save");
    store::save_to_dir(dir.path(), &builtin::report_a4("Wire / list", "wires")).expect("save");
    std::fs::write(dir.path().join("broken.kdt.json"), "{").expect("write");
    std::fs::write(dir.path().join("notes.txt"), "ignored").expect("write");
    let (found, bad) = store::load_dir(dir.path());
    assert_eq!(
        found.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(),
        ["Cover sheet", "Wire / list"]
    );
    assert_eq!(bad.len(), 1);
    assert!(bad[0].0.ends_with("broken.kdt.json"));
    assert!(store::load_dir(&dir.path().join("missing")).0.is_empty());
    assert_eq!(store::file_name("Wire / list"), "Wire___list.kdt.json");
}

#[test]
fn project_templates_shadow_user_and_builtin_ones() {
    let mut mine = builtin::report_a4("Parts list", "bom");
    mine.description = "project".into();
    let mut user = builtin::report_a4("Parts list", "bom");
    user.description = "user".into();
    let user_only = builtin::report_a4("Only user", "wires");
    let user = [user, user_only];
    let project = [mine];
    let (origin, t) = store::resolve(&project, &user, "Parts list").expect("found");
    assert_eq!(
        (origin, t.description.as_str()),
        (Origin::Project, "project")
    );
    assert_eq!(
        store::resolve(&[], &user, "Parts list").expect("found").0,
        Origin::User
    );
    assert_eq!(
        store::resolve(&[], &user, "Only user").expect("found").0,
        Origin::User
    );
    assert_eq!(
        store::resolve(&[], &[], "Cover sheet").expect("found").0,
        Origin::Builtin
    );
    assert!(store::resolve(&[], &[], "nothing").is_none());
    let names: Vec<_> = store::all(&project, &user)
        .into_iter()
        .map(|(_, t)| t.name)
        .collect();
    assert_eq!(names.iter().filter(|n| *n == "Parts list").count(), 1);
}

#[test]
fn the_entry_names_where_the_user_finds_the_designer() {
    use novo_reports::designer::ENTRY;
    assert_eq!(
        (ENTRY.workspace, ENTRY.tab, ENTRY.command_id),
        ("Reports", "Designer", "report.designer")
    );
}
