//! The editing tools as a state machine, independent of any window.
//!
//! The view (or a test) feeds pointer positions, clicks and keys in; the
//! [`Editor`] edits the [`Document`] one gesture per undo step and says what
//! to preview and what the status bar should read. Tools are sticky
//! (DESIGN.md §4): after a placement or a wire the tool stays active, and
//! Escape climbs the ladder: proposals or the step in progress, then the
//! tool, then the selection.

use novo_core::{
    AnyId, ChangeSet, DeviceId, DeviceTag, Direction, Document, Endpoint, PageId, Placement, Point,
    Project, Rotation, SymbolId, SymbolInstance, SymbolKey, SymbolRole, TextAnchor, TextContent,
    TextId, TextPlacement, WireId,
};

use crate::catalog::{Catalog, SymbolClass, classify, model_groups, model_pins};
use crate::edit::{self, PlaceOptions, Transform};
use crate::geometry::{self, EPS, Target, endpoint_direction, route_through, snap, target_at};
use crate::lines::{self, elbow, exact_bends, lock_axis};
use crate::naming::{self, Numbering, parse_tag_input, tag_conflict};
use crate::overlap::Refusal;
use crate::potential::{self, FEED_SYMBOL, PotentialMenu, Suggestion};
use crate::row::{self, RowDrag};

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
    /// Corners: R flipped the elbow of the leg in progress to run along the
    /// other axis first (DESIGN.md §4). Cleared by the next click.
    pub flip: bool,
}

/// How the wire tool turns clicks into a route (DESIGN.md §4, "Wiring
/// modes"). A preference of the person at the keyboard, not of the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WiringMode {
    /// Click where the wire should turn; the lines between the clicks
    /// connect by themselves (EPLAN style).
    #[default]
    Corners,
    /// Each click commits one straight segment locked to an axis; nothing is
    /// routed (AutoCAD style).
    Lines,
}

/// What happens to the wires between facing free pins after a drop
/// (DESIGN.md §2). Joining pins that touch, and splitting a wire a pin lands
/// in, happen in every mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ConnectMode {
    /// Draw them with the drop, as one undo step.
    #[default]
    Draw,
    /// Propose each one; Enter accepts, Delete declines.
    Ask,
    /// Draw and propose nothing.
    Never,
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
    /// Shift: the wires on the moved pins stretch along instead of staying
    /// behind as open ends.
    pub carry: bool,
}

/// A label that is dragged on its own, leaving what it names where it is.
#[derive(Debug, Clone, PartialEq)]
pub enum LabelSource {
    /// A text record: its offset changes.
    Record(TextId),
    /// Text the canvas draws by itself (a symbol's tag, a wire's number):
    /// the first drag gives it a record at the place it is drawn.
    Drawn {
        owner: AnyId,
        content: TextContent,
        /// Where it is drawn now, and which point of its box sits there.
        at: Point,
        anchor: TextAnchor,
    },
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
    /// Shift+M: move the selection with the wires on its pins.
    MoveWithWires,
    /// Delete.
    Delete,
    /// Repeat the last tool when nothing is in progress (the first item of
    /// the context menu on empty canvas; Enter is for proposals).
    Repeat,
    /// Shift+W: switch between Corners and Lines, also mid-wire.
    WireMode,
    /// Backspace: takes back the last corner or segment of the wire in
    /// progress.
    Back,
    /// Enter or a double click while a wire is in progress: finishes on the
    /// last point when it is on a pin or a wire.
    Finish,
    /// A double click while a wire is in progress: the click before it was
    /// the last corner; the wire ends there, on a pin or wire if one is
    /// there and as an open end if not (DESIGN.md §4, "Open ends").
    FinishOpen,
    /// A double click with the Connect tool and no wire in progress: on
    /// empty canvas it starts a wire there, open at that end.
    StartFree,
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
    /// Lines: the stretch of another wire the next line would lie over, to
    /// tint; `wire` is then kept up to the last legal point (DESIGN.md §4).
    pub blocked: Vec<(Point, Point)>,
    /// Corners: the corner or T piece the next click places, with the arms
    /// it would have (DESIGN.md §4 "Wiring modes"). R flips the elbow.
    pub piece: Option<WirePiece>,
}

/// A corner (two arms) or T (three arms) piece of a wire in progress.
#[derive(Debug, Clone, PartialEq)]
pub struct WirePiece {
    pub at: Point,
    pub arms: Vec<Direction>,
}

/// The direction of the axis-parallel stretch from `a` towards `b`.
fn arm(a: Point, b: Point) -> Option<Direction> {
    if a.approx_eq(b, EPS) {
        None
    } else if (a.x - b.x).abs() < EPS {
        Some(if b.y > a.y {
            Direction::Down
        } else {
            Direction::Up
        })
    } else if (a.y - b.y).abs() < EPS {
        Some(if b.x > a.x {
            Direction::Right
        } else {
            Direction::Left
        })
    } else {
        None
    }
}

/// The piece at the last bend of `path` (start, bends, pointer), or the T
/// piece at the pointer when it is on another wire: the branch points back
/// the way the wire came, the other two arms run along the wire joined.
fn pending_piece(
    project: &Project,
    page: PageId,
    path: &[Point],
    on: Option<Target>,
) -> Option<WirePiece> {
    let n = path.len();
    if let Some(Target::Wire(w, at)) = on {
        let branch = arm(at, *path.get(n.checked_sub(2)?)?)?;
        let (_, seg) = geometry::wires_through(project, page, at)
            .into_iter()
            .find(|(id, _)| *id == w)?;
        let pts = geometry::wire_points(project, w)?;
        let (a, b) = (arm(at, pts[seg])?, arm(at, pts[seg + 1])?);
        return Some(WirePiece {
            at,
            arms: vec![branch, a, b],
        });
    }
    if n < 3 {
        return None;
    }
    let (prev, at, next) = (path[n - 3], path[n - 2], path[n - 1]);
    let arms = vec![arm(at, prev)?, arm(at, next)?];
    // Two arms on one line are a straight stretch, not a corner.
    (arms[0].vector() != -arms[1].vector()).then_some(WirePiece { at, arms })
}

/// The pin a target is, if it is one.
fn pin_of(target: Target) -> Option<Endpoint> {
    match target {
        Target::Endpoint(e @ Endpoint::Pin { .. }, _) => Some(e),
        _ => None,
    }
}

/// The reason shown when a line would run back over the wire being drawn.
const SELF_OVERLAP_NOTICE: &str = "A line cannot run back over the wire";

/// What dropping the single selected contact on `target` would link: the
/// contact, the coil it joins and the coil's tag. `None` for any other drop.
fn link_target(
    project: &Project,
    selection: &[AnyId],
    target: AnyId,
) -> Option<(SymbolId, SymbolId, DeviceTag)> {
    let ([AnyId::Symbol(mine)], AnyId::Symbol(coil)) = (selection, target) else {
        return None;
    };
    let (a, b) = (project.symbol(*mine)?, project.symbol(coil)?);
    let (
        SymbolRole::Device { main: false, .. } | SymbolRole::Unassigned,
        SymbolRole::Device { device, main: true },
    ) = (&a.role, &b.role)
    else {
        return None;
    };
    if a.id == b.id || a.device() == Some(*device) {
        return None;
    }
    Some((*mine, coil, project.device(*device)?.tag.clone()))
}

/// What giving `device` the tag `tag` of another device would link: the
/// symbols of `device` and the first symbol of the device that has the tag
/// (its coil, when it has one). `None` when there is nothing to link.
fn link_plan(
    project: &Project,
    device: DeviceId,
    tag: &DeviceTag,
) -> Option<(Vec<SymbolId>, SymbolId)> {
    let other = *project.devices_by_tag(tag).iter().find(|&&d| d != device)?;
    let mine: Vec<SymbolId> = project
        .symbols_of_device(device)
        .iter()
        .map(|s| s.id)
        .collect();
    let partner = project.symbols_of_device(other).first().map(|s| s.id)?;
    (!mine.is_empty()).then_some((mine, partner))
}

/// The reason shown when a line would lie over another wire.
fn overlap_notice(_refusal: &Refusal) -> String {
    "Overlaps another wire".into()
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
    /// Tab holds the highlight: where the pointer was when it did (once
    /// known), until the pointer travels away (DESIGN.md §5).
    held: Option<Option<Point>>,
    /// The pointer, as last given to [`Editor::follow_pointer`].
    pointer: Option<Point>,
    /// The terminal strip in use, once the user placed or chose one.
    pub strip: Option<DeviceId>,
    wire_mode: WiringMode,
    connect: ConnectMode,
    /// The page and pick radius of the last pointer move, for keys that act
    /// at the end of the wire in progress.
    hovered: Option<(PageId, f64)>,
    notice: Option<String>,
    link_hint: Option<String>,
    row: Option<RowDrag>,
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
            held: None,
            pointer: None,
            supply_names: true,
            numbering: Numbering::default(),
            strip: None,
            wire_mode: WiringMode::default(),
            connect: ConnectMode::default(),
            hovered: None,
            notice: None,
            link_hint: None,
            row: None,
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
    /// The change is buffered until [`Editor::take_changes`], as for
    /// [`Editor::rename_page`].
    pub fn new_page(&mut self, document: &mut Document, after: PageId) -> Option<PageId> {
        self.leave_tool();
        match document.transact("New page", |tx| crate::pages::add_page(tx, Some(after))) {
            Ok(((page, note), changes)) => {
                self.changes.merge(&changes);
                self.notice = note;
                Some(page)
            }
            Err(e) => {
                self.notice = Some(e.to_string());
                None
            }
        }
    }

    /// Renames a page as one undo step. A dotted, empty or taken name is
    /// refused with the reason in the notice (DESIGN.md §3).
    ///
    /// Like [`Editor::new_page`], the change is buffered in the editor
    /// until [`Editor::take_changes`]: a caller outside the canvas frame
    /// (the page list, the command field) takes it at once so storage
    /// records the rename in the same frame.
    pub fn rename_page(&mut self, document: &mut Document, page: PageId, name: &str) -> bool {
        let mut refused = None;
        let result = document.transact("Rename page", |tx| {
            refused = crate::pages::rename_page(tx, page, name).err();
            Ok(())
        });
        match (refused, result) {
            (Some(problem), _) => {
                self.notice = Some(problem);
                false
            }
            (None, Ok((_, changes))) => {
                self.changes.merge(&changes);
                self.notice = None;
                true
            }
            (None, Err(e)) => {
                self.notice = Some(e.to_string());
                false
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

    /// What dropping a symbol does about facing free pins.
    pub fn connect_mode(&self) -> ConnectMode {
        self.connect
    }

    /// Chooses the facing-pin behavior (a per-user setting).
    pub fn set_connect_mode(&mut self, mode: ConnectMode) {
        self.connect = mode;
        if mode != ConnectMode::Ask {
            self.clear_proposals();
        }
    }

    /// Draws the facing wires of `symbols` inside the transaction that
    /// dropped them, when the mode is [`ConnectMode::Draw`]. A line that
    /// cannot be drawn is left out, not an error.
    fn draw_facing(&self, tx: &mut novo_core::Tx<'_>, symbols: &[SymbolId]) {
        if !self.auto_connect || self.connect != ConnectMode::Draw {
            return;
        }
        let placed: Vec<SymbolInstance> = symbols
            .iter()
            .filter_map(|&s| tx.project().symbol(s).cloned())
            .collect();
        let found = edit::facing_wires(tx.project(), &placed, &|s| self.outline(s));
        for f in found {
            let _ = edit::accept(tx, &f);
        }
    }

    /// How the wire tool turns clicks into a route.
    pub fn wire_mode(&self) -> WiringMode {
        self.wire_mode
    }

    /// Chooses the wiring mode. The points of a wire in progress stay and
    /// the next click follows the new mode; wires that exist do not change.
    pub fn set_wire_mode(&mut self, mode: WiringMode) {
        if self.wire_mode != mode {
            self.wire_mode = mode;
            if let Tool::Wire(w) = &mut self.tool {
                w.flip = false;
            }
        }
        self.notice = None;
    }

    /// Shift+W.
    pub fn toggle_wire_mode(&mut self) {
        self.set_wire_mode(match self.wire_mode {
            WiringMode::Corners => WiringMode::Lines,
            WiringMode::Lines => WiringMode::Corners,
        });
    }

    /// Starts the wire tool.
    pub fn begin_wire(&mut self) {
        self.set_tool(Tool::Wire(WireTool::default()));
    }

    /// A press with a symbol on the pointer: a drag from here places a row
    /// (DESIGN.md §4, "Placing a row"). `false` when there is nothing to
    /// place in a row (no symbol, or a child symbol of a given device).
    pub fn begin_row(&mut self) -> bool {
        let (Tool::Place(p), Some(at)) = (&self.tool, self.cursor) else {
            return false;
        };
        if p.parent.is_some() {
            return false;
        }
        self.proposals.clear();
        self.row = Some(RowDrag::new(at));
        true
    }

    /// Whether a row is being dragged out.
    pub fn row_active(&self) -> bool {
        self.row.is_some() && matches!(self.tool, Tool::Place(_))
    }

    /// The pointer moved during a row drag.
    pub fn row_follow(&mut self, pointer: Point, hysteresis: f64) {
        if let Some(r) = &mut self.row {
            r.follow(pointer, hysteresis);
        }
    }

    /// `+` or `-` (or the wheel) during a row drag: one grid step wider or
    /// narrower.
    pub fn row_step(&mut self, delta: i32) {
        if let Some(r) = &mut self.row {
            r.extra_steps += delta;
        }
    }

    /// Where the row's symbols go now and the position it stopped before.
    fn row_plan(&self, doc: &Document, page: PageId) -> Option<(Vec<Point>, Option<Point>, f64)> {
        let (Tool::Place(p), Some(r)) = (&self.tool, &self.row) else {
            return None;
        };
        let symbol = self.catalog.get(&p.symbol)?;
        let placement = Placement {
            origin: r.press,
            rotation: p.rotation,
            mirrored: p.mirrored,
        };
        // A row of terminals matches the strip it joins.
        let strip_pitch = match classify(symbol) {
            SymbolClass::Terminal { .. } => edit::adjacent_strip(doc.project(), page, r.press)
                .and_then(|strip| row::strip_pitch(doc.project(), page, strip, r, self.grid)),
            _ => None,
        };
        let pitch = row::pitch(symbol, &placement, r, self.grid, strip_pitch);
        let (at, stopped) = row::positions(
            doc.project(),
            &self.catalog,
            page,
            symbol,
            placement,
            r,
            pitch,
        );
        Some((at, stopped, pitch))
    }

    /// A drag with a 1-pole symbol along its pole axis places one
    /// multi-pole symbol (poles at the strip pitch), not several 1-poles.
    fn pole_plan(&self) -> Option<row::PoleRow> {
        let (Tool::Place(p), Some(r)) = (&self.tool, &self.row) else {
            return None;
        };
        let placement = Placement {
            origin: r.press,
            rotation: p.rotation,
            mirrored: p.mirrored,
        };
        row::pole_row(&p.symbol, &placement, r)
    }

    /// Release: places the whole row as one undo step (a drag that never
    /// got past one pitch places the single symbol).
    pub fn end_row(&mut self, doc: &mut Document, page: PageId) -> Clicked {
        if let (Some(poles), Tool::Place(p)) = (self.pole_plan(), self.tool.clone()) {
            self.row = None;
            let multi = PlaceTool {
                symbol: poles.symbol.to_owned(),
                ..p
            };
            return self.commit_place(doc, page, &multi, poles.origin);
        }
        let Some((at, _, _)) = self.row_plan(doc, page) else {
            self.row = None;
            return Clicked::Unhandled;
        };
        let Tool::Place(p) = self.tool.clone() else {
            self.row = None;
            return Clicked::Unhandled;
        };
        let press = self.row.take().map_or(Point::ZERO, |r| r.press);
        if at.len() <= 1 {
            return self.commit_place(doc, page, &p, at.first().copied().unwrap_or(press));
        }
        let Some(symbol) = self.catalog.get(&p.symbol).cloned() else {
            return Clicked::Refused;
        };
        let options = PlaceOptions {
            parent: None,
            auto_connect: self.auto_connect,
            numbering: self.numbering,
            ..PlaceOptions::default()
        };
        let label = format!("Place {} x{}", symbol.name, at.len());
        let result = doc.transact(label, |tx| {
            let mut options = options.clone();
            let mut ids = Vec::new();
            for &origin in &at {
                let placement = Placement {
                    origin,
                    rotation: p.rotation,
                    mirrored: p.mirrored,
                };
                let id = edit::place(tx, page, &symbol, placement, &options)?;
                // The whole row is one strip, wherever its first terminal went.
                if options.strip.is_none() {
                    options.strip = edit::strip_of_symbol(tx.project(), id);
                }
                ids.push(id);
            }
            self.draw_facing(tx, &ids);
            Ok(ids)
        });
        let ids = result.as_ref().ok().map(|(ids, _)| ids.clone());
        if result.is_ok() {
            self.catalog.used(&p.symbol);
        }
        let r = self.finish(result.map(|(_, c)| c), None);
        if let Some(ids) = ids {
            self.propose(doc, &ids);
        }
        r
    }

    /// Picks up the selection to move it with the pointer.
    pub fn begin_move(&mut self, selection: &[AnyId]) -> bool {
        self.begin_move_with(selection, false)
    }

    /// [`Self::begin_move`], with `carry` taking the wires along (Shift).
    pub fn begin_move_with(&mut self, selection: &[AnyId], carry: bool) -> bool {
        self.link_hint = None;
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
            carry,
        }));
        true
    }

    /// Escape, first rung: cancels the step in progress.
    pub fn cancel_step(&mut self) -> bool {
        // Esc cancels a row drag and keeps the symbol on the pointer.
        if self.row.take().is_some() {
            return true;
        }
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

    /// Escape, second rung: leaves the tool. Proposals go with it.
    pub fn leave_tool(&mut self) -> bool {
        self.clear_proposals();
        if matches!(self.tool, Tool::Select) {
            return false;
        }
        self.tool = Tool::Select;
        self.notice = None;
        true
    }

    /// Another page was opened (or the document replaced): everything that
    /// refers to the old page's objects goes. The tool is left (a move in
    /// hand, a wire in progress, a placement and its proposals), and the
    /// hover target, pointer position, pending selection and notice are
    /// dropped, so the next click on the new page starts from nothing.
    pub fn page_changed(&mut self) {
        self.leave_tool();
        self.target = None;
        self.cursor = None;
        self.select = None;
        self.notice = None;
    }

    /// The pointer moved to `raw` (page millimetres). `tolerance` is the
    /// pick radius in millimetres at the current zoom.
    pub fn hover(&mut self, doc: &Document, page: PageId, raw: Point, tolerance: f64) {
        self.hovered = Some((page, tolerance));
        let project = doc.project();
        self.target = match &self.tool {
            Tool::Wire(_) => target_at(project, page, raw, tolerance),
            // A pin wins over the symbol body near it: dragging from it
            // connects (DESIGN.md §4).
            Tool::Select => target_at(project, page, raw, tolerance)
                .filter(|t| matches!(t, Target::Endpoint(Endpoint::Pin { .. }, _))),
            _ => None,
        };
        // A wire is joined on the grid, not wherever the pointer crosses it.
        if let Some(Target::Wire(w, q)) = self.target {
            let at = geometry::snap_along_wire(project, w, q, self.grid);
            self.target = Some(Target::Wire(w, at));
        }
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
            // A click on a pin starts a wire there, with no tool switch; the
            // next click continues it.
            Tool::Select => match self.target {
                Some(start @ Target::Endpoint(Endpoint::Pin { .. }, _)) => {
                    self.tool = Tool::Wire(WireTool {
                        start: Some(start),
                        corners: Vec::new(),
                        drag: false,
                        flip: false,
                    });
                    self.notice = None;
                    Clicked::Step
                }
                _ => Clicked::Unhandled,
            },
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
                    (Some(start), None) => {
                        let last = w.corners.last().copied().unwrap_or(start.position());
                        match self.wire_mode {
                            WiringMode::Corners => {
                                if w.flip
                                    && let Some(e) = elbow(last, at, true)
                                {
                                    w.corners.push(e);
                                }
                                w.flip = false;
                                w.corners.push(at);
                            }
                            WiringMode::Lines => {
                                let p = lock_axis(last, at);
                                if p.approx_eq(last, EPS) {
                                    self.notice =
                                        Some("Move away from the last point to draw a line".into());
                                    return Clicked::Refused;
                                }
                                let mut so_far = vec![start.position()];
                                so_far.extend(w.corners.iter().copied());
                                so_far.push(p);
                                if lines::route_overlaps_itself(&so_far).is_some() {
                                    self.notice = Some(SELF_OVERLAP_NOTICE.into());
                                    return Clicked::Refused;
                                }
                                let from_pin = (w.corners.is_empty()).then(|| pin_of(start));
                                let check = lines::check_leg(
                                    doc.project(),
                                    page,
                                    last,
                                    p,
                                    from_pin.flatten(),
                                    None,
                                );
                                if let Err(r) = check {
                                    self.notice = Some(overlap_notice(&r));
                                    return Clicked::Refused;
                                }
                                w.corners.push(p);
                            }
                        }
                        self.tool = Tool::Wire(w);
                        self.notice = None;
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
                        // Ask first: a refused final click keeps the wire in
                        // progress instead of discarding every point.
                        if let Err(reason) = self.preflight(doc.project(), page, &w, start, end) {
                            self.notice = Some(reason);
                            if w.drag {
                                self.tool = Tool::Select;
                            }
                            return Clicked::Refused;
                        }
                        self.commit_wire(doc, page, w, start, end)
                    }
                }
            }
            Tool::Move(m) => {
                let delta = at - m.from;
                let grid = self.grid;
                let result = doc.transact("Move", |tx| {
                    if delta != Point::ZERO {
                        let how = Transform::Move(delta);
                        if m.carry {
                            edit::transform_interactive(tx, &m.ids, how)?;
                        } else {
                            edit::transform_detached(tx, &m.ids, how)?;
                        }
                    }
                    let pivot = edit::pivot(tx.project(), &m.ids, grid);
                    if let Some(c) = pivot {
                        for _ in 0..m.turns {
                            edit::transform_interactive(tx, &m.ids, Transform::Rotate(c))?;
                        }
                        if m.mirrored {
                            edit::transform_interactive(tx, &m.ids, Transform::Mirror(c))?;
                        }
                    }
                    if self.auto_connect {
                        edit::settle(tx, &edit::symbols_in(&m.ids))?;
                        self.draw_facing(tx, &edit::symbols_in(&m.ids));
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
            auto_connect: self.auto_connect,
            numbering: self.numbering,
            ..PlaceOptions::default()
        };
        if let Some(parent) = &options.parent {
            let tag = naming::complete_tag(doc.project(), page, parent);
            let project = doc.project();
            let device = project.devices_by_tag(&tag).first().copied();
            match (classify(&symbol), device) {
                (SymbolClass::Child { .. }, None) => {
                    self.notice = Some(format!("No device {tag}; place it first"));
                    return Clicked::Refused;
                }
                // A device has one main function (DESIGN.md: the coil is
                // where its contacts are referenced from).
                (SymbolClass::Main { .. }, Some(device)) => {
                    if let Some(main) =
                        edit::main_symbol(project, device).and_then(|s| project.symbol(s))
                    {
                        let page = project.page(main.page).map_or("", |p| p.name.as_str());
                        self.notice = Some(format!(
                            "{tag} already has its main function on page {page}; \
                             place a contact with `no {tag}`"
                        ));
                        return Clicked::Refused;
                    }
                }
                _ => {}
            }
        }
        let result = doc.transact(format!("Place {}", symbol.name), |tx| {
            let id = edit::place(tx, page, &symbol, placement, &options)?;
            self.draw_facing(tx, &[id]);
            Ok(id)
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
        if !self.auto_connect || self.connect != ConnectMode::Ask {
            return;
        }
        let project = doc.project();
        let placed: Vec<SymbolInstance> = symbols
            .iter()
            .filter_map(|&s| project.symbol(s).cloned())
            .collect();
        self.proposals = edit::facing_wires(project, &placed, &|s| self.outline(s));
        self.proposal = 0;
        self.held = None;
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

    /// The drawn lines of a placed symbol on its page, as segments: what a
    /// proposed wire may not run through. Curves are flattened; text and the
    /// empty space inside the bounding box do not count.
    pub fn outline(&self, sym: &SymbolInstance) -> Option<Vec<(Point, Point)>> {
        use novo_symbols::Graphic;
        let def = self.catalog.get(&sym.symbol.0)?;
        let at = |p: [f64; 2]| sym.placement.apply(Point::new(p[0], p[1]));
        let mut out = Vec::new();
        let mut chain = |pts: Vec<[f64; 2]>, closed: bool| {
            for w in pts.windows(2) {
                out.push((at(w[0]), at(w[1])));
            }
            if closed && pts.len() > 2 {
                out.push((at(pts[pts.len() - 1]), at(pts[0])));
            }
        };
        let curve = |c: [f64; 2], rx: f64, ry: f64, a0: f64, a1: f64| -> Vec<[f64; 2]> {
            const STEPS: usize = 24;
            (0..=STEPS)
                .map(|i| {
                    let a = (a0 + (a1 - a0) * i as f64 / STEPS as f64).to_radians();
                    [c[0] + rx * a.cos(), c[1] + ry * a.sin()]
                })
                .collect()
        };
        for g in &def.graphics {
            match g {
                Graphic::Line { from, to, .. } => chain(vec![*from, *to], false),
                Graphic::Polyline { points, closed, .. } => chain(points.clone(), *closed),
                Graphic::Rect { at: a, size, .. } => chain(
                    vec![
                        *a,
                        [a[0] + size[0], a[1]],
                        [a[0] + size[0], a[1] + size[1]],
                        [a[0], a[1] + size[1]],
                    ],
                    true,
                ),
                Graphic::Circle { center, r, .. } => {
                    chain(curve(*center, *r, *r, 0.0, 360.0), false)
                }
                Graphic::Ellipse { center, rx, ry, .. } => {
                    chain(curve(*center, *rx, *ry, 0.0, 360.0), false);
                }
                Graphic::Arc {
                    center,
                    r,
                    start,
                    end,
                    ..
                } => chain(curve(*center, *r, *r, *start, *end), false),
                Graphic::Text { .. } => {}
            }
        }
        Some(out)
    }

    /// The facing-pin wires the last drop proposes.
    pub fn proposals(&self) -> &[edit::FacingWire] {
        &self.proposals
    }

    /// Drops the proposals of the last drop. They belong to that drop only:
    /// a page change, undo or redo, or any other edit ends them, so Enter
    /// never draws a line the preview no longer shows.
    pub fn clear_proposals(&mut self) {
        self.proposals.clear();
        self.proposal = 0;
        self.held = None;
    }

    /// The highlighted proposal: the one Enter accepts and Delete declines.
    pub fn highlighted(&self) -> Option<usize> {
        (!self.proposals.is_empty()).then(|| self.proposal.min(self.proposals.len() - 1))
    }

    /// The pointer moved to `at`: the highlight follows it to the nearest
    /// proposal, unless Tab holds it there; the hold ends once the pointer
    /// travels more than `hold` (24 px in world units) from where it was
    /// (DESIGN.md §5).
    pub fn follow_pointer(&mut self, at: Point, hold: f64) {
        self.pointer = Some(at);
        match self.held {
            Some(None) => {
                self.held = Some(Some(at));
                return;
            }
            Some(Some(from)) if from.distance(at) <= hold => return,
            _ => self.held = None,
        }
        let nearest = self
            .proposals
            .iter()
            .enumerate()
            .map(|(i, f)| {
                let (q, _) = crate::geometry::closest_on_segment(at, f.a, f.b);
                (i, q.distance(at))
            })
            .min_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((i, _)) = nearest {
            self.proposal = i;
        }
    }

    /// Tab: highlights the next proposal and holds it there.
    pub fn next_proposal(&mut self) -> bool {
        if self.proposals.is_empty() {
            return false;
        }
        self.proposal = (self.proposal + 1) % self.proposals.len();
        self.held = Some(self.pointer);
        true
    }

    /// Whether `at` is on the highlighted proposal, within `tolerance`.
    pub fn on_highlighted(&self, at: Point, tolerance: f64) -> bool {
        self.highlighted().is_some_and(|i| {
            let f = &self.proposals[i];
            let (q, _) = crate::geometry::closest_on_segment(at, f.a, f.b);
            q.distance(at) <= tolerance
        })
    }

    /// Delete on the highlighted proposal: declines it for good, as one
    /// undo step, drawing nothing; it is not proposed again.
    pub fn decline_proposal(&mut self, doc: &mut Document) -> bool {
        let Some(i) = self.highlighted() else {
            return false;
        };
        let proposal = self.proposals.remove(i);
        if self.proposal >= self.proposals.len() {
            self.proposal = 0;
        }
        let result = doc.transact("Decline connection", |tx| edit::decline(tx, &proposal));
        self.finish(result.map(|(_, c)| c), None) == Clicked::Edited
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
        // The pins moved or got a wire since the drop: what was shown is no
        // longer what Enter would draw, so the proposal just goes.
        if !edit::proposal_holds(doc.project(), &proposal) {
            return false;
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
            flip: false,
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

    /// Stores the wire in progress from `start` to `end`: Corners lets the
    /// router join the points, Lines stores them exactly as drawn.
    fn commit_wire(
        &mut self,
        doc: &mut Document,
        page: PageId,
        w: WireTool,
        start: Target,
        end: Target,
    ) -> Clicked {
        let mut corners = w.corners.clone();
        // A drag from a pin is always routed, in either mode (DESIGN.md §4).
        let mode = if w.drag {
            WiringMode::Corners
        } else {
            self.wire_mode
        };
        let result = match mode {
            WiringMode::Corners => {
                if w.flip {
                    let last = corners.last().copied().unwrap_or(start.position());
                    if let Some(e) = elbow(last, end.position(), true) {
                        corners.push(e);
                    }
                }
                doc.transact("Draw wire", |tx| {
                    edit::draw_wire(tx, page, start, &corners, end)
                })
            }
            WiringMode::Lines => doc.transact("Draw wire", |tx| {
                lines::draw_wire_exact(tx, page, start, &corners, end)
            }),
        };
        // A wire that could not be stored leaves the one in progress as it
        // was (a drag, which has nothing to go back to, ends).
        if result.is_ok() {
            self.tool = if w.drag {
                Tool::Select
            } else {
                Tool::Wire(WireTool::default())
            };
        } else if w.drag {
            self.tool = Tool::Select;
        }
        self.finish(result.map(|(_, c)| c), None)
    }

    /// Whether the wire in progress may end at `end`, asked of the same
    /// overlap rule the commit uses, as the reason to show when it may not.
    fn preflight(
        &self,
        project: &Project,
        page: PageId,
        w: &WireTool,
        start: Target,
        end: Target,
    ) -> std::result::Result<(), String> {
        let (a, b) = (start.position(), end.position());
        let lines = self.wire_mode == WiringMode::Lines && !w.drag;
        let corners = if lines {
            let drawn = exact_bends(a, &w.corners, b);
            let mut route = vec![a];
            route.extend(drawn.iter().copied());
            route.push(b);
            if lines::route_overlaps_itself(&route).is_some() {
                return Err(SELF_OVERLAP_NOTICE.into());
            }
            drawn
        } else {
            let mut corners = w.corners.clone();
            if w.flip && !w.drag {
                let last = corners.last().copied().unwrap_or(a);
                corners.extend(elbow(last, b, true));
            }
            corners
        };
        edit::check_draw(project, page, start, &corners, end).map_err(|r| overlap_notice(&r))
    }

    /// A double click with the Connect tool and no wire in progress starts
    /// a wire open at the pointer, on empty canvas only.
    fn start_free(&mut self) -> bool {
        let (Tool::Wire(w), Some(at)) = (&mut self.tool, self.cursor) else {
            return false;
        };
        if w.start.is_some() || self.target.is_some() {
            return false;
        }
        w.start = Some(Target::Endpoint(Endpoint::Free { at }, at));
        self.notice = None;
        true
    }

    /// Enter or a double click: finishes on the last point of the wire in
    /// progress when that point is on a pin or a wire.
    fn finish_wire_here(&mut self, doc: &mut Document, open_ok: bool) -> bool {
        let Tool::Wire(w) = self.tool.clone() else {
            return false;
        };
        let (Some(start), Some((page, tolerance))) = (w.start, self.hovered) else {
            return false;
        };
        let Some(&last) = w.corners.last() else {
            // A double click on a pin that has just started a wire has
            // nothing to finish yet, and nothing to complain about.
            if !open_ok {
                self.notice = Some("Click a pin or wire to finish".into());
            }
            return true;
        };
        let end = match target_at(doc.project(), page, last, tolerance) {
            Some(end) => end,
            // A double click on nothing leaves an open end.
            None if open_ok => Target::Endpoint(Endpoint::Free { at: last }, last),
            None => {
                self.notice = Some("The last point is not on a pin or a wire".into());
                return true;
            }
        };
        let mut rest = w;
        rest.corners.pop();
        if start == end {
            self.notice = Some("Pick a different end for the wire".into());
            return true;
        }
        self.commit_wire(doc, page, rest, start, end);
        true
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
            EditorKey::WireMode => {
                self.toggle_wire_mode();
                true
            }
            EditorKey::Back => match &mut self.tool {
                Tool::Wire(w) if w.start.is_some() && !w.drag => {
                    w.flip = false;
                    if w.corners.pop().is_none() {
                        // Nothing left to take back: the start goes too.
                        *w = WireTool::default();
                    }
                    true
                }
                _ => false,
            },
            EditorKey::Finish => self.finish_wire_here(doc, false),
            EditorKey::FinishOpen => self.finish_wire_here(doc, true),
            EditorKey::StartFree => self.start_free(),
            EditorKey::Rotate | EditorKey::Mirror => {
                let rotate = key == EditorKey::Rotate;
                let corners = self.wire_mode == WiringMode::Corners;
                match &mut self.tool {
                    // R flips the elbow of the leg in progress; in Lines it
                    // does nothing, and it never rotates the wire.
                    Tool::Wire(w) if rotate && w.start.is_some() => {
                        if corners {
                            w.flip = !w.flip;
                        }
                        true
                    }
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
            EditorKey::MoveWithWires => self.begin_move_with(selection, true),
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
        // Rotating, mirroring, moving or deleting changes what the last drop
        // faced.
        self.clear_proposals();
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
            edit::transform_interactive(tx, selection, Transform::Rotate(c))?;
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
            edit::transform_interactive(tx, selection, Transform::Mirror(c))?;
            // No preview here, so only pins dropped inside wires connect.
            if settle {
                edit::settle(tx, &edit::symbols_in(selection))?;
            }
            Ok(())
        })
    }

    /// How far a label dragged from `from` to `to` moves: both ends snap to
    /// the grid, as a moved symbol does.
    pub fn label_delta(&self, from: Point, to: Point) -> Point {
        snap(to, self.grid) - snap(from, self.grid)
    }

    /// Moves one label by `delta`, one undo step. The thing it names stays.
    pub fn move_label(
        &mut self,
        doc: &mut Document,
        page: PageId,
        source: &LabelSource,
        delta: Point,
    ) -> bool {
        if delta == Point::ZERO {
            return false;
        }
        self.run(doc, "Move label", |tx| {
            match source {
                LabelSource::Record(id) => {
                    let mut placement = tx
                        .project()
                        .text(*id)
                        .map(|t| t.placement)
                        .ok_or(novo_core::ModelError::NotFound(AnyId::Text(*id)))?;
                    placement.offset = placement.offset + delta;
                    tx.move_text(*id, placement)
                }
                LabelSource::Drawn {
                    owner,
                    content,
                    at,
                    anchor,
                } => {
                    let id = tx.add_text(
                        page,
                        content.clone(),
                        TextPlacement {
                            anchor: *anchor,
                            ..Default::default()
                        },
                        Some(*owner),
                    )?;
                    // The offset is from the host's own origin, whatever
                    // the host is: read it back instead of knowing it.
                    let origin = tx
                        .project()
                        .text_position(id)
                        .ok_or(novo_core::ModelError::NotFound(AnyId::Text(id)))?;
                    tx.move_text(
                        id,
                        TextPlacement {
                            offset: *at + delta - origin,
                            anchor: *anchor,
                            ..Default::default()
                        },
                    )
                }
            }
        })
    }

    /// Drops a dragged wire part `delta` further on, one undo step.
    pub fn move_wire_part(
        &mut self,
        doc: &mut Document,
        wire: WireId,
        kind: edit::PartKind,
        delta: Point,
    ) -> bool {
        if delta == Point::ZERO {
            return false;
        }
        self.run(doc, "Move wire", |tx| {
            edit::move_wire_part(tx, wire, kind, delta)
        })
    }

    /// How far a wire part dragged from `from` to `to` moves.
    pub fn part_delta(
        &self,
        doc: &Document,
        wire: WireId,
        kind: edit::PartKind,
        from: Point,
        to: Point,
    ) -> Point {
        edit::part_delta(doc.project(), wire, kind, self.grid, from, to)
    }

    /// Removes one corner of a wire's route, one undo step.
    pub fn remove_corner(&mut self, doc: &mut Document, wire: WireId, index: usize) -> bool {
        self.run(doc, "Delete corner", |tx| {
            edit::remove_corner(tx, wire, index)
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
            edit::transform_interactive(tx, selection, Transform::Move(delta))?;
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
            // A contact given its coil's tag joins the coil's device.
            return match link_plan(doc.project(), device, &tag) {
                Some((mine, partner)) => self
                    .check_link(doc.project(), &mine, partner)
                    .map_err(|why| format!("{reason}: {why}"))
                    .and_then(|()| self.link_contacts(doc, &mine, partner, &tag)),
                None => Err(reason),
            };
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

    /// Links each of `mine` to `partner`'s device as one undo step ("Linked
    /// to -K1"), or says in plain words why it cannot, changing nothing.
    pub fn link_contacts(
        &mut self,
        doc: &mut Document,
        mine: &[SymbolId],
        partner: SymbolId,
        tag: &DeviceTag,
    ) -> Result<(), String> {
        self.check_link(doc.project(), mine, partner)?;
        let result = doc.transact(format!("Link to {tag}"), |tx| {
            for &s in mine {
                let old = tx.project().symbol(s).and_then(SymbolInstance::device);
                let command =
                    novo_xref::link_command(tx.project(), self.catalog.library(), s, partner)
                        .map_err(|e| novo_core::ModelError::Refused {
                            what: "link".into(),
                            reason: e.to_string(),
                        })?;
                tx.execute(command)?;
                // The contact's own device is left empty (placing gave it a
                // description, which keeps the link from removing it).
                if let Some(old) = old
                    && tx.project().referrers(old).next().is_none()
                    && tx
                        .project()
                        .device(old)
                        .is_some_and(|d| d.parts.is_empty() && d.properties.is_empty())
                {
                    tx.execute(novo_core::Command::Remove {
                        id: AnyId::Device(old),
                    })?;
                }
            }
            Ok(())
        });
        match result {
            Ok((_, changes)) => {
                self.changes.merge(&changes);
                self.notice = Some(format!("Linked to {tag}"));
                Ok(())
            }
            Err(e) => Err(e.to_string()),
        }
    }

    /// A contact dragged onto a coil (DESIGN.md §4, "row place and link"):
    /// links it to the coil's device instead of dropping it there. `true`
    /// when the drop was a link (done or refused with a notice), `false`
    /// when it is an ordinary drop. Esc has already cancelled nothing here:
    /// the caller ends the move.
    pub fn drop_link(&mut self, doc: &mut Document, selection: &[AnyId], target: AnyId) -> bool {
        let Some((mine, coil, tag)) = link_target(doc.project(), selection, target) else {
            return false;
        };
        if let Err(reason) = self.link_contacts(doc, &[mine], coil, &tag) {
            self.notice = Some(reason);
        }
        if matches!(self.tool, Tool::Move(_)) {
            self.tool = Tool::Select;
        }
        self.link_hint = None;
        true
    }

    /// While a contact is carried over a coil, the hint that dropping links
    /// it ("Link to -K1"); `None` clears it.
    pub fn hint_link(&mut self, project: &Project, selection: &[AnyId], over: Option<AnyId>) {
        self.link_hint = over
            .and_then(|t| link_target(project, selection, t))
            .map(|(_, _, tag)| format!("Link to {tag} · Esc cancels"));
    }

    /// Whether `mine` can be linked to `partner`, as the reason if not.
    fn check_link(
        &self,
        project: &Project,
        mine: &[SymbolId],
        partner: SymbolId,
    ) -> Result<(), String> {
        for &s in mine {
            novo_xref::link_command(project, self.catalog.library(), s, partner)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
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
            return match link_plan(doc.project(), device, &tag) {
                Some((mine, partner)) => {
                    self.check_link(doc.project(), &mine, partner).map(|()| tag)
                }
                None => Err(reason),
            };
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

    /// Right-click a feed, Set potential: one undo step for all `feeds`.
    pub fn set_feed_potential(
        &mut self,
        doc: &mut Document,
        feeds: &[novo_core::SymbolId],
        spec: &potential::PotentialSpec,
    ) -> bool {
        let label = format!("Set potential {}", spec.name);
        self.run(doc, &label, |tx| {
            potential::set_feed_potential(tx, feeds, spec).map(|_| ())
        })
    }

    /// Right-click wires, Override potential: each wire takes `spec` for
    /// itself, or with `None` goes back to what its net carries. One step.
    pub fn override_wire_potential(
        &mut self,
        doc: &mut Document,
        wires: &[WireId],
        spec: Option<&potential::PotentialSpec>,
    ) -> bool {
        let label = match spec {
            Some(s) => format!("Override potential {}", s.name),
            None => "Remove potential override".to_string(),
        };
        self.run(doc, &label, |tx| {
            let id = match spec {
                Some(s) => Some(edit::find_or_add_potential(tx, &s.name, s.kind, s.voltage)?),
                None => None,
            };
            for &w in wires {
                tx.set_wire_potential(w, id)?;
            }
            Ok(())
        })
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

    /// Lines: the points the pointer would add to the wire in progress, up
    /// to the first line that would lie over another wire, which is returned
    /// as the reason. A pin or wire under the pointer ends the wire, with one
    /// right-angle bend when the last point is not lined up with it.
    fn lines_pending(
        &self,
        doc: &Document,
        page: PageId,
        w: &WireTool,
        cursor: Point,
    ) -> (Vec<Point>, Option<(Point, Point)>) {
        let Some(start) = w.start else {
            return (Vec::new(), None);
        };
        let last = w.corners.last().copied().unwrap_or(start.position());
        let points = match self.target {
            Some(_) => {
                let mut v = exact_bends(last, &[], cursor);
                v.push(cursor);
                v
            }
            None => vec![lock_axis(last, cursor)],
        };
        let mut legal = Vec::new();
        let mut so_far = vec![start.position()];
        so_far.extend(w.corners.iter().copied());
        let mut prev = last;
        for (i, &q) in points.iter().enumerate() {
            so_far.push(q);
            if let Some(stretch) = lines::route_overlaps_itself(&so_far) {
                return (legal, Some(stretch));
            }
            let from_pin = (w.corners.is_empty() && i == 0).then(|| pin_of(start));
            let to_pin = (i + 1 == points.len())
                .then(|| self.target.and_then(pin_of))
                .flatten();
            if let Err(r) =
                lines::check_leg(doc.project(), page, prev, q, from_pin.flatten(), to_pin)
            {
                return (legal, Some((r.from, r.to)));
            }
            legal.push(q);
            prev = q;
        }
        (legal, None)
    }

    /// What the next click would do, for the status bar (principle 8).
    pub fn status(&self, doc: &Document, page: PageId) -> String {
        let text = match &self.tool {
            // Quiet until something is in hand (DESIGN.md §3).
            Tool::Select if self.pin_hovered() => "Drag from the pin to connect".into(),
            Tool::Select => String::new(),
            Tool::Place(_) if self.pole_plan().is_some() => {
                let poles = self.pole_plan().map_or(0, |p| p.poles);
                format!(
                    "{poles}-pole · pitch {} mm · R/X turn · Esc cancels",
                    row::POLE_PITCH
                )
            }
            Tool::Place(_) if self.row.is_some() => {
                match self.row_plan(doc, page) {
                    Some((at, stopped, pitch)) => format!(
                        "Row ×{} · pitch {pitch} mm{} · +/- spacing · R/X turn · Esc cancels",
                        at.len(),
                        if stopped.is_some() {
                            " · stops before another device"
                        } else {
                            ""
                        }
                    ),
                    None => String::new(),
                }
            }
            Tool::Place(p) => {
                let name = self
                    .catalog
                    .get(&p.symbol)
                    .map_or(p.symbol.as_str(), |s| s.name.as_str());
                let tag = self.catalog.get(&p.symbol).and_then(|s| match classify(s) {
                    SymbolClass::Main { letter } | SymbolClass::Child { letter } => Some(match &p
                        .parent
                    {
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
            Tool::Wire(w) if self.wire_mode == WiringMode::Lines && !w.drag => {
                let blocked = self.cursor.is_some_and(|c| {
                    self.lines_pending(doc, page, w, c)
                        .1
                        .is_some()
                });
                if blocked {
                    "Overlaps another wire · Backspace takes back the last line".into()
                } else {
                    "Lines: click a pin to finish, empty space to draw a line · Backspace undo · Esc cancels · Shift+W corners"
                        .into()
                }
            }
            Tool::Wire(_) => {
                "Corners: click a pin or wire to finish, empty space for a corner · R flips the bend · Backspace undo · Esc cancels · Shift+W lines"
                    .into()
            }
            Tool::Move(_) => self.link_hint.clone().unwrap_or_else(|| {
                "Click to drop · R rotate · X mirror · Esc cancels the move".into()
            }),
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
                let pole_plan = self.pole_plan();
                let shown = pole_plan.as_ref().map_or(p.symbol.as_str(), |r| r.symbol);
                if let Some(symbol) = self.catalog.get(shown) {
                    let (bridges, pass_through) = model_groups(symbol);
                    let origins = match pole_plan.as_ref() {
                        Some(r) => vec![r.origin],
                        None => match self.row_plan(doc, page) {
                            Some((at, _, _)) if !at.is_empty() => at,
                            _ => vec![cursor],
                        },
                    };
                    for origin in origins {
                        preview.symbols.push(SymbolInstance {
                            id: SymbolId::from(PREVIEW_ID),
                            page,
                            symbol: SymbolKey(symbol.id.clone()),
                            placement: Placement {
                                origin,
                                rotation: p.rotation,
                                mirrored: p.mirrored,
                            },
                            role: SymbolRole::Unassigned,
                            pins: model_pins(symbol),
                            bridges: bridges.clone(),
                            pass_through: pass_through.clone(),
                            feeds: Vec::new(),
                            properties: Default::default(),
                        });
                    }
                }
            }
            Tool::Wire(w) => {
                if let Some(start) = w.start {
                    let a = start.position();
                    preview.wire.push(a);
                    if self.wire_mode == WiringMode::Lines && !w.drag {
                        let (legal, refusal) = self.lines_pending(doc, page, w, cursor);
                        preview.wire.extend(w.corners.iter().copied());
                        preview.wire.extend(legal);
                        preview.blocked.extend(refusal);
                    } else {
                        let from = match start {
                            Target::Endpoint(e, _) => endpoint_direction(project, e),
                            Target::Wire(..) => None,
                        };
                        let to = match self.target {
                            Some(Target::Endpoint(e, _)) => endpoint_direction(project, e),
                            _ => None,
                        };
                        let mut corners = w.corners.clone();
                        if w.flip {
                            let last = corners.last().copied().unwrap_or(a);
                            corners.extend(elbow(last, cursor, true));
                        }
                        let bends = route_through(a, from, &corners, cursor, to);
                        preview.wire.extend(bends);
                        preview.wire.push(cursor);
                        preview.piece = pending_piece(project, page, &preview.wire, self.target);
                    }
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
        } else if matches!(self.tool, Tool::Place(_) | Tool::Move(_))
            && self.auto_connect
            && self.connect != ConnectMode::Never
        {
            preview.proposals = edit::facing_wires(project, &preview.symbols, &|s| self.outline(s))
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
