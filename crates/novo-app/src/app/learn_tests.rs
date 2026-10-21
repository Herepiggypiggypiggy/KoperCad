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
    for id in [
        "dock.select",
        "dock.place",
        "dock.connect",
        "dock.corners",
        "dock.cable",
        "dock.undo",
        "dock.redo",
    ] {
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
        let name = super::learn_labels::anchor_label(id).unwrap_or("?");
        out.push_str(&format!("- `{id}`: {{ui:{id}}} shows **{name}**\n"));
    }
    out.push_str(
        "\n## Menus and their commands\n\n\
         Every menu of the bar and every command row in it is an anchor too, with no list to keep:\n\
         `{ui:menu.File}` is the menu in the bar and `{ui:cmd.export.sheets}` is the row of the command\n\
         with that id (`cmd.` + the command id). The card shows the row's own label. While the menu is\n\
         closed the ring points at the menu that holds the row.\n\n\
         ## Folded controls\n\n\
         A cabinet bar button that has folded into **More** (a narrow window) points at\n\
         `cabinet.more` while it is not on screen.\n",
    );
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
    assert!(!d.app.learn.show_drafts);
    assert!(
        d.app.learn.catalog.visible(false).count() > 0,
        "the build ships lessons"
    );
}

const DEMOING: &str = r#"
format = 1
id = "demoing"
version = 1
title = "Demoing"
summary = "Show me how."
track = "basics"
project = "@example"

[[step]]
id = "typed"
title = "Ask by name"
text = "Ask for a breaker."
point = { ui = "dock.place" }
demo = [
  { press = "tool.place" },
  { type = "mcb" },
  { key = "Enter" },
  { move = { ui = "dock.place" } },
  { click = true },
]

[[step]]
id = "derived"
title = "Place a relay"
text = "Place a relay."
expect = { count = { kind = "symbol", at_least = 999 } }
solve = [ { place = { symbol = "iec/coils/relay", page = "1", x = 80, y = 104 } } ]

[[step]]
id = "reading"
title = "Read"
text = "Nothing to show."
"#;

fn demoing() -> (tempfile::TempDir, Drive) {
    let (dir, mut d) = drive();
    let mut files = files();
    files.insert("demoing/lesson.toml".into(), DEMOING.into());
    d.app.learn.use_files(files);
    (dir, d)
}

#[test]
fn show_me_how_plays_the_steps_demonstration_and_the_card_lists_it() {
    let (_dir, mut d) = demoing();
    d.app.start_lesson("demoing");
    d.frame();
    d.frame();
    assert!(d.app.learn.demo.is_none(), "nothing plays until asked");
    assert!(d.app.learn_how_lines().is_empty());
    d.app.learn_show_me();
    d.frame();
    d.frame();
    assert!(d.app.learn.demo.is_some());
    let lines = d.app.learn_how_lines();
    // Press, type, key and click are lines; the move is not.
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert!(lines[0].0.starts_with("1. Press "), "{lines:?}");
    assert!(lines[1].0.contains("Type mcb"), "{lines:?}");
    assert!(lines[2].0.contains("Press Enter"), "{lines:?}");
    // The step changing ends it.
    d.app.learn_move(super::learn::Move::Skip);
    d.frame();
    d.frame();
    assert!(d.app.learn.demo.is_none());
}

#[test]
fn the_pointer_and_ghost_of_a_demonstration_land_on_screen() {
    let (_dir, mut d) = demoing();
    d.app.start_lesson("demoing");
    d.frame();
    d.frame();
    let canvas = d.app.wb.canvas_area.unwrap().1;
    let start = canvas.center();

    // The authored step: at its end the pointer is on the dock control.
    let step = d.app.learn.run.as_ref().unwrap().step().unwrap().clone();
    let demo = novo_learn::Demo::for_step(&step);
    let end = demo.frame(demo.duration() + 0.1);
    let scene = d.app.demo_scene(&end, canvas, start);
    let dock = d.app.learn_anchor("dock.place").unwrap();
    assert_eq!(scene.cursor, Some(dock.center()));

    // The derived step: a ghost on the page where the relay lands.
    d.app.learn_move(super::learn::Move::Skip);
    d.frame();
    d.frame();
    let step = d.app.learn.run.as_ref().unwrap().step().unwrap().clone();
    assert_eq!(step.id, "derived");
    let demo = novo_learn::Demo::for_step(&step);
    let end = demo.frame(demo.duration() + 0.1);
    let scene = d.app.demo_scene(&end, canvas, start);
    let (at, place, alpha) = scene.ghost.expect("a ghost");
    assert_eq!(place.symbol, "iec/coils/relay");
    assert_eq!(alpha, 1.0);
    assert!(at.x.is_finite() && at.y.is_finite());
    assert!(scene.cursor.is_some());
}

#[test]
fn a_reading_step_has_no_demonstration_to_start() {
    let (_dir, mut d) = demoing();
    d.app.start_lesson("demoing");
    d.frame();
    d.app.learn_move(super::learn::Move::Skip);
    d.app.learn_move(super::learn::Move::Skip);
    d.frame();
    d.frame();
    assert_eq!(step(&d), "reading");
    assert!(!d.app.learn_start_demo());
}

#[test]
fn a_feed_target_follows_the_feed_wherever_the_person_dropped_it() {
    let (_dir, mut d) = pointing();
    d.app.start_lesson("pointing");
    d.frame();
    d.frame();
    let canvas = d.app.wb.canvas_area.unwrap().1;
    // Find a feed the sample drawing has, by its own data.
    let (name, origin, page) = {
        let project = d.doc().project();
        let symbol = project
            .symbols()
            .find(|s| matches!(s.role, novo_core::SymbolRole::PotentialFeed { .. }))
            .expect("the example has a feed");
        let novo_core::SymbolRole::PotentialFeed { potential } = &symbol.role else {
            unreachable!()
        };
        let name = project.potential(*potential).unwrap().name.clone();
        (name, symbol.placement.origin, symbol.page)
    };
    d.app.show_page(page);
    d.frame();
    d.frame();
    let target = novo_learn::Target {
        at: Some(novo_learn::lesson::At {
            feed: Some(name),
            ..Default::default()
        }),
        ..Default::default()
    };
    let rect = d
        .app
        .target_rect(&target, canvas)
        .expect("the feed is on screen");
    // The box is the feed's own place on screen: its origin (the triangle)
    // is inside it, by the same mapping the canvas draws with.
    let viewport = d.app.canvas.schematic_ref().unwrap().canvas.viewport;
    let s = viewport.world_to_screen(novo_canvas::vec2(origin.x as f32, origin.y as f32));
    let at = canvas.min + eframe::egui::vec2(s.x, s.y);
    assert!(rect.contains(at), "{rect:?} should hold the feed at {at:?}");
    // And it moves with the feed: pan the view and the box follows.
    d.app.canvas.schematic().unwrap().canvas.viewport.center += novo_canvas::vec2(10.0, 0.0);
    d.frame();
    let moved = d.app.target_rect(&target, canvas).expect("still on screen");
    assert!(
        moved.center().x < rect.center().x,
        "panning right moves it left"
    );
}

#[test]
fn a_lesson_stays_whole_when_the_live_view_goes_on_and_off() {
    let (_dir, mut d) = drive();
    d.app.start_lesson("tiny");
    d.frame();
    d.frame();
    let card = super::panel::id("lesson_card");
    let height = |d: &Drive| d.ctx.memory(|m| m.area_rect(card)).unwrap().height();
    let before = height(&d);
    assert!(before > 60.0, "the full card has a step text: {before}");
    d.app.run_command(&d.ctx.clone(), "sim.live", None);
    d.frame();
    d.frame();
    assert!(d.app.wb.live.on);
    assert!(
        d.app.learn.card_on(),
        "the lesson is still running and shown"
    );
    assert_eq!(step(&d), "hello");
    assert!(
        (height(&d) - before).abs() < 1.0,
        "the card is not folded away"
    );
    d.app.run_command(&d.ctx.clone(), "sim.live", None);
    d.frame();
    assert!(!d.app.wb.live.on && d.app.learn.card_on());
}

#[test]
fn a_wire_demonstration_marks_where_the_wire_starts_and_ends() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    d.app.start_lesson("wiring-modes");
    d.frame();
    d.frame();
    if step(&d) == "intro" {
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "learn.next", None);
        d.frame();
    }
    assert_eq!(step(&d), "pin-drag");
    let canvas = d.app.wb.canvas_area.unwrap().1;
    assert!(d.app.learn_start_demo());
    let step = d.app.learn.run.as_ref().unwrap().step().unwrap().clone();
    let demo = novo_learn::Demo::for_step(&step);
    // Before the drag begins and after it ends, both ends are marked.
    for t in [0.0, demo.duration() + 0.1] {
        let scene = d.app.demo_scene(&demo.frame(t), canvas, canvas.center());
        let numbers: Vec<_> = scene.markers.iter().map(|m| (m.1, m.2)).collect();
        assert_eq!(numbers, [(1, false), (2, true)], "at {t}");
    }
    let scene = d
        .app
        .demo_scene(&demo.frame(demo.duration() + 0.1), canvas, canvas.center());
    // The markers sit on the pins named: -F1:2 and -S1:13.
    let pin = |device: &str, pin: &str| {
        let target = novo_learn::Target {
            at: Some(novo_learn::lesson::At {
                device: Some(device.into()),
                pin: Some(pin.into()),
                ..Default::default()
            }),
            ..Default::default()
        };
        d.app.target_rect(&target, canvas).unwrap().center()
    };
    assert_eq!(scene.markers[0].0, pin("-F1", "2"));
    assert_eq!(scene.markers[1].0, pin("-S1", "13"));
    assert_ne!(scene.markers[0].0, scene.markers[1].0);
    // The ghost wire joins them, in full.
    assert_eq!(scene.drag, Some((scene.markers[0].0, scene.markers[1].0)));
    // After the plays it settles on the markers and the wire, until Stop.
    d.app.learn.demo.as_mut().unwrap().started = Some(-1000.0);
    d.frame();
    d.frame();
    assert!(d.app.learn.demo.is_some(), "the markers stay");
    d.app.learn_stop_demo();
    assert!(d.app.learn.demo.is_none());
}

#[test]
fn every_anchor_has_a_name_for_ui_markup_and_no_name_is_orphaned() {
    use super::learn::ANCHORS;
    use super::learn_labels::{ANCHOR_LABELS, anchor_label};
    for id in ANCHORS {
        assert!(
            anchor_label(id).is_some(),
            "anchor `{id}` has no name in learn_labels.rs (what `{{ui:{id}}}` shows)"
        );
    }
    for (id, _) in ANCHOR_LABELS {
        assert!(
            ANCHORS.contains(id),
            "learn_labels.rs names `{id}`, which is not in ANCHORS"
        );
    }
}

/// `docs/design/lesson-ui-names.md`: control names lessons still write as
/// plain bold text. The list may only shrink: a lesson writes a control with
/// `{ui:anchor.id}` so the name is the one on screen and the highlight can
/// point at it. Run with `UPDATE_UI_NAMES=1` to rewrite it after fixing some.
fn ui_names_now() -> Vec<String> {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let mut lines = Vec::new();
    for entry in &d.app.learn.catalog.entries {
        for name in novo_learn::lint::ui_names(&entry.lesson, super::learn_labels::ANCHOR_LABELS) {
            let hint = name
                .anchor
                .map(|a| format!(" (write {{ui:{a}}})"))
                .unwrap_or_default();
            lines.push(format!(
                "- {}/{}: `{}`{hint}",
                name.lesson, name.step, name.phrase
            ));
        }
    }
    lines.sort();
    lines
}

#[test]
fn lessons_name_no_new_control_as_plain_text() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/design/lesson-ui-names.md");
    let now = ui_names_now();
    if std::env::var_os("UPDATE_UI_NAMES").is_some() {
        let mut out = String::from(
            "# Control names written as plain text\n\n\
             Generated by `UPDATE_UI_NAMES=1 cargo test -p novo-app lessons_name_no_new_control`.\n\
             A step names a button, menu or panel with `{ui:anchor.id}` (the id from\n\
             `lesson-anchors.md`; register one with `NovoApp::anchor` and a line in\n\
             `learn_labels.rs`), not as `**bold**` text, so the name is the one on screen.\n\
             These steps still do; the test fails when a step adds one that is not listed.\n\
             Fix a step, then regenerate to shrink the list.\n\n",
        );
        for line in &now {
            out.push_str(line);
            out.push('\n');
        }
        std::fs::write(&path, out).unwrap();
    }
    let have = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    let listed: std::collections::BTreeSet<&str> = have.lines().collect();
    let new: Vec<&String> = now
        .iter()
        .filter(|l| !listed.contains(l.as_str()))
        .collect();
    assert!(
        new.is_empty(),
        "control names written as plain text; use {{ui:id}} (register an anchor if there is none):\n{}",
        new.iter()
            .map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn menu_rows_are_anchors_that_fall_back_to_their_menu() {
    use super::learn_labels::{anchor_fallback, any_anchor_label, menu_anchors};
    let rows: Vec<_> = menu_anchors()
        .iter()
        .filter(|m| m.id.starts_with("cmd."))
        .collect();
    assert!(rows.len() > 20, "{} command rows", rows.len());
    for row in rows {
        let parent = anchor_fallback(&row.id).expect("a row points at its menu");
        assert!(parent.starts_with("menu."), "{} -> {parent}", row.id);
        assert!(anchor_fallback(&parent).is_none());
        assert_eq!(any_anchor_label(&row.id), Some(row.label.clone()));
    }
    assert_eq!(
        anchor_fallback("cabinet.heat").as_deref(),
        Some("cabinet.more")
    );
    assert!(anchor_fallback("cabinet.more").is_none());
    // The lint knows them: a lesson may write {ui:cmd.file.save}.
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let known = |id: &str| d.app.learn.lint_registry().anchors.contains(id);
    assert!(known("menu.File"));
    assert!(known(&format!(
        "cmd.{}",
        novo_workflow::menus::layout("File")
            .iter()
            .find_map(|i| match i {
                novo_workflow::menus::MenuItem::Command(c) => Some(c.id),
                _ => None,
            })
            .unwrap()
    )));
}

#[test]
fn help_next_lesson_names_and_starts_the_lesson_on_offer() {
    let (_dir, mut d) = drive();
    let ctx = d.ctx.clone();
    let info = novo_workflow::commands::get("help.next_lesson").expect("registered");
    assert_eq!(d.app.command_label(info), "Next lesson: Tiny lesson");
    assert!(crate::app::app_runs("help.next_lesson"));
    d.app.run_command(&ctx, "help.next_lesson", None);
    d.frame();
    assert_eq!(step(&d), "hello", "it starts the lesson");
    assert_eq!(d.app.command_label(info), "Continue lesson: Tiny lesson");
    d.app.run_command(&ctx, "learn.close", None);
    d.app.run_command(&ctx, "help.next_lesson", None);
    assert!(d.app.learn.card_on(), "it brings the card back");
    assert_eq!(step(&d), "hello", "and does not start over");
}

#[test]
fn the_window_lists_a_chosen_path_in_its_order() {
    let (_dir, mut d) = drive();
    let mut files = files();
    files.insert(
        "paths.toml".into(),
        "[[path]]\nid = \"only\"\ntitle = \"Only path\"\nlessons = [\"tiny\"]\n".into(),
    );
    d.app.learn.use_files(files);
    let entries: Vec<_> = d.app.learn.catalog.visible(false).cloned().collect();
    let groups = d.app.lesson_groups(&entries);
    assert_eq!(groups[0].0, "Basics", "tracks by default");
    d.app.learn.only_path = Some("only".into());
    let groups = d.app.lesson_groups(&entries);
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].0, "Only path");
    assert_eq!(groups[0].2[0].lesson.id, "tiny");
    d.app.learn.window = true;
    d.frame();
    d.app.run_command(&d.ctx.clone(), "help.learn", None);
    assert!(d.app.learn.only_path.is_none(), "opening plainly clears it");
}

#[test]
fn the_command_field_finds_and_starts_a_lesson() {
    let (_dir, mut d) = drive();
    let entries = d.app.learn.palette_entries();
    assert_eq!(entries[0].id, "tiny");
    assert_eq!(entries[0].title, "Tiny lesson");
    let ctx = d.ctx.clone();
    d.app
        .run_target(&ctx, novo_workflow::palette::Target::Lesson("tiny".into()));
    d.frame();
    assert_eq!(step(&d), "hello", "picking the row starts the lesson");
    d.app
        .run_target(&ctx, novo_workflow::palette::Target::Lesson("tiny".into()));
    assert_eq!(step(&d), "hello", "picking it again resumes, not restarts");
}

#[test]
fn a_panel_chip_names_the_lesson_for_its_topic_and_steps_aside_when_it_runs() {
    let (_dir, mut d) = drive();
    let mut files = files();
    files.insert(
        "tiny/lesson.toml".into(),
        LESSON.replace(
            "project = \"@blank\"",
            "project = \"@blank\"\ntopics = [\"terminals\"]",
        ),
    );
    d.app.learn.use_files(files);
    assert!(d.app.learn.chip("cables").is_none(), "no lesson for it");
    let (id, label) = d.app.learn.chip("terminals").expect("a chip");
    assert_eq!(
        (id.as_str(), label.as_str()),
        ("tiny", "Lesson: Tiny lesson")
    );
    d.app.learn_run_command_lesson(&id);
    d.frame();
    assert!(d.app.learn.chip("terminals").is_none(), "the card is on it");
}

#[test]
fn reset_progress_waits_for_the_second_click() {
    let (_dir, mut d) = drive();
    d.app.learn.window = true;
    d.app.start_lesson("tiny");
    d.frame();
    d.app.learn_move(super::learn::Move::Next);
    d.app.learn.confirm_reset = true;
    for _ in 0..3 {
        d.frame();
    }
    assert!(
        d.app.learn.progress().of("tiny").is_some(),
        "drawing the question changes nothing"
    );
    d.app.learn.reset_progress();
    assert!(d.app.learn.progress().of("tiny").is_none());
    assert!(!d.app.learn.confirm_reset, "the question closes");
}

#[test]
fn open_finished_version_opens_the_finished_drawing_on_the_last_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.start_lesson("placing-parts");
    d.frame();
    assert!(
        !d.app.card().unwrap().can_open_finished,
        "not offered on the first step"
    );
    let steps = d.app.learn.run.as_ref().unwrap().progress().1;
    for _ in 1..steps {
        d.app.learn_move(super::learn::Move::Skip);
    }
    d.frame();
    assert!(
        d.app.card().unwrap().can_open_finished,
        "offered at the end"
    );
    assert!(d.app.learn_open_finished());
    d.frame();
    assert!(d.app.learn.run.is_none(), "the lesson ends");
    assert!(
        d.doc().project().info().name.starts_with("Finished:"),
        "{}",
        d.doc().project().info().name
    );
    assert!(d.doc().project().symbols().count() > 0);
}

#[test]
fn the_inspector_keeps_above_the_lesson_card_at_1366() {
    let (_dir, mut d) = drive();
    d.app.start_lesson("tiny");
    let size = eframe::egui::vec2(1366.0, 768.0);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    let k = d.place("km3", 80.0, 96.0);
    d.editor_leave();
    d.select(k);
    for _ in 0..6 {
        d.frame_sized(size, Vec::new());
    }
    let rect = |name: &str| {
        d.ctx
            .memory(|m| m.area_rect(super::panel::id(name)))
            .unwrap_or_else(|| panic!("{name} is not shown"))
    };
    let (card, inspector) = (rect("lesson_card"), rect("inspector"));
    assert!(
        inspector.max.y <= card.min.y || inspector.max.x <= card.min.x,
        "the inspector {inspector:?} covers the lesson card {card:?}"
    );
}

#[test]
fn a_challenge_gives_its_hints_one_at_a_time_and_is_solved_by_the_fix() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.learn.advance_after = std::time::Duration::ZERO;
    d.app.start_lesson("motor-wont-hold");
    d.frame();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.live", None);
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(step(&d), "hold", "the goal step is open");
    let card = d.app.card().unwrap();
    assert!(card.challenge);
    assert_eq!((card.hints_open.len(), card.hints_total), (0, 3));
    d.app.learn_open_hint();
    d.app.learn_open_hint();
    let card = d.app.card().unwrap();
    assert_eq!(card.hints_open.len(), 2, "two of three hints are open");
    for _ in 0..5 {
        d.app.learn_open_hint();
    }
    assert_eq!(d.app.card().unwrap().hints_open.len(), 3);
    // The fault is real: pressing start without repairing anything does not
    // satisfy the goal; with the repair (the step's whole solve) it does.
    let solve = d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .step()
        .unwrap()
        .solve
        .clone();
    d.replay_op(&solve[1]).unwrap();
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(step(&d), "hold", "start alone drops out again");
    d.replay_op(&solve[0]).unwrap();
    d.replay_op(&solve[1]).unwrap();
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(step(&d), "stop", "the repaired holding contact holds");
}

#[test]
fn the_broken_contactor_challenge_only_ticks_after_the_wire_is_drawn() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.learn.advance_after = std::time::Duration::ZERO;
    d.app.start_lesson("coil-wont-pull-in");
    d.frame();
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, "sim.live", None);
    for _ in 0..4 {
        d.frame();
    }
    assert_eq!(step(&d), "pull-in");
    let solve = d
        .app
        .learn
        .run
        .as_ref()
        .unwrap()
        .step()
        .unwrap()
        .solve
        .clone();
    d.replay_op(&solve[1]).unwrap();
    for _ in 0..3 {
        d.frame();
    }
    assert_eq!(
        step(&d),
        "pull-in",
        "start does nothing while the wire is missing"
    );
    d.replay_op(&solve[0]).unwrap();
    d.replay_op(&solve[1]).unwrap();
    for _ in 0..3 {
        d.frame();
    }
    assert_ne!(step(&d), "pull-in", "with the wire the coil pulls in");
}
