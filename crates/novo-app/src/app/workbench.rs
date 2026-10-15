//! The Gate 1 engines hosted in the Workbench: the command field and keys
//! (novo-workflow), cross references (novo-xref), the Terminals drawer and
//! terminal proposals (novo-terminals), the issue pill (novo-checks) and
//! PDF export (novo-pdf).
//!
//! Everything here reads the model through queries and changes it through
//! the backend's document, one undo step per gesture, and tells the canvas
//! and storage about every change ([`NovoApp::changed_outside_canvas`]).

use std::path::PathBuf;

use eframe::egui;
use novo_canvas::SelectOp;
use novo_canvas::model::item_id;
use novo_checks::Problem;
use novo_core::{AnyId, ChangeSet, DeviceId, Document, SymbolId};
use novo_editor::EditorKey;
use novo_symbols::Library;
use novo_workflow::commands::Context;
use novo_workflow::keymap::{FAVOURITE_COMMANDS, Focus, Keymap};
use novo_workflow::palette::{Mode, Sources, Target};
use novo_xref::{LiveXref, Navigator, Place, Reference};

use super::{Level, NovoApp};
use crate::actions::Action;
use crate::command_field::CommandField;
use crate::keys;

/// Drawer height as a share of the window (docs/DESIGN.md §3: at most 35%).
pub(super) const DRAWER_SHARE: f32 = 0.3;

/// The drawer's tabs. Only Terminals exists so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drawer {
    Terminals,
}

/// State of the hosted engines, owned by [`NovoApp`].
pub(crate) struct Workbench {
    pub(super) field: CommandField,
    pub(super) keymap: Keymap,
    pub(super) xref: LiveXref,
    pub(super) nav: Navigator,
    /// Where to select and centre once the canvas shows its page.
    pub(super) pending: Option<Place>,
    /// The drawing's layer and rect last frame: a focused widget there is
    /// the canvas, anywhere else a panel.
    pub(super) canvas_area: Option<(egui::LayerId, egui::Rect)>,
    /// Shift+G: every occurrence of the selection's device.
    pub(super) occurrences: Option<Vec<Reference>>,
    pub(super) drawer: Option<Drawer>,
    /// The terminal proposal Enter accepts, an index into this page's.
    pub(super) proposal: usize,
    /// Where the pointer was when Space went down, until it comes up.
    pub(super) space_down: Option<egui::Pos2>,
    /// The pointer on the drawing (mm), when it is over the canvas.
    pub(super) pointer_world: Option<novo_core::Point>,
    /// Shift+P: false hides the marks on the drawing (counters stay).
    pub(super) marks: bool,
    /// Design-check results for one project revision.
    pub(super) problems: Option<(u64, Vec<Problem>)>,
    /// Where the issue pill was drawn, so its own click does not close
    /// the list it opens.
    pub(super) pill_rect: egui::Rect,
    /// The issue list under the pill is open.
    pub(super) issues_open: bool,
    /// F8 position in the problem list.
    pub(super) problem_at: Option<usize>,
    /// The last PDF written, for tests and the toast.
    pub(super) last_export: Option<PathBuf>,
}

impl Workbench {
    /// The command field remembers the commands run in earlier sessions
    /// (newest first), so they lead its empty menu again.
    pub(super) fn with_recent(ids: &[String]) -> Self {
        let mut wb = Workbench::default();
        wb.field
            .palette
            .restore_recent_commands(ids.iter().map(String::as_str));
        wb
    }
}

impl Default for Workbench {
    fn default() -> Self {
        Workbench {
            field: CommandField::default(),
            keymap: Keymap::workflow(),
            xref: LiveXref::default(),
            nav: Navigator::default(),
            pending: None,
            canvas_area: None,
            occurrences: None,
            drawer: None,
            proposal: 0,
            space_down: None,
            pointer_world: None,
            marks: true,
            problems: None,
            pill_rect: egui::Rect::NOTHING,
            issues_open: false,
            problem_at: None,
            last_export: None,
        }
    }
}

/// Commands the editor reads from the keyboard itself: the shell leaves
/// those key presses alone (they still run from the command field).
const EDITOR_KEYS: &[&str] = &[
    "edit.escape",
    "edit.rename",
    "edit.delete",
    "edit.rotate",
    "edit.mirror",
    "tool.connect",
    "nav.previous_page",
    "nav.next_page",
];

/// Commands the app runs besides the shell actions (`Action`) and the
/// favourites; the command field shows the rest as "Comes later".
const APP_COMMANDS: &[&str] = &[
    "tool.place",
    "tool.repeat",
    "tool.connect",
    "view.command_field_here",
    "edit.rotate",
    "edit.mirror",
    "edit.delete",
    "edit.rename",
    "edit.escape",
    "nav.next_page",
    "nav.previous_page",
    "page.new",
    "nav.partner",
    "nav.all_references",
    "nav.back",
    "nav.forward",
    "proposal.accept",
    "proposal.next",
    "proposal.accept_all",
    "view.terminals_drawer",
    "view.toggle_marks",
    "check.problems",
    "check.next_problem",
    "check.previous_problem",
    "export.pdf",
];

/// Whether the app can run a command now (not "comes later").
pub(crate) fn app_runs(id: &str) -> bool {
    APP_COMMANDS.contains(&id) || FAVOURITE_COMMANDS.contains(&id) || Action::from_id(id).is_some()
}

impl NovoApp {
    pub(super) fn library(&self) -> Option<&Library> {
        self.canvas
            .schematic_ref()
            .map(|view| view.editor.catalog.library())
    }

    /// The canvas selection as model ids.
    pub fn selection(&self) -> Vec<AnyId> {
        self.canvas
            .schematic_ref()
            .map(|view| view.selection())
            .unwrap_or_default()
    }

    pub(super) fn selected_symbol(&self) -> Option<SymbolId> {
        match self.selection().as_slice() {
            [AnyId::Symbol(s)] => Some(*s),
            _ => None,
        }
    }

    pub(super) fn here(&self) -> Option<Place> {
        Some(Place {
            page: self.current_page?,
            symbol: self.selected_symbol(),
        })
    }

    /// What availability in the command field is decided on.
    pub(super) fn command_context(&self) -> Context {
        let doc = self.backend.document();
        let selection = self.selection();
        let devices = doc.map_or(0, |d| {
            let project = d.project();
            let mut devices: Vec<DeviceId> = selection
                .iter()
                .filter_map(|&id| novo_editor::edit::device_of(project, id))
                .collect();
            devices.dedup();
            devices.len()
        });
        Context {
            has_project: doc.is_some(),
            has_page: self.current_page.is_some(),
            selection: selection.len(),
            has_device: devices == 1,
            can_undo: doc.is_some_and(Document::can_undo),
            can_redo: doc.is_some_and(Document::can_redo),
            clipboard: false,
            can_go_back: self.wb.nav.history.can_go_back(),
            can_go_forward: self.wb.nav.history.can_go_forward(),
        }
    }

    // ------------------------------------------------------------------
    // Keys and the command field

    /// Who has the keyboard, for the keymap.
    pub(super) fn key_focus(&self, ctx: &egui::Context) -> Focus {
        if let Some(focus) = self.wb.field.focus() {
            focus
        } else if ctx.text_edit_focused() {
            Focus::TextField
        } else if self.is_open() && !self.panel_has_focus(ctx) {
            Focus::Canvas
        } else {
            Focus::Panel
        }
    }

    /// A button or other widget outside the drawing has the keyboard, so
    /// Enter, Tab and G go to it rather than to the canvas.
    fn panel_has_focus(&self, ctx: &egui::Context) -> bool {
        let Some(id) = ctx.memory(|m| m.focused()) else {
            return false;
        };
        let Some(widget) = ctx.read_response(id) else {
            return false;
        };
        match self.wb.canvas_area {
            Some((layer, rect)) => widget.layer_id != layer || !rect.contains_rect(widget.rect),
            None => true,
        }
    }

    /// Enter, Tab and Delete belong to the editor's wire proposals while it
    /// has any; otherwise to the terminal proposals shown on this page.
    fn terminal_proposals_have_keys(&self) -> bool {
        let wires = self
            .canvas
            .schematic_ref()
            .is_some_and(|v| !v.editor.proposals().is_empty());
        !wires && self.wb.marks && !self.page_proposals().is_empty()
    }

    /// Delete with the pointer on the highlighted terminal proposal
    /// declines it rather than deleting anything; elsewhere Delete deletes
    /// (as the editor does for wire proposals).
    fn declines_proposal(&self) -> bool {
        self.terminal_proposals_have_keys() && self.pointer_on_proposal()
    }

    /// Where the pointer is on the drawing, in millimetres, for the keys
    /// that act on what is under it.
    pub(super) fn track_pointer(&mut self, ctx: &egui::Context) {
        let at = ctx.pointer_hover_pos();
        self.wb.pointer_world = match (at, self.wb.canvas_area, self.canvas.schematic_ref()) {
            (Some(at), Some((_, rect)), Some(view)) if rect.contains(at) => {
                let local = at - rect.min;
                let w = view
                    .canvas
                    .viewport
                    .screen_to_world(novo_canvas::Vec2::new(local.x, local.y));
                Some(novo_core::Point::new(f64::from(w.x), f64::from(w.y)))
            }
            _ => None,
        };
    }

    /// Whether the shell runs `command` from a key press itself.
    pub(super) fn claims_key(&self, command: &str) -> bool {
        // Delete with the pointer on the highlighted terminal proposal
        // declines it (DESIGN.md §5); otherwise it is the editor's.
        if command == "edit.delete" {
            return self.declines_proposal();
        }
        if EDITOR_KEYS.contains(&command) {
            return false;
        }
        match command {
            // Enter and Tab belong to the editor's wire proposals while it
            // has any; otherwise to the terminal proposals on this page.
            "proposal.accept" | "proposal.next" => self.terminal_proposals_have_keys(),
            id if FAVOURITE_COMMANDS.contains(&id) => true,
            id => match Action::from_id(id) {
                Some(action) => self.is_enabled(action),
                None => novo_workflow::commands::is_known(id),
            },
        }
    }

    /// Escape closes the command field or a list over the canvas first.
    /// Taken before the canvas runs, which would read it as its own Escape
    /// ladder while the pointer is over the drawing.
    pub(super) fn escape_popups(&mut self, ctx: &egui::Context) {
        let open = self.wb.field.is_open() || self.wb.occurrences.is_some() || self.wb.issues_open;
        if open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.wb.field.is_open() {
                self.wb.field.close();
            } else if self.wb.occurrences.is_some() {
                self.wb.occurrences = None;
            } else {
                self.wb.issues_open = false;
            }
        }
    }

    /// The commands this frame's key presses run, taken out of the input.
    pub(super) fn key_commands(&self, ctx: &egui::Context) -> Vec<&'static str> {
        let focus = self.key_focus(ctx);
        keys::take_commands(ctx, &self.wb.keymap, focus, |c| self.claims_key(c))
    }

    /// Runs a command from a key press. Space only arms the command field:
    /// it opens on the release, unless the pointer moved more than 4 px in
    /// between (a held Space pans); every other key runs its command.
    pub(super) fn run_key_command(&mut self, ctx: &egui::Context, id: &str) {
        if id == "view.command_field_here" {
            self.wb.space_down = ctx.pointer_latest_pos().or(Some(egui::Pos2::ZERO));
        } else {
            self.run_command(ctx, id, None);
        }
    }

    /// Runs a command by its novo-workflow id, from a key or the command
    /// field. `argument` is what was typed after its verb.
    pub(super) fn run_command(&mut self, ctx: &egui::Context, id: &str, argument: Option<&str>) {
        if let Some(slot) = FAVOURITE_COMMANDS.iter().position(|&c| c == id) {
            let digit = (slot + 1).to_string();
            let target = self.with_field(|field, sources| field.pick_shortcut(&digit, sources));
            if let Some(target) = target {
                self.run_target(ctx, target);
            }
            return;
        }
        if let Some(action) = Action::from_id(id) {
            if action == Action::CommandPalette {
                self.wb.field.open(Mode::All, None);
            } else {
                self.run(ctx, action);
            }
            return;
        }
        let pointer = ctx.pointer_latest_pos();
        match id {
            "tool.place" => match argument.filter(|a| !a.trim().is_empty()) {
                Some(text) => self.wb.field.open_with(Mode::Place, text, pointer),
                None => self.wb.field.open(Mode::Place, pointer),
            },
            "view.command_field_here" => self.wb.field.open(Mode::All, pointer),
            "tool.connect" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_wire();
                }
            }
            "tool.repeat" => self.editor_key(EditorKey::Repeat),
            "edit.rotate" => self.editor_key(EditorKey::Rotate),
            "edit.mirror" => self.editor_key(EditorKey::Mirror),
            "edit.delete" if self.declines_proposal() => self.dismiss_proposal(),
            "edit.delete" => self.editor_key(EditorKey::Delete),
            "nav.next_page" | "nav.previous_page" => {
                let step = if id == "nav.next_page" { 1 } else { -1 };
                if let (Some(doc), Some(page)) = (self.backend.document(), self.current_page)
                    && let Some(to) = novo_editor::pages::neighbour(doc.project(), page, step)
                {
                    self.show_page(to);
                }
            }
            "page.new" => self.new_page(),
            "nav.partner" => self.go_partner(),
            "nav.all_references" => self.list_occurrences(),
            "nav.back" | "nav.forward" => self.go_history(id == "nav.back"),
            "proposal.accept" => self.accept_proposal(),
            "proposal.next" => {
                // Tab is "next proposal" on the drawing: egui's own focus
                // move to the next widget is undone.
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
                self.next_proposal();
            }
            "proposal.accept_all" => self.accept_all_proposals(),
            "view.terminals_drawer" => self.toggle_drawer(Drawer::Terminals),
            "view.cables_drawer" | "view.devices_drawer" => {
                self.message(Level::Info, "The Cables and Devices drawers come later");
            }
            "view.toggle_marks" => self.wb.marks ^= true,
            "check.problems" => self.wb.issues_open = true,
            "check.next_problem" => self.step_problem(true),
            "check.previous_problem" => self.step_problem(false),
            "export.pdf" => {
                self.export_pdf();
            }
            other => {
                let label = novo_workflow::commands::get(other).map_or(other, |c| c.label);
                self.message(Level::Info, format!("{label} is not in the app yet"));
            }
        }
    }

    /// Does what picking a command field row asks.
    pub(super) fn run_target(&mut self, ctx: &egui::Context, target: Target) {
        match target {
            Target::Command(id) => {
                self.settings.remember_action(id);
                self.run_command(ctx, id, None);
            }
            Target::Run { command, argument } => self.run_command(ctx, command, Some(&argument)),
            Target::Page(page) => self.show_page(page),
            Target::Device(device) => {
                let to = self.device_main(device);
                match to {
                    Some(place) => self.jump(place, true),
                    None => self.message(Level::Info, "That device is not drawn on any page"),
                }
            }
            Target::Location { symbol, .. } => {
                let page = self
                    .backend
                    .document()
                    .and_then(|d| d.project().symbol(symbol))
                    .map(|s| s.page);
                if let Some(page) = page {
                    self.jump(
                        Place {
                            page,
                            symbol: Some(symbol),
                        },
                        true,
                    );
                }
            }
            Target::Symbol(symbol) => self.begin_place(&symbol, None),
            Target::Child { symbol, device } => {
                let tag = self
                    .backend
                    .document()
                    .and_then(|d| d.project().device(device))
                    .map(|d| d.tag.to_string());
                self.begin_place(&symbol, tag.as_deref());
            }
        }
    }

    pub(super) fn begin_place(&mut self, symbol: &str, parent: Option<&str>) {
        if self.current_page.is_none() {
            return;
        }
        if let Some(view) = self.canvas.schematic() {
            view.editor.begin_place(symbol, parent);
        }
    }

    pub(super) fn editor_key(&mut self, key: EditorKey) {
        let selection = self.selection();
        self.with_editor(|editor, doc| editor.key(doc, key, &selection));
    }

    pub(super) fn new_page(&mut self) {
        let Some(after) = self.current_page else {
            return;
        };
        if let Some(Some(page)) = self.with_editor(|editor, doc| editor.new_page(doc, after)) {
            self.show_page(page);
        }
    }

    /// Runs an editor edit from outside the canvas frame (a command or a
    /// key the shell handles) and records what it changed at once: storage
    /// and the views hear about it now, so a save or autosave before the
    /// next frame cannot miss it. `None` without an editor or a project.
    pub(super) fn with_editor<R>(
        &mut self,
        edit: impl FnOnce(&mut novo_editor::Editor, &mut Document) -> R,
    ) -> Option<R> {
        let view = self.canvas.schematic()?;
        let doc = self.backend.document_mut()?;
        let result = edit(&mut view.editor, doc);
        let changes = view.editor.take_changes();
        self.changed_outside_canvas(&changes);
        Some(result)
    }

    /// Shows the command field when open and runs its pick.
    pub(super) fn command_field(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.field.is_open() {
            return;
        }
        let picked = self.with_field(|field, sources| field.show(ctx, canvas, sources));
        if let Some(target) = picked {
            self.run_target(ctx, target);
        }
    }

    /// Opens the command field, types `query` and presses Enter, as a user
    /// would; a step row (`no `) only completes the text. Returns whether a
    /// row ran. For scripts and tests that drive the whole window.
    pub fn type_command(&mut self, ctx: &egui::Context, mode: Mode, query: &str) -> bool {
        self.wb.field.open(mode, None);
        self.wb.field.type_text(query);
        let picked = self.with_field(|field, sources| field.enter(sources));
        match picked {
            Some(target) => {
                self.run_target(ctx, target);
                true
            }
            None => false,
        }
    }

    /// Runs `f` with the command field and what it searches.
    pub(super) fn with_field<R>(
        &mut self,
        f: impl FnOnce(&mut CommandField, &Sources<'_>) -> R,
    ) -> R {
        let mut field = std::mem::take(&mut self.wb.field);
        let result = {
            let sources = sources(self);
            f(&mut field, &sources)
        };
        self.wb.field = field;
        result
    }

    // ------------------------------------------------------------------
    // Changes made outside the canvas

    /// Records a change made by the shell (not the canvas): the canvas and
    /// storage hear about it, and the shown page stays valid.
    pub(super) fn changed_outside_canvas(&mut self, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        if let Some(doc) = self.backend.document() {
            self.canvas.document_changed(doc, changes);
        }
        self.backend.record(changes);
        self.fix_current_page();
    }

    /// Jumps to a place: opens its page, then selects and centres its
    /// symbol. `record` puts where we were in the back history.
    pub(super) fn jump(&mut self, to: Place, record: bool) {
        if record && let Some(here) = self.here() {
            self.wb.nav.history.record(here);
        }
        self.show_page(to.page);
        self.wb.pending = Some(to);
    }

    /// After the canvas showed its page: selects and centres a pending
    /// jump target, and lights up what the selection is linked to.
    pub(super) fn after_canvas_frame(&mut self) {
        if let Some(place) = self.wb.pending
            && Some(place.page) == self.current_page
        {
            self.wb.pending = None;
            if let (Some(symbol), Some(view)) = (place.symbol, self.canvas.schematic()) {
                let item = item_id(AnyId::Symbol(symbol));
                view.canvas.selection.clear();
                view.canvas.selection.apply([item], SelectOp::Replace);
                view.canvas.reveal([item]);
            }
        }
        self.highlight_related();
    }

    /// Fills the canvas marks for the shown page before it draws: terminal
    /// proposals and problems. Shift+P hides them (the counters stay).
    pub(super) fn update_marks(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let mut list = super::terminals::proposal_marks(&self.page_proposals(), self.wb.proposal);
        list.extend(self.problem_marks(page));
        let hidden = !self.wb.marks;
        // Cross references are printed drawing content: they go into the
        // page scene, like the tags, so the PDF shows them the same way.
        let notes = self.xref_notes(page);
        if let Some(view) = self.canvas.schematic() {
            view.canvas.marks.list = list;
            view.canvas.marks.hidden = hidden;
            view.set_notes(notes);
        }
    }

    /// Opens the field for a tapped Space once it is released.
    pub(super) fn space_released(&mut self, ctx: &egui::Context) {
        let Some(down) = self.wb.space_down else {
            return;
        };
        let (released, pointer) = ctx.input(|i| {
            let up = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Space,
                        pressed: false,
                        ..
                    }
                )
            });
            (up, i.pointer.latest_pos())
        });
        if !released {
            return;
        }
        self.wb.space_down = None;
        let moved = pointer.map_or(0.0, |p| p.distance(down));
        if moved <= 4.0 {
            self.wb.field.open(Mode::All, pointer.or(Some(down)));
        }
    }
}

/// What the command field searches, from the app's parts.
fn sources(app: &NovoApp) -> Sources<'_> {
    Sources {
        project: app.backend.document().map(Document::project),
        library: app.library(),
        keymap: &app.wb.keymap,
        context: app.command_context(),
    }
}
