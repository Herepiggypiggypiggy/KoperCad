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
use novo_learn::check::Check;
use novo_learn::lesson::{CabinetStep, SolveOp, WireOp};
use novo_learn::{Phase, Step};

use super::cabinet::Ask;
use super::gate1_tests::Drive;
use super::workspace::Workspace;

/// Pick tolerance for clicks, in millimetres.
const TOL: f64 = 1.0;

thread_local! {
    /// Clicks the headless window could not deliver, raised as events instead.
    static SYNTHETIC_CLICKS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Filled by [`Drive::replay_lesson`]: pointers to controls that were not on screen.
    pub(super) static POINTER_MISSES: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// A `manual` reason that only says the replay could not click a panel button.
fn is_click_only(why: &str) -> bool {
    why.contains("no script clicks panel buttons")
        || why.contains("opens a window; no script clicks")
}

fn id_of_run(app: &super::NovoApp) -> String {
    app.learn
        .run
        .as_ref()
        .map(|r| r.lesson().id.clone())
        .unwrap_or_default()
}

/// Steps whose `solve` the editor cannot carry out today, each with the
/// reason. A step that starts working must come off this list.
const KNOWN_BROKEN: &[(&str, &str)] = &[];

/// What happened to one step.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Outcome {
    /// The solve ran and the check ticked.
    Ticked,
    /// A reading step, or one with nothing to check.
    Reading,
    /// An `expect` but no `solve`: nothing to replay.
    Unverified,
    /// The author marked the step `manual` (a file dialog, say), with why.
    Manual(String),
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
            SolveOp::Page(p) => {
                // A jump to a page puts where we were in the back history.
                let from = self.app.here();
                let page = self.replay_page(&p.page)?;
                if let Some(from) = from.filter(|f| f.page != page) {
                    self.app.wb.nav.history.record(from);
                }
                Ok(())
            }
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
                if let Some(seconds) = k.hold {
                    let event = |pressed| egui::Event::Key {
                        key,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::NONE,
                    };
                    let start = self.ctx.input(|i| i.time) + 1.0;
                    self.frame_at(start, vec![event(true)]);
                    let mut elapsed = 0.0;
                    while elapsed < seconds {
                        elapsed += 0.1;
                        self.frame_at(start + elapsed, Vec::new());
                    }
                    self.frame_at(start + elapsed + 0.05, vec![event(false)]);
                    return Ok(());
                }
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
            SolveOp::Tee(t) => self.replay_tee(&t.tee.from, t.tee.at),
            SolveOp::FeedRow(f) => self.replay_feed_row(&f.feed_row),
            SolveOp::Cable(c) => self.replay_cable(&c.cable.wires),
            SolveOp::IssueRevision(i) => {
                let ctx = self.ctx.clone();
                self.app
                    .run_command(&ctx, "revision.issue", Some(&i.issue_revision.description));
                let draft = self
                    .app
                    .wb
                    .issue
                    .take()
                    .ok_or("the issue window did not open")?;
                self.app.submit_issue(draft);
                self.frame();
                Ok(())
            }
            SolveOp::AssignPart(a) => self.replay_assign_part(&a.assign_part),
            SolveOp::UiClick(u) => {
                self.app.wb.wheel = None;
                self.frame();
                self.frame();
                // A control folded away or inside a closed menu is reached the
                // way a learner reaches it: through the control that opens it.
                for _ in 0..4 {
                    let mut id = u.ui_click.clone();
                    while self.app.learn_drawn(&id).is_none() {
                        id = super::learn_labels::anchor_fallback(&id).ok_or_else(|| {
                            format!("the control `{}` is not on screen", u.ui_click)
                        })?;
                    }
                    let rect = self.app.learn_drawn(&id).expect("drawn");
                    self.pointer_click(rect.center(), false);
                    if id == u.ui_click {
                        return Ok(());
                    }
                }
                Err(format!("the control `{}` did not appear", u.ui_click))
            }
            SolveOp::ListRow(l) => {
                let l = &l.list_row;
                self.app.wb.wheel = None;
                self.frame();
                self.frame();
                let rect = self.app.learn_row_rect(&l.list, &l.row).ok_or_else(|| {
                    format!(
                        "no row `{}` in `{}` (rows drawn: {})",
                        l.row,
                        l.list,
                        self.app.learn_rows_drawn(&l.list)
                    )
                })?;
                self.pointer_click(rect.center(), l.right);
                if let Some(entry) = &l.choose {
                    let rect = self
                        .app
                        .learn_row_rect(&l.list, entry)
                        .ok_or_else(|| format!("no menu entry `{entry}` for `{}`", l.list))?;
                    self.pointer_click(rect.center(), false);
                }
                Ok(())
            }
            SolveOp::TypeText(t) => {
                self.frame_with(vec![egui::Event::Text(t.type_text.clone())]);
                self.frame();
                Ok(())
            }
            SolveOp::Rename(r) => self.replay_rename(&r.rename.target, &r.rename.to),
            SolveOp::CabinetPlace(c) => {
                let a = &c.cabinet_place;
                self.replay_cabinet(&a.device, |id| Ask::Drop(id, a.x, a.y))
            }
            SolveOp::CabinetRotate(c) => {
                self.replay_cabinet(&c.cabinet_rotate, |id| Ask::Rotate(vec![id]))
            }
            SolveOp::CabinetRemove(c) => {
                self.replay_cabinet(&c.cabinet_remove, |id| Ask::Remove(vec![id]))
            }
            SolveOp::Cabinet(c) => self.replay_cabinet_preview(c.cabinet),
        }
    }

    /// An action on one device of the Cabinet plate, as the canvas raises it.
    fn replay_cabinet(&mut self, tag: &str, ask: impl FnOnce(u64) -> Ask) -> Result<(), String> {
        let (structure, layout, cands) = self
            .app
            .cabinet_plate()
            .ok_or("the Cabinet shows no plate (open it first)")?;
        let id = self
            .doc()
            .project()
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, tag))
            .map(|d| d.id.raw())
            .ok_or_else(|| format!("nothing named `{tag}`"))?;
        self.app
            .cabinet_ask(ask(id), Some(structure), layout, &cands);
        self.frame();
        Ok(())
    }

    /// Place all (the preview) or accepting it.
    fn replay_cabinet_preview(&mut self, step: CabinetStep) -> Result<(), String> {
        let (structure, layout, cands) = self
            .app
            .cabinet_plate()
            .ok_or("the Cabinet shows no plate (open it first)")?;
        match step {
            CabinetStep::PlaceAll => {
                self.app
                    .cabinet_ask(Ask::PlaceAll, Some(structure), layout, &cands);
                self.frame();
                if self.app.cabinet.preview.is_none() {
                    return Err("Place all found nothing to place".into());
                }
                // The button click that raises the preview.
                self.app
                    .learn_event(novo_learn::Event::Clicked("cabinet.place_all".into()));
            }
            CabinetStep::Accept => {
                if self.app.cabinet.preview.is_none() {
                    return Err("there is no Place all preview to accept".into());
                }
                self.app
                    .cabinet_ask(Ask::Accept, Some(structure), None, &[]);
                self.frame();
            }
        }
        Ok(())
    }

    /// A wire from a pin that ends on another wire at a point, as dragging
    /// it there does: the T joins the two.
    fn replay_tee(&mut self, from: &str, at: [f64; 2]) -> Result<(), String> {
        let (page, start) = self.replay_pin(from)?;
        self.replay_page_id(page);
        self.replay_leave();
        let doc = self.app.backend.document().ok_or("no project")?;
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        view.editor.hover(doc, page, start, TOL);
        if !view.editor.begin_drag() {
            return Err(format!("no pin `{from}` to drag from"));
        }
        let result = self.replay_click(page, Point::new(at[0], at[1]));
        self.replay_leave();
        result
    }

    /// A row of feeds dragged out from a point at the pole pitch.
    fn replay_feed_row(&mut self, row: &novo_learn::lesson::FeedRowArgs) -> Result<(), String> {
        const PITCH: f64 = 8.0;
        let page = self.replay_page(&row.page)?;
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.app.backend.document().ok_or("no project")?;
        if !view
            .editor
            .begin_place("iec/connectivity/potential_feed", None)
        {
            return Err("the feed cannot be placed".into());
        }
        view.editor.hover(doc, page, Point::new(row.x, row.y), TOL);
        if !view.editor.begin_row() {
            return Err("a row cannot start here".into());
        }
        let last = row.names.len().saturating_sub(1) as f64;
        view.editor
            .row_follow(Point::new(row.x + PITCH * last + 1.0, row.y), PITCH);
        let doc = self.app.backend.document_mut().ok_or("no project")?;
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        let done = view.editor.end_row(doc, page);
        let notice = view.editor.notice().map(str::to_owned);
        self.replay_leave();
        if done == novo_editor::tool::Clicked::Edited {
            Ok(())
        } else {
            Err(notice.unwrap_or_else(|| "the row was not placed".into()))
        }
    }

    /// A cable over the wires that join these pairs of pins, as the cables
    /// drawer's button makes it.
    fn replay_cable(&mut self, pairs: &[[String; 2]]) -> Result<(), String> {
        let project = self.doc().project();
        let mut wires = Vec::new();
        for [a, b] in pairs {
            let pa = novo_learn::check::pins_named(project, a);
            let pb = novo_learn::check::pins_named(project, b);
            let found = project.wires().find(|w| {
                let (f, t) = (w.from.as_pin(), w.to.as_pin());
                pa.iter().any(|x| Some(*x) == f) && pb.iter().any(|y| Some(*y) == t)
                    || pb.iter().any(|x| Some(*x) == f) && pa.iter().any(|y| Some(*y) == t)
            });
            wires.push(found.ok_or_else(|| format!("no wire {a} - {b}"))?.id);
        }
        self.app.make_cable(&wires);
        self.frame();
        Ok(())
    }

    /// A part with this maker and order number on a device, as the
    /// spreadsheet import or the part picker leaves it.
    fn replay_assign_part(&mut self, a: &novo_learn::lesson::AssignPartArgs) -> Result<(), String> {
        let project = self.doc().project();
        let device = project
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, &a.device))
            .ok_or_else(|| format!("nothing named `{}`", a.device))?
            .id;
        let doc = self.app.backend.document_mut().ok_or("no project")?;
        let (_, changes) = doc
            .transact("Assign part", |tx| {
                let part = tx.add_part(&a.manufacturer, &a.order_number)?;
                tx.assign_part(device, part)
            })
            .map_err(|e| e.to_string())?;
        self.app.changed_outside_canvas(&changes);
        self.frame();
        Ok(())
    }

    /// The rename box's Enter: a terminal by designation, an interruption
    /// point by name, or a device by tag.
    fn replay_rename(&mut self, target: &str, to: &str) -> Result<(), String> {
        let project = self.doc().project();
        let label_symbol: Option<SymbolId> = if let Some((strip, name)) = target.rsplit_once(':') {
            let strip = project
                .devices()
                .find(|d| novo_learn::check::tag_matches(&d.tag, strip))
                .ok_or_else(|| format!("nothing named `{strip}`"))?
                .id;
            let terminal = project
                .terminals_of_strip(strip)
                .into_iter()
                .find(|t| t.name == name)
                .ok_or_else(|| format!("no terminal `{target}`"))?
                .id;
            project
                .symbols()
                .find(|s| {
                    matches!(&s.role, novo_core::SymbolRole::Terminal { terminals }
                        if terminals.contains(&terminal))
                })
                .map(|s| s.id)
        } else {
            project
                .symbols()
                .find(|s| {
                    matches!(&s.role, novo_core::SymbolRole::Interruption { name }
                        if name == target)
                })
                .map(|s| s.id)
        };
        if let Some(symbol) = label_symbol {
            let doc = self.app.backend.document_mut().ok_or("no project")?;
            let (result, changes) = doc
                .transact("Rename", |tx| {
                    Ok(novo_editor::edit::rename_label(tx, symbol, to))
                })
                .map_err(|e| e.to_string())?;
            self.app.changed_outside_canvas(&changes);
            self.frame();
            return result;
        }
        let device = project
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, target))
            .ok_or_else(|| format!("nothing named `{target}` to rename"))?
            .id;
        let doc = self.app.backend.document_mut().ok_or("no project")?;
        let view = self.app.canvas.schematic().ok_or("no editor canvas")?;
        let result = view.editor.rename(doc, device, to);
        self.frame();
        result
    }

    /// One frame at egui time `time` seconds, so a held key counts time.
    fn frame_at(&mut self, time: f64, events: Vec<egui::Event>) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1600.0, 900.0),
            )),
            time: Some(time),
            events,
            ..Default::default()
        };
        let app = &mut self.app;
        let mut output = self.ctx.run_ui(input, |ui| app.show(ui));
        output.textures_delta.clear();
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
        // Nothing named: a click on an empty spot of the plate.
        if text.is_empty() {
            self.app.cabinet.view.selection.clear();
            self.frame();
            return Ok(());
        }
        // In the Cabinet a click lands on the plate.
        if !text.contains(':')
            && matches!(self.app.workspace(), Workspace::Cabinet | Workspace::View3d)
            && self.replay_select_on_plate(text, add).is_ok()
        {
            return Ok(());
        }
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
        if !shift && super::live::is_momentary(self.doc().project(), badge.symbol) {
            // A pushbutton goes down with the click and springs back on the
            // frame after, when no pointer holds it. The lesson's check is
            // read while it is down, as it is on the frames of a real press.
            self.app.live_pole_pressed(badge.symbol, badge.group);
            let ctx = self.ctx.clone();
            self.app.learn_step(&ctx);
        } else {
            self.app
                .live_pole_clicked(badge.symbol, badge.group, !shift);
        }
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
        // Windows an earlier lesson's solve left open would sit over the
        // controls this lesson clicks.
        self.app.wb.macros.open = false;
        self.app.wb.macro_save = None;
        self.app.wb.project_settings = false;
        self.app.cabinet.plate_card = false;
        self.app.wb.numbering = None;
        self.app.wb.wheel = None;
        // And so would a dialog (settings, shortcuts): Escape closes each.
        for _ in 0..4 {
            self.frame_with(vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }]);
            self.frame();
        }
        self.app.start_lesson(id);
        self.frame();
        self.frame();
        let mut out = Vec::new();
        let mut guard = 0;
        let mut next = 0;
        let mut pointer_gap = false;
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
            // The lessons tell the learner to close a reading window (Place
            // rules, Rails and ducts, Cut list) when done; left open it sits
            // over the bar and takes the next click.
            self.app.cabinet.rules.editor_open = false;
            self.app.cabinet.rules.cut_open = false;
            self.app.cabinet.rails.open = false;
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
            let lesson_id = id_of_run(&self.app);
            let before = self.pointer_misses(&lesson_id, &step);
            let synthetic = SYNTHETIC_CLICKS.with(std::cell::Cell::get);
            let outcome = self.replay_step(&step);
            // Once a click was only simulated, or a step could not be played,
            // the panels after it are not where a learner's would be.
            if SYNTHETIC_CLICKS.with(std::cell::Cell::get) != synthetic
                || matches!(
                    outcome,
                    Outcome::Unverified | Outcome::Manual(_) | Outcome::Unsupported(_)
                )
            {
                pointer_gap = true;
            }
            // Not on screen before the step and still not once it is done:
            // a ring a learner never sees (a step that opens its own window
            // is allowed to point at it only after).
            if !before.is_empty() && !pointer_gap {
                let after = self.pointer_misses(&lesson_id, &step);
                for miss in before.into_iter().filter(|m| after.contains(m)) {
                    POINTER_MISSES.with(|m| m.borrow_mut().push(miss));
                }
            }
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

    /// The control pointers of a step (its ring and its numbered notes) that
    /// name an anchor which is nowhere on screen while the step is current.
    /// The lint only knows the id exists; this is the "the Apply button the
    /// text points at is not drawn" check.
    fn pointer_misses(&mut self, lesson: &str, step: &Step) -> Vec<String> {
        self.frame();
        self.frame();
        let Some((_, canvas)) = self.app.wb.canvas_area else {
            return Vec::new();
        };
        let mut targets: Vec<&novo_learn::Target> = step.point.iter().collect();
        targets.extend(step.note.iter().map(|n| &n.target));
        targets
            .into_iter()
            .filter(|t| t.ui.is_some() && self.app.target_rect(t, canvas).is_none())
            .map(|t| {
                let ids =
                    t.ui.as_ref()
                        .map(|u| u.ids().join(" > "))
                        .unwrap_or_default();
                format!("{lesson}/{}: `{ids}`", step.id)
            })
            .collect()
    }

    /// A pointer click at `at`, with the right button when `secondary`.
    fn pointer_click(&mut self, at: egui::Pos2, secondary: bool) {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: if secondary {
                egui::PointerButton::Secondary
            } else {
                egui::PointerButton::Primary
            },
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame_with(vec![egui::Event::PointerMoved(at)]);
        self.frame_with(vec![button(true)]);
        self.frame_with(vec![button(false)]);
        self.frame();
        self.frame();
    }

    /// A real pointer click on the middle of an anchored control.
    fn replay_click_anchor(&mut self, step_id: &str, id: &str) -> Result<(), String> {
        // A wheel left up by an earlier hold swallows every click.
        self.app.wb.wheel = None;
        self.frame();
        self.frame();
        let rect = self
            .app
            .learn_anchor(id)
            .ok_or_else(|| format!("the control `{id}` is not on screen"))?;
        let at = rect.center();
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        self.frame_with(vec![egui::Event::PointerMoved(at)]);
        self.frame_with(vec![button(true)]);
        self.frame_with(vec![button(false)]);
        self.frame();
        self.frame();
        let ticked = self.app.learn.run.as_ref().is_some_and(|r| {
            r.phase() == Phase::Ticked || r.step().is_none_or(|s| s.id != step_id)
        });
        if !ticked {
            // The headless window cannot always deliver the press (a window
            // an earlier step left open can swallow it). The control is on
            // screen, which is what this proves; raise the click it would
            // have made and keep the screen state as it is.
            self.app
                .learn_event(novo_learn::Event::Clicked(id.to_owned()));
            self.frame();
            self.frame();
            SYNTHETIC_CLICKS.with(|c| c.set(c.get() + 1));
        }
        Ok(())
    }

    fn replay_step(&mut self, step: &Step) -> Outcome {
        if step.expect.is_none() {
            // A reading step may still carry a `solve` that opens the panel
            // the next step points at: do it, so the screen is where a
            // learner's would be.
            for op in &step.solve {
                if self.replay_op(op).is_err() {
                    break;
                }
            }
            return Outcome::Reading;
        }
        if step.solve.is_empty() {
            // "Click X" with no script: click the control where it is drawn,
            // as a learner does. A control that is not there, or does not
            // take the click, fails the step. This also covers steps marked
            // manual only because no script clicked panel buttons.
            if let Some(Check::Clicked(id)) = &step.expect
                && step.manual.as_deref().is_none_or(is_click_only)
            {
                if let Err(why) = self.replay_click_anchor(&step.id, id) {
                    return Outcome::Failed(why);
                }
            } else if let Some(why) = &step.manual {
                return Outcome::Manual(why.clone());
            } else {
                return Outcome::Unverified;
            }
        } else if let Some(why) = &step.manual {
            return Outcome::Manual(why.clone());
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
    let mut manual: Vec<String> = Vec::new();
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
                    eprintln!("  no solve: {key}");
                }
                Outcome::Manual(why) => {
                    manual.push(format!("{key}: {why}"));
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
        "replay: {ticked} steps ticked, {unverified} without solve, {unsupported} with ops not replayable yet, {} not scripted (marked manual), {} waiting for controls this build lacks, {} after a gap, {} known broken, {} failed",
        manual.len(),
        waiting.len(),
        downstream.len(),
        known_seen.len(),
        failed.len()
    );
    for f in &failed {
        eprintln!("  FAILED: {f}");
    }
    for w in waiting.iter().chain(&downstream) {
        eprintln!("  not counted: {w}");
    }
    // A ring the learner never sees: the step points at a control that is
    // not on screen before the step or after it. Steps the lint already
    // marks as ahead of this build only warn.
    let misses: Vec<String> = POINTER_MISSES
        .with(|m| m.borrow().clone())
        .into_iter()
        .filter(|m| {
            let key = m.split(": ").next().unwrap_or_default();
            !ahead.iter().any(|(l, s)| format!("{l}/{s}") == key)
        })
        .collect();
    assert!(
        misses.is_empty(),
        "steps whose ring points at a control that is not on screen (register the anchor, \
         add a fallback in learn_labels::anchor_fallback, or fix the step):\n{}",
        misses.join("\n")
    );
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
