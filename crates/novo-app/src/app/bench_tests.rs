//! Bench output from the app: the three PDFs, the window and its drafts.

use novo_reports::{FerruleStandard, LabelSettings};

use super::bench::Tab;
use super::gate1_tests::Drive;
use super::route_all_tests::wired_cabinet;
use crate::actions::Action;

fn app_with_wires() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    wired_cabinet(&mut d);
    let wires: Vec<_> = d.doc().project().wires().map(|w| w.id).collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Sizes", |tx| {
            for id in wires {
                tx.update::<novo_core::Wire>(id, |w| w.cross_section_mm2 = Some(1.5))?;
            }
            Ok(())
        })
        .unwrap();
    (dir, d)
}

fn is_pdf(path: &std::path::Path) -> bool {
    std::fs::read(path).is_ok_and(|b| b.starts_with(b"%PDF"))
}

#[test]
fn nothing_to_cut_writes_no_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    for name in ["cut.pdf", "ferrules.pdf", "labels.pdf"] {
        let path = dir.path().join(name);
        assert_eq!(d.app.export_bench_cut_to(&path), None);
        assert_eq!(d.app.export_ferrules_to(&path), None);
        assert_eq!(d.app.export_labels_to(&path), None);
        assert!(!path.exists());
    }
}

#[test]
fn the_three_bench_pdfs_are_written() {
    let (dir, mut d) = app_with_wires();
    let cut = dir.path().join("cut.pdf");
    let r = d.app.export_bench_cut_to(&cut);
    assert_eq!(r, Some(cut.clone()), "{:?}", d.app.messages().last());
    assert!(is_pdf(&cut));
    let ferrules = dir.path().join("ferrules.pdf");
    assert_eq!(d.app.export_ferrules_to(&ferrules), Some(ferrules.clone()));
    assert!(is_pdf(&ferrules));
    // The default labels are the wire flags: two for the one wire.
    let labels = dir.path().join("labels.pdf");
    assert_eq!(d.app.export_labels_to(&labels), Some(labels.clone()));
    assert!(is_pdf(&labels));
    let msg = d.app.messages().last().expect("a message").text.clone();
    assert!(msg.contains("2 labels on 1 sheets"), "{msg}");
}

#[test]
fn the_commands_export_a_pdf_each() {
    let (_dir, mut d) = app_with_wires();
    let ctx = d.ctx.clone();
    for (id, part) in [
        ("export.bench_cut", "bench cut list"),
        ("export.ferrules", "ferrules"),
        ("export.labels", "labels"),
    ] {
        d.app.wb.last_export = None;
        d.app.run_command(&ctx, id, None);
        d.frame();
        // A project with no file exports to Documents, which a test box may lack.
        let Some(path) = d.app.wb.last_export.clone() else {
            let msg = d.app.messages().last().unwrap().text.clone();
            assert!(msg.contains("Documents folder"), "{id}: {msg}");
            continue;
        };
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.contains(part) && name.ends_with(".pdf"), "{name}");
        assert!(is_pdf(&path), "{id}");
        let _ = std::fs::remove_file(&path);
    }
}

#[test]
fn the_window_opens_on_the_saved_settings_and_renders_every_tab() {
    let (_dir, mut d) = app_with_wires();
    let ctx = d.ctx.clone();
    let standard = FerruleStandard {
        spare_percent: 25.0,
        ..FerruleStandard::default()
    };
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Standard", |tx| FerruleStandard::store(tx, &standard))
        .unwrap();
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    for tab in [Tab::CutList, Tab::Ferrules, Tab::Labels] {
        d.app.wb.bench.as_mut().expect("open").tab = tab;
        d.frame();
        d.frame();
    }
    let state = d.app.wb.bench.as_ref().expect("open");
    assert_eq!(state.standard, standard);
    assert_eq!(state.labels, LabelSettings::default());
}

#[test]
fn exports_use_the_drafts_in_the_window_and_the_project_otherwise() {
    let (dir, mut d) = app_with_wires();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    // An invalid draft blocks the ferrule table but not the cut list.
    d.app.wb.bench.as_mut().unwrap().standard.sizes.clear();
    d.app
        .wb
        .bench
        .as_mut()
        .unwrap()
        .standard
        .sizes
        .push(novo_reports::FerruleSize {
            mm2: 0.0,
            colour: String::new(),
            length_mm: 0.0,
        });
    let ferrules = dir.path().join("f.pdf");
    assert_eq!(d.app.export_ferrules_to(&ferrules), None);
    assert!(!ferrules.exists());
    // A draft with a custom tape stock prints one page per label.
    let state = d.app.wb.bench.as_mut().unwrap();
    state.standard = FerruleStandard::default();
    state.labels.stock = novo_reports::stock_presets().pop().unwrap();
    let labels = dir.path().join("tape.pdf");
    assert_eq!(d.app.export_labels_to(&labels), Some(labels.clone()));
    let msg = d.app.messages().last().unwrap().text.clone();
    assert!(msg.contains("2 labels on 2 pieces of tape"), "{msg}");
    // Closing the window goes back to the project's settings.
    d.app.wb.bench = None;
    let again = dir.path().join("sheet.pdf");
    d.app.export_labels_to(&again);
    let msg = d.app.messages().last().unwrap().text.clone();
    assert!(msg.contains("1 sheets"), "{msg}");
}

#[test]
fn the_brother_csv_has_one_file_per_chosen_kind() {
    let (dir, mut d) = app_with_wires();
    let files = d.app.export_labels_csv_in(dir.path(), "p");
    assert_eq!(files.len(), 1);
    let text = std::fs::read_to_string(&files[0]).unwrap();
    assert_eq!(text.lines().count(), 2, "one row per wire end");
    assert!(files[0].starts_with(dir.path()));
}

#[test]
fn an_invalid_draft_blocks_the_cut_list_and_the_machine_csv_follows_the_draft() {
    let (dir, mut d) = app_with_wires();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "view.bench", None);
    d.frame();
    let state = d.app.wb.bench.as_mut().unwrap();
    state.standard.sizes[3].length_mm = 0.0;
    let cut = dir.path().join("cut.pdf");
    let csv = dir.path().join("cut.csv");
    assert_eq!(d.app.export_bench_cut_to(&cut), None);
    assert_eq!(d.app.export_machine_csv_to(&csv), None);
    assert!(!cut.exists() && !csv.exists());
    // A valid draft with a longer contact length shows in the CSV (strip 14).
    let state = d.app.wb.bench.as_mut().unwrap();
    state.standard.sizes[3].length_mm = 14.0;
    assert_eq!(d.app.export_machine_csv_to(&csv), Some(csv.clone()));
    let text = std::fs::read_to_string(&csv).unwrap();
    assert!(text.contains("A1.5-14"), "{text}");
}

#[test]
fn the_cut_table_rows_carry_a_tilde_exactly_when_the_length_is_an_estimate() {
    let (_dir, d) = app_with_wires();
    let jobs = novo_reports::bench_jobs(d.doc().project());
    assert!(!jobs.is_empty());
    for job in &jobs {
        let cells = super::bench::cut_row_texts(job);
        assert_eq!(
            cells[5].starts_with('~'),
            novo_reports::is_estimate(&job.basis),
            "{cells:?}"
        );
        assert_eq!(cells[1], "1.5");
        assert_eq!(cells[6], job.basis);
        assert!(cells.iter().all(|c| !c.is_empty()), "{cells:?}");
    }
}

#[test]
fn the_default_size_is_filled_in_for_unsized_wires_in_one_undo_step() {
    let (_dir, mut d) = app_with_wires();
    // `app_with_wires` sized every wire; clear them again.
    let ids: Vec<_> = d.doc().project().wires().map(|w| w.id).collect();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Clear", |tx| {
            for id in ids {
                tx.update::<novo_core::Wire>(id, |w| w.cross_section_mm2 = None)?;
            }
            Ok(())
        })
        .unwrap();
    let before = novo_reports::unset_wires(d.doc().project());
    assert!(!before.no_size.is_empty());
    d.app.fill_wire_defaults(&novo_reports::WireDefaults {
        size_mm2: Some(2.5),
        ..novo_reports::WireDefaults::default()
    });
    let after = novo_reports::unset_wires(d.doc().project());
    assert!(after.no_size.is_empty());
    let msg = d.app.messages().last().expect("a message").text.clone();
    assert!(msg.contains("Set the size of"), "{msg}");
    // One undo gives the unsized wires back.
    assert!(
        d.app
            .backend
            .document_mut()
            .unwrap()
            .undo()
            .unwrap()
            .is_some(),
        "one step to undo"
    );
    assert_eq!(
        novo_reports::unset_wires(d.doc().project()).no_size.len(),
        before.no_size.len()
    );
}
