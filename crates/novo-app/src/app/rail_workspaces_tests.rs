//! The 3D and Cabinet rails (lanes V3b and V3c): the rows are real, the rail
//! fits and leaves the canvas furniture alone, and a click runs the command
//! the row names with no state of its own.

use eframe::egui;

use super::gate1_tests::Drive;
use super::rail::{self, MAX_PER_ZONE};
use super::rail_3d::{self, ALIGN_FLYOUT, HIDE_FLYOUT};
use super::rail_cabinet;
use super::rail_schematic::{HISTORY, look};
use super::view3d_options::ViewOption;
use super::view3d_tests::drive_with_cabinet;
use super::workspace::Workspace;

const SIZES: [egui::Vec2; 2] = [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)];

fn go(d: &mut Drive, size: egui::Vec2, id: &'static str) {
    let ctx = d.ctx.clone();
    d.app.run_command(&ctx, id, None);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
}

#[test]
fn every_row_is_real_and_a_zone_holds_at_most_six() {
    let zones: [(&str, &[&str]); 6] = [
        ("3d tools", rail_3d::TOOLS),
        ("3d assists", rail_3d::ASSISTS),
        ("3d panels", rail_3d::PANELS),
        ("cabinet tools", rail_cabinet::TOOLS),
        ("cabinet assists", rail_cabinet::ASSISTS),
        ("cabinet panels", rail_cabinet::PANELS),
    ];
    for (name, zone) in zones {
        assert!(zone.len() <= MAX_PER_ZONE, "{name} has {}", zone.len());
        let mut seen = std::collections::HashSet::new();
        for &id in zone {
            assert!(seen.insert(id), "{name}: {id} twice");
            let (_, label, _) = look(id).unwrap_or_else(|| panic!("{name}: {id} has no look"));
            assert!(label.chars().count() <= novo_theme::presentation::MAX_SHORT);
            if !id.starts_with("rail.") {
                assert!(
                    novo_workflow::commands::is_known(id),
                    "{name}: {id} is not a command"
                );
                assert!(
                    super::workbench::app_runs(id),
                    "{name}: {id} does not run from a click"
                );
            }
        }
    }
    for &id in ALIGN_FLYOUT.iter().chain(HIDE_FLYOUT) {
        assert!(look(id).is_some(), "{id} has no look");
        assert!(novo_workflow::commands::is_known(id), "{id}");
    }
}

#[test]
fn both_rails_fit_both_screen_sizes_and_leave_the_canvas_furniture_alone() {
    for size in SIZES {
        let dir = tempfile::tempdir().unwrap();
        let mut d = drive_with_cabinet(dir.path());
        for (workspace, id) in [
            (Workspace::Cabinet, "view.cabinet"),
            (Workspace::View3d, "view.3d"),
        ] {
            go(&mut d, size, id);
            assert_eq!(d.app.workspace(), workspace);
            let rail_rect = d.app.wb.rail_rect.expect("the rail is shown");
            let (_, canvas) = d.app.wb.canvas_area.unwrap();
            assert!(rail_rect.left() <= 1.0, "{rail_rect:?}");
            assert!(rail_rect.right() <= canvas.left() + 1.0);
            assert!((rail_rect.width() - rail::WIDTH).abs() < 2.0);
            let rows = match workspace {
                Workspace::View3d => {
                    rail_3d::TOOLS.len() + rail_3d::ASSISTS.len() + rail_3d::PANELS.len()
                }
                _ => {
                    rail_cabinet::TOOLS.len()
                        + rail_cabinet::ASSISTS.len()
                        + rail_cabinet::PANELS.len()
                }
            } + HISTORY.len()
                + 1;
            let need = rows as f32 * (rail::ENTRY + 2.0) + 3.0 * 9.0 + 12.0;
            assert!(
                need <= rail_rect.height(),
                "{workspace:?} {size:?}: needs {need} of {}",
                rail_rect.height()
            );
        }
        // The 3D toolbar, which holds the status chip, starts right of the rail.
        let rail_right = d.app.wb.rail_rect.unwrap().right();
        let toolbar = d
            .ctx
            .memory(|m| m.area_rect(super::panel::id("view3d_toolbar")))
            .expect("the toolbar is drawn");
        assert!(toolbar.left() >= rail_right, "{toolbar:?} under the rail");
    }
}

#[test]
fn a_3d_row_runs_its_command_and_shows_the_one_state() {
    let size = SIZES[0];
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    go(&mut d, size, "view.3d");
    let ctx = d.ctx.clone();
    assert!(
        d.app
            .view3d_rail()
            .tools
            .iter()
            .find(|e| e.id == "rail.select")
            .unwrap()
            .active
    );
    d.app.rail_3d_click(&ctx, "view.3d_measure");
    d.frame_sized(size, Vec::new());
    d.frame_sized(size, Vec::new());
    assert!(d.app.view3d_option(ViewOption::Measure));
    let rail = d.app.view3d_rail();
    let measure = rail.assists.iter().find(|e| e.id == "view.3d_measure");
    assert!(measure.unwrap().active, "the row follows the option");
    // Select puts the pointer back from Measure and Move.
    d.app.rail_3d_click(&ctx, "rail.select");
    assert!(!d.app.view3d_option(ViewOption::Measure));
    // The same option flipped by its command shows on the row too.
    go(&mut d, size, "view.3d_move");
    let rail = d.app.view3d_rail();
    assert!(
        rail.tools
            .iter()
            .find(|e| e.id == "view.3d_move")
            .unwrap()
            .active
    );
    assert!(
        !rail
            .tools
            .iter()
            .find(|e| e.id == "rail.select")
            .unwrap()
            .active
    );
    // Align opens its flyout with the eight rows.
    d.app.rail_3d_click(&ctx, "rail.align");
    d.frame_sized(size, Vec::new());
    assert_eq!(rail::open_flyout(&ctx).as_deref(), Some("rail.align"));
    let rail = d.app.view3d_rail();
    let align = rail.tools.iter().find(|e| e.id == "rail.align").unwrap();
    assert_eq!(align.flyout.len(), ALIGN_FLYOUT.len());
    let hide = rail.assists.iter().find(|e| e.id == "rail.hide").unwrap();
    assert_eq!(hide.flyout.len(), HIDE_FLYOUT.len());
}

#[test]
fn a_cabinet_row_runs_its_command_and_follows_the_state() {
    let size = SIZES[0];
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    go(&mut d, size, "view.cabinet");
    let ctx = d.ctx.clone();
    assert!(!d.app.cabinet.rails.open);
    d.app.rail_cabinet_click(&ctx, "cabinet.rails");
    d.frame_sized(size, Vec::new());
    d.frame_sized(size, Vec::new());
    assert!(d.app.cabinet.rails.open);
    let rail = d.app.cabinet_rail();
    assert!(
        rail.tools
            .iter()
            .find(|e| e.id == "cabinet.rails")
            .unwrap()
            .active
    );
    d.app.rail_cabinet_click(&ctx, "view.cabinet_table");
    d.frame_sized(size, Vec::new());
    d.frame_sized(size, Vec::new());
    assert!(d.app.cabinet.table);
    let rail = d.app.cabinet_rail();
    assert!(
        rail.panels
            .iter()
            .find(|e| e.id == "view.cabinet_table")
            .unwrap()
            .active
    );
    // Pages opens the page list like the Schematic rail's row.
    d.app.rail_cabinet_click(&ctx, "rail.pages");
    assert!(d.app.rail_panel_open("rail.pages"));
}

#[test]
fn reports_and_commissioning_have_their_rail() {
    let size = SIZES[0];
    let dir = tempfile::tempdir().unwrap();
    let mut d = drive_with_cabinet(dir.path());
    go(&mut d, size, "view.3d");
    assert!(d.app.wb.rail_rect.is_some());
    go(&mut d, size, "view.reports");
    assert!(d.app.wb.rail_rect.is_some(), "the Reports rail");
    go(&mut d, size, "view.commissioning");
    assert!(d.app.wb.rail_rect.is_some(), "the Commissioning rail");
}

/// `homes::RELOCATED` says which commands a rail is the home of. The rails
/// decide what they draw, so the two lists must agree both ways: a command
/// the table sends to a rail is drawn on one, and a command that a rail
/// draws and that has no menu row says so in the table.
#[test]
fn the_relocated_table_matches_what_the_rails_draw() {
    use super::{rail_reports as rr, rail_schematic as rs};
    use novo_workflow::homes::{Home, RELOCATED};
    let drawn: std::collections::HashSet<&str> = [
        rs::TOOLS,
        rs::ASSISTS,
        rs::PANELS,
        rs::HISTORY,
        rs::CONNECT_FLYOUT,
        rail_3d::TOOLS,
        rail_3d::ASSISTS,
        rail_3d::PANELS,
        rail_3d::ALIGN_FLYOUT,
        rail_3d::HIDE_FLYOUT,
        rail_cabinet::TOOLS,
        rail_cabinet::ASSISTS,
        rail_cabinet::PANELS,
        rr::REPORTS_TOOLS,
        rr::REPORTS_ASSISTS,
        rr::REPORTS_PANELS,
        rr::EXPORT_FLYOUT,
        rr::COMMISSIONING_TOOLS,
        rr::COMMISSIONING_ASSISTS,
        rr::COMMISSIONING_PANELS,
    ]
    .into_iter()
    .flatten()
    .copied()
    .collect();
    for (id, home) in RELOCATED {
        if *home == Home::Rail {
            assert!(
                drawn.contains(id),
                "{id} is a Rail home but no rail draws it"
            );
        }
    }
    for id in &drawn {
        if id.starts_with("rail.") {
            continue;
        }
        let homes = novo_workflow::homes::homes(id);
        assert!(!homes.is_empty(), "{id} is drawn on a rail and has no home");
    }
}
