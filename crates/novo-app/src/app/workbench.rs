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
use novo_canvas::model::item_id;
use novo_canvas::{SelectOp, Vec2};
use novo_checks::Problem;
use novo_core::{AnyId, ChangeSet, DeviceId, Document, SymbolId};
use novo_editor::EditorKey;
use novo_symbols::Library;
use novo_workflow::commands::Context;
use novo_workflow::keymap::{FAVOURITE_COMMANDS, Focus, Keymap};
use novo_workflow::palette::{Mode, Sources, Target};
use novo_xref::{LiveXref, Navigator, Place, Reference};

use super::terminals::paint_proposals;
use super::xref::paint_labels;
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
    /// Shift+G: every occurrence of the selection's device.
    pub(super) occurrences: Option<Vec<Reference>>,
    pub(super) drawer: Option<Drawer>,
    /// The terminal proposal Enter accepts, an index into this page's.
    pub(super) proposal: usize,
    /// Shift+P hides cross-reference labels and proposal marks.
    pub(super) marks: bool,
    /// Design-check results for one project revision.
    pub(super) problems: Option<(u64, Vec<Problem>)>,
    /// The issue list under the pill is open.
    pub(super) issues_open: bool,
    /// F8 position in the problem list.
    pub(super) problem_at: Option<usize>,
    /// The last PDF written, for tests and the toast.
    pub(super) last_export: Option<PathBuf>,
}

impl Default for Workbench {
    fn default() -> Self {
        Workbench {
            field: CommandField::default(),
            keymap: Keymap::workflow(),
            xref: LiveXref::default(),
            nav: Navigator::default(),
            pending: None,
            occurrences: None,
            drawer: None,
            proposal: 0,
            marks: true,
            problems: None,
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
        } else if self.is_open() {
            Focus::Canvas
        } else {
            Focus::Panel
        }
    }

    /// Whether the shell runs `command` from a key press itself.
    pub(super) fn claims_key(&self, command: &str) -> bool {
        if EDITOR_KEYS.contains(&command) {
            return false;
        }
        match command {
            // Enter and Tab belong to the editor's wire proposals while it
            // has any; otherwise to the terminal proposals on this page.
            "proposal.accept" | "proposal.next" => {
                let wires = self
                    .canvas
                    .schematic_ref()
                    .is_some_and(|v| !v.editor.proposals().is_empty());
                !wires && self.wb.marks && !self.page_proposals().is_empty()
            }
            id if FAVOURITE_COMMANDS.contains(&id) => true,
            id => match Action::from_id(id) {
                Some(action) => self.is_enabled(action),
                None => novo_workflow::commands::is_known(id),
            },
        }
    }

    /// The commands this frame's key presses run, taken out of the input.
    pub(super) fn key_commands(&self, ctx: &egui::Context) -> Vec<&'static str> {
        let focus = self.key_focus(ctx);
        keys::take_commands(ctx, &self.wb.keymap, focus, |c| self.claims_key(c))
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
            "edit.rotate" => self.editor_key(EditorKey::Rotate),
            "edit.mirror" => self.editor_key(EditorKey::Mirror),
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
            // `accept all` can reach here as `accept` with `all` after it.
            "proposal.accept" if argument.is_some_and(|a| a.trim() == "all") => {
                self.accept_all_proposals();
            }
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
            Target::Command(id) => self.run_command(ctx, id, None),
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
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        // The editor keeps the change and hands it back from its next
        // frame, which records it like any other canvas edit.
        view.editor.key(doc, key, &selection);
    }

    pub(super) fn new_page(&mut self) {
        let Some(after) = self.current_page else {
            return;
        };
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        if let Some(page) = view.editor.new_page(doc, after) {
            self.show_page(page);
        }
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
            let origin = place.symbol.and_then(|s| {
                self.backend
                    .document()
                    .and_then(|d| d.project().symbol(s))
                    .map(|s| s.placement.origin)
            });
            if let (Some(symbol), Some(view)) = (place.symbol, self.canvas.schematic()) {
                view.canvas.selection.clear();
                view.canvas
                    .selection
                    .apply([item_id(AnyId::Symbol(symbol))], SelectOp::Replace);
                if let Some(o) = origin {
                    view.canvas.viewport.center = Vec2::new(o.x as f32, o.y as f32);
                }
            }
        }
        self.highlight_related();
    }

    /// Cross-reference labels and terminal proposals over the drawing.
    pub(super) fn paint_overlays(&mut self, ui: &egui::Ui, rect: egui::Rect) {
        if !self.wb.marks {
            return;
        }
        let Some(page) = self.current_page else {
            return;
        };
        let labels = self.xref_labels(page);
        let proposals = self.page_proposals();
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            return;
        };
        let overlay = Overlay {
            painter: ui.painter().with_clip_rect(rect),
            viewport: view.canvas.viewport,
            rect,
        };
        paint_labels(&overlay, doc.project(), &view.editor, &labels);
        paint_proposals(&overlay, &proposals, self.wb.proposal);
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

/// Where overlays over the drawing paint, and how world millimetres map to
/// the screen.
pub(super) struct Overlay {
    pub painter: egui::Painter,
    pub viewport: novo_canvas::Viewport,
    pub rect: egui::Rect,
}

impl Overlay {
    pub fn to_screen(&self, x: f64, y: f64) -> egui::Pos2 {
        let s = self.viewport.world_to_screen(Vec2::new(x as f32, y as f32));
        self.rect.min + egui::vec2(s.x, s.y)
    }
}
