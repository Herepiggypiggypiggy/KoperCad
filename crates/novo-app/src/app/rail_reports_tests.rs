//! The Reports and Commissioning rails: every row is real, the rail fits,
//! it is on screen only in its workspace, and a row does what it says.

use eframe::egui;

use super::commissioning::Tab;
use super::gate1_tests::Drive;
use super::rail::{self, MAX_PER_ZONE};
use super::rail_reports::{
    COMMISSIONING_ASSISTS, COMMISSIONING_PANELS, COMMISSIONING_TOOLS, EXPORT_FLYOUT,
    REPORTS_ASSISTS, REPORTS_PANELS, REPORTS_TOOLS,
};
use super::rail_schematic::look;
use super::workspace::Workspace;
use crate::actions::Action;

fn example(size: egui::Vec2, workspace: Workspace) -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame_sized(size, Vec::new());
    d.app.switch_workspace(workspace);
    for _ in 0..4 {
        d.frame_sized(size, Vec::new());
    }
    (dir, d)
}

fn click(d: &mut Drive, size: egui::Vec2, at: egui::Pos2) {
    let ev = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_sized(size, vec![egui::Event::PointerMoved(at)]);
    d.frame_sized(size, Vec::new());
    d.frame_sized(size, vec![ev(true)]);
    d.frame_sized(size, vec![ev(false)]);
    d.frame_sized(size, Vec::new());
}

#[test]
fn every_row_is_real_and_a_zone_holds_at_most_six() {
    for zone in [
        REPORTS_TOOLS,
        REPORTS_ASSISTS,
        REPORTS_PANELS,
        EXPORT_FLYOUT,
        COMMISSIONING_TOOLS,
        COMMISSIONING_ASSISTS,
        COMMISSIONING_PANELS,
    ] {
        assert!(zone.len() <= MAX_PER_ZONE);
        let mut seen = std::collections::HashSet::new();
        for &id in zone {
            assert!(seen.insert(id), "{id} twice");
            let (_, label, _) = look(id).unwrap_or_else(|| panic!("{id} has no look"));
            assert!(label.chars().count() <= novo_theme::presentation::MAX_SHORT);
            if !id.starts_with("rail.") {
                assert!(novo_workflow::commands::is_known(id), "{id}");
            }
        }
    }
}

#[test]
fn each_workspace_has_its_rail() {
    let size = egui::vec2(1366.0, 768.0);
    for workspace in [Workspace::Reports, Workspace::Commissioning] {
        let (_dir, d) = example(size, workspace);
        let rect = d.app.wb.rail_rect.expect("the rail is shown");
        assert!(rect.left() <= 1.0 && (rect.width() - rail::WIDTH).abs() < 2.0);
        let rows = d.app.workspace_rail(
            if workspace == Workspace::Reports {
                REPORTS_TOOLS
            } else {
                COMMISSIONING_TOOLS
            },
            if workspace == Workspace::Reports {
                REPORTS_ASSISTS
            } else {
                COMMISSIONING_ASSISTS
            },
            if workspace == Workspace::Reports {
                REPORTS_PANELS
            } else {
                COMMISSIONING_PANELS
            },
        );
        let n = rows.tools.len() + rows.assists.len() + rows.panels.len();
        let need = (n + 1) as f32 * (rail::ENTRY + 2.0) + 3.0 * 9.0 + 12.0;
        assert!(need <= rect.height(), "{workspace:?} needs {need}");
    }
}

#[test]
fn the_commissioning_rail_picks_the_panel_tab_and_shows_which() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = example(size, Workspace::Commissioning);
    let ctx = d.ctx.clone();
    let rail = d.app.workspace_rail(
        COMMISSIONING_TOOLS,
        COMMISSIONING_ASSISTS,
        COMMISSIONING_PANELS,
    );
    assert!(rail.tools[0].active, "Checks is the tab in hand");
    // A real click on the second row (Punch).
    let rect = d.app.wb.rail_rect.unwrap();
    let at = egui::pos2(
        rect.center().x,
        rect.top() + 6.0 + 1.5 * (rail::ENTRY + 2.0),
    );
    click(&mut d, size, at);
    assert_eq!(d.app.wb.commissioning.as_ref().unwrap().tab, Tab::Punch);
    d.app.workspace_rail_click(&ctx, "rail.cm_test");
    assert_eq!(d.app.wb.commissioning.as_ref().unwrap().tab, Tab::Test);
    let rail = d.app.workspace_rail(
        COMMISSIONING_TOOLS,
        COMMISSIONING_ASSISTS,
        COMMISSIONING_PANELS,
    );
    let active: Vec<_> = rail
        .tools
        .iter()
        .filter(|e| e.active)
        .map(|e| e.id)
        .collect();
    assert_eq!(active, ["rail.cm_test"]);
}

#[test]
fn next_open_moves_the_walk_and_decides_nothing() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = example(size, Workspace::Commissioning);
    let ctx = d.ctx.clone();
    let before = d.app.commissioning_progress().unwrap().done();
    d.app.workspace_rail_click(&ctx, "rail.cm_next");
    d.frame_sized(size, Vec::new());
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    assert!(mode.selected.is_some(), "the walk went to a target");
    assert_eq!(d.app.commissioning_progress().unwrap().done(), before);
}

#[test]
fn export_opens_its_card_and_a_second_click_shuts_it() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = example(size, Workspace::Reports);
    let ctx = d.ctx.clone();
    let rail = d
        .app
        .workspace_rail(REPORTS_TOOLS, REPORTS_ASSISTS, REPORTS_PANELS);
    let export = rail.panels.iter().find(|e| e.id == "rail.export").unwrap();
    assert_eq!(export.flyout.len(), EXPORT_FLYOUT.len());
    d.app.workspace_rail_click(&ctx, "rail.export");
    assert_eq!(rail::open_flyout(&ctx).as_deref(), Some("rail.export"));
    d.app.workspace_rail_click(&ctx, "rail.export");
    assert!(rail::open_flyout(&ctx).is_none());
    // Leaving the workspace closes the card.
    d.app.workspace_rail_click(&ctx, "rail.export");
    d.app.switch_workspace(Workspace::Cabinet);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    assert!(rail::open_flyout(&ctx).is_none());
}
