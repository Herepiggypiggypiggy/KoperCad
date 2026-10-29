//! The add-on manager through the whole window, headless: the command opens a window that fits
//! the screen at both sizes, an installed add-on adds its rows to the command search and takes
//! them away when turned off, and a refused file shows why.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_workflow::palette::{Mode, Target};

use super::gate1_tests::Drive;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../qa/fixtures/addons")
        .join(name)
}

fn drive() -> (Drive, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.init_addons_at(dir.path().join("addons"));
    (d, dir)
}

fn rows(d: &mut Drive, query: &str) -> Vec<novo_workflow::palette::Row> {
    d.app.wb.field.open(Mode::All, None);
    d.app.wb.field.type_text(query);
    let rows = d.app.with_field(|field, sources| field.rows(sources));
    d.app.wb.field.close();
    rows
}

#[test]
fn nothing_is_installed_at_first_and_the_command_opens_the_manager() {
    let (mut d, _dir) = drive();
    assert!(!d.app.addon_enabled("demo-tools"));
    assert!(d.app.wb.addons.ext.commands.is_empty());
    let found = rows(&mut d, "add-ons");
    assert!(
        found
            .iter()
            .any(|r| r.target == Target::Command("addons.manage")),
        "Ctrl+K finds Add-ons"
    );
    let ctx = d.ctx.clone();
    assert!(d.app.type_command(&ctx, Mode::All, "add-ons"));
    d.frame();
    assert!(d.app.wb.addons.manager.is_some());
}

#[test]
fn the_manager_fits_the_screen_at_both_sizes() {
    for (w, h) in [(1366.0, 768.0), (1920.0, 1080.0)] {
        let (mut d, _dir) = drive();
        d.app.open_addons();
        d.frame_sized(egui::vec2(w, h), Vec::new());
        d.frame_sized(egui::vec2(w, h), Vec::new());
        let rect = d
            .ctx
            .memory(|m| m.area_rect(super::panel::id("addons")))
            .expect("the manager is on screen");
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, h));
        assert!(screen.contains_rect(rect), "{rect:?} inside {screen:?}");
        assert!(rect.width() >= 600.0 && rect.height() >= 400.0, "{rect:?}");
    }
}

#[test]
fn installing_adds_rows_and_turning_off_takes_them_away() {
    let (mut d, dir) = drive();
    d.app.begin_install(fixture("demo-tools-0.1.0.kcaddon"));
    d.frame();
    let prompt = d
        .app
        .wb
        .addons
        .manager
        .as_ref()
        .and_then(|m| m.prompt.as_ref());
    assert!(prompt.is_some(), "the install prompt is shown");
    assert!(prompt.unwrap().inspection.needs_trust_tick());
    // an unsigned file needs the tick: the store itself refuses without it
    let store = d.app.wb.addons.store.clone().unwrap();
    assert!(
        store
            .install_file(&fixture("demo-tools-0.1.0.kcaddon"), Default::default())
            .is_err()
    );
    store
        .install_file(
            &fixture("demo-tools-0.1.0.kcaddon"),
            novo_addons::InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    d.app.refresh_addons();
    d.frame();

    assert!(d.app.addon_enabled("demo-tools"));
    let found = rows(&mut d, "demo status");
    let hit = found
        .iter()
        .find(|r| r.target == Target::Addon("addon.demo-tools.status".into()))
        .expect("the command search finds the add-on's command");
    assert!(hit.detail.contains("Add-on: Demo tools"), "{}", hit.detail);
    // the palette-only command is found too; the rail one is not a menu row
    assert!(
        rows(&mut d, "search-only")
            .iter()
            .any(|r| r.target == Target::Addon("addon.demo-tools.search-only".into()))
    );
    assert_eq!(d.app.wb.addons.ext.menu_rows("Simulate").len(), 1);

    // running it opens its panel
    let ctx = d.ctx.clone();
    assert!(d.app.type_command(&ctx, Mode::All, "demo status panel"));
    assert!(
        d.app
            .wb
            .addons
            .open_panels
            .contains("addon.demo-tools.status")
    );
    d.frame();
    assert!(
        d.ctx
            .memory(|m| m.area_rect(super::panel::id("addon.demo-tools.status")))
            .is_some(),
        "the panel is drawn"
    );

    // off: the rows and the panel go
    store.set_enabled("demo-tools", false).unwrap();
    d.app.refresh_addons();
    d.frame();
    assert!(!d.app.addon_enabled("demo-tools"));
    assert!(
        !rows(&mut d, "demo status")
            .iter()
            .any(|r| matches!(&r.target, Target::Addon(_)))
    );
    assert!(d.app.wb.addons.ext.commands.is_empty());
    assert!(d.app.wb.addons.open_panels.is_empty());
    drop(dir);
}

#[test]
fn a_file_that_needs_a_newer_program_is_refused_in_words() {
    let (mut d, _dir) = drive();
    d.app.begin_install(fixture("future-addon-1.0.0.kcaddon"));
    let m = d.app.wb.addons.manager.as_ref().unwrap();
    assert!(m.prompt.is_none());
    let (_, text) = m.notice.as_ref().expect("a notice says why");
    assert!(text.contains("needs KoperCAD 9.0.0 or newer"), "{text}");
}

#[test]
fn a_file_that_is_not_an_add_on_is_refused_in_words() {
    let (mut d, dir) = drive();
    let junk = dir.path().join("junk.kcaddon");
    std::fs::write(&junk, b"this is not a zip").unwrap();
    d.app.begin_install(junk);
    let m = d.app.wb.addons.manager.as_ref().unwrap();
    assert!(m.prompt.is_none());
    assert!(
        m.notice
            .as_ref()
            .unwrap()
            .1
            .contains("not a readable add-on")
    );
}

fn install_engine_fixture(d: &mut Drive) {
    let store = d.app.wb.addons.store.clone().unwrap();
    store
        .install_file(
            &fixture("demo-engine-0.1.0.kcaddon"),
            novo_addons::InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    d.app.refresh_addons();
    d.frame();
}

#[test]
fn an_engine_answers_buttons_and_commands() {
    let (mut d, _dir) = drive();
    install_engine_fixture(&mut d);
    assert!(d.app.addon_enabled("demo-engine"));
    // a button press goes to the engine and its lamp answer is kept
    let reply = d
        .app
        .addon_engine_call(
            "demo-engine",
            "button",
            &serde_json::json!({"panel": "panel", "id": "start"}),
        )
        .expect("it has an engine")
        .expect("the engine answers");
    d.app.apply_engine_reply("demo-engine", &reply);
    let key = ("demo-engine".to_owned(), "power".to_owned());
    assert_eq!(
        d.app.wb.addons.lamps.get(&key).map(|l| l.1),
        Some(Some(true)),
        "the Power lamp is lit"
    );
    // a command without a panel goes to the engine too
    d.app.run_addon_command("addon.demo-engine.ping");
    assert!(d.app.wb.addons.stopped.is_empty());
    // a request the engine does not know is a refusal in its words, not a stop
    let refused = d
        .app
        .addon_engine_call("demo-engine", "dance", &serde_json::json!({}))
        .unwrap();
    assert_eq!(refused, Err("unknown request".to_owned()));
    assert!(d.app.wb.addons.stopped.is_empty());
}

#[test]
fn lamps_an_engine_names_but_no_panel_declares_are_not_kept() {
    let (mut d, _dir) = drive();
    install_engine_fixture(&mut d);
    let reply = serde_json::json!({"lamps": {
        "power": {"lit": true},
        "not-in-any-panel": {"lit": true},
        "x2": {"lit": false},
    }});
    d.app.apply_engine_reply("demo-engine", &reply);
    let kept: Vec<_> = d
        .app
        .wb
        .addons
        .lamps
        .keys()
        .map(|(_, lamp)| lamp.as_str())
        .collect();
    assert_eq!(kept, ["power"]);
}

#[test]
fn a_broken_engine_stops_alone_and_takes_its_rows_with_it() {
    let (mut d, _dir) = drive();
    install_engine_fixture(&mut d);
    let store = d.app.wb.addons.store.clone().unwrap();
    store
        .install_file(
            &fixture("demo-tools-0.1.0.kcaddon"),
            novo_addons::InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    d.app.refresh_addons();
    assert!(
        d.app
            .wb
            .addons
            .ext
            .commands
            .iter()
            .any(|c| c.addon_id == "demo-engine")
    );
    d.app.run_addon_command("addon.demo-engine.crash");
    d.frame();
    let why = d
        .app
        .wb
        .addons
        .stopped
        .get("demo-engine")
        .expect("the engine stopped");
    assert!(!why.is_empty());
    // its commands and panels are gone from the tables, the other add-on is untouched
    assert!(
        !d.app
            .wb
            .addons
            .ext
            .commands
            .iter()
            .any(|c| c.addon_id == "demo-engine")
    );
    assert!(
        d.app
            .wb
            .addons
            .ext
            .panel("addon.demo-tools.status")
            .is_some()
    );
    // and it answers nothing more this run
    let again = d
        .app
        .addon_engine_call("demo-engine", "hello", &serde_json::json!({}))
        .unwrap();
    assert!(again.is_err());
}

/// A stand-in for the controller package: only the id matters to the gate.
fn install_controllers_stand_in(d: &mut Drive) {
    let manifest = "[addon]\nid = \"controllers\"\nname = \"Controllers\"\nversion = \"0.1.0\"\npublisher = \"test\"\ndescription = \"stand-in\"\nrequires_app = \">=0.1.0\"\napi_version = 1\npermissions = []\n";
    let bytes = novo_addons::pack::pack_parts(manifest.as_bytes(), &[], None).expect("packs");
    d.app
        .wb
        .addons
        .store
        .clone()
        .unwrap()
        .install_bytes(
            &bytes,
            novo_addons::InstallOptions {
                accept_unsigned: true,
                enable: true,
            },
        )
        .unwrap();
    d.app.refresh_addons();
    d.frame();
}

#[test]
fn the_controller_import_wizard_needs_the_controllers_add_on() {
    let (mut d, _dir) = drive();
    let ctx = d.ctx.clone();
    let info = novo_workflow::commands::get("file.import_controller").unwrap();
    // without the add-on the menu row is greyed with the reason, and the command (Ctrl+K,
    // scripts) says so instead of opening the wizard
    assert_eq!(
        d.app.command_unavailable(info),
        Some("Needs the Controllers add-on: install and turn it on in File > Add-ons...")
    );
    d.app.run_command(&ctx, "file.import_controller", None);
    d.frame();
    assert!(d.app.wb.controller_wizard.is_none());
    // with it on, the row works and the wizard opens
    install_controllers_stand_in(&mut d);
    assert_eq!(d.app.command_unavailable(info), None);
    d.app.run_command(&ctx, "file.import_controller", None);
    d.frame();
    assert!(d.app.wb.controller_wizard.is_some());
    // turned off again, it is gone
    let store = d.app.wb.addons.store.clone().unwrap();
    store.set_enabled("controllers", false).unwrap();
    d.app.refresh_addons();
    assert!(d.app.command_unavailable(info).is_some());
}
