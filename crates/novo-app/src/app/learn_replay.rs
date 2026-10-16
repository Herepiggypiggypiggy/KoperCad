//! The replay gate: every lesson is played from its own sample project, each
//! step's `solve` going through the same editor calls a person's clicks and
//! keys make, and the step's `expect` must tick. A step whose instruction the
//! editor refuses fails here, not in front of a learner.
//!
//! Steps with an `expect` and no `solve` are listed as unverified, and solve
//! ops this replay cannot perform yet as unsupported; neither fails the gate.

use eframe::egui;
use novo_core::{AnyId, PageId, Point, SymbolId};
use novo_editor::tool::{EditorKey, WiringMode};
use novo_learn::lesson::{SolveOp, WireOp};
use novo_learn::{Phase, Step};

use super::gate1_tests::Drive;

/// Pick tolerance for clicks, in millimetres.
const TOL: f64 = 1.0;

/// Steps whose `solve` the editor cannot carry out today, each with the
/// reason. A step that starts working must come off this list.
const KNOWN_BROKEN: &[(&str, &str)] = &[(
    "interruption-points/same-name",
    "needs a `rename` solve op (double-click the name, Ctrl+A, type, Enter); `xref.link` cannot pair points on two pages",
)];

/// What happened to one step.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Outcome {
    /// The solve ran and the check ticked.
    Ticked,
    /// A reading step, or one with nothing to check.
    Reading,
    /// An `expect` but no `solve`: nothing to replay.
    Unverified,
    /// A solve op the replay cannot perform yet.
    Unsupported(String),
    /// The solve ran, the check did not tick.
    Failed(String),
}

impl Drive {
    fn replay_page(&mut self, name: &str) -> Result<PageId, String> {
        let page = self
            .doc()
            .project()
            .pages_in_order()
            .into_iter()
            .find(|p| p.name == name || p.title == name)
            .map(|p| p.id)
            .ok_or_else(|| format!("no page `{name}`"))?;
        self.app.show_page(page);
        self.frame();
        Ok(page)
    }

    fn replay_pin(&self, text: &str) -> Result<(PageId, Point), String> {
        let project = self.doc().project();
        let found = novo_learn::check::pins_named(project, text);
        let pin = found
            .first()
            .ok_or_else(|| format!("no pin `{text}` in the project"))?;
        let symbol = project.symbol(pin.symbol).ok_or("symbol")?;
        let at = symbol
            .pin_position(pin.pin)
            .ok_or_else(|| format!("pin `{text}` has no position"))?;
        Ok((symbol.page, at))
    }

    fn replay_click(&mut self, page: PageId, at: Point) -> Result<(), String> {
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.app.backend.document_mut().ok_or("no project")?;
        view.editor.click(doc, page, at, TOL);
        let notice = view.editor.notice().map(str::to_owned);
        self.frame();
        match notice {
            Some(n) => Err(n),
            None => Ok(()),
        }
    }

    fn replay_leave(&mut self) {
        if let Some(view) = self.app.canvas.schematic() {
            view.editor.leave_tool();
        }
        self.frame();
    }

    pub(super) fn replay_op(&mut self, op: &SolveOp) -> Result<(), String> {
        let ctx = self.ctx.clone();
        match op {
            SolveOp::Command(c) => {
                self.app.run_command(&ctx, &c.command, c.arg.as_deref());
                self.frame();
                Ok(())
            }
            SolveOp::Page(p) => self.replay_page(&p.page).map(|_| ()),
            SolveOp::Place(p) => {
                let a = &p.place;
                let page = self.replay_page(&a.page)?;
                let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
                if !view.editor.begin_place(&a.symbol, a.parent.as_deref()) {
                    return Err(view
                        .editor
                        .notice()
                        .unwrap_or("the symbol cannot be placed")
                        .to_owned());
                }
                for _ in 0..a.turns % 4 {
                    let doc = self.app.backend.document_mut().ok_or("no project")?;
                    let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
                    view.editor.key(doc, EditorKey::Rotate, &[]);
                }
                let result = self.replay_click(page, Point::new(a.x, a.y));
                self.replay_leave();
                result
            }
            SolveOp::Wire(w) => self.replay_wire(w),
            SolveOp::LivePole(l) => self.replay_live_pole(&l.live_pole.device, l.live_pole.shift),
            SolveOp::LiveCoil(l) => {
                if !self.app.wb.live.on {
                    return Err("the live view is not on (turn it on first)".into());
                }
                let device = &l.live_coil.device;
                let symbols = self.replay_symbols(device)?;
                let page = self.app.current_page().ok_or("no page")?;
                let badge = super::live::coil_badges(
                    self.doc().project(),
                    page,
                    &Default::default(),
                    &Default::default(),
                )
                .into_iter()
                .find(|b| b.output == l.live_coil.output && symbols.contains(&b.symbol))
                .ok_or_else(|| format!("`{device}` has no coil badge on this page"))?;
                self.app.live_coil_clicked(badge.symbol, badge.output);
                self.frame();
                Ok(())
            }
            SolveOp::SetPotential(p) => self.replay_set_potential(&p.set_potential),
            SolveOp::Key(k) => {
                let key =
                    egui::Key::from_name(&k.key).ok_or_else(|| format!("no key `{}`", k.key))?;
                self.frame_with(vec![egui::Event::Key {
                    key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::NONE,
                }]);
                self.frame();
                Ok(())
            }
            SolveOp::Select(s) => self.replay_select(&s.select, s.add),
            other => Err(format!("unsupported:{}", op_name(other))),
        }
    }

    fn replay_wire(&mut self, w: &WireOp) -> Result<(), String> {
        let (page, from) = self.replay_pin(&w.wire[0])?;
        let (_, to) = self.replay_pin(&w.wire[1])?;
        self.replay_page_id(page);
        if let Some(mode) = &w.mode {
            let mode = match mode.as_str() {
                "lines" => WiringMode::Lines,
                _ => WiringMode::Corners,
            };
            if let Some(view) = self.app.canvas.schematic() {
                view.editor.set_wire_mode(mode);
            }
        }
        let ctx = self.ctx.clone();
        self.app.run_command(&ctx, "tool.connect", None);
        self.frame();
        let mut result = self.replay_click(page, from);
        for via in &w.via {
            if result.is_ok() {
                result = self.replay_click(page, Point::new(via[0], via[1]));
            }
        }
        if result.is_ok() {
            result = self.replay_click(page, to);
        }
        self.replay_leave();
        result
    }

    /// Selects a device (all its symbols) or one pin's symbol, as a click
    /// (or Shift+click) on it does.
    fn replay_select(&mut self, text: &str, add: bool) -> Result<(), String> {
        let project = self.doc().project();
        let mut found: Vec<SymbolId> = Vec::new();
        if text.contains(':') {
            found.extend(
                novo_learn::check::pins_named(project, text)
                    .first()
                    .map(|p| p.symbol),
            );
        } else {
            for symbol in project.symbols() {
                let tag = novo_editor::edit::device_of(project, AnyId::Symbol(symbol.id))
                    .and_then(|d| project.device(d))
                    .map(|d| d.tag.clone());
                if tag.is_some_and(|t| novo_learn::check::tag_matches(&t, text)) {
                    found.push(symbol.id);
                }
            }
        }
        if found.is_empty() && !text.contains(':') {
            return self.replay_select_on_plate(text, add);
        }
        let first = *found
            .first()
            .ok_or_else(|| format!("nothing named `{text}` to select"))?;
        let page = self
            .doc()
            .project()
            .symbol(first)
            .map(|s| s.page)
            .ok_or("symbol")?;
        self.replay_page_id(page);
        if !add {
            self.app.cabinet.view.selection.clear();
        }
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        if !add {
            view.canvas.selection.clear();
        }
        view.canvas.selection.apply(
            found
                .iter()
                .map(|s| novo_canvas::model::item_id(AnyId::Symbol(*s))),
            novo_canvas::SelectOp::Add,
        );
        self.frame();
        Ok(())
    }

    /// Selects a device that has no symbol but a place on the plate, as a
    /// click on it in the Cabinet does.
    fn replay_select_on_plate(&mut self, text: &str, add: bool) -> Result<(), String> {
        let project = self.doc().project();
        let id = project
            .all::<novo_core::CabinetPlacement>()
            .filter_map(|p| project.device(p.device))
            .find(|d| novo_learn::check::tag_matches(&d.tag, text))
            .map(|d| d.id)
            .ok_or_else(|| format!("nothing named `{text}` to select"))?;
        let selection = &mut self.app.cabinet.view.selection;
        if !add {
            selection.clear();
        }
        selection.apply(
            [novo_canvas::cabinet::box_item(id.raw())],
            novo_canvas::SelectOp::Add,
        );
        self.frame();
        Ok(())
    }

    fn replay_symbols(&self, text: &str) -> Result<Vec<SymbolId>, String> {
        let project = self.doc().project();
        let found: Vec<SymbolId> = project
            .symbols()
            .filter(|symbol| {
                novo_editor::edit::device_of(project, AnyId::Symbol(symbol.id))
                    .and_then(|d| project.device(d))
                    .is_some_and(|d| novo_learn::check::tag_matches(&d.tag, text))
            })
            .map(|s| s.id)
            .collect();
        if found.is_empty() {
            Err(format!("nothing named `{text}`"))
        } else {
            Ok(found)
        }
    }

    fn replay_live_pole(&mut self, device: &str, shift: bool) -> Result<(), String> {
        if !self.app.wb.live.on {
            return Err("the live view is not on (turn it on first)".into());
        }
        let symbols = self.replay_symbols(device)?;
        let page = self.app.current_page().ok_or("no page")?;
        let badge = super::live::pole_badges(self.doc().project(), page, &self.app.wb.live.poles)
            .into_iter()
            .find(|b| symbols.contains(&b.symbol))
            .ok_or_else(|| format!("`{device}` has no pole badge on this page"))?;
        self.app
            .live_pole_clicked(badge.symbol, badge.group, !shift);
        self.frame();
        Ok(())
    }

    fn replay_set_potential(
        &mut self,
        args: &novo_learn::lesson::SetPotentialArgs,
    ) -> Result<(), String> {
        if let Some(name) = &args.page {
            self.replay_page(name)?;
        }
        let page = self.app.current_page().ok_or("no page")?;
        let at = Point::new(args.feed_at[0], args.feed_at[1]);
        let feed = self
            .doc()
            .project()
            .symbols()
            .find(|s| {
                s.page == page && {
                    let o = s.placement.origin;
                    (o.x - at.x).hypot(o.y - at.y) < 1.0
                }
            })
            .map(|s| s.id)
            .ok_or_else(|| format!("no feed at {at:?}"))?;
        let menu =
            novo_editor::potential_menu::feed_menu(self.doc().project(), &[AnyId::Symbol(feed)])
                .ok_or("that symbol is not a potential feed")?;
        let chip = menu
            .chips
            .iter()
            .find(|c| c.name == args.chip)
            .cloned()
            .ok_or_else(|| format!("the menu has no chip `{}`", args.chip))?;
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.app.backend.document_mut().ok_or("no project")?;
        if !view.editor.set_feed_potential(doc, &[feed], &chip) {
            return Err(view
                .editor
                .notice()
                .unwrap_or("the chip was refused")
                .to_owned());
        }
        self.frame();
        Ok(())
    }

    fn replay_page_id(&mut self, page: PageId) {
        self.app.show_page(page);
        self.frame();
    }

    /// Plays every step of a lesson; one outcome per step, in order.
    pub(super) fn replay_lesson(&mut self, id: &str) -> Vec<(String, Outcome)> {
        self.app.start_lesson(id);
        self.frame();
        self.frame();
        let mut out = Vec::new();
        let mut guard = 0;
        let mut next = 0;
        while let Some(step) = self.app.learn.run.as_ref().and_then(|r| r.step()).cloned() {
            // Steps that ticked by themselves before the replay got to them
            // (their check already held) have nothing of their own to show.
            let (index, passed): (usize, Vec<Step>) = self
                .app
                .learn
                .run
                .as_ref()
                .map(|r| {
                    let lesson = r.lesson();
                    let index = r.index();
                    (index, lesson.steps[next.min(index)..index].to_vec())
                })
                .unwrap_or_default();
            for skipped in passed {
                let outcome = if skipped.expect.is_some() {
                    Outcome::Unverified
                } else {
                    Outcome::Reading
                };
                out.push((skipped.id, outcome));
            }
            next = index + 1;
            guard += 1;
            if guard > 200
                || self
                    .app
                    .learn
                    .run
                    .as_ref()
                    .is_none_or(|r| r.phase() == Phase::Finished)
            {
                break;
            }
            let outcome = self.replay_step(&step);
            out.push((step.id.clone(), outcome));
            // Move on whatever happened, so one failure shows once.
            if let Some(run) = self.app.learn.run.as_mut()
                && run.step().is_some_and(|s| s.id == step.id)
            {
                if run.phase() == Phase::Ticked {
                    run.advance();
                } else {
                    run.skip();
                }
            }
            self.frame();
            if self.app.learn.run.is_none() {
                break;
            }
        }
        out
    }

    fn replay_step(&mut self, step: &Step) -> Outcome {
        if step.expect.is_none() {
            return Outcome::Reading;
        }
        if step.solve.is_empty() {
            return Outcome::Unverified;
        }
        for op in &step.solve {
            if let Err(why) = self.replay_op(op) {
                return match why.strip_prefix("unsupported:") {
                    Some(name) => Outcome::Unsupported(name.to_owned()),
                    None => Outcome::Failed(why),
                };
            }
        }
        self.frame();
        self.frame();
        let ticked = self.app.learn.run.as_ref().is_some_and(|r| {
            r.phase() == Phase::Ticked || r.step().is_none_or(|s| s.id != step.id)
        });
        if ticked {
            Outcome::Ticked
        } else {
            let notice = self
                .app
                .canvas
                .schematic_ref()
                .and_then(|v| v.editor.notice().map(str::to_owned));
            Outcome::Failed(notice.unwrap_or_else(|| "the check did not tick".into()))
        }
    }
}

fn op_name(op: &SolveOp) -> &'static str {
    match op {
        SolveOp::Command(_) => "command",
        SolveOp::Place(_) => "place",
        SolveOp::Wire(_) => "wire",
        SolveOp::Tee(_) => "tee",
        SolveOp::FeedRow(_) => "feed_row",
        SolveOp::Select(_) => "select",
        SolveOp::Page(_) => "page",
        SolveOp::Cable(_) => "cable",
        SolveOp::IssueRevision(_) => "issue_revision",
        SolveOp::AssignPart(_) => "assign_part",
        SolveOp::Key(_) => "key",
        SolveOp::LivePole(_) => "live_pole",
        SolveOp::LiveCoil(_) => "live_coil",
        SolveOp::SetPotential(_) => "set_potential",
    }
}

#[test]
fn every_shipped_lesson_replays_from_its_own_solve() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.learn.advance_after = std::time::Duration::ZERO;
    let ids: Vec<String> = d
        .app
        .learn
        .catalog
        .entries
        .iter()
        .map(|e| e.lesson.id.clone())
        .collect();
    // Steps that name a command, control or signal this build lacks still
    // fail to tick; those are the lint's warnings, and they are not the
    // editor refusing what a lesson says.
    let ahead: std::collections::BTreeSet<(String, String)> = d
        .app
        .learn
        .lint_report()
        .findings
        .iter()
        .filter(|f| f.message.contains("not in this build"))
        .filter_map(|f| Some((f.lesson.clone(), f.step.clone()?)))
        .collect();
    let mut failed = Vec::new();
    let mut waiting = Vec::new();
    let mut downstream = Vec::new();
    let mut known_seen = Vec::new();
    let (mut ticked, mut unverified, mut unsupported) = (0, 0, 0);
    for id in ids {
        // After a step this replay could not perform, later steps may lack
        // what it would have drawn; their failures are not the editor's.
        let mut gap = false;
        for (step, outcome) in d.replay_lesson(&id) {
            let key = format!("{id}/{step}");
            match outcome {
                Outcome::Failed(why) => {
                    if let Some((_, reason)) = KNOWN_BROKEN.iter().find(|(k, _)| *k == key) {
                        known_seen.push(format!("{key}: {reason}"));
                        eprintln!("  known broken: {key}: {reason}");
                        gap = true;
                    } else if ahead.contains(&(id.clone(), step.clone())) {
                        waiting.push(format!("{key}: {why}"));
                        gap = true;
                    } else if gap {
                        downstream.push(format!("{key}: {why}"));
                    } else {
                        failed.push(format!("{key}: {why}"));
                    }
                }
                Outcome::Ticked => {
                    ticked += 1;
                    assert!(
                        !KNOWN_BROKEN.iter().any(|(k, _)| *k == key),
                        "{key} works now: take it off KNOWN_BROKEN"
                    );
                }
                Outcome::Reading => {}
                Outcome::Unverified => {
                    unverified += 1;
                    gap = true;
                }
                Outcome::Unsupported(name) => {
                    unsupported += 1;
                    gap = true;
                    eprintln!("  unsupported: {key}: {name}");
                }
            }
        }
    }
    eprintln!(
        "replay: {ticked} steps ticked, {unverified} without solve, {unsupported} with ops not replayable yet, {} waiting for controls this build lacks, {} after a gap, {} known broken, {} failed",
        waiting.len(),
        downstream.len(),
        known_seen.len(),
        failed.len()
    );
    for w in waiting.iter().chain(&downstream) {
        eprintln!("  not counted: {w}");
    }
    assert!(
        failed.is_empty(),
        "steps the editor refused:\n{}",
        failed.join("\n")
    );
}

const SAMPLE: &str = r#"
format = 1
id = "replayed"
version = 1
title = "Replayed"
summary = "The gate itself."
track = "basics"
project = "@blank"

[[step]]
id = "fit"
title = "Fit"
text = "Fit the view."
expect = { command = "view.fit" }
solve = [ { command = "view.fit" } ]

[[step]]
id = "no-such-symbol"
title = "Place something that is not in the library"
text = "x"
expect = { count = { kind = "symbol", at_least = 1 } }
solve = [ { place = { symbol = "iec/nothing/at_all", page = "1", x = 40, y = 40 } } ]

[[step]]
id = "wrong-check"
title = "A solve that does not do what the check wants"
text = "x"
expect = { count = { kind = "symbol", at_least = 9 } }
solve = [ { command = "view.fit" } ]

[[step]]
id = "no-solve"
title = "No script"
text = "x"
expect = { count = { kind = "wire", at_least = 9 } }
"#;

#[test]
fn the_gate_catches_a_solve_the_editor_refuses_or_that_does_not_tick() {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    let mut files = novo_learn::Files::new();
    files.insert(
        "tracks.toml".into(),
        "[[track]]\nid = \"basics\"\ntitle = \"Basics\"\norder = 1\n".into(),
    );
    files.insert("replayed/lesson.toml".into(), SAMPLE.into());
    d.app.learn.use_files(files);
    d.app.learn.use_progress_file(dir.path().join("learn.json"));
    d.app.learn.advance_after = std::time::Duration::ZERO;
    d.app.run_command(&d.ctx.clone(), "file.new", None);
    let outcomes = d.replay_lesson("replayed");
    let by = |id: &str| {
        outcomes
            .iter()
            .find(|(s, _)| s == id)
            .map(|(_, o)| o.clone())
    };
    assert_eq!(by("fit"), Some(Outcome::Ticked), "{outcomes:?}");
    assert!(
        matches!(by("no-such-symbol"), Some(Outcome::Failed(_))),
        "{outcomes:?}"
    );
    assert!(
        matches!(by("wrong-check"), Some(Outcome::Failed(_))),
        "{outcomes:?}"
    );
    assert_eq!(by("no-solve"), Some(Outcome::Unverified), "{outcomes:?}");
}

#[test]
fn key_hints_in_ready_lessons_name_a_bound_key() {
    // `{key:command}` shows the person's own key; a command with no chord
    // reads "(no key)" and the step then teaches nothing. Drafts and early
    // access lessons are listed, ready ones fail.
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    d.app.learn.reload();
    let mut unbound: Vec<(bool, String)> = Vec::new();
    for entry in &d.app.learn.catalog.entries {
        let lesson = &entry.lesson;
        for step in &lesson.steps {
            let mut texts: Vec<&str> = vec![&step.title, &step.text];
            texts.extend(step.text_nl.as_deref());
            texts.extend(step.how.iter().chain(&step.how_nl).map(String::as_str));
            texts.extend(step.note.iter().map(|n| n.text.as_str()));
            for text in texts {
                let Ok(spans) = novo_learn::parse_markup(text) else {
                    continue;
                };
                for (kind, value) in novo_learn::markup::references(&spans) {
                    if kind == "key" && d.app.wb.keymap.shortcut_text(value).is_none() {
                        unbound.push((
                            lesson.status == novo_learn::Status::Ready,
                            format!("{}/{}: {{key:{value}}}", lesson.id, step.id),
                        ));
                    }
                }
            }
        }
    }
    unbound.sort();
    unbound.dedup();
    for (_, line) in &unbound {
        eprintln!("  no key: {line}");
    }
    let ready: Vec<&String> = unbound.iter().filter(|(r, _)| *r).map(|(_, l)| l).collect();
    assert!(
        ready.is_empty(),
        "ready lessons name a command with no key:\n{ready:?}"
    );
}
