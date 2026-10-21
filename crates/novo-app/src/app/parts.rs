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
use novo_interop::part3d::cache::MeshCache;
use novo_interop::part3d::service::{GeometryJob, GeometryService, GeometryState, TIME_LIMIT};
use novo_interop::part3d::{Fallback, MeshOrigin, Quality};
use novo_parts::{
    Catalog, CatalogPart, ConnectionKind, DataSource, FileKind, ImportFlow, Lifecycle,
    LocalLibrary, PartKey, RowAction, Side, SourceRecord, SourceTable, browser_rows,
    default_downloads_dir, default_inbox, default_library_root, display_value, is_internal_key,
    maker_guide, property_label, user_catalog_path,
};

use novo_theme::table::{
    ROW_HEIGHT, RowState, TableAlign, TableCell, TableColumn, column_widths, table_header,
    table_row,
};
use novo_theme::{Palette, widgets};

use super::parts_import::{Import, Phase};
use super::revisions::today;
use super::{Level, NovoApp};

/// The detail pane is a share of the window, kept between these widths.
const DETAIL_MIN: f32 = 280.0;
const DETAIL_MAX: f32 = 420.0;

/// The window's first size: the old 820 x 520 on a small screen, more on a
/// large one so the table is not squeezed (a person can still resize it).
fn window_size(ctx: &egui::Context) -> [f32; 2] {
    let screen = ctx.content_rect().size();
    [
        (screen.x * 0.68).clamp(820.0, 1240.0),
        (screen.y * 0.78).clamp(520.0, 860.0),
    ]
}

/// State of the open Parts window.
pub(crate) struct PartsWindow {
    query: String,
    /// Only parts of this maker; `None` shows all.
    maker: Option<String>,
    selected: Option<PartKey>,
    /// Scroll the selected row into view on the next frame (set when the
    /// arrow keys move the selection).
    reveal: bool,
    catalog: Catalog,
    library: Option<LocalLibrary>,
    sources: SourceTable,
    /// The import in progress or just finished, shown in the status strip.
    import: Option<Import>,
    /// The folder the Downloads watch looks in.
    downloads: Option<PathBuf>,
    /// The user's catalog file, where read data is saved.
    catalog_path: Option<PathBuf>,
    /// The part whose Remove from library awaits a second click.
    confirm_remove: Option<PartKey>,
    focus_search: bool,
    /// Builds the 3D models of imported STEP files in the background;
    /// created on the first frame, when the repaint handle is known.
    geometry: Option<GeometryService>,
    /// The Quality tab (completeness of the catalog).
    quality: super::parts_quality::QualityTab,
    /// The Connection points block of the part detail.
    pins: super::parts_pins::PinsState,
    /// The New part form, while open.
    new_part: Option<NewPartForm>,
    /// The maker guide (what to download, where to save), while open.
    guide: bool,
    /// Repaints the window when a background read finishes; set on the first
    /// frame.
    wake: Option<egui::Context>,
}

/// The fields of the New part form.
#[derive(Default)]
struct NewPartForm {
    maker: String,
    order_number: String,
    type_number: String,
    description: String,
    /// Letter codes, comma separated (`K, Q`).
    letter_codes: String,
    width: String,
    height: String,
    depth: String,
    error: Option<String>,
}

impl NewPartForm {
    /// The part the form describes, or what is missing.
    fn build(&self) -> Result<CatalogPart, String> {
        let (maker, order) = (self.maker.trim(), self.order_number.trim());
        if maker.is_empty() || order.is_empty() {
            return Err("Maker and order number are required".into());
        }
        let mut part = CatalogPart::new(maker, order);
        part.type_number = self.type_number.trim().to_owned();
        part.description = self.description.trim().to_owned();
        part.letter_codes = self
            .letter_codes
            .split([',', ';', ' '])
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_uppercase)
            .collect();
        let size = |text: &str, name: &str| -> Result<Option<f64>, String> {
            let t = text.trim().replace(',', ".");
            if t.is_empty() {
                return Ok(None);
            }
            match t.parse::<f64>() {
                Ok(v) if v > 0.0 && v.is_finite() => Ok(Some(v)),
                _ => Err(format!("{name} must be a positive number in mm")),
            }
        };
        let dims = (
            size(&self.width, "Width")?,
            size(&self.height, "Height")?,
            size(&self.depth, "Depth")?,
        );
        match dims {
            (None, None, None) => {}
            (Some(width_mm), Some(height_mm), Some(depth_mm)) => {
                part.dimensions = Some(novo_parts::Dimensions {
                    width_mm,
                    height_mm,
                    depth_mm,
                });
            }
            _ => return Err("Give width, height and depth together, or none".into()),
        }
        Ok(part)
    }
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
            reveal: false,
            catalog,
            library,
            sources,
            import: None,
            downloads: default_downloads_dir(),
            catalog_path: user_catalog_path(),
            confirm_remove: None,
            focus_search: true,
            geometry: None,
            quality: Default::default(),
            pins: Default::default(),
            new_part: None,
            guide: false,
            wake: None,
        }
    }

    /// Swaps in a catalog read again from disk, keeping the selection when
    /// the part is still there.
    pub(super) fn set_catalog(&mut self, catalog: Catalog) {
        self.catalog = catalog;
        if self
            .selected
            .as_ref()
            .is_some_and(|k| self.catalog.get(k).is_none())
        {
            self.selected = None;
        }
    }

    #[cfg(test)]
    pub(super) fn set_catalog_path_for_test(&mut self, path: PathBuf) {
        self.catalog_path = Some(path);
    }

    /// Saves a hand-made part into the user's catalog and the open list.
    pub(super) fn add_user_part(&mut self, part: CatalogPart) -> Result<(), String> {
        let path = self
            .catalog_path
            .clone()
            .ok_or("No profile folder is known, so there is nowhere to save the part")?;
        const TAKEN: &str = "This maker and order number is already in the catalog (select it in the list to edit it)";
        if self.catalog.get(&part.key()).is_some() {
            return Err(TAKEN.into());
        }
        let mut user = if path.exists() {
            Catalog::load(&path).map_err(|e| e.to_string())?
        } else {
            Catalog::new()
        };
        if user.get(&part.key()).is_some() {
            return Err(TAKEN.into());
        }
        user.upsert(part.clone());
        user.save(&path).map_err(|e| e.to_string())?;
        let key = part.key();
        self.catalog.upsert(part);
        self.selected = Some(key);
        Ok(())
    }

    /// The part a dropped or picked file belongs to: the one being
    /// imported, else the selected one.
    pub(super) fn target(&self) -> Option<CatalogPart> {
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

    /// Imports a file for the target part. The file is stored and read on a
    /// worker thread (a big ZIP or EDZ must not freeze the window); the
    /// window collects the result in [`PartsWindow::collect_import`].
    fn import_file(&mut self, path: &Path, flow: ImportFlow) {
        let Some(part) = self.target() else {
            return;
        };
        if self.import.as_ref().is_none_or(|i| i.key != part.key()) {
            self.start_import(&part.key());
        }
        let Some(library) = self.library.clone() else {
            if let Some(i) = self.import.as_mut() {
                i.phase = Phase::Failed("there is no profile folder to keep the library in".into());
            }
            return;
        };
        let wake = self.wake.clone();
        let (sources, catalog_path, today) =
            (self.sources.clone(), self.catalog_path.clone(), today());
        if let Some(i) = self.import.as_mut() {
            i.begin_read(
                part,
                path,
                flow,
                library,
                sources,
                catalog_path,
                today,
                move || {
                    if let Some(ctx) = &wake {
                        ctx.request_repaint();
                    }
                },
            );
        }
    }

    /// Makes `key` the selected part. A wait for a download belongs to the
    /// part it was started for; picking another part ends it, so a file
    /// dropped next is never filed under the old part.
    fn select(&mut self, key: PartKey) {
        if self
            .import
            .as_ref()
            .is_some_and(|i| i.key != key && matches!(i.phase, Phase::Waiting | Phase::Confirm(_)))
        {
            self.import = None;
        }
        self.selected = Some(key);
    }

    /// One frame of the import: collects a finished read, ends a wait or
    /// read that ran too long, and takes in a file the Downloads watch found.
    fn pump_import(&mut self, now: f64) {
        self.collect_import(now);
        if let Some(found) = self.import.as_mut().and_then(|i| i.poll(now)) {
            if found.names_the_part {
                self.import_file(&found.path, ImportFlow::Downloads);
            } else if let Some(i) = self.import.as_mut() {
                i.phase = Phase::Confirm(found.path);
            }
        }
    }

    /// Moves the import's clocks on and merges what a finished read added
    /// into the open catalog. Called every frame.
    fn collect_import(&mut self, now: f64) {
        if let Some(done) = self.import.as_mut().and_then(|i| i.tick(now)) {
            for p in done.changed {
                self.catalog.upsert(p);
            }
        }
    }

    /// Asks for a STEP file and stores it for the selected part.
    fn choose_model(&mut self) {
        if let Some(path) = crate::dialogs::FileDialog::new()
            .set_title("Choose the 3D model (STEP)")
            .add_filter("STEP model", &["stp", "step"])
            .pick_file()
        {
            self.import_file(&path, ImportFlow::Manual);
        }
    }

    /// The model state of a part with a stored STEP file; asks the service
    /// to build it the first time. `None` when the part has no STEP file.
    fn model_state(&self, part: &CatalogPart, records: &[SourceRecord]) -> Option<GeometryState> {
        let record = records
            .iter()
            .find(|r| FileKind::of_file_name(&r.file_name) == FileKind::Step)?;
        let (service, library) = (self.geometry.as_ref()?, self.library.as_ref()?);
        let key = service.request(model_job(part, record, library));
        Some(service.geometry(key))
    }

    /// Writes the detected terminal positions of `key` into the part and the
    /// user's catalog file.
    fn save_pins(&mut self, key: &PartKey) {
        let Some(mut part) = self.catalog.get(key).cloned() else {
            return;
        };
        let Some(detection) = self.pins.detection(key).cloned() else {
            return;
        };
        let written = super::parts_pins::apply(&mut part, &detection, self.pins.with_estimates);
        let saved = match (written, self.catalog_path.as_ref()) {
            (0, _) => Err("Nothing to save: no terminal was measured".to_owned()),
            (_, None) => Err("No profile folder is known, so there is nowhere to save".to_owned()),
            (_, Some(path)) => super::parts_pins::save_to_user_catalog(path, &part),
        };
        self.pins.note = Some(match saved {
            Ok(()) => {
                self.catalog.upsert(part);
                format!("Saved {written} terminal positions to your catalog.")
            }
            Err(e) => e,
        });
    }

    /// Writes hand-typed terminal positions of `key` into the part and the
    /// user's catalog file.
    fn save_typed_pins(&mut self, key: &PartKey, edits: &[(String, [f64; 3])]) {
        let Some(mut part) = self.catalog.get(key).cloned() else {
            return;
        };
        let written = super::parts_pins::apply_typed(&mut part, edits);
        let saved = match (written, self.catalog_path.as_ref()) {
            (0, _) => Err("Nothing to save: no valid position was typed".to_owned()),
            (_, None) => Err("No profile folder is known, so there is nowhere to save".to_owned()),
            (_, Some(path)) => super::parts_pins::save_to_user_catalog(path, &part),
        };
        self.pins.note = Some(match saved {
            Ok(()) => {
                self.catalog.upsert(part);
                format!("Saved {written} typed terminal positions to your catalog.")
            }
            Err(e) => e,
        });
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
const CLASSIFICATION: &str = "Classification";
const ORDER_DATA: &str = "Order data";
const TERMINALS: &str = "Terminals";
const LET_THROUGH: &str = "Let-through";
const SOURCE: &str = "Source";

/// What a value's source tag says, or nothing when the source is unknown
/// (an unknown source is not labelled; a known one always is).
fn source_label(source: DataSource) -> Option<&'static str> {
    match source {
        DataSource::MakerDatasheet => Some("maker datasheet"),
        DataSource::Typed => Some("typed"),
        DataSource::Estimated => Some("estimated"),
        DataSource::Unknown => None,
    }
}

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
    let mut blocks: Vec<DetailSection> = [
        DIMENSIONS,
        POWER_LOSS,
        CLEARANCES,
        CONNECTIONS,
        TECHNICAL,
        CLASSIFICATION,
        ORDER_DATA,
        TERMINALS,
        LET_THROUGH,
        SOURCE,
    ]
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
        let mut text = match value {
            Value::Text(t) if t.is_empty() => continue,
            other => with_unit(display_value(other), unit),
        };
        if let Some(label) = source_label(part.source_of(key)) {
            text = format!("{text}  ({label})");
        }
        let title = section_of(key);
        let block = blocks
            .iter_mut()
            .find(|b| b.title == title)
            .expect("every title has a block");
        block.rows.push((label, text));
    }
    push_v2_rows(part, &mut blocks);
    blocks.retain(|b| !b.rows.is_empty());
    blocks
}

/// The schema v2 blocks: classification, order data, terminals, the
/// let-through table and where the part came from. Nothing is filled in for
/// fields the part does not have.
fn push_v2_rows(part: &CatalogPart, blocks: &mut [DetailSection]) {
    let mut add = |title: &str, label: &str, value: String| {
        blocks
            .iter_mut()
            .find(|b| b.title == title)
            .expect("every title has a block")
            .rows
            .push((label.to_owned(), value));
    };
    if let Some(code) = &part.class.etim {
        add(CLASSIFICATION, "ETIM class", code.clone());
    }
    if let Some(code) = &part.class.eclass {
        add(CLASSIFICATION, "ECLASS", code.clone());
    }
    if !part.function.is_empty() {
        add(CLASSIFICATION, "Function", part.function.clone());
    }
    let lifecycle = match part.lifecycle {
        Lifecycle::Active => Some("active"),
        Lifecycle::PhasingOut => Some("phasing out"),
        Lifecycle::Discontinued => Some("discontinued"),
        Lifecycle::Unknown => None,
    };
    if let Some(text) = lifecycle {
        add(CLASSIFICATION, "Lifecycle", text.into());
    }
    if let Some(r) = &part.replaced_by {
        add(
            CLASSIFICATION,
            "Replaced by",
            format!("{} {}", r.manufacturer, r.order_number),
        );
    }
    if !part.alternatives.is_empty() {
        let list: Vec<String> = part
            .alternatives
            .iter()
            .map(|r| format!("{} {}", r.manufacturer, r.order_number))
            .collect();
        add(CLASSIFICATION, "Alternatives", list.join(", "));
    }
    if let Some(price) = &part.price {
        let per = if price.per > 1 {
            format!(" per {}", price.per)
        } else {
            String::new()
        };
        let date = if price.date.is_empty() {
            String::new()
        } else {
            format!(" ({})", price.date)
        };
        add(
            ORDER_DATA,
            "List price",
            format!("{:.2} {}{per}{date}", price.amount, price.currency),
        );
    }
    for t in &part.terminals {
        let mut bits = Vec::new();
        if let Some((lo, hi)) = t.wire_mm2 {
            bits.push(format!("{lo}\u{2013}{hi} mm\u{b2}"));
        }
        if let Some(label) = connection_label(t.kind) {
            bits.push(label.to_owned());
        }
        if let Some(label) = side_label(t.side) {
            bits.push(label.to_owned());
        }
        if let Some([x, y, z]) = t.position_mm {
            bits.push(format!("at {x}, {y}, {z} mm"));
        }
        add(TERMINALS, &format!("Terminal {}", t.name), bits.join(", "));
    }
    for p in &part.let_through {
        add(
            LET_THROUGH,
            &format!("At {} kA", p.ik_ka),
            format!("{} A\u{b2}s, peak {} kA", p.i2t_a2s, p.ip_ka),
        );
    }
    if let Some(p) = &part.provenance {
        {
            add(SOURCE, "Origin", p.source.clone());
            if !p.file.is_empty() {
                add(SOURCE, "File", p.file.clone());
            }
            if !p.imported.is_empty() {
                add(SOURCE, "Imported", p.imported.clone());
            }
            if !p.licence_note.is_empty() {
                add(SOURCE, "Licence", p.licence_note.clone());
            }
        }
    }
}

fn connection_label(kind: ConnectionKind) -> Option<&'static str> {
    Some(match kind {
        ConnectionKind::Screw => "screw",
        ConnectionKind::PushIn => "push-in",
        ConnectionKind::Spring => "spring",
        ConnectionKind::Screwless => "screwless",
        ConnectionKind::Lug => "lug",
        ConnectionKind::Pin => "pin",
        ConnectionKind::Faston => "faston",
        ConnectionKind::SolderPin => "solder pin",
        ConnectionKind::Unknown => return None,
    })
}

fn side_label(side: Side) -> Option<&'static str> {
    Some(match side {
        Side::Top => "top",
        Side::Bottom => "bottom",
        Side::Left => "left",
        Side::Right => "right",
        Side::Front => "front",
        Side::Unknown => return None,
    })
}

/// What a click in the window asks for; run after drawing, so drawing only
/// reads.
enum PartsAction {
    /// Open the table import with its mapping step.
    ImportTable,
    Select(PartKey),
    Import(PartKey),
    OpenPage(String),
    ChooseFile,
    ChooseModel,
    /// Scan the part's STEP model for connection points.
    DetectPins(PartKey),
    /// Save the detected positions into the user's catalog.
    SavePins(PartKey),
    /// Save positions typed by hand into the user's catalog.
    SaveTypedPins(PartKey, Vec<(String, [f64; 3])>),
    ChooseFolder,
    StopImport,
    ConfirmFile(PathBuf),
    DeclineFile(PathBuf),
    AskRemove(PartKey),
    CancelRemove,
    Remove(PartKey),
    /// Give the selected devices this part.
    UseForSelection(PartKey),
    /// Pick catalog files (BMEcat, CSV, ZIP) and import them as a catalog.
    ImportCatalogFiles,
    /// Import everything in the default inbox.
    ImportInbox,
    OpenInbox,
    OpenLibrary,
    NewPart,
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

    /// The command `parts.import_step`: with a part picked in the Parts
    /// window it asks for the file; otherwise it opens the window and says
    /// what to do.
    pub(super) fn import_part_model(&mut self) {
        self.open_parts_browser();
        match self.wb.parts.as_mut() {
            Some(win) if win.selected.is_some() => win.choose_model(),
            _ => self.message(
                Level::Info,
                "Pick a part in the Parts window, then import its 3D model",
            ),
        }
    }

    pub(super) fn parts_window(&mut self, ctx: &egui::Context) {
        let Some(win) = self.wb.parts.as_mut() else {
            return;
        };
        let mut open = true;
        let mut actions = Vec::new();
        win.wake.get_or_insert_with(|| ctx.clone());
        win.geometry.get_or_insert_with(|| {
            let wake = ctx.clone();
            GeometryService::new(
                MeshCache::open_default(),
                Quality::Normal,
                TIME_LIMIT,
                move || wake.request_repaint(),
            )
        });
        let dragging = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let shown = super::panel::dialog(ctx, "Parts", window_size(ctx))
            .open(super::panel::esc(ctx, &mut open))
            // The list and the detail pane scroll on their own; an outer
            // scroll area would give them no height to fill.
            .scroll([false, false])
            .show(ctx, |ui| parts_ui(ui, win, dragging, &mut actions));
        // A file dropped on the window, or one the watch found.
        let over = shown
            .as_ref()
            .zip(ctx.input(|i| i.pointer.latest_pos()))
            .is_some_and(|(r, pos)| r.response.rect.contains(pos));
        let dropped: Vec<PathBuf> = ctx.input(|i| {
            if !over {
                return Vec::new();
            }
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .filter(|p| !p.as_os_str().is_empty())
                .collect()
        });
        let mut catalog_files = Vec::new();
        let mut table_file = None;
        let mut no_part_for = None;
        if win.target().is_some() {
            if let Some(path) = dropped.first() {
                win.import_file(path, ImportFlow::Manual);
            }
        } else {
            // No part picked: a catalog file is imported as a catalog.
            for path in &dropped {
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
                if name.as_deref().is_some_and(|n| {
                    n.rsplit('.')
                        .next()
                        .is_some_and(|e| e.eq_ignore_ascii_case("xlsx"))
                }) {
                    table_file = Some(path.clone());
                    continue;
                }
                match name.as_deref().map(FileKind::of_file_name) {
                    Some(FileKind::Bmecat | FileKind::Csv | FileKind::Zip) => {
                        catalog_files.push(path.clone());
                    }
                    _ => no_part_for = name,
                }
            }
        }
        let now = ctx.input(|i| i.time);
        win.pump_import(now);
        if win
            .import
            .as_ref()
            .is_some_and(|i| matches!(i.phase, Phase::Waiting | Phase::Reading))
        {
            ctx.request_repaint_after(std::time::Duration::from_secs_f64(
                super::parts_import::POLL_SECONDS,
            ));
        }
        let mut close = !open;
        let mut error = None;
        let mut assign = None;
        let mut open_dir = None;
        for action in actions {
            match action {
                PartsAction::ImportTable => {
                    if let Some(path) = crate::dialogs::FileDialog::new()
                        .set_title("Parts table (CSV or Excel)")
                        .add_filter("Parts table", &["csv", "tsv", "txt", "xlsx"])
                        .pick_file()
                    {
                        table_file = Some(path);
                    }
                }
                PartsAction::ImportCatalogFiles => {
                    if let Some(files) = crate::dialogs::FileDialog::new()
                        .set_title("Catalog files (BMEcat, CSV, ZIP)")
                        .add_filter("Catalog files", &["xml", "bmecat", "csv", "zip"])
                        .pick_files()
                    {
                        catalog_files.extend(files);
                    }
                }
                PartsAction::ImportInbox => {
                    if let Some(inbox) = default_inbox() {
                        open_dir = Some((inbox, true));
                    }
                }
                PartsAction::OpenInbox => open_dir = default_inbox().map(|d| (d, false)),
                PartsAction::OpenLibrary => {
                    open_dir = default_library_root().map(|d| (d, false));
                }
                PartsAction::NewPart => win.new_part = Some(NewPartForm::default()),
                PartsAction::Select(key) => win.select(key),
                PartsAction::Import(key) => {
                    win.start_import(&key);
                    if let Some(url) = win.import.as_ref().and_then(|i| i.page_url.clone()) {
                        ctx.open_url(egui::OpenUrl::new_tab(url));
                    }
                }
                PartsAction::OpenPage(url) => ctx.open_url(egui::OpenUrl::new_tab(url)),
                PartsAction::ChooseFile => {
                    if let Some(path) = crate::dialogs::FileDialog::new()
                        .set_title("Choose the downloaded file")
                        .pick_file()
                    {
                        win.import_file(&path, ImportFlow::Manual);
                    }
                }
                PartsAction::ChooseModel => win.choose_model(),
                PartsAction::DetectPins(key) => {
                    let step = win
                        .records(&key)
                        .into_iter()
                        .find(|r| FileKind::of_file_name(&r.file_name) == FileKind::Step);
                    if let (Some(step), Some(lib), Some(part)) =
                        (step, win.library.as_ref(), win.catalog.get(&key).cloned())
                    {
                        let path = lib.path_of(&step);
                        win.pins.start(ctx, &part, path);
                    }
                }
                PartsAction::SavePins(key) => win.save_pins(&key),
                PartsAction::SaveTypedPins(key, edits) => win.save_typed_pins(&key, &edits),
                PartsAction::ChooseFolder => {
                    if let Some(dir) = crate::dialogs::FileDialog::new()
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
                PartsAction::ConfirmFile(path) => win.import_file(&path, ImportFlow::Downloads),
                PartsAction::DeclineFile(path) => {
                    if let Some(i) = win.import.as_mut() {
                        i.decline(path);
                    }
                }
                PartsAction::AskRemove(key) => win.confirm_remove = Some(key),
                PartsAction::CancelRemove => win.confirm_remove = None,
                PartsAction::Remove(key) => {
                    win.confirm_remove = None;
                    if let Some(Err(e)) = win.library.as_ref().map(|l| l.remove(&key)) {
                        error = Some(format!("Parts library: {e}"));
                    }
                }
                PartsAction::UseForSelection(key) => assign = win.catalog.get(&key).cloned(),
                PartsAction::Close => close = true,
            }
        }
        if let Some(text) = error {
            self.message(Level::Error, text);
        }
        if !catalog_files.is_empty() {
            self.import_catalog_files(&catalog_files);
        }
        if let Some(path) = table_file {
            self.import_parts_table_from(&path, None, novo_parts::saved_mappings_path());
        }
        if let Some(name) = no_part_for {
            self.message(
                Level::Info,
                format!(
                    "Pick a part first to import {name}, or drop a catalog file (BMEcat, CSV, ZIP)"
                ),
            );
        }
        match open_dir {
            Some((dir, true)) => {
                if let Err(e) = super::parts_bulk::prepare_and_import_inbox(self, &dir) {
                    self.message(Level::Error, e);
                }
            }
            Some((dir, false)) => {
                if let Err(e) = super::parts_bulk::open_folder(&dir) {
                    self.message(Level::Error, e);
                }
            }
            None => {}
        }
        if let Some(part) = assign {
            self.use_part_for_selection(&part);
        }
        if close {
            self.wb.parts = None;
        }
    }
}

/// The maker guide: per maker the page to use, whether it needs a login and
/// the inbox folder to save into (`novo_parts::maker_guide`).
fn guide_ui(ui: &mut egui::Ui, win: &mut PartsWindow) {
    ui.group(|ui| {
        ui.horizontal(|ui| {
            ui.strong("Maker guide");
            ui.weak("KoperCAD never contacts a maker site: you download, it reads.");
            if ui.button("Hide").clicked() {
                win.guide = false;
            }
        });
        if let Some(dir) = default_inbox() {
            ui.weak(format!("Inbox: {}", dir.display()));
        }
        egui::ScrollArea::vertical()
            .id_salt("maker_guide")
            .max_height(160.0)
            .show(ui, |ui| {
                for line in maker_guide(&win.makers()) {
                    ui.label(line);
                }
            });
    });
}

/// The New part form: a hand-made part goes into the user's catalog.
fn new_part_ui(ui: &mut egui::Ui, win: &mut PartsWindow) {
    let Some(form) = win.new_part.as_mut() else {
        return;
    };
    let (mut create, mut cancel) = (false, false);
    ui.group(|ui| {
        ui.strong("New part");
        egui::Grid::new("new_part").num_columns(2).show(ui, |ui| {
            for (label, text, hint) in [
                ("Maker", &mut form.maker, "Phoenix Contact"),
                (
                    "Order number",
                    &mut form.order_number,
                    "required, unique per maker",
                ),
                ("Type", &mut form.type_number, ""),
                ("Description", &mut form.description, ""),
                (
                    "Letter codes",
                    &mut form.letter_codes,
                    "K, Q (what devices it fits)",
                ),
                ("Width (mm)", &mut form.width, ""),
                ("Height (mm)", &mut form.height, ""),
                ("Depth (mm)", &mut form.depth, ""),
            ] {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(text).hint_text(hint));
                ui.end_row();
            }
        });
        if let Some(e) = &form.error {
            ui.colored_label(crate::theme::ui_palette().warning, e);
        }
        ui.horizontal(|ui| {
            create = ui.button("Save to my catalog").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    if create {
        match form.build() {
            Ok(part) => match win.add_user_part(part) {
                Ok(()) => win.new_part = None,
                Err(e) => {
                    if let Some(f) = win.new_part.as_mut() {
                        f.error = Some(e);
                    }
                }
            },
            Err(e) => form.error = Some(e),
        }
    } else if cancel {
        win.new_part = None;
    }
}

fn parts_ui(
    ui: &mut egui::Ui,
    win: &mut PartsWindow,
    dragging: bool,
    actions: &mut Vec<PartsAction>,
) {
    if let Some(jump) = super::parts_quality::tab(
        ui,
        &mut win.quality,
        &win.catalog,
        win.catalog_path
            .as_deref()
            .map(novo_parts::review_path_for)
            .as_deref(),
    ) {
        if let Some(key) = jump {
            (win.selected, win.query, win.maker) = (Some(key), String::new(), None);
        }
        return;
    }
    let mut search_focused = false;
    ui.horizontal(|ui| {
        let search = ui.add(
            egui::TextEdit::singleline(&mut win.query)
                .hint_text("Search order number, type or description")
                .desired_width(280.0),
        );
        if std::mem::take(&mut win.focus_search) {
            search.request_focus();
        }
        search_focused = search.has_focus();
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
            ui.menu_button("More", |ui| {
                for (label, hint, action) in [
                    (
                        "Import inbox",
                        "Import every file in the inbox folder.",
                        PartsAction::ImportInbox,
                    ),
                    (
                        "Open inbox folder",
                        "The folder to save downloaded catalog files in, one folder per maker.",
                        PartsAction::OpenInbox,
                    ),
                    (
                        "Open library folder",
                        "Where the STEP files and source records of imported parts are kept.",
                        PartsAction::OpenLibrary,
                    ),
                ] {
                    if ui.button(label).on_hover_text(hint).clicked() {
                        actions.push(action);
                        ui.close();
                    }
                }
                if ui
                    .button("Maker guide")
                    .on_hover_text("What to download from each maker and where to save it.")
                    .clicked()
                {
                    win.guide ^= true;
                    ui.close();
                }
            });
            if ui
                .button("Import table\u{2026}")
                .on_hover_text(
                    "Read a CSV or Excel parts table with any columns: you say what each column is, check the rows, then add them. You can also drop an .xlsx file on this window.",
                )
                .clicked()
            {
                actions.push(PartsAction::ImportTable);
            }
            if ui
                .button("Import catalog files")
                .on_hover_text(
                    "Read BMEcat, CSV or ZIP catalog files from a maker or wholesaler into your catalog. You can also drop them on this window.",
                )
                .clicked()
            {
                actions.push(PartsAction::ImportCatalogFiles);
            }
            if widgets::ghost_button(ui, "New part\u{2026}")
                .on_hover_text("Add a part by hand to your own catalog.")
                .clicked()
            {
                actions.push(PartsAction::NewPart);
            }
        });
    });
    if win.guide {
        guide_ui(ui, win);
    }
    if win.new_part.is_some() {
        new_part_ui(ui, win);
    }
    ui.add_space(4.0);
    let rows = win.rows();
    let (up, down, esc) = ui.input(|i| {
        (
            i.key_pressed(egui::Key::ArrowUp),
            i.key_pressed(egui::Key::ArrowDown),
            i.key_pressed(egui::Key::Escape),
        )
    });
    if search_focused && (up || down) && !rows.is_empty() {
        let at = win
            .selected
            .as_ref()
            .and_then(|k| rows.iter().position(|(r, _)| r == k));
        let next = match (at, down) {
            (None, _) => 0,
            (Some(i), true) => (i + 1).min(rows.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
        };
        actions.push(PartsAction::Select(rows[next].0.clone()));
        win.reveal = true;
    }
    if esc && (search_focused || ui.memory(|m| m.focused()).is_none()) {
        actions.push(PartsAction::Close);
    }
    if win.import.is_some() || dragging {
        egui::Panel::bottom("parts_status")
            .resizable(false)
            .show(ui, |ui| status_ui(ui, win, dragging, actions));
    }
    let detail_width = (ui.available_width() * 0.38).clamp(DETAIL_MIN, DETAIL_MAX);
    egui::Panel::right("parts_detail")
        .exact_size(detail_width)
        .resizable(false)
        .show(ui, |ui| detail_ui(ui, win, actions));
    egui::CentralPanel::default().show(ui, |ui| list_ui(ui, win, &rows, actions));
}

/// The part table: maker, order number, description and whether the part is
/// in the library. A click selects the row; the detail pane shows it.
fn list_ui(
    ui: &mut egui::Ui,
    win: &mut PartsWindow,
    rows: &[(PartKey, RowAction)],
    actions: &mut Vec<PartsAction>,
) {
    if rows.is_empty() {
        ui.add_space(12.0);
        if win.query.trim().is_empty() && win.maker.is_none() {
            ui.weak(
                "The catalog is empty. Press Import catalog files above, or use File > Import parts > Import parts from a folder to load a whole folder of maker files.",
            );
        } else {
            ui.weak("No part matches. Change the search or the maker.");
        }
        return;
    }
    let columns = [
        ("Maker", TableAlign::Left),
        ("Order no.", TableAlign::Left),
        ("Description", TableAlign::Left),
        ("Library", TableAlign::Right),
    ]
    .map(|(header, align)| TableColumn {
        header: header.to_owned(),
        align,
    });
    let widths = column_widths(ui.available_width(), &[1.1, 1.5, 3.0, 1.5]);
    table_header(ui, &columns, &widths, None);
    let mut area = egui::ScrollArea::vertical()
        .id_salt("parts_rows")
        .auto_shrink([false, false]);
    if std::mem::take(&mut win.reveal)
        && let Some(at) = win
            .selected
            .as_ref()
            .and_then(|k| rows.iter().position(|(r, _)| r == k))
    {
        let step = ROW_HEIGHT + ui.spacing().item_spacing.y;
        area = area.vertical_scroll_offset((at as f32 - 3.0).max(0.0) * step);
    }
    area.show_rows(ui, ROW_HEIGHT, rows.len(), |ui, range| {
        for (key, action) in &rows[range] {
            let Some(part) = win.catalog.get(key) else {
                continue;
            };
            let library = match action {
                RowAction::InLibrary => "In library",
                _ => "Not imported",
            };
            let trust = super::parts_quality::trust_at(win.catalog_path.as_deref(), part);
            let description = match super::parts_quality::trust_tag(&trust) {
                Some(tag) => format!("[{tag}] {}", part.description),
                None => part.description.clone(),
            };
            let cells = [
                TableCell {
                    text: &part.manufacturer,
                    numeric: false,
                },
                TableCell {
                    text: &part.order_number,
                    numeric: true,
                },
                TableCell {
                    text: &description,
                    numeric: false,
                },
                TableCell {
                    text: library,
                    numeric: false,
                },
            ];
            let state = if win.selected.as_ref() == Some(key) {
                RowState::Selected
            } else {
                RowState::Normal
            };
            let response = table_row(ui, &columns, &widths, &cells, state, ("part_row", key));
            if response.clicked() {
                actions.push(PartsAction::Select(key.clone()));
            }
        }
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
            let minutes = (super::parts_import::WAIT_LIMIT_SECONDS / 60.0).round();
            ui.weak(format!(
                "Waiting for the download: {}. If nothing arrives within {minutes} minutes this stops by \
itself; Stop ends it now.",
                import.waited_text()
            ));
            let passed = import.passed_over();
            if !passed.is_empty() {
                ui.colored_label(
                    ui.visuals().warn_fg_color,
                    format!(
                        "Arrived in Downloads but not taken, because the name does not mention {}: \
{}. If one of them is the file, press Choose file\u{2026} and pick it.",
                        import.label,
                        passed.join(", ")
                    ),
                );
            }
            if let Some(w) = &import.order_warning {
                ui.colored_label(ui.visuals().warn_fg_color, w);
            }
            for (i, step) in import.steps.iter().enumerate() {
                ui.label(format!("{}. {step}", i + 1));
            }
            if import.page_url.is_some() && !import.page_verified {
                ui.weak(
                    "This page address is not confirmed. If it does not show the part, find it \
on the maker's site and drop the file here.",
                );
            }
            ui.horizontal(|ui| {
                if let Some(url) = &import.page_url
                    && ui.button("Open page again").clicked()
                {
                    actions.push(PartsAction::OpenPage(url.clone()));
                }
                if ui.button("Choose file\u{2026}").clicked() {
                    actions.push(PartsAction::ChooseFile);
                }
                if ui.button("Watch another folder\u{2026}").clicked() {
                    actions.push(PartsAction::ChooseFolder);
                }
                if ui.button("Stop").clicked() {
                    actions.push(PartsAction::StopImport);
                }
            });
        }
        Phase::Reading => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.strong(format!("Reading the file for {}", import.label));
            });
            ui.weak("This runs in the background; you can keep working.");
            if ui.button("Stop").clicked() {
                actions.push(PartsAction::StopImport);
            }
        }
        Phase::Confirm(path) => {
            ui.strong(format!("Importing {}", import.label));
            ui.label(format!(
                "{} arrived in Downloads, but its name does not mention this part. Import it?",
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            ));
            ui.horizontal(|ui| {
                if ui.button("Import it").clicked() {
                    actions.push(PartsAction::ConfirmFile(path.clone()));
                }
                if ui.button("Not this one").clicked() {
                    actions.push(PartsAction::DeclineFile(path.clone()));
                }
            });
        }
        Phase::Done(summary) => {
            ui.colored_label(
                crate::theme::ui_palette().success,
                format!("Imported {} ({})", import.label, summary.file),
            );
            for line in &summary.lines {
                ui.label(line);
            }
            if ui.button("Done").clicked() {
                actions.push(PartsAction::StopImport);
            }
        }
        Phase::Failed(e) => {
            ui.colored_label(
                ui.visuals().error_fg_color,
                format!("Could not import {}: {e}", import.label),
            );
            ui.horizontal(|ui| {
                if ui.button("Try again").clicked() {
                    actions.push(PartsAction::Import(import.key.clone()));
                }
                if ui.button("Dismiss").clicked() {
                    actions.push(PartsAction::StopImport);
                }
            });
        }
    }
}

fn detail_ui(ui: &mut egui::Ui, win: &mut PartsWindow, actions: &mut Vec<PartsAction>) {
    let Some(part) = win
        .selected
        .as_ref()
        .and_then(|k| win.catalog.get(k))
        .cloned()
    else {
        ui.add_space(8.0);
        ui.weak("Pick a part in the list to see its data.");
        return;
    };
    let part = &part;
    let p = Palette::of(ui);
    let key = part.key();
    let records = win.records(&key);
    let mut pins = std::mem::take(&mut win.pins);
    ui.add_space(4.0);
    egui::ScrollArea::vertical()
        .id_salt("parts_detail_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add(egui::Label::new(egui::RichText::new(&part.order_number).heading()).wrap());
            ui.label(&part.manufacturer);
            if !part.description.is_empty() {
                ui.label(egui::RichText::new(&part.description).color(p.text_secondary));
            }
            super::parts_quality::trust_badge(
                ui,
                &super::parts_quality::trust_at(win.catalog_path.as_deref(), part),
            );
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if widgets::primary_button(ui, "Use for selected device")
                    .on_hover_text(
                        "Gives the devices selected on the drawing this part (one undo step)",
                    )
                    .clicked()
                {
                    actions.push(PartsAction::UseForSelection(key.clone()));
                }
                if !win.in_library(&key) && ui.button("Import").clicked() {
                    actions.push(PartsAction::Import(key.clone()));
                }
            });
            for section in detail_sections(part) {
                widgets::collapsing_section(
                    ui,
                    ("parts_detail", section.title),
                    section.title,
                    &section.rows.len().to_string(),
                    section.title != "Technical data",
                    |ui| {
                        for (label, value) in &section.rows {
                            detail_row(ui, label, value);
                        }
                    },
                );
            }
            widgets::collapsing_section(ui, "parts_source", "Source", "", false, |ui| {
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
            });
            widgets::collapsing_section(ui, "parts_model", "3D model", "", true, |ui| {
                ui.label(model_status(win.model_state(part, &records).as_ref()));
                if ui.button("Import 3D model (STEP)...").clicked() {
                    actions.push(PartsAction::ChooseModel);
                }
            });
            let has_step = records
                .iter()
                .any(|r| FileKind::of_file_name(&r.file_name) == FileKind::Step);
            match super::parts_pins::block(ui, &mut pins, part, has_step) {
                Some(super::parts_pins::PinsAction::Detect) => {
                    actions.push(PartsAction::DetectPins(key.clone()));
                }
                Some(super::parts_pins::PinsAction::Save) => {
                    actions.push(PartsAction::SavePins(key.clone()));
                }
                Some(super::parts_pins::PinsAction::SaveTyped(edits)) => {
                    actions.push(PartsAction::SaveTypedPins(key.clone(), edits));
                }
                None => {}
            }
            if !records.is_empty() {
                ui.add_space(8.0);
                if win.confirm_remove.as_ref() == Some(&key) {
                    ui.label("Delete the saved files? You can download them again from the maker.");
                    ui.horizontal(|ui| {
                        if widgets::danger_button(ui, "Remove").clicked() {
                            actions.push(PartsAction::Remove(key.clone()));
                        }
                        if ui.button("Cancel").clicked() {
                            actions.push(PartsAction::CancelRemove);
                        }
                    });
                } else if widgets::ghost_button(ui, "Remove from library").clicked() {
                    actions.push(PartsAction::AskRemove(key));
                }
            }
        });
    win.pins = pins;
}

/// One data row of the detail pane: the name on the left, the value after
/// it, wrapping instead of running past the pane.
fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    let p = Palette::of(ui);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(112.0, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(112.0);
                ui.add(egui::Label::new(egui::RichText::new(label).color(p.text_secondary)).wrap());
            },
        );
        ui.add(egui::Label::new(egui::RichText::new(value).color(p.text_primary)).wrap());
    });
}

fn model_job(part: &CatalogPart, record: &SourceRecord, library: &LocalLibrary) -> GeometryJob {
    GeometryJob {
        part_id: format!("{}|{}", part.manufacturer, part.order_number),
        content_id: record.sha256.clone(),
        step_path: library.path_of(record),
        size_mm: part
            .dimensions
            .map(|d| [d.width_mm, d.height_mm, d.depth_mm]),
    }
}

/// The status line of a part's 3D model.
pub(crate) fn model_status(state: Option<&GeometryState>) -> String {
    match state {
        None => "No 3D model. The part is drawn as a box from its catalog size.".into(),
        Some(GeometryState::None | GeometryState::Pending) => "Building the 3D model...".into(),
        Some(GeometryState::Failed(why)) => why.clone(),
        Some(GeometryState::Ready(geometry)) => match &geometry.origin {
            MeshOrigin::Step { .. } => format!(
                "3D model: STEP, {} triangles",
                geometry.detail().triangle_count()
            ),
            MeshOrigin::Box => {
                "No 3D model. The part is drawn as a box from its catalog size.".into()
            }
            MeshOrigin::Fallback(why) => fallback_text(why).to_owned(),
        },
    }
}

fn fallback_text(why: &Fallback) -> &'static str {
    match why {
        Fallback::NoModel => "No 3D model. The part is drawn as a box from its catalog size.",
        Fallback::Unreadable => {
            "KoperCAD cannot read this file as a STEP model. Import a different export from the maker."
        }
        Fallback::NoGeometry => {
            "The STEP file holds no solid shapes. Import a different export from the maker."
        }
        Fallback::TooComplex => {
            "The model is too detailed for a quick preview. Import a simpler export from the maker."
        }
        Fallback::TimedOut => "Reading the model took too long.",
        Fallback::Cancelled => "Building the model was cancelled.",
    }
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
    fn detail_shows_schema_v2_fields_and_labels_known_sources_only() {
        use novo_parts::{
            Classification, LetThroughPoint, PartRef, PartTerminal, Price, Provenance,
        };
        let mut p = part(&[
            ("rated_current_a", Value::Int(16)),
            ("breaking_capacity_icu_ka", Value::Float(10.0)),
        ]);
        p.sources
            .insert("rated_current_a".into(), DataSource::MakerDatasheet);
        p.class = Classification {
            etim: Some("EC000042".into()),
            eclass: None,
        };
        p.lifecycle = Lifecycle::PhasingOut;
        p.replaced_by = Some(PartRef {
            manufacturer: "Acme".into(),
            order_number: "A-2".into(),
        });
        p.price = Some(Price {
            amount: 12.5,
            currency: "EUR".into(),
            per: 10,
            date: "2026-10-15".into(),
        });
        p.terminals = vec![PartTerminal {
            name: "1".into(),
            kind: ConnectionKind::PushIn,
            wire_mm2: Some((0.5, 4.0)),
            side: Side::Top,
            position_mm: None,
        }];
        p.let_through = vec![LetThroughPoint {
            ik_ka: 6.0,
            i2t_a2s: 20000.0,
            ip_ka: 3.1,
        }];
        p.provenance = Some(Provenance {
            source: "import:bmecat".into(),
            file: "acme.xml".into(),
            ..Default::default()
        });
        let all = detail_sections(&p);
        let find = |title: &str, label: &str| -> String {
            all.iter()
                .find(|s| s.title == title)
                .and_then(|s| s.rows.iter().find(|r| r.0 == label))
                .unwrap_or_else(|| panic!("{title} / {label} missing"))
                .1
                .clone()
        };
        assert_eq!(
            find("Technical data", "Rated current"),
            "16 A  (maker datasheet)"
        );
        // No source recorded, so no label: unknown is not dressed up.
        assert!(!find("Technical data", "Breaking capacity icu").contains('('));
        assert_eq!(find("Classification", "ETIM class"), "EC000042");
        assert_eq!(find("Classification", "Lifecycle"), "phasing out");
        assert_eq!(find("Classification", "Replaced by"), "Acme A-2");
        assert_eq!(
            find("Order data", "List price"),
            "12.50 EUR per 10 (2026-10-15)"
        );
        assert_eq!(
            find("Terminals", "Terminal 1"),
            "0.5\u{2013}4 mm\u{b2}, push-in, top"
        );
        assert_eq!(
            find("Let-through", "At 6 kA"),
            "20000 A\u{b2}s, peak 3.1 kA"
        );
        assert_eq!(find("Source", "Origin"), "import:bmecat");
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
        let done = win
            .import
            .as_mut()
            .unwrap()
            .wait_read(std::time::Duration::from_secs(20));
        for p in done.into_iter().flat_map(|d| d.changed) {
            win.catalog.upsert(p);
        }
        let imp = win.import.as_ref().unwrap();
        assert!(matches!(imp.phase, Phase::Done(_)), "{:?}", imp.phase);
        let row = win.rows().into_iter().find(|(k, _)| *k == key).unwrap();
        assert_eq!(row.1, RowAction::InLibrary);
        assert_eq!(win.records(&key)[0].flow, ImportFlow::Manual);
    }

    #[test]
    fn model_status_says_what_is_drawn() {
        assert!(model_status(None).starts_with("No 3D model"));
        assert_eq!(
            model_status(Some(&GeometryState::Pending)),
            "Building the 3D model..."
        );
        let timed_out = novo_interop::part3d::fallback_box(None, Fallback::TimedOut);
        let state = GeometryState::Ready(std::sync::Arc::new(timed_out));
        assert!(model_status(Some(&state)).contains("too long"));
        let unreadable = novo_interop::part3d::fallback_box(None, Fallback::Unreadable);
        let state = GeometryState::Ready(std::sync::Arc::new(unreadable));
        assert!(model_status(Some(&state)).contains("Import a different export"));
        let boxed = GeometryState::Ready(std::sync::Arc::new(novo_interop::part3d::box_mesh([
            18.0, 90.0, 60.0,
        ])));
        assert!(model_status(Some(&boxed)).starts_with("No 3D model"));
    }

    /// Runs the window body for one frame in a window-sized panel and
    /// returns the actions it queued, the rectangle it used and the text it
    /// painted.
    fn frame(
        ctx: &egui::Context,
        win: &mut PartsWindow,
        size: egui::Vec2,
        events: Vec<egui::Event>,
    ) -> (Vec<PartsAction>, egui::Rect, String) {
        let mut actions = Vec::new();
        let mut used = egui::Rect::NOTHING;
        let mut out = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)),
                events,
                ..Default::default()
            },
            |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    parts_ui(ui, win, false, &mut actions);
                    used = ui.min_rect();
                });
            },
        );
        out.textures_delta.clear();
        let mut text = String::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &clipped.shape {
                text.push_str(t.galley.text());
                text.push('\n');
            }
        }
        (actions, used, text)
    }

    /// A context with the app's fonts and style, as the real window has.
    fn themed() -> egui::Context {
        let ctx = egui::Context::default();
        novo_theme::apply(&ctx);
        ctx.set_fonts(novo_theme::fonts::definitions());
        ctx
    }

    fn click(at: egui::Pos2) -> Vec<egui::Event> {
        let button = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        vec![egui::Event::PointerMoved(at), button(true), button(false)]
    }

    /// The part rows of the list are packed one under the next: clicking
    /// down the list selects every row in order and leaves no gap (the old
    /// list floated its rows apart and a click often hit nothing).
    #[test]
    fn clicking_down_the_list_selects_each_row_in_order_without_gaps() {
        for size in [egui::vec2(820.0, 480.0), egui::vec2(1240.0, 700.0)] {
            let mut win = PartsWindow::new(Catalog::starter(), None, SourceTable::builtin());
            let ctx = themed();
            frame(&ctx, &mut win, size, Vec::new());
            frame(&ctx, &mut win, size, Vec::new());
            let rows = win.rows();
            let mut seen: Vec<PartKey> = Vec::new();
            // The list is the left part of the window: sweep a column in it.
            let x = 60.0;
            let mut y = 150.0;
            while y < size.y - 30.0 {
                frame(&ctx, &mut win, size, Vec::new());
                let (actions, ..) = frame(&ctx, &mut win, size, click(egui::pos2(x, y)));
                for a in actions {
                    if let PartsAction::Select(key) = a
                        && seen.last() != Some(&key)
                    {
                        seen.push(key);
                    }
                }
                y += 4.0;
            }
            assert!(seen.len() >= 8, "{size:?}: only {} rows hit", seen.len());
            let order: Vec<&PartKey> = rows.iter().map(|(k, _)| k).collect();
            // Consecutive rows, starting at the first row hit.
            let first = order.iter().position(|k| **k == seen[0]).unwrap();
            for (n, key) in seen.iter().enumerate() {
                assert_eq!(order.get(first + n), Some(&key), "{size:?}: row {n}");
            }
        }
    }

    /// Selecting a row shows its data in the detail pane, inside the window.
    #[test]
    fn a_selected_part_shows_its_data_inside_the_window() {
        let size = egui::vec2(820.0, 480.0);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, size);
        let mut win = PartsWindow::new(Catalog::starter(), None, SourceTable::builtin());
        let ctx = themed();
        frame(&ctx, &mut win, size, Vec::new());
        let (_, _, before) = frame(&ctx, &mut win, size, Vec::new());
        assert!(before.contains("Pick a part in the list"), "{before}");
        // Pick the part on the last visible-or-not row and draw it.
        let key = win.rows()[2].0.clone();
        let part = win.catalog.get(&key).unwrap().clone();
        win.selected = Some(key);
        frame(&ctx, &mut win, size, Vec::new());
        let (_, used, text) = frame(&ctx, &mut win, size, Vec::new());
        assert!(screen.contains_rect(used), "{used:?} leaves {screen:?}");
        assert!(!text.contains("Pick a part in the list"));
        assert!(text.contains("Use for selected device"), "{text}");
        for section in detail_sections(&part) {
            assert!(
                text.to_uppercase().contains(&section.title.to_uppercase()),
                "section {} missing in {text}",
                section.title
            );
        }
    }
}

#[cfg(test)]
#[path = "parts_import_tests.rs"]
mod import_tests;
