//! The read-only query layer and saved report presets.

use novo_core::Document;
use novo_reports::query::{self, Presets, QValue, SavedQueries};
use novo_reports::{LiveReports, Report};
use novo_tests::motor_starter;

fn doc() -> Document {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture builds");
    doc
}

fn text_rows(doc: &Document, sql: &str) -> Vec<Vec<String>> {
    let r = query::run(doc.project(), sql).unwrap_or_else(|e| panic!("{}", e.describe(sql)));
    r.rows
        .iter()
        .map(|row| row.iter().map(QValue::display).collect())
        .collect()
}

#[test]
fn select_filter_and_sort_devices() {
    let d = doc();
    let all = text_rows(&d, "SELECT tag FROM devices ORDER BY tag");
    assert_eq!(all.len(), 8);
    let breakers = text_rows(
        &d,
        "SELECT tag, order_number FROM devices WHERE tag LIKE '%F1' OR tag LIKE '%-Q%' ORDER BY tag DESC",
    );
    assert!(!breakers.is_empty() && breakers.len() < 8);
    let mut sorted = breakers.clone();
    sorted.sort_by(|a, b| b[0].cmp(&a[0]));
    assert_eq!(breakers, sorted);
}

#[test]
fn a_query_agrees_with_the_report_of_the_same_name() {
    let d = doc();
    let list = novo_reports::DeviceList.generate(d.project());
    let tags: Vec<String> = list.sections[0]
        .rows
        .iter()
        .map(|r| r.cells[0].to_string())
        .collect();
    let queried: Vec<String> = text_rows(&d, "SELECT tag FROM devices")
        .into_iter()
        .map(|r| r[0].clone())
        .collect();
    assert_eq!(tags, queried);
    let wires = novo_reports::WireList.generate(d.project());
    // The wire list leaves out feeds; the workshop view of the table keeps them.
    let n = text_rows(&d, "SELECT count(*) FROM wires")[0][0]
        .parse::<usize>()
        .unwrap();
    assert!(n >= wires.sections[0].rows.len());
}

#[test]
fn totals_group_and_having() {
    let d = doc();
    let by_kind = text_rows(
        &d,
        "SELECT kind, count(*) AS n FROM devices GROUP BY kind ORDER BY n DESC, kind",
    );
    let total: usize = by_kind.iter().map(|r| r[1].parse::<usize>().unwrap()).sum();
    assert_eq!(total, 8);
    let only = text_rows(
        &d,
        "SELECT kind, count(*) AS n FROM devices GROUP BY kind HAVING count(*) > 1 ORDER BY count(*) DESC",
    );
    assert!(only.iter().all(|r| r[1].parse::<usize>().unwrap() > 1));
    // A total over no rows is still one row.
    assert_eq!(
        text_rows(
            &d,
            "SELECT count(*), sum(parts) FROM devices WHERE tag = 'nothing'"
        ),
        vec![vec!["0".to_owned(), String::new()]]
    );
    let strips = text_rows(
        &d,
        "SELECT strip, count(*), group_concat(name, '/') FROM terminals GROUP BY strip",
    );
    assert_eq!(strips.len(), 1);
    assert_eq!(strips[0][1], "4");
    assert_eq!(strips[0][2].split('/').count(), 4);
}

#[test]
fn empty_values_are_null() {
    let d = doc();
    let unsized_wires = text_rows(
        &d,
        "SELECT number FROM wires WHERE cross_section_mm2 IS NULL",
    );
    let sized = text_rows(
        &d,
        "SELECT number FROM wires WHERE cross_section_mm2 IS NOT NULL",
    );
    assert_eq!(unsized_wires.len() + sized.len(), 25);
    // A comparison with an empty value is neither true nor false.
    let neither = text_rows(
        &d,
        "SELECT number FROM wires WHERE cross_section_mm2 > 0 OR cross_section_mm2 <= 0",
    );
    assert_eq!(neither.len(), sized.len());
}

#[test]
fn distinct_functions_and_arithmetic() {
    let d = doc();
    let makers = text_rows(
        &d,
        "SELECT DISTINCT upper(manufacturer) FROM parts ORDER BY 1",
    );
    let mut seen = makers.clone();
    seen.dedup();
    assert_eq!(makers, seen);
    let cable = text_rows(
        &d,
        "SELECT tag, length_m * 2, round(length_m / 7, 1), coalesce(order_number, 'none') || '!' FROM cables",
    );
    assert_eq!(cable[0][1], "24");
    assert_eq!(cable[0][2], "1.7");
    assert!(cable[0][3].ends_with('!'));
    assert_eq!(
        text_rows(&d, "SELECT 7 / 2, 8 / 2, 1 / 0 FROM cables"),
        vec![vec!["3.5", "4", ""]]
    );
}

#[test]
fn rows_keep_their_model_objects_for_jumping() {
    let d = doc();
    let r = query::run(d.project(), "SELECT tag FROM devices WHERE tag LIKE '%F1'").unwrap();
    assert_eq!(r.sources.len(), r.rows.len());
    assert!(matches!(r.sources[0][0], novo_core::AnyId::Device(_)));
    let grouped = query::run(
        d.project(),
        "SELECT strip, count(*) FROM terminals GROUP BY strip",
    )
    .unwrap();
    assert_eq!(grouped.sources[0].len(), 4);
}

#[test]
fn errors_name_the_problem_and_suggest_a_fix() {
    let d = doc();
    let sql = "SELECT tga FROM devices";
    let e = query::run(d.project(), sql).unwrap_err();
    assert!(e.message.contains("Did you mean `tag`"), "{e}");
    assert_eq!(&sql[e.at..e.at + 3], "tga");
    let e = query::run(d.project(), "SELECT * FROM device").unwrap_err();
    assert!(e.message.contains("Did you mean `devices`"), "{e}");
    let e = query::run(d.project(), "SELECT * FROM devices WHERE count(*) > 1").unwrap_err();
    assert!(e.message.contains("HAVING"), "{e}");
    let e = query::run(d.project(), "SELECT frob(tag) FROM devices").unwrap_err();
    assert!(e.message.contains("no function"), "{e}");
    let e = query::run(d.project(), "UPDATE devices SET tag = 'x'").unwrap_err();
    assert!(e.message.contains("only read"), "{e}");
    assert!(query::check(d.project(), "SELECT tag FROM devices").is_ok());
}

#[test]
fn a_query_never_changes_the_project() {
    let d = doc();
    let before = d.project().revision();
    for sql in [
        "SELECT * FROM devices",
        "SELECT * FROM wires",
        "SELECT * FROM terminals",
    ] {
        query::run(d.project(), sql).unwrap();
    }
    assert_eq!(d.project().revision(), before);
}

#[test]
fn presets_are_saved_in_the_project_and_print_as_a_report() {
    let mut d = doc();
    assert!(Presets::of(d.project()).list.is_empty());
    let mut presets = Presets::default();
    presets
        .upsert(
            "Parts to order",
            "SELECT order_number, devices FROM parts WHERE devices > 0",
        )
        .unwrap();
    presets
        .upsert("Broken", "SELECT nope FROM devices")
        .unwrap();
    presets
        .upsert("  parts to ORDER ", "SELECT order_number FROM parts")
        .unwrap();
    assert_eq!(presets.list.len(), 2, "same name (case aside) replaces");
    assert!(presets.upsert("   ", "SELECT 1 FROM parts").is_err());
    d.transact("Save report presets", |tx| presets.save(tx))
        .unwrap();

    let loaded = Presets::of(d.project());
    assert_eq!(loaded, presets);
    // Undo takes the presets away again with the rest of the edit.
    let out = SavedQueries.generate(d.project());
    assert_eq!(out.sections.len(), 2);
    assert_eq!(out.sections[0].title, "parts to ORDER");
    assert_eq!(out.sections[0].rows.len(), 10);
    assert!(out.sections[1].title.contains("does not run"));
    assert!(
        out.sections[1].rows[0].cells[0]
            .to_string()
            .contains("no column")
    );

    // The standard list carries it, so lists, exports and pages see it.
    let mut live = LiveReports::standard();
    assert!(live.ids().contains(&"queries"));
    assert_eq!(live.get(d.project(), "queries").unwrap().sections.len(), 2);

    // Removing the last preset removes the stored block.
    let mut empty = loaded;
    assert!(empty.remove("BROKEN"));
    assert!(empty.remove("parts to order"));
    d.transact("Clear", |tx| empty.save(tx)).unwrap();
    assert!(
        d.project()
            .info()
            .addon_data
            .get("reports", "presets")
            .is_none()
    );

    d.undo().unwrap();
    assert_eq!(Presets::of(d.project()).list.len(), 2);
}

#[test]
fn a_report_changes_when_the_project_changes() {
    let mut d = doc();
    let mut presets = Presets::default();
    presets
        .upsert("Count", "SELECT count(*) AS n FROM devices")
        .unwrap();
    d.transact("Save", |tx| presets.save(tx)).unwrap();
    let mut live = LiveReports::standard();
    let n = |live: &mut LiveReports, d: &Document| {
        live.get(d.project(), "queries").unwrap().sections[0].rows[0].cells[0].to_string()
    };
    assert_eq!(n(&mut live, &d), "8");
    let (_, changes) = d
        .transact("More", |tx| {
            tx.add_device("=A1+CAB-K99".parse().unwrap(), "Extra")
        })
        .unwrap();
    live.notify(d.project(), &changes);
    assert_eq!(n(&mut live, &d), "9");
}

#[test]
fn the_builder_makes_text_that_runs() {
    let d = doc();
    let mut b = query::Builder::new("wires");
    b.columns = vec!["number".into(), "from".into(), "to".into()];
    b.filters = vec![query::Filter {
        column: "from".into(),
        op: query::FilterOp::Contains,
        value: "F1".into(),
    }];
    b.sort = vec![("from".into(), false)];
    let r = query::run(d.project(), &b.to_sql()).unwrap();
    assert_eq!(r.columns.len(), 3);
    assert!(!r.rows.is_empty());
    assert!(r.rows.iter().all(|row| row[1].display().contains("F1")));
}

#[test]
fn every_example_runs_on_a_real_project() {
    let d = doc();
    for (name, sql) in query::EXAMPLES {
        query::run(d.project(), sql).unwrap_or_else(|e| panic!("{name}: {}", e.describe(sql)));
    }
}

#[test]
fn schema_lists_every_table_with_columns() {
    let d = doc();
    let schema = query::schema(d.project());
    assert_eq!(schema.len(), query::TABLES.len());
    assert!(
        schema
            .iter()
            .all(|t| !t.columns.is_empty() && t.rows.is_empty())
    );
}

#[test]
fn a_saved_query_that_would_overflow_the_stack_prints_a_problem() {
    let mut d = doc();
    // Written straight into the stored block, as a hostile file would be.
    let deep = format!(
        "SELECT {}1{} FROM devices",
        "(".repeat(100_000),
        ")".repeat(100_000)
    );
    let chain = format!("SELECT 1{} FROM devices", "+1".repeat(500_000));
    let text = serde_json::json!({"presets": [
        {"name": "Deep", "sql": deep},
        {"name": "Chain", "sql": chain},
        {"name": "Fine", "sql": "SELECT count(*) FROM devices"},
    ]})
    .to_string();
    d.transact("Load", |tx| {
        tx.set_addon_block(
            "reports",
            "presets",
            novo_core::AddonBlock { format: 1, text },
        )
    })
    .unwrap();
    let out = SavedQueries.generate(d.project());
    assert_eq!(out.sections.len(), 3);
    assert!(out.sections[0].title.contains("does not run"));
    assert!(out.sections[1].title.contains("does not run"));
    assert_eq!(out.sections[2].rows[0].cells[0].to_string(), "8");
    let mut presets = Presets::default();
    assert!(presets.upsert("Long", &"x".repeat(20_000)).is_err());
}

#[test]
fn integer_edges_do_not_panic() {
    let d = doc();
    for sql in [
        "SELECT (-9223372036854775807 - 1) / -1 FROM devices",
        "SELECT (-9223372036854775807 - 1) * -1 FROM devices",
        "SELECT -(-9223372036854775807 - 1) FROM devices",
        "SELECT abs(-9223372036854775807 - 1) FROM devices",
        "SELECT 9223372036854775807 + 1 FROM devices",
        "SELECT -9223372036854775807 - 2 FROM devices",
        "SELECT 9223372036854775807 * 9223372036854775807 FROM devices",
        "SELECT 9223372036854775808 / 0 FROM devices",
        "SELECT (-9223372036854775807 - 1) / 1 FROM devices",
        "SELECT substr(tag, 99999999999999999999, 5), round(1e1, 99) FROM devices",
        "SELECT sum(9223372036854775807) FROM devices",
    ] {
        // Some are errors (1e1 is not a number in this language); none panic.
        let _ = query::run(d.project(), sql);
    }
    let r = text_rows(
        &d,
        "SELECT (-9223372036854775807 - 1) / -1 FROM devices LIMIT 1",
    );
    assert_eq!(r[0][0], "9223372036854775808");
    let r = text_rows(&d, "SELECT 9223372036854775807 + 1 FROM devices LIMIT 1");
    assert_eq!(r[0][0], "9223372036854775808");
}
