//! Cross references in the app (novo-xref, WS-10): labels next to coils
//! and contacts, G / Shift+G, Alt+Left / Alt+Right and jumps to a device.

use eframe::egui;
use novo_canvas::ItemId;
use novo_canvas::model::{
    NoteAnchor, NoteDensity, NoteFade, NoteGlyph, NoteRow, SymbolNote, item_id,
};
use novo_core::{AnyId, DeviceId, PageId, Properties, SymbolId, SymbolRole, Value};
use novo_xref::{ContactKind, Label, LabelAnchor, LiveXref, Place, Reference, XrefIndex};
use std::collections::{BTreeMap, HashSet};

use super::{Level, NovoApp};
use crate::settings::XrefView;
use crate::theme;

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
        let hovered = std::mem::take(&mut self.wb.hover_preview);
        if let Some(view) = self.canvas.schematic() {
            let lit = view.where_lit();
            view.canvas.set_related(
                related
                    .into_iter()
                    .map(|s| item_id(AnyId::Symbol(s)))
                    .chain(lit)
                    .chain(hovered.into_iter().map(item_id)),
            );
        }
    }

    /// The cross-reference labels of a page.
    pub(super) fn xref_labels(&mut self, page: PageId) -> Vec<Label> {
        xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref)
            .map(|index| index.labels_on_page(page))
            .unwrap_or_default()
    }

    /// The cross-reference labels of a page, as notes for the page scene,
    /// as the View > Layers > Cross references choice shows them and with
    /// what the page and its coils hide left out.
    pub(super) fn xref_notes(&mut self, page: PageId) -> Vec<SymbolNote> {
        // The strength of a fade is a setting; the canvas restyles when it changes.
        if theme::set_xref_fade_alpha(self.settings.xref_fade_alpha()) {
            self.canvas.restyle();
        }
        let view = self.settings.xref_view();
        if view == XrefView::Hidden || self.xref_hidden_on(page) {
            return Vec::new();
        }
        let (density, spare) = (self.settings.xref_density(), !self.settings.xref_hide_spare);
        let fade = match view {
            XrefView::Overlap => NoteFade::WhereCrossed,
            XrefView::Faded => NoteFade::Faded,
            XrefView::Full | XrefView::Hidden => NoteFade::Clear,
        };
        let awake = if fade == NoteFade::Clear {
            HashSet::new()
        } else {
            self.awake_notes()
        };
        let hidden = self.devices_hiding_references();
        let labels = self.xref_labels(page);
        let project = self.backend.document().map(|d| d.project());
        labels
            .into_iter()
            .filter(|l| {
                let device = project
                    .and_then(|p| p.symbol(l.symbol))
                    .and_then(|s| device_of(&s.role));
                !device.is_some_and(|d| hidden.contains(&d))
            })
            .map(|l| {
                let fade = if awake.contains(&item_id(l.symbol)) {
                    NoteFade::Clear
                } else {
                    fade
                };
                SymbolNote {
                    fade,
                    ..note(l, density, spare)
                }
            })
            .collect()
    }

    /// Whether the page hides its cross references on screen.
    fn xref_hidden_on(&self, page: PageId) -> bool {
        self.backend
            .document()
            .and_then(|d| d.project().page(page))
            .is_some_and(|p| is_hidden(&p.properties))
    }

    /// Whether the open page hides its cross references (a status bar chip).
    pub(super) fn page_hides_references(&self) -> bool {
        self.current_page.is_some_and(|p| self.xref_hidden_on(p))
    }

    /// The tag of the selected symbol's device when it hides its cross
    /// references (a status bar chip), so a hidden table can be found again.
    pub(super) fn selected_device_hiding_references(&self) -> Option<String> {
        let project = self.backend.document()?.project();
        let device = device_of(&project.symbol(self.selected_symbol()?)?.role)?;
        let device = project.device(device)?;
        is_hidden(&device.properties).then(|| device.tag.to_string())
    }

    /// The devices whose cross references are hidden on screen.
    fn devices_hiding_references(&self) -> HashSet<DeviceId> {
        self.backend
            .document()
            .map(|d| {
                d.project()
                    .devices()
                    .filter(|dev| is_hidden(&dev.properties))
                    .map(|dev| dev.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Shift+X: cross references full, then faded where they overlap, then
    /// faded, then hidden, then full again.
    pub(super) fn cycle_xref_view(&mut self) {
        let next = self.settings.xref_view().next();
        self.settings.xref_view = next.id().into();
        self.message(
            Level::Info,
            match next {
                XrefView::Full => "Cross references shown in full",
                XrefView::Overlap => {
                    "Cross references fade where they overlap something; select or hover one to read it"
                }
                XrefView::Faded => "Cross references faded; select or hover one to read it",
                XrefView::Hidden => {
                    "Cross references hidden (they still print). Shift+X shows them"
                }
            },
        );
    }

    /// Hides or shows the cross references of the selected device on
    /// screen (the contact table under its coil and the coil location next
    /// to its contacts): one undo step, kept in the project as `xref.hide`
    /// on the device. The PDF still prints them.
    pub(super) fn toggle_device_references(&mut self) {
        let device = self.selected_symbol().and_then(|s| {
            self.backend
                .document()
                .and_then(|d| d.project().symbol(s))
                .and_then(|s| device_of(&s.role))
        });
        let Some(device) = device else {
            self.message(
                Level::Info,
                "Select a coil or contact first to hide or show its cross references",
            );
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().device(device) else {
            return;
        };
        let hide = !is_hidden(&old.properties);
        let label = format!(
            "{} cross references of {}",
            if hide { "Hide" } else { "Show" },
            old.tag
        );
        let tag = old.tag.to_string();
        match doc.transact(label, |tx| {
            tx.set_property(device, XREF_HIDE, hide.then_some(Value::Bool(true)))
        }) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    if hide {
                        format!("Cross references of {tag} hidden on screen (they still print)")
                    } else {
                        format!("Cross references of {tag} shown")
                    },
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// Hides or shows the cross references of the page on screen: one undo
    /// step, kept in the project as `xref.hide` on the page. The PDF still
    /// prints them.
    pub(super) fn toggle_page_references(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let Some(old) = doc.project().page(page) else {
            return;
        };
        let hide = !is_hidden(&old.properties);
        let label = format!(
            "{} cross references of page {}",
            if hide { "Hide" } else { "Show" },
            old.name
        );
        match doc.transact(label, |tx| {
            tx.set_property(page, XREF_HIDE, hide.then_some(Value::Bool(true)))
        }) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                self.message(
                    Level::Info,
                    if hide {
                        "Cross references of this page hidden on screen (they still print)"
                    } else {
                        "Cross references of this page shown"
                    },
                );
            }
            Err(e) => self.message(Level::Error, format!("Could not change it: {e}")),
        }
    }

    /// The symbols whose cross references stay at full opacity while the
    /// rest are faded: the selected symbol, what it is linked to on this
    /// page, and the one under the pointer.
    fn awake_notes(&mut self) -> HashSet<ItemId> {
        let mut awake: HashSet<ItemId> = HashSet::new();
        if let Some(symbol) = self.selected_symbol() {
            awake.insert(item_id(symbol));
            if let Some(index) = xref_of(&*self.backend, &*self.canvas, &mut self.wb.xref) {
                awake.extend(index.highlight(symbol).on_page.into_iter().map(item_id));
            }
        }
        if let Some(view) = self.canvas.schematic() {
            awake.extend(view.canvas.hovered());
        }
        awake
    }

    /// Every page's cross-reference notes, for the PDF.
    pub(super) fn xref_notes_by_page(&mut self) -> BTreeMap<PageId, Vec<SymbolNote>> {
        let pages: Vec<PageId> = self
            .backend
            .document()
            .map(|d| d.project().pages().map(|p| p.id).collect())
            .unwrap_or_default();
        let (density, spare) = (self.settings.xref_density(), !self.settings.xref_hide_spare);
        let Some(index) = self.xref() else {
            return BTreeMap::new();
        };
        pages
            .into_iter()
            .map(|p| {
                let notes = index
                    .labels_on_page(p)
                    .into_iter()
                    .map(|l| note(l, density, spare))
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

    /// A double click on a cross reference: a row of a coil's contact table
    /// jumps to that contact, a contact's reference to its coil. Alt+Left
    /// comes back.
    pub(super) fn follow_note_jump(&mut self) {
        let Some(jump) = self.canvas.schematic().and_then(|v| v.take_note_jump()) else {
            return;
        };
        let place = {
            let Some(index) = self.xref() else {
                return;
            };
            match jump.to {
                Some(to) => index.reference(to).map(Place::of),
                None => index.partners(jump.from).first().map(|r| Place::of(r)),
            }
        };
        match place {
            Some(place) => self.wb.deferred_jump = Some(place),
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
        let project = self.backend.document().map(|d| d.project());
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
                        let title = project
                            .and_then(|p| p.page(r.location.page))
                            .map(|p| p.title.as_str())
                            .unwrap_or_default();
                        let text = r.list_row_from(&reader, title);
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
/// contact image becomes the contact table (its warnings stay text below),
/// followed by the spare contacts when `spare` is set.
pub(crate) fn note(label: Label, density: NoteDensity, spare: bool) -> SymbolNote {
    let mut rows: Vec<NoteRow> = label
        .rows
        .iter()
        .map(|r| NoteRow {
            target: Some(r.symbol),
            ..NoteRow::new(glyph_of(r.kind), &r.pins, &r.location)
        })
        .collect();
    let mut lines = if rows.is_empty() {
        label.lines.clone()
    } else {
        label.extra_lines().to_vec()
    };
    if !rows.is_empty() && spare {
        lines.extend(label.spare_more_line());
        rows.extend(label.spare.iter().map(|r| NoteRow {
            spare: true,
            ..NoteRow::new(glyph_of(r.kind), &r.pins, "spare")
        }));
    }
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
        fade: Default::default(),
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

/// The property that hides cross references on screen, on a page or a
/// device (it is not printed differently: the PDF always prints them).
pub(crate) const XREF_HIDE: &str = "xref.hide";

fn is_hidden(properties: &Properties) -> bool {
    matches!(properties.get(XREF_HIDE), Some(Value::Bool(true)))
}

fn device_of(role: &SymbolRole) -> Option<DeviceId> {
    match role {
        SymbolRole::Device { device, .. } => Some(*device),
        _ => None,
    }
}
