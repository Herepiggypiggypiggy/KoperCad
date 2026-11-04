//! Failed jobs as flags (lane T12) and the census of the files area.

use super::Level;
use super::flags_jobs::{DISMISS, OPEN_FOLDER, job_flag_id};
use crate::app::gate1_tests::Drive;
use novo_flags::{Action, Anchor, Severity, Source};
use std::path::Path;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    (dir, d)
}

#[test]
fn a_failed_export_is_a_red_flag_on_the_job_not_a_toast() {
    let (_dir, mut d) = drive();
    d.app.job_fail_at(
        "export.dxf",
        "DXF export failed",
        Level::Error,
        "Could not export a.dxf: The disk is full. Free some space or pick another drive",
        Path::new("/tmp/out/a.dxf"),
    );
    let flags = d.app.job_flags();
    assert_eq!(flags.len(), 1);
    let f = &flags[0];
    assert_eq!(f.severity, Severity::Red);
    assert_eq!(f.source, Source::Job);
    assert_eq!(f.anchors, vec![Anchor::Job("export.dxf".into())]);
    assert_eq!(f.title, "DXF export failed");
    assert!(f.detail.0[0].contains("Free some space"));
    assert!(f.actions.contains(&Action::Retry));
    assert!(f.actions.contains(&Action::Open(OPEN_FOLDER.into())));
    assert!(f.actions.contains(&Action::Copy));
    assert!(f.actions.contains(&Action::Open(DISMISS.into())));
    // The message is in the log, but nothing pops up.
    let last = d.app.messages().last().unwrap();
    assert_eq!(last.level, Level::Error);
    assert!(last.dismissed);
    assert_eq!(f.id, job_flag_id("export.dxf"));
}

#[test]
fn the_same_job_failing_twice_is_one_flag_and_success_clears_it() {
    let (_dir, mut d) = drive();
    for _ in 0..2 {
        d.app
            .job_fail("export.pdf", "PDF export failed", Level::Error, "x");
    }
    assert_eq!(d.app.job_flags().len(), 1);
    d.app
        .job_fail("export.dxf", "DXF export failed", Level::Warning, "y");
    assert_eq!(d.app.job_flags().len(), 2);
    assert_eq!(d.app.job_flags()[1].severity, Severity::Yellow);
    d.app.job_ok("export.pdf");
    let left = d.app.job_flags();
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].title, "DXF export failed");
    d.app.job_ok("export.pdf");
    assert_eq!(d.app.job_flags().len(), 1, "clearing twice is harmless");
}

#[test]
fn information_is_not_a_flag() {
    let (_dir, mut d) = drive();
    d.app.job_fail("export.pdf", "PDF", Level::Info, "Done");
    assert!(d.app.job_flags().is_empty());
    assert_eq!(d.app.messages().last().unwrap().text, "Done");
}

#[test]
fn retry_runs_the_command_again_and_dismiss_clears() {
    let (_dir, mut d) = drive();
    d.app
        .job_fail("export.pdf", "PDF export failed", Level::Error, "x");
    let id = job_flag_id("export.pdf");
    assert!(d.app.run_job_action(id, &Action::Retry));
    assert!(d.app.wb.queued.contains(&"export.pdf"));
    assert!(d.app.job_flags().is_empty());
    d.app.job_fail_at(
        "import.dxf",
        "DXF import failed",
        Level::Error,
        "Could not import a.dxf: group code `abc` is not a number",
        Path::new("/tmp/a.dxf"),
    );
    let id = job_flag_id("import.dxf");
    // Retry asks for the file again; Open folder shows where it was read from.
    let f = &d.app.job_flags()[0];
    assert!(f.actions.contains(&Action::Retry));
    assert!(f.actions.contains(&Action::Open(OPEN_FOLDER.into())));
    assert!(!f.detail.0[0].contains('`'), "{:?}", f.detail);
    assert!(d.app.run_job_action(id, &Action::Open(DISMISS.into())));
    assert!(d.app.job_flags().is_empty());
}

#[test]
fn the_top_bar_triangle_opens_a_popover_with_the_actions() {
    let (_dir, mut d) = drive();
    d.app
        .job_fail("export.pdf", "PDF export failed", Level::Error, "Disk full");
    d.frame();
    d.frame();
    let id = job_flag_id("export.pdf");
    d.app.wb.flags.open = Some((id, eframe::egui::pos2(900.0, 40.0)));
    d.frame();
    assert!(d.app.wb.flags.open.is_some());
    d.key(eframe::egui::Key::Escape, eframe::egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.wb.flags.open.is_none());
    assert_eq!(d.app.job_flags().len(), 1, "closing does not clear it");
}

#[test]
fn every_retry_command_exists() {
    for job in [
        "export.pdf",
        "export.dxf",
        "import.dxf",
        "export.step",
        "export.aml",
        "export.eplan",
        "export.spreadsheet",
        "export.workpack",
        "export.bench_packet",
        "export.bench_cut",
        "export.ferrules",
        "export.labels",
        "export.change_report",
        "export.variant_pdf",
    ] {
        let command = super::flags_jobs::retry_command(job).expect(job);
        assert!(
            novo_workflow::commands::get(command).is_some() || crate::app::app_runs(command),
            "{job}: {command}"
        );
    }
}

/// The files of this area: the lane's list in LANE-PROMPTS.md.
fn area_files() -> Vec<std::path::PathBuf> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let named = [
        "interop",
        "dxf",
        "print_preview",
        "sheet_import",
        "sheets",
        "workpack",
        "handover",
        "change_report",
        "changes",
        "compare",
        "revisions",
        "variants",
        "review_notes",
        "designer",
        "manual",
    ];
    let prefixed = ["eplan_", "pdf", "bench", "reports"];
    let mut out: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let Some(stem) = name.strip_suffix(".rs") else {
                return false;
            };
            !stem.ends_with("_tests")
                && (named.contains(&stem) || prefixed.iter().any(|p| stem.starts_with(p)))
        })
        .collect();
    out.sort();
    out
}

/// Toast sites that raise an error or a warning: `.message(Level::Error`,
/// `.raise(super::Level::Warning` and the like, outside the tests.
fn toast_sites(source: &str) -> usize {
    let code = source.split("#[cfg(test)]").next().unwrap_or(source);
    let mut n = 0;
    for call in [".message(", ".raise("] {
        for (at, _) in code.match_indices(call) {
            let rest = code[at + call.len()..].trim_start();
            let rest = rest.strip_prefix("super::").unwrap_or(rest);
            if rest.starts_with("Level::Error") || rest.starts_with("Level::Warning") {
                n += 1;
            }
        }
    }
    n
}

/// The ceiling for this area only goes down (design section 12). It is one
/// file for this area, so lane PRs of other areas never edit the same line.
#[test]
fn problem_census_files() {
    let ceiling: usize = include_str!("problem_census_files.txt")
        .trim()
        .parse()
        .expect("the ceiling is a number");
    let counts: Vec<(String, usize)> = area_files()
        .iter()
        .map(|p| {
            let n = toast_sites(&std::fs::read_to_string(p).unwrap());
            (p.file_name().unwrap().to_string_lossy().into_owned(), n)
        })
        .filter(|(_, n)| *n > 0)
        .collect();
    let total: usize = counts.iter().map(|(_, n)| n).sum();
    assert!(
        total <= ceiling,
        "{total} red or yellow toasts in the files area, ceiling {ceiling}: {counts:?}. \
         A failed job is a job flag (`job_fail`); lower the ceiling when you remove one."
    );
}

#[test]
fn a_job_failure_never_touches_a_toast_on_screen() {
    let (_dir, mut d) = drive();
    d.app
        .message(Level::Error, "Could not write a.pdf: disk full");
    d.app.message(Level::Warning, "Something else happened");
    d.app.job_fail(
        "export.pdf",
        "PDF export failed",
        Level::Error,
        "Could not write a.pdf: disk full",
    );
    let flag = &d.app.job_flags()[0];
    assert_eq!(flag.detail.0, ["Could not write a.pdf: disk full"]);
    let shown: Vec<_> = d
        .app
        .messages()
        .iter()
        .filter(|m| !m.dismissed)
        .map(|m| m.text.as_str())
        .collect();
    assert_eq!(
        shown,
        [
            "Could not write a.pdf: disk full",
            "Something else happened"
        ],
        "the toasts that were up stay up, unchanged"
    );
}

#[test]
fn a_failed_job_goes_when_the_project_is_replaced() {
    let (_dir, mut d) = drive();
    d.app
        .job_fail("export.pdf", "PDF export failed", Level::Error, "x");
    d.app.document_replaced();
    assert!(d.app.job_flags().is_empty());
    assert!(d.app.wb.flags.jobs.is_empty());
}

#[test]
fn many_failed_jobs_keep_the_top_bar_in_the_window() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, crate::actions::Action::NewProject);
    for (job, title) in [
        ("export.pdf", "PDF export failed"),
        ("export.dxf", "DXF export failed"),
        ("import.aml", "AML import failed"),
    ] {
        d.app.job_fail(job, title, Level::Error, "x");
    }
    d.frame_sized(eframe::egui::vec2(1366.0, 768.0), Vec::new());
    d.frame_sized(eframe::egui::vec2(1366.0, 768.0), Vec::new());
    let rect = d.app.wb.job_pill_rect;
    assert!(rect.width() > 0.0, "the pill is drawn");
    assert!(rect.right() <= 1366.0 && rect.left() >= 0.0, "{rect:?}");
    assert!(d.app.wb.pill_rect.right() <= 1366.0);
    assert!(
        rect.right() <= d.app.wb.pill_rect.left() + 1.0,
        "left of Issues"
    );
}
