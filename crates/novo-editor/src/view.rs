//! The schematic editor on screen: a `novo-canvas` canvas showing one page
//! with real symbol artwork, driven by the [`Editor`].
//!
//! The app shell (WS-01) owns the window and gives the central area to
//! [`SchematicView::show`] with the open document and page. Keys follow the
//! DESIGN.md §4 key table and act only while the drawing has the keyboard
//! (principle 10): I insert, W wire, M move, R rotate, X mirror, F2 rename,
//! Delete, Page Up / Page Down, Escape ladder. Enter is kept for accepting
//! proposals (DESIGN.md §4); repeat-last-tool is the first item of the
//! context menu on empty canvas. Ctrl chords (undo, save, palette)
//! stay with the shell.

use egui::{Key, Modifiers};
use novo_canvas::model::{
    self, LabelKind, ModelStyle, PageScene, PlaceholderArt, SheetContent, SymbolArt, SymbolNote,
    item_id,
};
use novo_canvas::widget::{CanvasView, EscapeTarget};
use novo_canvas::{Anchor, Color, Item, ItemId, Label, Primitive, SelectOp, Shape, Stroke, Vec2};
use novo_core::{AnyId, ChangeSet, Command, DeviceId, Document, PageId, Point, SymbolRole, WireId};

use crate::art::LibraryArt;
use crate::catalog::Match;
use crate::edit::device_of;
use crate::potential::PotentialMenu;
use crate::tool::{Editor, EditorKey};

/// Rows shown in the insert search.
const SEARCH_ROWS: usize = 8;
/// Opacity of things that follow the pointer, in the hover colour.
const GHOST_ALPHA: u8 = 200;
/// The hint on the highlighted proposal (DESIGN.md §5).
const PROPOSAL_HINT: &str = "↵ accept · Tab next";
/// Hint text height, in screen points.
const HINT_PX: f32 = 11.0;
/// After Tab the highlight stays put until the pointer travels this far,
/// in screen points (DESIGN.md §5).
const HOLD_PX: f32 = 24.0;
/// A pin wins over the symbol body within this many screen points.
const PIN_RADIUS: f32 = 10.0;
/// A row drag switches axis only when the other distance leads by this
/// many screen points (DESIGN.md §4).
const ROW_HYSTERESIS_PX: f32 = 8.0;
/// First preview item id; previews count down from here.
const PREVIEW_BASE: u64 = u64::MAX - 16;

struct InsertSearch {
    query: String,
    anchor: egui::Pos2,
    selected: usize,
    focus: bool,
}

/// What the rename box edits.
#[derive(Clone, Copy)]
enum RenameTarget {
    Device(DeviceId),
    /// A wire's number; empty text clears it.
    Wire(WireId),
}

struct RenameBox {
    target: RenameTarget,
    text: String,
    anchor: egui::Pos2,
    error: Option<String>,
    focus: bool,
}

struct ContextMenu {
    anchor: egui::Pos2,
    /// Nothing was selected: the menu is for the canvas itself.
    empty: bool,
    potentials: PotentialMenu,
    device: Option<DeviceId>,
    /// Drawn terminals in the selection (terminal actions show for these).
    terminals: usize,
    page: PageId,
    opened: bool,
}

/// One page of the schematic on screen, with the editor's tools.
pub struct SchematicView {
    pub editor: Editor,
    pub canvas: CanvasView,
    art: LibraryArt,
    /// How the page is drawn; the shell sets its theme's look.
    style: ModelStyle,
    scene: Option<PageScene>,
    revision: Option<u64>,
    previews: Vec<ItemId>,
    insert: Option<InsertSearch>,
    rename: Option<RenameBox>,
    menu: Option<ContextMenu>,
    status: Option<String>,
    /// A page the view wants the shell to open (Page Up / Page Down, a new
    /// page).
    goto: Option<PageId>,
    /// Commands the context menu asked the shell to run, by workflow id.
    commands: Vec<&'static str>,
    /// Items a release would select while a selection box is dragged.
    band_count: Option<usize>,
    /// The press that closed a popup, until its release.
    swallow: bool,
    /// The move in progress was picked up by dragging a selected object (no
    /// M key); releasing drops it.
    drag_move: bool,
    /// Where the canvas was drawn last frame, in screen points.
    rect: egui::Rect,
    /// Text the shell draws next to symbols (cross references).
    notes: Vec<SymbolNote>,
    /// `notes` changed since they were last handed to the scene.
    notes_dirty: bool,
    /// What a generated page shows inside its frame (the shell sets it).
    sheet: Option<SheetContent>,
    /// `sheet` changed since it was last handed to the scene.
    sheet_dirty: bool,
}

impl Default for SchematicView {
    fn default() -> Self {
        SchematicView::new(Editor::default())
    }
}

/// Lets the canvas' Escape ladder ask the editor first.
struct Ladder<'a>(&'a mut Editor);

impl EscapeTarget for Ladder<'_> {
    fn cancel_step(&mut self) -> bool {
        self.0.cancel_step()
    }
    fn leave_tool(&mut self) -> bool {
        self.0.leave_tool()
    }
}

fn point(v: Vec2) -> Point {
    Point::new(f64::from(v.x), f64::from(v.y))
}

impl SchematicView {
    pub fn new(editor: Editor) -> Self {
        let style = ModelStyle::default();
        let art = LibraryArt::new(editor.catalog.library(), style.symbol);
        SchematicView {
            editor,
            canvas: CanvasView::new(),
            art,
            style,
            scene: None,
            revision: None,
            previews: Vec::new(),
            insert: None,
            rename: None,
            menu: None,
            status: None,
            goto: None,
            commands: Vec::new(),
            band_count: None,
            swallow: false,
            drag_move: false,
            rect: egui::Rect::NOTHING,
            notes: Vec::new(),
            notes_dirty: false,
            sheet: None,
            sheet_dirty: false,
        }
    }

    /// Text drawn next to symbols on the shown page, such as cross
    /// references. It becomes part of the page scene, so it scales, prints
    /// and hit-tests like the symbols' own tags. Cheap to call every frame.
    pub fn set_notes(&mut self, notes: Vec<SymbolNote>) {
        if notes != self.notes {
            self.notes = notes;
            self.notes_dirty = true;
        }
    }

    /// The content of a generated page (a report sheet) drawn inside the
    /// page frame, or `None` for a schematic page. Cheap to call every
    /// frame; kept until changed, across page rebuilds.
    pub fn set_sheet(&mut self, sheet: Option<SheetContent>) {
        if sheet != self.sheet {
            self.sheet = sheet;
            self.sheet_dirty = true;
        }
    }

    /// The sheet the page scene draws inside the frame, if any.
    pub fn sheet(&self) -> Option<&SheetContent> {
        self.scene.as_ref().and_then(PageScene::sheet)
    }

    /// The shell changed the document outside the view (undo, redo).
    pub fn document_changed(&mut self, document: &Document, changes: &ChangeSet) {
        // Undo and redo move things the proposals were measured against.
        self.editor.clear_proposals();
        if let Some(scene) = &mut self.scene {
            scene.apply(document.project(), changes, &self.art);
            self.revision = Some(document.project().revision());
        }
    }

    /// A different project was opened, created or closed.
    pub fn document_replaced(&mut self) {
        self.scene = None;
        self.revision = None;
        self.previews.clear();
        self.leave_page();
        self.canvas.zoom_to_fit();
    }

    /// Drops everything tied to the page shown so far.
    fn leave_page(&mut self) {
        self.editor.page_changed();
        self.canvas.selection.clear();
        self.insert = None;
        self.rename = None;
        self.menu = None;
    }

    /// Draws the page with `style` (the app's theme) instead of the
    /// default look; the scene is rebuilt on the next frame.
    pub fn set_style(&mut self, style: ModelStyle) {
        self.art = LibraryArt::new(self.editor.catalog.library(), style.symbol);
        self.style = style;
        self.scene = None;
        self.revision = None;
    }

    /// The page the user asked to open (Page Up / Page Down, "New page").
    /// The shell opens it and passes it to the next [`show`](Self::show).
    pub fn take_page_request(&mut self) -> Option<PageId> {
        self.goto.take()
    }

    /// The commands the context menu chose that only the shell can run
    /// (Place component, Go to partner, Zoom to fit), by workflow id.
    pub fn take_commands(&mut self) -> Vec<&'static str> {
        std::mem::take(&mut self.commands)
    }

    /// How many items releasing the selection box being dragged would
    /// select; `None` when no box is being dragged.
    pub fn band_count(&self) -> Option<usize> {
        self.band_count
    }

    /// Whether the in-place rename box is open.
    pub fn renaming(&self) -> bool {
        self.rename.is_some()
    }

    /// The scene being drawn, for queries on what is under a point.
    pub fn scene(&self) -> Option<&PageScene> {
        self.scene.as_ref()
    }

    pub fn zoom_to_fit(&mut self) {
        self.canvas.zoom_to_fit();
    }

    /// What the next click does, for the status bar.
    pub fn status_text(&self) -> Option<String> {
        self.status.clone()
    }

    /// The model object the canvas picks under world point `w`.
    fn model_at(&self, w: Vec2) -> Option<AnyId> {
        let scene = self.scene.as_ref()?;
        let tol = self
            .canvas
            .viewport
            .screen_to_world_len(self.canvas.pick_tolerance);
        let hit = scene.scene().pick(w, tol, self.canvas.viewport.zoom)?;
        scene.model_id(hit.id)
    }

    /// The selection as model ids.
    pub fn selection(&self) -> Vec<AnyId> {
        let Some(scene) = &self.scene else {
            return Vec::new();
        };
        self.canvas
            .selection
            .ids()
            .iter()
            .filter_map(|&i| scene.model_id(i))
            .collect()
    }

    fn select(&mut self, ids: &[AnyId]) {
        self.canvas.selection.clear();
        self.canvas
            .selection
            .apply(ids.iter().map(|&id| item_id(id)), SelectOp::Replace);
    }

    /// Keeps the page scene in step with the document.
    fn sync(&mut self, document: &Document, page: PageId) {
        let project = document.project();
        let stale = match &self.scene {
            Some(s) => s.page() != page || self.revision != Some(project.revision()),
            None => true,
        };
        if stale {
            let fresh = self.scene.as_ref().is_none_or(|s| s.page() != page);
            match &mut self.scene {
                Some(s) if s.page() == page => s.rebuild(project, &self.art),
                _ => {
                    let mut scene = PageScene::new(project, page, &self.art);
                    scene.set_style(self.style, project, &self.art);
                    self.scene = Some(scene);
                }
            }
            self.previews.clear();
            if fresh {
                // A page opened by the shell (page list, Page Up / Page
                // Down) or a new document: the tool, its proposals, the
                // selection and the popups all refer to the old page's
                // objects, so none of them carries over.
                self.leave_page();
                self.canvas.zoom_to_fit();
            }
            self.revision = Some(project.revision());
            self.notes_dirty = true;
            self.sheet_dirty = true;
        }
        if self.sheet_dirty
            && let Some(scene) = &mut self.scene
        {
            scene.set_sheet(project, self.sheet.clone());
            self.sheet_dirty = false;
        }
        if self.notes_dirty
            && let Some(scene) = &mut self.scene
        {
            scene.set_notes(project, self.notes.iter().cloned(), &self.art);
            self.notes_dirty = false;
        }
    }

    fn apply_changes(&mut self, document: &Document) -> ChangeSet {
        let changes = self.editor.take_changes();
        if let Some(scene) = &mut self.scene
            && !changes.is_empty()
        {
            scene.apply(document.project(), &changes, &self.art);
            self.revision = Some(document.project().revision());
        }
        if let Some(ids) = self.editor.take_selection() {
            self.select(&ids);
        }
        changes
    }

    /// Draws the tool preview into the page scene, replacing last frame's.
    fn update_preview(&mut self, document: &Document, page: PageId) {
        let Some(scene) = &mut self.scene else {
            return;
        };
        let layer = scene.layers().symbols;
        for id in self.previews.drain(..) {
            scene.scene_mut().remove(id);
        }
        let preview = self.editor.preview(document, page);
        let ghost_color = opacity(self.canvas.style.hover.color, GHOST_ALPHA);
        let ghost = Stroke::world(0.35, ghost_color);
        let mut items: Vec<Item> = Vec::new();
        for sym in &preview.symbols {
            let local = self
                .art
                .artwork(sym)
                .unwrap_or_else(|| PlaceholderArt::draw(sym, ghost));
            let place = model::placement(&sym.placement);
            let mut item = Item::new(layer).unselectable();
            item.primitives = local
                .iter()
                .map(|p| {
                    let mut p = p.transformed(&place);
                    p.stroke = p.stroke.map(|s| Stroke {
                        color: ghost_color,
                        ..s
                    });
                    p.fill = p.fill.map(|_| ghost_color);
                    p
                })
                .collect();
            items.push(item);
        }
        if preview.wire.len() >= 2 {
            let pts = preview.wire.iter().map(|p| model::vec(*p));
            items.push(
                Item::new(layer)
                    .unselectable()
                    .with(Primitive::stroked(Shape::polyline(pts, false), ghost)),
            );
        }
        if !preview.proposals.is_empty() {
            let zoom = self.canvas.viewport.zoom;
            items.push(proposal_item(
                layer,
                &preview.proposals,
                preview.current,
                zoom,
                self.canvas.style.proposal,
            ));
        }
        if let Some(t) = preview.target {
            items.push(Item::new(layer).unselectable().with(Primitive::stroked(
                Shape::circle(model::vec(t), 1.2),
                Stroke::screen(2.0, self.canvas.style.hover.color),
            )));
        }
        for (i, item) in items.into_iter().enumerate() {
            let id = ItemId(PREVIEW_BASE - i as u64);
            scene.scene_mut().insert(id, item);
            self.previews.push(id);
        }
    }

    fn popup_open(&self) -> bool {
        self.insert.is_some() || self.rename.is_some() || self.menu.is_some()
    }

    /// Draws the page and handles the editing tools. Call once per frame
    /// with the open document and page; edits go through
    /// `Document::transact`, so they share the shell's undo history.
    /// Draws the page and runs the tools. Returns every change the editor
    /// made to `document` this frame, merged, so the shell can record it
    /// (incremental save, autosave) and tell its other views.
    pub fn show(&mut self, ui: &mut egui::Ui, document: &mut Document, page: PageId) -> ChangeSet {
        if document.project().page(page).is_none() {
            self.scene = None;
            return ChangeSet::default();
        }
        self.sync(document, page);
        self.update_preview(document, page);

        // The press that closes a popup only closes it: nothing on the
        // canvas acts on it, through to its release.
        let popup_before = self.popup_open();
        if popup_before && ui.input(|i| i.pointer.any_pressed()) {
            self.swallow = true;
        }
        self.canvas.interaction.select =
            !self.editor.tool_active() && !popup_before && !self.swallow;
        self.canvas.interaction.escape_on_hover = false;
        let Some(scene) = &self.scene else {
            return ChangeSet::default();
        };
        let out = self.canvas.show(ui, scene.scene());
        self.band_count = out.band_count;
        let response = out.response.clone();
        let rect = response.rect;
        self.rect = rect;
        if response.clicked() || response.secondary_clicked() {
            response.request_focus();
        }
        let zoom = self.canvas.viewport.zoom;
        let tolerance = f64::from(self.canvas.pick_tolerance.max(PIN_RADIUS) / zoom);
        // A drag from a pin connects (DESIGN.md §4): the pin under the
        // press, not under the pointer once the drag is recognised.
        if response.drag_started_by(egui::PointerButton::Primary)
            && !self.swallow
            && !self.editor.tool_active()
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let local = o - rect.min;
            let w = self
                .canvas
                .viewport
                .screen_to_world(Vec2::new(local.x, local.y));
            self.editor.hover(document, page, point(w), tolerance);
            if self.editor.begin_drag() {
                self.canvas.selection.clear();
                // The canvas started a selection band on the same press; a
                // wire drag must not also box-select on release.
                self.canvas.escape();
            }
        }
        // A press and drag with a symbol on the pointer places a row
        // (DESIGN.md §4): the first symbol at the press, the rest as far as
        // the pointer stretches.
        if response.drag_started_by(egui::PointerButton::Primary)
            && !self.swallow
            && !self.popup_open()
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
        {
            let local = o - rect.min;
            let w = self
                .canvas
                .viewport
                .screen_to_world(Vec2::new(local.x, local.y));
            self.editor.hover(document, page, point(w), tolerance);
            if self.editor.begin_row() {
                self.canvas.escape();
            }
        }
        if self.editor.row_active()
            && let Some(w) = out.cursor_raw
        {
            self.editor.row_follow(
                point(w),
                f64::from(ROW_HYSTERESIS_PX / self.canvas.viewport.zoom),
            );
            let (more, less) = ui.input(|i| {
                (
                    i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals),
                    i.key_pressed(Key::Minus),
                )
            });
            if more {
                self.editor.row_step(1);
            }
            if less {
                self.editor.row_step(-1);
            }
        }
        if self.editor.row_active() && response.drag_stopped_by(egui::PointerButton::Primary) {
            self.editor.end_row(document, page);
        }
        // Press on an object, drag: it is selected and moves with the
        // pointer, no M needed (DESIGN.md §4); Esc cancels, release drops.
        if response.drag_started_by(egui::PointerButton::Primary)
            && !self.swallow
            && !self.editor.tool_active()
            && !self.editor.dragging()
            && let Some(o) = ui.input(|i| i.pointer.press_origin())
            && let Some(scene) = &self.scene
        {
            let local = o - rect.min;
            let w = self
                .canvas
                .viewport
                .screen_to_world(Vec2::new(local.x, local.y));
            let tol = self
                .canvas
                .viewport
                .screen_to_world_len(self.canvas.pick_tolerance);
            if let Some(hit) = scene.scene().pick(w, tol, self.canvas.viewport.zoom)
                && scene.model_id(hit.id).is_some()
            {
                // Cancel the band the canvas may have begun, then select.
                // A press on a selected object keeps the whole selection.
                let keep: Vec<ItemId> = if self.canvas.selection.contains(hit.id) {
                    self.canvas.selection.ids().to_vec()
                } else {
                    vec![hit.id]
                };
                self.canvas.escape();
                self.canvas.selection.apply(keep, SelectOp::Replace);
                self.editor.hover(document, page, point(w), tolerance);
                let selection = self.selection();
                // Only devices drag; a wire is selected and nothing else
                // (a moved wire would have to keep the no-overlap rule).
                if matches!(scene.model_id(hit.id), Some(AnyId::Symbol(_))) {
                    self.drag_move = self.editor.begin_move(&selection);
                }
            }
        }
        if let Some(w) = out.cursor_raw {
            self.editor.hover(document, page, point(w), tolerance);
            self.editor
                .follow_pointer(point(w), f64::from(HOLD_PX / zoom));
        }
        if self.editor.dragging()
            && response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(w) = out.cursor_raw
        {
            self.editor.click(document, page, point(w), tolerance);
        }
        if self.drag_move
            && let Some(w) = out.cursor_raw
        {
            let over = self.model_at(w);
            let selection = self.selection();
            self.editor.hint_link(document.project(), &selection, over);
        }
        if self.drag_move && response.drag_stopped_by(egui::PointerButton::Primary) {
            self.drag_move = false;
            if matches!(self.editor.tool(), crate::tool::Tool::Move(_))
                && let Some(w) = out.cursor_raw
            {
                // A contact dropped on a coil links to it; anything else
                // is an ordinary drop.
                let target = self.model_at(w);
                let selection = self.selection();
                let linked = target.is_some_and(|t| self.editor.drop_link(document, &selection, t));
                if !linked {
                    self.editor.click(document, page, point(w), tolerance);
                }
            }
        }
        // Clicks drive the active tool; with none, the canvas selects (and
        // the editor drops unaccepted proposals).
        // A double click while a wire is in progress ends it where the
        // first click put the last corner, instead of clicking again.
        let wiring = matches!(
            self.editor.tool(),
            crate::tool::Tool::Wire(w) if w.start.is_some()
        );
        if wiring && !self.swallow && response.double_clicked_by(egui::PointerButton::Primary) {
            let selection = self.selection();
            self.editor.key(document, EditorKey::FinishOpen, &selection);
        } else if !wiring
            && !self.swallow
            && matches!(self.editor.tool(), crate::tool::Tool::Wire(_))
            && response.double_clicked_by(egui::PointerButton::Primary)
        {
            let selection = self.selection();
            self.editor.key(document, EditorKey::StartFree, &selection);
        } else if response.clicked_by(egui::PointerButton::Primary)
            && !self.swallow
            && let Some(w) = out.cursor_raw
        {
            self.editor.click(document, page, point(w), tolerance);
        }
        // Pin under the pointer: the connect cursor.
        if !self.editor.tool_active() && self.editor.pin_hovered() && response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
        }

        // Keys, only while the drawing has the keyboard.
        let ctx = ui.ctx().clone();
        let focus_elsewhere = ctx
            .memory(|m| m.focused())
            .is_some_and(|id| id != response.id);
        let keyboard =
            !focus_elsewhere && !self.popup_open() && (response.has_focus() || response.hovered());
        if keyboard {
            self.handle_keys(&ctx, document, page, &out, rect);
        }

        // Double click on text renames it: a device tag in place, a wire
        // number in place. Other text and bare devices fall through.
        if response.double_clicked_by(egui::PointerButton::Primary)
            && !self.editor.tool_active()
            && !self.popup_open()
            && let (Some(hit), Some(scene)) = (&out.hit, &self.scene)
            && let Some(label) = scene.label_hit(document.project(), hit)
        {
            let anchor = ctx.pointer_latest_pos().unwrap_or(rect.center());
            let start = match (label.kind, label.owner) {
                (LabelKind::DeviceTag, owner) => device_of(document.project(), owner)
                    .and_then(|d| document.project().device(d))
                    .map(|d| (RenameTarget::Device(d.id), d.tag.to_string())),
                (LabelKind::WireNumber, AnyId::Wire(w)) => document.project().wire(w).map(|w| {
                    (
                        RenameTarget::Wire(w.id),
                        w.number.clone().unwrap_or_default(),
                    )
                }),
                _ => None,
            };
            if let Some((target, text)) = start {
                self.rename = Some(RenameBox {
                    target,
                    text,
                    anchor,
                    error: None,
                    focus: true,
                });
            }
        }

        // Double click on a device with nothing in hand opens its
        // properties, the cursor in the tag (DESIGN.md §4); the first click
        // of the pair has selected it. The shell owns the properties.
        if response.double_clicked_by(egui::PointerButton::Primary)
            && !self.editor.tool_active()
            && !self.popup_open()
            && let (Some(hovered), Some(scene)) = (out.hovered, &self.scene)
            && let Some(id) = scene.model_id(hovered)
            && device_of(document.project(), id).is_some()
        {
            self.commands.push("edit.properties");
        }

        // Right click: select what is under the pointer, then the menu.
        if out.context_click.is_some() {
            if let (Some(hovered), Some(scene)) = (out.hovered, &self.scene)
                && !self.canvas.selection.contains(hovered)
                && scene.model_id(hovered).is_some()
            {
                self.canvas.selection.apply([hovered], SelectOp::Replace);
            }
            let selection = self.selection();
            let potentials = self.editor.potential_menu(document, &selection);
            let device = selection
                .iter()
                .find_map(|&id| device_of(document.project(), id));
            let terminals = selection
                .iter()
                .filter(|&&id| match id {
                    AnyId::Symbol(s) => document
                        .project()
                        .symbol(s)
                        .is_some_and(|s| matches!(s.role, SymbolRole::Terminal { .. })),
                    _ => false,
                })
                .count();
            let anchor = ctx.pointer_latest_pos().unwrap_or(rect.center());
            self.menu = Some(ContextMenu {
                anchor,
                empty: selection.is_empty(),
                potentials,
                device,
                terminals,
                page,
                opened: true,
            });
        }

        self.insert_popup(&ctx);
        self.rename_popup(&ctx, document);
        self.context_menu(&ctx, document);

        if self.swallow && !self.popup_open() && ctx.input(|i| !i.pointer.any_down()) {
            self.swallow = false;
        }
        let changes = self.apply_changes(document);
        self.status = Some(self.editor.status(document, page));
        changes
    }

    fn handle_keys(
        &mut self,
        ctx: &egui::Context,
        document: &mut Document,
        page: PageId,
        out: &novo_canvas::widget::CanvasOutput,
        rect: egui::Rect,
    ) {
        // Exactly no modifiers: Shift+R is not R.
        let pressed = |key: Key| ctx.input_mut(|i| consume_plain_key(i, key));
        // Enter accepts the highlighted proposal, Tab moves to the next one
        // and Delete on it declines it (DESIGN.md §4, §5).
        if !self.editor.proposals().is_empty() {
            if pressed(Key::Enter) {
                self.editor.accept_proposal(document);
            } else if pressed(Key::Tab) {
                self.editor.next_proposal();
            } else if out.cursor_raw.is_some_and(|w| {
                // Only with the pointer on it; elsewhere Delete deletes.
                let zoom = self.canvas.viewport.zoom;
                let tolerance = f64::from(self.canvas.pick_tolerance.max(PIN_RADIUS) / zoom);
                self.editor.on_highlighted(point(w), tolerance)
            }) && pressed(Key::Delete)
            {
                self.editor.decline_proposal(document);
            }
        }
        let selection = self.selection();
        // Wiring (DESIGN.md §4): Shift+W switches mode, Backspace takes back
        // the last corner or line, Enter finishes on the last point.
        if consume_shift_key(ctx, Key::W) {
            self.editor.key(document, EditorKey::WireMode, &selection);
        }
        if pressed(Key::Backspace) {
            self.editor.key(document, EditorKey::Back, &selection);
        }
        if self.editor.proposals().is_empty() && pressed(Key::Enter) {
            self.editor.key(document, EditorKey::Finish, &selection);
        }
        for (key, step) in [(Key::PageUp, -1), (Key::PageDown, 1)] {
            if pressed(key)
                && let Some(p) = crate::pages::neighbour(document.project(), page, step)
            {
                self.editor.leave_tool();
                self.goto = Some(p);
            }
        }
        if pressed(Key::Escape) {
            self.canvas.escape_with(Some(&mut Ladder(&mut self.editor)));
        }
        if pressed(Key::I) || pressed(Key::Q) {
            let anchor = ctx
                .pointer_latest_pos()
                .filter(|p| rect.contains(*p))
                .unwrap_or(rect.center());
            self.insert = Some(InsertSearch {
                query: String::new(),
                anchor,
                selected: 0,
                focus: true,
            });
        }
        if pressed(Key::F2) {
            let device = selection
                .iter()
                .find_map(|&id| device_of(document.project(), id));
            if let Some(device) = device
                && let Some(d) = document.project().device(device)
            {
                let anchor = ctx.pointer_latest_pos().unwrap_or(rect.center());
                self.rename = Some(RenameBox {
                    target: RenameTarget::Device(device),
                    text: d.tag.to_string(),
                    anchor,
                    error: None,
                    focus: true,
                });
            }
        }
        for (key, action) in [
            (Key::W, EditorKey::Wire),
            (Key::M, EditorKey::Move),
            (Key::R, EditorKey::Rotate),
            (Key::X, EditorKey::Mirror),
            (Key::Delete, EditorKey::Delete),
        ] {
            if pressed(key) {
                if key == Key::M && out.cursor_raw.is_none() {
                    continue;
                }
                self.editor.key(document, action, &selection);
            }
        }
    }

    fn insert_popup(&mut self, ctx: &egui::Context) {
        let Some(search) = &mut self.insert else {
            return;
        };
        let mut pick: Option<Match> = None;
        let mut close = false;
        // Digits pick a favourite while nothing is typed.
        if search.query.is_empty() {
            for (n, key) in [
                Key::Num1,
                Key::Num2,
                Key::Num3,
                Key::Num4,
                Key::Num5,
                Key::Num6,
                Key::Num7,
                Key::Num8,
                Key::Num9,
            ]
            .into_iter()
            .enumerate()
            {
                if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, key))
                    && let Some(id) = self.editor.catalog.favourite(n + 1)
                {
                    pick = Some(Match {
                        symbol: id.into(),
                        name: String::new(),
                        parent: None,
                        hint: String::new(),
                    });
                }
            }
        }
        let hits: Vec<Match> = self
            .editor
            .catalog
            .search(&search.query)
            .into_iter()
            .take(SEARCH_ROWS)
            .collect();
        let (up, down, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if up {
            search.selected = search.selected.saturating_sub(1);
        }
        if down && search.selected + 1 < hits.len() {
            search.selected += 1;
        }
        search.selected = search.selected.min(hits.len().saturating_sub(1));
        if enter && let Some(hit) = hits.get(search.selected) {
            pick = Some(hit.clone());
        }
        if escape {
            close = true;
        }
        let favourites: Vec<String> = self
            .editor
            .catalog
            .favourites()
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let name = self
                    .editor
                    .catalog
                    .get(id)
                    .map_or(id.as_str(), |s| s.name.as_str());
                format!("{} {}", i + 1, short(name))
            })
            .collect();
        egui::Area::new(egui::Id::new("novo-editor-insert"))
            .order(egui::Order::Foreground)
            .fixed_pos(search.anchor)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(320.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut search.query)
                            .hint_text("Symbol: no, nc, coil, m3, K1 no …")
                            .desired_width(f32::INFINITY),
                    );
                    if search.focus {
                        edit.request_focus();
                        search.focus = false;
                    }
                    if edit.changed() {
                        search.selected = 0;
                    }
                    if search.query.is_empty() {
                        ui.horizontal_wrapped(|ui| {
                            for f in &favourites {
                                ui.weak(f);
                            }
                        });
                    }
                    ui.separator();
                    for (i, hit) in hits.iter().enumerate() {
                        let text = match (&hit.parent, hit.hint.is_empty()) {
                            (Some(p), _) => format!("{}  ·  of {p}", hit.name),
                            (None, false) => format!("{}  ·  {}", hit.name, hit.hint),
                            (None, true) => hit.name.clone(),
                        };
                        if ui.selectable_label(i == search.selected, text).clicked() {
                            pick = Some(hit.clone());
                        }
                    }
                    if hits.is_empty() {
                        ui.weak("No symbol matches");
                    }
                });
            });
        if let Some(hit) = pick {
            self.editor.begin_place(&hit.symbol, hit.parent.as_deref());
            close = true;
        }
        if close {
            self.insert = None;
        }
    }

    fn rename_popup(&mut self, ctx: &egui::Context, document: &mut Document) {
        let Some(rename) = &mut self.rename else {
            return;
        };
        let (enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        let mut close = escape;
        egui::Area::new(egui::Id::new("novo-editor-rename"))
            .order(egui::Order::Foreground)
            .fixed_pos(rename.anchor)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(220.0);
                    let edit = ui.add(
                        egui::TextEdit::singleline(&mut rename.text).desired_width(f32::INFINITY),
                    );
                    if rename.focus {
                        edit.request_focus();
                        rename.focus = false;
                    }
                    if edit.changed()
                        && let RenameTarget::Device(device) = rename.target
                    {
                        rename.error = self
                            .editor
                            .check_rename(document, device, &rename.text)
                            .err();
                    }
                    match &rename.error {
                        Some(e) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                        None => {
                            ui.weak("Enter renames · Esc cancels");
                        }
                    }
                });
            });
        if enter {
            let done = match rename.target {
                RenameTarget::Device(device) => self.editor.rename(document, device, &rename.text),
                RenameTarget::Wire(wire) => {
                    let number = rename.text.trim();
                    let number = (!number.is_empty()).then(|| number.to_string());
                    document
                        .execute(Command::SetWireNumber { wire, number })
                        .map(|_| ())
                        .map_err(|e| e.to_string())
                }
            };
            match done {
                Ok(()) => close = true,
                Err(e) => rename.error = Some(e),
            }
        }
        if close {
            self.rename = None;
        }
    }

    fn context_menu(&mut self, ctx: &egui::Context, document: &mut Document) {
        let Some(menu) = &mut self.menu else {
            return;
        };
        if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
            self.menu = None;
            return;
        }
        let repeat = self.editor.repeat_label();
        let mut chosen: Option<MenuChoice> = None;
        let area = egui::Area::new(egui::Id::new("novo-editor-context"))
            .order(egui::Order::Foreground)
            .fixed_pos(menu.anchor)
            .show(ctx, |ui| {
                egui::Frame::menu(ui.style()).show(ui, |ui| {
                    ui.set_min_width(220.0);
                    // DESIGN.md §4: on an object its own actions come first;
                    // on empty canvas, repeat-last-tool.
                    if menu.empty {
                        if let Some(label) = &repeat
                            && ui.button(label.as_str()).clicked()
                        {
                            chosen = Some(MenuChoice::Repeat);
                        }
                        for (label, id) in [
                            ("Place component  ▸", "tool.place_wheel"),
                            ("Connect   W", "tool.connect"),
                            ("Zoom to fit   Home", "view.fit"),
                        ] {
                            if ui.button(label).clicked() {
                                chosen = Some(MenuChoice::Command(id));
                            }
                        }
                        if ui.button("New page after this").clicked() {
                            chosen = Some(MenuChoice::NewPage);
                        }
                        return;
                    }
                    let p = &menu.potentials;
                    if let Some(carried) = &p.carried {
                        ui.add_enabled(false, egui::Button::new(carried.as_str()));
                    }
                    for (i, s) in p.declare.iter().enumerate() {
                        let text = if i == 0 {
                            egui::RichText::new(&s.label).strong()
                        } else {
                            egui::RichText::new(&s.label)
                        };
                        if ui.button(text).clicked() {
                            chosen = Some(MenuChoice::Potential(i));
                        }
                    }
                    if p.carried.is_some() || !p.declare.is_empty() {
                        ui.separator();
                    }
                    if menu.device.is_some() && ui.button("Rename   F2").clicked() {
                        chosen = Some(MenuChoice::Rename);
                    }
                    if menu.device.is_some() && ui.button("Go to partner   G").clicked() {
                        chosen = Some(MenuChoice::Command("nav.partner"));
                    }
                    if menu.terminals > 0 {
                        if menu.terminals > 1 {
                            if ui.button("Connect terminals").clicked() {
                                chosen = Some(MenuChoice::Command("terminal.connect"));
                            }
                            if ui.button("Disconnect terminals").clicked() {
                                chosen = Some(MenuChoice::Command("terminal.disconnect"));
                            }
                        }
                        if ui.button("Strip number up   Up").clicked() {
                            chosen = Some(MenuChoice::Command("terminal.strip_up"));
                        }
                        if ui.button("Lowest free strip number   Ctrl+Up").clicked() {
                            chosen = Some(MenuChoice::Command("terminal.strip_lowest"));
                        }
                        ui.separator();
                    }
                    for (label, choice) in [
                        ("Move   M", MenuChoice::Key(EditorKey::Move)),
                        ("Rotate   R", MenuChoice::Key(EditorKey::Rotate)),
                        ("Mirror   X", MenuChoice::Key(EditorKey::Mirror)),
                        ("Delete   Del", MenuChoice::Key(EditorKey::Delete)),
                    ] {
                        if ui.button(label).clicked() {
                            chosen = Some(choice);
                        }
                    }
                });
            });
        let clicked_outside = !menu.opened
            && ctx.input(|i| i.pointer.any_pressed())
            && !area.response.contains_pointer();
        menu.opened = false;
        let anchor = menu.anchor;
        let wires = menu.potentials.wires.clone();
        let device = menu.device;
        let page = menu.page;
        let suggestion = |i: usize| menu.potentials.declare.get(i).cloned();
        let choice_suggestion = match chosen {
            Some(MenuChoice::Potential(i)) => suggestion(i),
            _ => None,
        };
        if chosen.is_some() || clicked_outside {
            self.menu = None;
        }
        let selection = self.selection();
        match chosen {
            Some(MenuChoice::Repeat) => {
                self.editor.key(document, EditorKey::Repeat, &selection);
            }
            Some(MenuChoice::Potential(_)) => {
                if let Some(s) = choice_suggestion {
                    self.editor.assign_potentials(document, &wires, &s);
                }
            }
            Some(MenuChoice::Rename) => {
                if let Some(device) = device
                    && let Some(d) = document.project().device(device)
                {
                    self.rename = Some(RenameBox {
                        target: RenameTarget::Device(device),
                        text: d.tag.to_string(),
                        anchor,
                        error: None,
                        focus: true,
                    });
                }
            }
            Some(MenuChoice::Key(k)) => {
                self.editor.key(document, k, &selection);
            }
            Some(MenuChoice::Command(id)) => self.commands.push(id),
            Some(MenuChoice::NewPage) => {
                if let Some(p) = self.editor.new_page(document, page) {
                    self.goto = Some(p);
                }
            }
            None => {}
        }
    }
}

#[derive(Clone, Copy)]
enum MenuChoice {
    Repeat,
    Potential(usize),
    Rename,
    Key(EditorKey),
    NewPage,
    Command(&'static str),
}

/// Consumes a press of `key` with no modifiers at all. egui's
/// `consume_key` ignores extra Shift and Alt, which would make Shift+R
/// rotate.
fn consume_plain_key(input: &mut egui::InputState, key: Key) -> bool {
    let found = input.events.iter().position(|e| {
        matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. }
            if *k == key && modifiers.is_none())
    });
    found.map(|i| input.events.remove(i)).is_some()
}

/// Shift plus `key`, once per press: a held key repeats and is swallowed, so
/// Shift+W never flips the wiring mode back and forth.
fn consume_shift_key(ctx: &egui::Context, key: Key) -> bool {
    ctx.input_mut(|input| {
        let mut pressed = false;
        input.events.retain(|e| match e {
            egui::Event::Key {
                key: k,
                pressed: true,
                repeat,
                modifiers,
                ..
            } if *k == key && modifiers.shift && !modifiers.ctrl && !modifiers.alt => {
                pressed |= !repeat;
                false
            }
            _ => true,
        });
        pressed
    })
}

/// A symbol name cut to a short favourite label.
fn short(name: &str) -> String {
    let s: String = name.chars().take(18).collect();
    if s.len() < name.len() {
        format!("{s}…")
    } else {
        s
    }
}

/// `color` at `alpha`.
fn opacity(color: Color, alpha: u8) -> Color {
    let [r, g, b, _] = color.0;
    Color::rgba(r, g, b, alpha)
}

/// Proposal marks (DESIGN.md §5), in the canvas style's proposal colour:
/// dashed at that strength (about 55%) and 1 px; the highlighted one, which
/// Enter takes, dashed at full strength and 2 px with the
/// "↵ accept · Tab next" hint. Never solid, so none reads as a drawn wire.
fn proposal_item(
    layer: novo_canvas::LayerId,
    proposals: &[(Point, Point)],
    current: Option<usize>,
    zoom: f32,
    color: Color,
) -> Item {
    let mut item = Item::new(layer).unselectable();
    let strong = opacity(color, 255);
    for (i, &(a, b)) in proposals.iter().enumerate() {
        let line = Shape::line(model::vec(a), model::vec(b));
        let stroke = if current == Some(i) {
            let gap = 4.0 / zoom;
            let mid = model::vec(a).lerp(model::vec(b), 0.5);
            item.labels.push(
                Label::new(mid + Vec2::new(gap, -gap), PROPOSAL_HINT, HINT_PX / zoom)
                    .anchored(Anchor::BottomLeft)
                    .colored(strong),
            );
            Stroke::screen(2.0, strong)
        } else {
            Stroke::screen(1.0, color)
        };
        item.primitives.extend(
            crate::art::dashes(&line)
                .into_iter()
                .map(|dash| Primitive::stroked(dash, stroke)),
        );
    }
    item
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposals_are_dashed_and_the_highlighted_one_stronger_with_the_hint() {
        let proposals = [
            (Point::new(0.0, 0.0), Point::new(40.0, 0.0)),
            (Point::new(0.0, 10.0), Point::new(40.0, 10.0)),
        ];
        let color = Color::rgba(70, 110, 160, 140);
        for current in [None, Some(0), Some(1)] {
            let item = proposal_item(novo_canvas::LayerId(0), &proposals, current, 4.0, color);
            for (i, &(a, b)) in proposals.iter().enumerate() {
                let y = a.y as f32;
                let mine: Vec<_> = item
                    .primitives
                    .iter()
                    .filter(|p| match &p.shape {
                        Shape::Line { a, b } => a.y == y && b.y == y,
                        _ => false,
                    })
                    .collect();
                // Never solid: each 40 mm proposal is several dashes.
                assert!(mine.len() > 1, "proposal {i} is not dashed ({current:?})");
                let full = model::vec(a).distance(model::vec(b));
                for p in &mine {
                    let Shape::Line { a, b } = &p.shape else {
                        unreachable!()
                    };
                    assert!(a.distance(*b) < full * 0.5);
                    let s = p.stroke.unwrap();
                    assert_eq!(&s.color.0[..3], &color.0[..3]);
                    assert_eq!(s.unit, novo_canvas::WidthUnit::Screen);
                    if current == Some(i) {
                        assert_eq!(s.color.0[3], 255);
                        assert_eq!(s.width, 2.0);
                    } else {
                        assert_eq!(s.color, color);
                        assert_eq!(s.width, 1.0);
                    }
                }
            }
            // One hint, on the highlighted proposal only, 11 px at any zoom.
            assert_eq!(item.labels.len(), usize::from(current.is_some()));
            if let (Some(i), Some(l)) = (current, item.labels.first()) {
                assert_eq!(l.text, PROPOSAL_HINT);
                assert_eq!(l.height * 4.0, HINT_PX);
                assert!((l.pos.y - proposals[i].0.y as f32).abs() < 2.0);
            }
        }
    }

    use crate::tool::Tool;
    use novo_core::SymbolRole;

    struct Harness {
        ctx: egui::Context,
        view: SchematicView,
        doc: Document,
        page: PageId,
        /// Everything `show` reported, for the shell's save path.
        reported: ChangeSet,
    }

    impl Harness {
        fn new() -> Self {
            let mut doc = Document::default();
            let (page, _) = doc.transact("Add page", |tx| tx.add_page("1")).unwrap();
            let mut h = Harness {
                ctx: egui::Context::default(),
                view: SchematicView::default(),
                doc,
                page,
                reported: ChangeSet::default(),
            };
            h.frame(Vec::new());
            h.frame(Vec::new());
            h
        }

        fn frame(&mut self, events: Vec<egui::Event>) {
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(1200.0, 800.0),
                )),
                events,
                ..Default::default()
            };
            let (view, doc, page) = (&mut self.view, &mut self.doc, self.page);
            let mut changes = ChangeSet::default();
            let mut out = self
                .ctx
                .run_ui(input, |ui| changes = view.show(ui, doc, page));
            self.reported.merge(&changes);
            out.textures_delta.clear();
        }

        fn screen(&self, x: f64, y: f64) -> egui::Pos2 {
            let s = self
                .view
                .canvas
                .viewport
                .world_to_screen(Vec2::new(x as f32, y as f32));
            self.view.rect.min + egui::vec2(s.x, s.y)
        }

        fn key(&mut self, key: Key) {
            self.frame(vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Modifiers::NONE,
            }]);
        }

        fn drag(&mut self, from: (f64, f64), to: (f64, f64)) {
            let a = self.screen(from.0, from.1);
            let b = self.screen(to.0, to.1);
            let button = |pos, pressed| egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![egui::Event::PointerMoved(a)]);
            self.frame(vec![button(a, true)]);
            for i in 1..=8 {
                let t = i as f32 / 8.0;
                self.frame(vec![egui::Event::PointerMoved(a + (b - a) * t)]);
            }
            self.frame(vec![button(b, false)]);
            self.frame(Vec::new());
        }

        fn hover(&mut self, x: f64, y: f64) {
            let pos = self.screen(x, y);
            self.frame(vec![egui::Event::PointerMoved(pos)]);
        }

        fn click(&mut self, x: f64, y: f64, button: egui::PointerButton) {
            let pos = self.screen(x, y);
            self.frame(vec![egui::Event::PointerMoved(pos)]);
            let press = |pressed| egui::Event::PointerButton {
                pos,
                button,
                pressed,
                modifiers: Modifiers::NONE,
            };
            self.frame(vec![press(true)]);
            self.frame(vec![press(false)]);
            self.frame(Vec::new());
        }
    }

    #[test]
    fn insert_search_then_clicks_place_devices() {
        let mut h = Harness::new();
        h.hover(100.0, 100.0);
        h.key(Key::I);
        assert!(h.view.insert.is_some());
        h.frame(vec![egui::Event::Text("coil".into())]);
        assert_eq!(h.view.insert.as_ref().unwrap().query, "coil");
        h.key(Key::Enter);
        assert!(h.view.insert.is_none());
        assert!(h.view.editor.tool_active());
        h.click(100.0, 100.0, egui::PointerButton::Primary);
        h.click(140.0, 100.0, egui::PointerButton::Primary);
        let tags: Vec<String> = h
            .doc
            .project()
            .devices()
            .map(|d| d.tag.to_string())
            .collect();
        assert_eq!(tags, ["-K1", "-K2"]);
        // The shell hears of every edit, so storage can record it.
        for s in h.doc.project().symbols() {
            assert!(h.reported.added.contains(&AnyId::Symbol(s.id)));
        }
        // The scene shows both symbols, and the status bar the next tag.
        let scene = h.view.scene.as_ref().unwrap();
        for s in h.doc.project().symbols() {
            assert!(scene.scene().contains(item_id(s.id)));
        }
        assert!(h.view.status_text().unwrap().contains("-K3"));
        // Escape leaves the tool.
        h.hover(120.0, 120.0);
        h.key(Key::Escape);
        assert!(!h.view.editor.tool_active());
    }

    #[test]
    fn favourite_digit_picks_a_symbol() {
        let mut h = Harness::new();
        h.hover(100.0, 100.0);
        h.key(Key::I);
        h.key(Key::Num2);
        assert!(h.view.insert.is_none());
        h.click(100.0, 100.0, egui::PointerButton::Primary);
        let s = h.doc.project().symbols().next().unwrap();
        assert_eq!(s.symbol.0, "iec/contacts/no");
        assert!(matches!(s.role, SymbolRole::Device { main: false, .. }));
    }

    #[test]
    fn undo_from_the_shell_redraws_the_page() {
        let mut h = Harness::new();
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(100.0, 100.0, egui::PointerButton::Primary);
        let sym = h.doc.project().symbols().next().unwrap().id;
        assert!(
            h.view
                .scene
                .as_ref()
                .unwrap()
                .scene()
                .contains(item_id(sym))
        );
        let changes = h.doc.undo().unwrap().unwrap();
        h.view.document_changed(&h.doc, &changes);
        h.frame(Vec::new());
        assert!(
            !h.view
                .scene
                .as_ref()
                .unwrap()
                .scene()
                .contains(item_id(sym))
        );
    }

    #[test]
    fn f2_renames_in_place_and_refuses_a_used_tag() {
        let mut h = Harness::new();
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(100.0, 100.0, egui::PointerButton::Primary);
        h.click(140.0, 100.0, egui::PointerButton::Primary);
        h.hover(150.0, 150.0);
        h.key(Key::Escape);
        // The last placed device is selected; F2 opens the rename box.
        h.key(Key::F2);
        let rename = h.view.rename.as_mut().expect("rename box");
        assert_eq!(rename.text, "-K2");
        rename.text = "-K1".into();
        h.key(Key::Enter);
        assert!(
            h.view
                .rename
                .as_ref()
                .unwrap()
                .error
                .as_ref()
                .unwrap()
                .contains("already used")
        );
        h.view.rename.as_mut().unwrap().text = "-K7".into();
        h.key(Key::Enter);
        assert!(h.view.rename.is_none());
        let mut tags: Vec<String> = h
            .doc
            .project()
            .devices()
            .map(|d| d.tag.to_string())
            .collect();
        tags.sort();
        assert_eq!(tags, ["-K1", "-K7"]);
    }

    #[test]
    fn right_click_on_three_wires_offers_three_phase_first() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/protection/fuse_3p", None);
        h.click(40.0, 40.0, egui::PointerButton::Primary);
        h.view.editor.begin_place("iec/contactors/main_3p", None);
        h.click(40.0, 120.0, egui::PointerButton::Primary);
        h.view.editor.begin_wire();
        for x in [40.0, 48.0, 56.0] {
            h.click(x, 56.0, egui::PointerButton::Primary);
            h.click(x, 120.0, egui::PointerButton::Primary);
        }
        h.view.editor.leave_tool();
        assert_eq!(h.doc.project().wires().count(), 3);
        let wires: Vec<AnyId> = h.doc.project().wires().map(|w| AnyId::Wire(w.id)).collect();
        h.view.select(&wires);
        h.click(48.0, 88.0, egui::PointerButton::Secondary);
        let menu = h.view.menu.as_ref().expect("context menu");
        assert_eq!(
            menu.potentials.declare[0].label,
            "Declare source: 3~ 400 V (L1, L2, L3)"
        );
        assert!(!menu.empty);
    }

    /// Places a fuse and a contactor facing it at `x`, which leaves
    /// proposals.
    fn with_proposals(h: &mut Harness, x: f64) {
        h.view.editor.begin_place("iec/protection/fuse_3p", None);
        h.click(x, 40.0, egui::PointerButton::Primary);
        h.view.editor.begin_place("iec/contactors/main_3p", None);
        h.click(x, 120.0, egui::PointerButton::Primary);
        assert_eq!(h.view.editor.proposals().len(), 3);
        assert_eq!(h.doc.project().wires().count(), 0);
    }

    #[test]
    fn the_highlight_follows_the_pointer_tab_holds_it_and_delete_declines() {
        let mut h = Harness::new();
        with_proposals(&mut h, 40.0);
        let mid = |h: &Harness, i: usize| {
            let f = &h.view.editor.proposals()[i];
            ((f.a.x + f.b.x) / 2.0, (f.a.y + f.b.y) / 2.0)
        };
        let px = 1.0 / f64::from(h.view.canvas.viewport.zoom);
        // The pointer picks the nearest proposal.
        let (x, y) = mid(&h, 2);
        h.hover(x, y);
        assert_eq!(h.view.editor.highlighted(), Some(2));
        // Tab moves on and holds there while the pointer stays near...
        h.key(Key::Tab);
        assert_eq!(h.view.editor.highlighted(), Some(0));
        h.hover(x, y + 10.0 * px);
        assert_eq!(h.view.editor.highlighted(), Some(0));
        // ...and lets go once it travels more than 24 px.
        h.hover(x, y + 30.0 * px);
        assert_eq!(h.view.editor.highlighted(), Some(2));
        // Delete on the highlighted one declines it and draws nothing.
        h.key(Key::Delete);
        assert_eq!(h.view.editor.proposals().len(), 2);
        assert_eq!(h.doc.project().wires().count(), 0);
        assert!(h.doc.project().symbols().count() == 2);
        // Enter takes the highlighted one, the one under the pointer.
        let (x, y) = mid(&h, 1);
        h.hover(x, y);
        let target = h.view.editor.proposals()[1];
        h.key(Key::Enter);
        assert_eq!(h.doc.project().wires().count(), 1);
        let wire = h.doc.project().wires().next().unwrap();
        let ends = [wire.from, wire.to];
        assert!(ends.contains(&target.from) && ends.contains(&target.to));
        // Away from the highlighted proposal Delete is the usual delete: it
        // removes the selection (the contactor just placed), not a proposal.
        let selected = h.view.selection();
        assert!(!selected.is_empty());
        h.hover(x + 60.0 * px, y);
        let before = h.doc.project().symbols().count();
        h.key(Key::Delete);
        assert_eq!(h.doc.project().symbols().count(), before - 1);
    }

    #[test]
    fn a_page_change_drops_the_proposals() {
        let mut h = Harness::new();
        let first = h.page;
        let page = h.view.editor.new_page(&mut h.doc, first).unwrap();
        with_proposals(&mut h, 40.0);
        h.hover(150.0, 150.0);
        h.key(Key::PageDown);
        assert_eq!(h.view.take_page_request(), Some(page));
        assert!(h.view.editor.proposals().is_empty());
        h.page = page;
        h.frame(Vec::new());
        h.key(Key::Enter);
        assert_eq!(h.doc.project().wires().count(), 0);
        // A page the shell opens some other way (the page list) does too.
        h.page = first;
        h.frame(Vec::new());
        with_proposals(&mut h, 120.0);
        h.page = page;
        h.frame(Vec::new());
        assert!(h.view.editor.proposals().is_empty());
        h.key(Key::Enter);
        assert_eq!(h.doc.project().wires().count(), 0);
    }

    #[test]
    fn undo_drops_the_proposals() {
        let mut h = Harness::new();
        with_proposals(&mut h, 40.0);
        let changes = h.doc.undo().unwrap().unwrap();
        h.view.document_changed(&h.doc, &changes);
        assert!(h.view.editor.proposals().is_empty());
        let changes = h.doc.redo().unwrap().unwrap();
        h.view.document_changed(&h.doc, &changes);
        h.frame(Vec::new());
        h.key(Key::Enter);
        assert_eq!(h.doc.project().wires().count(), 0);
    }

    #[test]
    fn new_page_then_page_up_and_down() {
        let mut h = Harness::new();
        let first = h.page;
        h.click(100.0, 100.0, egui::PointerButton::Secondary);
        assert_eq!(h.view.menu.as_ref().map(|m| m.page), Some(first));
        h.view.menu = None;
        let page = h.view.editor.new_page(&mut h.doc, first).unwrap();
        assert_eq!(h.doc.project().page(page).unwrap().name, "2");
        h.hover(100.0, 100.0);
        h.key(Key::PageDown);
        assert_eq!(h.view.take_page_request(), Some(page));
        h.page = page;
        h.frame(Vec::new());
        h.hover(100.0, 100.0);
        h.key(Key::PageUp);
        assert_eq!(h.view.take_page_request(), Some(first));
        h.page = first;
        h.frame(Vec::new());
        h.key(Key::PageUp);
        assert_eq!(h.view.take_page_request(), None);
    }

    #[test]
    fn enter_does_not_repeat_and_empty_canvas_menu_offers_repeat() {
        let mut h = Harness::new();
        h.view.editor.begin_wire();
        h.hover(100.0, 100.0);
        h.key(Key::Escape);
        assert!(!h.view.editor.tool_active());
        // Enter is for proposals, not repeat-last-tool.
        h.key(Key::Enter);
        assert!(!h.view.editor.tool_active());
        h.click(100.0, 100.0, egui::PointerButton::Secondary);
        let menu = h.view.menu.as_ref().expect("context menu");
        assert!(menu.empty);
        assert!(h.view.editor.repeat_label().is_some());
    }

    fn pin_at(doc: &Document, origin: Point, name: &str) -> Point {
        let s = doc
            .project()
            .symbols()
            .find(|s| s.placement.origin == origin)
            .unwrap();
        s.pin_position(s.pin_index(name).unwrap()).unwrap()
    }

    #[test]
    fn a_drag_from_a_pin_connects_without_w() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(60.0, 60.0, egui::PointerButton::Primary);
        h.click(140.0, 120.0, egui::PointerButton::Primary);
        h.view.editor.leave_tool();
        let a = pin_at(&h.doc, Point::new(60.0, 60.0), "A2");
        let b = pin_at(&h.doc, Point::new(140.0, 120.0), "A1");
        // Hovering the pin shows it can be dragged from.
        h.hover(a.x, a.y);
        assert!(h.view.editor.pin_hovered());
        h.drag((a.x, a.y), (b.x, b.y));
        assert_eq!(h.doc.project().wires().count(), 1);
        assert!(!h.view.editor.tool_active());
    }

    #[test]
    fn a_press_just_off_a_pin_starts_a_wire_and_does_not_box_select() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(60.0, 60.0, egui::PointerButton::Primary);
        h.click(140.0, 120.0, egui::PointerButton::Primary);
        h.view.editor.leave_tool();
        let a = pin_at(&h.doc, Point::new(60.0, 60.0), "A2");
        let b = pin_at(&h.doc, Point::new(140.0, 120.0), "A1");
        // Within the pin target but outside the old, smaller one.
        let off = f64::from(PIN_RADIUS - 1.0) / f64::from(h.view.canvas.viewport.zoom);
        h.drag((a.x - off, a.y), (b.x, b.y));
        assert_eq!(h.doc.project().wires().count(), 1);
        assert!(h.view.canvas.selection.is_empty());
    }

    #[test]
    fn dragging_an_object_moves_it_without_m() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(60.0, 60.0, egui::PointerButton::Primary);
        h.view.editor.leave_tool();
        let at = |h: &Harness| h.doc.project().symbols().next().unwrap().placement.origin;
        let before = at(&h);
        // Press on the body between the pins, drag 40 mm right, 20 down.
        let (a1, a2) = (pin_at(&h.doc, before, "A1"), pin_at(&h.doc, before, "A2"));
        // A point on the symbol's body that the canvas picks and no pin
        // target covers.
        let tol = h
            .view
            .canvas
            .viewport
            .screen_to_world_len(h.view.canvas.pick_tolerance);
        let zoom = h.view.canvas.viewport.zoom;
        let scene = h.view.scene.as_ref().unwrap().scene();
        let mid = (0..200)
            .map(|i| {
                (
                    a1.x - 20.0 + f64::from(i % 20) * 2.0,
                    a1.y + f64::from(i / 20) * 1.6,
                )
            })
            .find(|&(x, y)| {
                let far = |p: Point| (p.x - x).hypot(p.y - y) * f64::from(zoom) > 12.0;
                far(a1)
                    && far(a2)
                    && scene
                        .pick(Vec2::new(x as f32, y as f32), tol, zoom)
                        .is_some()
            })
            .expect("a pickable body point");
        h.drag(mid, (mid.0 + 40.0, mid.1 + 20.0));
        let after = at(&h);
        assert_eq!(after, Point::new(before.x + 40.0, before.y + 20.0));
        assert!(!h.view.editor.tool_active());
    }

    /// A point the canvas picks as `sym` that no pin target covers.
    fn body_point(h: &Harness, sym: novo_core::SymbolId) -> (f64, f64) {
        let tol = h
            .view
            .canvas
            .viewport
            .screen_to_world_len(h.view.canvas.pick_tolerance);
        let zoom = h.view.canvas.viewport.zoom;
        let scene = h.view.scene.as_ref().unwrap();
        let s = h.doc.project().symbol(sym).unwrap();
        let pins: Vec<Point> = (0..s.pins.len() as u16)
            .filter_map(|i| s.pin_position(i))
            .collect();
        let o = s.placement.origin;
        (0..1600)
            .map(|i| {
                (
                    o.x - 20.0 + f64::from(i % 40),
                    o.y - 20.0 + f64::from(i / 40),
                )
            })
            .find(|&(x, y)| {
                let far = |p: &Point| (p.x - x).hypot(p.y - y) * f64::from(zoom) > 12.0;
                pins.iter().all(far)
                    && scene
                        .scene()
                        .pick(Vec2::new(x as f32, y as f32), tol, zoom)
                        .and_then(|hit| scene.model_id(hit.id))
                        == Some(AnyId::Symbol(sym))
            })
            .expect("a pickable body point")
    }

    #[test]
    fn dragging_a_contact_onto_a_coil_links_it_in_one_step() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(60.0, 60.0, egui::PointerButton::Primary);
        h.view.editor.begin_place("iec/contacts/no", None);
        h.click(160.0, 60.0, egui::PointerButton::Primary);
        h.view.editor.leave_tool();
        let mut syms: Vec<_> = h
            .doc
            .project()
            .symbols()
            .map(|s| (s.id, s.placement.origin))
            .collect();
        syms.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
        let (coil, contact) = (syms[0].0, syms[1].0);
        let before = h.doc.project().devices().count();
        let depth = h.doc.undo_depth();
        h.drag(body_point(&h, contact), body_point(&h, coil));
        let device = h.doc.project().symbol(coil).unwrap().device();
        assert_eq!(h.doc.project().symbol(contact).unwrap().device(), device);
        assert_eq!(h.doc.project().devices().count(), before - 1);
        assert_eq!(h.doc.undo_depth(), depth + 1);
        assert!(
            h.view
                .editor
                .notice()
                .is_some_and(|n| n.starts_with("Linked to"))
        );
    }

    #[test]
    fn dragging_with_a_symbol_on_the_pointer_places_a_row() {
        let mut h = Harness::new();
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/contacts/no", None);
        let depth = h.doc.undo_depth();
        h.drag((60.0, 60.0), (220.0, 62.0));
        let n = h.doc.project().symbols().count();
        assert!(n >= 3, "{n} symbols");
        assert_eq!(h.doc.undo_depth(), depth + 1);
        assert!(h.view.canvas.selection.is_empty());
        // The symbol stays on the pointer for the next one.
        assert!(matches!(h.view.editor.tool(), Tool::Place(_)));
    }

    #[test]
    fn the_click_that_closes_a_menu_does_not_place() {
        let mut h = Harness::new();
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(100.0, 100.0, egui::PointerButton::Secondary);
        assert!(h.view.menu.is_some());
        h.click(160.0, 160.0, egui::PointerButton::Primary);
        assert!(h.view.menu.is_none());
        assert_eq!(h.doc.project().symbols().count(), 0);
        // The next click places as usual.
        h.click(160.0, 160.0, egui::PointerButton::Primary);
        assert_eq!(h.doc.project().symbols().count(), 1);
    }

    #[test]
    fn shift_r_is_not_r() {
        let mut h = Harness::new();
        h.view.editor.begin_place("iec/coils/relay", None);
        h.hover(100.0, 100.0);
        h.frame(vec![egui::Event::Key {
            key: Key::R,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::SHIFT,
        }]);
        assert!(
            matches!(h.view.editor.tool(), Tool::Place(p) if p.rotation == novo_core::Rotation::R0)
        );
        h.key(Key::R);
        assert!(
            matches!(h.view.editor.tool(), Tool::Place(p) if p.rotation == novo_core::Rotation::R90)
        );
    }

    /// A second page added the way the shell does it (not through the
    /// editor, which would leave the tool itself), with a relay on the
    /// first page at (100, 100). Returns the new page and the relay.
    fn two_pages(h: &mut Harness) -> (PageId, novo_core::SymbolId) {
        h.view.editor.auto_connect = false;
        h.view.editor.begin_place("iec/coils/relay", None);
        h.click(100.0, 100.0, egui::PointerButton::Primary);
        h.view.editor.leave_tool();
        let sym = h.doc.project().symbols().next().unwrap().id;
        let (page, _) = h.doc.transact("Add page", |tx| tx.add_page("2")).unwrap();
        (page, sym)
    }

    fn placement(h: &Harness, sym: novo_core::SymbolId) -> novo_core::Placement {
        h.doc.project().symbol(sym).unwrap().placement
    }

    #[test]
    fn a_move_in_hand_does_not_survive_a_page_change() {
        let mut h = Harness::new();
        let (page, sym) = two_pages(&mut h);
        let before = placement(&h, sym);
        h.view.select(&[AnyId::Symbol(sym)]);
        h.hover(100.0, 100.0);
        h.key(Key::M);
        assert!(matches!(h.view.editor.tool(), Tool::Move(_)));
        let depth = h.doc.undo_depth();
        // The shell opens the other page (the page list).
        h.page = page;
        h.frame(Vec::new());
        assert!(!h.view.editor.tool_active());
        assert!(!h.view.editor.in_progress());
        h.click(160.0, 160.0, egui::PointerButton::Primary);
        assert_eq!(placement(&h, sym), before);
        assert_eq!(h.doc.undo_depth(), depth);
    }

    #[test]
    fn a_wire_in_progress_does_not_survive_a_page_change() {
        let mut h = Harness::new();
        let (page, _) = two_pages(&mut h);
        let a2 = pin_at(&h.doc, Point::new(100.0, 100.0), "A2");
        h.view.editor.begin_wire();
        h.click(a2.x, a2.y, egui::PointerButton::Primary);
        assert!(h.view.editor.in_progress());
        let depth = h.doc.undo_depth();
        h.page = page;
        h.frame(Vec::new());
        assert!(!h.view.editor.tool_active());
        assert!(h.view.editor.preview(&h.doc, page).wire.is_empty());
        h.click(160.0, 160.0, egui::PointerButton::Primary);
        h.click(160.0, 200.0, egui::PointerButton::Primary);
        assert_eq!(h.doc.project().wires().count(), 0);
        assert_eq!(h.doc.undo_depth(), depth);
    }

    #[test]
    fn the_selection_does_not_survive_a_page_change() {
        let mut h = Harness::new();
        let (page, sym) = two_pages(&mut h);
        let before = placement(&h, sym);
        h.view.select(&[AnyId::Symbol(sym)]);
        assert_eq!(h.view.selection(), [AnyId::Symbol(sym)]);
        h.click(100.0, 100.0, egui::PointerButton::Secondary);
        assert!(h.view.menu.is_some());
        let depth = h.doc.undo_depth();
        h.page = page;
        h.frame(Vec::new());
        assert!(h.view.selection().is_empty());
        assert!(h.view.menu.is_none());
        // Keys on the new page have nothing of the old page to act on.
        h.hover(100.0, 100.0);
        for key in [Key::R, Key::X, Key::M, Key::Delete] {
            h.key(key);
        }
        h.click(160.0, 160.0, egui::PointerButton::Primary);
        assert_eq!(placement(&h, sym), before);
        assert!(h.doc.project().symbol(sym).is_some());
        assert_eq!(h.doc.undo_depth(), depth);
    }

    #[test]
    fn a_single_click_on_a_symbol_selects_it() {
        let mut h = Harness::new();
        h.view.editor.begin_place("iec/contactors/main_3p", None);
        h.click(60.0, 80.0, egui::PointerButton::Primary);
        h.key(Key::Escape);
        let sym = h.doc.project().symbols().next().unwrap().id;
        let body = h
            .view
            .editor
            .body(h.doc.project().symbol(sym).unwrap())
            .unwrap();
        let (x, y) = ((body.0.x + body.1.x) / 2.0, (body.0.y + body.1.y) / 2.0);
        h.click(x, y, egui::PointerButton::Primary);
        h.frame(Vec::new());
        assert_eq!(h.view.selection(), [AnyId::Symbol(sym)]);
    }
}
