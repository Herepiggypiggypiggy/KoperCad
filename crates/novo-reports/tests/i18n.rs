//! Report and PDF languages (DOC-2): switching changes every report string,
//! missing keys are listed, the project keeps the table.

use novo_core::{Document, Value};
use novo_reports::designer::{Fields, builtin, preview_data, render};
use novo_reports::i18n::{
    self, Catalog, FIXED_SECTION_TITLES, LANGUAGE_KEY, ProjectTable, TABLE_KEY, Translator,
    missing, render_in, strings_of_output, strings_of_template, translate_layout, translate_output,
    translate_template,
};
use novo_reports::{Cell, Column, ReportOutput, Section, generate_all, standard_reports};
use novo_tests::motor_starter;

fn nl() -> Translator {
    Translator::new("nl", &ProjectTable::default())
}

fn motor_reports() -> Vec<ReportOutput> {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    generate_all(doc.project())
}

#[test]
fn shipped_dutch_file_is_clean() {
    let nl = i18n::builtin()
        .into_iter()
        .find(|c| c.code == "nl")
        .unwrap();
    assert_eq!(nl.name, "Nederlands");
    assert!(nl.problems().is_empty(), "{:?}", nl.problems());
    assert!(nl.entries.len() > 100);
}

#[test]
fn switching_language_changes_every_report_string() {
    let tr = nl();
    for output in motor_reports() {
        let dutch = translate_output(&output, &tr);
        let same = tr
            .translate(&output.title)
            .is_none_or(|t| t == output.title);
        assert_eq!(
            dutch.title == output.title,
            same,
            "title of {}",
            output.report
        );
        for (a, b) in output.sections.iter().zip(&dutch.sections) {
            for (ca, cb) in a.columns.iter().zip(&b.columns) {
                // Words that are the same in Dutch (units, Tag, Type) stay.
                let same = tr.translate(&ca.header).is_none_or(|t| t == ca.header);
                assert_eq!(
                    ca.header == cb.header,
                    same,
                    "{} / {}",
                    output.report,
                    ca.header
                );
                assert_eq!(ca.key, cb.key, "column keys never change");
            }
            assert_eq!(a.rows, b.rows, "project data is not translated");
            assert_eq!(a.key, b.key);
        }
    }
}

#[test]
fn english_changes_nothing() {
    for output in motor_reports() {
        assert_eq!(translate_output(&output, &Translator::english()), output);
    }
}

#[test]
fn nothing_is_missing_for_the_standard_reports() {
    let tr = nl();
    for output in motor_reports() {
        let strings = strings_of_output(&output);
        assert!(!strings.is_empty());
        assert_eq!(
            missing(&tr, &strings),
            Vec::<String>::new(),
            "{}",
            output.report
        );
    }
    assert_eq!(standard_reports().len(), motor_reports().len());
    for title in FIXED_SECTION_TITLES {
        assert!(tr.knows(title), "section title {title}");
    }
    for t in builtin::all() {
        let strings = strings_of_template(&t);
        assert_eq!(missing(&tr, &strings), Vec::<String>::new(), "{}", t.name);
    }
}

#[test]
fn missing_keys_are_listed() {
    let mut out = ReportOutput {
        report: "x".into(),
        title: "Brand new report".into(),
        revision: 0,
        sections: vec![Section::new(
            "a",
            "-X1",
            vec![Column::new("q", "Qty", 1.0), Column::new("z", "Zorp", 1.0)],
        )],
    };
    out.sections[0].push(vec![Cell::Int(1), "a".into()], Vec::new());
    let tr = nl();
    assert_eq!(
        missing(&tr, &strings_of_output(&out)),
        vec!["Brand new report".to_owned(), "Zorp".to_owned()]
    );
    // The strip tag is data, not a key; unknown text prints in English.
    let dutch = translate_output(&out, &tr);
    assert_eq!(dutch.title, "Brand new report");
    assert_eq!(dutch.sections[0].title, "-X1");
    assert_eq!(dutch.sections[0].columns[0].header, "Aantal");
    // English misses nothing.
    assert!(missing(&Translator::english(), &strings_of_output(&out)).is_empty());
}

#[test]
fn project_entries_win_and_new_languages_work() {
    let mut table = ProjectTable::default();
    table.set("nl", "Device list", "Componentenlijst").unwrap();
    let tr = Translator::new("nl", &table);
    assert_eq!(tr.tr("Device list"), "Componentenlijst");
    assert_eq!(tr.tr("Wire list"), "Draadlijst", "shipped text still used");

    let mut de = Catalog::new("de", "Deutsch");
    de.set("Wire list", "Drahtliste");
    assert!(table.put_language(de).is_empty());
    let tr = Translator::new("de", &table);
    assert_eq!(tr.tr("Wire list"), "Drahtliste");
    assert_eq!(tr.tr("Device list"), "Device list");
    let langs = i18n::languages(&table);
    let codes: Vec<&str> = langs.iter().map(|l| l.code.as_str()).collect();
    assert_eq!(codes, ["en", "nl", "de"]);
    assert!(!langs[2].shipped);
}

#[test]
fn patterns_fill_in_their_values() {
    let tr = nl();
    assert_eq!(tr.tr("Line 12"), "Regel 12");
    assert_eq!(tr.tr("Table of contents =A1+CAB"), "Inhoudsopgave =A1+CAB");
    assert_eq!(tr.tr("page 2 / 7"), "pagina 2 / 7");
    assert!(tr.translate("Line ").is_none());
}

#[test]
fn placeholder_changes_are_caught() {
    let mut c = Catalog::new("xx", "Test");
    c.set("Revision {revision}", "Revisie");
    c.set("Page", " ");
    c.set("Sheet {page}", "Blad {page}");
    let problems = c.problems();
    assert_eq!(problems.len(), 2);
    assert!(problems.iter().any(|p| p.key == "Revision {revision}"));
    assert!(problems.iter().any(|p| p.key == "Page"));
}

#[test]
fn project_keeps_language_and_table() {
    let mut doc = Document::default();
    let mut table = ProjectTable::default();
    table.set("nl", "Qty", "Aant.").unwrap();
    let text = table.to_property().expect("not empty");
    doc.transact("Languages", |tx| {
        tx.set_info(|i| {
            i.properties
                .insert(LANGUAGE_KEY.into(), Value::Text("nl".into()));
            i.properties
                .insert(TABLE_KEY.into(), Value::Text(text.clone()));
        })
    })
    .expect("set");
    let tr = i18n::translator_of(doc.project());
    assert_eq!(tr.code(), "nl");
    assert_eq!(tr.tr("Qty"), "Aant.");
    assert_eq!(tr.tr("Wire"), "Draad");
    assert_eq!(ProjectTable::of_project(doc.project()), table);

    let empty = Document::default();
    assert!(i18n::translator_of(empty.project()).is_english());
    assert_eq!(ProjectTable::default().to_property(), None);
}

#[test]
fn designer_templates_render_in_the_language() {
    let tr = nl();
    for t in builtin::all() {
        let data = preview_data(&t, 30);
        let fields = Fields::sample();
        let english = render(&t, &data, &fields);
        let same = render_in(&t, &data, &fields, &Translator::english());
        assert_eq!(english, same, "English is the plain render: {}", t.name);

        let dutch = render_in(&t, &data, &fields, &tr);
        assert_eq!(dutch.pages.len(), english.pages.len(), "{}", t.name);
        assert!(dutch.problems.is_empty(), "{:?}", dutch.problems);
        let words = |r: &novo_reports::designer::Rendered| -> Vec<String> {
            r.pages
                .iter()
                .flat_map(|p| p.texts.iter().map(|x| x.text.clone()))
                .collect()
        };
        if t.title_block.is_some() {
            let d = words(&dutch);
            assert!(d.iter().any(|s| s == "Getekend/gecontr."), "{}", t.name);
            assert!(
                d.iter()
                    .any(|s| s == "1 van 1" || s.ends_with(" van 2") || s.contains(" van "))
            );
            assert!(!d.iter().any(|s| s == "Drawn / checked"));
        }
        // Geometry and line work are identical.
        for (a, b) in english.pages.iter().zip(&dutch.pages) {
            assert_eq!(a.lines, b.lines);
            assert_eq!(a.texts.len(), b.texts.len());
        }
    }
}

#[test]
fn translated_template_keeps_fields_and_name() {
    let t = builtin::cover_form();
    let d = translate_template(&t, &nl());
    assert_eq!(d.name, t.name);
    let revision = d
        .elements
        .iter()
        .find_map(|e| match e {
            novo_reports::designer::Element::Text { text, .. } if text.starts_with("Revisie") => {
                Some(text.clone())
            }
            _ => None,
        })
        .expect("revision line");
    assert_eq!(revision, "Revisie {revision}  {revision_date}");
}

#[test]
fn explicit_template_key_wins() {
    let mut t = builtin::cover_form();
    if let Some(novo_reports::designer::Element::Text { key, text, .. }) = t.elements.get_mut(1) {
        *text = "Free words".into();
        *key = Some("Cover sheet".into());
    }
    let d = translate_template(&t, &nl());
    let found = d.elements.iter().any(
        |e| matches!(e, novo_reports::designer::Element::Text { text, .. } if text == "Voorblad"),
    );
    assert!(found);
}

#[test]
fn pager_words_follow_the_language() {
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    let toc = generate_all(doc.project()).remove(0);
    let setup = novo_reports::PageSetup::default();
    let pages = novo_reports::paginate(&toc, &setup);
    let layout = novo_reports::layout_page(&toc, &pages[0], &setup);
    let dutch = translate_layout(&layout, &nl());
    assert!(
        layout.texts.iter().any(|t| t.text.starts_with("page 1 / ")),
        "English pager"
    );
    assert!(
        dutch
            .texts
            .iter()
            .any(|t| t.text.starts_with("pagina 1 / "))
    );
    assert!(!dutch.texts.iter().any(|t| t.text.starts_with("page ")));
}

#[test]
fn dutch_title_block_captions_fit_their_column() {
    // The caption column is 24 mm with 2 mm text: about 20 characters.
    let tr = nl();
    for t in builtin::all() {
        for row in t.title_block.iter().flat_map(|b| &b.rows) {
            let caption = tr.tr(&row.caption);
            assert!(caption.chars().count() <= 20, "{caption}");
        }
    }
}

#[test]
fn typed_translations_cannot_drop_or_rename_placeholders() {
    let mut table = ProjectTable::default();
    assert!(table.set("nl", "Line {n}", "Regel").is_err());
    assert!(table.set("nl", "Line {n}", "Regel {x}").is_err());
    assert!(table.set("nl", "Wire", "Draad {n}").is_err());
    assert!(table.set("nl", "Wire", "  ").is_err());
    assert_eq!(table.to_property(), None, "nothing was stored");
    assert!(table.set("nl", "Line {n}", "Rij {n}").is_ok());
    assert!(
        table
            .set("nl", "page {page} / {pages}", "{pages} / {page}")
            .is_ok()
    );
}

#[test]
fn a_loaded_language_file_loses_its_bad_entries() {
    let mut file = Catalog::new("de", "Deutsch");
    file.set("Wire", "Draht");
    file.set("Line {n}", "Zeile");
    let mut table = ProjectTable::default();
    let bad = table.put_language(file);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].key, "Line {n}");
    let tr = Translator::new("de", &table);
    assert_eq!(tr.tr("Wire"), "Draht");
    assert_eq!(tr.tr("Line 4"), "Line 4");
}

#[test]
fn a_captured_value_that_looks_like_a_placeholder_is_kept() {
    let mut table = ProjectTable::default();
    table.set("nl", "Line {a} {b}", "{b} {a}").unwrap();
    let tr = Translator::new("nl", &table);
    assert_eq!(tr.tr("Line {b} x"), "x {b}");
}

#[test]
fn a_language_file_round_trips() {
    let mut table = ProjectTable::default();
    table.set("nl", "Wire", "Ader").unwrap();
    let mine = Translator::new("nl", &table);
    // What the user exports is the effective language: shipped plus own.
    let mut file = i18n::builtin().remove(0);
    file.set("Wire", "Ader");
    let json = file.to_json();
    let back = Catalog::from_json(&json).unwrap();
    assert_eq!(back, file);
    let mut other = ProjectTable::default();
    assert!(other.put_language(back).is_empty());
    assert_eq!(Translator::new("nl", &other).tr("Wire"), "Ader");
    assert_eq!(mine.tr("Wire"), "Ader");
}

#[test]
fn a_bad_language_file_is_refused_with_a_reason() {
    assert!(Catalog::from_json("not json").is_err());
    assert!(matches!(
        Catalog::from_json(r#"{"code":"  ","entries":{}}"#),
        Err(i18n::LanguageFileError::NoCode)
    ));
    let c =
        Catalog::from_json(r#"{"code":"DE","name":"Deutsch","entries":{"Wire":"Draht"}}"#).unwrap();
    assert_eq!(c.code, "de");
}

#[test]
fn merging_a_language_file_keeps_the_projects_own_entries() {
    let mut table = ProjectTable::default();
    table.set("nl", "Wire", "Ader").unwrap();
    let mut file = Catalog::new("nl", "Nederlands");
    file.set("Wire", "Draad");
    file.set("Cable", "Kabel");
    file.set("Line {n}", "Regel");
    let bad = table.merge_language(file);
    assert_eq!(bad.len(), 1);
    let tr = Translator::new("nl", &table);
    assert_eq!(tr.tr("Wire"), "Ader", "the project's entry wins");
    assert_eq!(tr.tr("Cable"), "Kabel", "the file fills in the rest");
    assert_eq!(table.language("nl").unwrap().name, "Nederlands");
}

#[test]
fn the_bench_cut_list_summary_reads_in_dutch() {
    use novo_reports::{BenchCut, Report};
    let mut doc = Document::default();
    motor_starter(&mut doc).expect("fixture");
    // The fixture has no cabinet, so every length is an estimate.
    let output = BenchCut.generate(doc.project());
    let tr = nl();
    let summary = &output.sections[0];
    assert!(
        summary.title.starts_with("What to pull · "),
        "{}",
        summary.title
    );
    let dutch = translate_output(&output, &tr);
    let title = &dutch.sections[0].title;
    assert!(title.starts_with("Te trekken · "), "{title}");
    assert!(title.contains("lengtes zijn schattingen"), "{title}");
    assert!(!title.contains("estimates"), "{title}");
    // The numbers are filled in, and every header and the title are known.
    assert!(!title.contains('{'), "{title}");
    assert_eq!(dutch.sections[0].columns[7].header, "Geschat");
    assert_eq!(
        missing(&tr, &strings_of_output(&output)),
        Vec::<String>::new()
    );
}

#[test]
fn every_fixed_cell_word_has_a_dutch_entry() {
    let tr = nl();
    for text in i18n::FIXED_CELL_TEXTS
        .iter()
        .chain(i18n::FIXED_CELL_PATTERNS)
        .filter(|t| **t != "No")
    {
        assert!(tr.lookup(text).is_some(), "no Dutch entry for {text:?}");
    }
    assert!(tr.lookup("No (answer)").is_some());
}

fn measured_report() -> ReportOutput {
    let mut section = Section::new(
        "s",
        "",
        vec![
            Column::new("what", "Description", 20.0),
            Column::new("len", "Length", 8.0).right(),
            Column::new("ok", "Check", 8.0),
            Column::new("no", "No", 4.0),
        ],
    );
    section.push(
        vec![
            Cell::text("Wire 1"),
            Cell::Number(1.5),
            Cell::text("overfull"),
            Cell::Int(3),
        ],
        vec![],
    );
    section.push(
        vec![
            Cell::text("overfull"),
            Cell::Number(12.0),
            Cell::text("3 overlaps"),
            Cell::text("No"),
        ],
        vec![],
    );
    ReportOutput {
        report: "t".into(),
        title: "Total".into(),
        revision: 0,
        sections: vec![section],
    }
}

#[test]
fn dutch_prints_measured_values_with_a_comma() {
    use novo_reports::{CsvOptions, to_csv};
    let dutch = translate_output(&measured_report(), &nl());
    let s = &dutch.sections[0];
    assert_eq!(s.decimal, ',');
    assert_eq!(s.cell_text(&s.rows[0].cells[1]), "1,5");
    assert_eq!(s.cell_text(&s.rows[1].cells[1]), "12");
    // Spreadsheets keep real numbers.
    assert_eq!(s.rows[0].cells[1], Cell::Number(1.5));
    // CSV with a comma delimiter quotes the value.
    let csv = to_csv(&dutch, &CsvOptions::default());
    assert!(csv.contains("\"1,5\""), "{csv}");
    // English and a language without a `decimal` keep the point.
    let english = translate_output(&measured_report(), &Translator::english());
    assert_eq!(english.sections[0].decimal, '.');
    let mut table = ProjectTable::default();
    table.put_language(Catalog::new("de", "Deutsch"));
    let de = Translator::new("de", &table);
    assert_eq!(de.decimal(), '.');
}

#[test]
fn fixed_cell_words_follow_the_language_and_data_does_not() {
    let dutch = translate_output(&measured_report(), &nl());
    let rows = dutch.sections[0].text_rows();
    // A fixed word in a status column is translated, the same word as a
    // description (project data position) too only when it is the whole cell.
    assert_eq!(rows[0][2], "overvol");
    assert_eq!(rows[1][2], "3 overlappingen");
    // The answer `No` is `Nee`; the column header `No` stays the number.
    assert_eq!(rows[1][3], "Nee");
    assert_eq!(dutch.sections[0].columns[3].header, "Nr");
    // Names that merely contain a fixed word are untouched.
    assert_eq!(rows[0][0], "Wire 1");
}

#[test]
fn a_language_file_can_set_the_decimal_separator() {
    let file = r#"{"code":"de","name":"Deutsch","decimal":",","entries":{}}"#;
    let c = Catalog::from_json(file).unwrap();
    assert_eq!(c.decimal_char(), Some(','));
    let mut table = ProjectTable::default();
    table.put_language(c);
    let de = Translator::new("de", &table);
    assert_eq!(de.decimal(), ',');
    assert_eq!(de.catalog().decimal.as_deref(), Some(","));
    // The exported file keeps it and reads back the same.
    let back = Catalog::from_json(&de.catalog().to_json()).unwrap();
    assert_eq!(back.decimal_char(), Some(','));
    // Anything but `,` or `.` is ignored.
    let odd = Catalog::from_json(r#"{"code":"x","decimal":"'"}"#).unwrap();
    assert_eq!(odd.decimal_char(), None);
}

#[test]
fn only_real_fixed_texts_match_the_checklist_patterns() {
    let tr = nl();
    let mut s = Section::new("s", "", vec![Column::new("c", "Description", 20.0)]);
    for text in [
        "R1 DIN rail",
        "Heavy duty DIN rail",
        "R2 at 20",
        "Panel at door",
        "R3 at 2.5",
        "Left",
        "Plate2",
        "Plateau",
    ] {
        s.push(vec![Cell::text(text)], vec![]);
    }
    let out = ReportOutput {
        report: "t".into(),
        title: "".into(),
        revision: 0,
        sections: vec![s],
    };
    let rows: Vec<String> = translate_output(&out, &tr).sections[0]
        .text_rows()
        .into_iter()
        .flatten()
        .collect();
    assert_eq!(
        rows,
        [
            "R1 DIN-rail",
            // A part description that only ends like the pattern is data.
            "Heavy duty DIN rail",
            "R2 op 20",
            "Panel at door",
            "R3 op 2,5",
            "Links",
            "Plaat2",
            "Plateau",
        ]
    );
}

#[test]
fn only_measure_placeholders_follow_the_decimal_sign() {
    let mut table = ProjectTable::default();
    table.set("nl", "Sheet {n}", "Blad {n}").unwrap();
    let tr = Translator::new("nl", &table);
    // A sheet name that looks like a decimal keeps its point.
    assert_eq!(tr.tr("Sheet 1.2"), "Blad 1.2");
    assert_eq!(tr.tr("Hole dia 4.2"), "Gat dia 4,2");
    // A measure placeholder only captures a number.
    assert_eq!(tr.tr("Hole dia big"), "Hole dia big");
}
