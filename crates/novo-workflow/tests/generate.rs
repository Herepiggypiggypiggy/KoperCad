//! Generating pages from a spec sheet, and writing the sheet back.

use std::collections::BTreeMap;

use novo_core::*;
use novo_workflow::generate::*;
use novo_workflow::options::{Assignment, OptionDef, define_option};
use novo_workflow::parametric::*;

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).unwrap()
}

fn pins() -> Vec<Pin> {
    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 10.0)]
}

fn starter() -> BlockDefinition {
    let mut doc = Document::default();
    let (mut def, _) = doc
        .transact("Draw", |tx| {
            let page = tx.add_page("1")?;
            tx.update::<Page>(page, |p| p.structure = tag("=A1"))?;
            let q = tx.add_device(tag("=A1-Q1"), "Breaker {rating} A")?;
            let m = tx.add_device(tag("=A1-M1"), "Motor {power} kW")?;
            let sym = |tx: &mut Tx<'_>, x: f64, d: DeviceId| {
                tx.place_symbol(
                    page,
                    "x",
                    Placement::at(Point::new(x, 0.0)),
                    pins(),
                    SymbolRole::Device {
                        device: d,
                        main: true,
                    },
                )
            };
            let (q1, m1) = (sym(tx, 10.0, q)?, sym(tx, 40.0, m)?);
            tx.connect(Endpoint::pin(q1, 1), Endpoint::pin(m1, 0))?;
            // A junction left of the symbols pulls the block's padded corner to a fractional x, so a
            // block that is not snapped lands off the grid.
            let j = tx.add_junction(page, Point::new(-3.5, 0.0))?;
            let id = tx.define_block(
                "starter",
                &[q1.into(), m1.into(), j.into()],
                Point::new(10.0, 0.0),
                Vec::new(),
            )?;
            Ok(tx.project().block_definition(id).unwrap().clone())
        })
        .unwrap();
    set_param(
        &mut def,
        "power",
        Value::Float(4.0),
        ParamSpec {
            kind: ParamKind::Number,
            min: Some(0.09),
            max: Some(75.0),
            ..ParamSpec::default()
        },
    );
    set_param(
        &mut def,
        "rating",
        Value::Int(0),
        ParamSpec {
            derive: Some("next_size(power * 1000 / (sqrt(3) * 400 * 0.85) * 1.25, 6, 10, 16, 20, 25, 32, 40, 50, 63, 80, 100)".into()),
            ..ParamSpec::default()
        },
    );
    def
}

fn lookup(name: &str) -> Option<BlockDefinition> {
    name.eq_ignore_ascii_case("starter").then(starter)
}

fn empty() -> Document {
    Document::default()
}

fn assignment() -> Assignment {
    BTreeMap::new()
}

#[test]
fn csv_round_trip_with_quotes_and_semicolons() {
    let s = SpecSheet::from_csv("Macro;Page;power\nstarter;\"Pumps, main\";7,5\n");
    assert_eq!(s.headers, vec!["Macro", "Page", "power"]);
    assert_eq!(s.rows[0][1], "Pumps, main");
    let back = SpecSheet::from_csv(&s.to_csv());
    assert_eq!(back, s);
}

#[test]
fn rows_become_placements_with_calculated_values() {
    let sheet = SpecSheet::from_csv(
        "Macro,Page,Structure,power\nstarter,Pumps,=A1,7.5\nstarter,Pumps,,15\nstarter,Pumps,,\n",
    );
    let mut doc = empty();
    let (result, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    let g = result.unwrap();
    assert_eq!(g.pages.len(), 1);
    assert_eq!(g.placements.len(), 3);
    let page = doc.project().page(g.pages[0]).unwrap();
    assert_eq!(page.title, "Pumps");
    assert_eq!(page.structure.to_string(), "=A1");
    let mut descriptions: Vec<String> = doc
        .project()
        .devices()
        .map(|d| d.description.clone())
        .filter(|d| d.starts_with("Breaker"))
        .collect();
    descriptions.sort();
    // 7.5 kW -> 16 A, 15 kW -> 32 A, the default 4 kW -> 10 A.
    assert_eq!(
        descriptions,
        vec!["Breaker 10 A", "Breaker 16 A", "Breaker 32 A"]
    );
    let mut tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    tags.sort();
    tags.dedup();
    assert_eq!(tags.len(), 6);
    // Nothing overlaps: every symbol origin is distinct and inside the page.
    let origins: std::collections::BTreeSet<(i64, i64)> = doc
        .project()
        .symbols()
        .map(|s| (s.placement.origin.x as i64, s.placement.origin.y as i64))
        .collect();
    assert_eq!(origins.len(), 6);
}

#[test]
fn generated_blocks_sit_on_the_snap_grid() {
    // The page flow works from a padded bounding box that is off the grid (the fixture's junction
    // makes it fractional); the block's symbols must still sit a whole number of grid steps from
    // their template position, so the pins line up with what the designer places next.
    let sheet = SpecSheet::from_csv("Macro,Page\nstarter,A\nstarter,A\nstarter,B\n");
    let mut doc = empty();
    let (result, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    result.unwrap();
    let grid = novo_symbols::GRID;
    let on_grid = |v: f64| ((v / grid) - (v / grid).round()).abs() < 1e-9;
    let mut seen = 0;
    for s in doc.project().symbols() {
        let SymbolRole::Device { device, .. } = &s.role else {
            continue;
        };
        // Offsets from the block anchor (x 10) in `starter`: the breaker at 0, the motor at 30.
        let template_x = match doc.project().device(*device).unwrap().tag.product[0]
            .chars()
            .next()
        {
            Some('Q') => 0.0,
            Some('M') => 30.0,
            other => panic!("unexpected device {other:?}"),
        };
        let o = s.placement.origin;
        assert!(
            on_grid(o.x - template_x),
            "block shifted by a fraction of a grid step: x {}",
            o.x
        );
        assert!(
            on_grid(o.y),
            "block shifted by a fraction of a grid step: y {}",
            o.y
        );
        seen += 1;
    }
    assert_eq!(seen, 6);
}

#[test]
fn a_long_list_flows_onto_more_pages() {
    let mut csv = String::from("Macro,Page,power\n");
    for i in 0..60 {
        csv.push_str(&format!("starter,Motors,{}\n", 1 + i % 10));
    }
    let sheet = SpecSheet::from_csv(&csv);
    let mut doc = empty();
    let (result, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    let g = result.unwrap();
    assert!(g.pages.len() >= 2, "{} pages", g.pages.len());
    let titles: Vec<String> = g
        .pages
        .iter()
        .map(|p| doc.project().page(*p).unwrap().title.clone())
        .collect();
    assert_eq!(titles[0], "Motors");
    assert_eq!(titles[1], "Motors (2)");
    // Every symbol sits inside its page.
    for s in doc.project().symbols() {
        let (w, h) = doc.project().page(s.page).unwrap().format.size_mm();
        assert!(s.placement.origin.x > 0.0 && s.placement.origin.x < w);
        assert!(s.placement.origin.y > 0.0 && s.placement.origin.y < h);
    }
    assert_eq!(doc.project().devices().count(), 120);
}

#[test]
fn problems_are_listed_by_line_and_nothing_is_generated() {
    let sheet = SpecSheet::from_csv(
        "Macro,power,Powr\nstarter,7.5,1\nnope,3,\nstarter,500,\nstarter,abc,\n",
    );
    let mut doc = empty();
    let (result, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    let problems = result.unwrap_err();
    let lines: Vec<usize> = problems.iter().map(|p| p.line).collect();
    assert_eq!(lines, vec![3, 4, 5]);
    assert!(problems[0].message.contains("not in the library"));
    assert!(problems[1].message.contains("above the maximum"));
    assert!(problems[2].message.contains("not a number"));
    assert_eq!(doc.project().pages().count(), 0);
    assert_eq!(doc.project().devices().count(), 0);
}

#[test]
fn unknown_columns_are_warned_about() {
    let sheet = SpecSheet::from_csv("Macro,power,Powr\nstarter,7.5,1\n");
    let g = preview(empty().project(), &sheet, &lookup, &assignment(), Mode::Add).unwrap();
    assert_eq!(g.warnings.len(), 1);
    assert!(g.warnings[0].contains("Powr"));
}

#[test]
fn missing_macro_column_is_one_clear_problem() {
    let sheet = SpecSheet::from_csv("Page,power\nPumps,1\n");
    let err = preview(empty().project(), &sheet, &lookup, &assignment(), Mode::Add).unwrap_err();
    assert_eq!(err.len(), 1);
    assert!(err[0].message.contains("Macro"));
}

#[test]
fn when_column_follows_the_project_options() {
    let mut doc = empty();
    doc.transact("Opt", |tx| {
        define_option(tx, OptionDef::yes_no("conveyor", "Conveyor", false))
    })
    .unwrap();
    let sheet = SpecSheet::from_csv(
        "Macro,Page,When,power\nstarter,Main,,5\nstarter,Conveyor,opt.conveyor,2\nstarter,Conveyor,opt.conveyor,3\n",
    );
    let off = novo_workflow::options::current(doc.project());
    let g = preview(doc.project(), &sheet, &lookup, &off, Mode::Add).unwrap();
    assert_eq!(g.placements.len(), 1);
    assert_eq!(g.skipped, vec![3, 4]);
    assert_eq!(g.pages.len(), 1, "an empty Conveyor page is not made");
    let mut on = off.clone();
    on.insert("conveyor".into(), Value::Bool(true));
    let g = preview(doc.project(), &sheet, &lookup, &on, Mode::Add).unwrap();
    assert_eq!(g.placements.len(), 3);
    assert_eq!(g.pages.len(), 2);
    // A condition naming an option the project lacks is a problem, not a guess.
    let bad = SpecSheet::from_csv("Macro,When\nstarter,opt.ghost\n");
    assert!(preview(doc.project(), &bad, &lookup, &off, Mode::Add).is_err());
}

#[test]
fn preview_leaves_the_project_alone_and_generation_is_one_undo_step() {
    let sheet = SpecSheet::from_csv("Macro,Page,power\nstarter,A,5\nstarter,B,6\n");
    let mut doc = empty();
    preview(doc.project(), &sheet, &lookup, &assignment(), Mode::Add).unwrap();
    assert_eq!(doc.project().pages().count(), 0);
    let (r, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    r.unwrap();
    assert_eq!(doc.project().pages().count(), 2);
    doc.undo().unwrap();
    assert_eq!(doc.project().pages().count(), 0);
}

#[test]
fn the_sheet_comes_back_from_the_project_and_regenerates_it() {
    let sheet = SpecSheet::from_csv(
        "Macro,Page,Structure,power\nstarter,Pumps,=A1,7.5\nstarter,Pumps,=A1,15\nstarter,Fans,=A1,0.75\n",
    );
    let mut doc = empty();
    let (r, _) = doc
        .transact("Generate", |tx| {
            generate(tx, &sheet, &lookup, &assignment(), Mode::Add)
        })
        .unwrap();
    r.unwrap();
    let back = spec_from_project(doc.project());
    assert_eq!(back.headers, vec!["Macro", "Page", "Structure", "power"]);
    assert_eq!(back.rows.len(), 3);
    assert_eq!(back.rows[0][1], "Pumps");
    assert_eq!(back.rows[0][3], "7.5");
    assert_eq!(back.rows[2][3], "0.75");
    assert_eq!(macros_used(&back), vec!["starter".to_string()]);

    // Edit a value in "Excel" and regenerate in place.
    let mut edited = back.clone();
    edited.rows[0][3] = "11".into();
    let (result, _) = doc
        .transact("Regenerate", |tx| {
            generate(tx, &edited, &lookup, &assignment(), Mode::Replace)
        })
        .unwrap();
    let g = result.unwrap();
    assert_eq!(g.replaced_pages, 2);
    assert_eq!(doc.project().pages().count(), 2);
    let ratings: std::collections::BTreeSet<String> = doc
        .project()
        .devices()
        .map(|d| d.description.clone())
        .filter(|d| d.starts_with("Breaker"))
        .collect();
    assert!(ratings.contains("Breaker 25 A"), "{ratings:?}"); // 11 kW
    assert_eq!(doc.project().devices().count(), 6);
    // Tags are reused, not piled up: replacing frees the old ones.
    let mut tags: Vec<String> = doc.project().devices().map(|d| d.tag.to_string()).collect();
    tags.sort();
    assert_eq!(tags.iter().filter(|t| t.contains("-M")).count(), 3);
}

#[test]
fn a_sheet_cannot_ask_for_an_unbounded_number_of_placements() {
    let mut csv = String::from("Macro,power\n");
    for _ in 0..(MAX_PLACEMENTS + 1) {
        csv.push_str("starter,1\n");
    }
    let sheet = SpecSheet::from_csv(&csv);
    let err = preview(empty().project(), &sheet, &lookup, &assignment(), Mode::Add).unwrap_err();
    assert!(err[0].message.contains("limit"));
}
