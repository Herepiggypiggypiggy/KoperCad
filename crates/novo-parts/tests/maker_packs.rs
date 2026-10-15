//! Maker recipes: golden files per maker, and the watch and order checks.
//!
//! `golden/<maker>/sample.*` is read through the real import path
//! (`ImportJob::import_file`) and compared with `sample.expected.json`.
//! Regenerate after a deliberate change with
//! `UPDATE_GOLDEN=1 cargo test -p novo-parts --test maker_packs`, then read
//! the diff: the expected files are the specification of what each recipe
//! reads. See `golden/README.md` for what these samples are.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use novo_parts::*;
use serde_json::{Value as Json, json};

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden")
}

fn slug_to_maker(slug: &str) -> String {
    slug.replace('-', " ")
}

/// Imports `file` for `maker` through the library and returns the outcome.
fn import(maker: &str, order: &str, file: &Path) -> ImportOutcome {
    let dir = tempfile::tempdir().unwrap();
    let lib = LocalLibrary::at(dir.path());
    let sources = SourceTable::builtin();
    let fetcher = ScriptedFetcher::default();
    let mut job = ImportJob::new(&lib, &sources, &fetcher, maker, order, "2026-10-15");
    match job.import_file(file, ImportFlow::Manual).clone() {
        ImportState::Done(o) => o,
        other => panic!("{}: {other:?}", file.display()),
    }
}

fn snapshot(o: &ImportOutcome) -> Json {
    let r = &o.recipe;
    json!({
        "parts": o.parts,
        "notes": o.notes,
        "report": {
            "parts": r.parts,
            "mapped": r.mapped.iter().map(|m| format!("{}: {} -> {}", m.order_number, m.feature, m.became)).collect::<Vec<_>>(),
            "skipped": r.skipped.iter().map(|s| format!("{}: {} ({})", s.order_number, s.feature, s.reason)).collect::<Vec<_>>(),
            "unmapped": r.unmapped,
            "warnings": r.warnings,
            "summary": r.summary_lines(),
        },
    })
}

#[test]
fn every_maker_has_a_golden_sample_that_reads_as_recorded() {
    let update = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut seen = Vec::new();
    let mut slugs: Vec<_> = fs::read_dir(golden_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    slugs.sort();
    for slug in slugs {
        let dir = golden_dir().join(&slug);
        let sample = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_stem().is_some_and(|s| s == "sample")
                    && p.extension().is_some_and(|e| e != "json")
            })
            .unwrap_or_else(|| panic!("{slug}: no sample file"));
        let maker = slug_to_maker(&slug);
        assert!(recipe_for(&maker).is_some(), "{slug} has no recipe");
        let got = snapshot(&import(&maker, "x", &sample));
        let expected_path = dir.join("sample.expected.json");
        let text = serde_json::to_string_pretty(&got).unwrap() + "\n";
        if update {
            fs::write(&expected_path, &text).unwrap();
        } else {
            let want = fs::read_to_string(&expected_path)
                .unwrap_or_else(|_| panic!("{slug}: no sample.expected.json (UPDATE_GOLDEN=1)"));
            assert_eq!(
                text.replace("\r\n", "\n"),
                want.replace("\r\n", "\n"),
                "{slug}: the recipe reads the sample differently from the recorded result"
            );
        }
        seen.push(slug);
    }
    // One golden per recipe, so a new recipe cannot ship untested.
    for recipe in RECIPES {
        let slug = recipe
            .manufacturer
            .to_lowercase()
            .replace('ü', "ue")
            .replace(' ', "-");
        assert!(
            seen.contains(&slug),
            "recipe {} has no golden sample",
            recipe.manufacturer
        );
    }
}

fn part<'a>(o: &'a ImportOutcome, order: &str) -> &'a CatalogPart {
    o.parts
        .iter()
        .find(|p| p.order_number == order)
        .unwrap_or_else(|| panic!("no part {order}"))
}

#[test]
fn a_terminal_block_gets_class_terminals_range_and_source_tags() {
    let o = import(
        "Phoenix Contact",
        "3044076",
        &golden_dir().join("phoenix-contact/sample.xml"),
    );
    let p = part(&o, "3044076");
    assert_eq!(p.function, "terminal");
    assert_eq!(p.letter_codes, ["X"]);
    assert_eq!(p.class.etim.as_deref(), Some("EC000897"));
    assert_eq!(
        p.data.get(eng::RATED_CURRENT_A),
        Some(&novo_core::Value::Float(24.0))
    );
    assert_eq!(
        p.source_of(eng::RATED_CURRENT_A),
        DataSource::MakerDatasheet
    );
    assert_eq!(p.terminals.len(), 2);
    assert_eq!(p.terminals[0].kind, ConnectionKind::Screw);
    // The widest range over rigid and flexible: 0.14 to 4 mm2.
    assert_eq!(p.terminals[0].wire_mm2, Some((0.14, 4.0)));
    let d = p.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (5.2, 47.1, 42.2));
    let prov = p.provenance.as_ref().unwrap();
    assert_eq!(prov.source, "import:bmecat");
    assert!(!prov.redistributable());
    // A type name where the order number belongs is imported, with a warning.
    assert!(o.recipe.warnings.iter().any(|w| w.starts_with("UT 2,5")));
}

#[test]
fn german_wording_and_a_range_with_a_decimal_comma() {
    let o = import(
        "Weidmüller",
        "1020000000",
        &golden_dir().join("weidmueller/sample.xml"),
    );
    let p = part(&o, "1020000000");
    assert_eq!(
        p.manufacturer, "Weidmüller",
        "the maker's long name is made canonical"
    );
    assert_eq!(p.terminals[0].wire_mm2, Some((0.5, 4.0)));
    assert_eq!(p.terminals[0].kind, ConnectionKind::Screw);
    assert_eq!(p.class.eclass.as_deref(), Some("27-14-11-01"));
    assert_eq!(p.lifecycle, Lifecycle::Active);
    let old = part(&o, "1020001000");
    assert_eq!(old.lifecycle, Lifecycle::PhasingOut);
    assert_eq!(old.replaced_by.as_ref().unwrap().order_number, "1020000000");
}

#[test]
fn a_contactor_reads_ac3_values_and_leaves_per_conductor_losses_alone() {
    let o = import(
        "Siemens",
        "3RT2016-1BB41",
        &golden_dir().join("siemens/sample.xml"),
    );
    let p = part(&o, "3RT2016-1BB41");
    assert_eq!(
        p.data.get(eng::AC3_CURRENT_A),
        Some(&novo_core::Value::Float(9.0))
    );
    assert_eq!(
        p.data.get(eng::RATED_POWER_KW),
        Some(&novo_core::Value::Float(4.0))
    );
    assert_eq!(
        p.data.get(eng::COIL_VOLTAGE_V),
        Some(&novo_core::Value::Float(24.0))
    );
    assert_eq!(p.data.get(eng::POLES), Some(&novo_core::Value::Int(3)));
    assert!(!p.data.contains_key(eng::POWER_LOSS_W));
    assert!(
        o.recipe
            .skipped
            .iter()
            .any(|s| s.reason.contains("per pole or conductor"))
    );
    assert_eq!(p.function, "contactor");
    let mpcb = part(&o, "3RV2011-1EA10");
    assert_eq!(
        mpcb.data.get(eng::SETTING_MIN_A),
        Some(&novo_core::Value::Float(2.8))
    );
    assert_eq!(
        mpcb.data.get(eng::SETTING_MAX_A),
        Some(&novo_core::Value::Float(4.0))
    );
    // kA is stored as kA, not converted twice.
    assert_eq!(
        mpcb.data.get(eng::ICS_KA),
        Some(&novo_core::Value::Float(50.0))
    );
}

#[test]
fn schneider_wording_status_and_replacement() {
    let o = import(
        "Schneider Electric",
        "LC1D09BD",
        &golden_dir().join("schneider-electric/sample.xml"),
    );
    let p = part(&o, "LC1D09BD");
    assert_eq!(p.lifecycle, Lifecycle::Active);
    assert_eq!(p.data.get(eng::POLES), Some(&novo_core::Value::Int(3)));
    assert_eq!(
        p.data.get(eng::POWER_LOSS_W),
        Some(&novo_core::Value::Float(1.2))
    );
    let old = part(&o, "LC1D09B7");
    assert_eq!(old.lifecycle, Lifecycle::PhasingOut);
    assert_eq!(old.replaced_by.as_ref().unwrap().order_number, "LC1D09BD");
}

#[test]
fn a_breaker_reads_curve_and_breaking_capacity() {
    let o = import(
        "ABB",
        "2CDS251001R0164",
        &golden_dir().join("abb/sample.xml"),
    );
    let p = part(&o, "2CDS251001R0164");
    assert_eq!(
        p.data.get(eng::TRIP_CURVE),
        Some(&novo_core::Value::Text("C".into()))
    );
    assert_eq!(
        p.data.get(eng::ICU_KA),
        Some(&novo_core::Value::Float(10.0))
    );
    assert_eq!(p.data.get(eng::ICS_KA), Some(&novo_core::Value::Float(7.5)));
    assert_eq!(p.function, "miniature_circuit_breaker");
}

#[test]
fn a_cable_reads_section_cores_diameter_and_resistance() {
    let o = import("Lapp", "1119303", &golden_dir().join("lapp/sample.xml"));
    let p = part(&o, "1119303");
    assert_eq!(p.function, "cable");
    assert_eq!(
        p.data.get(eng::SECTION_MM2),
        Some(&novo_core::Value::Float(1.5))
    );
    assert_eq!(p.data.get(eng::CORE_COUNT), Some(&novo_core::Value::Int(3)));
    assert_eq!(
        p.data.get(eng::OUTER_DIAMETER_MM),
        Some(&novo_core::Value::Float(8.6))
    );
    assert_eq!(
        p.data.get(eng::R20_OHM_PER_KM),
        Some(&novo_core::Value::Float(13.3))
    );
    assert_eq!(
        p.data.get(eng::MAX_TEMP_C),
        Some(&novo_core::Value::Float(80.0))
    );
    assert!(p.terminals.is_empty(), "a cable has no terminals");
}

#[test]
fn a_windows_1252_semicolon_price_list_reads_with_price_and_currency() {
    let o = import("WAGO", "2002-1201", &golden_dir().join("wago/sample.csv"));
    let p = part(&o, "2002-1201");
    assert_eq!(p.description, "Durchgangsklemme 2-Leiter, 2,5 mm²");
    let price = p.price.as_ref().unwrap();
    assert_eq!((price.amount, price.currency.as_str()), (0.42, "EUR"));
    assert_eq!(p.terminals.len(), 2);
    assert_eq!(p.terminals[0].kind, ConnectionKind::PushIn);
    // A price without a currency is not guessed.
    assert!(part(&o, "2002-1202").price.is_none());
    assert!(
        o.recipe
            .skipped
            .iter()
            .any(|s| s.reason == "price without a currency")
    );
    assert!(o.recipe.warnings.iter().any(|w| w.starts_with("2002-12:")));
    assert_eq!(p.provenance.as_ref().unwrap().source, "import:csv");
}

#[test]
fn rittal_sizes_come_from_the_csv_columns_and_the_price_keeps_its_unit() {
    let o = import("Rittal", "1060500", &golden_dir().join("rittal/sample.csv"));
    let p = part(&o, "AE 1060.500");
    let d = p.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (600.0, 600.0, 210.0));
    assert_eq!(p.function, "enclosure");
    let price = p.price.as_ref().unwrap();
    assert_eq!(
        (price.amount, price.per, price.date.as_str()),
        (123.45, 1, "2026-01-01")
    );
}

#[test]
fn a_power_supply_reads_output_and_loss() {
    let o = import(
        "Murrelektronik",
        "85064",
        &golden_dir().join("murrelektronik/sample.csv"),
    );
    let p = part(&o, "85064");
    assert_eq!(p.function, "power_supply");
    // A size written with its unit (`60 mm`) is read, not dropped.
    let d = p.dimensions.unwrap();
    assert_eq!((d.width_mm, d.height_mm, d.depth_mm), (60.0, 124.0, 117.0));
    assert_eq!(
        p.data.get(local_keys::OUTPUT_CURRENT_A),
        Some(&novo_core::Value::Float(10.0))
    );
    assert_eq!(
        p.data.get(local_keys::OUTPUT_VOLTAGE_V),
        Some(&novo_core::Value::Float(24.0))
    );
    assert_eq!(
        p.data.get(eng::POWER_LOSS_W),
        Some(&novo_core::Value::Float(15.0))
    );
    assert!(o.recipe.warnings.iter().any(|w| w.starts_with("MICO:")));
}

#[test]
fn a_catalog_zip_is_read_for_its_bmecat_and_keeps_other_xml_for_eplan() {
    let bmecat = fs::read(golden_dir().join("lapp/sample.xml")).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let zip_path = dir.path().join("lapp_etim_bmecat.zip");
    {
        use std::io::Write;
        let mut w = zip::ZipWriter::new(fs::File::create(&zip_path).unwrap());
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("catalog/lapp.xml", opts).unwrap();
        w.write_all(&bmecat).unwrap();
        w.start_file("eplan/part.xml", opts).unwrap();
        w.write_all(b"<parts><part id=\"1\"/></parts>").unwrap();
        w.finish().unwrap();
    }
    let o = import("Lapp", "1119303", &zip_path);
    assert_eq!(o.parts.len(), 1);
    assert_eq!(o.parts[0].function, "cable");
    assert_eq!(o.xml_members.len(), 1);
    assert_eq!(o.xml_members[0].0, "eplan/part.xml");
}

fn watch(maker: &str, order: &str, file_name: &str) -> Vec<Arrival> {
    let dir = tempfile::tempdir().unwrap();
    let since = SystemTime::now() - Duration::from_secs(5);
    let mut w = DownloadsWatcher::for_maker(dir.path(), since, maker, order);
    fs::write(dir.path().join(file_name), b"data").unwrap();
    let _ = w.poll(); // first look records the size
    w.poll()
}

#[test]
fn the_watch_knows_the_numbers_makers_shorten_and_the_files_they_accept() {
    // ABB leaves the R-suffix off its file names.
    let a = watch("ABB", "1SDA066721R1", "1SDA066721_STEP.zip");
    assert_eq!(a.len(), 1);
    assert!(a[0].names_the_part);
    // Rittal files carry the number without the dot or the series prefix.
    assert_eq!(watch("Rittal", "SK 3209.500", "3209500.stp").len(), 1);
    // A catalog package without the number is offered for confirmation.
    let p = watch("Lapp", "1119303", "lapp_etim_bmecat_2026.zip");
    assert_eq!(p.len(), 1);
    assert!(!p[0].names_the_part);
    // An unrelated CSV or ZIP is left alone.
    assert!(watch("Lapp", "1119303", "invoice-4711.csv").is_empty());
    assert!(watch("Lapp", "1119303", "holiday.zip").is_empty());
    // WAGO's downloads are not EDZ files.
    assert!(watch("WAGO", "2002-1201", "something.edz").is_empty());
    // Phoenix accepts an EDZ by type.
    assert_eq!(watch("Phoenix Contact", "3044076", "export.edz").len(), 1);
}

#[test]
fn a_maker_without_a_recipe_still_gets_structured_data_and_provenance() {
    let o = import(
        "Example Maker",
        "9000-0001",
        &golden_dir().join("phoenix-contact/sample.xml"),
    );
    let p = part(&o, "3044076");
    assert_eq!(
        p.data.get(eng::RATED_CURRENT_A),
        Some(&novo_core::Value::Float(24.0))
    );
    assert!(p.provenance.is_some());
    // No recipe, so no order-number opinions.
    assert!(
        o.recipe.warnings.is_empty()
            || o.recipe
                .warnings
                .iter()
                .all(|w| !w.contains("does not look like"))
    );
}
