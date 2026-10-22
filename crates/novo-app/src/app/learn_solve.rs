//! The editor-side half of a lesson step's `solve`: the document-changing
//! ops, run through the same editor calls a person's clicks make. The replay
//! gate (`learn_replay`) drives them with frames in between; the card's
//! "Skip step" applies them live so the steps after it still find their parts.

use eframe::egui;
use novo_core::{AnyId, Document, PageId, Point, SymbolId};
use novo_editor::tool::{EditorKey, WiringMode};
use novo_learn::lesson::{CabinetStep, SolveOp, WireOp};

use super::cabinet::Ask;

use super::NovoApp;
use super::workspace::Workspace;

/// Pick tolerance for clicks, in millimetres.
const TOL: f64 = 1.0;

impl NovoApp {
    /// Applies one document-changing `solve` op. `None` when the op is not
    /// one of those (a click on a control, a held key, a live view badge):
    /// those need the screen and are only played by the replay.
    pub(super) fn solve_op(
        &mut self,
        ctx: &egui::Context,
        op: &SolveOp,
    ) -> Option<Result<(), String>> {
        Some(match op {
            SolveOp::Command(c) => {
                self.run_command(ctx, &c.command, c.arg.as_deref());
                Ok(())
            }
            SolveOp::Page(p) => {
                // A jump to a page puts where we were in the back history.
                let from = self.here();
                self.solve_page(&p.page).map(|page| {
                    if let Some(from) = from.filter(|f| f.page != page) {
                        self.wb.nav.history.record(from);
                    }
                })
            }
            SolveOp::Place(p) => self.solve_place(&p.place),
            SolveOp::Wire(w) => self.solve_wire(ctx, w),
            SolveOp::Select(s) => self.solve_select(&s.select, s.add),
            SolveOp::Choose(c) => self.learn_predict(c.choose),
            SolveOp::Tee(t) => self.solve_tee(&t.tee.from, t.tee.at),
            SolveOp::FeedRow(f) => self.solve_feed_row(&f.feed_row),
            SolveOp::Cable(c) => self.solve_cable(&c.cable.wires),
            SolveOp::IssueRevision(i) => {
                self.run_command(ctx, "revision.issue", Some(&i.issue_revision.description));
                match self.wb.issue.take() {
                    Some(draft) => {
                        self.submit_issue(draft);
                        Ok(())
                    }
                    None => Err("the issue window did not open".into()),
                }
            }
            SolveOp::AssignPart(a) => self.solve_assign_part(&a.assign_part),
            SolveOp::Rename(r) => self.solve_rename(&r.rename.target, &r.rename.to),
            SolveOp::SetPotential(p) => self.solve_set_potential(&p.set_potential),
            SolveOp::CabinetPlace(c) => {
                let a = &c.cabinet_place;
                self.solve_cabinet(&a.device, |id| Ask::Drop(id, a.x, a.y))
            }
            SolveOp::CabinetRotate(c) => {
                self.solve_cabinet(&c.cabinet_rotate, |id| Ask::Rotate(vec![id]))
            }
            SolveOp::CabinetRemove(c) => {
                self.solve_cabinet(&c.cabinet_remove, |id| Ask::Remove(vec![id]))
            }
            SolveOp::Cabinet(c) => self.solve_cabinet_preview(c.cabinet),
            _ => return None,
        })
    }

    /// A Cabinet edit of one device, as the plate's own gesture does it.
    fn solve_cabinet(&mut self, tag: &str, ask: impl FnOnce(u64) -> Ask) -> Result<(), String> {
        let (structure, layout, cands) = self
            .cabinet_plate()
            .ok_or("the Cabinet shows no plate (open it first)")?;
        let id = self
            .solve_doc()?
            .project()
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, tag))
            .map(|d| d.id.raw())
            .ok_or_else(|| format!("nothing named `{tag}`"))?;
        self.cabinet_ask(ask(id), Some(structure), layout, &cands);
        Ok(())
    }

    /// Place all (raises the preview) or accepting it, as the plate's buttons do.
    fn solve_cabinet_preview(&mut self, step: CabinetStep) -> Result<(), String> {
        let (structure, layout, cands) = self
            .cabinet_plate()
            .ok_or("the Cabinet shows no plate (open it first)")?;
        match step {
            CabinetStep::PlaceAll => {
                self.cabinet_ask(Ask::PlaceAll, Some(structure), layout, &cands);
                if self.cabinet.preview.is_none() {
                    return Err("Place all found nothing to place".into());
                }
                self.learn_event(novo_learn::Event::Clicked("cabinet.place_all".into()));
            }
            CabinetStep::Accept => {
                if self.cabinet.preview.is_none() {
                    return Err("there is no Place all preview to accept".into());
                }
                self.cabinet_ask(Ask::Accept, Some(structure), None, &[]);
            }
        }
        Ok(())
    }

    fn solve_place(&mut self, a: &novo_learn::lesson::PlaceArgs) -> Result<(), String> {
        let page = self.solve_page(&a.page)?;
        // A step done halfway is not placed twice.
        let there = self.solve_doc()?.project().symbols().any(|s| {
            s.page == page
                && s.symbol.0 == a.symbol
                && (s.placement.origin.x - a.x).abs() < 3.0
                && (s.placement.origin.y - a.y).abs() < 3.0
        });
        if there {
            return Ok(());
        }
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        if !view.editor.begin_place(&a.symbol, a.parent.as_deref()) {
            return Err(view
                .editor
                .notice()
                .unwrap_or("the symbol cannot be placed")
                .to_owned());
        }
        for _ in 0..a.turns % 4 {
            let doc = self.backend.document_mut().ok_or("no project")?;
            let view = self.canvas.schematic().ok_or("no editor canvas")?;
            view.editor.key(doc, EditorKey::Rotate, &[]);
        }
        let result = self.solve_click(page, Point::new(a.x, a.y));
        self.solve_leave();
        result
    }

    /// The replay gate draws a frame here; the live app has nothing to wait for.
    fn settle(&mut self) {
        #[cfg(test)]
        if let Some(frame) = self.learn.settle.clone() {
            frame(self);
        }
    }

    fn solve_doc(&self) -> Result<&Document, String> {
        self.document()
            .ok_or_else(|| "no project is open".to_string())
    }

    pub(super) fn solve_page(&mut self, name: &str) -> Result<PageId, String> {
        let page = self
            .solve_doc()?
            .project()
            .pages_in_order()
            .into_iter()
            .find(|p| p.name == name || p.title == name)
            .map(|p| p.id)
            .ok_or_else(|| format!("no page `{name}`"))?;
        self.show_page(page);
        self.settle();
        Ok(page)
    }

    pub(super) fn solve_pin(&self, text: &str) -> Result<(PageId, Point), String> {
        let project = self.solve_doc()?.project();
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

    pub(super) fn solve_click(&mut self, page: PageId, at: Point) -> Result<(), String> {
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.backend.document_mut().ok_or("no project")?;
        view.editor.click(doc, page, at, TOL);
        let notice = view.editor.notice().map(str::to_owned);
        self.settle();
        match notice {
            Some(n) => Err(n),
            None => Ok(()),
        }
    }

    pub(super) fn solve_leave(&mut self) {
        if let Some(view) = self.canvas.schematic() {
            view.editor.leave_tool();
        }
        self.settle();
    }

    /// A wire from a pin that ends on another wire at a point, as dragging
    /// it there does: the T joins the two.
    pub(super) fn solve_tee(&mut self, from: &str, at: [f64; 2]) -> Result<(), String> {
        let (page, start) = self.solve_pin(from)?;
        self.solve_page_id(page);
        self.solve_leave();
        let doc = self.backend.document().ok_or("no project")?;
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        view.editor.hover(doc, page, start, TOL);
        if !view.editor.begin_drag() {
            return Err(format!("no pin `{from}` to drag from"));
        }
        let result = self.solve_click(page, Point::new(at[0], at[1]));
        self.solve_leave();
        result
    }

    /// A row of feeds dragged out from a point at the pole pitch.
    pub(super) fn solve_feed_row(
        &mut self,
        row: &novo_learn::lesson::FeedRowArgs,
    ) -> Result<(), String> {
        const PITCH: f64 = 8.0;
        let page = self.solve_page(&row.page)?;
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.backend.document().ok_or("no project")?;
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
        let doc = self.backend.document_mut().ok_or("no project")?;
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        let done = view.editor.end_row(doc, page);
        let notice = view.editor.notice().map(str::to_owned);
        self.solve_leave();
        if done == novo_editor::tool::Clicked::Edited {
            Ok(())
        } else {
            Err(notice.unwrap_or_else(|| "the row was not placed".into()))
        }
    }

    /// A cable over the wires that join these pairs of pins, as the cables
    /// drawer's button makes it.
    pub(super) fn solve_cable(&mut self, pairs: &[[String; 2]]) -> Result<(), String> {
        let project = self.solve_doc()?.project();
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
        self.make_cable(&wires);
        self.settle();
        Ok(())
    }

    /// A part with this maker and order number on a device, as the
    /// spreadsheet import or the part picker leaves it.
    pub(super) fn solve_assign_part(
        &mut self,
        a: &novo_learn::lesson::AssignPartArgs,
    ) -> Result<(), String> {
        let project = self.solve_doc()?.project();
        let device = project
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, &a.device))
            .ok_or_else(|| format!("nothing named `{}`", a.device))?
            .id;
        let doc = self.backend.document_mut().ok_or("no project")?;
        let (_, changes) = doc
            .transact("Assign part", |tx| {
                let part = tx.add_part(&a.manufacturer, &a.order_number)?;
                tx.assign_part(device, part)
            })
            .map_err(|e| e.to_string())?;
        self.changed_outside_canvas(&changes);
        self.settle();
        Ok(())
    }

    /// The rename box's Enter: a terminal by designation, an interruption
    /// point by name, or a device by tag.
    pub(super) fn solve_rename(&mut self, target: &str, to: &str) -> Result<(), String> {
        let project = self.solve_doc()?.project();
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
            let doc = self.backend.document_mut().ok_or("no project")?;
            let (result, changes) = doc
                .transact("Rename", |tx| {
                    Ok(novo_editor::edit::rename_label(tx, symbol, to))
                })
                .map_err(|e| e.to_string())?;
            self.changed_outside_canvas(&changes);
            self.settle();
            return result;
        }
        let device = project
            .devices()
            .find(|d| novo_learn::check::tag_matches(&d.tag, target))
            .ok_or_else(|| format!("nothing named `{target}` to rename"))?
            .id;
        let doc = self.backend.document_mut().ok_or("no project")?;
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        let result = view.editor.rename(doc, device, to);
        self.settle();
        result
    }

    pub(super) fn solve_wire(&mut self, ctx: &egui::Context, w: &WireOp) -> Result<(), String> {
        let (page, from) = self.solve_pin(&w.wire[0])?;
        let (_, to) = self.solve_pin(&w.wire[1])?;
        self.solve_page_id(page);
        if let Some(mode) = &w.mode {
            let mode = match mode.as_str() {
                "lines" => WiringMode::Lines,
                _ => WiringMode::Corners,
            };
            if let Some(view) = self.canvas.schematic() {
                view.editor.set_wire_mode(mode);
            }
        }
        self.run_command(ctx, "tool.connect", None);
        self.settle();
        let mut result = self.solve_click(page, from);
        for via in &w.via {
            if result.is_ok() {
                result = self.solve_click(page, Point::new(via[0], via[1]));
            }
        }
        if result.is_ok() {
            result = self.solve_click(page, to);
        }
        self.solve_leave();
        result
    }

    /// Selects a device (all its symbols) or one pin's symbol, as a click
    /// (or Shift+click) on it does.
    pub(super) fn solve_select(&mut self, text: &str, add: bool) -> Result<(), String> {
        // Nothing named: a click on an empty spot of the plate.
        if text.is_empty() {
            self.cabinet.view.selection.clear();
            self.settle();
            return Ok(());
        }
        // In the Cabinet a click lands on the plate.
        if !text.contains(':')
            && matches!(self.workspace(), Workspace::Cabinet | Workspace::View3d)
            && self.solve_select_on_plate(text, add).is_ok()
        {
            return Ok(());
        }
        if let Some((a, b)) = text.split_once('~') {
            return self.solve_select_wire(a.trim(), b.trim(), add);
        }
        let project = self.solve_doc()?.project();
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
            return self.solve_select_on_plate(text, add);
        }
        let first = *found
            .first()
            .ok_or_else(|| format!("nothing named `{text}` to select"))?;
        let page = self
            .solve_doc()?
            .project()
            .symbol(first)
            .map(|s| s.page)
            .ok_or("symbol")?;
        self.solve_page_id(page);
        if !add {
            self.cabinet.view.selection.clear();
        }
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        if !add {
            view.canvas.selection.clear();
        }
        view.canvas.selection.apply(
            found
                .iter()
                .map(|s| novo_canvas::model::item_id(AnyId::Symbol(*s))),
            novo_canvas::SelectOp::Add,
        );
        self.settle();
        Ok(())
    }

    /// Selects the wire drawn between two pins (`select = "-S1:11~-S1:12"`),
    /// as a click on it does.
    fn solve_select_wire(&mut self, a: &str, b: &str, add: bool) -> Result<(), String> {
        let project = self.solve_doc()?.project();
        let from = novo_learn::check::pins_named(project, a);
        let to = novo_learn::check::pins_named(project, b);
        let joins = |w: &novo_core::Wire| {
            let pin = |e: &novo_core::Endpoint| match e {
                novo_core::Endpoint::Pin { symbol, pin } => {
                    Some(novo_core::PinRef::new(*symbol, *pin))
                }
                _ => None,
            };
            let (x, y) = (pin(&w.from), pin(&w.to));
            from.iter().any(|f| {
                to.iter()
                    .any(|t| (x == Some(*f) && y == Some(*t)) || (x == Some(*t) && y == Some(*f)))
            })
        };
        let (id, page) = project
            .wires()
            .find(|w| joins(w))
            .map(|w| (w.id, w.page))
            .ok_or_else(|| format!("no wire between `{a}` and `{b}`"))?;
        self.solve_page_id(page);
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        if !add {
            view.canvas.selection.clear();
        }
        view.canvas.selection.apply(
            [novo_canvas::model::item_id(AnyId::Wire(id))],
            novo_canvas::SelectOp::Add,
        );
        self.settle();
        Ok(())
    }

    /// Selects a device that has no symbol but a place on the plate, as a
    /// click on it in the Cabinet does.
    pub(super) fn solve_select_on_plate(&mut self, text: &str, add: bool) -> Result<(), String> {
        let project = self.solve_doc()?.project();
        let id = project
            .all::<novo_core::CabinetPlacement>()
            .filter_map(|p| project.device(p.device))
            .find(|d| novo_learn::check::tag_matches(&d.tag, text))
            .map(|d| d.id)
            .ok_or_else(|| format!("nothing named `{text}` to select"))?;
        let selection = &mut self.cabinet.view.selection;
        if !add {
            selection.clear();
        }
        selection.apply(
            [novo_canvas::cabinet::box_item(id.raw())],
            novo_canvas::SelectOp::Add,
        );
        self.settle();
        Ok(())
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn solve_symbols(&self, text: &str) -> Result<Vec<SymbolId>, String> {
        let project = self.solve_doc()?.project();
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

    pub(super) fn solve_set_potential(
        &mut self,
        args: &novo_learn::lesson::SetPotentialArgs,
    ) -> Result<(), String> {
        if let Some(name) = &args.page {
            self.solve_page(name)?;
        }
        let page = self.current_page().ok_or("no page")?;
        let at = Point::new(args.feed_at[0], args.feed_at[1]);
        let feed = self
            .solve_doc()?
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
        let menu = novo_editor::potential_menu::feed_menu(
            self.solve_doc()?.project(),
            &[AnyId::Symbol(feed)],
        )
        .ok_or("that symbol is not a potential feed")?;
        let chip = menu
            .chips
            .iter()
            .find(|c| c.name == args.chip)
            .cloned()
            .ok_or_else(|| format!("the menu has no chip `{}`", args.chip))?;
        let view = self.canvas.schematic().ok_or("no editor canvas")?;
        let doc = self.backend.document_mut().ok_or("no project")?;
        if !view.editor.set_feed_potential(doc, &[feed], &chip) {
            return Err(view
                .editor
                .notice()
                .unwrap_or("the chip was refused")
                .to_owned());
        }
        self.settle();
        Ok(())
    }

    pub(super) fn solve_page_id(&mut self, page: PageId) {
        self.show_page(page);
        self.settle();
    }
}
