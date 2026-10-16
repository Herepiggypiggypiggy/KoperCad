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
