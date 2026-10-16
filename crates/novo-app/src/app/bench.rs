//! Bench output: what a workshop that cuts, strips and crimps by hand takes
//! to the bench. Reports > Bench output (Ctrl+K "bench output") opens the
//! window; the three PDFs also have commands of their own:
//!
//! - bench cut list: wires grouped by cross-section and colour, tick boxes,
//!   strip lengths, ferrules and the cut length with the end allowance
//!   (lengths that are estimates carry a `~`);
//! - ferrule table: how many of which size and colour to crimp, twin
//!   ferrules where two wires share a clamp, spares, and the ferrule
//!   standard (colour and contact length per cross-section) it was worked
//!   out with, editable here and saved in the project;
//! - label sheets: wire flags, terminal, device and cable labels on A4
//!   sticker sheets or on tape, at 1:1.
//!
//! Files are written next to the project, like the other exports. The
//! window works on a draft of the standard and the label settings; "Save to
//! project" stores them (one undo step) and the exports always use what the
//! window shows.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_reports::{
    BenchCut, FerruleSize, FerruleStandard, FerruleTable, LabelRun, LabelSettings, LabelStock,
    Labels, Report, ReportOutput, SheetGrid, TapeStock, WireCutJob, ferrule_counts, is_estimate,
    label_items, layout_labels, stock_presets, wire_cut_jobs_with, with_spares,
};
use novo_theme::tokens;

use super::pdf::write_replacing;
use super::sheets::{hash, ours_or_free};
use super::{Level, NovoApp};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Tab {
    #[default]
    CutList,
    Ferrules,
    Labels,
}

/// What was computed for one project revision and ferrule standard.
struct Cache {
    revision: u64,
    standard: FerruleStandard,
    jobs: Vec<WireCutJob>,
    labels: ReportOutput,
}

/// The window's state: the drafts being edited and the last computation.
pub(super) struct BenchState {
    pub(super) tab: Tab,
    pub(super) standard: FerruleStandard,
    pub(super) labels: LabelSettings,
    cache: Option<Cache>,
}

impl BenchState {
    fn open(standard: FerruleStandard, labels: LabelSettings) -> BenchState {
        BenchState {
            tab: Tab::default(),
            standard,
            labels,
            cache: None,
        }
    }
}

/// What the bench PDFs are made from: the project's wire jobs under a
/// ferrule standard, and the label settings.
struct Inputs {
    standard: FerruleStandard,
    labels: LabelSettings,
}

fn pdf_options(
    title: String,
    keys: impl Iterator<Item = String>,
    language: novo_reports::i18n::Translator,
) -> novo_pdf::ReportPdfOptions {
    novo_pdf::ReportPdfOptions {
        title,
        language,
        // Rows tall enough to tick and to pencil a note on.
        row_mm: keys.map(|k| (k, 8.0)).collect(),
        ..novo_pdf::ReportPdfOptions::default()
    }
}

impl NovoApp {
    /// Opens the Bench output window on the project's saved settings.
    pub(super) fn open_bench(&mut self) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        self.wb.bench = Some(BenchState::open(
            FerruleStandard::of(project),
            LabelSettings::of(project),
        ));
    }

    /// The window's drafts when it is open, else the project's settings.
    fn bench_inputs(&self) -> Option<Inputs> {
        if let Some(state) = &self.wb.bench {
            return Some(Inputs {
                standard: state.standard.clone(),
                labels: state.labels.clone(),
            });
        }
        let project = self.backend.document()?.project();
        Some(Inputs {
            standard: FerruleStandard::of(project),
            labels: LabelSettings::of(project),
        })
    }

    fn bench_path(&mut self, what: &str, ext: &str) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        Some(ours_or_free(
            &dir,
            &format!("{stem} {what}"),
            ext,
            &self.wb.sheet_exports,
        ))
    }

    fn bench_write(&mut self, path: &Path, bytes: &[u8], what: &str) -> Option<PathBuf> {
        let replaced = path.exists();
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.wb.sheet_exports.insert(path.to_owned(), hash(bytes));
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {what}: {}", path.display()));
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not write {}: {e}", path.display()),
                );
                None
            }
        }
    }

    pub(super) fn export_bench_cut(&mut self) -> Option<PathBuf> {
        let path = self.bench_path("bench cut list", "pdf")?;
        self.export_bench_cut_to(&path)
    }

    /// Writes the bench cut list PDF to `path`.
    pub fn export_bench_cut_to(&mut self, path: &Path) -> Option<PathBuf> {
        let inputs = self.bench_inputs()?;
        if !inputs.standard.is_valid() {
            self.message(
                Level::Warning,
                "The ferrule standard has a size twice or a value that is not a number above zero",
            );
            return None;
        }
        let name = self.project_name().unwrap_or_default();
        let project = self.backend.document()?.project();
        let jobs = wire_cut_jobs_with(project, &inputs.standard);
        if jobs.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
        let report = BenchCut::build(project, &jobs);
        let options = pdf_options(
            format!("{name}  Bench cut list"),
            report.sections.iter().map(|s| s.key.clone()),
            self.report_translator(),
        );
        let bytes = novo_pdf::export_report_pdf(&[&report], &options);
        let estimates = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
        let what = if estimates == 0 {
            format!("bench cut list ({} wires)", jobs.len())
        } else {
            format!(
                "bench cut list ({} wires, {estimates} lengths are estimates)",
                jobs.len()
            )
        };
        self.bench_write(path, &bytes, &what)
    }

    /// The machine job file (CSV) under the window's draft standard, so it
    /// agrees with the bench PDFs.
    pub(super) fn export_machine_csv(&mut self) -> Option<PathBuf> {
        let path = self.bench_path("wire cut", "csv")?;
        self.export_machine_csv_to(&path)
    }

    /// Writes the machine job file to `path`.
    pub fn export_machine_csv_to(&mut self, path: &Path) -> Option<PathBuf> {
        let inputs = self.bench_inputs()?;
        if !inputs.standard.is_valid() {
            self.message(
                Level::Warning,
                "The ferrule standard has a size twice or a value that is not a number above zero",
            );
            return None;
        }
        let project = self.backend.document()?.project();
        let jobs = wire_cut_jobs_with(project, &inputs.standard);
        if jobs.is_empty() {
            self.message(Level::Info, "No wires to cut yet: connect two device pins");
            return None;
        }
        let options = novo_reports::CsvOptions {
            delimiter: ';',
            ..novo_reports::CsvOptions::default()
        };
        let text = novo_reports::wire_cut_csv(&jobs, &options);
        self.bench_write(path, text.as_bytes(), &format!("{} wires", jobs.len()))
    }

    pub(super) fn export_ferrules(&mut self) -> Option<PathBuf> {
        let path = self.bench_path("ferrules", "pdf")?;
        self.export_ferrules_to(&path)
    }

    /// Writes the ferrule table PDF to `path`.
    pub fn export_ferrules_to(&mut self, path: &Path) -> Option<PathBuf> {
        let inputs = self.bench_inputs()?;
        if !inputs.standard.is_valid() {
            self.message(
                Level::Warning,
                "The ferrule standard has a size twice or a value that is not a number above zero",
            );
            return None;
        }
        let name = self.project_name().unwrap_or_default();
        let project = self.backend.document()?.project();
        let jobs = wire_cut_jobs_with(project, &inputs.standard);
        let report = FerruleTable::build(project, &jobs, &inputs.standard);
        if report.section("ferrules").is_none() {
            let (unknown, off) = novo_reports::ends_without_ferrule(&jobs);
            let why = if jobs.is_empty() {
                "No ferrules to crimp yet: connect two device pins".to_owned()
            } else if unknown > 0 {
                format!(
                    "No ferrules to crimp: {unknown} wire ends have no wire size. Set the size on the wires (select them, Inspector)"
                )
            } else if off > 0 {
                "No ferrules to crimp: ferrules are switched off on every wire".to_owned()
            } else {
                "No ferrules to crimp yet".to_owned()
            };
            self.message(Level::Warning, why);
            return None;
        }
        let options = pdf_options(
            format!("{name}  Ferrules"),
            std::iter::once("ferrules".to_owned()),
            self.report_translator(),
        );
        let bytes = novo_pdf::export_report_pdf(&[&report], &options);
        self.bench_write(path, &bytes, "ferrule table")
    }

    /// The label run for the current settings.
    fn label_run(&self, inputs: &Inputs) -> Option<(LabelRun, usize)> {
        let project = self.backend.document()?.project();
        let report = Labels.generate(project);
        let items = label_items(&report, &inputs.labels.kinds);
        let count = items.len();
        Some((layout_labels(&items, &inputs.labels), count))
    }

    pub(super) fn export_labels(&mut self) -> Option<PathBuf> {
        let path = self.bench_path("labels", "pdf")?;
        self.export_labels_to(&path)
    }

    /// Writes the label sheets PDF to `path`.
    pub fn export_labels_to(&mut self, path: &Path) -> Option<PathBuf> {
        let inputs = self.bench_inputs()?;
        if !inputs.labels.is_valid() {
            self.message(
                Level::Warning,
                "The label stock does not fit its sheet: check the sizes",
            );
            return None;
        }
        let (run, items) = self.label_run(&inputs)?;
        if run.pages.is_empty() {
            self.message(Level::Info, "No labels to print for the chosen kinds yet");
            return None;
        }
        let name = self.project_name().unwrap_or_default();
        let bytes = novo_pdf::export_layouts_pdf(&run.pages, &format!("{name}  Labels"), &[]);
        let mut what = format!("{} labels on {} {}", run.labels, run.pages.len(), {
            if run.per_page > 1 {
                "sheets"
            } else {
                "pieces of tape"
            }
        });
        if items != run.labels {
            what.push_str(&format!(
                " ({items} labels x {} copies)",
                inputs.labels.copies
            ));
        }
        if run.too_small > 0 {
            what.push_str(&format!(
                "; {} have text under 2 mm, use bigger labels",
                run.too_small
            ));
        }
        self.bench_write(path, &bytes, &what)
    }

    /// The chosen label kinds as Brother P-touch Editor database files, one
    /// per kind.
    pub(super) fn export_labels_csv(&mut self) -> Vec<PathBuf> {
        let Some((dir, stem)) = self.export_base() else {
            return Vec::new();
        };
        self.export_labels_csv_in(&dir, &stem)
    }

    /// [`Self::export_labels_csv`] into `dir`, files named after `stem`.
    pub fn export_labels_csv_in(&mut self, dir: &Path, stem: &str) -> Vec<PathBuf> {
        let Some(inputs) = self.bench_inputs() else {
            return Vec::new();
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        let report = Labels.generate(project);
        let kinds: Vec<String> = if inputs.labels.kinds.is_empty() {
            Labels::KINDS.iter().map(|k| (*k).to_owned()).collect()
        } else {
            inputs.labels.kinds.clone()
        };
        let mut out = Vec::new();
        for kind in kinds {
            if report.section(&kind).is_none() {
                continue;
            }
            let text = novo_reports::labels_csv(
                &report,
                &[kind.as_str()],
                novo_reports::LabelLayout::Brother,
            );
            let path = ours_or_free(
                dir,
                &format!("{stem} labels {kind} brother"),
                "csv",
                &self.wb.sheet_exports,
            );
            if let Some(p) = self.bench_write(&path, text.as_bytes(), &format!("{kind} labels")) {
                out.push(p);
            }
        }
        if out.is_empty() {
            self.message(Level::Info, "No labels to export for the chosen kinds yet");
        }
        out
    }

    // ---------------------------------------------------------------- window

    pub(super) fn bench_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.bench.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let revision = project.revision();
        let stale = state
            .cache
            .as_ref()
            .is_none_or(|c| c.revision != revision || c.standard != state.standard);
        if stale && state.standard.is_valid() {
            state.cache = Some(Cache {
                revision,
                standard: state.standard.clone(),
                jobs: wire_cut_jobs_with(project, &state.standard),
                labels: Labels.generate(project),
            });
        }
        let stored_standard = FerruleStandard::of(project);
        let stored_labels = LabelSettings::of(project);
        let rules = novo_routing::WireRules::of(project);

        let mut open = true;
        let mut action: Option<Act> = None;
        super::panel::dialog(ctx, "Bench output", [560.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut state.tab, Tab::CutList, "Cut list");
                    ui.selectable_value(&mut state.tab, Tab::Ferrules, "Ferrules");
                    ui.selectable_value(&mut state.tab, Tab::Labels, "Labels");
                });
                ui.separator();
                match state.tab {
                    Tab::CutList => cut_tab(ui, &state, &rules, &mut action),
                    Tab::Ferrules => ferrule_tab(ui, &mut state, &stored_standard, &mut action),
                    Tab::Labels => label_tab(ui, &mut state, &stored_labels, &mut action),
                }
            });
        if open {
            self.wb.bench = Some(state);
        }
        match action {
            Some(Act::ExportCut) => {
                self.export_bench_cut();
            }
            Some(Act::ExportMachineCsv) => {
                self.export_machine_csv();
            }
            Some(Act::ExportFerrules) => {
                self.export_ferrules();
            }
            Some(Act::ExportLabels) => {
                self.export_labels();
            }
            Some(Act::ExportLabelsCsv) => {
                self.export_labels_csv();
            }
            Some(Act::SaveStandard(standard)) => {
                self.cabinet_edit("Ferrule standard", move |tx| {
                    FerruleStandard::store(tx, &standard)
                });
            }
            Some(Act::SaveLabels(labels)) => {
                self.cabinet_edit("Label settings", move |tx| {
                    LabelSettings::store(tx, &labels)
                });
            }
            None => {}
        }
    }
}

enum Act {
    ExportCut,
    ExportMachineCsv,
    ExportFerrules,
    ExportLabels,
    ExportLabelsCsv,
    SaveStandard(FerruleStandard),
    SaveLabels(LabelSettings),
}

fn cut_tab(
    ui: &mut egui::Ui,
    state: &BenchState,
    rules: &novo_routing::WireRules,
    act: &mut Option<Act>,
) {
    let Some(cache) = state.cache.as_ref().filter(|_| state.standard.is_valid()) else {
        ui.colored_label(
            tokens::WARNING,
            "The ferrule standard is not valid: fix it on the Ferrules tab",
        );
        return;
    };
    let jobs = &cache.jobs;
    if jobs.is_empty() {
        ui.weak("No wires to cut yet: connect two device pins");
        return;
    }
    let metres: f64 = jobs.iter().map(|j| j.cut_mm).sum::<f64>() / 1000.0;
    let estimates = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
    let groups = jobs
        .windows(2)
        .filter(|w| w[0].cross_section_mm2 != w[1].cross_section_mm2 || w[0].colour != w[1].colour)
        .count()
        + 1;
    ui.label(format!(
        "{} wires in {groups} groups, {metres:.1} m of wire to pull",
        jobs.len()
    ));
    if estimates > 0 {
        ui.colored_label(
            tokens::WARNING,
            format!(
                "{estimates} lengths are estimates and print with a ~: the wire is not routed through a duct, or a device has no pin position. Measure those on the plate."
            ),
        );
    } else {
        ui.colored_label(tokens::SUCCESS, "Every length comes from a routed wire");
    }
    ui.add_space(4.0);
    ui.weak(format!(
        "Cut length = route + {} mm per end for a ferrule ({} mm from {} mm² for a lug). Change the allowances under Cabinet > Routes.",
        rules.ferrule_mm, rules.lug_mm, rules.lug_from_mm2
    ));
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if ui
            .button("Export bench cut list (PDF)")
            .on_hover_text("Grouped by size and colour, longest first, with tick boxes")
            .clicked()
        {
            *act = Some(Act::ExportCut);
        }
        if ui
            .button("Machine CSV")
            .on_hover_text("The same wires as one CSV for a wire-processing machine")
            .clicked()
        {
            *act = Some(Act::ExportMachineCsv);
        }
    });
}

fn ferrule_tab(
    ui: &mut egui::Ui,
    state: &mut BenchState,
    stored: &FerruleStandard,
    act: &mut Option<Act>,
) {
    ui.strong("Ferrule standard");
    ui.weak(
        "Colour and contact length per cross-section. The default is the DIN 46228-4 colour code as commonly sold; makers differ, above 16 mm² in particular. Edit it to match your stock.",
    );
    let mut remove = None;
    egui::Grid::new("bench_standard")
        .num_columns(4)
        .spacing([8.0, 3.0])
        .show(ui, |ui| {
            ui.weak("mm²");
            ui.weak("Colour");
            ui.weak("Contact mm");
            ui.label("");
            ui.end_row();
            for (i, size) in state.standard.sizes.iter_mut().enumerate() {
                ui.add(
                    egui::DragValue::new(&mut size.mm2)
                        .speed(0.05)
                        .range(0.0..=300.0)
                        .max_decimals(2),
                );
                ui.add(egui::TextEdit::singleline(&mut size.colour).desired_width(90.0));
                ui.add(
                    egui::DragValue::new(&mut size.length_mm)
                        .speed(0.5)
                        .range(0.0..=60.0)
                        .max_decimals(1),
                );
                if ui
                    .small_button("x")
                    .on_hover_text("Remove this size")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        state.standard.sizes.remove(i);
    }
    ui.horizontal(|ui| {
        if ui.small_button("Add size").clicked() {
            state.standard.sizes.push(FerruleSize {
                mm2: 0.0,
                colour: String::new(),
                length_mm: 10.0,
            });
        }
        if ui.small_button("Sort").clicked() {
            state.standard.sort();
        }
        if ui.small_button("Reset to default").clicked() {
            state.standard = FerruleStandard::default();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Spares");
        ui.add(
            egui::DragValue::new(&mut state.standard.spare_percent)
                .range(0.0..=100.0)
                .suffix(" %"),
        );
        ui.label("Twin ferrules up to");
        ui.add(
            egui::DragValue::new(&mut state.standard.twin_to_mm2)
                .range(0.0..=50.0)
                .suffix(" mm²"),
        )
        .on_hover_text("Where two wires of one size end on one clamp. 0 turns twins off");
    });
    let valid = state.standard.is_valid();
    if !valid {
        ui.colored_label(
            tokens::WARNING,
            "Each size must be listed once, above zero, with a contact length above zero",
        );
    }
    ui.separator();
    ui.strong("To crimp");
    match &state.cache {
        Some(cache) if valid => {
            let counts = ferrule_counts(&cache.jobs, &state.standard);
            if counts.is_empty() {
                ui.weak("No ferrules yet: connect two device pins and set the wire sizes");
            }
            for c in &counts {
                ui.label(format!(
                    "{} {}  {}  x {} (prepare {})",
                    c.ferrule,
                    if c.twin { "twin" } else { "" },
                    c.colour,
                    c.pieces,
                    with_spares(c.pieces, state.standard.spare_percent)
                ));
            }
        }
        _ => {}
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let changed = state.standard != *stored;
        if ui
            .add_enabled(valid && changed, egui::Button::new("Save to project"))
            .on_hover_text("Keep this standard in the project file (one undo step)")
            .clicked()
        {
            *act = Some(Act::SaveStandard(state.standard.clone()));
        }
        if ui
            .add_enabled(valid, egui::Button::new("Export ferrule table (PDF)"))
            .clicked()
        {
            *act = Some(Act::ExportFerrules);
        }
        if changed {
            ui.weak("not saved yet");
        }
    });
}

fn label_tab(
    ui: &mut egui::Ui,
    state: &mut BenchState,
    stored: &LabelSettings,
    act: &mut Option<Act>,
) {
    ui.strong("Print which labels");
    ui.horizontal(|ui| {
        for (kind, title) in [
            ("wires", "Wire flags"),
            ("terminals", "Terminals"),
            ("devices", "Devices"),
            ("cables", "Cables"),
        ] {
            let count = state
                .cache
                .as_ref()
                .and_then(|c| c.labels.section(kind))
                .map_or(0, |s| s.rows.len());
            let mut on = state.labels.kinds.iter().any(|k| k == kind);
            if ui.checkbox(&mut on, format!("{title} ({count})")).changed() {
                state.labels.kinds.retain(|k| k != kind);
                if on {
                    state.labels.kinds.push(kind.to_owned());
                }
            }
        }
    });
    ui.add_space(4.0);
    ui.strong("Label stock");
    let name = state.labels.stock.name();
    egui::ComboBox::from_id_salt("bench_stock")
        .selected_text(name)
        .width(240.0)
        .show_ui(ui, |ui| {
            for preset in stock_presets() {
                let selected = state.labels.stock == preset;
                if ui.selectable_label(selected, preset.name()).clicked() {
                    state.labels.stock = preset;
                }
            }
        });
    match &mut state.labels.stock {
        LabelStock::Sheet(g) => sheet_fields(ui, g),
        LabelStock::Tape(t) => tape_fields(ui, t),
    }
    ui.horizontal(|ui| {
        ui.label("Copies of each");
        ui.add(egui::DragValue::new(&mut state.labels.copies).range(1..=99));
        if matches!(state.labels.stock, LabelStock::Sheet(_)) {
            ui.label("Start at label");
            ui.add(egui::DragValue::new(&mut state.labels.start_slot).range(1..=999))
                .on_hover_text(
                    "Continue on a partly used sheet: the first free sticker, counted row by row",
                );
        }
    });
    ui.collapsing("Printer fit", |ui| {
        ui.checkbox(
            &mut state.labels.outlines,
            "Test print: draw a line around every label",
        );
        ui.horizontal(|ui| {
            ui.label("Shift");
            ui.add(
                egui::DragValue::new(&mut state.labels.offset_x)
                    .speed(0.1)
                    .range(-20.0..=20.0)
                    .suffix(" mm across"),
            );
            ui.add(
                egui::DragValue::new(&mut state.labels.offset_y)
                    .speed(0.1)
                    .range(-20.0..=20.0)
                    .suffix(" mm down"),
            );
        });
        ui.weak("Print at 100% (actual size), not fit to page. Test on plain paper and hold it against the sheet to the light.");
    });
    let valid = state.labels.is_valid();
    if !valid {
        ui.colored_label(
            tokens::WARNING,
            "The label sizes do not fit the sheet: check the sheet fields",
        );
    }
    if let (Some(cache), true) = (&state.cache, valid) {
        let items = label_items(&cache.labels, &state.labels.kinds);
        let run = layout_labels(&items, &state.labels);
        if run.pages.is_empty() {
            ui.weak("No labels for the chosen kinds yet");
        } else {
            ui.label(format!(
                "{} labels on {} {}",
                run.labels,
                run.pages.len(),
                if run.per_page > 1 {
                    "sheets"
                } else {
                    "pieces of tape"
                }
            ));
        }
        if run.too_small > 0 {
            ui.colored_label(
                tokens::WARNING,
                format!(
                    "{} labels have text under 2 mm: choose bigger labels or wider tape",
                    run.too_small
                ),
            );
        }
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        let changed = state.labels != *stored;
        if ui
            .add_enabled(valid && changed, egui::Button::new("Save to project"))
            .clicked()
        {
            *act = Some(Act::SaveLabels(state.labels.clone()));
        }
        if ui
            .add_enabled(valid, egui::Button::new("Export label sheets (PDF)"))
            .clicked()
        {
            *act = Some(Act::ExportLabels);
        }
        if ui
            .button("P-touch CSV")
            .on_hover_text("One file per kind for Brother P-touch Editor (tab separated)")
            .clicked()
        {
            *act = Some(Act::ExportLabelsCsv);
        }
    });
}

fn sheet_fields(ui: &mut egui::Ui, g: &mut SheetGrid) {
    ui.collapsing("Sheet sizes", |ui| {
        let before = g.clone();
        egui::Grid::new("bench_sheet")
            .num_columns(4)
            .spacing([8.0, 3.0])
            .show(ui, |ui| {
                let mm = |ui: &mut egui::Ui, v: &mut f64| {
                    ui.add(
                        egui::DragValue::new(v)
                            .speed(0.1)
                            .range(0.0..=500.0)
                            .suffix(" mm"),
                    );
                };
                ui.label("Columns");
                ui.add(egui::DragValue::new(&mut g.cols).range(1..=50));
                ui.label("Rows");
                ui.add(egui::DragValue::new(&mut g.rows).range(1..=100));
                ui.end_row();
                ui.label("Label width");
                mm(ui, &mut g.label_w);
                ui.label("Label height");
                mm(ui, &mut g.label_h);
                ui.end_row();
                ui.label("Left margin");
                mm(ui, &mut g.left);
                ui.label("Top margin");
                mm(ui, &mut g.top);
                ui.end_row();
                ui.label("Pitch across");
                mm(ui, &mut g.pitch_x);
                ui.label("Pitch down");
                mm(ui, &mut g.pitch_y);
                ui.end_row();
            });
        ui.weak("Pitch is the distance from one label's corner to the next (label plus gap).");
        if *g != before {
            g.name = "Custom A4 sheet".to_owned();
        }
    });
}

fn tape_fields(ui: &mut egui::Ui, t: &mut TapeStock) {
    ui.horizontal(|ui| {
        let mut auto = t.length_mm.is_none();
        if ui.checkbox(&mut auto, "Length follows the text").changed() {
            t.length_mm = if auto { None } else { Some(40.0) };
        }
        if let Some(len) = &mut t.length_mm {
            ui.add(egui::DragValue::new(len).range(5.0..=300.0).suffix(" mm"));
        }
    });
    ui.weak("One label per page, as wide as the tape. Print it from the printer's driver at the tape size.");
}
