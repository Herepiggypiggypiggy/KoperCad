//! Headless runs of the steps of the lessons generated-pages, wire-list-and-bom and pdf-export.
//! Each lesson is started the way the Learn window starts it (`start_lesson`, so its own
//! `project.json` opens) and each instruction a step gives is carried out through the app: key
//! chords as egui key events, the command field (`type_command`), real pointer clicks on the
//! menu bar, the page list and dialog buttons (found by their painted label), typed text. The
//! lesson's own runner must then tick the step from its `expect`; a step that stays open, or is
//! ticked before the person did anything, fails. A second test per lesson replays the `solve`
//! scripts alone.
//!
//! Not driven headless, and what is checked instead:
//! - the file dialog of File > Import spreadsheet (`import_sheet_from` stands in for its answer;
//!   the preview and Enter are driven for real);
//! - Excel (the workbook is edited with `novo_interop::sheets`);
//! - the PDF viewer (the exported file is read: it exists and has bookmarks; bookmark texts,
//!   links and the title block are asserted by `novo-tests` documents.rs);
//! - the canvas drawing (no graphics adapter): pointers on report pages are model checks;
//! - exports of a lesson project go to Documents, which the test machine lacks, so the project is
//!   saved first (`save_to`) and the files land next to it.

use std::time::Duration;

use eframe::egui;
use novo_core::Project;
use novo_learn::Phase;
use novo_workflow::palette::Mode;

use super::gate1_tests::Drive;

/// A lesson run in a headless window.
struct Ox {
    d: Drive,
    _dir: tempfile::TempDir,
    /// The clock of the headless window, seconds.
    time: f64,
    /// Every text painted in the last frame with its rectangle, bottom layer first: the
    /// visible labels a person can click.
    labels: Vec<(String, egui::Rect)>,
}

fn collect_text(shape: &egui::epaint::Shape, out: &mut Vec<(String, egui::Rect)>) {
    use egui::epaint::Shape;
    match shape {
        Shape::Text(t) => {
            let r = t.galley.rect;
            out.push((
                t.galley.text().to_string(),
                egui::Rect::from_min_size(t.pos + r.min.to_vec2(), r.size()),
            ));
        }
        Shape::Vec(v) => v.iter().for_each(|s| collect_text(s, out)),
        _ => {}
    }
}

impl Ox {
    /// Starts the shipped lesson `id` from its own start project.
    fn start(id: &str) -> Ox {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.app.learn.use_progress_file(dir.path().join("learn.json"));
        d.app.learn.advance_after = Duration::ZERO;
        d.app.start_lesson(id);
        let mut lx = Ox {
            d,
            _dir: dir,
            time: 1.0,
            labels: Vec::new(),
        };
        lx.frames(3);
        assert!(lx.d.app.learn_active(), "lesson `{id}` did not start");
        lx
    }

    /// One frame with these events; the painted texts are remembered.
    fn frame_with(&mut self, events: Vec<egui::Event>) {
        self.time += 0.05;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(self.time),
            events,
            ..Default::default()
        };
        let app = &mut self.d.app;
        let mut output = self.d.ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
        self.labels.clear();
        for clipped in &output.shapes {
            collect_text(&clipped.shape, &mut self.labels);
        }
    }

    fn frame(&mut self) {
        self.frame_with(Vec::new());
    }

    /// The rectangle of the topmost painted text that is exactly `text`.
    fn label(&self, text: &str) -> Option<egui::Rect> {
        // The lesson card repeats control names in its `how` lines (bold spans paint as
        // labels of their own): skip the labels from its "N OF M" header to its Restart
        // button, so a menu drawn after the card still counts.
        let end = self
            .labels
            .iter()
            .rposition(|(t, _)| t.contains("Restart lesson"));
        let start = self
            .labels
            .iter()
            .rposition(|(t, _)| t.contains('·') && t.contains(" OF "));
        let card = match (start, end) {
            (Some(s), Some(e)) if s < e => Some(s..=e),
            _ => None,
        };
        self.labels
            .iter()
            .enumerate()
            .rev()
            .filter(|(i, _)| !card.as_ref().is_some_and(|c| c.contains(i)))
            .map(|(_, l)| l)
            .find(|(t, _)| t.trim() == text)
            .map(|(_, r)| *r)
    }

    /// Clicks the control whose visible label is `text`.
    fn click_label(&mut self, text: &str) {
        let rect = self.label(text).unwrap_or_else(|| {
            let seen: Vec<&str> = self.labels.iter().map(|(t, _)| t.as_str()).collect();
            panic!("no control labelled `{text}` is on screen; labels: {seen:?}")
        });
        self.button(
            rect.center(),
            egui::PointerButton::Primary,
            egui::Modifiers::NONE,
        );
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    fn project(&self) -> &Project {
        self.d.doc().project()
    }

    /// Id of the open step.
    fn step(&self) -> String {
        self.d
            .app
            .learn
            .run
            .as_ref()
            .and_then(|r| r.step())
            .map(|s| s.id.clone())
            .unwrap_or_else(|| "<finished>".to_owned())
    }

    fn phase(&self) -> Phase {
        self.d.app.learn.run.as_ref().unwrap().phase()
    }

    /// The step `id` is open and its check does not hold yet.
    fn open(&mut self, id: &str) {
        self.frames(2);
        assert_eq!(
            self.step(),
            id,
            "step `{id}` should be the open one (it was ticked before the person did anything, \
             or an earlier step did not tick)"
        );
        assert_eq!(self.phase(), Phase::Active, "`{id}` ticked on its own");
    }

    /// After the person's actions the step `id` is ticked: the runner moved past it, or a
    /// reading step waits for Next, which is pressed.
    fn ticked(&mut self, id: &str) {
        self.frames(3);
        if self.step() == id {
            assert_eq!(
                self.phase(),
                Phase::Ticked,
                "step `{id}` did not tick after its instruction ran. {}",
                self.say()
            );
            self.next();
        }
        self.frames(2);
    }

    /// A reading step: Next.
    fn read(&mut self, id: &str) {
        self.open_reading(id);
        self.next();
        self.frames(2);
    }

    fn open_reading(&mut self, id: &str) {
        self.frames(2);
        assert_eq!(self.step(), id, "reading step `{id}` should be open");
    }

    fn next(&mut self) {
        self.d.app.learn_move(super::learn::Move::Next);
        self.frames(2);
    }

    /// The last toast, for failure messages.
    fn say(&self) -> String {
        let last: Vec<String> = self
            .d
            .app
            .messages()
            .iter()
            .rev()
            .take(3)
            .map(|m| m.text.clone())
            .collect();
        format!("messages (latest first): {last:?}")
    }

    // ----- keys and the command field ---------------------------------------------------

    fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame_with(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame();
    }

    fn press(&mut self, key: egui::Key) {
        self.key(key, egui::Modifiers::NONE);
    }

    /// Ctrl+K, the words, Enter.
    fn palette(&mut self, words: &str) {
        assert!(
            self.d
                .app
                .type_command(&self.d.ctx.clone(), Mode::All, words),
            "the command field has no row for `{words}`"
        );
        self.frames(2);
    }

    fn palette_place(&mut self, words: &str) {
        assert!(
            self.d
                .app
                .type_command(&self.d.ctx.clone(), Mode::Place, words),
            "the place field has no row for `{words}`"
        );
        self.frames(2);
    }

    // ----- the pointer -------------------------------------------------------------------

    /// The screen position of a point of the shown page; the view is moved to show it first.
    fn screen(&mut self, x: f64, y: f64) -> egui::Pos2 {
        for _ in 0..3 {
            let (_, rect) = self.d.app.wb.canvas_area.expect("the canvas was shown");
            let view = self.d.app.canvas.schematic().expect("the editor canvas");
            let p = novo_canvas::Vec2::new(x as f32, y as f32);
            let at = view.canvas.viewport.world_to_screen(p);
            let pos = rect.min + egui::vec2(at.x, at.y);
            if rect.shrink(40.0).contains(pos) {
                return pos;
            }
            let near = novo_canvas::Aabb::new(
                p - novo_canvas::Vec2::splat(8.0),
                p + novo_canvas::Vec2::splat(8.0),
            );
            view.canvas.reveal_area(near);
            self.frame();
        }
        panic!("({x}, {y}) cannot be brought on screen");
    }

    fn button(&mut self, pos: egui::Pos2, button: egui::PointerButton, modifiers: egui::Modifiers) {
        let ev = |pressed| egui::Event::PointerButton {
            pos,
            button,
            pressed,
            modifiers,
        };
        self.frame_with(vec![egui::Event::PointerMoved(pos)]);
        self.frame_with(vec![ev(true)]);
        self.frame_with(vec![ev(false)]);
        self.frame();
    }

    fn click(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.button(pos, egui::PointerButton::Primary, egui::Modifiers::NONE);
    }
}

// ----- the `solve` scripts ----------------------------------------------------------------

/// Runs one operation of a step's `solve` through the app, as "Do it for me" would.
fn solve_op(l: &mut Ox, op: &novo_learn::lesson::SolveOp) {
    use novo_learn::lesson::SolveOp;
    match op {
        SolveOp::Command(c) => {
            let ctx = l.d.ctx.clone();
            l.d.app.run_command(&ctx, &c.command, c.arg.as_deref());
            l.frames(2);
        }
        SolveOp::Page(p) => {
            let id = l
                .project()
                .pages()
                .find(|x| x.name == p.page || x.title == p.page)
                .unwrap_or_else(|| panic!("solve: no page `{}`", p.page))
                .id;
            l.d.app.show_page(id);
            l.frames(2);
        }
        SolveOp::Place(p) => {
            let p = &p.place;
            let library = novo_symbols::Library::builtin().expect("library");
            let symbol = library.get(&p.symbol).expect("symbol");
            let page = l
                .project()
                .pages()
                .find(|x| x.name == p.page || x.title == p.page)
                .expect("page")
                .id;
            let doc = l.d.app.backend.document_mut().expect("project");
            let (_, changes) = doc
                .transact("Place", |tx| {
                    novo_editor::edit::place(
                        tx,
                        page,
                        symbol,
                        novo_core::Placement::at(novo_core::Point::new(p.x, p.y)),
                        &novo_editor::edit::PlaceOptions::new(),
                    )
                })
                .expect("place");
            l.d.app.changed_outside_canvas(&changes);
            l.frames(2);
        }
        SolveOp::AssignPart(a) => {
            let a = &a.assign_part;
            assign_part(l, &a.device, &a.manufacturer, &a.order_number);
        }
        SolveOp::IssueRevision(i) => {
            l.d.app
                .open_issue_revision(Some(&i.issue_revision.description));
            let draft = l.d.app.wb.issue.take().expect("the issue window opens");
            l.d.app.submit_issue(draft);
            l.frames(2);
        }
        other => panic!("these lessons use no solve op {other:?}"),
    }
}

/// What the spreadsheet import does for a device: a part with this maker and order number.
fn assign_part(l: &mut Ox, device: &str, manufacturer: &str, order_number: &str) {
    let id = l
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with(device))
        .unwrap_or_else(|| panic!("no device {device}"))
        .id;
    let doc = l.d.app.backend.document_mut().expect("project");
    let (_, changes) = doc
        .transact("Assign part", |tx| {
            let part = tx.add_part(manufacturer, order_number)?;
            tx.assign_part(id, part)
        })
        .expect("assign");
    l.d.app.changed_outside_canvas(&changes);
    l.frames(2);
}

/// Replays a whole lesson with its `solve` scripts, in order: every action step must tick
/// from its script alone, a reading step takes Next.
fn replay(id: &str) {
    replay_in(id, None);
}

/// Like [`replay`], with the project saved in a temporary folder first (exports need a place).
fn replay_saved(id: &str) {
    let dir = tempfile::tempdir().unwrap();
    replay_in(id, Some(dir.path().join("Pump.novo")));
}

fn replay_in(id: &str, save: Option<std::path::PathBuf>) {
    let mut l = Ox::start(id);
    if let Some(path) = save {
        assert!(l.d.app.save_to(&path), "{}", l.say());
    }
    let steps = l.d.app.learn.run.as_ref().unwrap().lesson().steps.clone();
    for step in &steps {
        l.frames(2);
        assert_eq!(l.step(), step.id, "{id}: the runner is on another step");
        if step.expect.is_some() {
            assert_eq!(
                l.phase(),
                Phase::Active,
                "{id}/{}: the check already holds when the step opens",
                step.id
            );
            assert!(!step.solve.is_empty(), "{id}/{}: no solve", step.id);
        }
        for op in &step.solve {
            solve_op(&mut l, op);
        }
        if step.id == "issue-submit" {
            // A reading step has no solve (GAP): what the step says is done here.
            solve_op(
                &mut l,
                &novo_learn::lesson::SolveOp::IssueRevision(novo_learn::lesson::IssueRevisionOp {
                    issue_revision: novo_learn::lesson::IssueRevisionArgs {
                        description: "First issue".into(),
                    },
                }),
            );
        }
        if step.expect.is_some() {
            l.ticked(&step.id);
        } else {
            l.read(&step.id);
        }
    }
    assert_eq!(l.phase(), Phase::Finished, "{id}: the last step is done");
}

/// The page list: the "Pages" button opens it (a window of 1700 px or more keeps it open),
/// then the row is clicked.
fn open_page_from_list(l: &mut Ox, row: &str) {
    if l.label(row).is_none() {
        l.click_label("Pages");
        l.frames(2);
    }
    l.click_label(row);
    l.frames(3);
}

/// The sheets of the generated pages: name and report.
fn sheets(project: &Project) -> Vec<(String, String)> {
    novo_reports::LiveReports::standard()
        .generated_sheets(project)
        .into_iter()
        .map(|s| (s.name, s.report))
        .collect()
}

/// The text rows of a report, as the page prints them.
fn report_rows(project: &Project, id: &str) -> Vec<Vec<String>> {
    let output = novo_reports::generate_all(project)
        .into_iter()
        .find(|r| r.report == id)
        .expect("report");
    output.sections.iter().flat_map(|s| s.text_rows()).collect()
}

fn shown(l: &Ox) -> String {
    let page = l.d.app.current_page().expect("a page is shown");
    let page = l.project().page(page).expect("page");
    format!("{} {}", page.name, page.title)
}

#[test]
fn generated_pages_every_step_works_from_its_own_start() {
    let mut l = Ox::start("generated-pages");
    l.read("intro");

    // open-contents: the Pages button, then the row under Contents and reports.
    l.open("open-contents");
    open_page_from_list(&mut l, "0a  Table of contents");
    l.ticked("open-contents");
    assert_eq!(shown(&l), "0 Table of contents");

    l.read("read-contents");

    // back-to-drawing: Page Down twice.
    l.open("back-to-drawing");
    l.press(egui::Key::PageDown);
    assert_eq!(shown(&l), "1 Main circuit");
    l.press(egui::Key::PageDown);
    l.ticked("back-to-drawing");
    assert_eq!(shown(&l), "2 Control circuit");

    // place-lamp: Q, the words, Enter, a click right of the coil.
    l.open("place-lamp");
    let before = l.project().symbols().count();
    l.palette_place("lamp green");
    l.click(104.0, 168.0);
    assert_eq!(l.project().symbols().count(), before + 1, "{}", l.say());
    l.ticked("place-lamp");
    let lamp = l
        .project()
        .devices()
        .find(|d| d.description.to_lowercase().contains("lamp"))
        .expect("the lamp is a device");
    println!("lamp tag {}", lamp.tag);

    // open-device-list: Page Down twice more.
    l.open("open-device-list");
    l.press(egui::Key::PageDown);
    assert_eq!(shown(&l), "10 Bill of materials");
    l.press(egui::Key::PageDown);
    l.ticked("open-device-list");
    assert_eq!(shown(&l), "20 Device list");

    // new-row: the device list has a row for the lamp (reading step).
    l.open_reading("new-row");
    {
        let rows = report_rows(l.project(), "devices");
        let at = rows
            .iter()
            .position(|r| r[0].ends_with("-P1"))
            .unwrap_or_else(|| panic!("no lamp row in {rows:?}"));
        // The first row sits at y = 35 mm and rows are 6 mm high: the pointer's y = 59.
        assert_eq!(35 + 6 * at, 59, "the lamp is the fifth row: {rows:?}");
        assert_eq!(rows[at][3], "Indicator lamp, green");
        assert_eq!(rows[at][7], "=A1+CAB/2.3", "drawn at page 2, column 3");
    }
    l.next();

    // sheet-names: the bill of materials is 10a + 10b; the letters skip i, l, o.
    l.open_reading("sheet-names");
    {
        let sheets = sheets(l.project());
        let names: Vec<&str> = sheets.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["0a", "0b", "10a", "10b", "20", "30", "40", "50a", "50b"],
            "the lamp has no part: the bill of materials has two sheets"
        );
    }
    l.next();
    l.open_reading("what-you-edit");
}

#[test]
fn generated_pages_solve_scripts_replay() {
    replay("generated-pages");
}

// ----- menus ---------------------------------------------------------------------------------

/// Opens a menu of the menu bar by its title (the first label with that text, which is the
/// menu bar: "Reports" is also the name of a workspace tab) and clicks the entries in turn.
fn menu(l: &mut Ox, path: &[&str]) {
    l.frames(2);
    let first = l
        .labels
        .iter()
        .find(|(t, _)| t == path[0])
        .unwrap_or_else(|| panic!("no menu {}", path[0]))
        .1;
    l.button(
        first.center(),
        egui::PointerButton::Primary,
        egui::Modifiers::NONE,
    );
    l.frames(2);
    for entry in &path[1..] {
        l.click_label(entry);
        l.frames(2);
    }
}

fn texts(l: &Ox) -> Vec<String> {
    l.labels.iter().map(|(t, _)| t.clone()).collect()
}

#[test]
fn wire_list_and_bom_every_step_works_from_its_own_start() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = Ox::start("wire-list-and-bom");
    // A person's lesson project is not saved yet and exports into Documents\KoperCAD; the
    // headless machine has no Documents folder, so the project is saved first.
    let file = dir.path().join("Pump.novo");
    assert!(l.d.app.save_to(&file), "{}", l.say());
    l.read("intro");

    // open-wire-list: Reports > Wire list.
    l.open("open-wire-list");
    menu(&mut l, &["Reports", "Wire list"]);
    l.ticked("open-wire-list");
    assert_eq!(l.d.app.workspace(), super::workspace::Workspace::Reports);
    assert!(texts(&l).iter().any(|t| t == "Wire list"));

    // find-wire: wire 4 is the row the text names; the filter keeps the U1 wires.
    l.open_reading("find-wire");
    {
        let rows = report_rows(l.project(), "wires");
        let row = rows.iter().find(|r| r[0] == "4").expect("wire 4");
        assert_eq!(row[1], "=A1+CAB-X1:1");
        assert_eq!(row[2], "=A1+FLD-M1:U1");
        assert_eq!(row[6], "=A1-W1 BK");
    }
    assert!(texts(&l).iter().any(|t| t == "=A1+FLD-M1:U1"));
    assert!(texts(&l).iter().any(|t| t == "=A1+CAB-F1:2"));
    l.click_label("Filter rows (Ctrl+F)");
    l.frame_with(vec![egui::Event::Text("U1".into())]);
    l.frames(2);
    assert!(
        texts(&l).iter().any(|t| t == "=A1+FLD-M1:U1"),
        "the U1 wire stays"
    );
    assert!(
        !texts(&l).iter().any(|t| t == "=A1+CAB-F1:2"),
        "wires without U1 are filtered away"
    );
    l.next();

    // open-cable-diagram: Reports > Cable diagram.
    l.open("open-cable-diagram");
    menu(&mut l, &["Reports", "Cable diagram"]);
    l.ticked("open-cable-diagram");
    {
        let rows = report_rows(l.project(), "cables");
        assert_eq!(rows[0][0], "=A1-W1");
        assert_eq!(
            (
                rows[0][5].as_str(),
                rows[0][6].as_str(),
                rows[0][7].as_str()
            ),
            ("4", "4", "0")
        );
    }

    // open-bom: Reports > Bill of materials; see-missing: the second section.
    l.open("open-bom");
    menu(&mut l, &["Reports", "Bill of materials"]);
    l.ticked("open-bom");
    {
        let rows = report_rows(l.project(), "bom");
        let block = rows
            .iter()
            .find(|r| r[3] == "3044076")
            .expect("terminal block row");
        assert_eq!(block[1], "3");
        assert!(block[7].contains("X1:1") && block[7].contains("X1:3"));
    }
    l.open_reading("see-missing");
    {
        let output = novo_reports::generate_all(l.project())
            .into_iter()
            .find(|r| r.report == "bom")
            .unwrap();
        let missing = output
            .sections
            .iter()
            .find(|s| s.title == "Devices without parts")
            .expect("section");
        let rows = missing.text_rows();
        assert_eq!(rows.len(), 1);
        assert!(rows[0][0].ends_with("-P1"));
    }
    l.next();

    // export-sheet: File > Export interchange > Export for bulk edit (.xlsx).
    l.open("export-sheet");
    menu(
        &mut l,
        &["File", "Export interchange", "Export for bulk edit (.xlsx)"],
    );
    l.ticked("export-sheet");
    let xlsx =
        l.d.app
            .wb
            .last_export
            .clone()
            .expect("the export names its file");
    assert!(xlsx.extension().is_some_and(|e| e == "xlsx"), "{xlsx:?}");
    println!("sheet file {xlsx:?} {}", l.say());

    // edit-part: what a person does in Excel, done on the workbook.
    l.open_reading("edit-part");
    {
        let bytes = std::fs::read(&xlsx).unwrap();
        let mut book = novo_interop::sheets::read_file("Pump sheets.xlsx", &bytes).expect("read");
        let table = &mut book.tables[0];
        println!("devices columns {:?}", table.headers);
        let tag = table.column("Tag").expect("Tag column");
        let maker = table.column("Manufacturer").expect("Manufacturer column");
        let order = table.column("Order number").expect("Order number column");
        let row = table
            .rows
            .iter_mut()
            .find(|r| r.cells[tag].as_text().ends_with("-P1"))
            .expect("the -P1 row");
        row.cells[maker] = novo_interop::sheets::Cell::text("Siemens");
        row.cells[order] = novo_interop::sheets::Cell::text("3SU1106-6AA40-1AA0");
        std::fs::write(&xlsx, novo_interop::sheets::xlsx_bytes(&book).unwrap()).unwrap();
    }
    l.next();

    // import-part: File > Import spreadsheet opens a file dialog (not driven headless); the
    // dialog's answer is the saved file. Then the preview, and Enter applies it.
    l.open("import-part");
    l.d.app.import_sheet_from(&xlsx);
    l.frames(3);
    assert!(
        texts(&l).iter().any(|t| t.starts_with("Apply 1 change")),
        "{:?}",
        texts(&l)
    );
    l.press(egui::Key::Enter);
    l.ticked("import-part");
    let lamp = l
        .project()
        .devices()
        .find(|d| d.tag.to_string().ends_with("-P1"))
        .unwrap();
    let part = l
        .project()
        .part(*lamp.parts.first().expect("the lamp has a part"))
        .unwrap();
    assert_eq!(
        (part.manufacturer.as_str(), part.order_number.as_str()),
        ("Siemens", "3SU1106-6AA40-1AA0")
    );

    // bom-follows: one section, the lamp is a row with Qty 1.
    l.open_reading("bom-follows");
    {
        let output = novo_reports::generate_all(l.project())
            .into_iter()
            .find(|r| r.report == "bom")
            .unwrap();
        assert_eq!(
            output.sections.len(),
            1,
            "the section Devices without parts is gone"
        );
        let rows = output.sections[0].text_rows();
        let lamp = rows
            .iter()
            .find(|r| r[3] == "3SU1106-6AA40-1AA0")
            .expect("lamp row");
        assert_eq!(lamp[1], "1");
    }
    l.next();

    // export-bom: the command field, as the step says; the file sits next to the project.
    l.open("export-bom");
    l.palette("export bom");
    l.ticked("export-bom");
    let made: Vec<String> = std::fs::read_dir(dir.path())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with("Bill of materials.xlsx"))
        .collect();
    assert_eq!(made.len(), 1, "{made:?} {}", l.say());
    println!("bom file {made:?}");
}

#[test]
fn wire_list_and_bom_solve_scripts_replay() {
    let dir = tempfile::tempdir().unwrap();
    // The replay needs a saved project for the exports (see above); it saves it the first time
    // a step needs a file, through the same hook the main test uses.
    let _ = dir;
    replay_saved("wire-list-and-bom");
}

#[test]
fn pdf_export_every_step_works_from_its_own_start() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = Ox::start("pdf-export");
    // Saved first: a lesson project exports into Documents\KoperCAD, which this machine lacks.
    let file = dir.path().join("Pump.novo");
    assert!(l.d.app.save_to(&file), "{}", l.say());
    l.d.app.wb.pdf_dir = Some(dir.path().to_owned());
    assert!(l.project().latest_revision().is_none(), "no revision yet");
    l.read("intro");

    // issue-open: Ctrl+K, issue, Enter. The window proposes index A.
    l.open("issue-open");
    l.palette("issue");
    l.ticked("issue-open");
    assert!(
        texts(&l).iter().any(|t| t == "Issue revision A"),
        "{:?}",
        texts(&l)
    );
    assert!(texts(&l).iter().any(|t| t == "What changed"));
    assert!(texts(&l).iter().any(|t| t == "Index"));
    assert!(texts(&l).iter().any(|t| t == "Date"));
    assert!(texts(&l).iter().any(|t| t == "By"));

    // issue-submit: type the description, click "Issue A".
    l.open_reading("issue-submit");
    l.frame_with(vec![egui::Event::Text("First issue".into())]);
    l.frames(2);
    l.click_label("Issue A");
    l.frames(3);
    let latest = l.project().latest_revision().expect("revision A is issued");
    assert_eq!(latest.index, "A");
    assert_eq!(latest.description, "First issue");
    assert!(l.say().contains("Issued revision A"), "{}", l.say());
    for page in l.project().pages() {
        let generated = matches!(page.kind, novo_core::PageKind::Generated { .. });
        assert_eq!(
            page.frozen.is_some(),
            !generated,
            "{} {}",
            page.name,
            page.title
        );
    }
    l.next();

    // title-block: the revision row prints A and the date (model: the page is frozen with A).
    l.open_reading("title-block");
    l.next();

    // export: Ctrl+K, export pdf, Enter. The file sits in the PDF folder (the test points it at the project folder).
    l.open("export");
    l.palette("export pdf");
    l.ticked("export");
    let pdf = file.with_extension("pdf");
    let bytes = std::fs::read(&pdf).unwrap_or_else(|e| panic!("{pdf:?}: {e} {}", l.say()));
    assert!(bytes.starts_with(b"%PDF"));
    assert!(bytes.windows(8).any(|w| w == b"Outlines"), "bookmarks");
    assert!(l.say().contains("Exported"), "{}", l.say());

    // the reading steps: where, bookmarks, links, freeze, pages.
    l.read("where-is-it");
    l.read("check-bookmarks");
    l.read("check-links");
    l.read("freeze");
    // A drawn page changed since the issue gets the dot: the app lists the changed pages.
    l.open_reading("pages-and-next-time");
}

#[test]
fn pdf_export_solve_scripts_replay() {
    replay_saved("pdf-export");
}

/// The click paths the steps give next to the keys work as well.
#[test]
fn the_menu_routes_and_the_page_list_click_work() {
    let dir = tempfile::tempdir().unwrap();
    let mut l = Ox::start("pdf-export");
    assert!(l.d.app.save_to(&dir.path().join("Pump.novo")));
    l.d.app.wb.pdf_dir = Some(dir.path().to_owned());
    menu(&mut l, &["File", "Revisions", "Issue revision"]);
    assert!(
        texts(&l).iter().any(|t| t == "Issue revision A"),
        "File > Revisions > Issue revision"
    );
    l.press(egui::Key::Escape);
    l.frames(2);
    menu(&mut l, &["File", "Export drawings", "Export PDF"]);
    assert!(
        l.say().contains("Exported"),
        "File > Export drawings > Export PDF: {}",
        l.say()
    );
    menu(
        &mut l,
        &["File", "Export reports", "Export bill of materials"],
    );
    assert!(
        l.say().contains("Bill of materials"),
        "File > Export reports > Export bill of materials: {}",
        l.say()
    );

    // generated-pages: a click on the row in the page list also reaches page 2.
    let mut l = Ox::start("generated-pages");
    open_page_from_list(&mut l, "0a  Table of contents");
    assert_eq!(shown(&l), "0 Table of contents");
    open_page_from_list(&mut l, "Control circuit");
    assert_eq!(shown(&l), "2 Control circuit");
}
