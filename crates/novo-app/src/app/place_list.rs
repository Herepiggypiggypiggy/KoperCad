//! Place from a pasted list (Ctrl+K, `place list`): paste lines such as
//! `K1 contactor 25A` or `F1 mcb 3p 16A x5` (or cells copied from Excel) and
//! place all the devices at once, on a grid beside what is drawn, tagged,
//! with their parts when the line names one, as one undo step. The window
//! previews every line first; lines that name nothing are shown and left
//! out. The reading of the text is `novo_parts::place_list`.

use std::collections::HashMap;

use eframe::egui;
use novo_core::{DeviceTag, PageId, PartId, Placement, Point, Project, SymbolId, SymbolRole, Tx};
use novo_editor::catalog::{SymbolClass, classify};
use novo_editor::edit::{self, PlaceOptions};
use novo_parts::place_list::{self, Outcome, Resolved};
use novo_parts::{Catalog, PartKey, find_project_part};
use novo_symbols::Library;
use novo_workflow::macro_place::{self, Bounds, PlaceView};

use super::{Level, NovoApp};
use crate::theme::ui_palette;

/// Gap between cells, in mm.
const GAP: (f64, f64) = (16.0, 14.0);

/// What placing (or previewing) a list did.
#[derive(Debug, Default, Clone, PartialEq)]
pub(super) struct PlaceReport {
    pub(super) devices: usize,
    pub(super) with_part: usize,
    /// Typed tags that were already used; those devices were numbered
    /// automatically instead.
    pub(super) clashes: Vec<String>,
    pub(super) symbols: Vec<SymbolId>,
}

/// Places every placeable line on `page` in a grid, in the caller's
/// transaction (one undo step).
pub(super) fn place_lines(
    tx: &mut Tx<'_>,
    page: PageId,
    lines: &[Resolved],
    catalog: &Catalog,
    library: &Library,
    view: &PlaceView,
) -> novo_core::Result<PlaceReport> {
    let todo: Vec<(&Resolved, &novo_symbols::Symbol)> = lines
        .iter()
        .filter_map(|l| Some((l, library.get(l.symbol()?)?)))
        .collect();
    let mut report = PlaceReport::default();
    let total: usize = todo.iter().map(|(l, _)| l.wanted.qty as usize).sum();
    if total == 0 {
        return Ok(report);
    }
    let (mut cell_w, mut cell_h) = (0.0_f64, 0.0_f64);
    for (_, s) in &todo {
        let (lo, hi) = s.bounds();
        cell_w = cell_w.max(hi[0] - lo[0]);
        cell_h = cell_h.max(hi[1] - lo[1]);
    }
    let round5 = |v: f64| (v / 5.0).ceil() * 5.0;
    let cell = (
        round5(cell_w.max(4.0) + GAP.0),
        round5(cell_h.max(4.0) + GAP.1),
    );
    let sheet_w = view.sheet.map_or(420.0, |s| s.0);
    let cols = (((sheet_w - 40.0) / cell.0).floor() as usize)
        .clamp(1, 12)
        .min(total);
    let rows = total.div_ceil(cols);
    let bounds = Bounds::at_origin(cols as f64 * cell.0, rows as f64 * cell.1);
    let (origin, _) = macro_place::origin_in_view(tx.project(), page, bounds, view, 20.0);

    let mut parts: HashMap<PartKey, PartId> = HashMap::new();
    let mut index = 0usize;
    for (line, symbol) in todo {
        let (lo, _) = symbol.bounds();
        let main = matches!(classify(symbol), SymbolClass::Main { .. });
        for n in 0..line.wanted.qty {
            let corner = Point::new(
                origin.x + (index % cols) as f64 * cell.0,
                origin.y + (index / cols) as f64 * cell.1,
            );
            index += 1;
            let mut options = PlaceOptions::new();
            options.auto_connect = false;
            if let Some(tag) = line.wanted.tag_for(n) {
                let taken = tx
                    .project()
                    .devices()
                    .any(|d| d.tag.product.last().map(String::as_str) == Some(tag.as_str()));
                if !main {
                    // A contact or other function joins the device it names.
                    options.parent = DeviceTag::parse(&format!("-{tag}")).ok();
                } else if taken {
                    report.clashes.push(tag);
                } else {
                    options.parent = DeviceTag::parse(&format!("-{tag}")).ok();
                }
            }
            let placement = Placement::at(Point::new(corner.x - lo[0], corner.y - lo[1]));
            let id = edit::place(tx, page, symbol, placement, &options)?;
            report.devices += 1;
            report.symbols.push(id);
            let Outcome::Part { key, .. } = &line.outcome else {
                continue;
            };
            let Some(SymbolRole::Device { device, .. }) =
                tx.project().symbol(id).map(|s| s.role.clone())
            else {
                continue;
            };
            let part = match parts.get(key) {
                Some(&p) => p,
                None => {
                    let Some(entry) = catalog.get(key) else {
                        continue;
                    };
                    let id = match find_project_part(tx.project(), key) {
                        Some(p) => p,
                        None => tx.insert(|id| entry.to_model(id))?,
                    };
                    parts.insert(key.clone(), id);
                    id
                }
            };
            tx.assign_part(device, part)?;
            report.with_part += 1;
        }
    }
    Ok(report)
}

/// What the window worked out for one text.
struct Preview {
    text: String,
    revision: u64,
    lines: Vec<Resolved>,
    report: Result<PlaceReport, String>,
}

/// The window while it is open.
#[derive(Default)]
pub(crate) struct PlaceListWindow {
    pub(super) text: String,
    preview: Option<Preview>,
    focus: bool,
}

#[cfg(test)]
impl PlaceListWindow {
    pub(super) fn set_text(&mut self, text: &str) {
        self.text = text.to_owned();
    }
}

fn symbol_id_of(library: &Library, part: &novo_parts::CatalogPart) -> Option<String> {
    crate::place_palette::symbol_for_part(library, part).map(|s| s.id.clone())
}

impl NovoApp {
    pub(super) fn open_place_list(&mut self) {
        if self.backend.document().is_some() && self.current_page.is_some() {
            self.wb.place_list = Some(PlaceListWindow {
                focus: true,
                ..PlaceListWindow::default()
            });
        }
    }

    /// Reads `text` against the catalog and library and dry-runs the placement.
    pub(super) fn place_list_preview(
        &self,
        text: &str,
    ) -> Option<(Vec<Resolved>, Result<PlaceReport, String>)> {
        let catalog = self.wb.part_catalog.get()?;
        let library = self.library()?;
        let page = self.current_page?;
        let project: &Project = self.backend.document()?.project();
        let lines = place_list::resolve(place_list::parse(text), catalog, library, &|p| {
            symbol_id_of(library, p)
        });
        let view = self.macro_view(Some(page));
        let mut copy = project.clone();
        let report = copy
            .transact("Preview place list", |tx| {
                place_lines(tx, page, &lines, catalog, library, &view)
            })
            .map(|(r, _)| r)
            .map_err(|e| e.to_string());
        Some((lines, report))
    }

    /// Places the previewed lines, as one undo step.
    pub(super) fn place_list_apply(&mut self, lines: &[Resolved]) -> bool {
        let Some(page) = self.current_page else {
            return false;
        };
        let view = self.macro_view(Some(page));
        let (Some(catalog), Some(library), Some(doc)) = (
            self.wb.part_catalog.get(),
            self.canvas
                .schematic_ref()
                .map(|v| v.editor.catalog.library()),
            self.backend.document_mut(),
        ) else {
            return false;
        };
        let result = doc.transact("Place from list", |tx| {
            place_lines(tx, page, lines, catalog, library, &view)
        });
        match result {
            Ok((report, changes)) => {
                self.changed_outside_canvas(&changes);
                let ids: Vec<novo_core::AnyId> = report.symbols.iter().map(|&s| s.into()).collect();
                if let Some(view) = self.canvas.schematic() {
                    view.select_ids(&ids);
                }
                let mut text = format!("Placed {} devices", report.devices);
                if !report.clashes.is_empty() {
                    text.push_str(&format!(
                        "; {} already in use, numbered automatically",
                        report.clashes.join(", ")
                    ));
                }
                self.message(Level::Info, text);
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not place the list: {e}"));
                false
            }
        }
    }

    pub(super) fn place_list_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.place_list.take() else {
            return;
        };
        if let Some(note) = self.wb.part_catalog.ensure() {
            self.message(Level::Warning, note);
        }
        let revision = self.wb.part_catalog.get().map_or(0, Catalog::revision);
        let stale = win
            .preview
            .as_ref()
            .is_none_or(|p| p.text != win.text || p.revision != revision);
        if stale {
            win.preview = self
                .place_list_preview(&win.text)
                .map(|(lines, report)| Preview {
                    text: win.text.clone(),
                    revision,
                    lines,
                    report,
                });
        }
        let symbol_name: HashMap<String, String> = match (self.library(), &win.preview) {
            (Some(library), Some(preview)) => preview
                .lines
                .iter()
                .filter_map(|l| {
                    let id = l.symbol()?;
                    Some((id.to_owned(), library.get(id)?.name.clone()))
                })
                .collect(),
            _ => HashMap::new(),
        };
        let mut open = true;
        let mut place = false;
        let p = ui_palette();
        super::panel::dialog(ctx, "Place from a list", [640.0, 540.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("Parts list");
                    novo_theme::widgets::help_mark(
                        ui,
                        "One device per line: an optional tag, an article number or a symbol word, \
and a quantity. Examples: `K1 contactor 25A`, `F1 mcb 3p 16A x5`, `3RT2016-1BB41`, `4x LC1D09`. \
Cells copied from Excel work too (quantity in its own column). A tag counts up with the quantity \
(F1 x3 gives F1, F2, F3). A part is assigned only when exactly one fits; otherwise the device is \
placed without one. Lines that name nothing are left out.",
                    );
                });
                let edit = egui::TextEdit::multiline(&mut win.text)
                    .desired_rows(6)
                    .desired_width(f32::INFINITY)
                    .hint_text("K1 contactor 25A\nF1 mcb 3p 16A x5")
                    .code_editor();
                let response = ui.add(edit);
                if win.focus {
                    response.request_focus();
                    win.focus = false;
                }
                ui.separator();
                let Some(preview) = &win.preview else {
                    ui.weak("Open a schematic page first.");
                    return;
                };
                let devices = place_list::device_count(&preview.lines);
                let left_out = preview.lines.iter().filter(|l| !l.placeable()).count();
                ui.horizontal_wrapped(|ui| {
                    match &preview.report {
                        Ok(r) => {
                            ui.label(format!("{devices} devices, {} with a part", r.with_part));
                            if !r.clashes.is_empty() {
                                ui.colored_label(
                                    p.warning,
                                    format!("{} in use", r.clashes.join(", ")),
                                );
                            }
                        }
                        Err(e) => {
                            ui.colored_label(p.error, e);
                        }
                    }
                    if left_out > 0 {
                        ui.colored_label(p.warning, format!("{left_out} left out"));
                    }
                });
                let ready = devices > 0 && preview.report.is_ok();
                let button =
                    egui::Button::new(format!("Place {devices}")).min_size(egui::vec2(140.0, 28.0));
                if ui.add_enabled(ready, button).clicked() {
                    place = true;
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in &preview.lines {
                        let name = match &line.wanted.tag {
                            Some(t) => format!("{}×  {t}  {}", line.wanted.qty, line.wanted.text),
                            None => format!("{}×  {}", line.wanted.qty, line.wanted.text),
                        };
                        ui.add(egui::Label::new(name).wrap());
                        let (color, text) = match &line.outcome {
                            Outcome::Part { key, .. } => (
                                p.success,
                                format!("{} {}", key.manufacturer(), key.order_number()),
                            ),
                            Outcome::Symbol { symbol, fitting } => {
                                let symbol = symbol_name.get(symbol).unwrap_or(symbol);
                                let note = match fitting {
                                    0 => String::new(),
                                    n => format!("  ({n} parts fit)"),
                                };
                                (p.text_secondary, format!("{symbol}{note}"))
                            }
                            Outcome::Unknown(why) => (p.error, (*why).to_owned()),
                        };
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("   {text}")).color(color),
                            )
                            .wrap(),
                        );
                        ui.add_space(2.0);
                    }
                });
            });
        if place
            && let Some(preview) = win.preview.take()
            && self.place_list_apply(&preview.lines)
        {
            return;
        }
        if open {
            self.wb.place_list = Some(win);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::gate1_tests::Drive;
    use crate::actions::Action;

    #[test]
    fn a_pasted_list_places_tagged_devices_as_one_undo_step() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        assert!(super::super::workbench::app_runs("place.list"));
        d.app.run_command(&ctx, "place.list", None);
        assert!(d.app.wb.place_list.is_some());
        let mut win = d.app.wb.place_list.take().unwrap();
        win.set_text("F1 mcb x3\nnonsense words here\nK1 contactor\n");
        d.app.wb.place_list = Some(win);
        d.frame();
        d.frame();
        let preview = d.app.wb.place_list.as_ref().unwrap();
        assert!(preview.preview.is_some(), "the window worked the list out");
        let lines = preview.preview.as_ref().unwrap().lines.clone();
        assert_eq!(lines.len(), 3);
        assert!(!lines[1].placeable());

        let before = d
            .app
            .backend
            .document()
            .unwrap()
            .project()
            .devices()
            .count();
        assert!(d.app.place_list_apply(&lines));
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(project.devices().count(), before + 4);
        let mut tags: Vec<String> = project.devices().map(|x| x.tag.to_string()).collect();
        tags.sort();
        for want in ["-F1", "-F2", "-F3", "-K1"] {
            assert!(tags.iter().any(|t| t == want), "{want} in {tags:?}");
        }
        // One undo step takes all four away.
        d.app.run(&ctx, Action::Undo);
        let project = d.app.backend.document().unwrap().project();
        assert_eq!(project.devices().count(), before);
    }

    #[test]
    fn an_article_number_places_its_device_with_the_part() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        d.app.run_command(&ctx, "place.list", None);
        d.frame();
        let order = {
            let catalog = d
                .app
                .wb
                .part_catalog
                .get()
                .expect("catalog loaded by the window");
            let library = d.app.library().expect("library");
            catalog
                .parts()
                .iter()
                .find(|p| super::symbol_id_of(library, p).is_some())
                .map(|p| p.order_number.clone())
                .expect("a catalog part with a symbol")
        };
        let mut win = d.app.wb.place_list.take().unwrap();
        win.set_text(&format!("{order} x2"));
        d.app.wb.place_list = Some(win);
        d.frame();
        d.frame();
        let lines = d
            .app
            .wb
            .place_list
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap()
            .lines
            .clone();
        assert!(d.app.place_list_apply(&lines));
        let project = d.app.backend.document().unwrap().project();
        let with_part = project.devices().filter(|x| !x.parts.is_empty()).count();
        assert_eq!(with_part, 2);
        // Both devices share one project part.
        assert_eq!(project.parts().count(), 1);
    }

    fn snapshot(d: &Drive) -> String {
        // The records only: undo does not rewind the id counter, revision or caches.
        let text = format!("{:?}", d.app.backend.document().unwrap().project());
        let from = text.find(" pages: ").expect("pages field");
        let to = text.find(", index: ").expect("index field");
        text[from..to].to_owned()
    }

    fn open_window(d: &mut Drive, text: &str) -> Vec<novo_parts::place_list::Resolved> {
        let ctx = d.ctx.clone();
        d.app.run_command(&ctx, "place.list", None);
        d.frame();
        let mut win = d.app.wb.place_list.take().unwrap();
        win.set_text(text);
        d.app.wb.place_list = Some(win);
        d.frame();
        d.frame();
        d.app
            .wb
            .place_list
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap()
            .lines
            .clone()
    }

    #[test]
    fn undo_restores_the_project_and_a_preview_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        let lines = open_window(&mut d, "F1 mcb x3\nK1 contactor\n");
        d.frame();
        let start = snapshot(&d);
        // A preview (the window's own dry run) leaves the project as it was.
        let (_, report) = d
            .app
            .place_list_preview("F1 mcb x3\nK1 contactor\n")
            .unwrap();
        assert_eq!(report.unwrap().devices, 4);
        assert_eq!(snapshot(&d), start);
        assert!(d.app.place_list_apply(&lines));
        assert_ne!(snapshot(&d), start);
        d.app.run(&ctx, Action::Undo);
        assert_eq!(snapshot(&d), start);
    }

    #[test]
    fn a_tag_already_used_is_numbered_automatically() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::NewProject);
        d.frame();
        let first = open_window(&mut d, "K1 contactor");
        assert!(d.app.place_list_apply(&first));
        let again = open_window(&mut d, "K1 contactor");
        // The preview already reports the clash.
        let report = d
            .app
            .wb
            .place_list
            .as_ref()
            .unwrap()
            .preview
            .as_ref()
            .unwrap()
            .report
            .clone();
        assert_eq!(report.unwrap().clashes, vec!["K1".to_string()]);
        assert!(d.app.place_list_apply(&again));
        let project = d.app.backend.document().unwrap().project();
        let mut tags: Vec<String> = project.devices().map(|x| x.tag.to_string()).collect();
        tags.sort();
        assert_eq!(tags.len(), 2, "{tags:?}");
        assert_eq!(tags.iter().filter(|t| *t == "-K1").count(), 1, "{tags:?}");
    }
}
