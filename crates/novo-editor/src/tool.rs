//! The editing tools as a state machine, independent of any window.
//!
//! The view (or a test) feeds pointer positions, clicks and keys in; the
//! [`Editor`] edits the [`Document`] one gesture per undo step and says what
//! to preview and what the status bar should read. Tools are sticky
//! (DESIGN.md §4): after a placement or a wire the tool stays active, and
//! Escape climbs the ladder: proposals or the step in progress, then the
//! tool, then the selection.

use novo_core::{
    AnyId, ChangeSet, DeviceId, DeviceTag, Document, Endpoint, PageId, Placement, Point, Rotation,
    SymbolId, SymbolInstance, SymbolKey, SymbolRole, WireId,
};

use crate::catalog::{Catalog, SymbolClass, classify, model_groups, model_pins};
use crate::edit::{self, PlaceOptions, Transform};
use crate::geometry::{Target, endpoint_direction, route_through, snap, target_at};
use crate::naming::{self, Numbering, parse_tag_input, tag_conflict};
use crate::potential::{self, FEED_SYMBOL, PotentialMenu, Suggestion};

/// Placing one symbol, over and over.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaceTool {
    /// Library id.
    pub symbol: String,
    /// Parent device of a child symbol (`K1 no`).
    pub parent: Option<DeviceTag>,
    pub rotation: Rotation,
    pub mirrored: bool,
}

/// Drawing wires pin to pin.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WireTool {
    /// Where the wire in progress starts.
    pub start: Option<Target>,
    /// Corners clicked in empty space so far.
    pub corners: Vec<Point>,
    /// Started by dragging from a pin (DESIGN.md §4): the release ends the
    /// wire and the editor goes back to selecting.
    pub drag: bool,
}

/// Moving the selection: it follows the pointer until the next click.
#[derive(Debug, Clone, PartialEq)]
pub struct MoveTool {
    pub ids: Vec<AnyId>,
    /// Where the move was picked up (snapped).
    pub from: Point,
    /// Quarter turns and mirroring applied on drop.
    pub turns: u8,
    pub mirrored: bool,
}

/// The active tool.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Tool {
    #[default]
    Select,
    Place(PlaceTool),
    Wire(WireTool),
    Move(MoveTool),
}

impl Tool {
    /// The same tool with nothing in progress, for "repeat last tool".
    fn fresh(&self) -> Tool {
        match self {
            Tool::Wire(_) => Tool::Wire(WireTool::default()),
            Tool::Move(_) | Tool::Select => Tool::Select,
            Tool::Place(p) => Tool::Place(p.clone()),
        }
    }
}

/// Keys the editor understands while the drawing has focus. The view maps
/// physical keys to these (DESIGN.md §4 keys).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKey {
    /// R: rotate what is being placed or moved, else the selection.
    Rotate,
    /// X: mirror.
    Mirror,
    /// W: wire tool.
    Wire,
    /// M: move the selection.
    Move,
    /// Delete.
    Delete,
    /// Repeat the last tool when nothing is in progress (the first item of
    /// the context menu on empty canvas; Enter is for proposals).
    Repeat,
}

/// What a click did.
#[derive(Debug, Clone, PartialEq)]
pub enum Clicked {
    /// The tool did nothing with it; the canvas may select.
    Unhandled,
    /// A step of the tool (a wire start or corner).
    Step,
    /// An edit was committed.
    Edited,
    /// The click was refused; the reason is in [`Editor::notice`].
    Refused,
}

/// What to draw on top of the page while a tool is active.
#[derive(Debug, Clone, Default)]
pub struct Preview {
    /// Symbols following the pointer (placement or move), as they would be.
    pub symbols: Vec<SymbolInstance>,
    /// Model objects being moved, to dim in place.
    pub moving: Vec<AnyId>,
    /// The wire in progress: start, bends and the pointer.
    pub wire: Vec<Point>,
    /// A pin, junction or wire point the next click would connect to.
    pub target: Option<Point>,
    /// Proposed wires between facing free pins ([`edit::facing_wires`]),
    /// drawn dashed: before a drop, what the drop would propose; after it,
    /// the proposals Enter accepts.
    pub proposals: Vec<(Point, Point)>,
    /// The proposal Enter would accept (an index into `proposals`).
    pub current: Option<usize>,
}

/// Id used for preview symbols; never a real model id.
pub const PREVIEW_ID: u64 = u64::MAX - 1;

/// The schematic editor's state for one open page.
pub struct Editor {
    pub catalog: Catalog,
    tool: Tool,
    last: Option<Tool>,
    cursor: Option<Point>,
    target: Option<Target>,
    /// Snap grid for placement, in millimetres (the symbol grid).
    pub grid: f64,
    /// Connect pins that land on wires or face free pins.
    pub auto_connect: bool,
    /// How new devices are numbered (a project rule).
    pub numbering: Numbering,
    /// The rule that recognises PE, N and PEN from connection names
    /// (`-X4:PE`) without any action (DESIGN.md §2). On by default.
    pub supply_names: bool,
    /// Facing-pin wires proposed by the last drop, and the highlighted one.
    proposals: Vec<edit::FacingWire>,
    proposal: usize,
    /// The terminal strip in use, once the user placed or chose one.
    pub strip: Option<DeviceId>,
    notice: Option<String>,
    changes: ChangeSet,
    select: Option<Vec<AnyId>>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor::new(Catalog::builtin())
    }
}

impl Editor {
    pub fn new(catalog: Catalog) -> Self {
        Editor {
            catalog,
            tool: Tool::Select,
            last: None,
            cursor: None,
            target: None,
            grid: novo_symbols::GRID,
            auto_connect: true,
            proposals: Vec::new(),
            proposal: 0,
            supply_names: true,
            numbering: Numbering::default(),
            strip: None,
            notice: None,
            changes: ChangeSet::default(),
            select: None,
        }
    }

    pub fn tool(&self) -> &Tool {
        &self.tool
    }

    /// Whether a tool other than selection is active (the canvas turns its
    /// own selection off then).
    pub fn tool_active(&self) -> bool {
        !matches!(self.tool, Tool::Select)
    }

    /// What repeat-last-tool (the first context menu row on empty canvas)
    /// would bring back.
    pub fn repeat_label(&self) -> Option<String> {
        match self.last.as_ref()? {
            Tool::Place(p) => Some(format!(
                "Place {}",
                self.catalog
                    .get(&p.symbol)
                    .map_or(p.symbol.as_str(), |s| s.name.as_str())
            )),
            Tool::Wire(_) => Some("Wire".into()),
            _ => None,
        }
    }

    /// Whether a step is in progress (a wire started, a move in hand).
    pub fn in_progress(&self) -> bool {
        match &self.tool {
            Tool::Wire(w) => w.start.is_some(),
            Tool::Move(_) => true,
            _ => false,
        }
    }

    /// The last message for the user (a refused edit and why).
    pub fn notice(&self) -> Option<&str> {
        self.notice.as_deref()
    }

    /// What changed since the last call, for the canvas to redraw.
    pub fn take_changes(&mut self) -> ChangeSet {
        std::mem::take(&mut self.changes)
    }

    /// Adds a schematic page after `after`, named and formatted to follow
    /// it (`3` after `2`), as one undo step. Leaves any tool first.
    pub fn new_page(&mut self, document: &mut Document, after: PageId) -> Option<PageId> {
        self.leave_tool();
        match document.transact("New page", |tx| crate::pages::add_page(tx, Some(after))) {
            Ok((page, changes)) => {
                self.changes.merge(&changes);
                self.notice = None;
                Some(page)
            }
            Err(e) => {
                self.notice = Some(e.to_string());
                None
            }
        }
    }

    /// A selection the editor wants shown (the device just placed).
    pub fn take_selection(&mut self) -> Option<Vec<AnyId>> {
        self.select.take()
    }

    /// The pointer position after snapping, in page millimetres.
    pub fn cursor(&self) -> Option<Point> {
        self.cursor
    }

    fn set_tool(&mut self, tool: Tool) {
        if !matches!(tool, Tool::Select) {
            self.last = Some(tool.fresh());
        }
        self.tool = tool;
        self.notice = None;
    }

    /// Starts placing a library symbol; `parent` is the device code typed
    /// before it (`K1` in `K1 no`).
    pub fn begin_place(&mut self, symbol: &str, parent: Option<&str>) -> bool {
        if self.catalog.get(symbol).is_none() {
            self.notice = Some(format!("No symbol {symbol}"));
            return false;
        }
        let parent = match parent.map(|p| naming::parse_tag_input(p, &DeviceTag::default())) {
            None => None,
            Some(Ok(tag)) => Some(tag),
            Some(Err(e)) => {
                self.notice = Some(format!("Parent tag: {e}"));
                return false;
            }
        };
        self.set_tool(Tool::Place(PlaceTool {
            symbol: symbol.into(),
            parent,
            rotation: Rotation::R0,
            mirrored: false,
        }));
        true
    }

    /// Starts the wire tool.
    pub fn begin_wire(&mut self) {
        self.set_tool(Tool::Wire(WireTool::default()));
    }

    /// Picks up the selection to move it with the pointer.
    pub fn begin_move(&mut self, selection: &[AnyId]) -> bool {
        let ids: Vec<AnyId> = selection
            .iter()
            .copied()
            .filter(|id| matches!(id, AnyId::Symbol(_) | AnyId::Junction(_) | AnyId::Wire(_)))
            .collect();
        let Some(from) = self.cursor else {
            return false;
        };
        if ids.is_empty() {
            self.notice = Some("Select something to move first".into());
            return false;
        }
        self.set_tool(Tool::Move(MoveTool {
            ids,
            from,
            turns: 0,
            mirrored: false,
        }));
        true
    }

    /// Escape, first rung: cancels the step in progress.
    pub fn cancel_step(&mut self) -> bool {
        if !self.proposals.is_empty() {
            self.proposals.clear();
            return true;
        }
        match &mut self.tool {
            Tool::Wire(w) if w.start.is_some() => {
                *w = WireTool::default();
                true
            }
            Tool::Move(_) => {
                self.tool = Tool::Select;
                true
            }
            _ => false,
        }
    }

    /// Escape, second rung: leaves the tool.
    pub fn leave_tool(&mut self) -> bool {
        if matches!(self.tool, Tool::Select) {
            return false;
        }
        self.tool = Tool::Select;
        self.notice = None;
        true
    }

    /// The pointer moved to `raw` (page millimetres). `tolerance` is the
    /// pick radius in millimetres at the current zoom.
    pub fn hover(&mut self, doc: &Document, page: PageId, raw: Point, tolerance: f64) {
        let project = doc.project();
        self.target = match &self.tool {
            Tool::Wire(_) => target_at(project, page, raw, tolerance),
            // A pin wins over the symbol body near it: dragging from it
            // connects (DESIGN.md §4).
            Tool::Select => target_at(project, page, raw, tolerance)
                .filter(|t| matches!(t, Target::Endpoint(Endpoint::Pin { .. }, _))),
            _ => None,
        };
        self.cursor = Some(match self.target {
            Some(t) => t.position(),
            None => snap(raw, self.grid),
        });
    }

    /// A primary click at `raw`.
    pub fn click(
        &mut self,
        doc: &mut Document,
        page: PageId,
        raw: Point,
        tolerance: f64,
    ) -> Clicked {
        self.hover(doc, page, raw, tolerance);
        let Some(at) = self.cursor else {
            return Clicked::Unhandled;
        };
        // Clicking elsewhere leaves proposals unaccepted.
        self.proposals.clear();
        match self.tool.clone() {
            Tool::Select => Clicked::Unhandled,
            Tool::Place(p) => self.commit_place(doc, page, &p, at),
            Tool::Wire(mut w) => {
                let target = self.target;
                match (w.start, target) {
                    (None, Some(t)) => {
                        w.start = Some(t);
                        self.tool = Tool::Wire(w);
                        self.notice = None;
                        Clicked::Step
                    }
                    (None, None) => {
                        self.notice = Some("Start a wire on a pin, junction or wire".into());
                        Clicked::Refused
                    }
                    (Some(_), None) if w.drag => {
                        // A drag released on nothing connects nothing.
                        self.tool = Tool::Select;
                        Clicked::Step
                    }
                    (Some(_), None) => {
                        w.corners.push(at);
                        self.tool = Tool::Wire(w);
                        Clicked::Step
                    }
                    (Some(start), Some(end)) => {
                        if start == end {
                            if w.drag {
                                self.tool = Tool::Select;
                                return Clicked::Step;
                            }
                            self.notice = Some("Pick a different end for the wire".into());
                            return Clicked::Refused;
                        }
                        let corners = w.corners.clone();
                        let result = doc.transact("Draw wire", |tx| {
                            edit::draw_wire(tx, page, start, &corners, end)
                        });
                        self.tool = if w.drag {
                            Tool::Select
                        } else {
                            Tool::Wire(WireTool::default())
                        };
                        self.finish(result.map(|(_, c)| c), None)
                    }
                }
            }
            Tool::Move(m) => {
                let delta = at - m.from;
                let grid = self.grid;
                let result = doc.transact("Move", |tx| {
                    if delta != Point::ZERO {
                        edit::transform(tx, &m.ids, Transform::Move(delta))?;
                    }
                    let pivot = edit::pivot(tx.project(), &m.ids, grid);
                    if let Some(c) = pivot {
                        for _ in 0..m.turns {
                            edit::transform(tx, &m.ids, Transform::Rotate(c))?;
                        }
                        if m.mirrored {
                            edit::transform(tx, &m.ids, Transform::Mirror(c))?;
                        }
                    }
                    if self.auto_connect {
                        edit::settle(tx, &edit::symbols_in(&m.ids))?;
                    }
                    Ok(())
                });
                self.tool = Tool::Select;
                let r = self.finish(result.map(|(_, c)| c), None);
                if r == Clicked::Edited {
                    self.propose(doc, &edit::symbols_in(&m.ids));
                }
                self.select = Some(m.ids);
                r
            }
        }
    }

    fn commit_place(
        &mut self,
        doc: &mut Document,
        page: PageId,
        p: &PlaceTool,
        at: Point,
    ) -> Clicked {
        let Some(symbol) = self.catalog.get(&p.symbol).cloned() else {
            return Clicked::Refused;
        };
        let placement = Placement {
            origin: at,
            rotation: p.rotation,
            mirrored: p.mirrored,
        };
        let options = PlaceOptions {
            parent: p.parent.clone(),
            strip: self.strip,
            auto_connect: self.auto_connect,
            numbering: self.numbering,
            ..PlaceOptions::default()
        };
        if let Some(parent) = &options.parent
            && matches!(classify(&symbol), SymbolClass::Child { .. })
        {
            let tag = naming::complete_tag(doc.project(), page, parent);
            if doc.project().devices_by_tag(&tag).is_empty() {
                self.notice = Some(format!("No device {tag}; place it first"));
                return Clicked::Refused;
            }
        }
        let result = doc.transact(format!("Place {}", symbol.name), |tx| {
            edit::place(tx, page, &symbol, placement, &options)
        });
        let placed = result.as_ref().ok().map(|(id, _)| AnyId::Symbol(*id));
        if let Ok((id, _)) = &result {
            self.catalog.used(&p.symbol);
            if let Some(SymbolRole::Terminal { terminals }) =
                doc.project().symbol(*id).map(|s| &s.role)
            {
                self.strip = terminals
                    .first()
                    .and_then(|t| doc.project().terminal(*t))
                    .map(|t| t.strip);
            }
        }
        let r = self.finish(result.map(|(_, c)| c), placed);
        if let Some(AnyId::Symbol(s)) = placed {
            self.propose(doc, &[s]);
        }
        r
    }

    /// Proposes the facing-pin wires of symbols just dropped.
    fn propose(&mut self, doc: &Document, symbols: &[SymbolId]) {
        if !self.auto_connect {
            return;
        }
        let project = doc.project();
        let placed: Vec<SymbolInstance> = symbols
            .iter()
            .filter_map(|&s| project.symbol(s).cloned())
            .collect();
        self.proposals = edit::facing_wires(project, &placed, &|s| self.body(s));
        self.proposal = 0;
    }

    /// The drawn extent of a placed symbol on its page.
    pub fn body(&self, sym: &SymbolInstance) -> Option<(Point, Point)> {
        let (a, b) = self.catalog.get(&sym.symbol.0)?.bounds();
        let corners = [[a[0], a[1]], [b[0], a[1]], [a[0], b[1]], [b[0], b[1]]]
            .map(|c| sym.placement.apply(Point::new(c[0], c[1])));
        let min = corners
            .iter()
            .fold(corners[0], |m, c| Point::new(m.x.min(c.x), m.y.min(c.y)));
        let max = corners
            .iter()
            .fold(corners[0], |m, c| Point::new(m.x.max(c.x), m.y.max(c.y)));
        Some((min, max))
    }

    /// The facing-pin wires the last drop proposes.
    pub fn proposals(&self) -> &[edit::FacingWire] {
        &self.proposals
    }

    /// Tab: highlights the next proposal.
    pub fn next_proposal(&mut self) -> bool {
        if self.proposals.is_empty() {
            return false;
        }
        self.proposal = (self.proposal + 1) % self.proposals.len();
        true
    }

    /// Enter: accepts the highlighted proposal as one undo step.
    pub fn accept_proposal(&mut self, doc: &mut Document) -> bool {
        if self.proposals.is_empty() {
            return false;
        }
        let i = self.proposal.min(self.proposals.len() - 1);
        let proposal = self.proposals.remove(i);
        if self.proposal >= self.proposals.len() {
            self.proposal = 0;
        }
        let result = doc.transact("Connect", |tx| edit::accept(tx, &proposal));
        self.finish(result.map(|(_, c)| c), None) == Clicked::Edited
    }

    /// Starts a wire by dragging from the pin under the pointer (DESIGN.md
    /// §4). Returns whether a pin was there.
    pub fn begin_drag(&mut self) -> bool {
        let Some(start @ Target::Endpoint(Endpoint::Pin { .. }, _)) = self.target else {
            return false;
        };
        if !matches!(self.tool, Tool::Select) {
            return false;
        }
        self.proposals.clear();
        self.tool = Tool::Wire(WireTool {
            start: Some(start),
            corners: Vec::new(),
            drag: true,
        });
        self.notice = None;
        true
    }

    /// Whether the pointer is on a pin a drag would connect from.
    pub fn pin_hovered(&self) -> bool {
        matches!(self.tool, Tool::Select)
            && matches!(self.target, Some(Target::Endpoint(Endpoint::Pin { .. }, _)))
    }

    /// Whether a wire is being dragged from a pin.
    pub fn dragging(&self) -> bool {
        matches!(&self.tool, Tool::Wire(w) if w.drag)
    }

    fn finish(&mut self, result: novo_core::Result<ChangeSet>, select: Option<AnyId>) -> Clicked {
        match result {
            Ok(changes) => {
                self.changes.merge(&changes);
                self.notice = None;
                if let Some(id) = select {
                    self.select = Some(vec![id]);
                }
                Clicked::Edited
            }
            Err(e) => {
                self.notice = Some(e.to_string());
                Clicked::Refused
            }
        }
    }

    /// A key press while the drawing has focus. Returns whether it was used.
    pub fn key(&mut self, doc: &mut Document, key: EditorKey, selection: &[AnyId]) -> bool {
        match key {
            EditorKey::Rotate | EditorKey::Mirror => {
                let rotate = key == EditorKey::Rotate;
                match &mut self.tool {
                    Tool::Place(p) => {
                        if rotate {
                            p.rotation = p.rotation.then(Rotation::R90);
                        } else {
                            p.mirrored = !p.mirrored;
                        }
                        true
                    }
                    Tool::Move(m) => {
                        if rotate {
                            m.turns = (m.turns + 1) % 4;
                        } else {
                            m.mirrored = !m.mirrored;
                        }
                        true
                    }
                    _ if !selection.is_empty() => {
                        if rotate {
                            self.rotate_selection(doc, selection)
                        } else {
                            self.mirror_selection(doc, selection)
                        }
                    }
                    _ => false,
                }
            }
            EditorKey::Wire => {
                self.begin_wire();
                true
            }
            EditorKey::Move => self.begin_move(selection),
            EditorKey::Delete => {
                if selection.is_empty() {
                    return false;
                }
                self.delete(doc, selection)
            }
            EditorKey::Repeat => {
                let idle = match &self.tool {
                    Tool::Select => true,
                    Tool::Wire(w) => w.start.is_none(),
                    _ => false,
                };
                match (&self.last, idle) {
                    (Some(last), true) if matches!(self.tool, Tool::Select) => {
                        self.tool = last.clone();
                        true
                    }
                    _ => false,
                }
            }
        }
    }

    fn run(
        &mut self,
        doc: &mut Document,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<()>,
    ) -> bool {
        let result = doc.transact(label, edit);
        self.finish(result.map(|(_, c)| c), None) == Clicked::Edited
    }

    /// Rotates the selection a quarter turn (R with nothing in hand).
    pub fn rotate_selection(&mut self, doc: &mut Document, selection: &[AnyId]) -> bool {
        let Some(c) = edit::pivot(doc.project(), selection, self.grid) else {
            return false;
        };
        let settle = self.auto_connect;
        self.run(doc, "Rotate", |tx| {
            edit::transform(tx, selection, Transform::Rotate(c))?;
            // No preview here, so only pins dropped inside wires connect.
            if settle {
                edit::settle(tx, &edit::symbols_in(selection))?;
            }
            Ok(())
        })
    }

    /// Mirrors the selection (X with nothing in hand).
    pub fn mirror_selection(&mut self, doc: &mut Document, selection: &[AnyId]) -> bool {
        let Some(c) = edit::pivot(doc.project(), selection, self.grid) else {
            return false;
        };
        let settle = self.auto_connect;
        self.run(doc, "Mirror", |tx| {
            edit::transform(tx, selection, Transform::Mirror(c))?;
            // No preview here, so only pins dropped inside wires connect.
            if settle {
                edit::settle(tx, &edit::symbols_in(selection))?;
            }
            Ok(())
        })
    }

    /// Moves the selection by a distance in one step (arrow keys, scripts).
    pub fn move_selection(
        &mut self,
        doc: &mut Document,
        selection: &[AnyId],
        delta: Point,
    ) -> bool {
        let settle = self.auto_connect;
        self.run(doc, "Move", |tx| {
            edit::transform(tx, selection, Transform::Move(delta))?;
            // No preview here, so only pins dropped inside wires connect.
            if settle {
                edit::settle(tx, &edit::symbols_in(selection))?;
            }
            Ok(())
        })
    }

    /// Deletes the selection; wires on deleted pins go with it.
    pub fn delete(&mut self, doc: &mut Document, selection: &[AnyId]) -> bool {
        let ok = self.run(doc, "Delete", |tx| edit::delete(tx, selection));
        if ok {
            self.select = Some(Vec::new());
        }
        ok
    }

    /// Renames a device from what the user typed (F2). A tag in use or a
    /// malformed one is refused with the reason, and nothing changes.
    pub fn rename(
        &mut self,
        doc: &mut Document,
        device: DeviceId,
        input: &str,
    ) -> Result<(), String> {
        let old = doc
            .project()
            .device(device)
            .map(|d| d.tag.clone())
            .ok_or("The device no longer exists")?;
        let tag = parse_tag_input(input, &old)?;
        if tag == old {
            return Ok(());
        }
        if let Some(reason) = tag_conflict(doc.project(), device, &tag) {
            return Err(reason);
        }
        let result = doc.transact(format!("Rename {old} to {tag}"), |tx| {
            edit::rename(tx, device, tag)
        });
        match result {
            Ok((_, changes)) => {
                self.changes.merge(&changes);
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// Checks what the user is typing as a new tag, for live feedback in
    /// the rename box.
    pub fn check_rename(
        &self,
        doc: &Document,
        device: DeviceId,
        input: &str,
    ) -> Result<DeviceTag, String> {
        let old = doc
            .project()
            .device(device)
            .map(|d| d.tag.clone())
            .ok_or("The device no longer exists")?;
        let tag = parse_tag_input(input, &old)?;
        if tag != old
            && let Some(reason) = tag_conflict(doc.project(), device, &tag)
        {
            return Err(reason);
        }
        Ok(tag)
    }

    /// Right-click potential choices for the selected wires.
    pub fn potential_menu(&self, doc: &Document, selection: &[AnyId]) -> PotentialMenu {
        let wires: Vec<WireId> = selection
            .iter()
            .filter_map(|id| match id {
                AnyId::Wire(w) => Some(*w),
                _ => None,
            })
            .collect();
        potential::menu(doc.project(), &wires, self.supply_names)
    }

    /// Applies a potential suggestion to wires in page order, as one step.
    pub fn assign_potentials(
        &mut self,
        doc: &mut Document,
        wires: &[WireId],
        suggestion: &Suggestion,
    ) -> bool {
        let Some(feed) = self.catalog.get(FEED_SYMBOL).cloned() else {
            self.notice = Some("The symbol library has no potential feed".into());
            return false;
        };
        let label = suggestion.label.clone();
        self.run(doc, &label, |tx| {
            potential::assign(tx, &feed, wires, &suggestion.specs)
        })
    }

    /// What the next click would do, for the status bar (principle 8).
    pub fn status(&self, doc: &Document, page: PageId) -> String {
        let text = match &self.tool {
            // Quiet until something is in hand (DESIGN.md §3).
            Tool::Select if self.pin_hovered() => "Drag from the pin to connect".into(),
            Tool::Select => String::new(),
            Tool::Place(p) => {
                let name = self
                    .catalog
                    .get(&p.symbol)
                    .map_or(p.symbol.as_str(), |s| s.name.as_str());
                let tag = self.catalog.get(&p.symbol).and_then(|s| match classify(s) {
                    SymbolClass::Main { letter } => Some(
                        naming::new_device_tag(doc.project(), page, letter, self.numbering)
                            .to_string(),
                    ),
                    SymbolClass::Child { letter } => Some(match &p.parent {
                        Some(t) => naming::complete_tag(doc.project(), page, t).to_string(),
                        None => naming::new_device_tag(doc.project(), page, letter, self.numbering)
                            .to_string(),
                    }),
                    _ => None,
                });
                match tag {
                    Some(tag) => format!(
                        "Click to place {name} as {tag} · R rotate · X mirror · Esc to stop"
                    ),
                    None => format!("Click to place {name} · R rotate · X mirror · Esc to stop"),
                }
            }
            Tool::Wire(w) if w.drag => "Release on a pin or wire to connect".into(),
            Tool::Wire(w) if w.start.is_none() => {
                "Click a pin, junction or wire to start a wire · Esc to stop".into()
            }
            Tool::Wire(_) => {
                "Click a pin or wire to finish, empty space for a corner · Esc cancels this wire"
                    .into()
            }
            Tool::Move(_) => "Click to drop · R rotate · X mirror · Esc cancels the move".into(),
        };
        let text = if self.proposals.is_empty() {
            text
        } else {
            let n = self.proposals.len();
            let what = if n == 1 {
                "1 connection proposed".to_string()
            } else {
                format!("{n} connections proposed")
            };
            [format!("{what} · ↵ accept · Tab next"), text]
                .into_iter()
                .filter(|t| !t.is_empty())
                .collect::<Vec<_>>()
                .join(" · ")
        };
        match &self.notice {
            Some(n) if text.is_empty() => n.clone(),
            Some(n) => format!("{n} · {text}"),
            None => text,
        }
    }

    /// What to draw over the page for the active tool.
    pub fn preview(&self, doc: &Document, page: PageId) -> Preview {
        let mut preview = Preview {
            target: self.target.map(|t| t.position()),
            ..Preview::default()
        };
        let Some(cursor) = self.cursor else {
            return preview;
        };
        let project = doc.project();
        match &self.tool {
            Tool::Select => {}
            Tool::Place(p) => {
                if let Some(symbol) = self.catalog.get(&p.symbol) {
                    let (bridges, pass_through) = model_groups(symbol);
                    preview.symbols.push(SymbolInstance {
                        id: SymbolId::from(PREVIEW_ID),
                        page,
                        symbol: SymbolKey(symbol.id.clone()),
                        placement: Placement {
                            origin: cursor,
                            rotation: p.rotation,
                            mirrored: p.mirrored,
                        },
                        role: SymbolRole::Unassigned,
                        pins: model_pins(symbol),
                        bridges,
                        pass_through,
                        properties: Default::default(),
                    });
                }
            }
            Tool::Wire(w) => {
                if let Some(start) = w.start {
                    let a = start.position();
                    let from = match start {
                        Target::Endpoint(e, _) => endpoint_direction(project, e),
                        Target::Wire(..) => None,
                    };
                    let to = match self.target {
                        Some(Target::Endpoint(e, _)) => endpoint_direction(project, e),
                        _ => None,
                    };
                    let bends = route_through(a, from, &w.corners, cursor, to);
                    preview.wire.push(a);
                    preview.wire.extend(bends);
                    preview.wire.push(cursor);
                }
            }
            Tool::Move(m) => {
                preview.moving = m.ids.clone();
                preview.symbols = move_ghosts(project, m, cursor, self.grid);
            }
        }
        if !self.proposals.is_empty() {
            preview.proposals = self.proposals.iter().map(|f| (f.a, f.b)).collect();
            preview.current = Some(self.proposal.min(self.proposals.len() - 1));
        } else if matches!(self.tool, Tool::Place(_) | Tool::Move(_)) && self.auto_connect {
            preview.proposals = edit::facing_wires(project, &preview.symbols, &|s| self.body(s))
                .into_iter()
                .map(|f| (f.a, f.b))
                .collect();
        }
        preview
    }
}

/// The moved symbols as they would be with the pointer at `cursor`.
fn move_ghosts(
    project: &novo_core::Project,
    m: &MoveTool,
    cursor: Point,
    grid: f64,
) -> Vec<SymbolInstance> {
    let delta = cursor - m.from;
    let mut ghosts: Vec<SymbolInstance> = m
        .ids
        .iter()
        .filter_map(|id| match id {
            AnyId::Symbol(s) => project.symbol(*s).cloned(),
            _ => None,
        })
        .collect();
    for g in &mut ghosts {
        g.placement.origin = g.placement.origin + delta;
    }
    let pivot = edit::pivot(project, &m.ids, grid).map(|c| c + delta);
    if let Some(c) = pivot {
        for g in &mut ghosts {
            for _ in 0..m.turns {
                g.placement.origin = c + Rotation::R90.apply(g.placement.origin - c);
                g.placement.rotation = g.placement.rotation.then(Rotation::R90);
            }
            if m.mirrored {
                g.placement.origin =
                    Point::new(2.0 * c.x - g.placement.origin.x, g.placement.origin.y);
                g.placement.rotation =
                    Rotation::from_degrees(-i32::from(g.placement.rotation.degrees()))
                        .expect("quarter turn");
                g.placement.mirrored = !g.placement.mirrored;
            }
        }
    }
    ghosts
}
