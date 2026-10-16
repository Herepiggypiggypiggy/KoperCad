//! The lesson shell (docs/design/learning-modules.md): a lesson opens its
//! own project, ticks from what the person does, moves on, keeps progress
//! and ends when another project opens.

use novo_learn::Files;

use super::gate1_tests::Drive;
use crate::actions::Action;

const TRACKS: &str = r#"
[[track]]
id = "basics"
title = "Basics"
summary = "The first lessons."
"#;

const LESSON: &str = r#"
format = 1
id = "tiny"
version = 1
title = "Tiny lesson"
summary = "Place a device, then fit the view."
track = "basics"
project = "@blank"

[[step]]
id = "hello"
title = "Read this"
text = "A reading step has a Next button."

[[step]]
id = "place"
title = "Place a device"
text = "Press {key:tool.place} and type a name."
expect = { count = { kind = "symbol", at_least = 1 } }

[[step]]
id = "fit"
title = "Fit the view"
text = "Press {key:view.fit}."
expect = { command = "view.fit" }

[[step]]
id = "pause"
title = "Look"
text = "Read, then press Next."
expect = { command = "view.fit" }
pause = true
"#;

fn files() -> Files {
    let mut files = Files::new();
    files.insert("tracks.toml".into(), TRACKS.into());
    files.insert("tiny/lesson.toml".into(), LESSON.into());
    files
}

fn drive() -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.use_files(files());
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.learn.advance_after = std::time::Duration::ZERO;
    (dir, d)
}

fn step(d: &Drive) -> String {
    d.app
        .learn
        .run
        .as_ref()
        .and_then(|r| r.step())
        .map(|s| s.id.clone())
        .unwrap_or_default()
}

#[test]
fn a_lesson_opens_its_own_project_marked_saved() {
    let (_dir, mut d) = drive();
    d.app.start_lesson("tiny");
    d.frame();
    assert_eq!(d.doc().project().info().name, "Lesson: Tiny lesson");
    assert!(!d.app.is_dirty(), "closing it untouched keeps nothing");
    assert_eq!(step(&d), "hello");
    assert!(d.app.learn_active());
}

#[test]
fn the_fast_keys_card_steps_aside_while_a_lesson_runs() {
    let (_dir, mut d) = drive();
    d.app.settings.onboarding.started = true;
    d.app.settings.onboarding.dismissed = false;
    d.app.start_lesson("tiny");
    d.frame();
    assert!(!d.app.walkthrough_on());
}

#[test]
fn steps_tick_from_the_drawing_and_from_commands_then_advance() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.start_lesson("tiny");
    d.frame();
    // A reading step waits for Next.
    d.frame();
    assert_eq!(step(&d), "hello");
    d.app.learn_move(super::learn::Move::Next);
    d.frame();
    assert_eq!(step(&d), "place");
    d.place("mcb3", 60.0, 60.0);
    d.frame();
    d.frame();
    assert_eq!(step(&d), "fit");
    d.app.run(&ctx, Action::ZoomToFit);
    d.frame();
    d.frame();
    assert_eq!(step(&d), "pause", "the fit command ticked the step");
    d.app.run(&ctx, Action::ZoomToFit);
    d.frame();
    d.frame();
    assert_eq!(step(&d), "pause", "a pause step waits for Next");
    d.app.learn_move(super::learn::Move::Next);
    d.frame();
    let run = d.app.learn.run.as_ref().unwrap();
    assert_eq!(run.phase(), novo_learn::Phase::Finished);
}

#[test]
fn a_step_does_not_tick_from_a_command_before_it_opened() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.start_lesson("tiny");
    d.app.run(&ctx, Action::ZoomToFit);
    d.frame();
    d.app.learn_move(super::learn::Move::Next);
    d.place("mcb3", 60.0, 60.0);
    d.frame();
    d.frame();
    assert_eq!(step(&d), "fit", "the earlier fit does not count");
}

#[test]
fn skip_back_and_restart_move_the_run() {
    let (_dir, mut d) = drive();
    d.app.start_lesson("tiny");
    d.frame();
    d.app.learn_move(super::learn::Move::Next);
    d.app.learn_move(super::learn::Move::Skip);
    assert_eq!(step(&d), "fit");
    d.app.learn_move(super::learn::Move::Back);
    assert_eq!(step(&d), "place");
    d.app.learn_restart();
    d.frame();
    assert_eq!(step(&d), "hello");
    assert_eq!(d.doc().project().info().name, "Lesson: Tiny lesson");
}

#[test]
fn progress_is_saved_and_survives_a_new_session() {
    let (dir, mut d) = drive();
    d.app.start_lesson("tiny");
    d.frame();
    d.app.learn_move(super::learn::Move::Next);
    let file = dir.path().join("learn.json");
    assert!(file.exists());
    let loaded = novo_learn::Progress::load(&file);
    let saved = loaded.of("tiny").unwrap();
    assert_eq!(saved.done, ["hello"]);
    assert_eq!(saved.current.as_deref(), Some("place"));
}

#[test]
fn opening_another_project_ends_the_lesson_but_keeps_its_progress() {
    let (dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.start_lesson("tiny");
    d.frame();
    d.app.learn_move(super::learn::Move::Next);
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.learn.run.is_none());
    let loaded = novo_learn::Progress::load(&dir.path().join("learn.json"));
    assert!(loaded.of("tiny").is_some());
}

#[test]
fn the_commands_drive_the_card() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "help.learn", Some("tiny"));
    d.frame();
    assert_eq!(step(&d), "hello", "help.learn with words starts the match");
    d.app.run_command(&ctx, "learn.next", None);
    assert_eq!(step(&d), "place");
    d.app.run_command(&ctx, "learn.back", None);
    assert_eq!(step(&d), "hello");
    d.app.run_command(&ctx, "learn.close", None);
    assert!(!d.app.learn.card_on());
    d.app.continue_lesson();
    assert!(d.app.learn.card_on());
}

#[test]
fn help_learn_without_words_opens_the_window() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::Learn);
    d.frame();
    assert!(d.app.learn.window);
    d.app.run(&ctx, Action::Learn);
    assert!(!d.app.learn.window);
}

#[test]
fn a_lesson_whose_project_is_missing_says_so_and_changes_nothing() {
    let (_dir, mut d) = drive();
    let mut files = files();
    files.insert(
        "tiny/lesson.toml".into(),
        LESSON.replace("project = \"@blank\"", "project = \"project.json\""),
    );
    d.app.learn.use_files(files);
    d.app.start_lesson("tiny");
    d.frame();
    assert!(d.app.learn.run.is_none());
    assert!(d.app.document().is_none());
}

#[test]
fn the_shipped_lessons_all_open() {
    // Every lesson in assets/lessons starts: its project loads and the
    // first step is there. (The per-step replay is a separate test.)
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let ids: Vec<String> = d
        .app
        .learn
        .catalog
        .entries
        .iter()
        .map(|e| e.lesson.id.clone())
        .collect();
    for id in ids {
        d.app.start_lesson(&id);
        d.frame();
        assert!(d.app.learn.run.is_some(), "lesson {id} did not start");
    }
}

const POINTING: &str = r#"
format = 1
id = "pointing"
version = 1
title = "Pointing"
summary = "Where the pointer goes."
track = "basics"
project = "@example"

[[step]]
id = "dock"
title = "The dock"
text = "Look at {ui:dock.place}."
point = { ui = "dock.place" }

[[step]]
id = "pin"
title = "A pin"
text = "Look at the pin."
point = { at = { device = "-K1", pin = "A1" } }

[[step]]
id = "device"
title = "A device"
text = "Look at the device."
point = { at = { device = "-F2" } }

[[step]]
id = "area"
title = "An area"
text = "Look at the area."
point = { area = { page = "1", x = 10.0, y = 10.0, w = 50.0, h = 40.0 } }
"#;

fn pointing() -> (tempfile::TempDir, Drive) {
    let (dir, mut d) = drive();
    let mut files = files();
    files.insert("pointing/lesson.toml".into(), POINTING.into());
    d.app.learn.use_files(files);
    (dir, d)
}

#[test]
fn controls_register_where_they_are_and_the_start_panel_offers_learn() {
    let (_dir, mut d) = drive();
    d.frame();
    d.frame();
    assert!(d.app.learn_anchor("start.learn").is_some());
    assert!(d.app.learn_anchor("menu.project").is_some());
    assert!(d.app.learn_anchor("palette.button").is_some());
    d.app.start_lesson("tiny");
    d.frame();
    d.frame();
    for id in ["dock.select", "dock.place", "dock.connect", "dock.corners"] {
        let rect = d.app.learn_anchor(id).unwrap_or_else(|| panic!("{id}"));
        assert!(rect.width() > 0.0 && rect.height() > 0.0, "{id}");
    }
}

#[test]
fn a_pointer_resolves_to_a_control_a_pin_a_device_and_an_area() {
    let (_dir, mut d) = pointing();
    d.app.start_lesson("pointing");
    d.frame();
    d.frame();
    let canvas = d.app.wb.canvas_area.unwrap().1;
    let steps: Vec<novo_learn::Target> = d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .lesson()
        .steps
        .iter()
        .filter_map(|s| s.point.clone())
        .collect();
    assert_eq!(steps.len(), 4);
    let dock = d.app.target_rect(&steps[0], canvas).expect("dock.place");
    assert_eq!(Some(dock), d.app.learn_anchor("dock.place"));
    assert!(d.app.learn_canvas_spot(&steps[1]).is_some(), "pin -K1:A1");
    assert!(d.app.learn_canvas_spot(&steps[2]).is_some(), "device -F2");
    let area = d.app.learn_canvas_spot(&steps[3]).expect("area on page 1");
    assert_eq!(area.max, (60.0, 50.0));
}

#[test]
fn the_card_words_come_from_the_ui_file_and_fall_back_to_the_key() {
    let (_dir, d) = drive();
    // ui.toml ships with the program.
    assert_eq!(d.app.learn.say("skip_step"), "Skip step");
    assert_eq!(
        d.app
            .learn
            .say_with("step_of", &[("n", "2"), ("total", "9")]),
        "2 of 9"
    );
    assert_eq!(d.app.learn.say("no_such_word"), "no_such_word");
}

/// `docs/design/lesson-anchors.md`, generated from the shell's lists so a
/// lesson author can see what exists. Run with `UPDATE_ANCHORS_DOC=1` to
/// rewrite it.
fn anchors_doc() -> String {
    let mut out = String::from(
        "# Lesson anchors and signals\n\n\
         Generated by `cargo test -p novo-app anchors_doc` from `ANCHORS` and `SIGNALS` in\n\
         `crates/novo-app/src/app/learn.rs`. Do not edit by hand: run the test with\n\
         `UPDATE_ANCHORS_DOC=1` after adding one.\n\n\
         A control a lesson may point at (`point = { ui = \"id\" }`, `{ui:id}` in text, `clicked` and\n\
         `opened` checks) is an anchor. Its owner registers it every frame it is on screen with\n\
         `NovoApp::anchor` or `anchor_response`. Ids keep their name when a panel moves.\n\n\
         ## Anchors\n\n",
    );
    for id in super::learn::ANCHORS {
        out.push_str(&format!("- `{id}`\n"));
    }
    out.push_str("\n## Signals\n\nFor `expect = { signal = \"name\" }`; raised where a key does not pass through a command.\n\n");
    for name in super::learn::SIGNALS {
        out.push_str(&format!("- `{name}`\n"));
    }
    out
}

#[test]
fn anchors_doc_is_current() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/design/lesson-anchors.md");
    let want = anchors_doc();
    if std::env::var_os("UPDATE_ANCHORS_DOC").is_some() {
        std::fs::write(&path, &want).unwrap();
    }
    // A Windows checkout may turn the file's line endings into CRLF.
    let have = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    assert_eq!(
        have, want,
        "docs/design/lesson-anchors.md is out of date: run `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`"
    );
}

#[test]
fn the_shipped_lessons_lint_without_errors_against_this_build() {
    // Ready lessons may only name commands, anchors and signals this build
    // has; drafts may be ahead of it and only warn.
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let report = d.app.learn.lint_report();
    let errors: Vec<String> = report.errors().map(ToString::to_string).collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn a_ready_lesson_naming_a_missing_anchor_is_coming_soon() {
    let (_dir, mut d) = drive();
    let mut files = files();
    files.insert(
        "tiny/lesson.toml".into(),
        LESSON.replace(
            "text = \"A reading step has a Next button.\"",
            "text = \"Look.\"\npoint = { ui = \"nowhere.at.all\" }",
        ),
    );
    d.app.learn.use_files(files);
    assert!(d.app.learn.is_coming_soon("tiny"));
}

#[test]
fn early_access_lessons_are_listed_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let d = Drive::new(&dir.path().join("untitled"));
    assert!(d.app.learn.show_drafts);
    assert!(
        d.app.learn.catalog.visible(true).count() > 0,
        "the build ships lessons"
    );
}
