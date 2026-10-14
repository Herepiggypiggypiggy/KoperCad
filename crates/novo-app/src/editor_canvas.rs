//! The schematic editor on the canvas: novo-editor's [`SchematicView`]
//! (WS-06) hosted behind the shell's [`CanvasView`].
//!
//! The view edits the document directly, one `Document::transact` per
//! gesture, and returns what changed each frame. That change set reaches
//! the backend (save, autosave) through [`OpenPage::edit`] and the shell's
//! other views through [`CanvasView::take_changes`]; it is never handed back
//! to the view itself, which already applied it.

use eframe::egui;
use novo_core::{ChangeSet, Document, PageId};
use novo_editor::view::SchematicView;

use crate::canvas::{CanvasView, OpenPage, empty_state};
use crate::theme;

pub struct EditorCanvas {
    view: SchematicView,
    /// Changes made by the view since the shell last asked.
    changes: ChangeSet,
}

impl Default for EditorCanvas {
    fn default() -> Self {
        let mut view = SchematicView::default();
        view.canvas.style = theme::canvas_style();
        view.set_style(theme::model_style());
        EditorCanvas {
            view,
            changes: ChangeSet::default(),
        }
    }
}

impl EditorCanvas {
    /// Registers the GPU pipeline. Call once, when eframe has started with
    /// the wgpu renderer. The editor draws without multisampling.
    pub fn install(render_state: &eframe::egui_wgpu::RenderState) {
        novo_canvas::widget::install(render_state, 1);
    }
}

impl CanvasView for EditorCanvas {
    fn show(&mut self, ui: &mut egui::Ui, open: Option<OpenPage<'_>>) {
        let Some(mut open) = open else {
            empty_state(ui);
            return;
        };
        let view = &mut self.view;
        let changes = open.edit(|document, page| view.show(ui, document, page));
        self.changes.merge(&changes);
    }

    fn document_changed(&mut self, document: &Document, changes: &ChangeSet) {
        self.view.document_changed(document, changes);
    }

    fn document_replaced(&mut self) {
        self.view.document_replaced();
        // Nothing the editor did to the old project may reach the new one.
        drop(self.view.editor.take_changes());
        self.changes = ChangeSet::default();
    }

    fn zoom_to_fit(&mut self) {
        self.view.zoom_to_fit();
    }

    fn status_text(&self) -> Option<String> {
        self.view.status_text()
    }

    fn take_changes(&mut self) -> ChangeSet {
        std::mem::take(&mut self.changes)
    }

    fn take_page_request(&mut self) -> Option<PageId> {
        self.view.take_page_request()
    }

    fn schematic(&mut self) -> Option<&mut SchematicView> {
        Some(&mut self.view)
    }

    fn schematic_ref(&self) -> Option<&SchematicView> {
        Some(&self.view)
    }
}
