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

pub(super) struct Drive {
    pub(super) ctx: egui::Context,
    pub(super) app: NovoApp,
}

impl Drive {
    pub(super) fn new(untitled: &Path) -> Drive {
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

    pub(super) fn frame(&mut self) {
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

    pub(super) fn doc(&self) -> &Document {
        self.app.document().expect("a project is open")
    }

    fn page(&self) -> PageId {
        self.app.current_page().expect("a page is shown")
    }

    /// I, the query, Enter, then a click on the page: one placement as the
    /// user does it. Returns the new symbol.
    pub(super) fn place(&mut self, query: &str, x: f64, y: f64) -> SymbolId {
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

/// What a saved and reopened project must keep: drawing pages, devices,
/// symbols, wires, terminals and potentials, in a comparable form. Opening
/// adds the generated pages a project lacks, so they are left out.
fn summary(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for page in project
        .pages_in_order()
        .into_iter()
        .filter(|p| !matches!(p.kind, novo_core::PageKind::Generated { .. }))
    {
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
    // Opening gave it its generated pages, after the drawing pages, and
    // still shows the first drawing page.
    let generated: Vec<String> = e
        .doc()
        .project()
        .pages_in_order()
        .into_iter()
        .filter(|p| matches!(p.kind, novo_core::PageKind::Generated { .. }))
        .map(|p| format!("{} {}", p.name, p.title))
        .collect();
    assert!(
        generated.iter().any(|p| p == "40 Terminal diagram"),
        "{generated:?}"
    );
    let shown = e.app.current_page.and_then(|p| e.doc().project().page(p));
    assert_eq!(shown.map(|p| p.name.as_str()), Some("1"));
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

    // Held and dragged more than 4 px, Space pans the drawing with the
    // pointer and opens nothing.
    let viewport = |d: &mut Drive| d.app.canvas.schematic_ref().unwrap().canvas.viewport;
    let before = viewport(&mut d);
    d.frame_with(vec![at.clone(), space(true)]);
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(703.0, 400.0))]);
    assert_eq!(viewport(&mut d).center, before.center, "not within 4 px");
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(760.0, 430.0))]);
    d.frame_with(vec![egui::Event::PointerMoved(egui::pos2(780.0, 440.0))]);
    d.frame_with(vec![space(false)]);
    d.frame();
    let after = viewport(&mut d);
    let moved = (after.center - before.center) * before.zoom;
    assert!(
        (moved.x + 80.0).abs() < 0.5 && (moved.y + 40.0).abs() < 0.5,
        "the drawing followed the pointer: {moved:?}"
    );
    assert!(!d.app.wb.field.is_open(), "a pan opens no field");

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

#[test]
fn a_deleted_generated_page_stays_deleted() {
    use novo_core::{DeviceTag, PageKind};
    let page_of = |d: &Drive, kind: &str| {
        d.doc()
            .project()
            .pages()
            .find(|p| matches!(&p.kind, PageKind::Generated { generator } if generator == kind))
            .map(|p| p.id)
    };
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert_eq!(page_of(&d, "toc"), None, "an empty project gets none");
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("A strip", |tx| {
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X1").unwrap())?;
            tx.add_terminal(strip, "1").map(drop)
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);

    // As on open: the terminal diagram and the contents arrive.
    d.app.add_generated_pages();
    let terminals = page_of(&d, "terminals").expect("a terminal diagram page");
    assert!(page_of(&d, "toc").is_some());

    // The user deletes it; the next open does not bring it back.
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("Delete page", |tx| tx.delete_page(terminals))
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    d.app.add_generated_pages();
    assert_eq!(page_of(&d, "terminals"), None);
}

#[test]
fn opening_an_issued_project_adds_no_generated_pages() {
    use novo_core::{DeviceTag, PageKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("A strip and an issue", |tx| {
            let strip = tx.add_terminal_strip(DeviceTag::parse("-X1").unwrap())?;
            tx.add_terminal(strip, "1")?;
            tx.issue_revision("A", "2026-10-15", "T", "First issue")
                .map(drop)
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    assert!(
        d.doc().project().pages().any(|p| p.frozen.is_some()),
        "the issue froze the drawing page"
    );

    d.app.add_generated_pages();
    let generated = d
        .doc()
        .project()
        .pages()
        .filter(|p| matches!(p.kind, PageKind::Generated { .. }))
        .count();
    assert_eq!(generated, 0, "an issued project is left as it is");
    assert!(
        d.app
            .messages()
            .iter()
            .any(|m| m.text == "Generated pages not added: the project has frozen pages"),
        "{:?}",
        d.app.messages()
    );
}

#[test]
fn plc_commands_add_a_card_send_a_signal_and_renumber() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let last = |d: &Drive| d.app.messages().last().map(|m| m.text.clone()).unwrap();

    // No card yet: to plc explains instead of failing.
    let sensor = d.place("pbnc", 40.0, 40.0);
    d.select(sensor);
    d.app.run_command(&ctx, "plc.to_plc", None);
    assert!(last(&d).contains("free DI channel"), "{}", last(&d));
    let depth = d.doc().undo_depth();

    // A bare `add card` says what it needs; a layout adds one card.
    d.app.run_command(&ctx, "plc.add_card", None);
    assert!(last(&d).contains("add card DI16"));
    d.app.run_command(&ctx, "plc.add_card", Some("DI16 DQ8"));
    assert!(last(&d).starts_with("Added card"), "{}", last(&d));
    assert_eq!(d.doc().undo_depth(), depth + 1);
    assert_eq!(novo_plc::cards(d.doc().project()).len(), 1);

    // To PLC draws a channel and wires it, in one undo step.
    let wires = d.doc().project().wires().count();
    let depth = d.doc().undo_depth();
    d.select(sensor);
    d.app.run_command(&ctx, "plc.to_plc", None);
    assert!(last(&d).starts_with("Sent to"), "{}", last(&d));
    assert_eq!(d.doc().project().wires().count(), wires + 1);
    assert_eq!(d.doc().undo_depth(), depth + 1);

    // Renumber on an ordered project opens no preview and adds no step.
    let depth = d.doc().undo_depth();
    d.app.run_command(&ctx, "plc.renumber_io", None);
    assert!(last(&d).contains("already in order"), "{}", last(&d));
    assert!(d.app.wb.renumber.is_none());
    assert_eq!(d.doc().undo_depth(), depth);

    // A gap opens the preview; Esc leaves the project alone, Enter applies.
    let card = novo_plc::cards(d.doc().project())[0].device;
    let doc = d.app.backend.document_mut().unwrap();
    let (_, changes) = doc
        .transact("Gap", |tx| {
            tx.set_property(card, "plc.start.i", Some(novo_core::Value::Int(8)))
        })
        .unwrap();
    d.app.changed_outside_canvas(&changes);
    let depth = d.doc().undo_depth();
    d.app.run_command(&ctx, "plc.renumber_io", None);
    assert!(d.app.wb.renumber.is_some(), "the preview is waiting");
    d.frame();
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    assert!(d.app.wb.renumber.is_none());
    assert_eq!(d.doc().undo_depth(), depth, "Esc changed nothing");
    d.app.run_command(&ctx, "plc.renumber_io", None);
    d.frame();
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(last(&d).starts_with("Renumbered"), "{}", last(&d));
    assert_eq!(d.doc().undo_depth(), depth + 1);
}

#[test]
fn the_tool_dock_switches_tools_and_steps_aside_for_a_drag() {
    use novo_editor::Tool;
    use novo_workflow::keymap::Focus;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.frame();
    let dock = || ctx.memory(|m| m.area_rect(egui::Id::new("tool_dock")));
    let home = dock().expect("the dock is shown");
    let (_, canvas) = d.app.wb.canvas_area.unwrap();
    assert!(
        (home.center().x - canvas.center().x).abs() < 1.0,
        "bottom centre"
    );

    // Connect is the third button: one click starts wiring, and keys stay
    // with the drawing.
    let side = novo_theme::tokens::CONTROL_HEIGHT + 4.0;
    let spacing = ctx.global_style().spacing.item_spacing.x;
    let margin = egui::Frame::popup(&ctx.global_style()).inner_margin.left as f32;
    let at = egui::pos2(
        home.left() + margin + 2.5 * side + 2.0 * spacing,
        home.center().y,
    );
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_with(vec![egui::Event::PointerMoved(at), button(true)]);
    d.frame_with(vec![button(false)]);
    d.frame();
    let tool = |d: &Drive| d.app.canvas.schematic_ref().unwrap().editor.tool().clone();
    assert!(matches!(tool(&d), Tool::Wire(_)), "{:?}", tool(&d));
    assert_eq!(d.app.key_focus(&ctx), Focus::Canvas);
    let centred =
        |r: Option<egui::Rect>| r.is_some_and(|r| (r.center().x - canvas.center().x).abs() < 1.0);
    assert!(centred(dock()), "an active tool alone does not move it");

    // A drag from the drawing that comes near the dock moves it aside.
    let home = dock().unwrap();
    let from = canvas.center();
    let near = egui::pos2(home.center().x, home.top() - 10.0);
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_with(vec![egui::Event::PointerMoved(from), press(from, true)]);
    d.frame_with(vec![egui::Event::PointerMoved(near)]);
    d.frame();
    let aside = dock().unwrap();
    assert!(aside.center().x > home.right(), "{aside:?} vs {home:?}");
    d.frame_with(vec![press(near, false)]);
    d.frame();
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.key(egui::Key::Escape, egui::Modifiers::NONE);
    d.frame();
    assert!(matches!(tool(&d), Tool::Select), "{:?}", tool(&d));
    assert!(centred(dock()), "back once the gesture ends");
}

#[test]
fn shift_w_switches_the_wiring_mode_and_the_setting_keeps_it() {
    use novo_editor::tool::WiringMode;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let mode = |d: &mut Drive| d.app.canvas.schematic().unwrap().editor.wire_mode();
    assert_eq!(mode(&mut d), WiringMode::Corners, "Corners is the default");

    let shift = egui::Modifiers::SHIFT;
    d.key(egui::Key::W, shift);
    d.frame();
    assert_eq!(mode(&mut d), WiringMode::Lines);
    assert_eq!(d.app.settings().wiring_mode(), WiringMode::Lines);

    // A held key repeats; only the first press counts.
    d.frame_with(vec![egui::Event::Key {
        key: egui::Key::W,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: shift,
    }]);
    d.frame();
    assert_eq!(mode(&mut d), WiringMode::Lines, "a repeat does not flip it");
    d.frame_with(vec![egui::Event::Key {
        key: egui::Key::W,
        physical_key: None,
        pressed: false,
        repeat: false,
        modifiers: shift,
    }]);

    d.key(egui::Key::W, shift);
    d.frame();
    assert_eq!(mode(&mut d), WiringMode::Corners);

    // A new session starts in the saved mode.
    let mut settings = Settings::default();
    settings.set_wiring_mode(WiringMode::Lines);
    let mut again = NovoApp::with_settings(
        Box::new(StorageBackend::with_untitled_dir(dir.path().join("u2"))),
        Box::new(EditorCanvas::default()),
        settings,
    );
    let ctx2 = egui::Context::default();
    novo_theme::apply(&ctx2);
    let mut out = ctx2.run_ui(egui::RawInput::default(), |ui| again.show(ui));
    out.textures_delta.clear();
    assert_eq!(
        again.canvas.schematic().map(|v| v.editor.wire_mode()),
        Some(WiringMode::Lines)
    );
}

/// One frame at egui time `time` seconds, with `events`.
fn frame_at(d: &mut Drive, time: f64, events: Vec<egui::Event>) {
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(1600.0, 900.0),
        )),
        time: Some(time),
        events,
        ..Default::default()
    };
    let app = &mut d.app;
    let mut output = d.ctx.run_ui(input, |ui| app.show(ui));
    output.textures_delta.clear();
}

fn key_event(key: egui::Key, pressed: bool) -> egui::Event {
    egui::Event::Key {
        key,
        physical_key: None,
        pressed,
        repeat: false,
        modifiers: egui::Modifiers::NONE,
    }
}

#[test]
fn a_held_place_key_opens_the_wheel_and_keys_pick_from_it() {
    use novo_editor::Tool;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.wb.keymap.shortcut_text("tool.place").is_some());
    // A refusal sets the editor's notice; the dock draws it (Error tint and
    // icon) without trouble, and the notice is what tells it apart.
    let view = d.app.canvas.schematic().unwrap();
    assert!(!view.editor.begin_place("no/such/symbol", None));
    assert!(view.editor.notice().is_some());
    d.frame();
    let text = d.app.canvas.schematic().unwrap().status_text();
    assert!(text.is_some_and(|t| t.contains("No symbol")));

    let placing = |d: &mut Drive| {
        matches!(
            d.app.canvas.schematic().unwrap().editor.tool(),
            Tool::Place(_)
        )
    };

    // A tap is still the command surface, opened on the release.
    let place = egui::Key::I;
    frame_at(
        &mut d,
        10.0,
        vec![egui::Event::PointerMoved(egui::pos2(800.0, 450.0))],
    );
    frame_at(&mut d, 10.1, vec![key_event(place, true)]);
    assert!(d.app.wb.wheel.is_none() && !d.app.wb.field.is_open());
    frame_at(&mut d, 10.15, vec![key_event(place, false)]);
    assert!(d.app.wb.field.is_open(), "a tap opens the command surface");
    d.app.wb.field.close();
    frame_at(&mut d, 10.2, vec![]);

    // A hold opens the wheel at the pointer; Esc closes it.
    frame_at(&mut d, 20.0, vec![key_event(place, true)]);
    frame_at(&mut d, 20.1, vec![]);
    assert!(d.app.wb.wheel.is_none(), "not yet");
    frame_at(&mut d, 20.3, vec![]);
    assert!(d.app.wb.wheel.is_some(), "held past 250 ms");
    frame_at(&mut d, 20.4, vec![key_event(place, false)]);
    assert!(
        d.app.wb.wheel.is_some(),
        "letting go over no item leaves it up"
    );
    frame_at(&mut d, 20.5, vec![key_event(egui::Key::Escape, true)]);
    assert!(d.app.wb.wheel.is_none());
    assert!(!placing(&mut d));

    // Keys 3 (Relay) then 1 (its first item) pick a coil, which follows
    // the pointer as in the place mode.
    d.app.run_command(&ctx, "tool.place_wheel", None);
    frame_at(&mut d, 30.0, vec![]);
    assert!(d.app.wb.wheel.is_some());
    frame_at(&mut d, 30.1, vec![key_event(egui::Key::Num3, true)]);
    frame_at(&mut d, 30.2, vec![key_event(egui::Key::Backspace, true)]);
    frame_at(&mut d, 30.3, vec![key_event(egui::Key::Num3, true)]);
    frame_at(&mut d, 30.4, vec![key_event(egui::Key::Num1, true)]);
    assert!(d.app.wb.wheel.is_none(), "picking closes the wheel");
    let tool = d.app.canvas.schematic().unwrap().editor.tool().clone();
    match tool {
        Tool::Place(p) => assert_eq!(p.symbol, "iec/coils/relay"),
        other => panic!("expected the coil to be in hand, got {other:?}"),
    }

    // Key 5 is Points: its first item is the potential feed, and every
    // item of every wedge exists in the library.
    d.app.run_command(&ctx, "tool.place_wheel", None);
    frame_at(&mut d, 40.0, vec![]);
    frame_at(&mut d, 40.1, vec![key_event(egui::Key::Num5, true)]);
    frame_at(&mut d, 40.2, vec![key_event(egui::Key::Num1, true)]);
    assert!(d.app.wb.wheel.is_none());
    match d.app.canvas.schematic().unwrap().editor.tool().clone() {
        Tool::Place(p) => assert_eq!(p.symbol, "iec/connectivity/potential_feed"),
        other => panic!("expected the feed in hand, got {other:?}"),
    }
    d.app.run_command(&ctx, "tool.place_wheel", None);
    frame_at(&mut d, 50.0, vec![]);
    for wedge in 0..5 {
        let items = d.app.wheel_items(wedge);
        assert!(!items.is_empty(), "wedge {wedge} has items");
    }
    assert_eq!(d.app.wheel_items(4).len(), 5, "Points has all five items");
}

#[test]
fn a_refusal_in_the_dock_and_the_place_key_come_from_the_keymap() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(d.app.wb.keymap.shortcut_text("tool.place").is_some());
    // A refusal sets the editor's notice; the dock draws it (Error tint and
    // icon) without trouble, and the notice is what tells it apart.
    let view = d.app.canvas.schematic().unwrap();
    assert!(!view.editor.begin_place("no/such/symbol", None));
    assert!(view.editor.notice().is_some());
    d.frame();
    let text = d.app.canvas.schematic().unwrap().status_text();
    assert!(text.is_some_and(|t| t.contains("No symbol")));
}

#[test]
fn the_inspector_edits_the_selected_device_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let inspector = || ctx.memory(|m| m.area_rect(egui::Id::new("inspector")));
    let k = d.place("km3", 80.0, 96.0);
    d.editor_leave();
    d.app.canvas.schematic().unwrap().canvas.selection.clear();
    d.frame();
    d.frame();
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("inspector"));
    let shown = |ctx: &egui::Context| ctx.memory(|m| m.areas().visible_last_frame(&layer));
    assert!(!shown(&ctx), "nothing selected, no inspector");

    d.select(k);
    d.frame();
    assert!(shown(&ctx), "{:?}", inspector());
    let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(k)).unwrap();
    let before = d
        .doc()
        .project()
        .device(device)
        .unwrap()
        .description
        .clone();

    // Type into the description and press Enter.
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("inspector_description")));
    d.frame();
    d.frame_with(vec![
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        },
        egui::Event::Text("Pump contactor".into()),
    ]);
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    let now = d
        .doc()
        .project()
        .device(device)
        .unwrap()
        .description
        .clone();
    assert_eq!(now, "Pump contactor");

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(
        d.doc().project().device(device).unwrap().description,
        before
    );
}

#[test]
fn a_double_click_on_a_device_puts_the_cursor_in_its_tag() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let coil = d.place("coil", 200.0, 144.0);
    d.frame();

    // The coil's body, on screen.
    let origin = d.doc().project().symbol(coil).unwrap().placement.origin;
    let view = d.app.canvas.schematic().unwrap();
    let at = view.canvas.viewport.world_to_screen(novo_canvas::Vec2::new(
        origin.x as f32 + 4.0,
        origin.y as f32 + 8.0,
    ));
    let rect = d.app.wb.canvas_area.unwrap().1;
    let pos = rect.min + egui::vec2(at.x, at.y);

    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame_at(&mut d, 50.0, vec![egui::Event::PointerMoved(pos)]);
    for (i, t) in [50.1, 50.15, 50.2, 50.25].into_iter().enumerate() {
        frame_at(&mut d, t, vec![button(i % 2 == 0)]);
    }
    frame_at(&mut d, 50.3, vec![]);
    frame_at(&mut d, 50.35, vec![]);
    assert_eq!(
        ctx.memory(|m| m.focused()),
        Some(egui::Id::new("inspector_tag")),
        "the tag field has the cursor"
    );
}

#[test]
fn an_inspector_edit_is_kept_when_the_selection_clears() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let k = d.place("km3", 80.0, 96.0);
    d.editor_leave();
    d.select(k);
    d.frame();
    let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(k)).unwrap();

    // Type into the description, then click away on empty canvas without
    // pressing Enter.
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("inspector_description")));
    d.frame();
    d.frame_with(vec![
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        },
        egui::Event::Text("Pump contactor".into()),
    ]);
    d.app.canvas.schematic().unwrap().canvas.selection.clear();
    d.frame();
    d.frame();
    assert_eq!(
        d.doc().project().device(device).unwrap().description,
        "Pump contactor",
        "the edit survives the selection clearing"
    );
}

#[test]
fn dragging_a_selection_box_reports_how_many_it_would_take() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.place("coil", 200.0, 144.0);
    let band = |d: &mut Drive| d.app.canvas.schematic().unwrap().band_count();
    assert_eq!(band(&mut d), None);

    // Press on empty canvas and drag across the coil.
    let (_, canvas) = d.app.wb.canvas_area.unwrap();
    let from = canvas.center() - egui::vec2(300.0, 200.0);
    let to = canvas.center() + egui::vec2(300.0, 200.0);
    let button = |pressed| egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    d.frame_with(vec![egui::Event::PointerMoved(from)]);
    d.frame_with(vec![button(true)]);
    d.frame_with(vec![egui::Event::PointerMoved(to)]);
    d.frame_with(vec![egui::Event::PointerMoved(to)]);
    assert!(band(&mut d).is_some(), "a box is being dragged");
    d.frame_with(vec![egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    }]);
    d.frame();
    assert_eq!(band(&mut d), None, "gone when released");
}

#[test]
fn link_joins_a_contact_to_a_coil_by_tags_or_by_selection() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let inspector = || ctx.memory(|m| m.area_rect(egui::Id::new("inspector")));
    let k = d.place("km3", 80.0, 96.0);
    d.editor_leave();
    d.app.canvas.schematic().unwrap().canvas.selection.clear();
    d.frame();
    d.frame();
    let layer = egui::LayerId::new(egui::Order::Middle, egui::Id::new("inspector"));
    let shown = |ctx: &egui::Context| ctx.memory(|m| m.areas().visible_last_frame(&layer));
    assert!(!shown(&ctx), "nothing selected, no inspector");

    d.select(k);
    d.frame();
    assert!(shown(&ctx), "{:?}", inspector());
    let device = novo_editor::edit::device_of(d.doc().project(), AnyId::Symbol(k)).unwrap();
    let before = d
        .doc()
        .project()
        .device(device)
        .unwrap()
        .description
        .clone();

    // Type into the description and press Enter.
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("inspector_description")));
    d.frame();
    d.frame_with(vec![
        egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        },
        egui::Event::Text("Pump contactor".into()),
    ]);
    d.key(egui::Key::Enter, egui::Modifiers::NONE);
    d.frame();
    let now = d
        .doc()
        .project()
        .device(device)
        .unwrap()
        .description
        .clone();
    assert_eq!(now, "Pump contactor");

    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(
        d.doc().project().device(device).unwrap().description,
        before
    );
    let coil = d.place("coil", 200.0, 144.0);
    let contact = d.place("no", 320.0, 144.0);
    let (coil_tag, contact_tag) = (d.tag_of(coil), d.tag_of(contact));
    assert_ne!(coil_tag, contact_tag);
    let last = |d: &Drive| d.app.messages().last().map(|m| m.text.clone()).unwrap();

    // A bare link with nothing selected asks for the two.
    d.app.run_command(&ctx, "xref.link", None);
    assert!(last(&d).contains("Pick the contact"), "{}", last(&d));
    // The tags name them: the contact joins the coil's device.
    let depth = d.doc().undo_depth();
    d.app.run_command(
        &ctx,
        "xref.link",
        Some(&format!("{contact_tag} {coil_tag}")),
    );
    assert_eq!(last(&d), "Linked");
    assert_eq!(d.tag_of(contact), coil_tag);
    assert_eq!(d.doc().undo_depth(), depth + 1);
    // Again: a refusal reads in plain words and changes nothing.
    d.app
        .run_command(&ctx, "xref.link", Some(&format!("{coil_tag} {coil_tag}")));
    assert!(!last(&d).is_empty() && last(&d) != "Linked", "{}", last(&d));
    assert_eq!(d.doc().undo_depth(), depth + 1);
}

#[test]
fn double_clicking_a_tag_opens_a_rename_box() {
    use novo_canvas::model::LabelKind;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let coil = d.place("coil", 200.0, 144.0);
    d.editor_leave();
    d.frame();

    // Find the tag on screen: the point whose pick is a device-tag label.
    let rect = d.app.wb.canvas_area.unwrap().1;
    let project = d.doc().project().clone();
    let view = d.app.canvas.schematic().unwrap();
    let origin = project.symbol(coil).unwrap().placement.origin;
    let scene = view.scene().unwrap();
    let zoom = view.canvas.viewport.zoom;
    let mut found = None;
    'scan: for dy in -40..=40 {
        for dx in -40..=40 {
            let w =
                novo_canvas::Vec2::new(origin.x as f32 + dx as f32, origin.y as f32 + dy as f32);
            if let Some(hit) = scene.scene().pick(w, 2.0, zoom)
                && let Some(l) = scene.label_hit(&project, &hit)
                && l.kind == LabelKind::DeviceTag
            {
                found = Some(w);
                break 'scan;
            }
        }
    }
    let w = found.expect("the coil shows a tag");
    let at = view.canvas.viewport.world_to_screen(w);
    let pos = rect.min + egui::vec2(at.x, at.y);
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame_at(&mut d, 50.0, vec![egui::Event::PointerMoved(pos)]);
    for (i, t) in [50.1, 50.15, 50.2, 50.25].into_iter().enumerate() {
        frame_at(&mut d, t, vec![button(i % 2 == 0)]);
    }
    frame_at(&mut d, 50.3, vec![]);
    assert!(
        d.app.canvas.schematic().unwrap().renaming(),
        "the rename box is open on the tag"
    );
    assert_ne!(
        ctx.memory(|m| m.focused()),
        Some(egui::Id::new("inspector_tag")),
        "the properties do not also open and take the cursor"
    );
    assert!(d.app.wb.queued.is_empty(), "no properties command queued");
}

#[test]
fn a_held_place_key_with_repeats_opens_only_the_wheel() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let place = egui::Key::I;
    let repeat = egui::Event::Key {
        key: place,
        physical_key: None,
        pressed: true,
        repeat: true,
        modifiers: egui::Modifiers::NONE,
    };
    frame_at(
        &mut d,
        10.0,
        vec![egui::Event::PointerMoved(egui::pos2(800.0, 450.0))],
    );
    frame_at(&mut d, 10.1, vec![key_event(place, true)]);
    frame_at(
        &mut d,
        10.2,
        vec![repeat.clone(), egui::Event::Text("i".into())],
    );
    frame_at(
        &mut d,
        10.3,
        vec![repeat.clone(), egui::Event::Text("i".into())],
    );
    frame_at(&mut d, 10.4, vec![repeat, egui::Event::Text("i".into())]);
    assert!(d.app.wb.wheel.is_some(), "repeats do not delay the wheel");
    frame_at(&mut d, 10.5, vec![key_event(place, false)]);
    assert!(d.app.wb.wheel.is_some());
    assert!(
        !d.app.wb.field.is_open(),
        "the part search does not open under the wheel"
    );
}

#[test]
fn double_clicking_a_terminal_a_feed_or_an_interruption_name_renames_it() {
    use novo_canvas::model::LabelKind;
    use novo_core::{Pin, Point, PotentialKind, SymbolRole, TerminalKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.doc().project().pages().next().unwrap().id;
    let (syms, _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("roles", |tx| {
            let x1 = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1")?)?;
            let t = tx.add_terminal(x1, "1")?;
            let pins = || vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)];
            let terminal = tx.place_symbol(
                page,
                "iec/terminals/end_bracket",
                novo_core::Placement::at(Point::new(100.0, 100.0)),
                pins(),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            let l1 = tx.add_potential("L1", PotentialKind::Line)?;
            let feed = tx.place_symbol(
                page,
                "iec/connectivity/potential_feed",
                novo_core::Placement::at(Point::new(200.0, 100.0)),
                vec![Pin::new("1", 0.0, 8.0)],
                SymbolRole::PotentialFeed { potential: l1 },
            )?;
            let _ = TerminalKind::Feedthrough;
            let jump = tx.place_symbol(
                page,
                "iec/connectivity/interruption",
                novo_core::Placement::at(Point::new(300.0, 100.0)),
                vec![Pin::new("1", 0.0, 0.0)],
                SymbolRole::Interruption { name: "5".into() },
            )?;
            Ok((terminal, feed, jump))
        })
        .unwrap();
    d.frame();
    let (terminal, feed, jump) = syms;

    let double_click_label = |d: &mut Drive, sym, t0: f64| -> bool {
        let project = d.doc().project().clone();
        let rect = d.app.wb.canvas_area.unwrap().1;
        let view = d.app.canvas.schematic().unwrap();
        let origin = project.symbol(sym).unwrap().placement.origin;
        let scene = view.scene().unwrap();
        let zoom = view.canvas.viewport.zoom;
        // The tolerance the canvas itself picks with.
        let tol = view.canvas.viewport.screen_to_world_len(5.0);
        let mut found = None;
        'scan: for dy in -80..=80 {
            for dx in -80..=80 {
                let w = novo_canvas::Vec2::new(
                    origin.x as f32 + dx as f32 * 0.5,
                    origin.y as f32 + dy as f32 * 0.5,
                );
                if let Some(hit) = scene.scene().pick(w, tol, zoom)
                    && let Some(l) = scene.label_hit(&project, &hit)
                    && matches!(
                        l.kind,
                        LabelKind::DeviceTag
                            | LabelKind::TerminalName
                            | LabelKind::InterruptionName
                            | LabelKind::Potential
                    )
                {
                    found = Some(w);
                    break 'scan;
                }
            }
        }
        let Some(w) = found else { return false };
        let at = view.canvas.viewport.world_to_screen(w);
        let pos = rect.min + egui::vec2(at.x, at.y);
        let button = |pressed| egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        frame_at(d, t0, vec![egui::Event::PointerMoved(pos)]);
        for (i, dt) in [0.1, 0.15, 0.2, 0.25].into_iter().enumerate() {
            frame_at(d, t0 + dt, vec![button(i % 2 == 0)]);
        }
        frame_at(d, t0 + 0.3, vec![]);
        true
    };

    for (sym, t0, text) in [(terminal, 50.0, "7"), (feed, 60.0, "L3"), (jump, 70.0, "9")] {
        assert!(
            double_click_label(&mut d, sym, t0),
            "the label is on the canvas"
        );
        let view = d.app.canvas.schematic().unwrap();
        assert!(view.renaming(), "the rename box opens for {text}");
        view.set_rename_text(text);
        frame_at(&mut d, t0 + 1.0, vec![key_event(egui::Key::Enter, true)]);
        frame_at(&mut d, t0 + 1.1, vec![key_event(egui::Key::Enter, false)]);
        assert!(!d.app.canvas.schematic().unwrap().renaming(), "{text}");
    }
    let project = d.doc().project();
    let SymbolRole::Terminal { terminals } = &project.symbol(terminal).unwrap().role else {
        panic!()
    };
    assert_eq!(project.terminal(terminals[0]).unwrap().name, "7");
    let SymbolRole::PotentialFeed { potential } = project.symbol(feed).unwrap().role else {
        panic!()
    };
    assert_eq!(project.potential(potential).unwrap().name, "L3");
    let SymbolRole::Interruption { name } = &project.symbol(jump).unwrap().role else {
        panic!()
    };
    assert_eq!(name, "9");
}

#[test]
fn the_preferences_window_sets_the_auto_connection_mode() {
    use novo_editor::tool::ConnectMode;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert_eq!(
        d.app.canvas.schematic().unwrap().editor.connect_mode(),
        ConnectMode::Draw,
        "drawn by default"
    );
    assert!(super::workbench::app_runs("app.preferences"));
    d.app.run_command(&ctx, "app.preferences", None);
    d.frame();
    assert!(d.app.wb.preferences, "the window opens");
    d.frame();
    d.app.settings.set_connect_mode(ConnectMode::Ask);
    d.frame();
    assert_eq!(
        d.app.canvas.schematic().unwrap().editor.connect_mode(),
        ConnectMode::Ask
    );
    d.app.settings.set_connect_mode(ConnectMode::Never);
    d.frame();
    assert_eq!(
        d.app.canvas.schematic().unwrap().editor.connect_mode(),
        ConnectMode::Never
    );
    frame_at(&mut d, 90.0, vec![key_event(egui::Key::Escape, true)]);
    assert!(!d.app.wb.preferences, "Esc closes it");
}

#[test]
fn the_issue_revision_window_proposes_everything_and_issues_in_one_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert!(super::workbench::app_runs("revision.issue"));
    d.app
        .run_command(&ctx, "revision.issue", Some("Motor M3 added"));
    d.frame();
    let draft = d.app.wb.issue.take().expect("the window opens");
    assert_eq!(draft.index, "A", "the first index of the rule");
    assert_eq!(draft.description, "Motor M3 added", "text after the verb");
    assert_eq!(draft.date.len(), 10);
    let mut draft = draft;
    draft.author = "TV".into();
    d.app.submit_issue(draft);
    d.frame();
    assert!(
        d.app.wb.issue.is_none(),
        "the window closes: {:?}",
        d.app.wb.issue
    );
    assert_eq!(d.app.settings.initials, "TV", "the initials are remembered");
    let project = d.app.backend.document().unwrap().project();
    let latest = project.latest_revision().unwrap();
    assert_eq!(
        (
            latest.index.as_str(),
            latest.author.as_str(),
            latest.description.as_str()
        ),
        ("A", "TV", "Motor M3 added")
    );
    assert!(
        project
            .pages()
            .all(|p| p.frozen.as_ref().is_some_and(|f| f.revision == "A"))
    );
    // The next proposal follows, and a taken index is refused in place.
    d.app.run_command(&ctx, "revision.issue", None);
    assert_eq!(d.app.wb.issue.as_ref().unwrap().index, "B");
    let mut again = d.app.wb.issue.take().unwrap();
    again.index = "A".into();
    d.app.submit_issue(again);
    assert!(d.app.wb.issue.as_ref().unwrap().error.is_some());
    d.app.run(&ctx, Action::Undo);
    d.app.run(&ctx, Action::Undo);
    d.frame();
}

#[test]
fn the_title_block_prints_the_set_revision_after_an_issue() {
    use novo_canvas::model::item_id;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.app.current_page.expect("a page is shown");
    let texts = |d: &Drive| -> Vec<String> {
        let view = d.app.canvas.schematic_ref().unwrap();
        let scene = view.scene().unwrap().scene();
        scene
            .get(item_id(page))
            .unwrap()
            .labels
            .iter()
            .map(|l| l.text.clone())
            .collect()
    };
    assert!(!texts(&d).iter().any(|t| t.contains("set A")));
    let doc = d.app.backend.document_mut().unwrap();
    let outcome = doc
        .execute(novo_core::Command::IssueRevision {
            index: "A".into(),
            date: "2026-10-15".into(),
            author: "TV".into(),
            description: String::new(),
        })
        .unwrap();
    d.app.changed_outside_canvas(&outcome.changes);
    d.frame();
    d.frame();
    assert!(
        texts(&d).iter().any(|t| t == "(set A)"),
        "a page without a kept snapshot shows only the set: {:?}",
        texts(&d)
    );
}

#[test]
fn changes_mode_marks_added_changed_and_removed_objects_since_a_revision() {
    use novo_canvas::marks::{MarkKind, MarkTarget};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    // Without a revision there is nothing to compare with.
    d.app.run_command(&ctx, "view.changes", None);
    assert!(d.app.wb.changes.is_none());
    let kept = d.place("mcb3", 80.0, 48.0);
    let gone = d.place("mcb3", 80.0, 120.0);
    d.frame();
    let issued = d
        .app
        .backend
        .issue_revision(None, "2026-10-15", "TV", "First");
    d.app.changed_outside_canvas(&issued.unwrap().changes);
    let added = d.place("mcb3", 160.0, 48.0);
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Move", |tx| {
            tx.update::<novo_core::SymbolInstance>(kept, |s| s.placement.origin.x += 10.0)
        })
        .unwrap();
    d.app
        .backend
        .document_mut()
        .unwrap()
        .transact("Delete", |tx| tx.remove(gone))
        .unwrap();
    d.frame();
    d.app.run_command(&ctx, "view.changes", None);
    d.frame();
    d.frame();
    assert!(d.app.wb.changes.is_some(), "the mode opens against A");
    let marks = d
        .app
        .canvas
        .schematic_ref()
        .unwrap()
        .canvas
        .marks
        .list
        .clone();
    let on = |kind, id: SymbolId| {
        marks.iter().any(|m| {
            m.kind == kind && m.target == MarkTarget::Item(novo_canvas::model::item_id(id))
        })
    };
    assert!(on(MarkKind::Added, added), "{marks:?}");
    assert!(on(MarkKind::Modified, kept));
    assert!(
        marks
            .iter()
            .any(|m| m.kind == MarkKind::Removed && matches!(m.target, MarkTarget::Area(_))),
        "a dashed ghost where the removed symbol was"
    );
    d.app.run_command(&ctx, "view.changes", None);
    d.frame();
    assert!(d.app.wb.changes.is_none(), "the same command turns it off");
}

#[test]
fn the_command_list_and_the_part_search_show_many_rows() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let height = |d: &Drive, id: &str| {
        d.ctx
            .memory(|m| m.area_rect(egui::Id::new(id)))
            .map_or(0.0, |r| r.height())
    };

    // Near the bottom of the window the lists still open with room.
    let low = egui::pos2(400.0, 860.0);
    d.app.wb.field.open(Mode::All, Some(low));
    d.frame();
    d.frame();
    assert!(
        height(&d, "command_field") > 250.0,
        "the command list shows more than a couple of rows, got {}",
        height(&d, "command_field")
    );
    d.app.wb.field.close();
    d.frame();

    d.app.wb.field.open(Mode::Place, Some(low));
    d.frame();
    d.frame();
    assert!(height(&d, "command_field") > 250.0);
}

#[test]
fn the_part_search_opens_with_room_near_the_bottom_of_the_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let low = egui::pos2(800.0, 880.0);
    frame_at(&mut d, 10.0, vec![egui::Event::PointerMoved(low)]);
    d.app.canvas.schematic().unwrap().open_insert_at(low);
    frame_at(&mut d, 10.1, vec![]);
    frame_at(&mut d, 10.2, vec![]);
    let area = ctx
        .memory(|m| m.area_rect(egui::Id::new("novo-editor-insert")))
        .expect("the search is open");
    assert!(area.height() > 250.0, "got {}", area.height());
    assert!(area.bottom() <= ctx.content_rect().bottom());
}

#[test]
fn a_double_click_on_bare_grid_starts_a_wire_without_the_wire_tool() {
    use novo_editor::Tool;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    d.editor_leave();
    d.frame();
    assert!(matches!(
        d.app.canvas.schematic().unwrap().editor.tool(),
        Tool::Select
    ));
    let rect = d.app.wb.canvas_area.unwrap().1;
    let pos = rect.center() + egui::vec2(40.0, 30.0);
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    frame_at(&mut d, 50.0, vec![egui::Event::PointerMoved(pos)]);
    for (i, t) in [50.1, 50.15, 50.2, 50.25].into_iter().enumerate() {
        frame_at(&mut d, t, vec![button(i % 2 == 0)]);
    }
    frame_at(&mut d, 50.3, vec![]);
    match d.app.canvas.schematic().unwrap().editor.tool() {
        Tool::Wire(w) => assert!(w.start.is_some(), "a wire is started"),
        other => panic!("expected the wire tool, got {other:?}"),
    }
}

#[test]
fn toggle_pe_is_a_command_and_an_earthing_wire_marks_terminals() {
    use novo_core::{Endpoint, Pin, Point, Rotation, SymbolRole, TerminalKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.doc().project().pages().next().unwrap().id;
    let ((term, earth), _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            let x1 = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1")?)?;
            let t = tx.add_terminal(x1, "1")?;
            let pins = || vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)];
            let term = tx.place_symbol(
                page,
                "iec/terminals/end_bracket",
                novo_core::Placement::at(Point::new(100.0, 100.0)),
                pins(),
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            let earth = tx.place_symbol(
                page,
                "iec/earth/earth",
                novo_core::Placement::at(Point::new(100.0, 140.0)),
                vec![Pin::new("1", 0.0, -2.5)],
                SymbolRole::Unassigned,
            )?;
            let _ = Rotation::R0;
            Ok((term, earth))
        })
        .unwrap();
    d.frame();
    let kind = |d: &Drive| {
        let p = d.doc().project();
        let SymbolRole::Terminal { terminals } = &p.symbol(term).unwrap().role else {
            panic!()
        };
        p.terminal(terminals[0]).unwrap().kind
    };
    assert_eq!(kind(&d), TerminalKind::Feedthrough);

    // The toggle works on the selected terminal and undoes in one step.
    d.select(term);
    d.frame();
    assert!(super::workbench::app_runs("terminal.toggle_pe"));
    d.app.run_command(&ctx, "terminal.toggle_pe", None);
    d.frame();
    assert_eq!(kind(&d), TerminalKind::ProtectiveEarth);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(kind(&d), TerminalKind::Feedthrough);

    // A wire drawn to the earth symbol earths the terminal on its own, in
    // the same undo step as the wire.
    let (_, changes) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("wire", |tx| {
            tx.connect(Endpoint::pin(term, 1), Endpoint::pin(earth, 0))
        })
        .unwrap();
    d.app.mark_earthed_terminals(&changes);
    assert_eq!(kind(&d), TerminalKind::ProtectiveEarth);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(kind(&d), TerminalKind::Feedthrough, "one undo takes both");
}

/// A page with two terminals, one of them next to an earth symbol (not yet
/// wired), and three interruption points out of order.
fn potentials_page(d: &mut Drive) -> (SymbolId, SymbolId, SymbolId, Vec<SymbolId>) {
    use novo_core::{Endpoint, Pin, PotentialKind, SymbolRole};
    let page = d.page();
    let ((t1, t2, earth, jumps), _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            let x1 = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1")?)?;
            tx.add_potential("PE", PotentialKind::ProtectiveEarth)?;
            let mut terminal = |name: &str, x: f64| {
                let t = tx.add_terminal(x1, name)?;
                tx.place_symbol(
                    page,
                    "iec/terminals/terminal",
                    novo_core::Placement::at(Point::new(x, 100.0)),
                    vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                    SymbolRole::Terminal { terminals: vec![t] },
                )
            };
            let t1 = terminal("1", 100.0)?;
            let t2 = terminal("2", 120.0)?;
            let earth = tx.place_symbol(
                page,
                "iec/earth/protective_earth",
                novo_core::Placement::at(Point::new(100.0, 160.0)),
                vec![Pin::new("PE", 0.0, 0.0)],
                SymbolRole::Unassigned,
            )?;
            let mut jumps = Vec::new();
            for (x, name) in [(300.0, "7"), (260.0, "7"), (220.0, "3")] {
                jumps.push(tx.place_symbol(
                    page,
                    "iec/connectivity/interruption",
                    novo_core::Placement::at(Point::new(x, 40.0)),
                    vec![Pin::new("1", 0.0, 0.0)],
                    SymbolRole::Interruption { name: name.into() },
                )?);
            }
            let _ = Endpoint::pin(t1, 0);
            Ok((t1, t2, earth, jumps))
        })
        .unwrap();
    (t1, t2, earth, jumps)
}

fn interruption_names(d: &Drive, ids: &[SymbolId]) -> Vec<String> {
    ids.iter()
        .map(|&i| match &d.doc().project().symbol(i).unwrap().role {
            novo_core::SymbolRole::Interruption { name } => name.clone(),
            _ => unreachable!(),
        })
        .collect()
}

#[test]
fn number_automatically_renumbers_interruption_points_in_one_undo_step() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let (_, _, _, jumps) = potentials_page(&mut d);
    d.frame();
    assert!(super::workbench::app_runs("interruption.renumber"));
    // Nothing selected: every point, reading order, the pair stays a pair.
    d.app.run_command(&ctx, "interruption.renumber", None);
    d.frame();
    assert_eq!(interruption_names(&d, &jumps), ["2", "2", "1"]);
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(interruption_names(&d, &jumps), ["7", "7", "3"]);
}

#[test]
fn a_pe_terminal_set_by_hand_carries_the_override_diamond() {
    use novo_canvas::MarkKind;
    use novo_core::{PeOverride, SymbolRole, TerminalKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let (t1, _, _, _) = potentials_page(&mut d);
    d.select(t1);
    let terminal = |d: &Drive| {
        let SymbolRole::Terminal { terminals } = &d.doc().project().symbol(t1).unwrap().role else {
            panic!()
        };
        d.doc().project().terminal(terminals[0]).unwrap().clone()
    };
    assert!(super::workbench::app_runs("terminal.toggle_pe"));
    d.app.run_command(&ctx, "terminal.toggle_pe", None);
    d.frame();
    let t = terminal(&d);
    assert_eq!(t.kind, TerminalKind::ProtectiveEarth);
    assert_eq!(t.pe_override, PeOverride::ForcedPe);
    let marks = &d.app.canvas.schematic_ref().unwrap().canvas.marks.list;
    assert!(
        marks.iter().any(|m| m.kind == MarkKind::Override),
        "a by-hand PE terminal carries the override diamond"
    );
}

#[test]
fn the_moving_preference_and_shift_choose_whether_wires_go_along() {
    use novo_editor::Tool;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let k = d.place("km3", 80.0, 96.0);
    d.editor_leave();
    d.select(k);
    d.frame();
    let carry = |d: &mut Drive| match d.app.canvas.schematic().unwrap().editor.tool() {
        Tool::Move(m) => m.carry,
        other => panic!("expected a move, got {other:?}"),
    };
    let hint = |d: &mut Drive| d.app.canvas.schematic().unwrap().status_text().unwrap();

    // Default: leave the wires; the hint offers Shift for the other one.
    d.app
        .canvas
        .schematic()
        .unwrap()
        .editor
        .begin_move(&[AnyId::Symbol(k)]);
    d.frame();
    assert!(!carry(&mut d));
    assert!(hint(&mut d).contains("Shift: take the wires along"));
    d.app.canvas.schematic().unwrap().editor.leave_tool();

    // Preference "takes its wires along": the hint flips.
    d.app.settings.move_carries = true;
    d.frame();
    d.app
        .canvas
        .schematic()
        .unwrap()
        .editor
        .begin_move(&[AnyId::Symbol(k)]);
    d.frame();
    assert!(carry(&mut d));
    assert!(hint(&mut d).contains("Shift: leave the wires"));
}

#[test]
fn tab_and_q_steer_a_wire_in_progress_from_the_keyboard() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let a = d.place("mcb3", 80.0, 60.0);
    let b = d.place("mcb3", 160.0, 60.0);
    let wires = d.doc().project().wires().count();

    // Start on a pin of A, then hold the pointer over the middle of B.
    d.app.run_command(&ctx, "tool.connect", None);
    let start = d.pin(a, true);
    d.click(start);
    let page = d.page();
    let origin = d.doc().project().symbol(b).unwrap().placement.origin;
    let body = Point::new(origin.x + 4.0, origin.y + 4.0);
    {
        let app = &mut d.app;
        let view = app.canvas.schematic().expect("the editor canvas");
        let doc = app.backend.document_mut().expect("a project");
        view.editor.hover(doc, page, body, TOL);
    }
    assert!(d.app.canvas.schematic_ref().unwrap().editor.is_wiring());

    // The keymap hands Q and Tab to the wire while it is in progress.
    d.key(egui::Key::Tab, egui::Modifiers::NONE);
    assert!(
        matches!(
            d.app.canvas.schematic_ref().unwrap().editor.target(),
            Some(novo_editor::geometry::Target::Endpoint(
                novo_core::Endpoint::Pin { symbol, .. },
                _
            )) if symbol == b
        ),
        "Tab aims at a pin of the part under the pointer"
    );
    d.key(egui::Key::Q, egui::Modifiers::NONE);
    assert_eq!(d.doc().project().wires().count(), wires + 1);
    assert!(!d.app.wb.field.is_open(), "Q did not open the part search");
}

#[test]
fn c_draws_a_line_and_the_wires_it_touches_become_one_cable() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let a = d.place("mcb3", 80.0, 60.0);
    let b = d.place("mcb3", 200.0, 60.0);
    let pins = |d: &Drive, s: SymbolId| {
        let sym = d.doc().project().symbol(s).unwrap();
        (0..sym.pins.len() as u16)
            .map(|i| sym.pin_position(i).unwrap())
            .collect::<Vec<_>>()
    };
    let (pa, pb) = (pins(&d, a), pins(&d, b));
    // Bottom pins of A to top pins of B, three wires.
    let mut lower: Vec<Point> = pa.iter().copied().filter(|p| p.y > 60.0).collect();
    let mut upper: Vec<Point> = pb.iter().copied().filter(|p| p.y < 60.0 + 20.0).collect();
    lower.sort_by(|p, q| p.x.total_cmp(&q.x));
    upper.sort_by(|p, q| p.x.total_cmp(&q.x));
    for (from, to) in lower.iter().zip(&upper) {
        d.wire(*from, *to);
    }
    let wires: Vec<_> = d.doc().project().wires().map(|w| w.id).collect();
    assert!(wires.len() >= 2, "wired: {}", wires.len());
    assert_eq!(d.doc().project().cables().count(), 0);

    // C, a line across the wires between the two parts, Enter.
    d.app.run_command(&ctx, "tool.cable", None);
    let (x, y0, y1) = (140.0, 40.0, 140.0);
    d.click(Point::new(x, y0));
    d.click(Point::new(x, y1));
    {
        let app = &mut d.app;
        let view = app.canvas.schematic().expect("the editor canvas");
        let doc = app.backend.document_mut().expect("a project");
        assert!(view.editor.key(doc, novo_editor::EditorKey::Finish, &[]));
    }
    d.frame();

    let project = d.doc().project();
    assert_eq!(
        project.cables().count(),
        1,
        "one cable for the touched wires"
    );
    let cable = project.cables().next().unwrap();
    let on_cable = wires
        .iter()
        .filter(|&&w| project.cable_of_wire(w) == Some(cable.id))
        .count();
    assert_eq!(on_cable, wires.len());

    // One undo step takes the cable away again.
    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert_eq!(d.doc().project().cables().count(), 0);
}

#[test]
fn reconnecting_a_wire_to_earth_marks_the_terminal_too() {
    use novo_core::{AnyId, ChangeSet, Endpoint, Pin, Point, SymbolRole, TerminalKind};
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let page = d.page();
    let ((term, wire), _) = d
        .app
        .backend
        .document_mut()
        .unwrap()
        .transact("setup", |tx| {
            let x1 = tx.add_terminal_strip(novo_core::DeviceTag::parse("-X1")?)?;
            let t = tx.add_terminal(x1, "1")?;
            let term = tx.place_symbol(
                page,
                "iec/terminals/end_bracket",
                novo_core::Placement::at(Point::new(100.0, 100.0)),
                vec![Pin::new("1", 0.0, -2.5), Pin::new("2", 0.0, 2.5)],
                SymbolRole::Terminal { terminals: vec![t] },
            )?;
            let earth = tx.place_symbol(
                page,
                "iec/earth/earth",
                novo_core::Placement::at(Point::new(100.0, 140.0)),
                vec![Pin::new("1", 0.0, -2.5)],
                SymbolRole::Unassigned,
            )?;
            let wire = tx.connect(Endpoint::pin(term, 1), Endpoint::pin(earth, 0))?;
            Ok((term, wire))
        })
        .unwrap();
    d.frame();
    let kind = |d: &Drive| {
        let p = d.doc().project();
        let SymbolRole::Terminal { terminals } = &p.symbol(term).unwrap().role else {
            panic!()
        };
        p.terminal(terminals[0]).unwrap().kind
    };
    assert_eq!(kind(&d), TerminalKind::Feedthrough);
    // The wire was changed, not added (an end moved onto the earth pin).
    let mut changes = ChangeSet::default();
    changes.modified.insert(AnyId::Wire(wire));
    d.app.mark_earthed_terminals(&changes);
    assert_eq!(kind(&d), TerminalKind::ProtectiveEarth);
}

#[test]
fn ctrl_d_copies_the_selection_with_the_next_tag_and_selects_the_copy() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    let coil = d.place("coil", 80.0, 60.0);
    let first = d.tag_of(coil);
    d.select(coil);
    let symbols = |d: &Drive| d.doc().project().symbols().count();
    assert_eq!(symbols(&d), 1);

    d.key(egui::Key::D, egui::Modifiers::COMMAND);
    assert_eq!(symbols(&d), 2, "a copy");
    let copy = match d.app.selection().as_slice() {
        [AnyId::Symbol(s)] => *s,
        other => panic!("the copy is selected, got {other:?}"),
    };
    assert_ne!(copy, coil);
    let second = d.tag_of(copy);
    assert_ne!(second, first, "next free tag");
    let at = |d: &Drive, s: SymbolId| d.doc().project().symbol(s).unwrap().placement.origin;
    assert!(
        at(&d, copy).x > at(&d, coil).x,
        "to the right of the original"
    );

    // Repeating copies the copy, one step further.
    d.key(egui::Key::D, egui::Modifiers::COMMAND);
    assert_eq!(symbols(&d), 3);
    let third = match d.app.selection().as_slice() {
        [AnyId::Symbol(s)] => *s,
        other => panic!("{other:?}"),
    };
    assert!(at(&d, third).x > at(&d, copy).x);
    assert_ne!(d.tag_of(third), second);

    // One undo step per Ctrl+D.
    d.app.backend.document_mut().unwrap().undo().unwrap();
    assert_eq!(symbols(&d), 2);
}

#[test]
fn the_project_standard_defaults_to_iec_and_shows_in_the_title() {
    use novo_symbols::Pack;
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    d.app.run(&ctx, Action::NewProject);
    d.frame();
    assert_eq!(d.app.standard(), Pack::Iec);
    assert_eq!(d.app.standard_suffix(), "");
    assert!(d.doc().project().info().rules.standard.is_empty());

    // The command opens the Project settings window.
    assert!(super::workbench::app_runs("project.standard"));
    d.app.run_command(&ctx, "project.standard", None);
    assert!(d.app.wb.project_settings);
    d.frame();

    // Choosing NFPA is saved in the project, shown beside the name, and
    // one undo step takes it back.
    d.app.set_standard(Pack::Nfpa);
    d.frame();
    assert_eq!(d.doc().project().info().rules.standard, "nfpa");
    assert_eq!(d.app.standard(), Pack::Nfpa);
    assert!(
        d.app.window_title.contains("NFPA"),
        "{}",
        d.app.window_title
    );
    d.app.run(&ctx, Action::Undo);
    d.frame();
    assert_eq!(d.app.standard(), Pack::Iec);
    assert!(!d.app.window_title.contains("NFPA"));
}

#[test]
fn the_parts_command_opens_and_closes_the_parts_window() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let ctx = d.ctx.clone();
    assert!(super::workbench::app_runs("parts.browser"));
    d.app.run_command(&ctx, "parts.browser", None);
    d.frame();
    d.frame();
    assert!(d.app.wb.parts.is_some(), "the window opens");
    d.app.wb.parts = None;
    d.frame();
    assert!(d.app.wb.parts.is_none());
}
