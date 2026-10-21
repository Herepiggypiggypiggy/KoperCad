//! Headless runs of the steps of the lessons cables-and-shields, wire-numbers, plc-io: each instruction a
//! step gives is carried out through the app (keys, palette text, clicks on controls by their visible label,
//! clicks at world positions on the drawing) from the lesson's own start project, and the lesson's own
//! checker (the running lesson in the app) must tick the step. A second test per lesson replays the
//! `solve` scripts and must tick the same steps.
//!
//! Not drivable headless, verified by their model effect instead: the Learn card itself (not drawn here),
//! hover texts, and right-click menus (not used by these lessons).
use super::gate1_tests::Drive;
use eframe::egui;

pub(super) struct Lv {
    pub d: Drive,
    _dir: tempfile::TempDir,
}
impl Lv {
    pub fn start(id: &str) -> Lv {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        d.app.learn.reload();
        d.app.learn.use_progress_file(dir.path().join("learn.json"));
        d.app.learn.advance_after = std::time::Duration::ZERO;
        d.app.start_lesson(id);
        let mut lv = Lv { d, _dir: dir };
        lv.frames(3);
        lv
    }
    pub fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.d.frame();
        }
    }
    pub fn step(&self) -> String {
        self.d
            .app
            .learn
            .run
            .as_ref()
            .and_then(|r| r.step())
            .map(|s| s.id.clone())
            .unwrap_or_else(|| "<end>".into())
    }
    pub fn key(&mut self, key: egui::Key, modifiers: egui::Modifiers) {
        let canvas = self.d.app.wb.canvas_area.unwrap().1;
        self.d
            .frame_with(vec![egui::Event::PointerMoved(canvas.center())]);
        for pressed in [true, false] {
            self.d.frame_with(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed,
                repeat: false,
                modifiers,
            }]);
        }
        self.frames(2);
    }
    /// Every labelled widget of the last frame with its rectangle, from the accessibility tree.
    pub fn widgets(&mut self) -> Vec<(String, egui::Rect)> {
        self.d.ctx.enable_accesskit();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            ..Default::default()
        };
        let app = &mut self.d.app;
        let out = self.d.ctx.run_ui(input, |ui| app.show(ui));
        let mut found = Vec::new();
        if let Some(update) = out.platform_output.accesskit_update {
            for (_, node) in update.nodes.iter() {
                if let (Some(label), Some(b)) = (node.label(), node.bounds()) {
                    found.push((
                        label.to_owned(),
                        egui::Rect::from_min_max(
                            egui::pos2(b.x0 as f32, b.y0 as f32),
                            egui::pos2(b.x1 as f32, b.y1 as f32),
                        ),
                    ));
                }
            }
        }
        found
    }
    /// Clicks the control with this visible label, as a mouse would.
    pub fn click_label(&mut self, label: &str) {
        self.frames(1);
        let rect = self
            .widgets()
            .into_iter()
            .find(|(l, _)| l == label)
            .unwrap_or_else(|| panic!("no control labelled `{label}`"))
            .1;
        self.click_screen(rect.center());
    }
    /// Whether the control with this visible label is drawn disabled.
    pub fn is_disabled(&mut self, label: &str) -> bool {
        self.frames(1);
        self.d.ctx.enable_accesskit();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            ..Default::default()
        };
        let app = &mut self.d.app;
        let out = self.d.ctx.run_ui(input, |ui| app.show(ui));
        let update = out.platform_output.accesskit_update.expect("accesskit");
        let (_, node) = update
            .nodes
            .iter()
            .find(|(_, n)| n.label() == Some(label))
            .unwrap_or_else(|| panic!("no control labelled `{label}`"));
        node.is_disabled()
    }
    pub fn has_label(&mut self, label: &str) -> bool {
        self.widgets().iter().any(|(l, _)| l == label)
    }
    pub fn click_screen(&mut self, pos: egui::Pos2) {
        let m = egui::Modifiers::NONE;
        self.d.frame_with(vec![egui::Event::PointerMoved(pos)]);
        for pressed in [true, false] {
            self.d.frame_with(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: m,
            }]);
        }
        self.frames(3);
    }
    /// Ctrl+K, the words, Enter: the command field as a person uses it.
    pub fn palette(&mut self, text: &str) {
        self.key(egui::Key::K, egui::Modifiers::CTRL);
        assert!(
            self.d.app.wb.field.is_open(),
            "Ctrl+K opens the command field"
        );
        self.d.frame_with(vec![egui::Event::Text(text.to_owned())]);
        self.frames(1);
        self.key(egui::Key::Enter, egui::Modifiers::NONE);
        assert!(
            !self.d.app.wb.field.is_open(),
            "Enter in the field runs `{text}` (field still open; toast: {})",
            self.last()
        );
    }
    /// Step `id` is the open step and has not ticked yet: the check is false before the action.
    pub fn not_yet(&mut self, id: &str) {
        self.frames(2);
        assert_eq!(self.step(), id, "the lesson is at another step");
        let run = self.d.app.learn.run.as_ref().unwrap();
        assert!(
            !run.is_done(id),
            "step `{id}` already ticked before anything was done"
        );
    }
    /// After the action the lesson's own check has ticked step `id`.
    pub fn ticked(&mut self, id: &str) {
        self.frames(4);
        let run = self.d.app.learn.run.as_ref().unwrap();
        assert!(
            run.is_done(id),
            "step `{id}` did not tick (open step `{}`, last toast: {})",
            self.step(),
            self.last()
        );
    }
    /// A reading step: Next opens the following one.
    pub fn read(&mut self, id: &str) {
        self.frames(2);
        assert_eq!(self.step(), id, "the lesson is at another step");
        self.d.app.learn_move(super::learn::Move::Next);
        self.frames(1);
    }
    /// A step with a check and `pause`: ticked by the action, then Next.
    pub fn ticked_then_next(&mut self, id: &str) {
        self.ticked(id);
        self.d.app.learn_move(super::learn::Move::Next);
        self.frames(1);
    }
    pub fn finished(&self) -> bool {
        self.d.app.learn.run.as_ref().unwrap().phase() == novo_learn::Phase::Finished
    }
    pub fn project(&self) -> &novo_core::Project {
        self.d.doc().project()
    }
    /// The page named `name` (name or title).
    pub fn page_id(&self, name: &str) -> novo_core::PageId {
        self.project()
            .pages()
            .find(|p| p.name == name || p.title == name)
            .unwrap_or_else(|| panic!("no page {name}"))
            .id
    }
    pub fn symbol_of(&self, tag: &str) -> novo_core::SymbolId {
        let p = self.project();
        p.symbols()
            .find(|s| {
                novo_editor::edit::device_of(p, novo_core::AnyId::Symbol(s.id))
                    .and_then(|d| p.device(d))
                    .is_some_and(|d| novo_learn::check::tag_matches(&d.tag, tag))
            })
            .unwrap_or_else(|| panic!("no symbol {tag}"))
            .id
    }
    /// The wire that joins the two pins named `tag:pin`.
    pub fn wire_between(&self, a: &str, b: &str) -> novo_core::WireId {
        let p = self.project();
        let pa = novo_learn::check::pins_named(p, a);
        let pb = novo_learn::check::pins_named(p, b);
        p.wires()
            .find(|w| {
                let (f, t) = (w.from.as_pin(), w.to.as_pin());
                pa.iter().any(|x| Some(*x) == f) && pb.iter().any(|y| Some(*y) == t)
                    || pb.iter().any(|x| Some(*x) == f) && pa.iter().any(|y| Some(*y) == t)
            })
            .unwrap_or_else(|| panic!("no wire {a} - {b}"))
            .id
    }
    /// A click on the drawing at a world position, as the select tool takes it.
    pub fn click_world(&mut self, x: f64, y: f64) {
        let pos = self.screen(x, y);
        self.click_screen(pos);
    }
    /// Where a world point (mm) is on screen, bringing it into view first.
    pub fn screen(&mut self, x: f64, y: f64) -> egui::Pos2 {
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
            self.d.frame();
        }
        panic!("({x}, {y}) cannot be brought on screen");
    }
    /// A point on the drawing (mm) where the pointer is over the symbol of `tag`: the hovered spot nearest the middle of all of them.
    pub fn on_symbol(&mut self, tag: &str) -> (f64, f64) {
        let sym = self.symbol_of(tag);
        let item = novo_canvas::model::item_id(novo_core::AnyId::Symbol(sym));
        let o = self.project().symbol(sym).unwrap().placement.origin;
        let mut hits = Vec::new();
        for dx in (-4..=24).map(f64::from) {
            for dy in (-4..=24).map(f64::from) {
                let pos = self.screen(o.x + dx, o.y + dy);
                self.d.frame_with(vec![egui::Event::PointerMoved(pos)]);
                if self.d.app.canvas.schematic().unwrap().canvas.hovered() == Some(item) {
                    hits.push((o.x + dx, o.y + dy));
                }
            }
        }
        assert!(!hits.is_empty(), "the pointer never lands on {tag}");
        let n = hits.len() as f64;
        let c = (
            hits.iter().map(|h| h.0).sum::<f64>() / n,
            hits.iter().map(|h| h.1).sum::<f64>() / n,
        );
        *hits
            .iter()
            .min_by(|a, b| {
                ((a.0 - c.0).powi(2) + (a.1 - c.1).powi(2))
                    .total_cmp(&((b.0 - c.0).powi(2) + (b.1 - c.1).powi(2)))
            })
            .unwrap()
    }
    /// A real click on the symbol of `tag`.
    pub fn click_symbol(&mut self, tag: &str) {
        let (x, y) = self.on_symbol(tag);
        self.click_world(x, y);
    }
    /// Selects a device's symbol, as a click on it does.
    pub fn select(&mut self, tag: &str) {
        let id = self.symbol_of(tag);
        let view = self.d.app.canvas.schematic().expect("editor");
        view.canvas.selection.clear();
        view.canvas.selection.apply(
            [novo_canvas::model::item_id(novo_core::AnyId::Symbol(id))],
            novo_canvas::SelectOp::Replace,
        );
        self.frames(2);
    }
    /// Runs a lesson `solve` script through the real command path.
    pub fn solve(&mut self, ops: &[novo_learn::lesson::SolveOp]) {
        use novo_learn::lesson::SolveOp;
        let ctx = self.d.ctx.clone();
        for op in ops {
            match op {
                SolveOp::Command(c) => self.d.app.run_command(&ctx, &c.command, c.arg.as_deref()),
                SolveOp::Page(pg) => {
                    let id = self.page_id(&pg.page);
                    self.d
                        .app
                        .run_target(&ctx, novo_workflow::palette::Target::Page(id));
                }
                SolveOp::Select(sel) => {
                    assert!(!sel.add);
                    self.select(&sel.select);
                }
                SolveOp::Cable(c) => {
                    let wires: Vec<_> = c
                        .cable
                        .wires
                        .iter()
                        .map(|[a, b]| self.wire_between(a, b))
                        .collect();
                    self.d.app.make_cable(&wires);
                }
                other => {
                    if let Err(why) = self.d.replay_op(other) {
                        panic!("this verifier cannot run {other:?}: {why}");
                    }
                }
            }
            self.frames(2);
        }
    }
    pub fn last(&self) -> String {
        self.d
            .app
            .messages()
            .last()
            .map(|m| m.text.clone())
            .unwrap_or_default()
    }
}

fn lesson(lv: &Lv) -> std::sync::Arc<novo_learn::Lesson> {
    std::sync::Arc::new(lv.d.app.learn.run.as_ref().unwrap().lesson().clone())
}

/// Replays every step's `solve` script from the start project: each step must be false before and true after.
fn replay_solves(id: &str) {
    let mut lv = Lv::start(id);
    let lesson = lesson(&lv);
    for step in &lesson.steps {
        lv.frames(2);
        assert_eq!(lv.step(), step.id);
        if step.expect.is_some() {
            assert!(
                !step.solve.is_empty(),
                "step `{}` has an expect and no solve",
                step.id
            );
            lv.not_yet(&step.id);
            lv.solve(&step.solve);
            lv.ticked(&step.id);
            if step.pause {
                lv.d.app.learn_move(super::learn::Move::Next);
            }
        } else {
            lv.d.app.learn_move(super::learn::Move::Next);
        }
    }
    assert!(lv.finished(), "the lesson ran to its end");
}

#[test]
fn cables_and_shields_solve_scripts_tick_every_step() {
    replay_solves("cables-and-shields");
}

#[test]
fn cables_and_shields_each_instruction_works_in_the_app() {
    let mut lv = Lv::start("cables-and-shields");
    // intro
    lv.read("intro");

    // open-drawer: Ctrl+4
    lv.not_yet("open-drawer");
    lv.key(egui::Key::Num4, egui::Modifiers::CTRL);
    assert_eq!(lv.d.app.wb.drawer, Some(super::workbench::Drawer::Cables));
    lv.ticked("open-drawer");

    // read-proposals: the tab, the button and the count the text names
    assert_eq!(novo_cables::propose_cables(lv.project()).len(), 2);
    assert!(lv.has_label("Cables") && lv.has_label("Accept 2 proposed"));
    let proposals = novo_cables::propose_cables(lv.project());
    let mut sizes: Vec<usize> = proposals.iter().map(|p| p.wires.len()).collect();
    sizes.sort_unstable();
    assert_eq!(
        sizes,
        [2, 4],
        "one for the level transmitter (2 wires), one for the motor (4 wires)"
    );
    lv.read("read-proposals");

    // cable-tool: C
    lv.not_yet("cable-tool");
    lv.key(egui::Key::C, egui::Modifiers::NONE);
    lv.ticked("cable-tool");

    // draw-line: click left of the first transmitter wire, right of the second, Enter
    lv.not_yet("draw-line");
    lv.click_world(80.0, 116.0);
    lv.click_world(104.0, 116.0);
    lv.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert!(
        lv.last().starts_with("Cable =P1-W1: 3x1.5 on 2 wires"),
        "{}",
        lv.last()
    );
    lv.ticked("draw-line");
    let w1 = lv
        .project()
        .cables()
        .find(|c| c.tag.to_string().ends_with("-W1"))
        .unwrap();
    assert_eq!(w1.cores.len(), 3);
    assert!(lv.has_label("Accept 1 proposed"), "one proposal is left");

    // accept: the drawer button
    lv.not_yet("accept");
    lv.click_label("Accept 1 proposed");
    assert!(
        lv.last().starts_with("Accepted 1 cable: =P1-W2 4G1.5"),
        "{}",
        lv.last()
    );
    lv.ticked("accept");
    assert_eq!(lv.project().cables().count(), 2);

    // read-cores: core names and potentials the text names
    let w2 = lv
        .project()
        .cables()
        .find(|c| c.tag.to_string().ends_with("-W2"))
        .unwrap()
        .id;
    let screen = novo_cables::cable_screen(lv.project(), w2).unwrap();
    let rows: Vec<(String, Option<String>)> = screen
        .rows
        .iter()
        .map(|r| (r.core.clone(), r.potential.clone()))
        .collect();
    assert_eq!(
        rows,
        [
            ("BN".into(), Some("L1".into())),
            ("BK".into(), Some("L2".into())),
            ("GY".into(), Some("L3".into())),
            ("GNYE".into(), Some("PE".into()))
        ]
    );
    let ends: Vec<String> = screen
        .rows
        .iter()
        .map(|r| r.ends[1].as_ref().unwrap().target.clone())
        .collect();
    assert_eq!(
        ends,
        [
            "=P1+FLD-M1:U1",
            "=P1+FLD-M1:V1",
            "=P1+FLD-M1:W1",
            "=P1+FLD-M1:PE"
        ]
    );
    lv.read("read-cores");

    // free-core: click -W1 in the list; its Cores column says 2 of 3
    assert_eq!(lv.step(), "free-core");
    lv.click_label("=P1-W1");
    assert_eq!(
        lv.d.app.wb.drawer_cable,
        Some(
            lv.project()
                .cables()
                .find(|c| c.tag.to_string().ends_with("-W1"))
                .unwrap()
                .id
        )
    );
    // (a plain cell label is not in the accessibility tree; the cell shows `{used} of {cores}` of this row)
    let row = novo_cables::overview(lv.project())
        .into_iter()
        .find(|r| r.tag.ends_with("-W1"))
        .unwrap();
    assert_eq!(
        (row.used, row.cores),
        (2, 3),
        "the Cores column reads 2 of 3"
    );
    lv.read("free-core");

    // part-length-shield (reading): the claims it makes about the program
    assert!(
        lv.project()
            .cables()
            .all(|c| c.part.is_none() && c.length_m.is_none()),
        "nothing sets a part or length from the screen yet"
    );
    lv.read("part-length-shield");
    assert_eq!(lv.step(), "done");
    lv.read("done");
    assert!(lv.finished());
}

fn report(lv: &mut Lv, id: &str) -> novo_reports::ReportOutput {
    let doc = lv.d.app.backend.document().unwrap();
    let project = doc.project();
    lv.d.app
        .wb
        .reports
        .live(project)
        .get(project, id)
        .unwrap()
        .clone()
}
#[test]
fn wire_numbers_solve_scripts_tick_every_step() {
    replay_solves("wire-numbers");
}

/// The wire numbers by the two pins of each wire.
fn numbers_by_wire(
    project: &novo_core::Project,
) -> std::collections::BTreeMap<String, Option<String>> {
    project
        .wires()
        .map(|w| {
            let n = |e: novo_core::Endpoint| {
                e.as_pin()
                    .and_then(|pin| project.pin_designation(pin))
                    .unwrap_or_default()
            };
            (format!("{} > {}", n(w.from), n(w.to)), w.number.clone())
        })
        .collect()
}

#[test]
fn the_number_button_is_disabled_when_there_is_nothing_to_change() {
    let mut lv = Lv::start("wire-numbers");
    lv.palette("wire numbering");
    assert!(lv.d.app.wb.numbering.is_some());
    assert!(!lv.is_disabled("Number"), "seven nets still have no number");
    assert_eq!(
        super::numbering::number_blocked(&novo_numbering::plan(
            lv.project(),
            &novo_numbering::Scheme::load(lv.project()),
            novo_numbering::Mode::Fill,
            &novo_numbering::Scope::Project,
        )),
        None
    );
    lv.click_label("Close window");
    lv.palette("number wires");
    lv.palette("wire numbering");
    assert!(lv.is_disabled("Number"), "every net is numbered");
    assert_eq!(
        super::numbering::number_blocked(&novo_numbering::plan(
            lv.project(),
            &novo_numbering::Scheme::load(lv.project()),
            novo_numbering::Mode::Fill,
            &novo_numbering::Scope::Project,
        )),
        Some("Nothing to change"),
        "the button says why"
    );
    let depth = lv.d.doc().undo_depth();
    lv.click_label("Number");
    assert_eq!(
        lv.d.doc().undo_depth(),
        depth,
        "a disabled button does nothing"
    );
}

#[test]
fn wire_numbers_each_instruction_works_in_the_app() {
    let mut lv = Lv::start("wire-numbers");
    let column = |out: &novo_reports::ReportOutput, head: &str| -> Vec<String> {
        let s = &out.sections[0];
        let c = s
            .column("wire_no")
            .or_else(|| s.columns.iter().position(|c| c.header == head))
            .unwrap();
        s.text_rows().into_iter().map(|r| r[c].clone()).collect()
    };
    lv.read("intro");

    // open-wire-list: Ctrl+K, "wire list", Enter
    lv.not_yet("open-wire-list");
    lv.palette("wire list");
    assert_eq!(lv.d.app.workspace(), super::workspace::Workspace::Reports);
    lv.ticked("open-wire-list");

    // read-list: the Wire no. column holds 1, 2 and eight empty cells
    let list = report(&mut lv, "wires");
    let mut have = column(&list, "Wire no.");
    have.sort();
    assert_eq!(have.iter().filter(|c| c.is_empty()).count(), 8);
    assert_eq!(have.iter().filter(|c| *c == "2").count(), 1);
    assert_eq!(have.iter().filter(|c| *c == "1").count(), 1);
    let heads: Vec<&str> = list.sections[0]
        .columns
        .iter()
        .map(|c| c.header.as_str())
        .collect();
    assert!(
        heads.contains(&"Potential") && heads.contains(&"Cable") && heads.contains(&"Wire no.")
    );
    lv.read("read-list");

    // back-to-drawing: Ctrl+1
    lv.not_yet("back-to-drawing");
    lv.key(egui::Key::Num1, egui::Modifiers::CTRL);
    assert_eq!(lv.d.app.workspace(), super::workspace::Workspace::Schematic);
    lv.ticked("back-to-drawing");

    // preview: the window and its plan
    lv.not_yet("preview");
    lv.palette("wire numbering");
    assert!(lv.d.app.wb.numbering.is_some());
    for label in ["Preview", "Number", "Close window"] {
        assert!(lv.has_label(label), "the window shows `{label}`");
    }
    // The scheme and mode block is collapsed on the Numbers tab: open it to read the scheme.
    assert!(!lv.has_label("Renumber all"));
    lv.click_label("Scheme and mode: sequential, number new wires, whole project");
    for label in ["Number new wires", "Renumber all"] {
        assert!(lv.has_label(label), "the window shows `{label}`");
    }
    let scheme = novo_numbering::Scheme::load(lv.project());
    assert_eq!(scheme.kind, novo_numbering::SchemeKind::Sequential);
    let plan = novo_numbering::plan(
        lv.project(),
        &scheme,
        novo_numbering::Mode::Fill,
        &novo_numbering::Scope::Project,
    );
    assert_eq!(plan.summary(), "7 numbered, 2 settled, 0 unchanged");
    assert_eq!(
        plan.changed_groups()
            .filter(|g| !g.old.is_empty() && g.new.as_deref() != g.old.first().map(String::as_str))
            .count(),
        0,
        "no number is changed, only empty nets are filled"
    );
    let before = numbers_by_wire(lv.project());
    lv.ticked_then_next("preview");
    lv.click_label("Close window");
    assert!(lv.d.app.wb.numbering.is_none());
    assert_eq!(
        numbers_by_wire(lv.project()),
        before,
        "the preview changed nothing"
    );

    // number-wires
    lv.not_yet("number-wires");
    let depth = lv.d.doc().undo_depth();
    lv.palette("number wires");
    assert_eq!(
        lv.last(),
        "Wire numbers: 7 numbered, 2 settled, 0 unchanged"
    );
    assert_eq!(lv.d.doc().undo_depth(), depth + 1, "one undo step");
    lv.ticked("number-wires");
    assert!(
        lv.project().wires().all(|w| w.number.is_some()),
        "every wire has a number"
    );
    let kept = numbers_by_wire(lv.project());
    assert_eq!(kept["=P1+CAB-X1:1 > =P1+FLD-M1:U1"].as_deref(), Some("1"));
    assert_eq!(kept["=P1+CAB-X1:2 > =P1+FLD-M1:V1"].as_deref(), Some("2"));
    assert!(
        !novo_numbering::check(lv.project(), &scheme)
            .iter()
            .any(|c| c.kind == novo_numbering::ConflictKind::Duplicate)
    );

    // type-number: click the wire from terminal 4 to the transmitter, number wire 100
    lv.not_yet("type-number");
    // The click coordinates were measured on the whole sheet.
    lv.d.app.canvas.zoom_to_sheet();
    lv.d.frame();
    lv.click_world(88.0, 116.0);
    let w = lv.wire_between("-X1:4", "-B1:+");
    assert_eq!(
        lv.d.app.selection(),
        vec![novo_core::AnyId::Wire(w)],
        "the click selects that wire"
    );
    // A number another net already uses is refused and nothing changes.
    let before_typed = numbers_by_wire(lv.project());
    lv.palette("number wire 1");
    assert_eq!(
        lv.last(),
        "Wire number: Wire number: 1 is already used on another net"
    );
    assert_eq!(numbers_by_wire(lv.project()), before_typed);
    lv.palette("number wire 100");
    assert_eq!(
        lv.last(),
        "Wire number 100 is fixed: numbering runs leave it alone"
    );
    lv.ticked("type-number");
    assert!(novo_numbering::is_locked(lv.project(), w));
    assert_eq!(lv.project().wire(w).unwrap().number.as_deref(), Some("100"));

    // renumber: window opens on Selection; Project, then Renumber
    lv.not_yet("renumber");
    lv.palette("renumber wires");
    assert!(lv.d.app.wb.numbering.is_some());
    lv.ticked("renumber");
    for label in ["Project", "Renumber"] {
        assert!(lv.has_label(label), "the window shows `{label}`");
    }
    lv.click_label("Project");
    lv.click_label("Renumber");
    assert_eq!(lv.last(), "Wire numbers: 8 renumbered, 1 unchanged");
    lv.d.app.learn_move(super::learn::Move::Next);
    lv.frames(1);
    assert!(lv.d.app.wb.numbering.is_none() || lv.has_label("Close window"));
    if lv.d.app.wb.numbering.is_some() {
        lv.click_label("Close window");
    }

    // the finished project is exactly this result
    let finished = novo_core::Project::from_json(include_str!(
        "../../../../assets/lessons/wire-numbers/finished.json"
    ))
    .unwrap();
    assert_eq!(
        numbers_by_wire(lv.project()),
        numbers_by_wire(&finished),
        "finished.json holds what the steps leave"
    );

    // wire-list-again
    lv.not_yet("wire-list-again");
    lv.palette("wire list");
    lv.ticked("wire-list-again");
    let list = report(&mut lv, "wires");
    let have = column(&list, "Wire no.");
    assert!(have.iter().all(|c| !c.is_empty()), "{have:?}");
    assert!(have.iter().any(|c| c == "100"));
    let heads: Vec<_> = have.clone();
    assert_eq!(heads.len(), 10);

    // cable-diagram
    lv.not_yet("cable-diagram");
    lv.palette("cable diagram");
    lv.ticked("cable-diagram");
    let cables = report(&mut lv, "cables");
    let w2 = cables
        .sections
        .iter()
        .find(|s| s.title.contains("-W2") || s.key.contains("-W2"))
        .expect("a section for -W2");
    let c = w2
        .columns
        .iter()
        .position(|c| c.header == "Wire no.")
        .unwrap();
    let nums: Vec<String> = w2.text_rows().iter().map(|r| r[c].clone()).collect();
    assert!(nums.iter().all(|n| !n.is_empty()), "{nums:?}");
    lv.read("done");
    assert!(lv.finished());
}

#[test]
fn plc_io_solve_scripts_tick_every_step() {
    // The solve scripts run the commands, the Enter on the preview and the live clicks (key, live_pole and
    // live_coil ops), so every step ticks from its own solve.
    let mut lv = Lv::start("plc-io");
    let lesson = lesson(&lv);
    for step in &lesson.steps {
        lv.frames(2);
        assert_eq!(lv.step(), step.id);
        if step.expect.is_none() {
            lv.d.app.learn_move(super::learn::Move::Next);
            continue;
        }
        lv.not_yet(&step.id);
        lv.solve(&step.solve);
        lv.ticked(&step.id);
        if step.pause {
            lv.d.app.learn_move(super::learn::Move::Next);
        }
    }
    assert!(lv.finished());
}

impl Lv {
    /// The round pole badge of -S1 in the live view, clicked where it is painted.
    fn click_badge_pole(&mut self) {
        let page = self.d.app.current_page().unwrap();
        let badges =
            super::live::pole_badges(self.project(), page, &novo_sim::PoleState::all_open());
        let sym = self.symbol_of("-S1");
        let b = badges
            .iter()
            .find(|b| b.symbol == sym)
            .expect("a pole badge on -S1");
        assert_eq!((b.at.x, b.at.y), (48.0, 44.0));
        self.click_world(f64::from(b.at.x), f64::from(b.at.y));
    }
    /// The square output badge of the DQ box, clicked where it is painted.
    fn click_badge_output(&mut self) {
        let page = self.d.app.current_page().unwrap();
        let badges = super::live::coil_badges(
            self.project(),
            page,
            &Default::default(),
            &Default::default(),
        );
        let b = badges.iter().find(|b| b.output).expect("an output badge");
        assert_eq!((b.at.x, b.at.y), (170.0, 51.0));
        self.click_world(f64::from(b.at.x), f64::from(b.at.y));
    }
}

#[test]
fn plc_io_each_instruction_works_in_the_app() {
    let mut lv = Lv::start("plc-io");
    let page_symbols = |lv: &Lv| {
        lv.project()
            .symbols()
            .filter(|s| s.page == lv.d.app.current_page().unwrap())
            .count()
    };
    lv.read("goal");

    // add-card
    lv.not_yet("add-card");
    assert_eq!(
        novo_plc::cards(lv.project()).len(),
        1,
        "only the analog card -K1"
    );
    lv.palette("add card DI16 DQ8");
    assert!(
        lv.last().starts_with("Added card =A1+CAB-A1 (DI16 DQ8)"),
        "{}",
        lv.last()
    );
    lv.ticked("add-card");
    assert_eq!(page_symbols(&lv), 4, "the card itself is not drawn");

    // select-button: a click on the symbol
    lv.not_yet("select-button");
    lv.click_symbol("-S1");
    lv.ticked("select-button");

    // button-to-plc
    lv.not_yet("button-to-plc");
    lv.palette("to plc");
    assert_eq!(lv.last(), "Sent to =A1+CAB-A1: next free DI channel");
    lv.ticked("button-to-plc");
    assert_eq!(page_symbols(&lv), 5);

    // lamp-to-plc
    lv.not_yet("lamp-to-plc");
    lv.click_symbol("-H1");
    assert_eq!(lv.d.app.selection().len(), 1);
    lv.palette("to plc");
    assert_eq!(lv.last(), "Sent to =A1+CAB-A1: next free DQ channel");
    lv.ticked("lamp-to-plc");
    assert_eq!(page_symbols(&lv), 6);
    let keys: Vec<String> = lv.project().symbols().map(|s| s.symbol.0.clone()).collect();
    assert!(
        keys.iter().any(|k| k.ends_with("plc/digital_input"))
            && keys.iter().any(|k| k.ends_with("plc/digital_output"))
    );

    // io-list: page 30
    lv.not_yet("io-list");
    lv.palette("page 30");
    lv.ticked("io-list");
    let rows = |lv: &mut Lv| -> Vec<Vec<String>> {
        report(lv, "io").sections[0]
            .text_rows()
            .into_iter()
            .filter(|r| r.iter().any(|c| c.contains("Pump")))
            .collect()
    };
    let io = rows(&mut lv);
    assert_eq!(io.len(), 2);
    let row = |addr: &str, ty: &str, sig: &str| {
        io.iter()
            .any(|r| r[0] == addr && r[1] == ty && r[2] == "=A1+CAB-A1" && r[5] == sig)
    };
    assert!(row("%IX12.0", "DI", "Pump start button"), "{io:?}");
    assert!(row("%QX0.0", "DQ", "Pump running lamp"), "{io:?}");

    // renumber: the preview, then Enter
    lv.not_yet("renumber");
    lv.palette("renumber io");
    assert!(lv.d.app.wb.renumber.is_some(), "the preview is waiting");
    assert!(lv.has_label("Renumber (Enter)") && lv.has_label("Cancel (Esc)"));
    let plans = lv.d.app.wb.renumber.as_ref().unwrap();
    let moves: Vec<(u32, u32)> = plans
        .iter()
        .flat_map(|p| p.moves.iter().map(|m| (m.from.unwrap(), m.to)))
        .collect();
    assert_eq!(moves, [(4, 0), (12, 8)]);
    lv.not_yet("renumber");
    lv.key(egui::Key::Enter, egui::Modifiers::NONE);
    assert_eq!(lv.last(), "Renumbered I/O: 2 start address(es) moved");
    lv.ticked("renumber");
    let io = rows(&mut lv);
    assert!(
        io.iter()
            .any(|r| r[0] == "%IX8.0" && r[5] == "Pump start button"),
        "{io:?}"
    );

    // back-to-drawing: page 1
    lv.not_yet("back-to-drawing");
    lv.palette("page 1");
    lv.ticked("back-to-drawing");

    // the finished project is this result
    let finished = novo_core::Project::from_json(include_str!(
        "../../../../assets/lessons/plc-io/finished.json"
    ))
    .unwrap();
    assert_eq!(
        finished.symbols().count(),
        lv.project().symbols().count(),
        "same number of symbols as the finished project"
    );

    // live-on: Shift+L
    lv.not_yet("live-on");
    lv.key(egui::Key::L, egui::Modifiers::SHIFT);
    assert!(lv.d.app.wb.live.on);
    lv.ticked("live-on");
    assert!(lv.has_label("Reset") && lv.has_label("PLC program…"));

    // press-button: the round badge on -S1
    lv.not_yet("press-button");
    lv.click_badge_pole();
    lv.ticked("press-button");

    // switch-output: the square badge on the DQ box; the lamp's pin carries voltage only now
    lv.not_yet("switch-output");
    lv.click_badge_output();
    lv.ticked("switch-output");
    lv.read("done");
    assert!(lv.finished());
}
