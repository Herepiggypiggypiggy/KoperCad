//! Cross references in the app (novo-xref, WS-10): labels next to coils
//! and contacts, G / Shift+G, Alt+Left / Alt+Right and jumps to a device.

use eframe::egui;
use novo_canvas::model::{NoteAnchor, NoteDensity, NoteGlyph, NoteRow, SymbolNote, item_id};
use novo_core::{AnyId, DeviceId, PageId, SymbolId};
use novo_xref::{ContactKind, Label, LabelAnchor, LiveXref, Place, Reference, XrefIndex};
use std::collections::BTreeMap;

use super::{Level, NovoApp};

impl NovoApp {
    /// Lights up what the selected symbol is linked to on this page (a
    /// coil's contacts, a contact's coil).
    pub(super) fn highlight_related(&mut self) {
        let related: Vec<SymbolId> = match self.selected_symbol() {
            Some(symbol) => xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref)
                .map(|index| index.highlight(symbol).on_page)
                .unwrap_or_default(),
            None => Vec::new(),
        };
        if let Some(view) = self.canvas.schematic() {
            let lit = view.where_lit();
            view.canvas.set_related(
                related
                    .into_iter()
                    .map(|s| item_id(AnyId::Symbol(s)))
                    .chain(lit),
            );
        }
    }

    /// The cross-reference labels of a page.
    pub(super) fn xref_labels(&mut self, page: PageId) -> Vec<Label> {
        xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref)
            .map(|index| index.labels_on_page(page))
            .unwrap_or_default()
    }

    /// The cross-reference labels of a page, as notes for the page scene.
    pub(super) fn xref_notes(&mut self, page: PageId) -> Vec<SymbolNote> {
        let density = self.settings.xref_density();
        self.xref_labels(page)
            .into_iter()
            .map(|l| note(l, density))
            .collect()
    }

    /// Every page's cross-reference notes, for the PDF.
    pub(super) fn xref_notes_by_page(&mut self) -> BTreeMap<PageId, Vec<SymbolNote>> {
        let pages: Vec<PageId> = self
            .backend
            .document()
            .map(|d| d.project().pages().map(|p| p.id).collect())
            .unwrap_or_default();
        let density = self.settings.xref_density();
        let Some(index) = self.xref() else {
            return BTreeMap::new();
        };
        pages
            .into_iter()
            .map(|p| {
                let notes = index
                    .labels_on_page(p)
                    .into_iter()
                    .map(|l| note(l, density))
                    .collect();
                (p, notes)
            })
            .collect()
    }

    pub(super) fn xref(&mut self) -> Option<&XrefIndex> {
        xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref)
    }

    pub(super) fn device_main(&mut self, device: DeviceId) -> Option<Place> {
        let index = self.xref()?;
        let to = index
            .main_of(device)
            .or_else(|| index.device_references(device).into_iter().next())?;
        Some(Place::of(to))
    }

    /// G: coil to contact and back, cycling while pressed again.
    pub(super) fn go_partner(&mut self) {
        let Some(here) = self.here() else {
            return;
        };
        if here.symbol.is_none() {
            self.message(
                Level::Info,
                "Select a coil, contact or interruption point first",
            );
            return;
        }
        let go = {
            let Some(index) = xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref) else {
                return;
            };
            self.wb.nav.go(index, here)
        };
        match go {
            Some(go) => {
                self.show_page(go.to.page);
                self.wb.pending = Some(go.to);
            }
            None => self.message(Level::Info, "Nothing to jump to from here"),
        }
    }

    /// Shift+G: lists every occurrence of the selection's device.
    pub(super) fn list_occurrences(&mut self) {
        let Some(here) = self.here() else {
            return;
        };
        let all: Vec<Reference> = match xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref) {
            Some(index) => self.wb.nav.all(index, here).into_iter().cloned().collect(),
            None => return,
        };
        if all.is_empty() {
            self.message(Level::Info, "Select a device that is drawn somewhere first");
        } else {
            self.wb.occurrences = Some(all);
        }
    }

    pub(super) fn go_history(&mut self, back: bool) {
        let Some(here) = self.here() else {
            return;
        };
        let to = if back {
            self.wb.nav.back(here)
        } else {
            self.wb.nav.forward(here)
        };
        if let Some(to) = to {
            self.show_page(to.page);
            self.wb.pending = Some(to);
        }
    }

    /// Shift+G list: every occurrence; a click jumps there.
    pub(super) fn occurrences_popup(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let Some(list) = &self.wb.occurrences else {
            return;
        };
        let reader = self
            .current_page
            .and_then(|p| self.backend.document().and_then(|d| d.project().page(p)))
            .map(|p| p.structure.clone())
            .unwrap_or_default();
        let mut chosen = None;
        let mut close = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape));
        let area = egui::Area::new(egui::Id::new("occurrences"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_TOP)
            .fixed_pos(egui::pos2(canvas.center().x, canvas.top() + 12.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_min_width(280.0);
                    ui.small("EVERY OCCURRENCE");
                    for r in list {
                        let text = r.row_from(&reader);
                        if ui.selectable_label(false, text).clicked() {
                            chosen = Some(r.clone());
                        }
                    }
                });
            });
        if area.response.clicked_elsewhere() {
            close = true;
        }
        if let Some(r) = chosen {
            if let Some(here) = self.here() {
                let to = self.wb.nav.jump(here, &r);
                self.show_page(to.page);
                self.wb.pending = Some(to);
            }
            close = true;
        }
        if close {
            self.wb.occurrences = None;
        }
    }
}

/// The cross-reference index for the open project, rebuilt when stale.
/// Takes the app's parts, so the rest of the app stays usable.
pub(super) fn xref_of<'a>(
    backend: &'a dyn crate::backend::ProjectBackend,
    canvas: &'a dyn crate::canvas::CanvasView,
    live: &'a mut LiveXref,
) -> Option<&'a XrefIndex> {
    let project = backend.document()?.project();
    Some(match canvas.schematic_ref() {
        Some(view) => live.get(project, view.editor.catalog.library()),
        None => live.get(project, &novo_xref::FromKey),
    })
}

/// A cross-reference label as a note drawn next to its symbol: a coil's
/// contact image becomes the contact table, its warnings stay text below.
pub(crate) fn note(label: Label, density: NoteDensity) -> SymbolNote {
    let rows: Vec<NoteRow> = label
        .rows
        .iter()
        .map(|r| NoteRow {
            glyph: glyph_of(r.kind),
            pins: r.pins.clone(),
            location: r.location.clone(),
        })
        .collect();
    let lines = if rows.is_empty() {
        label.lines
    } else {
        label.extra_lines().to_vec()
    };
    SymbolNote {
        symbol: label.symbol,
        lines,
        warning: label.warning,
        anchor: match label.anchor {
            LabelAnchor::Below => NoteAnchor::Below,
            LabelAnchor::Right => NoteAnchor::Right,
        },
        rows,
        density,
    }
}

/// The contact symbol a row of the table draws.
pub(crate) fn glyph_of(kind: ContactKind) -> NoteGlyph {
    match kind {
        ContactKind::Power => NoteGlyph::Power,
        ContactKind::NormallyClosed => NoteGlyph::NormallyClosed,
        ContactKind::Changeover => NoteGlyph::Changeover,
        ContactKind::NormallyOpen | ContactKind::Other => NoteGlyph::NormallyOpen,
    }
}
