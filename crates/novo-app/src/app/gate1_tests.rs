//! Gate 1 end to end through the whole window, headless: create a project,
//! place a direct-on-line motor starter over two pages through the command
//! field and the editor, wire it, see its terminals, potentials and cross
//! references, save, reopen, compare and export to PDF.

use std::path::Path;

use eframe::egui;
use novo_core::{AnyId, Document, PageId, Point, Project, SymbolId};
use novo_workflow::palette::Mode;

use super::NovoApp;
use crate::actions::Action;
use crate::backend::StorageBackend;
use crate::editor_canvas::EditorCanvas;
use crate::settings::Settings;

/// Pick tolerance for clicks, in millimetres.
const TOL: f64 = 1.0;

struct Drive {
    ctx: egui::Context,
    app: NovoApp,
}

impl Drive {
    fn new(untitled: &Path) -> Drive {
        let ctx = egui::Context::default();
        novo_theme::apply(&ctx);
        let app = NovoApp::with_settings(
            Box::new(StorageBackend::with_untitled_dir(untitled.to_owned())),
            Box::new(EditorCanvas::default()),
            Settings::default(),
        );
        let mut drive = Drive { ctx, app };
        drive.frame();
        drive
    }

    fn frame_with(&mut self, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
    }

    fn frame(&mut self) {
        self.frame_with(Vec::new());
    }

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame_with(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
    }

    fn doc(&self) -> &Document {
        self.app.document().expect("a project is open")
    }

    fn page(&self) -> PageId {
        self.app.current_page().expect("a page is shown")
    }

    /// I, the query, Enter, then a click on the page: one placement as the
    /// user does it. Returns the new symbol.
    fn place(&mut self, query: &str, x: f64, y: f64) -> SymbolId {
        let before: Vec<SymbolId> = self.doc().project().symbols().map(|s| s.id).collect();
        assert!(
            self.app.type_command(&self.ctx.clone(), Mode::Place, query),
            "`{query}` picks a symbol"
        );
        self.click(Point::new(x, y));
        let placed = self
            .doc()
            .project()
            .symbols()
            .map(|s| s.id)
            .find(|id| !before.contains(id))
            .unwrap_or_else(|| panic!("`{query}` placed nothing"));
        // Stacked devices propose their wires; Enter takes each (the
        // editor reads Enter only while the pointer is on the drawing, so
        // the test presses it through the editor).
        while self.has_wire_proposals() {
            let app = &mut self.app;
            let view = app.canvas.schematic().expect("the editor canvas");
            let doc = app.backend.document_mut().expect("a project");
            assert!(view.editor.accept_proposal(doc));
            self.frame();
        }
        self.editor_leave();
        self.frame();
        placed
    }

    fn click(&mut self, at: Point) {
        let page = self.page();
        let app = &mut self.app;
        let view = app.canvas.schematic().expect("the editor canvas");
        let doc = app.backend.document_mut().expect("a project");
        view.editor.click(doc, page, at, TOL);
        self.frame();
    }

    fn editor_leave(&mut self) {
        if let Some(view) = self.app.canvas.schematic() {
            view.editor.leave_tool();
        }
    }

    fn has_wire_proposals(&self) -> bool {
        self.app
            .canvas
            .schematic_ref()
            .is_some_and(|v| !v.editor.proposals().is_empty())
    }

    /// A wire from one pin to another with the connect tool (W).
    fn wire(&mut self, from: Point, to: Point) {
        self.app
            .run_command(&self.ctx.clone(), "tool.connect", None);
        self.click(from);
        self.click(to);
        self.editor_leave();
        self.frame();
    }

    fn pin(&self, symbol: SymbolId, top: bool) -> Point {
        let sym = self.doc().project().symbol(symbol).expect("symbol");
        let pins: Vec<Point> = (0..sym.pins.len())
            .filter_map(|i| sym.pin_position(u16::try_from(i).ok()?))
            .collect();
        let pick = |a: &&Point, b: &&Point| a.y.total_cmp(&b.y);
        let p = if top {
            pins.iter().min_by(pick)
        } else {
            pins.iter().max_by(pick)
        };
        *p.expect("the symbol has pins")
    }

    fn select(&mut self, symbol: SymbolId) {
        let view = self.app.canvas.schematic().expect("editor");
        view.canvas.selection.clear();
        view.canvas.selection.apply(
            [novo_canvas::model::item_id(AnyId::Symbol(symbol))],
            novo_canvas::SelectOp::Replace,
        );
        self.frame();
    }

    fn tag_of(&self, symbol: SymbolId) -> String {
        let project = self.doc().project();
        novo_editor::edit::device_of(project, AnyId::Symbol(symbol))
            .and_then(|d| project.device(d))
            .map(|d| d.tag.to_string())
            .unwrap_or_default()
    }
}

/// What a saved and reopened project must keep: pages, devices, symbols,
/// wires, terminals and potentials, in a comparable form.
fn summary(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for page in project.pages_in_order() {
        out.push(format!("page {} {}", page.name, page.title));
    }
    let mut devices: Vec<String> = project.devices().map(|d| d.tag.to_string()).collect();
    devices.sort();
    out.extend(devices.into_iter().map(|t| format!("device {t}")));
    let mut symbols: Vec<String> = project
        .symbols()
        .map(|s| {
            format!(
                "symbol {} {:.1},{:.1} {:?}",
                s.symbol.0, s.placement.origin.x, s.placement.origin.y, s.role
            )
        })
        .collect();
    symbols.sort();
    out.extend(symbols);
    let netlist = project.netlist();
    let mut wires: Vec<String> = project
        .wires()
        .map(|w| {
            format!(
                "wire {:?} {:?} {:?}",
                w.from,
                w.to,
                novo_editor::potential::wire_potential(project, &netlist, w.id)
            )
        })
        .collect();
    wires.sort();
    out.extend(wires);
    let mut terminals: Vec<String> = project
        .terminals()
        .filter_map(|t| project.terminal_designation(t.id))
        .collect();
    terminals.sort();
    out.extend(terminals.into_iter().map(|t| format!("terminal {t}")));
    out
}

#[test]
fn gate1_motor_starter_through_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));

    // A new project opens on its first page.
    d.app.run(&d.ctx.clone(), Action::NewProject);
    d.frame();
    let page1 = d.page();

    // Page 1 starts with the breaker. The coil is placed before its
    // contacts (the editor gives a coil its own new device), so the control
    // page comes next and the contactor's main contacts after it.
    let q1 = d.place("mcb3", 80.0, 48.0);

    // Page 2, control circuit: a new page, the coil of -K1, stop and start
    // buttons stacked above it, and the self-hold contact beside start.
    assert!(d.app.type_command(&d.ctx.clone(), Mode::All, "new page"));
    d.frame();
    let page2 = d.page();
    assert_ne!(page1, page2);
    let coil = d.place("coil", 200.0, 144.0);
    assert_eq!(d.tag_of(coil), "-K1");
    let s1 = d.place("pbnc", 200.0, 48.0);
    let s2 = d.place("pb", 200.0, 96.0);
    let hold = d.place("no K1", 232.0, 96.0);
    assert_eq!(
        d.tag_of(hold),
        "-K1",
        "the self-hold contact belongs to -K1"
    );
    let (s2_top, s2_bottom) = (d.pin(s2, true), d.pin(s2, false));
    let (hold_top, hold_bottom) = (d.pin(hold, true), d.pin(hold, false));
    d.wire(hold_top, s2_top);
    d.wire(hold_bottom, s2_bottom);
    {
        let project = d.doc().project();
        let netlist = project.netlist();
        let net = |s: SymbolId, top: bool| {
            let sym = project.symbol(s).unwrap();
            let want = d.pin(s, top);
            let index = (0..sym.pins.len())
                .find(|&i| sym.pin_position(u16::try_from(i).unwrap()) == Some(want))
                .unwrap();
            netlist.net_of(novo_core::Endpoint::pin(s, u16::try_from(index).unwrap()))
        };
        assert_eq!(net(hold, true), net(s2, true), "self-hold in parallel");
        assert_eq!(net(hold, false), net(s2, false));
        assert_eq!(net(s1, false), net(s2, true), "stop feeds start");
        assert_eq!(net(s2, false), net(coil, true), "start feeds the coil");
    }

    // Back on page 1 (Page Up): the main contacts of -K1, the overload
    // relay and the motor, stacked under the breaker so their wires are
    // proposed and accepted.
    d.app.run_command(&d.ctx.clone(), "nav.previous_page", None);
    d.frame();
    assert_eq!(d.page(), page1);
    let km = d.place("km3 K1", 80.0, 96.0);
    let _ol = d.place("ol", 80.0, 144.0);
    let m1 = d.place("m3", 80.0, 220.0);
    assert_eq!(d.tag_of(km), "-K1");
    assert_eq!(d.tag_of(m1), "-M1");
    let main_wires = d
        .doc()
        .project()
        .wires()
        .filter(|w| w.page == page1)
        .count();
    assert!(
        main_wires >= 9,
        "three poles wired three times: {main_wires}"
    );

    // L1, L2 and L3 are declared once, at the source, on the breaker's
    // outgoing wires; the potentials flow through the starter.
    {
        let project = d.doc().project();
        let feed: Vec<AnyId> = project
            .wires()
            .filter(|w| {
                w.ends()
                    .iter()
                    .any(|e| matches!(e, novo_core::Endpoint::Pin { symbol, .. } if *symbol == q1))
            })
            .map(|w| AnyId::Wire(w.id))
            .collect();
        assert_eq!(feed.len(), 3);
        let app = &mut d.app;
        let view = app.canvas.schematic().unwrap();
        let doc = app.backend.document_mut().unwrap();
        let menu = view.editor.potential_menu(doc, &feed);
        let first = menu.declare.first().expect("a source to declare").clone();
        view.editor.assign_potentials(doc, &menu.wires, &first);
    }
    d.frame();
    {
        let project = d.doc().project();
        let netlist = project.netlist();
        let names: std::collections::BTreeSet<String> = project
            .wires()
            .filter_map(|w| novo_editor::potential::wire_potential(project, &netlist, w.id))
            .collect();
        for p in ["L1", "L2", "L3"] {
            assert!(names.contains(p), "{p} flows: {names:?}");
        }
    }

    // The motor sits in the field: its wires cross into +FIELD,
    // so terminals are proposed there, and accepted.
    {
        let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(m1)).unwrap();
        let app = &mut d.app;
        let view = app.canvas.schematic().unwrap();
        let doc = app.backend.document_mut().unwrap();
        view.editor.rename(doc, device, "+FIELD-M1").unwrap();
    }
    d.frame();
    assert_eq!(d.app.page_proposals().len(), 3, "one terminal per phase");
    // Enter accepts the highlighted proposal, Tab moves on, `accept all`
    // takes the rest.
    d.key(egui::Key::Tab, egui::Modifiers::NONE);
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(d.app.page_proposals().len(), 2);
    assert!(d.app.type_command(&d.ctx.clone(), Mode::All, "accept all"));
    d.frame();
    assert!(d.app.page_proposals().is_empty());
    let strips = novo_terminals::strips(d.doc().project());
    assert_eq!(strips.len(), 1);
    let strip = novo_terminals::strip_view(d.doc().project(), strips[0]);
    assert_eq!(strip.rows.len(), 3, "{strip:?}");
    assert!(strip.tag.ends_with("-X1"), "{}", strip.tag);
    // Ctrl+3 shows the strip in the drawer.
    d.key(egui::Key::Num3, egui::Modifiers::COMMAND);
    assert!(d.app.wb.drawer.is_some());
    assert_eq!(d.app.drawer_strip(), Some(strips[0]));
    // Ctrl+4 and Ctrl+5 switch to Cables and Devices; Ctrl+5 again closes.
    d.key(egui::Key::Num4, egui::Modifiers::COMMAND);
    assert_eq!(d.app.wb.drawer, Some(super::workbench::Drawer::Cables));
    d.key(egui::Key::Num5, egui::Modifiers::COMMAND);
    assert_eq!(d.app.wb.drawer, Some(super::workbench::Drawer::Devices));
    d.key(egui::Key::Num5, egui::Modifiers::COMMAND);
    assert_eq!(d.app.wb.drawer, None);
    d.key(egui::Key::Num3, egui::Modifiers::COMMAND);

    // Cross references: the coil lists its contacts, a contact points back
    // at the coil.
    let labels = d.app.xref_labels(page2);
    let coil_label = labels
        .iter()
        .find(|l| l.symbol == coil)
        .expect("coil label");
    assert!(
        coil_label.lines.iter().any(|l| l.contains("1.")),
        "the coil shows the main contacts on page 1: {:?}",
        coil_label.lines
    );
    let page1_labels = d.app.xref_labels(page1);
    assert!(
        page1_labels
            .iter()
            .any(|l| l.symbol == km && l.lines.iter().any(|t| t.starts_with("2."))),
        "the main contacts point at the coil on page 2: {page1_labels:?}"
    );

    // Ctrl+K `K1`, Enter jumps to the coil; G goes to a contact and
    // Alt+Left comes back.
    assert_eq!(d.page(), page1);
    assert!(d.app.type_command(&d.ctx.clone(), Mode::All, "-K1"));
    d.frame();
    assert_eq!(d.page(), page2, "the device jumps to its coil");
    assert_eq!(d.app.selection(), vec![AnyId::Symbol(coil)]);
    d.key(egui::Key::G, egui::Modifiers::NONE);
    d.frame();
    let at_contact = d.app.selection();
    assert_eq!(at_contact.len(), 1);
    assert_ne!(at_contact, vec![AnyId::Symbol(coil)], "G left the coil");
    d.key(egui::Key::ArrowLeft, egui::Modifiers::ALT);
    d.frame();
    assert_eq!(d.page(), page2);
    assert_eq!(d.app.selection(), vec![AnyId::Symbol(coil)], "Alt+Left");
    d.select(coil);
    d.key(egui::Key::G, egui::Modifiers::SHIFT);
    assert!(
        d.app.wb.occurrences.as_ref().is_some_and(|o| o.len() >= 3),
        "Shift+G lists coil, main contacts and the self-hold contact"
    );

    // The issue pill counts what is left; it never blocks anything.
    let pill = novo_checks::Summary::of(d.app.problems()).pill_text();
    assert!(!pill.is_empty());

    // Save, reopen in a fresh window, and compare.
    let before = summary(d.doc().project());
    let file = dir.path().join("Starter.novo");
    assert!(d.app.save_to(&file));
    d.frame();
    // PDF export: two pages, written where asked.
    let pdf = dir.path().join("Starter.pdf");
    assert_eq!(d.app.export_pdf_to(&pdf), Some(pdf.clone()));
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let pages = lopdf::Document::load_mem(&bytes).unwrap().get_pages().len();
    assert_eq!(pages, 2, "one PDF page per project page");
    // NOVO_GATE1_OUT=<dir> keeps the project and the PDF, for screenshots.
    if let Some(out) = std::env::var_os("NOVO_GATE1_OUT") {
        let out = std::path::PathBuf::from(out);
        std::fs::create_dir_all(&out).unwrap();
        std::fs::copy(&pdf, out.join("Starter.pdf")).unwrap();
    }
    d.app.run(&d.ctx.clone(), Action::CloseProject);
    d.frame();

    let mut e = Drive::new(&dir.path().join("untitled2"));
    e.app.open_path(&file);
    e.frame();
    assert_eq!(summary(e.doc().project()), before, "the reopened project");
    if let Some(out) = std::env::var_os("NOVO_GATE1_OUT") {
        e.app.run(&e.ctx.clone(), Action::CloseProject);
        std::fs::copy(&file, std::path::Path::new(&out).join("Starter.novo")).unwrap();
        return;
    }
    assert_eq!(
        e.app.backend.file().as_deref(),
        Some(file.as_path()),
        "reopened from its file"
    );
}

/// Edits run from the command field (not drawn by the canvas) reach the
/// file even when the project is saved and closed before another frame.
#[test]
fn command_edits_are_saved_without_another_frame() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Rotated.novo");
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.run(&d.ctx.clone(), Action::NewProject);
    d.frame();
    assert!(d.app.save_to(&file));
    let k1 = d.place("coil", 100.0, 100.0);
    let s1 = d.place("pb", 160.0, 100.0);
    d.select(k1);

    // Rotate, then delete the push button, then a new page, all through the
    // command field, then save and close at once.
    let ctx = d.ctx.clone();
    assert!(d.app.type_command(&ctx, Mode::All, "rotate"));
    let rotated = d.doc().project().symbol(k1).unwrap().placement.rotation;
    let view = d.app.canvas.schematic().unwrap();
    view.canvas.selection.clear();
    view.canvas.selection.apply(
        [novo_canvas::model::item_id(AnyId::Symbol(s1))],
        novo_canvas::SelectOp::Replace,
    );
    d.app.run_command(&ctx, "edit.delete", None);
    assert!(d.app.type_command(&ctx, Mode::All, "new page"));
    let before = summary(d.doc().project());
    assert!(d.doc().project().symbol(s1).is_none(), "deleted");
    assert_eq!(d.doc().project().pages_in_order().len(), 2);
    assert!(d.app.save_to(&file));
    d.app.run(&ctx, Action::CloseProject);

    let mut e = Drive::new(&dir.path().join("untitled2"));
    e.app.open_path(&file);
    e.frame();
    let project = e.doc().project();
    assert_eq!(project.symbol(k1).unwrap().placement.rotation, rotated);
    assert_eq!(summary(project), before, "every command edit was written");
}

/// The marks on the drawing, keys and export details the reviews asked
/// for: proposals and problems as canvas marks, Shift+P, a tapped Space,
/// commands that come later, and where the PDF goes.
#[test]
fn marks_keys_and_export_follow_the_design() {
    use novo_canvas::{MarkKind, MarkTarget};
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("Plant 2.novo");
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.save_to(&file));
    let k = d.place("km3", 80.0, 96.0);
    let m = d.place("m3", 80.0, 172.0);
    {
        let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(m)).unwrap();
        let ok = d
            .app
            .with_editor(|editor, doc| editor.rename(doc, device, "+FIELD-M1"));
        assert_eq!(ok, Some(Ok(())));
    }
    d.frame();
    d.frame();

    // Terminal proposals are dashed Proposal marks on areas; the one Enter
    // takes is marked twice (heavier), never solid, and carries the note.
    let marks = d.app.canvas.schematic_ref().unwrap().canvas.marks.clone();
    let proposals: Vec<_> = marks
        .list
        .iter()
        .filter(|m| m.kind == MarkKind::Proposal)
        .collect();
    assert_eq!(proposals.len(), 4, "three proposals, the current one twice");
    assert!(
        proposals
            .iter()
            .all(|m| matches!(m.target, MarkTarget::Area(_)))
    );
    assert_eq!(
        proposals
            .iter()
            .filter(|m| m.text.contains("↵ accept"))
            .count(),
        1
    );
    // The canvas shows that note: on the proposal nearest the pointer, or
    // the top-left one without a pointer, which is the Enter target here.
    {
        let view = &d.app.canvas.schematic_ref().unwrap().canvas;
        let scene = novo_canvas::Scene::default();
        let placed = novo_canvas::marks::layout(&marks, &scene, &view.viewport);
        let Some(novo_canvas::marks::Placed::Outline { mark, .. }) =
            novo_canvas::marks::nearest_proposal(&placed, None)
        else {
            panic!("a proposal outline is placed");
        };
        assert!(
            marks.list[*mark].text.contains("↵ accept · Tab next"),
            "the hint is shown: {:?}",
            marks.list[*mark]
        );
    }
    // Problems sit on their objects: the motor's PE and the unwired
    // contactor poles are warnings on those symbols.
    let problems: Vec<_> = marks
        .list
        .iter()
        .filter(|m| m.kind == MarkKind::Warning || m.kind == MarkKind::Error)
        .collect();
    assert!(!problems.is_empty(), "problems are marked on the drawing");
    assert!(problems.iter().all(|m| !m.text.is_empty()), "hover text");
    let on = |s: SymbolId| MarkTarget::Item(novo_canvas::model::item_id(AnyId::Symbol(s)));
    assert!(
        problems
            .iter()
            .any(|p| p.target == on(k) || p.target == on(m))
    );

    // Shift+P hides the marks; the cross-reference labels are drawing
    // content and stay.
    d.key(egui::Key::P, egui::Modifiers::SHIFT);
    d.frame();
    assert!(d.app.canvas.schematic_ref().unwrap().canvas.marks.hidden);
    d.key(egui::Key::P, egui::Modifiers::SHIFT);
    d.frame();
    assert!(!d.app.canvas.schematic_ref().unwrap().canvas.marks.hidden);

    // A tapped Space opens the field on release, not on press.
    let space = |pressed| egui::Event::Key {
        key: egui::Key::Space,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    };
    let at = egui::Event::PointerMoved(egui::pos2(700.0, 400.0));
    d.frame_with(vec![at.clone(), space(true)]);
    d.frame();
    assert!(!d.app.wb.field.is_open(), "not on the press");
    d.frame_with(vec![space(false)]);
    d.frame();
    assert!(d.app.wb.field.is_open(), "on the release");
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(!d.app.wb.field.is_open());

    // Picked from the command field (or run by a script), it opens at once.
    d.app.run_command(&ctx, "view.command_field_here", None);
    assert!(d.app.wb.field.is_open());
    d.app.wb.field.close();

    // A command the app cannot run yet is listed greyed, with a reason.
    d.app.wb.field.open(Mode::All, None);
    d.app.wb.field.type_text("rules");
    let rows = d.app.with_field(|field, sources| field.rows(sources));
    let rules = rows
        .iter()
        .find(|r| r.target == novo_workflow::palette::Target::Command("project.rules"))
        .expect("the rules command is listed");
    assert_eq!(rules.disabled, Some("Comes later"));
    d.app.wb.field.close();

    // A saved project's PDF goes next to its file, named after it; a second
    // export says it replaced the first.
    let pdf = d.app.export_pdf().expect("exported");
    assert_eq!(pdf, dir.path().join("Plant 2.pdf"));
    assert!(pdf.is_file());
    d.app.export_pdf().expect("exported again");
    let last = &d.app.messages().last().unwrap().text;
    assert!(last.starts_with("Replaced "), "{last}");
}

#[test]
fn keys_go_to_a_focused_panel_button_not_the_canvas() {
    use novo_workflow::keymap::Focus;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert_eq!(d.app.key_focus(&ctx), Focus::Canvas, "nothing focused");

    // A click on the drawing focuses the canvas widget: still the canvas.
    let (_, rect) = d.app.wb.canvas_area.expect("the canvas was shown");
    let at = rect.center();
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_with(vec![egui::Event::PointerMoved(at), button(true)]);
    d.frame_with(vec![button(false)]);
    d.frame();
    assert!(
        ctx.memory(|m| m.focused()).is_some(),
        "the canvas took focus"
    );
    assert_eq!(d.app.key_focus(&ctx), Focus::Canvas);

    // A button over the window (another layer) with focus is a panel.
    let id = egui::Id::new("panel_button");
    for _ in 0..2 {
        let app = &mut d.app;
        let mut output = ctx.run_ui(Default::default(), |ui| {
            app.show(ui);
            egui::Area::new(egui::Id::new("panel"))
                .fixed_pos(egui::pos2(10.0, 60.0))
                .show(ui.ctx(), |ui| ui.push_id(id, |ui| ui.button("Panel")))
                .inner
                .inner
                .request_focus();
        });
        output.textures_delta.clear();
    }
    assert_eq!(d.app.key_focus(&ctx), Focus::Panel);
}

#[test]
/// A real pointer click on a symbol (moved, press, release) selects it
/// through the whole window. Under Xvfb a single click was reported not to
/// select; this pins the path down without X.
fn a_click_on_a_symbol_selects_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.run(&d.ctx.clone(), Action::NewProject);
    d.frame();
    let q1 = d.place("mcb3", 80.0, 48.0);
    d.frame();
    let item = novo_canvas::model::item_id(AnyId::Symbol(q1));
    let sym = d.doc().project().symbol(q1).unwrap().placement.origin;
    let local = {
        let view = d.app.canvas.schematic().unwrap();
        view.canvas
            .viewport
            .world_to_screen(novo_canvas::Vec2::new(sym.x as f32, sym.y as f32))
    };
    // Where the canvas sits in the window: hover until the symbol lights.
    let mut at = None;
    'scan: for oy in 0..80 {
        for ox in [0.0f32, 248.0] {
            let p = egui::pos2(local.x + ox, local.y + oy as f32);
            d.frame_with(vec![egui::Event::PointerMoved(p)]);
            let hovered = d.app.canvas.schematic().unwrap().canvas.hovered();
            if hovered == Some(item) {
                at = Some(p);
                break 'scan;
            }
        }
    }
    let p = at.expect("the symbol can be hovered");
    let btn = |pressed| egui::Event::PointerButton {
        pos: p,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    d.frame_with(vec![btn(true)]);
    d.frame_with(vec![btn(false)]);
    d.frame();
    let sel = d
        .app
        .canvas
        .schematic()
        .unwrap()
        .canvas
        .selection
        .ids()
        .to_vec();
    assert_eq!(sel, vec![item]);
}

#[test]
fn delete_declines_a_terminal_proposal_until_undone() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let k = d.place("km3", 80.0, 96.0);
    let m = d.place("m3", 80.0, 172.0);
    {
        let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(m)).unwrap();
        let ok = d
            .app
            .with_editor(|editor, doc| editor.rename(doc, device, "+FIELD-M1"));
        assert_eq!(ok, Some(Ok(())));
    }
    d.frame();
    d.editor_leave();
    d.app.canvas.schematic().unwrap().canvas.selection.clear();
    d.frame();
    assert!(d.app.selection().is_empty(), "{:?}", d.app.selection());
    let before = d.app.page_proposals().len();
    assert!(before > 0);
    let symbols = d.doc().project().symbols().count();

    // Away from the proposal, Delete declines nothing.
    let (_, rect) = d.app.wb.canvas_area.unwrap();
    d.frame_with(vec![egui::Event::PointerMoved(
        rect.left_top() + egui::vec2(2.0, 2.0),
    )]);
    d.key(egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(d.app.page_proposals().len(), before, "pointer elsewhere");

    // With the pointer on the highlighted proposal, it declines it, even
    // with a symbol selected: the selection is left alone.
    d.select(k);
    d.frame();
    let at = d.app.page_proposals()[d.app.wb.proposal.min(before - 1)].at;
    let view = &d.app.canvas.schematic_ref().unwrap().canvas.viewport;
    let p = view.world_to_screen(novo_canvas::Vec2::new(at.x as f32, at.y as f32));
    d.frame_with(vec![egui::Event::PointerMoved(
        rect.min + egui::vec2(p.x, p.y),
    )]);
    d.key(egui::Key::Delete, egui::Modifiers::NONE);
    assert_eq!(d.app.page_proposals().len(), before - 1, "declined");
    assert_eq!(
        d.doc().project().symbols().count(),
        symbols,
        "nothing deleted"
    );
    assert!(d.doc().project().symbol(k).is_some(), "the contactor stays");
    assert_eq!(d.app.selection(), vec![AnyId::Symbol(k)], "still selected");

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(d.app.page_proposals().len(), before, "undo brings it back");
}

#[test]
fn a_generated_page_shows_its_report_and_follows_each_edit() {
    use novo_core::{DeviceTag, Page, PageKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let schematic = d.page();
    let tag = DeviceTag::parse("-X1").unwrap();
    let doc = d.app.backend.document_mut().unwrap();
    let ((strip, page), changes) = doc
        .transact("Strip and its page", |tx| {
            let strip = tx.add_terminal_strip(tag)?;
            tx.add_terminal(strip, "1")?;
            let page = tx.add_page("20")?;
            tx.update::<Page>(page, |p| {
                p.kind = PageKind::Generated {
                    generator: "terminals".into(),
                }
            })?;
            Ok((strip, page))
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    d.app.show_page(page);
    d.frame();
    let texts = |d: &Drive| -> Vec<String> {
        let view = d.app.canvas.schematic_ref().unwrap();
        let sheet = view.sheet().expect("the generated page shows its sheet");
        assert_eq!(sheet.name, "20");
        sheet.texts.iter().map(|t| t.text.clone()).collect()
    };
    let before = texts(&d);
    assert!(before.iter().any(|t| t.contains("X1")), "{before:?}");
    assert!(!before.iter().any(|t| t == "77"), "{before:?}");

    // An edit shows on the sheet in the next frame, with no regenerate step.
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("Add 77", |tx| tx.add_terminal(strip, "77"))
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    d.frame();
    let after = texts(&d);
    assert!(after.iter().any(|t| t == "77"), "{after:?}");

    // Undo takes it back.
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(texts(&d), before);

    // A schematic page draws no sheet.
    d.app.show_page(schematic);
    d.frame();
    assert!(d.app.canvas.schematic_ref().unwrap().sheet().is_none());
}

#[test]
fn the_outline_lists_each_sheet_of_a_generated_page_and_opens_it() {
    use novo_core::{DeviceTag, Page, PageKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let doc = d.app.backend.document_mut().unwrap();
    let (page, changes) = doc
        .transact("Two strips and their page", |tx| {
            for tag in ["-X1", "-X2"] {
                let strip = tx.add_terminal_strip(DeviceTag::parse(tag).unwrap())?;
                tx.add_terminal(strip, "1")?;
            }
            let page = tx.add_page("20")?;
            tx.update::<Page>(page, |p| {
                p.kind = PageKind::Generated {
                    generator: "terminals".into(),
                }
            })?;
            Ok(page)
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);

    // One row per strip, each by its sheet name and what it shows.
    let rows: Vec<_> = d
        .app
        .page_rows()
        .into_iter()
        .filter(|r| r.page == page)
        .collect();
    let labels: Vec<_> = rows.iter().map(|r| r.label.as_str()).collect();
    assert_eq!(labels.len(), 2, "{labels:?}");
    assert!(
        labels[0].starts_with("20a  Terminal diagram · ") && labels[0].ends_with("-X1"),
        "{labels:?}"
    );
    assert!(
        labels[1].starts_with("20b") && labels[1].ends_with("-X2"),
        "{labels:?}"
    );

    // Opening the second row shows that sheet.
    d.app.show_sheet(page, rows[1].sheet);
    d.frame();
    let view = d.app.canvas.schematic_ref().unwrap();
    assert_eq!(view.sheet().map(|s| s.name.as_str()), Some("20b"));
    assert_eq!(view.sheet().and_then(|s| s.number), Some((2, 2)));
}
