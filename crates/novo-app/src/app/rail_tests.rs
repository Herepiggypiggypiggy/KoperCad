//! The tool rail: its rows are real, it fits the windows we support, it
//! expands, its flyout opens and closes, and the keyboard reaches it.

use eframe::egui;

use super::gate1_tests::Drive;
use super::rail::{self, MAX_PER_ZONE};
use super::rail_schematic::{ASSISTS, CONNECT_FLYOUT, HISTORY, PANELS, TOOLS, anchor_of, look};
use crate::actions::Action;

fn open(size: egui::Vec2) -> (tempfile::TempDir, Drive) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
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
    for (name, zone) in [
        ("tools", TOOLS),
        ("assists", ASSISTS),
        ("panels", PANELS),
        ("history", HISTORY),
    ] {
        assert!(zone.len() <= MAX_PER_ZONE, "{name} has {}", zone.len());
        let mut seen = std::collections::HashSet::new();
        for &id in zone {
            assert!(seen.insert(id), "{name}: {id} twice");
            assert!(look(id).is_some(), "{name}: {id} has no picture or name");
            if !id.starts_with("rail.") {
                assert!(
                    novo_workflow::commands::is_known(id),
                    "{name}: {id} is not a command"
                );
            }
            if let Some(a) = anchor_of(id) {
                assert!(super::learn::ANCHORS.contains(&a), "{id}: anchor {a}");
            }
        }
    }
    for &id in CONNECT_FLYOUT {
        let (_, label, _) = look(id).unwrap_or_else(|| panic!("{id} has no look"));
        assert!(label.chars().count() <= novo_theme::presentation::MAX_SHORT);
    }
    // Every command row is on the Draw side of the registry or a panel toggle.
    for &id in TOOLS.iter().chain(ASSISTS).chain(PANELS).chain(HISTORY) {
        if let Some((_, label, _)) = look(id) {
            assert!(
                label.chars().count() <= novo_theme::presentation::MAX_SHORT,
                "{id}"
            );
        }
    }
}

#[test]
fn the_rail_fits_without_scrolling_at_both_screen_sizes() {
    for size in [egui::vec2(1366.0, 768.0), egui::vec2(1920.0, 1080.0)] {
        let (_dir, d) = open(size);
        let rail_rect = d.app.wb.rail_rect.expect("the rail is shown");
        let (_, canvas) = d.app.wb.canvas_area.unwrap();
        assert!(rail_rect.left() <= 1.0, "{rail_rect:?}");
        assert!(rail_rect.right() <= canvas.left() + 1.0);
        assert!(
            (rail_rect.width() - rail::WIDTH).abs() < 2.0,
            "{rail_rect:?}"
        );
        let rows = TOOLS.len() + ASSISTS.len() + PANELS.len() + HISTORY.len() + 1;
        let need = rows as f32 * (rail::ENTRY + 2.0) + 3.0 * 9.0 + 12.0;
        assert!(
            need <= rail_rect.height(),
            "{size:?}: rail needs {need} of {}",
            rail_rect.height()
        );
        // Every row has a spot on screen: the last panel entry is inside.
        let devices = d.app.learn_anchor("dock.select").expect("Select");
        assert!(rail_rect.contains_rect(devices));
    }
}

#[test]
fn the_foot_button_expands_the_rail_and_the_choice_is_kept() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let narrow = d.app.wb.rail_rect.unwrap();
    assert!(!d.app.settings.rail_expanded);
    // The expand button is the lowest entry of the rail.
    let at = egui::pos2(narrow.center().x, narrow.bottom() - 6.0 - rail::ENTRY / 2.0);
    click(&mut d, size, at);
    assert!(d.app.settings.rail_expanded);
    let wide = d.app.wb.rail_rect.unwrap();
    assert!(
        (wide.width() - rail::WIDTH_EXPANDED).abs() < 2.0,
        "{wide:?}"
    );
    // And back.
    let at = egui::pos2(wide.left() + 20.0, wide.bottom() - 6.0 - rail::ENTRY / 2.0);
    click(&mut d, size, at);
    assert!(!d.app.settings.rail_expanded);
}

#[test]
fn connect_opens_its_flyout_and_a_row_sets_the_wiring_mode() {
    use novo_editor::tool::WiringMode;
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let ctx = d.ctx.clone();
    let connect = d.app.learn_anchor("dock.connect").unwrap();
    // A plain click starts the wire tool, as the dock's button did.
    click(&mut d, size, connect.center());
    let tool = |d: &Drive| d.app.canvas.schematic_ref().unwrap().editor.tool().clone();
    assert!(
        matches!(tool(&d), novo_editor::Tool::Wire(_)),
        "{:?}",
        tool(&d)
    );
    assert!(
        rail::open_flyout(&ctx).is_none(),
        "a plain click opens no card"
    );
    // A click on the tool in hand opens the card.
    click(&mut d, size, connect.center());
    assert_eq!(rail::open_flyout(&ctx).as_deref(), Some("tool.connect"));
    // The flyout shows Corners and Lines where the lessons look for them.
    let lines = d.app.learn_anchor("dock.lines").unwrap();
    assert!(
        lines.left() > connect.right(),
        "{lines:?} beside {connect:?}"
    );
    click(&mut d, size, lines.center());
    assert!(rail::open_flyout(&ctx).is_none(), "picking a row closes it");
    assert_eq!(d.app.settings.wiring_mode(), WiringMode::Lines);
    assert!(matches!(tool(&d), novo_editor::Tool::Wire(_)));

    // The tool is in hand: a click on it opens the card, a second shuts it.
    let connect = d.app.learn_anchor("dock.connect").unwrap();
    click(&mut d, size, connect.center());
    assert!(rail::open_flyout(&ctx).is_some());
    click(&mut d, size, connect.center());
    assert!(rail::open_flyout(&ctx).is_none());
    // The corner notch opens it too, tool in hand or not.
    let notch = egui::pos2(connect.left() + 26.0, connect.bottom() - 4.0);
    click(&mut d, size, notch);
    assert!(rail::open_flyout(&ctx).is_some());
    // A click elsewhere closes it too.
    let (_, canvas) = d.app.wb.canvas_area.unwrap();
    click(&mut d, size, canvas.center());
    assert!(rail::open_flyout(&ctx).is_none());
}

#[test]
fn a_click_on_a_tool_starts_it_and_the_keys_stay_with_the_drawing() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let ctx = d.ctx.clone();
    let cable = d.app.learn_anchor("dock.cable").unwrap();
    click(&mut d, size, cable.center());
    let tool = d.app.canvas.schematic_ref().unwrap().editor.tool().clone();
    assert!(matches!(tool, novo_editor::Tool::Cable(_)), "{tool:?}");
    assert_eq!(d.app.key_focus(&ctx), novo_workflow::keymap::Focus::Canvas);
    let select = d.app.learn_anchor("dock.select").unwrap();
    click(&mut d, size, select.center());
    let tool = d.app.canvas.schematic_ref().unwrap().editor.tool().clone();
    assert!(matches!(tool, novo_editor::Tool::Select), "{tool:?}");
}

#[test]
fn accept_proposals_waits_for_a_proposal() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let rail = d.app.schematic_rail(0);
    let accept = rail
        .assists
        .iter()
        .find(|e| e.id == "proposal.accept_all")
        .expect("the row");
    assert!(!accept.enabled);
    assert!(accept.tip.contains("No proposals"), "{}", accept.tip);
    let rail = d.app.schematic_rail(3);
    let accept = rail
        .assists
        .iter()
        .find(|e| e.id == "proposal.accept_all")
        .unwrap();
    assert_eq!(accept.badge.map(|b| b.count), Some(3));
    assert!(accept.tip.contains("3 proposals"), "{}", accept.tip);
    // The issues badge is the only warning coloured one.
    let issues = rail
        .panels
        .iter()
        .find(|e| e.id == "check.problems")
        .unwrap();
    assert!(issues.badge.is_some_and(|b| b.warn));
    assert!(
        rail.tools
            .iter()
            .chain(&rail.assists)
            .chain(&rail.history)
            .all(|e| e.badge.is_none_or(|b| !b.warn))
    );
}

#[test]
fn tab_reaches_the_rail_and_the_arrows_move_between_rows() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let ctx = d.ctx.clone();
    let rail_rect = d.app.wb.rail_rect.unwrap();
    let focused_in_rail = |ctx: &egui::Context| -> Option<egui::Rect> {
        let id = ctx.memory(|m| m.focused())?;
        let rect = ctx.read_response(id)?.rect;
        rail_rect.contains_rect(rect).then_some(rect)
    };
    let mut reached = None;
    for _ in 0..60 {
        d.key(egui::Key::Tab, egui::Modifiers::NONE);
        d.frame_sized(size, Vec::new());
        if let Some(r) = focused_in_rail(&ctx) {
            reached = Some(r);
            break;
        }
    }
    let first = reached.expect("Tab reaches the rail");
    d.key(egui::Key::ArrowDown, egui::Modifiers::NONE);
    d.frame_sized(size, Vec::new());
    let next = focused_in_rail(&ctx).expect("focus stays in the rail");
    assert!(next.top() > first.top(), "{first:?} -> {next:?}");
    d.key(egui::Key::ArrowUp, egui::Modifiers::NONE);
    d.frame_sized(size, Vec::new());
    let back = focused_in_rail(&ctx).unwrap();
    assert_eq!(back.top(), first.top());
}

#[test]
fn escape_closes_the_flyout_before_it_does_anything_else() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let ctx = d.ctx.clone();
    let connect = d.app.learn_anchor("dock.connect").unwrap();
    let notch = egui::pos2(connect.left() + 26.0, connect.bottom() - 4.0);
    click(&mut d, size, notch);
    assert!(rail::open_flyout(&ctx).is_some());
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.frame_sized(size, Vec::new());
    assert!(rail::open_flyout(&ctx).is_none());
}

#[test]
fn only_the_schematic_has_a_rail_so_far() {
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let ctx = d.ctx.clone();
    assert!(d.app.wb.rail_rect.is_some());
    d.app.run_command(&ctx, "view.cabinet", None);
    for _ in 0..3 {
        d.frame_sized(size, Vec::new());
    }
    assert!(d.app.wb.rail_rect.is_none(), "V3c brings the Cabinet rail");
}

#[test]
fn the_rail_stays_put_and_clickable_while_a_wire_is_under_way() {
    use novo_editor::Tool;
    // The old dock parked in the corner or moved aside while a wire was
    // drawn (scenarios 796 and 796b). The rail is a side panel: it has no
    // such state, and Select stays where it was and works.
    let size = egui::vec2(1366.0, 768.0);
    let (_dir, mut d) = open(size);
    let before = d.app.wb.rail_rect.unwrap();
    let select_at = d.app.learn_anchor("dock.select").unwrap();
    d.app.canvas.schematic().unwrap().editor.begin_wire();
    d.frame_sized(size, Vec::new());
    let tool = |d: &Drive| d.app.canvas.schematic_ref().unwrap().editor.tool().clone();
    assert!(matches!(tool(&d), Tool::Wire(_)));
    // Start the wire: a click on the sheet.
    let (_, canvas) = d.app.wb.canvas_area.unwrap();
    click(&mut d, size, canvas.center());
    assert_eq!(
        d.app.wb.rail_rect.unwrap(),
        before,
        "the rail does not move"
    );
    assert_eq!(d.app.learn_anchor("dock.select").unwrap(), select_at);
    click(&mut d, size, select_at.center());
    assert!(matches!(tool(&d), Tool::Select), "{:?}", tool(&d));
}
