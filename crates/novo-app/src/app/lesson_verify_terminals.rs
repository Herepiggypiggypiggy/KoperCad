//! Headless runs of the steps of the lessons terminal-strips, connection-screens and
//! interruption-points. Each lesson is started the way the Learn window starts it
//! (`start_lesson`, so its own `project.json` opens), and every step is carried out through
//! the app in the order the lesson gives: key chords as egui key events, the command field
//! (`type_command`, which is Ctrl+K, the typed words and Enter), real pointer clicks at the
//! screen position of a world point, and double and right clicks. The lesson's own runner
//! then has to tick the step from its `expect`; a step that stays open, or that was already
//! ticked when it opened, fails the test.
//!
//! What cannot be driven headless: a hover (the tooltips named in the notes) and the context
//! menu popup rows (the row is a button in an egui popup; the test runs the command the row
//! runs, through `run_command`, after opening the menu with a real right click so that the
//! row's presence is asserted from the editor's own menu model).

use std::time::Duration;

use eframe::egui;
use novo_core::{AnyId, PageId, Project};
use novo_learn::Phase;
use novo_workflow::palette::Mode;

use super::gate1_tests::Drive;

/// A lesson run in a headless window.
pub(super) struct Lx {
    pub(super) d: Drive,
    _dir: tempfile::TempDir,
    /// The clock of the headless window, seconds.
    time: f64,
    /// Every text painted in the last frame with its rectangle, bottom layer first: the
    /// visible labels a person can click.
    pub(super) labels: Vec<(String, egui::Rect)>,
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

impl Lx {
    /// Starts the shipped lesson `id` from its own start project.
    pub(super) fn start(id: &str) -> Lx {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.app.learn.use_progress_file(dir.path().join("learn.json"));
        d.app.learn.advance_after = Duration::ZERO;
        d.app.start_lesson(id);
        let mut lx = Lx {
            d,
            _dir: dir,
            time: 1.0,
            labels: Vec::new(),
        };
        lx.frames(3);
        // The pointer rests over the drawing, as a person's does while typing keys.
        if let Some((_, rect)) = lx.d.app.wb.canvas_area {
            lx.frame_with(vec![egui::Event::PointerMoved(rect.center())]);
        }
        lx.frames(2);
        assert!(lx.d.app.learn_active(), "lesson `{id}` did not start");
        lx
    }

    /// One frame with these events; the painted texts are remembered.
    pub(super) fn frame_with(&mut self, events: Vec<egui::Event>) {
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

    pub(super) fn frame(&mut self) {
        self.frame_with(Vec::new());
    }

    /// The rectangle of the topmost painted text that is exactly `text`.
    pub(super) fn label(&self, text: &str) -> Option<egui::Rect> {
        self.labels
            .iter()
            .rev()
            .find(|(t, _)| t.trim() == text)
            .map(|(_, r)| *r)
    }

    /// The rectangle of the first (lowest layer) painted text that is exactly `text`: the
    /// window's own control when the lesson card repeats the word.
    pub(super) fn label_first(&self, text: &str) -> Option<egui::Rect> {
        self.labels
            .iter()
            .find(|(t, _)| t.trim() == text)
            .map(|(_, r)| *r)
    }

    /// Clicks the window control (not the lesson card) whose visible label is `text`.
    pub(super) fn click_control(&mut self, text: &str) {
        let rect = self
            .label_first(text)
            .unwrap_or_else(|| panic!("no control labelled `{text}` is on screen"));
        self.button(
            rect.center(),
            egui::PointerButton::Primary,
            egui::Modifiers::NONE,
        );
    }

    /// Clicks the control whose visible label is `text`.
    pub(super) fn click_label(&mut self, text: &str) {
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

    pub(super) fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    pub(super) fn project(&self) -> &Project {
        self.d.doc().project()
    }

    /// Id of the open step.
    pub(super) fn step(&self) -> String {
        self.d
            .app
            .learn
            .run
            .as_ref()
            .and_then(|r| r.step())
            .map(|s| s.id.clone())
            .unwrap_or_else(|| "<finished>".to_owned())
    }

    pub(super) fn phase(&self) -> Phase {
        self.d.app.learn.run.as_ref().unwrap().phase()
    }

    /// The step `id` is open and its check does not hold yet.
    pub(super) fn open(&mut self, id: &str) {
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
    pub(super) fn ticked(&mut self, id: &str) {
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
    pub(super) fn read(&mut self, id: &str) {
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
    pub(super) fn say(&self) -> String {
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

    pub(super) fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        self.frame_with(vec![egui::Event::ModifiersChanged(modifiers)]);
        self.frame_with(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers,
        }]);
        self.frame_with(vec![egui::Event::Key {
            key,
            physical_key: None,
            pressed: false,
            repeat: false,
            modifiers,
        }]);
        self.frame_with(vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)]);
    }

    pub(super) fn press(&mut self, key: egui::Key) {
        self.key(key, egui::Modifiers::NONE);
    }

    /// Ctrl+K, the words, Enter.
    pub(super) fn palette(&mut self, words: &str) {
        assert!(
            self.d
                .app
                .type_command(&self.d.ctx.clone(), Mode::All, words),
            "the command field has no row for `{words}`"
        );
        self.frames(2);
    }

    // ----- the pointer -------------------------------------------------------------------

    pub(super) fn page(&self) -> PageId {
        self.d.app.current_page().expect("a page is shown")
    }

    /// The screen position of a point of the shown page; the view is moved to show it first.
    pub(super) fn screen(&mut self, x: f64, y: f64) -> egui::Pos2 {
        for _ in 0..3 {
            let (_, rect) = self.d.app.wb.canvas_area.expect("the canvas was shown");
            let view = self.d.app.canvas.schematic().expect("the editor canvas");
            let p = novo_canvas::Vec2::new(x as f32, y as f32);
            let at = view.canvas.viewport.world_to_screen(p);
            let pos = rect.min + egui::vec2(at.x, at.y);
            if rect.shrink(120.0).contains(pos) {
                return pos;
            }
            // The page opens fitted to its content, so a point beyond the
            // drawing can sit at the edge, under the docks: centre it.
            view.canvas.viewport.center = p;
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
        self.frame_with(vec![
            egui::Event::ModifiersChanged(modifiers),
            egui::Event::PointerMoved(pos),
        ]);
        self.frame_with(vec![ev(true)]);
        self.frame_with(vec![ev(false)]);
        self.frame_with(vec![egui::Event::ModifiersChanged(egui::Modifiers::NONE)]);
    }

    pub(super) fn click(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.button(pos, egui::PointerButton::Primary, egui::Modifiers::NONE);
    }

    pub(super) fn shift_click(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.button(pos, egui::PointerButton::Primary, egui::Modifiers::SHIFT);
    }

    pub(super) fn right_click(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.button(pos, egui::PointerButton::Secondary, egui::Modifiers::NONE);
    }

    pub(super) fn double_click(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.frame_with(vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false, true, false] {
            self.frame_with(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            }]);
        }
        self.frame();
    }

    pub(super) fn selection(&self) -> Vec<AnyId> {
        self.d.app.selection()
    }
}

// ----- the drawing ----------------------------------------------------------------------

impl Lx {
    /// The terminal `+CAB-X1:5` (designation as the drawer shows it, without location when
    /// `want` has none) with the world position of its body (the circle on the symbol).
    fn terminal(&self, want: &str) -> (novo_core::TerminalId, f64, f64) {
        let project = self.project();
        let t = project
            .terminals()
            .find(|t| {
                project
                    .terminal_designation(t.id)
                    .is_some_and(|d| d.ends_with(want))
            })
            .unwrap_or_else(|| panic!("no terminal {want}"));
        let symbol = project
            .symbols()
            .find(|s| {
                matches!(&s.role, novo_core::SymbolRole::Terminal { terminals } if terminals.contains(&t.id))
            })
            .unwrap_or_else(|| panic!("terminal {want} is not drawn"));
        (
            t.id,
            symbol.placement.origin.x,
            symbol.placement.origin.y + 4.0,
        )
    }

    fn click_terminal(&mut self, want: &str) {
        let (_, x, y) = self.terminal(want);
        self.click(x, y);
    }

    fn painted_ctl(&self, text: &str) -> bool {
        self.label_first(text).is_some()
    }

    fn painted(&self, text: &str) -> bool {
        self.label(text).is_some()
    }

    fn strip_name_of(&self, want: &str) -> String {
        let (id, ..) = self.terminal(want);
        let project = self.project();
        let t = project.terminal(id).unwrap();
        project.device(t.strip).unwrap().tag.to_string()
    }
}

#[test]
fn terminal_strips_steps_work_in_the_window() {
    let mut l = Lx::start("terminal-strips");
    assert_eq!(
        l.project().terminals().count(),
        0,
        "no terminal at the start"
    );

    l.read("goal");

    // accept-one: the dashed terminal is highlighted; Enter takes it.
    l.open("accept-one");
    assert_eq!(
        l.d.app.page_proposals().len(),
        8,
        "eight wires leave the cabinet"
    );
    l.press(egui::Key::Enter);
    l.ticked("accept-one");
    assert_eq!(l.project().terminals().count(), 1);
    assert!(
        l.project()
            .terminal_designation(l.project().terminals().next().unwrap().id)
            .unwrap()
            .ends_with("-X1:1")
    );

    // next-proposal: Tab moves the highlight.
    l.open("next-proposal");
    assert_eq!(l.d.app.wb.proposal, 0);
    l.press(egui::Key::Tab);
    assert_eq!(
        l.d.app.wb.proposal, 1,
        "the highlight moved to the next proposal"
    );
    l.ticked("next-proposal");

    // accept-all: Ctrl+K, the words, Enter.
    l.open("accept-all");
    l.palette("accept all");
    l.ticked("accept-all");
    assert_eq!(l.project().terminals().count(), 8);
    for name in [
        "-X1:1", "-X1:2", "-X1:3", "-X1:PE", "-X1:4", "-X1:5", "-X1:6", "-X1:7",
    ] {
        l.terminal(name);
    }

    // open-drawer: Ctrl+3.
    l.open("open-drawer");
    l.key(egui::Key::Num3, egui::Modifiers::COMMAND);
    l.ticked("open-drawer");
    assert!(
        l.painted("Terminals") && l.painted("+CAB-X1"),
        "the drawer shows the strip"
    );

    // read-table: the columns the text names.
    for head in [
        "Terminal",
        "Kind",
        "Bridged to",
        "Internal",
        "External",
        "Cable",
    ] {
        assert!(l.painted(head), "column {head}");
    }
    assert!(l.painted("PE"), "the PE row shows kind PE");
    l.read("read-table");

    // select-two: the claim "5 and 7 carry 0 V" (the potential of their wires).
    for n in ["-X1:5", "-X1:7"] {
        let project = l.project();
        let (id, ..) = l.terminal(n);
        let symbol = project
            .symbols()
            .find(|s| matches!(&s.role, novo_core::SymbolRole::Terminal { terminals } if terminals.contains(&id)))
            .unwrap()
            .id;
        let netlist = project.netlist();
        let names: Vec<_> = project
            .wires()
            .filter(|w| {
                w.ends().iter().any(
                    |e| matches!(e, novo_core::Endpoint::Pin { symbol: s, .. } if *s == symbol),
                )
            })
            .filter_map(|w| novo_editor::potential::wire_potential(project, &netlist, w.id))
            .collect();
        assert!(
            !names.is_empty() && names.iter().all(|p| p == "0V"),
            "{n} carries 0V: {names:?}"
        );
    }
    // select-two: click terminal 5, Shift+click terminal 7 (their bodies).
    l.open("select-two");
    l.click_terminal("-X1:5");
    assert_eq!(l.selection().len(), 1);
    let (_, x, y) = l.terminal("-X1:7");
    l.shift_click(x, y);
    l.ticked("select-two");
    assert_eq!(
        l.project().terminals().count(),
        8,
        "nothing was wired by the clicks"
    );

    // connect: right-click, row Connect terminals.
    l.open("connect");
    l.right_click(x, y);
    l.click_label("Connect terminals");
    l.ticked("connect");
    {
        let project = l.project();
        let jumper = |n: &str| project.terminal(l.terminal(n).0).unwrap().jumper;
        assert!(jumper("-X1:5").is_some() && jumper("-X1:5") == jumper("-X1:7"));
        assert!(jumper("-X1:6").is_none());
    }
    assert!(l.painted("J1"), "the Jumper column names the group");
    let row5 = l.labels.iter().position(|(t, _)| t == "5").unwrap();
    assert_eq!(
        l.labels[row5 + 3].0,
        "7",
        "Bridged to lists the partner terminal"
    );

    // toggle-pe: click the PE terminal, Ctrl+K, "pe", Enter.
    l.open("toggle-pe");
    l.click_terminal("-X1:PE");
    {
        let (id, ..) = l.terminal("-X1:PE");
        assert_eq!(
            l.project().terminal(id).unwrap().kind,
            novo_core::TerminalKind::ProtectiveEarth
        );
    }
    // The Kind column of the drawer: cells right below the head "Kind".
    let kind_cells = |l: &Lx, text: &str| {
        let head = l.label("Kind").unwrap();
        l.labels
            .iter()
            .filter(|(t, r)| {
                t == text && r.min.y > head.max.y && (r.center().x - head.center().x).abs() < 40.0
            })
            .count()
    };
    assert_eq!(kind_cells(&l, "PE"), 1);
    l.palette("pe");
    l.ticked("toggle-pe");
    assert_eq!(kind_cells(&l, "PE"), 0, "the drawer shows another kind now");
    {
        let (id, ..) = l.terminal("-X1:PE");
        assert_ne!(
            l.project().terminal(id).unwrap().kind,
            novo_core::TerminalKind::ProtectiveEarth,
            "feed-through now"
        );
    }

    // reset-pe: right-click, row Reset PE override.
    l.open("reset-pe");
    let (_, px, py) = l.terminal("-X1:PE");
    l.right_click(px, py);
    l.click_label("Reset PE override");
    l.ticked("reset-pe");
    {
        let (id, ..) = l.terminal("-X1:PE");
        assert_eq!(
            l.project().terminal(id).unwrap().kind,
            novo_core::TerminalKind::ProtectiveEarth,
            "PE again"
        );
    }

    // move-strip: with only the PE terminal selected, Up.
    l.open("move-strip");
    l.click_terminal("-X1:PE");
    l.press(egui::Key::ArrowUp);
    l.ticked("move-strip");
    assert!(
        l.strip_name_of(":PE").ends_with("-X2"),
        "{}",
        l.strip_name_of(":PE")
    );
    assert_eq!(l.project().terminals().count(), 8);

    l.read("done");
    assert_eq!(l.d.app.learn.run.as_ref().unwrap().phase(), Phase::Finished);
}

// ----- connection-screens --------------------------------------------------------------

impl Lx {
    /// The texts of the sheet on show.
    fn sheet_texts(&mut self) -> Vec<(String, f32, f32)> {
        let view = self
            .d
            .app
            .canvas
            .schematic_ref()
            .expect("the editor canvas");
        let sheet = view.sheet().expect("a generated page is shown");
        sheet
            .texts
            .iter()
            .map(|t| (t.text.clone(), t.min.x, t.min.y))
            .collect()
    }

    /// Clicks the table cell `text` in the drawer column under the head `head`.
    fn click_cell(&mut self, head: &str, text: &str) {
        let h = self.label_first(head).expect("column head");
        let rect = self
            .labels
            .iter()
            .filter(|(t, r)| {
                t.trim() == text && r.min.y > h.max.y && (r.min.x - h.min.x).abs() < 30.0
            })
            .map(|(_, r)| *r)
            .next()
            .unwrap_or_else(|| panic!("no cell `{text}` under `{head}`"));
        self.button(
            rect.center(),
            egui::PointerButton::Primary,
            egui::Modifiers::NONE,
        );
    }
}

/// Types text into the focused box, as the keyboard does.
impl Lx {
    fn type_text(&mut self, text: &str) {
        self.frame_with(vec![egui::Event::Text(text.to_owned())]);
        self.frame();
    }
}

#[test]
fn connection_screens_steps_work_in_the_window() {
    let mut l = Lx::start("connection-screens");
    l.read("goal");

    // open-diagram: the Pages button, then the row.
    l.open("open-diagram");
    l.click_control("Pages");
    l.click_label("40  Terminal diagram · +CAB-X1");
    l.ticked("open-diagram");

    // read-diagram: the heads, the cabinet side, the block and the jumper bar, the cable column.
    let texts = l.sheet_texts();
    for head in ["Internal", "Wire", "Terminal", "External", "Cable", "Page"] {
        assert!(
            texts.iter().any(|(t, _, y)| t == head && *y < 30.0),
            "head {head}"
        );
    }
    let inside = |x0: f32, x1: f32| {
        texts
            .iter()
            .filter(|(_, x, y)| *x >= x0 && *x < x1 && *y >= 26.0 && *y < 86.0)
            .count()
    };
    assert!(
        inside(14.0, 128.0) >= 16,
        "cabinet side: device pin and wire number"
    );
    assert!(inside(156.0, 178.0) >= 8, "the blocks");
    {
        let view = l.d.app.canvas.schematic_ref().unwrap();
        let sheet = view.sheet().unwrap();
        let bars = sheet
            .lines
            .iter()
            .filter(|ln| ln.from.x < 162.0 && ln.from.x > 150.0 && ln.from.y > 60.0)
            .count();
        assert!(bars >= 1, "a jumper bar left of the blocks of 5 and 7");
    }
    assert!(
        !texts
            .iter()
            .any(|(t, x, _)| *x > 319.0 && t.starts_with("-W2")),
        "no valve cable yet"
    );
    l.read("read-diagram");

    // open-cables: Ctrl+4.
    l.open("open-cables");
    l.key(egui::Key::Num4, egui::Modifiers::COMMAND);
    l.ticked("open-cables");
    assert!(
        l.painted_ctl("Accept 2 proposed"),
        "the button is in the drawer"
    );

    // accept-cables: click the button.
    l.open("accept-cables");
    l.click_control("Accept 2 proposed");
    l.ticked("accept-cables");
    assert_eq!(l.project().cables().count(), 3);

    // see-follow: the diagram names the new cables in the Cable column.
    let texts = l.sheet_texts();
    let cable_col = |name: &str| {
        texts
            .iter()
            .filter(|(t, x, _)| *x > 319.0 && *x < 370.0 && t.starts_with(name))
            .count()
    };
    assert_eq!(cable_col("-W2"), 2, "{texts:?}");
    assert_eq!(cable_col("-W3"), 2);
    l.read("see-follow");

    // open-terminals: Ctrl+3 while the Cables tab is open.
    l.open("open-terminals");
    l.key(egui::Key::Num3, egui::Modifiers::COMMAND);
    l.ticked("open-terminals");

    // show-terminal: click the name 4 in the first column of the table.
    l.open("show-terminal");
    l.click_cell("Terminal", "4");
    l.ticked("show-terminal");
    assert_eq!(
        l.d.app.selection().len(),
        1,
        "terminal 4 is selected on the drawing"
    );

    // rename: double click the number beside the terminal, replace it, Enter.
    l.open("rename");
    let (_, x, y) = l.terminal("-X1:4");
    l.double_click(x - 4.0, y);
    l.frames(2);
    assert!(
        l.d.app.canvas.schematic().unwrap().renaming(),
        "the rename box opened"
    );
    l.key(egui::Key::A, egui::Modifiers::COMMAND);
    l.type_text("11");
    l.press(egui::Key::Enter);
    l.ticked("rename");
    assert!(l.project().terminals().any(|t| t.name == "11"));
    assert!(!l.project().terminals().any(|t| t.name == "4"));

    // back-to-diagram: Alt+Left.
    l.open("back-to-diagram");
    l.key(egui::Key::ArrowLeft, egui::Modifiers::ALT);
    l.ticked("back-to-diagram");
    let texts = l.sheet_texts();
    assert!(
        texts
            .iter()
            .any(|(t, x, y)| t == "11" && (*x - 162.35).abs() < 1.0 && (*y - 61.0).abs() < 1.0),
        "row 11 sits where 4 was"
    );

    l.read("done");
    assert_eq!(l.d.app.learn.run.as_ref().unwrap().phase(), Phase::Finished);
}

// ----- interruption-points --------------------------------------------------------------

impl Lx {
    /// The interruption points as (name, page title, x, y, rotation).
    fn points(&self) -> Vec<(String, String, f64, f64, novo_core::Rotation)> {
        let project = self.project();
        project
            .symbols()
            .filter_map(|s| match &s.role {
                novo_core::SymbolRole::Interruption { name } => Some((
                    name.clone(),
                    project
                        .page(s.page)
                        .map(|p| p.title.clone())
                        .unwrap_or_default(),
                    s.placement.origin.x,
                    s.placement.origin.y,
                    s.placement.rotation,
                )),
                _ => None,
            })
            .collect()
    }

    /// Where a double click renames the name label of `symbol`: the first spot around its
    /// origin (on the 0.5 mm grid) whose pick hits that label, as the canvas picks.
    fn label_spot(&mut self, symbol: novo_core::SymbolId) -> Option<(f64, f64)> {
        let project = self.project().clone();
        let origin = project.symbol(symbol)?.placement.origin;
        let view = self.d.app.canvas.schematic()?;
        let scene = view.scene()?;
        let zoom = view.canvas.viewport.zoom;
        let tol = view.canvas.viewport.screen_to_world_len(5.0);
        let mut hits: Vec<(f64, f64)> = Vec::new();
        for dy in -60..=60 {
            for dx in -60..=60 {
                let w = novo_canvas::Vec2::new(
                    origin.x as f32 + dx as f32 * 0.5,
                    origin.y as f32 + dy as f32 * 0.5,
                );
                if let Some(hit) = scene.scene().pick(w, tol, zoom)
                    && let Some(l) = scene.label_hit(&project, &hit)
                    && matches!(
                        l.kind,
                        novo_canvas::model::LabelKind::TerminalName
                            | novo_canvas::model::LabelKind::InterruptionName
                    )
                    && l.owner == AnyId::Symbol(symbol)
                {
                    hits.push((f64::from(w.x), f64::from(w.y)));
                }
            }
        }
        // The middle of the picked area is the safest place to click.
        let n = hits.len() as f64;
        let mid = (
            hits.iter().map(|h| h.0).sum::<f64>() / n,
            hits.iter().map(|h| h.1).sum::<f64>() / n,
        );
        // The spot nearest the middle that is itself picked (a frame has an empty inside).
        hits.into_iter().min_by(|a, b| {
            let d = |p: &(f64, f64)| (p.0 - mid.0).powi(2) + (p.1 - mid.1).powi(2);
            d(a).total_cmp(&d(b))
        })
    }

    /// Where a click selects `symbol`: the middle of the spots around its origin that the canvas
    /// picks as that symbol.
    fn body_spot(&mut self, symbol: novo_core::SymbolId) -> Option<(f64, f64)> {
        let origin = self.project().symbol(symbol)?.placement.origin;
        let view = self.d.app.canvas.schematic()?;
        let scene = view.scene()?;
        let zoom = view.canvas.viewport.zoom;
        let tol = view.canvas.viewport.screen_to_world_len(5.0);
        let mut hits: Vec<(f64, f64)> = Vec::new();
        for dy in -60..=60 {
            for dx in -60..=60 {
                let w = novo_canvas::Vec2::new(
                    origin.x as f32 + dx as f32 * 0.5,
                    origin.y as f32 + dy as f32 * 0.5,
                );
                if let Some(hit) = scene.scene().pick(w, tol, zoom)
                    && scene.model_id(hit.id) == Some(AnyId::Symbol(symbol))
                    && hit.label.is_none()
                {
                    hits.push((f64::from(w.x), f64::from(w.y)));
                }
            }
        }
        let n = hits.len() as f64;
        let mid = (
            hits.iter().map(|h| h.0).sum::<f64>() / n,
            hits.iter().map(|h| h.1).sum::<f64>() / n,
        );
        // The spot nearest the middle that is itself picked (a frame has an empty inside).
        hits.into_iter().min_by(|a, b| {
            let d = |p: &(f64, f64)| (p.0 - mid.0).powi(2) + (p.1 - mid.1).powi(2);
            d(a).total_cmp(&d(b))
        })
    }

    fn page_title(&self) -> String {
        self.project()
            .page(self.page())
            .map(|p| p.title.clone())
            .unwrap_or_default()
    }

    /// Tap Q, type the words, Enter: the place field as a person uses it.
    fn place_field(&mut self, words: &str) {
        self.press(egui::Key::Q);
        self.type_text(words);
        self.press(egui::Key::Enter);
    }

    fn tool_is_place(&self) -> bool {
        matches!(
            self.d.app.canvas.schematic_ref().unwrap().editor.tool(),
            novo_editor::tool::Tool::Place(_)
        )
    }
}

#[test]
fn interruption_points_steps_work_in_the_window() {
    interruption_points_run(false);
}

/// The same walk, but `jump`, `renumber` and `back` run the lesson's own `solve` commands, so the
/// replay ends on the same pages as a person pressing the keys.
#[test]
fn interruption_points_solves_end_on_the_pages_the_text_names() {
    interruption_points_run(true);
}

impl Lx {
    /// Runs the `solve` commands of the step `id` through the command path.
    fn solve_commands(&mut self, id: &str) {
        let lesson = self.d.app.learn.run.as_ref().unwrap().lesson().clone();
        let step = lesson.steps.iter().find(|s| s.id == id).unwrap();
        assert!(!step.solve.is_empty());
        let ctx = self.d.ctx.clone();
        for op in &step.solve {
            let novo_learn::lesson::SolveOp::Command(c) = op else {
                panic!("step `{id}` solves with {op:?}");
            };
            self.d.app.run_command(&ctx, &c.command, c.arg.as_deref());
            self.frames(2);
        }
    }
}

fn interruption_points_run(by_solve: bool) {
    let mut l = Lx::start("interruption-points");
    let wires0 = l.project().wires().count();
    assert!(l.points().is_empty());
    l.read("goal");

    // first-point: tap Q, type ip, Enter, click under -S1:14.
    l.open("first-point");
    l.place_field("ip");
    l.click(40.0, 120.0);
    l.ticked("first-point");
    assert_eq!(l.points().len(), 1);
    assert_eq!(l.points()[0].0, "IP1");
    assert_eq!(
        l.project().wires().count(),
        wires0 + 1,
        "the wire to -S1:14 is drawn"
    );
    assert!(
        l.tool_is_place(),
        "the place tool stays armed after a click"
    );
    l.press(egui::Key::Escape);
    assert!(!l.tool_is_place(), "Esc ends placing");

    // next-page: PageDown.
    l.open("next-page");
    l.key(egui::Key::PageDown, egui::Modifiers::NONE);
    l.ticked("next-page");
    assert_eq!(l.page_title(), "Contactor coil");

    // pick-second: Q does not arm the tool, Enter after the words does.
    l.open("pick-second");
    l.press(egui::Key::Q);
    assert_eq!(l.phase(), Phase::Active, "Q alone does not tick the step");
    l.type_text("ip");
    l.press(egui::Key::Enter);
    l.ticked("pick-second");

    // second-point: R twice turns the piece in hand; the click drops it and draws the wire.
    l.open("second-point");
    let wires1 = l.project().wires().count();
    l.press(egui::Key::R);
    l.press(egui::Key::R);
    l.click(40.0, 40.0);
    l.press(egui::Key::Escape);
    assert!(!l.tool_is_place(), "Esc ends placing");
    l.ticked("second-point");
    {
        let pts = l.points();
        assert_eq!(pts.len(), 2);
        let second = pts.iter().find(|p| p.1 == "Contactor coil").unwrap();
        assert_eq!(second.0, "IP2");
        assert_eq!(second.4, novo_core::Rotation::R180, "turned half a turn");
    }
    assert_eq!(
        l.project().wires().count(),
        wires1 + 1,
        "the wire to -K1:A1 is drawn"
    );

    // same-name: double click the name, Ctrl+A, type, Enter.
    l.open("same-name");
    let (px, py) = {
        let p = l
            .points()
            .into_iter()
            .find(|p| p.1 == "Contactor coil")
            .unwrap();
        (p.2, p.3)
    };
    let symbol = l
        .project()
        .symbols()
        .find(|s| s.placement.origin.x == px && s.placement.origin.y == py)
        .unwrap()
        .id;
    let (lx, ly) = l.label_spot(symbol).expect("the name label can be picked");
    l.double_click(lx, ly);
    l.frames(2);
    assert!(
        l.d.app.canvas.schematic().unwrap().renaming(),
        "the rename box opened"
    );
    l.key(egui::Key::A, egui::Modifiers::COMMAND);
    l.type_text("IP1");
    l.press(egui::Key::Enter);
    l.ticked("same-name");
    assert!(l.points().iter().all(|p| p.0 == "IP1"));

    // jump: G with the point selected.
    l.open("jump");
    if by_solve {
        l.solve_commands("jump");
    } else {
        l.press(egui::Key::G);
    }
    l.ticked("jump");
    assert_eq!(l.page_title(), "Start signal");

    // renumber: the command field.
    l.open("renumber");
    if by_solve {
        l.solve_commands("renumber");
    } else {
        l.palette("number interruption");
    }
    l.ticked("renumber");
    assert!(l.points().iter().all(|p| p.0 == "1"), "{:?}", l.points());

    // back: G again.
    l.open("back");
    if by_solve {
        l.solve_commands("back");
    } else {
        l.press(egui::Key::G);
    }
    l.ticked("back");
    assert_eq!(l.page_title(), "Contactor coil");

    // contact-image: click the coil, read the label, Shift+G.
    l.open("contact-image");
    let coil_symbol = {
        let p = l.project();
        let d = p
            .devices()
            .find(|d| d.tag.to_string().ends_with("-K1"))
            .unwrap()
            .id;
        p.symbols_of_device(d)
            .into_iter()
            .find(|s| s.page == l.page())
            .unwrap()
            .id
    };
    l.frames(30); // the view is still gliding to the point it jumped to
    let coil = l.body_spot(coil_symbol).expect("the coil can be picked");
    l.click(coil.0, coil.1);
    let labels = l.d.app.xref_labels(l.page());
    let lines: Vec<String> = labels.iter().flat_map(|x| x.lines.clone()).collect();
    assert!(lines.iter().any(|t| t == "13-14 3.1"), "{lines:?}");
    assert!(lines.iter().any(|t| t == "21-22 3.2"), "{lines:?}");

    l.key(egui::Key::G, egui::Modifiers::SHIFT);
    assert!(
        l.d.app
            .wb
            .occurrences
            .as_ref()
            .is_some_and(|o| o.len() >= 3),
        "the list names the coil and its two contacts"
    );
    l.ticked("contact-image");

    l.read("done");
    assert_eq!(l.d.app.learn.run.as_ref().unwrap().phase(), Phase::Finished);
}

#[test]
fn the_three_lessons_lint_without_findings_that_matter() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let report = d.app.learn.lint_report();
    let mine: Vec<String> = report
        .findings
        .iter()
        .filter(|f| {
            [
                "terminal-strips",
                "connection-screens",
                "interruption-points",
            ]
            .contains(&f.lesson.as_str())
        })
        .map(ToString::to_string)
        .collect();
    assert!(
        mine.iter().all(|m| m.starts_with("warning")),
        "errors: {mine:?}"
    );
}
