//! The properties panel driven like a user would (WORKFLOW.md J12):
//! Ctrl+2, type part of a property name, Enter, type the value, Enter.

use egui::{Event, Key, Modifiers, RawInput};
use novo_core::{AnyId, Command, Document};
use novo_parts::Catalog;
use novo_parts::ui::{BulkEditor, PropertiesPanel};
use novo_tests::motor_starter;

struct Harness {
    ctx: egui::Context,
    panel: PropertiesPanel,
    catalog: Catalog,
}

impl Harness {
    fn new() -> Self {
        Harness {
            ctx: egui::Context::default(),
            panel: PropertiesPanel::new(),
            catalog: Catalog::starter(),
        }
    }

    /// One frame with `events`; returns the command the panel produced.
    fn frame(
        &mut self,
        doc: &Document,
        selection: &[AnyId],
        events: Vec<Event>,
    ) -> Option<Command> {
        let input = RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 800.0),
            )),
            ..Default::default()
        };
        let mut out = None;
        let mut output = self.ctx.run_ui(input, |ui| {
            out = self.panel.show(ui, doc.project(), selection, &self.catalog);
        });
        output.textures_delta.clear();
        out
    }
}

fn key(key: Key) -> Event {
    Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    }
}

fn text(t: &str) -> Event {
    Event::Text(t.into())
}

#[test]
fn j12_change_one_property_in_three_actions() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let selection = [AnyId::from(gate.k1_coil)];
    let mut h = Harness::new();
    assert!(h.frame(&doc, &selection, vec![]).is_none());

    // Ctrl+2 (the app shell calls focus_filter), type "desc", Enter.
    h.panel.focus_filter();
    h.frame(&doc, &selection, vec![]);
    h.frame(&doc, &selection, vec![text("desc")]);
    assert_eq!(h.panel.filter(), "desc");
    h.frame(&doc, &selection, vec![key(Key::Enter)]);
    h.frame(&doc, &selection, vec![]);
    // Type the value (replacing the selected old one), Enter.
    let mut command = h.frame(&doc, &selection, vec![text("Main contactor")]);
    for _ in 0..2 {
        command = command.or(h.frame(&doc, &selection, vec![key(Key::Enter)]));
    }
    let command = command.expect("Enter commits the value");
    doc.execute(command).unwrap();
    assert_eq!(
        doc.project().device(gate.k1).unwrap().description,
        "Main contactor"
    );
}

#[test]
fn escape_leaves_a_field_unchanged() {
    let mut doc = Document::default();
    let gate = motor_starter(&mut doc).unwrap();
    let selection = [AnyId::from(gate.k1)];
    let mut h = Harness::new();
    h.panel.focus_filter();
    h.frame(&doc, &selection, vec![]);
    h.frame(&doc, &selection, vec![text("description")]);
    h.frame(&doc, &selection, vec![key(Key::Enter)]);
    h.frame(&doc, &selection, vec![]);
    h.frame(&doc, &selection, vec![text("oops")]);
    for _ in 0..3 {
        assert!(h.frame(&doc, &selection, vec![key(Key::Escape)]).is_none());
    }
    assert_eq!(
        doc.project().device(gate.k1).unwrap().description,
        "Motor contactor"
    );
}

#[test]
fn bulk_editor_draws_every_device() {
    let mut doc = Document::default();
    motor_starter(&mut doc).unwrap();
    let ctx = egui::Context::default();
    let mut editor = BulkEditor::new();
    let catalog = Catalog::starter();
    for _ in 0..2 {
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            assert!(editor.show(ui, doc.project(), &catalog).is_none());
        });
        output.textures_delta.clear();
    }
}
