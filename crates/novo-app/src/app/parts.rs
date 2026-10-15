//! The Parts window: browse the part catalog (the starter set, the user's
//! own parts and everything imported), filter by maker, and read a part's
//! dimensions, power loss, clearances and connection data. Each row says
//! whether the part is in the local library or can be imported.
//!
//! The window is a thin view over `novo-parts`: rows come from
//! [`browser_rows`], the library and its source records from
//! [`LocalLibrary`]. Design: `docs/PARTS-LIBRARY-DESIGN.md` (section 3a).

use eframe::egui;
use novo_core::Value;
use novo_parts::{
    Catalog, CatalogPart, LocalLibrary, PartKey, RowAction, SourceRecord, SourceTable,
    browser_rows, display_value, is_internal_key, property_label,
};

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
    OpenPage(String),
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
        egui::Window::new("Parts")
            .open(&mut open)
            .default_size([820.0, 520.0])
            .show(ctx, |ui| parts_ui(ui, win, &mut actions));
        let mut close = !open;
        let mut error = None;
        for action in actions {
            match action {
                PartsAction::Select(key) => win.selected = Some(key),
                PartsAction::OpenPage(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
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

fn parts_ui(ui: &mut egui::Ui, win: &mut PartsWindow, actions: &mut Vec<PartsAction>) {
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
                            RowAction::Import => {
                                ui.add_enabled(false, egui::Button::new("Import"));
                            }
                            RowAction::OpenPage(Some(url)) => {
                                if ui.small_button("Import").clicked() {
                                    actions.push(PartsAction::OpenPage(url.clone()));
                                }
                            }
                            RowAction::OpenPage(None) => {
                                ui.weak("No page");
                            }
                        }
                    });
                });
            }
        });
    });
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
}
