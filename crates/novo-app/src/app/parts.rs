//! The Parts window: browse the part catalog (the starter set, the user's
//! own parts and everything imported), filter by maker, and read a part's
//! dimensions, power loss, clearances and connection data. Each row says
//! whether the part is in the local library or can be imported.
//!
//! The window is a thin view over `novo-parts`: rows come from
//! [`browser_rows`], the library and its source records from
//! [`LocalLibrary`]. Design: `docs/PARTS-LIBRARY-DESIGN.md` (section 3a).

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_core::Value;
use novo_parts::{
    Catalog, CatalogPart, ImportFlow, LocalLibrary, PartKey, RowAction, SourceRecord, SourceTable,
    browser_rows, default_downloads_dir, display_value, is_internal_key, property_label,
    user_catalog_path,
};

use super::parts_import::{Import, Phase, import_path};
use super::revisions::today;
use super::{Level, NovoApp};

/// State of the open Parts window.
pub(crate) struct PartsWindow {
    query: String,
    /// Only parts of this maker; `None` shows all.
    maker: Option<String>,
    selected: Option<PartKey>,
    catalog: Catalog,
    library: Option<LocalLibrary>,
    sources: SourceTable,
    /// The import in progress or just finished, shown in the status strip.
    import: Option<Import>,
    /// The folder the Downloads watch looks in.
    downloads: Option<PathBuf>,
    /// The user's catalog file, where read data is saved.
    catalog_path: Option<PathBuf>,
}

impl PartsWindow {
    pub(crate) fn new(
        catalog: Catalog,
        library: Option<LocalLibrary>,
        sources: SourceTable,
    ) -> Self {
        PartsWindow {
            query: String::new(),
            maker: None,
            selected: None,
            catalog,
            library,
            sources,
            import: None,
            downloads: default_downloads_dir(),
            catalog_path: user_catalog_path(),
        }
    }

    /// The part a dropped or picked file belongs to: the one being
    /// imported, else the selected one.
    fn target(&self) -> Option<CatalogPart> {
        let key = self
            .import
            .as_ref()
            .map(|i| i.key.clone())
            .or_else(|| self.selected.clone())?;
        self.catalog.get(&key).cloned()
    }

    fn start_import(&mut self, key: &PartKey) {
        let (Some(part), Some(library)) = (self.catalog.get(key), self.library.as_ref()) else {
            return;
        };
        self.import = Some(Import::start(
            part,
            library,
            &self.sources,
            self.downloads.as_deref(),
            &today(),
        ));
    }

    /// Imports a file for the target part and records the result.
    fn import_file(&mut self, path: &Path, flow: ImportFlow) {
        let Some(part) = self.target() else {
            return;
        };
        if self.import.as_ref().is_none_or(|i| i.key != part.key()) {
            self.start_import(&part.key());
        }
        let Some(library) = self.library.as_ref() else {
            return;
        };
        let result = import_path(
            &part,
            path,
            flow,
            library,
            &self.sources,
            self.catalog_path.as_deref(),
            &today(),
        );
        let phase = match result {
            Ok((summary, changed)) => {
                for p in changed {
                    self.catalog.upsert(p);
                }
                Phase::Done(summary)
            }
            Err(e) => Phase::Failed(e),
        };
        if let Some(i) = self.import.as_mut() {
            i.phase = phase;
        }
    }

    /// The makers in the catalog, sorted, each once.
    pub(crate) fn makers(&self) -> Vec<String> {
        let mut makers: Vec<String> = Vec::new();
        for part in self.catalog.parts() {
            if !makers
                .iter()
                .any(|m| m.eq_ignore_ascii_case(&part.manufacturer))
            {
                makers.push(part.manufacturer.clone());
            }
        }
        makers.sort_by_key(|m| m.to_lowercase());
        makers
    }

    /// The rows the search text and maker filter leave, as keys with their
    /// button.
    pub(crate) fn rows(&self) -> Vec<(PartKey, RowAction)> {
        let none = LocalLibrary::at(std::path::PathBuf::new());
        let library = self.library.as_ref().unwrap_or(&none);
        browser_rows(&self.catalog, library, &self.sources, &self.query)
            .into_iter()
            .filter(|r| {
                self.maker
                    .as_ref()
                    .is_none_or(|m| m.eq_ignore_ascii_case(&r.part.manufacturer))
            })
            .map(|r| (r.part.key(), r.action))
            .collect()
    }

    /// Whether the part's files are in the library. A missing library
    /// (no profile folder) means nothing is.
    fn in_library(&self, key: &PartKey) -> bool {
        self.library.as_ref().is_some_and(|l| l.has(key))
    }

    fn records(&self, key: &PartKey) -> Vec<SourceRecord> {
        self.library
            .as_ref()
            .and_then(|l| l.records_for(key).ok())
            .unwrap_or_default()
    }
}

/// One titled block of the part detail.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DetailSection {
    pub title: &'static str,
    /// (label, value with unit)
    pub rows: Vec<(String, String)>,
}

const DIMENSIONS: &str = "Dimensions";
const POWER_LOSS: &str = "Power loss";
const CLEARANCES: &str = "Clearances";
const CONNECTIONS: &str = "Connections";
const TECHNICAL: &str = "Technical data";

/// Which block a data key belongs to: by words in the key, so the keys the
/// power loss and clearance work adds (`power_loss_w`, `clearance_side_mm`,
/// ...) land in their block without this list knowing each one.
fn section_of(key: &str) -> &'static str {
    let k = key.to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| k.contains(w));
    if has(&["power_loss", "dissipation", "heat_loss"]) {
        POWER_LOSS
    } else if has(&["clearance", "mounting_distance", "spacing"]) {
        CLEARANCES
    } else if has(&[
        "wire",
        "conductor",
        "torque",
        "strip",
        "connection",
        "terminal",
        "screw",
        "push_in",
    ]) {
        CONNECTIONS
    } else if has(&["width", "height", "depth", "length", "diameter"]) {
        DIMENSIONS
    } else {
        TECHNICAL
    }
}

fn with_unit(value: String, unit: Option<&str>) -> String {
    match unit {
        Some(u) if !value.is_empty() => format!("{value} {u}"),
        _ => value,
    }
}

fn mm(v: f64) -> String {
    format!("{v}")
}

/// The detail of a part, grouped: dimensions first, then power loss,
/// clearances, connection data and the rest. Empty blocks are left out.
pub(crate) fn detail_sections(part: &CatalogPart) -> Vec<DetailSection> {
    let mut blocks: Vec<DetailSection> =
        [DIMENSIONS, POWER_LOSS, CLEARANCES, CONNECTIONS, TECHNICAL]
            .into_iter()
            .map(|title| DetailSection {
                title,
                rows: Vec::new(),
            })
            .collect();
    if let Some(d) = part.dimensions {
        blocks[0].rows.push((
            "Width \u{d7} height \u{d7} depth".into(),
            format!(
                "{} \u{d7} {} \u{d7} {} mm",
                mm(d.width_mm),
                mm(d.height_mm),
                mm(d.depth_mm)
            ),
        ));
    }
    for (key, value) in &part.data {
        if is_internal_key(key) || key == "letter_codes" {
            continue;
        }
        let (label, unit) = property_label(key);
        let text = match value {
            Value::Text(t) if t.is_empty() => continue,
            other => with_unit(display_value(other), unit),
        };
        let title = section_of(key);
        let block = blocks
            .iter_mut()
            .find(|b| b.title == title)
            .expect("every title has a block");
        block.rows.push((label, text));
    }
    blocks.retain(|b| !b.rows.is_empty());
    blocks
}

/// What a click in the window asks for; run after drawing, so drawing only
/// reads.
enum PartsAction {
    Select(PartKey),
    Import(PartKey),
    OpenPage(String),
    ChooseFile,
    ChooseFolder,
    StopImport,
    Remove(PartKey),
    Close,
}

impl NovoApp {
    /// Opens the Parts window with the catalog and library as they are now.
    pub(super) fn open_parts_browser(&mut self) {
        if self.wb.parts.is_some() {
            return;
        }
        let catalog = match Catalog::load_default() {
            Ok(c) => c,
            Err(e) => {
                self.message(Level::Error, format!("Parts catalog: {e}"));
                Catalog::starter()
            }
        };
        self.wb.parts = Some(PartsWindow::new(
            catalog,
            LocalLibrary::open_default(),
            SourceTable::builtin(),
        ));
    }

    pub(super) fn parts_window(&mut self, ctx: &egui::Context) {
        let Some(win) = self.wb.parts.as_mut() else {
            return;
        };
        let mut open = true;
        let mut actions = Vec::new();
        let dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
        egui::Window::new("Parts")
            .open(&mut open)
            .default_size([820.0, 520.0])
            .show(ctx, |ui| parts_ui(ui, win, dragging, &mut actions));
        // A file dropped on the window, or one the watch found.
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        if let Some(path) = dropped.first() {
            win.import_file(path, ImportFlow::Manual);
        }
        let now = ctx.input(|i| i.time);
        if let Some(path) = win.import.as_mut().and_then(|i| i.poll(now)) {
            win.import_file(&path, ImportFlow::Downloads);
        }
        if win
            .import
            .as_ref()
            .is_some_and(|i| i.phase == Phase::Waiting)
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                super::parts_import::POLL_SECONDS,
            ));
        }
        let mut close = !open;
        let mut error = None;
        for action in actions {
            match action {
                PartsAction::Select(key) => win.selected = Some(key),
                PartsAction::Import(key) => {
                    win.start_import(&key);
                    if let Some(url) = win.import.as_ref().and_then(|i| i.page_url.clone()) {
                        ctx.open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                PartsAction::OpenPage(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
                PartsAction::ChooseFile => {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Choose the downloaded file")
                        .pick_file()
                    {
                        win.import_file(&path, ImportFlow::Manual);
                    }
                }
                PartsAction::ChooseFolder => {
                    if let Some(dir) = rfd::FileDialog::new()
                        .set_title("Folder to watch for downloads")
                        .pick_folder()
                    {
                        win.downloads = Some(dir);
                        if let Some(key) = win.import.as_ref().map(|i| i.key.clone()) {
                            win.start_import(&key);
                        }
                    }
                }
                PartsAction::StopImport => win.import = None,
                PartsAction::Remove(key) => {
                    if let Some(Err(e)) = win.library.as_ref().map(|l| l.remove(&key)) {
                        error = Some(format!("Parts library: {e}"));
                    }
                }
                PartsAction::Close => close = true,
            }
        }
        if let Some(text) = error {
            self.message(Level::Error, text);
        }
        if close {
            self.wb.parts = None;
        }
    }
}

fn parts_ui(
    ui: &mut egui::Ui,
    win: &mut PartsWindow,
    dragging: bool,
    actions: &mut Vec<PartsAction>,
) {
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut win.query)
                .hint_text("Search order number, type or description")
                .desired_width(280.0),
        );
        let label = win.maker.clone().unwrap_or_else(|| "All makers".into());
        egui::ComboBox::from_id_salt("parts_maker")
            .selected_text(label)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut win.maker, None, "All makers");
                for m in win.makers() {
                    ui.selectable_value(&mut win.maker, Some(m.clone()), m);
                }
            });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("Close").clicked() {
                actions.push(PartsAction::Close);
            }
        });
    });
    ui.separator();
    let rows = win.rows();
    if win.import.is_some() || dragging {
        egui::Panel::bottom("parts_status")
            .resizable(false)
            .show(ui, |ui| status_ui(ui, win, dragging, actions));
    }
    egui::Panel::right("parts_detail")
        .exact_size(300.0)
        .resizable(false)
        .show(ui, |ui| detail_ui(ui, win, actions));
    egui::CentralPanel::default().show(ui, |ui| {
        if rows.is_empty() {
            ui.weak("No part matches.");
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 24.0, rows.len(), |ui, range| {
            for (key, action) in &rows[range] {
                let Some(part) = win.catalog.get(key) else {
                    continue;
                };
                ui.horizontal(|ui| {
                    let selected = win.selected.as_ref() == Some(key);
                    let text = format!(
                        "{}  {}  {}",
                        part.manufacturer, part.order_number, part.description
                    );
                    if ui
                        .selectable_label(selected, egui::RichText::new(text))
                        .clicked()
                    {
                        actions.push(PartsAction::Select(key.clone()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        match action {
                            RowAction::InLibrary => {
                                ui.colored_label(
                                    egui::Color32::from_rgb(90, 190, 120),
                                    "In library",
                                );
                            }
                            RowAction::Import | RowAction::OpenPage(_) => {
                                if ui.small_button("Import").clicked() {
                                    actions.push(PartsAction::Import(key.clone()));
                                }
                            }
                        }
                    });
                });
            }
        });
    });
}

/// The strip under the list: what the import is waiting for, or its result.
fn status_ui(ui: &mut egui::Ui, win: &PartsWindow, dragging: bool, actions: &mut Vec<PartsAction>) {
    if dragging {
        let who = win.target().map_or_else(
            || "Select a part first, then drop its file here.".to_owned(),
            |p| format!("Drop the file to import it for {p}."),
        );
        ui.strong(who);
        return;
    }
    let Some(import) = win.import.as_ref() else {
        return;
    };
    match &import.phase {
        Phase::Waiting => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.strong(format!("Importing {}", import.label));
            });
            match (&win.downloads, import.watching()) {
                (Some(dir), true) => ui.label(format!(
                    "Save the file from the maker's page. It is picked up from {} by itself; \
you can also drop it on this window.",
                    dir.display()
                )),
                _ => ui.label("Drop the downloaded file on this window, or choose it."),
            };
            if import.page_url.is_some() && !import.page_verified {
                ui.weak(
                    "This page address is not confirmed. If it does not show the part, find it \
on the maker's site and drop the file here.",
                );
            }
            ui.horizontal(|ui| {
                if let Some(url) = &import.page_url
                    && ui.small_button("Open page again").clicked()
                {
                    actions.push(PartsAction::OpenPage(url.clone()));
                }
                if ui.small_button("Choose file\u{2026}").clicked() {
                    actions.push(PartsAction::ChooseFile);
                }
                if ui.small_button("Watch another folder\u{2026}").clicked() {
                    actions.push(PartsAction::ChooseFolder);
                }
                if ui.small_button("Stop").clicked() {
                    actions.push(PartsAction::StopImport);
                }
            });
        }
        Phase::Done(summary) => {
            ui.colored_label(
                egui::Color32::from_rgb(90, 190, 120),
                format!("Imported {} ({})", import.label, summary.file),
            );
            for line in &summary.lines {
                ui.label(line);
            }
            if ui.small_button("Done").clicked() {
                actions.push(PartsAction::StopImport);
            }
        }
        Phase::Failed(e) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Could not import {}: {e}", import.label),
            );
            ui.horizontal(|ui| {
                if ui.small_button("Try again").clicked() {
                    actions.push(PartsAction::Import(import.key.clone()));
                }
                if ui.small_button("Dismiss").clicked() {
                    actions.push(PartsAction::StopImport);
                }
            });
        }
    }
}

fn detail_ui(ui: &mut egui::Ui, win: &PartsWindow, actions: &mut Vec<PartsAction>) {
    let Some(part) = win.selected.as_ref().and_then(|k| win.catalog.get(k)) else {
        ui.weak("Pick a part to see its data.");
        return;
    };
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.heading(&part.order_number);
        ui.label(&part.manufacturer);
        if !part.description.is_empty() {
            ui.weak(&part.description);
        }
        for section in detail_sections(part) {
            ui.add_space(6.0);
            ui.strong(section.title);
            egui::Grid::new(("parts_detail_grid", section.title))
                .num_columns(2)
                .show(ui, |ui| {
                    for (label, value) in &section.rows {
                        ui.label(label);
                        ui.label(value);
                        ui.end_row();
                    }
                });
        }
        let key = part.key();
        ui.add_space(8.0);
        ui.strong("Source");
        let records = win.records(&key);
        if records.is_empty() {
            ui.weak(if win.in_library(&key) {
                "In the library."
            } else {
                "Not imported yet. Import saves the maker's file on this computer."
            });
        }
        for r in &records {
            ui.label(format!("{} ({} bytes)", r.file_name, r.bytes));
            ui.weak(format!(
                "imported {} \u{b7} sha256 {}",
                r.imported,
                &r.sha256[..12.min(r.sha256.len())]
            ));
            if let Some(url) = &r.url {
                ui.hyperlink(url);
            }
            if let Some(url) = &r.terms_url {
                ui.hyperlink_to("Terms", url);
            }
        }
        if !records.is_empty() && ui.small_button("Remove from library").clicked() {
            actions.push(PartsAction::Remove(key));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::Properties;
    use novo_parts::Dimensions;

    fn part(data: &[(&str, Value)]) -> CatalogPart {
        let mut props = Properties::default();
        for (k, v) in data {
            props.insert((*k).into(), v.clone());
        }
        CatalogPart {
            manufacturer: "Acme".into(),
            order_number: "A-1".into(),
            dimensions: Some(Dimensions {
                width_mm: 45.0,
                height_mm: 80.0,
                depth_mm: 60.5,
            }),
            data: props,
            ..Default::default()
        }
    }

    #[test]
    fn detail_groups_dimensions_power_loss_clearances_and_connections() {
        let p = part(&[
            ("power_loss_w", Value::Float(3.5)),
            ("clearance_side_mm", Value::Float(10.0)),
            ("wire_max_mm2", Value::Float(4.0)),
            ("rated_current_a", Value::Int(9)),
            ("terminal.0.name", Value::Text("hidden".into())),
        ]);
        let titles: Vec<_> = detail_sections(&p).iter().map(|s| s.title).collect();
        assert_eq!(
            titles,
            [DIMENSIONS, POWER_LOSS, CLEARANCES, CONNECTIONS, TECHNICAL]
        );
        let all = detail_sections(&p);
        assert_eq!(all[0].rows[0].1, "45 \u{d7} 80 \u{d7} 60.5 mm");
        assert_eq!(all[1].rows[0], ("Power loss".into(), "3.5 W".into()));
        assert!(
            all.iter().flat_map(|s| &s.rows).all(|r| r.1 != "hidden"),
            "internal keys stay out"
        );
    }

    #[test]
    fn a_part_without_data_has_no_empty_blocks() {
        let mut p = part(&[]);
        p.dimensions = None;
        assert!(detail_sections(&p).is_empty());
    }

    #[test]
    fn the_maker_filter_and_search_narrow_the_rows() {
        let mut win = PartsWindow::new(Catalog::starter(), None, SourceTable::builtin());
        let all = win.rows().len();
        assert!(all > 0);
        let makers = win.makers();
        assert!(makers.len() > 1, "the starter set has several makers");
        win.maker = Some(makers[0].clone());
        let one = win.rows();
        assert!(!one.is_empty() && one.len() < all);
        assert!(one.iter().all(|(k, _)| {
            win.catalog
                .get(k)
                .is_some_and(|p| p.manufacturer == makers[0])
        }));
        win.query = "zzz-no-such-part".into();
        assert!(win.rows().is_empty());
    }

    #[test]
    fn rows_say_in_library_once_a_file_is_stored() {
        let dir = tempfile::tempdir().unwrap();
        let lib = LocalLibrary::at(dir.path());
        let catalog = Catalog::starter();
        let first = catalog.parts()[0].clone();
        lib.store(
            &novo_parts::NewSource {
                manufacturer: &first.manufacturer,
                order_number: &first.order_number,
                file_name: "x.stp",
                url: None,
                flow: novo_parts::ImportFlow::Manual,
                imported: "2026-10-15",
                terms_url: None,
            },
            b"x",
        )
        .unwrap();
        let win = PartsWindow::new(catalog, Some(lib), SourceTable::builtin());
        let rows = win.rows();
        let row = rows.iter().find(|(k, _)| *k == first.key()).unwrap();
        assert_eq!(row.1, RowAction::InLibrary);
        assert!(rows.iter().any(|(_, a)| *a != RowAction::InLibrary));
    }

    #[test]
    fn a_dropped_file_imports_for_the_selected_part_and_the_row_turns_green() {
        let dir = tempfile::tempdir().unwrap();
        let mut win = PartsWindow::new(
            Catalog::starter(),
            Some(LocalLibrary::at(dir.path().join("lib"))),
            SourceTable::builtin(),
        );
        win.downloads = None;
        win.catalog_path = Some(dir.path().join("parts.json"));
        let key = win.catalog.parts()[0].key();
        let file = dir.path().join("part.stp");
        std::fs::write(
            &file,
            "ISO-10303-21;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',(10.,20.,30.));\nENDSEC;\nEND-ISO-10303-21;\n",
        )
        .unwrap();
        win.import_file(&file, ImportFlow::Manual);
        assert!(win.import.is_none(), "nothing selected: nothing happens");
        win.selected = Some(key.clone());
        win.import_file(&file, ImportFlow::Manual);
        let imp = win.import.as_ref().unwrap();
        assert!(matches!(imp.phase, Phase::Done(_)), "{:?}", imp.phase);
        let row = win.rows().into_iter().find(|(k, _)| *k == key).unwrap();
        assert_eq!(row.1, RowAction::InLibrary);
        assert_eq!(win.records(&key)[0].flow, ImportFlow::Manual);
    }
}
