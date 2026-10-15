//! The hook between the shell and the drawing canvas.
//!
//! The shell gives the central area to a [`CanvasView`]. `novo-canvas`
//! (WS-03) provides the real one; [`PlaceholderCanvas`] stands in until then.

use eframe::egui;
use novo_core::{ChangeSet, Command, Document, ModelError, Outcome, PageId, Tx};

use novo_editor::view::SchematicView;

use crate::backend::ProjectBackend;

/// The open page, handed to the canvas each frame. Edits go through
/// [`OpenPage::transact`] or [`OpenPage::execute`], so they join the shared
/// undo history and reach the project file's save and autosave.
pub struct OpenPage<'a> {
    backend: &'a mut dyn ProjectBackend,
    page: PageId,
}

impl<'a> OpenPage<'a> {
    /// `None` when no project is open.
    pub fn new(backend: &'a mut dyn ProjectBackend, page: PageId) -> Option<Self> {
        backend.document()?;
        Some(OpenPage { backend, page })
    }

    pub fn page(&self) -> PageId {
        self.page
    }

    pub fn document(&self) -> &Document {
        self.backend
            .document()
            .expect("OpenPage exists only while a project is open")
    }

    /// Runs `edit` as one undo step, see `Document::transact`.
    pub fn transact<R>(
        &mut self,
        label: impl Into<String>,
        edit: impl FnOnce(&mut Tx<'_>) -> Result<R, ModelError>,
    ) -> Result<(R, ChangeSet), ModelError> {
        let document = self
            .backend
            .document_mut()
            .expect("OpenPage exists only while a project is open");
        let (value, changes) = document.transact(label, edit)?;
        self.backend.record(&changes);
        Ok((value, changes))
    }

    /// Hands the document to a view that edits it directly, one
    /// `Document::transact` per gesture (the schematic editor). `edit`
    /// returns every change it made; they are recorded like any other.
    pub fn edit(&mut self, edit: impl FnOnce(&mut Document, PageId) -> ChangeSet) -> ChangeSet {
        let page = self.page;
        let document = self
            .backend
            .document_mut()
            .expect("OpenPage exists only while a project is open");
        let changes = edit(document, page);
        if !changes.is_empty() {
            self.backend.record(&changes);
        }
        changes
    }

    /// Runs a command as one undo step, see `Document::execute`.
    pub fn execute(&mut self, command: Command) -> Result<Outcome, ModelError> {
        let document = self
            .backend
            .document_mut()
            .expect("OpenPage exists only while a project is open");
        let outcome = document.execute(command)?;
        self.backend.record(&outcome.changes);
        Ok(outcome)
    }
}

pub trait CanvasView {
    /// Draws `open` (or an empty state when `None`) into the whole of `ui`.
    fn show(&mut self, ui: &mut egui::Ui, open: Option<OpenPage<'_>>);

    /// Called after the shell changed the document outside the canvas
    /// (undo, redo), so cached drawing state can update just what changed.
    fn document_changed(&mut self, _document: &Document, _changes: &ChangeSet) {}

    /// Called when a different project was opened, created or closed; drop
    /// anything cached for the old one.
    fn document_replaced(&mut self) {}

    /// Fits the open page into the view.
    fn zoom_to_fit(&mut self) {}

    /// Short text for the status bar, such as cursor coordinates.
    fn status_text(&self) -> Option<String> {
        None
    }

    /// The changes the canvas made to the document since the last call
    /// (already recorded), so the shell can tell its other views.
    fn take_changes(&mut self) -> ChangeSet {
        ChangeSet::default()
    }

    /// A page the canvas asks the shell to open (Page Up, a new page).
    fn take_page_request(&mut self) -> Option<PageId> {
        None
    }

    /// Commands the canvas asks the shell to run, by workflow id.
    fn take_commands(&mut self) -> Vec<&'static str> {
        Vec::new()
    }

    /// The schematic editor, when this canvas is one.
    fn schematic(&mut self) -> Option<&mut SchematicView> {
        None
    }

    /// The schematic editor, read only.
    fn schematic_ref(&self) -> Option<&SchematicView> {
        None
    }
}

/// What the central area shows while no project is open.
pub(crate) fn empty_state(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
    let visuals = ui.visuals();
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "No project open\nCtrl+N new project · Ctrl+O open · Ctrl+K commands",
        egui::FontId::proportional(15.0),
        visuals.weak_text_color(),
    );
}

/// Draws a sheet outline on a dot grid and reports the cursor position, so the
/// shell layout can be checked before the real canvas lands.
#[derive(Default)]
pub struct PlaceholderCanvas {
    cursor_mm: Option<egui::Pos2>,
}

/// Grid pitch of the placeholder, in screen points.
const GRID: f32 = 20.0;
/// Millimetres per grid step, so the status bar reads like a drawing.
const MM_PER_GRID: f32 = 5.0;

impl CanvasView for PlaceholderCanvas {
    fn show(&mut self, ui: &mut egui::Ui, open: Option<OpenPage<'_>>) {
        if open.is_none() {
            self.cursor_mm = None;
            empty_state(ui);
            return;
        }
        let (response, painter) = ui.allocate_painter(ui.available_size(), egui::Sense::hover());
        let rect = response.rect;
        let visuals = ui.visuals();
        painter.rect_filled(rect, 0.0, visuals.extreme_bg_color);

        let dot = visuals.weak_text_color().gamma_multiply(0.5);
        let mut y = rect.top() + GRID;
        while y < rect.bottom() {
            let mut x = rect.left() + GRID;
            while x < rect.right() {
                painter.circle_filled(egui::pos2(x, y), 0.8, dot);
                x += GRID;
            }
            y += GRID;
        }

        let sheet = rect.shrink(GRID * 2.0);
        painter.rect_stroke(
            sheet,
            0.0,
            egui::Stroke::new(1.0, visuals.text_color()),
            egui::StrokeKind::Inside,
        );
        painter.text(
            sheet.center(),
            egui::Align2::CENTER_CENTER,
            "Canvas engine not connected yet",
            egui::FontId::proportional(14.0),
            visuals.weak_text_color(),
        );

        self.cursor_mm = response.hover_pos().map(|p| {
            let local = p - rect.left_top();
            egui::pos2(local.x / GRID * MM_PER_GRID, local.y / GRID * MM_PER_GRID)
        });
    }

    fn status_text(&self) -> Option<String> {
        self.cursor_mm
            .map(|p| format!("X {:.1} mm  Y {:.1} mm", p.x, p.y))
    }
}
