//! The real canvas: `novo-canvas` (WS-03) drawing the open page of the
//! project.
//!
//! Each page shown gets a [`PageScene`] (the page's drawing, kept in sync
//! with the model) and its own view, so pan, zoom and selection are kept per
//! page while switching between pages.

use std::collections::HashMap;

use eframe::egui;
use novo_canvas::model::{PageScene, PlaceholderArt};
use novo_canvas::widget::{self, CanvasView as View};
use novo_canvas::{Snapped, Vec2};
use novo_core::{ChangeSet, Document, PageId};

use crate::canvas::{CanvasView, OpenPage, empty_state};

struct PageCanvas {
    scene: PageScene,
    view: View,
}

#[derive(Default)]
pub struct ModelCanvas {
    pages: HashMap<PageId, PageCanvas>,
    /// The page drawn last frame, for zoom to fit.
    shown: Option<PageId>,
    cursor: Option<Snapped>,
}

impl ModelCanvas {
    /// Registers the GPU pipeline. Call once, when eframe has started with
    /// the wgpu renderer; `msaa_samples` is `NativeOptions::multisampling`.
    pub fn install(render_state: &eframe::egui_wgpu::RenderState, msaa_samples: u32) {
        widget::install(render_state, msaa_samples);
    }

    fn page_canvas(&mut self, document: &Document, page: PageId) -> &mut PageCanvas {
        self.pages.entry(page).or_insert_with(|| PageCanvas {
            scene: PageScene::new(document.project(), page, &PlaceholderArt),
            view: View::new(),
        })
    }
}

impl CanvasView for ModelCanvas {
    fn show(&mut self, ui: &mut egui::Ui, open: Option<OpenPage<'_>>) {
        let Some(open) = open else {
            self.shown = None;
            self.cursor = None;
            empty_state(ui);
            return;
        };
        let page = open.page();
        self.shown = Some(page);
        let canvas = self.page_canvas(open.document(), page);
        let output = canvas.view.show(ui, canvas.scene.scene());
        self.cursor = output.cursor;
    }

    fn document_changed(&mut self, document: &Document, changes: &ChangeSet) {
        let project = document.project();
        self.pages.retain(|&id, _| project.page(id).is_some());
        for canvas in self.pages.values_mut() {
            canvas.scene.apply(project, changes, &PlaceholderArt);
        }
    }

    fn document_replaced(&mut self) {
        self.pages.clear();
        self.shown = None;
        self.cursor = None;
    }

    fn zoom_to_fit(&mut self) {
        if let Some(canvas) = self.shown.and_then(|p| self.pages.get_mut(&p)) {
            canvas.view.zoom_to_fit();
        }
    }

    fn status_text(&self) -> Option<String> {
        self.cursor.map(|c| {
            let Vec2 { x, y } = c.point;
            format!("X {x:.1} mm  Y {y:.1} mm")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{MemoryBackend, ProjectBackend};

    fn run(canvas: &mut ModelCanvas, backend: &mut MemoryBackend, page: Option<PageId>) {
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let open = page.and_then(|p| OpenPage::new(backend, p));
            canvas.show(ui, open);
        });
        output.textures_delta.clear();
    }

    fn first_page(backend: &MemoryBackend) -> PageId {
        crate::page_labels(backend.document().unwrap())[0].0
    }

    #[test]
    fn keeps_one_scene_per_page_and_drops_removed_pages() {
        let mut backend = MemoryBackend::default();
        backend.new_project("Plant").unwrap();
        let mut canvas = ModelCanvas::default();
        let one = first_page(&backend);
        run(&mut canvas, &mut backend, Some(one));
        assert_eq!(canvas.shown, Some(one));

        let doc = backend.document_mut().unwrap();
        let (two, changes) = doc.transact("Add page", |tx| tx.add_page("2")).unwrap();
        canvas.document_changed(backend.document().unwrap(), &changes);
        run(&mut canvas, &mut backend, Some(two));
        assert_eq!(canvas.pages.len(), 2);

        let changes = backend.document_mut().unwrap().undo().unwrap().unwrap();
        canvas.document_changed(backend.document().unwrap(), &changes);
        assert_eq!(canvas.pages.len(), 1);
        assert!(canvas.pages.contains_key(&one));
    }

    #[test]
    fn a_new_project_starts_from_scratch() {
        let mut backend = MemoryBackend::default();
        backend.new_project("Plant").unwrap();
        let mut canvas = ModelCanvas::default();
        let one = first_page(&backend);
        run(&mut canvas, &mut backend, Some(one));
        canvas.document_replaced();
        assert!(canvas.pages.is_empty());
        run(&mut canvas, &mut backend, None);
        assert_eq!(canvas.shown, None);
        assert_eq!(canvas.status_text(), None);
    }
}
