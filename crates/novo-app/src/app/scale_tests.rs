//! Time budgets on a project of 600 pages, through the whole window,
//! headless: open, frames in every workspace, page change, search, and the
//! heavy exports. The budgets are for a release build at 600 pages (the
//! `Where the numbers come from` table in `handoff/scale-2/RESULTS.md`).
//!
//! Debug builds (CI) run a quarter of the project, 150 pages, and allow a
//! debug slowdown on top, so a cost that grows with pages times pages (the
//! Commissioning list once took 8.6 s a frame at 600 pages) still goes over
//! budget while a slow CI machine does not. For the real figures run
//! `cargo test --release -p novo-app --lib scale_tests -- --nocapture`.

use std::time::Instant;

use eframe::egui;
use novo_tests::{BigSpec, big_project};
use novo_workflow::palette::Mode;

use super::gate1_tests::Drive;
use super::standards_report::StandardsFormat;
use super::workspace::Workspace;
use crate::actions::Action;

/// Circuits of the generated project: two pages each.
const FEEDERS_RELEASE: usize = 300;
const FEEDERS_DEBUG: usize = 75;
/// How much slower than release the test build is allowed to be, on top of
/// the smaller project. The Windows CI runs 1,400 tests at once; with 12 the
/// Ctrl+K step once took 2.0 s against 1.5 s on timing alone.
const DEBUG_FACTOR: f64 = 25.0;

struct Budget {
    pages: usize,
    scale: f64,
    over: Vec<String>,
}

impl Budget {
    fn new(pages: usize) -> Self {
        let scale = if cfg!(debug_assertions) {
            DEBUG_FACTOR * pages as f64 / 600.0
        } else {
            1.0
        };
        Budget {
            pages,
            scale,
            over: Vec::new(),
        }
    }

    /// Runs `f`, prints the time and notes it when over `budget_ms` (the
    /// release figure at 600 pages).
    fn lap<T>(&mut self, label: &str, budget_ms: f64, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let out = f();
        self.note(label, budget_ms, start.elapsed().as_secs_f64() * 1000.0);
        out
    }

    fn note(&mut self, label: &str, budget_ms: f64, ms: f64) {
        let limit = budget_ms * self.scale;
        eprintln!(
            "{label}: {ms:.1} ms (budget {limit:.0} ms, {} pages)",
            self.pages
        );
        if ms > limit {
            self.over
                .push(format!("{label}: {ms:.0} ms, budget {limit:.0} ms"));
        }
    }

    /// The mean time of `n` frames.
    fn frames(&mut self, d: &mut Drive, label: &str, budget_ms: f64, n: usize) {
        let start = Instant::now();
        for _ in 0..n {
            d.frame_sized(egui::vec2(1366.0, 768.0), Vec::new());
        }
        let mean = start.elapsed().as_secs_f64() * 1000.0 / n as f64;
        self.note(&format!("{label} (mean of {n} frames)"), budget_ms, mean);
    }
}

#[test]
fn a_large_project_stays_inside_its_time_budgets() {
    let feeders = if cfg!(debug_assertions) {
        FEEDERS_DEBUG
    } else {
        FEEDERS_RELEASE
    };
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();

    // The file is written by a separate connection: the app's own would
    // still hold the lock and the open would wait for it.
    let mut big = novo_core::Document::default();
    big_project(&mut big, BigSpec::new(feeders)).unwrap();
    let pages = big.project().pages().count();
    let path = dir.path().join("big.kopercad");
    novo_storage::ProjectFile::create(&path, big.project()).expect("save");
    drop(big);
    let mut b = Budget::new(pages);

    b.lap("open the file", 1500.0, || d.app.open_path(&path));
    b.lap("first frame after open", 1000.0, || d.frame());
    b.frames(&mut d, "schematic frame", 100.0, 5);
    let order: Vec<_> = d
        .doc()
        .project()
        .pages_in_order()
        .iter()
        .map(|p| p.id)
        .collect();
    b.lap("go to a page in the middle and draw it", 100.0, || {
        d.app.show_page(order[order.len() / 2]);
        d.frame();
    });
    b.lap("Ctrl+K, type a tag, jump to it", 1000.0, || {
        d.app.type_command(&ctx, Mode::All, "F451");
        d.frame();
    });
    for (w, enter, steady) in [
        (Workspace::Cabinet, 500.0, 100.0),
        (Workspace::View3d, 500.0, 100.0),
        (Workspace::Reports, 1500.0, 100.0),
        (Workspace::Commissioning, 600.0, 100.0),
    ] {
        b.lap(&format!("switch to {w:?}"), enter, || {
            d.app.switch_workspace(w);
            d.frame();
        });
        b.frames(&mut d, &format!("{w:?} frame"), steady, 3);
    }
    d.app.switch_workspace(Workspace::Schematic);
    b.lap("open Autopilot", 1500.0, || d.app.open_autopilot());
    b.frames(&mut d, "Autopilot frame", 100.0, 2);
    let packet = dir.path().join("packet.pdf");
    b.lap("export the bench packet", 3000.0, || {
        d.app.export_bench_packet_to(&packet, &[])
    });
    assert!(packet.exists(), "the packet is written");
    let report = dir.path().join("standards.pdf");
    b.lap("write the standards report", 3000.0, || {
        d.app.write_standards_report(StandardsFormat::Pdf, &report)
    });
    assert!(report.exists(), "the report is written");

    assert!(
        b.over.is_empty(),
        "over budget at {} pages:\n{}",
        b.pages,
        b.over.join("\n")
    );
}
