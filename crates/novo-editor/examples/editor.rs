//! The schematic editor on its own, for trying it before the app shell
//! hosts it:
//!
//! ```text
//! cargo run -p novo-editor --example editor
//! ```
//!
//! I inserts a symbol (type `coil`, `no`, `m3`, `K1 no`, or a digit for a
//! favourite), W draws wires, M moves, R rotates, X mirrors, F2 renames,
//! Delete deletes, Enter repeats the last tool, Escape steps back, Ctrl+Z
//! and Ctrl+Y undo and redo, right click on wires assigns potentials.

use eframe::egui;
use novo_core::{Document, PageId};
use novo_editor::view::SchematicView;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("NovoElectric schematic editor")
            .with_inner_size([1400.0, 900.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(
        "novo-editor",
        options,
        Box::new(|cc| {
            let rs = cc
                .wgpu_render_state
                .as_ref()
                .ok_or("eframe started without the wgpu renderer")?;
            novo_canvas::widget::install(rs, 1);
            Ok(Box::new(EditorApp::new()))
        }),
    )
}

struct EditorApp {
    doc: Document,
    page: PageId,
    view: SchematicView,
}

impl EditorApp {
    fn new() -> Self {
        let mut doc = Document::default();
        let (page, _) = doc
            .transact("New page", |tx| {
                let page = tx.add_page("1")?;
                tx.update::<novo_core::Page>(page, |p| p.title = "Main circuit".into())?;
                Ok(page)
            })
            .expect("a new page");
        let mut view = SchematicView::default();
        if std::env::args().any(|a| a == "--demo") {
            demo(&mut doc, page, &mut view);
        }
        doc.clear_history();
        EditorApp { doc, page, view }
    }
}

impl eframe::App for EditorApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let (undo, redo) = ui.ctx().input_mut(|i| {
            (
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::Y),
            )
        });
        let step = if undo {
            self.doc.undo().ok().flatten()
        } else if redo {
            self.doc.redo().ok().flatten()
        } else {
            None
        };
        if let Some(changes) = step {
            self.view.document_changed(&self.doc, &changes);
        }
        egui::Panel::bottom("status").show(ui, |ui| {
            let name = self
                .doc
                .project()
                .page(self.page)
                .map_or("", |p| p.name.as_str());
            ui.label(format!(
                "Page {name}   {}",
                self.view.status_text().unwrap_or_default()
            ));
        });
        egui::CentralPanel::no_frame().show(ui, |ui| {
            // A real shell records these for saving (ProjectBackend::record).
            let _changes = self.view.show(ui, &mut self.doc, self.page);
        });
        // Page Up / Page Down and "New page after this" open another page.
        if let Some(page) = self.view.take_page_request() {
            self.page = page;
        }
    }
}

/// Draws the Gate 1 main circuit through the editor, the way a user would:
/// stacked devices wire themselves, then L1, L2 and L3 get their potential.
fn demo(doc: &mut Document, page: PageId, view: &mut SchematicView) {
    use novo_core::{AnyId, Point};
    let editor = &mut view.editor;
    let at = |x: f64, y: f64| Point::new(x, y);
    for (symbol, origin) in [
        ("iec/coils/relay", at(200.0, 120.0)),
        ("iec/protection/fuse_3p", at(80.0, 48.0)),
        ("iec/protection/overload_relay", at(80.0, 144.0)),
        ("iec/motors/motor_3ph", at(80.0, 220.0)),
        ("iec/operators/pushbutton_nc", at(200.0, 48.0)),
    ] {
        editor.begin_place(symbol, None);
        editor.click(doc, page, origin, 1.0);
    }
    for (symbol, origin) in [
        ("iec/contactors/main_3p", at(80.0, 96.0)),
        ("iec/contacts/no", at(232.0, 72.0)),
    ] {
        editor.begin_place(symbol, Some("-K1"));
        editor.click(doc, page, origin, 1.0);
    }
    editor.leave_tool();
    // Feed wires from the top of the page onto the fuses.
    let fuse = doc
        .project()
        .symbols()
        .find(|s| s.symbol.0 == "iec/protection/fuse_3p")
        .map(|s| s.id)
        .expect("fuse");
    let wires: Vec<_> = doc
        .project()
        .wires()
        .filter(|w| {
            w.ends()
                .iter()
                .any(|e| matches!(e, novo_core::Endpoint::Pin { symbol, .. } if *symbol == fuse))
        })
        .map(|w| w.id)
        .collect();
    let selection: Vec<AnyId> = wires.iter().map(|&w| AnyId::Wire(w)).collect();
    let (ordered, suggestions) = editor.potential_suggestions(doc, &selection);
    if let Some(first) = suggestions.first() {
        editor.assign_potentials(doc, &ordered, first);
    }
}
