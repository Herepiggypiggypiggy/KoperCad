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
use novo_canvas::model::{self, PageScene, PlaceholderArt, SymbolArt, item_id};
use novo_canvas::widget::{CanvasView, EscapeTarget};
use novo_canvas::{Color, Item, ItemId, Primitive, SelectOp, Shape, Stroke, Vec2};
use novo_core::{AnyId, ChangeSet, DeviceId, Document, PageId, Point};

use crate::art::LibraryArt;
use crate::catalog::Match;
use crate::edit::device_of;
use crate::potential::PotentialMenu;
use crate::tool::{Editor, EditorKey};

/// Rows shown in the insert search.
const SEARCH_ROWS: usize = 8;
/// Colour of things that follow the pointer.
const GHOST: Color = Color::rgba(0, 120, 215, 200);
/// Proposal marks and the pin-hover ring (DESIGN.md §5, steel blue).
const STEEL: Color = Color::rgb(70, 130, 180);
const PROPOSAL: Color = Color::rgba(70, 130, 180, 140);
/// A pin wins over the symbol body within this many screen points.
const PIN_RADIUS: f32 = 6.0;
/// First preview item id; previews count down from here.
const PREVIEW_BASE: u64 = u64::MAX - 16;

struct InsertSearch {
    query: String,
    anchor: egui::Pos2,
    selected: usize,
    focus: bool,
}

struct RenameBox {
    device: DeviceId,
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
    page: PageId,
    opened: bool,
}

/// One page of the schematic on screen, with the editor's tools.
pub struct SchematicView {
    pub editor: Editor,
    pub canvas: CanvasView,
    art: LibraryArt,
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
    /// The press that closed a popup, until its release.
    swallow: bool,
    /// Where the canvas was drawn last frame, in screen points.
    rect: egui::Rect,
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
        let style = novo_canvas::model::ModelStyle::default();
        let art = LibraryArt::new(editor.catalog.library(), style.symbol);
        SchematicView {
            editor,
            canvas: CanvasView::new(),
            art,
            scene: None,
            revision: None,
            previews: Vec::new(),
            insert: None,
            rename: None,
            menu: None,
            status: None,
            goto: None,
            swallow: false,
            rect: egui::Rect::NOTHING,
        }
    }

    /// The shell changed the document outside the view (undo, redo).
    pub fn document_changed(&mut self, document: &Document, changes: &ChangeSet) {
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
        self.canvas.selection.clear();
        self.editor.leave_tool();
        self.insert = None;
        self.rename = None;
        self.menu = None;
        self.canvas.zoom_to_fit();
    }

    /// The page the user asked to open (Page Up / Page Down, "New page").
    /// The shell opens it and passes it to the next [`show`](Self::show).
    pub fn take_page_request(&mut self) -> Option<PageId> {
        self.goto.take()
    }

    pub fn zoom_to_fit(&mut self) {
        self.canvas.zoom_to_fit();
    }

    /// What the next click does, for the status bar.
    pub fn status_text(&self) -> Option<String> {
        self.status.clone()
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
                _ => self.scene = Some(PageScene::new(project, page, &self.art)),
            }
            self.previews.clear();
            if fresh {
                self.canvas.selection.clear();
                self.canvas.zoom_to_fit();
            }
            self.revision = Some(project.revision());
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
        let ghost = Stroke::world(0.35, GHOST);
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
                    p.stroke = p.stroke.map(|s| Stroke { color: GHOST, ..s });
                    p.fill = p.fill.map(|_| GHOST);
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
            // Proposals are dashed steel blue; the one Enter takes is solid
            // (DESIGN.md §5).
            let mut item = Item::new(layer).unselectable();
            for (i, &(a, b)) in preview.proposals.iter().enumerate() {
                let line = Shape::line(model::vec(a), model::vec(b));
                if preview.current == Some(i) {
                    item = item.with(Primitive::stroked(line, Stroke::world(0.35, STEEL)));
                } else {
                    for dash in crate::art::dashes(&line) {
                        item = item.with(Primitive::stroked(dash, Stroke::world(0.35, PROPOSAL)));
                    }
                }
            }
            items.push(item);
        }
        if let Some(t) = preview.target {
            items.push(Item::new(layer).unselectable().with(Primitive::stroked(
                Shape::circle(model::vec(t), 1.2),
                Stroke::screen(2.0, STEEL),
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
            }
        }
        if let Some(w) = out.cursor_raw {
            self.editor.hover(document, page, point(w), tolerance);
        }
        if self.editor.dragging()
            && response.drag_stopped_by(egui::PointerButton::Primary)
            && let Some(w) = out.cursor_raw
        {
            self.editor.click(document, page, point(w), tolerance);
        }
        // Clicks drive the active tool; with none, the canvas selects (and
        // the editor drops unaccepted proposals).
        if response.clicked_by(egui::PointerButton::Primary)
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
            let anchor = ctx.pointer_latest_pos().unwrap_or(rect.center());
            self.menu = Some(ContextMenu {
                anchor,
                empty: selection.is_empty(),
                potentials,
                device,
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
        // (DESIGN.md §4).
        if !self.editor.proposals().is_empty() {
            if pressed(Key::Enter) {
                self.editor.accept_proposal(document);
            } else if pressed(Key::Tab) {
                self.editor.next_proposal();
            }
        }
        let selection = self.selection();
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
        if pressed(Key::I) {
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
                    device,
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
                    if edit.changed() {
                        rename.error = self
                            .editor
                            .check_rename(document, rename.device, &rename.text)
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
            match self.editor.rename(document, rename.device, &rename.text) {
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
                        device,
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

/// A symbol name cut to a short favourite label.
fn short(name: &str) -> String {
    let s: String = name.chars().take(18).collect();
    if s.len() < name.len() {
        format!("{s}…")
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
