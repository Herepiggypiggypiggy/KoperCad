//! Signing off a Fixed punch item on the pad.

use eframe::egui;
use novo_commissioning::{PunchItem, PunchState, Severity};

use super::commissioning_signature::SignDraft;
use super::gate1_tests::Drive;
use crate::actions::Action;

fn fixed_item(d: &mut Drive) -> u64 {
    let mode = d.app.wb.commissioning.as_mut().unwrap();
    mode.tab = super::commissioning::Tab::Punch;
    mode.store.add_punch(PunchItem {
        id: 0,
        title: "loose ferrule".into(),
        note: String::new(),
        severity: Severity::Minor,
        state: PunchState::Fixed,
        target: None,
        check: None,
        raised_by: "tv".into(),
        raised_at: "2026-10-17T09:00:00Z".into(),
        closed_by: None,
        closed_at: None,
        signature: None,
        photos: Vec::new(),
    })
}

fn event(pos: egui::Pos2, pressed: bool) -> egui::Event {
    egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn drawing_on_the_pad_and_signing_verifies_the_item_with_the_strokes() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let id = fixed_item(&mut d);
    d.frame();
    d.app.wb.commissioning.as_mut().unwrap().signing = Some(SignDraft::new(id, "A. Boss"));
    d.frame();
    d.frame();
    let button = d
        .app
        .learn_anchor("commissioning.sign")
        .expect("the sign button");
    // the pad sits right above the button row: drag a stroke across it
    let y = button.top() - 60.0;
    let x0 = button.left() + 10.0;
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(x0, y))]);
    d.frame_with(vec![event(egui::pos2(x0, y), true)]);
    for step in 1..=8 {
        let pos = egui::pos2(x0 + 12.0 * step as f32, y + 4.0 * step as f32);
        d.frame_with(vec![egui::Event::PointerMoved(pos)]);
    }
    d.frame_with(vec![event(egui::pos2(x0 + 96.0, y + 32.0), false)]);
    d.frame();
    // now click Sign and verify
    let button = d
        .app
        .learn_anchor("commissioning.sign")
        .expect("the sign button");
    d.frame_with(vec![egui::Event::PointerMoved(button.center())]);
    d.frame_with(vec![event(button.center(), true)]);
    d.frame_with(vec![event(button.center(), false)]);
    d.frame();
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    let item = mode.store.punch(id).unwrap();
    assert_eq!(item.state, PunchState::Verified);
    let sig = item.signature.as_ref().expect("signed");
    assert_eq!(sig.name, "A. Boss");
    assert!(!sig.strokes.is_empty(), "the stroke was kept");
    assert!(mode.signing.is_none(), "the pad closes");
}

#[test]
fn the_verified_button_opens_the_pad_instead_of_verifying() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::OpenExample);
    d.frame();
    d.app.run_command(&ctx, "view.commissioning", None);
    d.frame();
    let id = fixed_item(&mut d);
    d.frame();
    d.frame();
    let button = d
        .app
        .learn_anchor("commissioning.resolve")
        .expect("resolve button");
    d.frame_with(vec![egui::Event::PointerMoved(button.center())]);
    d.frame_with(vec![event(button.center(), true)]);
    d.frame_with(vec![event(button.center(), false)]);
    d.frame();
    let mode = d.app.wb.commissioning.as_ref().unwrap();
    assert_eq!(mode.store.punch(id).unwrap().state, PunchState::Fixed);
    assert!(mode.signing.as_ref().is_some_and(|s| s.item == id));
}
