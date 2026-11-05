//! Problem triangles in the shell (lane T14).

use novo_flags::{Anchor, Severity, Source};

use crate::actions::Action;
use crate::app::gate1_tests::Drive;

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    (dir, d)
}

#[test]
fn a_failed_job_is_a_flag_on_the_job_and_the_next_success_clears_it() {
    let (_dir, mut d) = drive();
    d.app.job_flag(
        "test.job",
        true,
        "Thing not written",
        "Could not write x: disk full",
        Some("help.open_log_folder"),
    );
    let jobs = d.app.job_flags();
    assert_eq!(jobs.len(), 1);
    let flag = &jobs[0];
    assert_eq!(flag.severity, Severity::Red);
    assert_eq!(flag.source, Source::Job);
    assert_eq!(flag.anchors, vec![Anchor::Job("test.job".into())]);
    assert!(flag.accessible_name().starts_with("Error: "));
    // Raising it again with new words replaces it: one flag, one count.
    d.app
        .job_flag("test.job", false, "Thing not written", "Again", None);
    assert_eq!(d.app.job_flags().len(), 1);
    assert_eq!(d.app.job_flags()[0].severity, Severity::Yellow);
    d.app.job_ok("test.job");
    assert!(d.app.job_flags().is_empty());
}

#[test]
fn the_popover_offers_the_fix_and_dismiss_clears_the_flag() {
    let (_dir, mut d) = drive();
    d.app.job_flag(
        "diag.log_folder",
        false,
        "Log folder did not open",
        "Could not open it",
        Some("help.open_log_folder"),
    );
    let id = d.app.job_flags()[0].id;
    d.app.wb.flags.open = Some((id, eframe::egui::pos2(300.0, 300.0)));
    d.frame();
    d.frame();
    assert!(d.app.wb.flags.open.is_some());
    d.key(eframe::egui::Key::Escape, eframe::egui::Modifiers::NONE);
    d.frame();
    assert!(d.app.wb.flags.open.is_none());
    assert!(
        !d.app.job_flags().is_empty(),
        "Esc closes, it does not clear"
    );
}

#[test]
fn a_control_flag_goes_when_its_window_is_gone() {
    let (_dir, mut d) = drive();
    d.app.control_flag(
        "project_rules",
        "rules",
        false,
        "Rules not saved",
        "x",
        None,
    );
    assert_eq!(d.app.control_flags("project_rules").len(), 1);
    assert!(d.app.control_flags("project_settings").is_empty());
    d.app.control_clear_all("project_rules");
    assert!(d.app.control_flags("project_rules").is_empty());
}

#[test]
fn two_jobs_are_two_flags_with_the_worst_first() {
    let (_dir, mut d) = drive();
    d.app.job_flag("a", false, "A", "a", None);
    d.app.job_flag("b", true, "B", "b", None);
    let jobs = d.app.job_flags();
    assert_eq!(jobs.len(), 2);
    assert!(jobs.iter().any(|f| f.severity == Severity::Red));
}

#[test]
fn every_fix_command_a_shell_flag_names_exists() {
    for (command, label) in super::flags_shell::COMMANDS {
        assert!(!label.is_empty());
        assert!(
            super::workbench::APP_COMMANDS.contains(command),
            "{command}"
        );
    }
}

/// How many places in the shell files still say a problem as a toast or a
/// coloured sentence. The number only goes down (design section 12); T15 makes
/// it zero.
#[test]
fn shell_census_ceiling() {
    const FILES: &[&str] = &[
        "addons.rs",
        "addons_look.rs",
        "addons_widgets.rs",
        "learn.rs",
        "learn_ui.rs",
        "onboarding.rs",
        "preferences.rs",
        "project_settings.rs",
        "project_rules.rs",
        "languages.rs",
        "userkeymap.rs",
        "diagnostics.rs",
        "centre.rs",
        "drawers.rs",
        "describe_recipes.rs",
        "assistant/datasheet.rs",
    ];
    const CEILING: usize = 5;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let mut found = Vec::new();
    for file in FILES {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            let toast = code.contains("Level::Error") || code.contains("Level::Warning");
            let banner = code.contains("banner(") && !code.contains("fn banner");
            let coloured = (code.contains("colored_label(") || code.contains(".color("))
                && (code.contains("pal.warning")
                    || code.contains("pal.error")
                    || code.contains("palette.warning")
                    || code.contains("palette.error")
                    || code.contains("ui_palette().warning")
                    || code.contains("ui_palette().error"));
            if toast || banner || coloured {
                found.push(format!("{file}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(
        found.len() <= CEILING,
        "{} sites, ceiling {CEILING}:\n{}",
        found.len(),
        found.join("\n")
    );
}
