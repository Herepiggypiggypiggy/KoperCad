//! Variants and generated engineering in the app (docs/design/variants.md).
//!
//! - **Project options and variants** (Ctrl+K `options`, or `variant <name>`):
//!   the machine's options (with or without a conveyor, 400 V or 480 V), the
//!   conditions that tie pages and devices to them, named variants, and what the
//!   current choice includes: pages, devices and the bill of materials, with
//!   an exact "what does this add" against any variant. *Export PDF of a
//!   variant* writes the documents of one machine without touching the project.
//! - **Generate pages from a spec sheet** (Ctrl+K `generate`): pick a `.xlsx`
//!   or CSV with one row per macro placement, see every page and tag it would
//!   make and every problem with its line, then generate as one undo step.
//!   *Export the spec sheet* writes the same sheet back from the project, so
//!   Excel can edit it and the pages regenerate.
//!
//! The engines are `novo_workflow::{options, parametric, generate, expr}`;
//! this file is the thin window around them.

use std::collections::BTreeMap;
use std::path::PathBuf;

use eframe::egui;
use novo_core::{AnyId, Project, Value};
use novo_interop::sheets::{read_grid, write_grid};
use novo_workflow::generate::{
    Generated, Mode, SheetProblem, SpecSheet, generate, macros_used, preview, spec_from_project,
};
use novo_workflow::options::{
    self, Assignment, Clause, Configuration, Difference, Finding, OptionDef, clauses_of,
    condition_from_clauses, condition_of, tests_for,
};
use novo_workflow::parametric::ParamKind;

use super::pdf::write_replacing;
use super::sheets::{hash, ours_or_free};
use super::{Level, NovoApp};

/// What kind of option the add form makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum NewKind {
    #[default]
    YesNo,
    Choice,
    Number,
}

/// The add-option form.
#[derive(Debug, Clone, Default)]
struct NewOption {
    id: String,
    label: String,
    kind: NewKind,
    /// Choices, comma separated.
    choices: String,
    min: f64,
    max: f64,
    default_number: f64,
}

/// The Project options window while it is open.
#[derive(Debug, Default)]
pub(crate) struct OptionsWindow {
    new: NewOption,
    variant_name: String,
    /// The variant the current choice is compared with; empty for none.
    compare: String,
    /// The expression text boxes under Advanced, with what each was last
    /// filled from (so an outside change, like Tie, shows up in them).
    page_raw: ConditionText,
    device_raw: ConditionText,
    error: Option<String>,
    /// What the window shows, kept until the project or the choice changes.
    shown: Option<Shown>,
}

#[derive(Debug, Default)]
struct ConditionText {
    text: String,
    synced: Option<(String, Option<String>)>,
}

#[derive(Debug)]
struct Shown {
    revision: u64,
    compare: String,
    configuration: Configuration,
    difference: Option<Difference>,
    findings: Vec<Finding>,
    /// Heat inside the cabinets with the current choice, and with the
    /// variant it is compared with.
    heat: Option<CabinetHeat>,
    heat_other: Option<CabinetHeat>,
}

/// The sum of the device losses inside the cabinets of one choice.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct CabinetHeat {
    /// Watts at the load factors the parts and devices carry.
    pub watts: f64,
    /// Cabinet devices with no loss figure (counted as 0 W).
    pub without_figure: usize,
}

/// Heat inside the cabinets when the machine is built as `assignment`: the
/// sum of the losses the parts carry (`power_loss_w`), as the Cabinet
/// sizing reads them. A model of the heat the devices put in, not of the
/// temperature; the Heat view does that.
pub(super) fn cabinet_heat(project: &Project, assignment: &Assignment) -> Option<CabinetHeat> {
    let made = options::variant_project(project, assignment).ok()?;
    let (mut watts, mut without_figure) = (0.0, 0);
    for structure in made.structures().filter(|s| s.enclosure) {
        let devices = novo_layout::sizing_model::sizing_devices(&made, structure.id);
        watts += novo_layout::sizing::total_loss(&devices);
        without_figure += devices.iter().filter(|d| d.power_loss <= 0.0).count();
    }
    Some(CabinetHeat {
        watts,
        without_figure,
    })
}

/// The Generate window while it is open.
#[derive(Default)]
pub(crate) struct GenerateWindow {
    path: Option<PathBuf>,
    sheet: Option<SpecSheet>,
    macros: BTreeMap<String, novo_core::BlockDefinition>,
    pub(super) missing: Vec<String>,
    mode: Mode,
    result: Option<Result<Generated, Vec<SheetProblem>>>,
    /// Project revision and mode the result was worked out for.
    result_for: Option<(u64, Mode, Assignment)>,
    error: Option<String>,
}

impl GenerateWindow {
    #[cfg(test)]
    pub(super) fn sheet_for_test(&self) -> Option<&SpecSheet> {
        self.sheet.as_ref()
    }

    #[cfg(test)]
    pub(super) fn macros_for_test(&self) -> &BTreeMap<String, novo_core::BlockDefinition> {
        &self.macros
    }
}

fn value_text(v: &Value) -> String {
    v.to_string()
}

impl NovoApp {
    pub(super) fn open_options(&mut self) {
        if self.backend.document().is_some() {
            self.wb.options = Some(OptionsWindow::default());
        }
    }

    pub(super) fn open_generate(&mut self) {
        if self.backend.document().is_some() {
            self.wb.generate = Some(GenerateWindow::default());
        }
    }

    /// `variant <name>`: makes a saved variant the current choice.
    pub(super) fn switch_variant(&mut self, name: Option<&str>) {
        let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) else {
            self.open_options();
            return;
        };
        self.edit_project("Switch variant", |tx| options::apply_variant(tx, name));
    }

    /// One undo step; a refusal is shown as a message.
    pub(super) fn edit_project(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<()>,
    ) -> bool {
        let Some(doc) = self.backend.document_mut() else {
            return false;
        };
        match doc.transact(label, edit) {
            Ok(((), changes)) => {
                self.changed_outside_canvas(&changes);
                true
            }
            Err(e) => {
                self.message(Level::Error, format!("{label}: {e}"));
                false
            }
        }
    }

    // ----- project options -----------------------------------------------

    pub(super) fn options_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.options.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        let current = options::current(&project);
        let defs = options::options(&project);
        let variants = options::variants(&project);
        let page = self.current_page;
        let selected_device = super::inspector::selected_device(&project, &self.selection());

        if win
            .shown
            .as_ref()
            .is_none_or(|s| s.revision != project.revision() || s.compare != win.compare)
        {
            let difference = (!win.compare.is_empty())
                .then(|| options::variant_assignment(&project, &win.compare).ok())
                .flatten()
                .map(|other| options::diff(&project, &other, &current));
            win.shown = Some(Shown {
                revision: project.revision(),
                compare: win.compare.clone(),
                configuration: options::configuration(&project, &current),
                difference,
                findings: options::lint(&project),
                heat: cabinet_heat(&project, &current),
                heat_other: (!win.compare.is_empty())
                    .then(|| options::variant_assignment(&project, &win.compare).ok())
                    .flatten()
                    .and_then(|other| cabinet_heat(&project, &other)),
            });
        }
        let mut set: Vec<(String, Value)> = Vec::new();
        let mut remove: Option<String> = None;
        let mut define: Option<OptionDef> = None;
        let mut save_variant = false;
        let mut apply: Option<String> = None;
        let mut delete: Option<String> = None;
        let mut page_condition: Option<Option<String>> = None;
        let mut device_condition: Option<Option<String>> = None;
        let mut export_variant = false;
        let mut tie: Option<(OptionDef, Value)> = None;
        let mut untie = false;
        let targets = self.tie_targets();
        let nothing_selected = self.selection().is_empty();
        let tie_words = self.targets_words(&targets);
        let tie_words = if nothing_selected && !tie_words.is_empty() {
            format!("Nothing is selected, so this acts on {tie_words}")
        } else {
            tie_words
        };
        let tie_state = targets
            .first()
            .and_then(|t| options::describe_condition(&project, *t))
            .unwrap_or_else(|| "always appears".to_owned());
        let mut open = true;

        super::panel::dialog(ctx, "Project options and variants", [560.0, 640.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    novo_theme::widgets::section_label(ui, "Options");
                    ui.weak(
                        "What this machine can have. Pages and devices tied to an option appear in \
the documents only when it is on; the working project always keeps everything, and the \
Schematic and Cabinet always show everything.",
                    );
                    if defs.is_empty() {
                        ui.add_space(4.0);
                        ui.label("No options yet. Add one below, for example Conveyor.");
                    }
                    for def in &defs {
                        ui.horizontal(|ui| {
                            ui.label(def.label());
                            let mut value = current
                                .get(&def.id)
                                .cloned()
                                .unwrap_or_else(|| def.default.clone());
                            if option_editor(ui, def, &mut value)
                                && current.get(&def.id) != Some(&value)
                            {
                                set.push((def.id.clone(), value));
                            }
                            ui.weak(format!("opt.{}", def.id));
                            if ui.small_button("Remove").clicked() {
                                remove = Some(def.id.clone());
                            }
                        });
                    }
                    ui.add_space(4.0);
                    ui.collapsing("Add an option", |ui| {
                        add_option_form(ui, &mut win.new, &mut define, &mut win.error);
                    });
                    if let Some(error) = &win.error {
                        novo_theme::widgets::banner(
                            ui,
                            novo_theme::widgets::Level::Error,
                            error,
                            |_| {},
                        );
                    }

                    ui.separator();
                    novo_theme::widgets::section_label(ui, "Variants");
                    ui.weak("A saved set of choices: Standard, With conveyor, Export 480 V.");
                    for v in &variants {
                        ui.horizontal(|ui| {
                            let is_current =
                                options::complete(&project, &v.values).is_ok_and(|a| a == current);
                            ui.label(&v.name);
                            if is_current {
                                ui.weak("current");
                            } else if ui.small_button("Apply").clicked() {
                                apply = Some(v.name.clone());
                            }
                            if ui
                                .selectable_label(win.compare == v.name, "Compare")
                                .on_hover_text(
                                    "What the current choice adds to or takes from this one",
                                )
                                .clicked()
                            {
                                win.compare = if win.compare == v.name {
                                    String::new()
                                } else {
                                    v.name.clone()
                                };
                            }
                            if ui.small_button("Delete").clicked() {
                                delete = Some(v.name.clone());
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut win.variant_name)
                                .hint_text("Name of this choice")
                                .desired_width(180.0),
                        );
                        if ui
                            .add_enabled(
                                !win.variant_name.trim().is_empty() && !defs.is_empty(),
                                egui::Button::new("Save current choice as a variant"),
                            )
                            .clicked()
                        {
                            save_variant = true;
                        }
                    });

                    ui.separator();
                    novo_theme::widgets::section_label(ui, "Conditions");
                    ui.weak(
                        "Appears when ... Pick the options a page or device needs. \
No rule means it always appears.",
                    );
                    ui.strong("Current page");
                    if let Some(page) = page {
                        let current = condition_of(&project, page);
                        if let Some(change) = condition_editor(
                            ui,
                            &format!("page-{page:?}"),
                            &defs,
                            current,
                            &mut win.page_raw,
                        ) {
                            page_condition = Some(change);
                        }
                    } else {
                        ui.weak("Open a page to give it a condition.");
                    }
                    if let Some(device) = selected_device {
                        let tag = project
                            .device(device)
                            .map(|d| d.tag.to_string())
                            .unwrap_or_default();
                        ui.strong(format!("Selected {tag}"));
                        let current = condition_of(&project, device);
                        if let Some(change) = condition_editor(
                            ui,
                            &format!("device-{device:?}"),
                            &defs,
                            current,
                            &mut win.device_raw,
                        ) {
                            device_condition = Some(change);
                        }
                    } else {
                        ui.weak("Select a device on the drawing to give it a condition too.");
                    }

                    ui.add_space(6.0);
                    ui.strong("Tie the selection");
                    ui.weak(format!(
                        "{} now: {}. Pick the answer the selection needs; the selected \
devices and wires (or the page, when nothing is selected) then appear only with it.",
                        tie_words, tie_state
                    ));
                    ui.horizontal_wrapped(|ui| {
                        for def in &defs {
                            for (label, value) in tie_choices(def) {
                                if ui
                                    .small_button(label)
                                    .on_hover_text(format!("Only while {} is this", def.label()))
                                    .clicked()
                                {
                                    tie = Some((def.clone(), value));
                                }
                            }
                        }
                        if ui
                            .small_button("Always")
                            .on_hover_text("Remove the condition: it appears in every variant")
                            .clicked()
                        {
                            untie = true;
                        }
                    });

                    if let Some(shown) = &win.shown {
                        ui.separator();
                        configuration_section(ui, shown, &win.compare);
                        if !shown.findings.is_empty() {
                            ui.separator();
                            novo_theme::widgets::section_label(ui, "Check");
                            for f in &shown.findings {
                                novo_theme::widgets::banner(
                                    ui,
                                    novo_theme::widgets::Level::Warning,
                                    &finding_text(&project, f),
                                    |_| {},
                                );
                            }
                        }
                    }

                    ui.separator();
                    if novo_theme::widgets::primary_button(ui, "Export PDF of the current choice")
                        .on_hover_text("The documents of this variant; the project is not changed")
                        .clicked()
                    {
                        export_variant = true;
                    }
                });
            });

        let mut touched = false;
        for (id, value) in set {
            touched = true;
            self.edit_project("Project option", |tx| options::set_option(tx, &id, value));
        }
        if let Some(id) = remove {
            touched = true;
            self.edit_project("Remove option", |tx| options::remove_option(tx, &id));
        }
        if let Some(def) = define {
            touched = true;
            let id = def.id.clone();
            if self.edit_project("Add option", |tx| options::define_option(tx, def)) {
                win.error = None;
                win.new = NewOption::default();
            } else {
                win.error = Some(format!(
                    "Could not add '{id}'. Use lowercase letters, digits and _."
                ));
            }
        }
        if save_variant {
            let name = win.variant_name.trim().to_owned();
            if self.edit_project("Save variant", |tx| options::save_variant(tx, &name, "")) {
                win.variant_name.clear();
            }
        }
        if let Some(name) = apply {
            touched = true;
            self.edit_project("Switch variant", |tx| options::apply_variant(tx, &name));
        }
        if let Some(name) = delete {
            if win.compare == name {
                win.compare.clear();
            }
            self.edit_project("Delete variant", |tx| options::delete_variant(tx, &name));
        }
        if let (Some(change), Some(page)) = (page_condition, page) {
            let result = self.edit_project("Page condition", |tx| {
                options::set_condition(tx, page, change.as_deref())
            });
            if !result {
                win.error = Some("That condition is not valid".into());
            }
        }
        if let (Some(change), Some(device)) = (device_condition, selected_device) {
            self.edit_project("Device condition", |tx| {
                options::set_condition(tx, device, change.as_deref())
            });
        }
        if let Some((def, value)) = tie {
            self.tie_selection(&def, &value);
        }
        if untie {
            self.untie_command();
        }
        if export_variant {
            self.export_variant_pdf(None);
        }
        let _ = touched;
        if open {
            self.wb.options = Some(win);
        }
    }

    /// `export variant <name>`: the PDF of one variant (the current choice
    /// when no name is given), next to the project, as `<name> <variant>.pdf`.
    pub(super) fn export_variant_pdf(&mut self, variant: Option<&str>) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let label = variant
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("current")
            .to_owned();
        let path = ours_or_free(
            &dir,
            &format!("{stem} {label}"),
            "pdf",
            &self.wb.sheet_exports,
        );
        self.export_variant_pdf_to(&path, variant)
    }

    /// Writes the PDF of a variant to `path`.
    pub fn export_variant_pdf_to(
        &mut self,
        path: &std::path::Path,
        variant: Option<&str>,
    ) -> Option<PathBuf> {
        let project = self.backend.document()?.project().clone();
        let (assignment, label) = match variant.map(str::trim).filter(|v| !v.is_empty()) {
            Some(name) => match options::variant_assignment(&project, name) {
                Ok(a) => (a, name.to_owned()),
                Err(e) => {
                    self.message(Level::Error, format!("Export variant: {e}"));
                    return None;
                }
            },
            None => (options::current(&project), "current".to_owned()),
        };
        let made = match options::machine_project(&project, &assignment) {
            Ok(p) => p,
            Err(e) => {
                self.message(Level::Error, format!("Export variant: {e}"));
                return None;
            }
        };
        let options = novo_pdf::PdfOptions::default();
        let art = self
            .library()
            .map(|lib| novo_editor::art::LibraryArt::new(lib, options.style.symbol));
        let result = match &art {
            Some(art) => novo_pdf::export_pdf_with_art(&made, &options, art),
            None => {
                novo_pdf::export_pdf_with_art(&made, &options, &novo_canvas::model::PlaceholderArt)
            }
        }
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            write_replacing(path, &bytes).map_err(|e| e.to_string())?;
            Ok(hash(&bytes))
        });
        match result {
            Ok(h) => {
                self.wb.sheet_exports.insert(path.to_owned(), h);
                self.message(
                    Level::Info,
                    format!(
                        "Exported the {label} variant ({} pages) to {}",
                        made.pages().count(),
                        path.display()
                    ),
                );
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the PDF: {e}"));
                None
            }
        }
    }

    // ----- spec sheet ----------------------------------------------------

    /// Writes the spec sheet of the project's placed macros as `.xlsx`.
    pub(super) fn export_spec(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = ours_or_free(
            &dir,
            &format!("{stem} spec"),
            "xlsx",
            &self.wb.sheet_exports,
        );
        self.export_spec_to(&path)
    }

    pub fn export_spec_to(&mut self, path: &std::path::Path) -> Option<PathBuf> {
        let sheet = spec_from_project(self.backend.document()?.project());
        if sheet.rows.is_empty() {
            self.message(
                Level::Info,
                "Nothing to export: no macros are placed in this project yet",
            );
            return None;
        }
        let mut rows = vec![sheet.headers.clone()];
        rows.extend(sheet.rows.iter().cloned());
        let bytes = match write_grid(&rows) {
            Ok(b) => b,
            Err(e) => {
                self.message(Level::Error, format!("Could not write the spec sheet: {e}"));
                return None;
            }
        };
        match write_replacing(path, &bytes) {
            Ok(()) => {
                self.wb.sheet_exports.insert(path.to_owned(), hash(&bytes));
                self.message(
                    Level::Info,
                    format!("Exported {} rows to {}", sheet.rows.len(), path.display()),
                );
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

    pub(super) fn generate_window(&mut self, ctx: &egui::Context) {
        let Some(mut win) = self.wb.generate.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project().clone()) else {
            return;
        };
        let assignment = options::current(&project);
        let mut pick = false;
        let mut run = false;
        let mut open = true;
        let has_problems = matches!(win.result, Some(Err(_)));

        super::panel::dialog(ctx, "Generate pages from a spec sheet", [560.0, 560.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                ui.weak(
                    "One row per macro: Macro, Page, Structure, When, then a column for each macro \
parameter. Pages and tags are made fresh; nothing is generated if any row has a problem.",
                );
                ui.horizontal(|ui| {
                    if ui.button("Choose spec sheet (.xlsx or .csv)").clicked() {
                        pick = true;
                    }
                    match &win.path {
                        Some(p) => ui.label(p.display().to_string()),
                        None => ui.weak("no file chosen"),
                    };
                });
                if let Some(error) = &win.error {
                    novo_theme::widgets::banner(
                        ui,
                        novo_theme::widgets::Level::Error,
                        error,
                        |_| {},
                    );
                }
                if win.sheet.is_some() {
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut win.mode, Mode::Add, "Add new pages");
                        ui.radio_value(
                            &mut win.mode,
                            Mode::Replace,
                            "Replace pages an earlier run made",
                        )
                        .on_hover_text(
                            "Removes the pages and devices generated before, then generates again",
                        );
                    });
                }
                for name in &win.missing {
                    novo_theme::widgets::banner(
                        ui,
                        novo_theme::widgets::Level::Warning,
                        &format!("Macro '{name}' is not in your library"),
                        |_| {},
                    );
                }
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| match &win.result {
                    None => {
                        ui.weak("Choose a spec sheet to see what it would make.");
                    }
                    Some(Err(problems)) => {
                        novo_theme::widgets::banner(
                            ui,
                            novo_theme::widgets::Level::Error,
                            &format!("{} problem(s); nothing would be generated", problems.len()),
                            |_| {},
                        );
                        for p in problems {
                            ui.label(p.to_string());
                        }
                    }
                    Some(Ok(g)) => {
                        ui.strong(format!(
                            "{} placement(s) on {} page(s)",
                            g.placements.len(),
                            g.pages.len()
                        ));
                        if g.replaced_pages > 0 {
                            ui.weak(format!(
                                "{} earlier page(s) would be replaced",
                                g.replaced_pages
                            ));
                        }
                        if !g.skipped.is_empty() {
                            ui.weak(format!(
                                "{} row(s) left out by their When condition",
                                g.skipped.len()
                            ));
                        }
                        for w in &g.warnings {
                            novo_theme::widgets::banner(
                                ui,
                                novo_theme::widgets::Level::Warning,
                                w,
                                |_| {},
                            );
                        }
                        ui.add_space(4.0);
                        for p in &g.placements {
                            for report in &p.reports {
                                let tags: Vec<String> =
                                    report.tags.iter().map(|t| t.to_string()).collect();
                                ui.label(format!(
                                    "line {}: {}  {}",
                                    p.line,
                                    p.macro_name,
                                    tags.join(", ")
                                ));
                            }
                        }
                    }
                });
                ui.separator();
                let ready = matches!(&win.result, Some(Ok(g)) if !g.placements.is_empty());
                if ui
                    .add_enabled_ui(ready && !has_problems, |ui| {
                        novo_theme::widgets::primary_button(ui, "Generate")
                    })
                    .inner
                    .clicked()
                {
                    run = true;
                }
            });

        if pick {
            self.pick_spec(&mut win);
        }
        if let Some(sheet) = &win.sheet {
            let key = (project.revision(), win.mode, assignment.clone());
            if win.result_for.as_ref() != Some(&key) {
                let macros = &win.macros;
                let lookup = |name: &str| macros.get(&name.to_lowercase()).cloned();
                win.result = Some(preview(&project, sheet, &lookup, &assignment, win.mode));
                win.result_for = Some(key);
            }
        }
        if run && let Some(sheet) = win.sheet.clone() {
            let macros = win.macros.clone();
            let lookup = move |name: &str| macros.get(&name.to_lowercase()).cloned();
            let mode = win.mode;
            let mut outcome: Option<Result<Generated, Vec<SheetProblem>>> = None;
            let Some(doc) = self.backend.document_mut() else {
                return;
            };
            match doc.transact("Generate from spec sheet", |tx| {
                generate(tx, &sheet, &lookup, &assignment, mode)
            }) {
                Ok((result, changes)) => {
                    self.changed_outside_canvas(&changes);
                    outcome = Some(result);
                }
                Err(e) => win.error = Some(e.to_string()),
            }
            match outcome {
                Some(Ok(g)) => {
                    self.message(
                        Level::Info,
                        format!(
                            "Generated {} placement(s) on {} page(s)",
                            g.placements.len(),
                            g.pages.len()
                        ),
                    );
                    if let Some(first) = g.pages.first() {
                        self.show_page(*first);
                    }
                    return;
                }
                Some(Err(problems)) => win.result = Some(Err(problems)),
                None => {}
            }
        }
        if open {
            self.wb.generate = Some(win);
        }
    }

    fn pick_spec(&mut self, win: &mut GenerateWindow) {
        let Some(path) = crate::dialogs::FileDialog::new()
            .add_filter("Spec sheet", &["xlsx", "csv"])
            .pick_file()
        else {
            return;
        };
        self.load_spec(win, path);
    }

    pub(super) fn load_spec(&mut self, win: &mut GenerateWindow, path: PathBuf) {
        win.error = None;
        win.result_for = None;
        let sheet = match path.extension().and_then(|e| e.to_str()) {
            Some(e) if e.eq_ignore_ascii_case("xlsx") => std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|b| read_grid(&b).map_err(|e| e.to_string()))
                .map(|mut rows| SpecSheet {
                    headers: rows.remove(0),
                    rows,
                }),
            _ => std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .map(|t| SpecSheet::from_csv(&t)),
        };
        match sheet {
            Ok(sheet) => {
                let (macros, missing) = load_macros(&macros_used(&sheet));
                win.macros = macros;
                win.missing = missing;
                win.sheet = Some(sheet);
                win.path = Some(path);
            }
            Err(e) => {
                win.error = Some(format!("Could not read {}: {e}", path.display()));
                win.sheet = None;
                win.result = None;
            }
        }
    }
}

/// The macros the sheet names, from the user's library and the starters
/// that ship with the app, keyed by lowercase name; and the names not found.
pub(crate) fn load_macros(
    names: &[String],
) -> (BTreeMap<String, novo_core::BlockDefinition>, Vec<String>) {
    use novo_parts::{MacroLibrary, starter_macros};
    let mut found: BTreeMap<String, novo_core::BlockDefinition> = BTreeMap::new();
    // Unit tests read only the shipped starters, so a macro in the developer's
    // own library cannot change what they expect.
    let user = if cfg!(test) {
        None
    } else {
        MacroLibrary::user()
    };
    if let Some(lib) = user {
        let (entries, _) = lib.list();
        for entry in entries {
            let key = entry.name.to_lowercase();
            if names.iter().any(|n| n.eq_ignore_ascii_case(&entry.name))
                && let Ok(def) = lib.definition_for_project(&entry.path)
            {
                found.insert(key, def);
            }
        }
    }
    for (_, file) in starter_macros() {
        let key = file.name.to_lowercase();
        if names.iter().any(|n| n.eq_ignore_ascii_case(&file.name)) {
            let mut def = file.definition;
            novo_workflow::parametric::enrich_starter(&mut def);
            found.entry(key).or_insert(def);
        }
    }
    let missing = names
        .iter()
        .filter(|n| !found.contains_key(&n.to_lowercase()))
        .cloned()
        .collect();
    (found, missing)
}

fn option_editor(ui: &mut egui::Ui, def: &OptionDef, value: &mut Value) -> bool {
    let before = value.clone();
    match def.spec.kind {
        ParamKind::Bool => {
            let mut on = matches!(value, Value::Bool(true));
            ui.checkbox(&mut on, "");
            *value = Value::Bool(on);
        }
        ParamKind::Choice => {
            let shown = value_text(value);
            egui::ComboBox::from_id_salt(("option", &def.id))
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    for c in &def.spec.choices {
                        if ui.selectable_label(value_text(value) == *c, c).clicked() {
                            *value = def.check(&Value::Text(c.clone())).unwrap_or(before.clone());
                        }
                    }
                });
        }
        ParamKind::Number => {
            let mut n = match value {
                Value::Int(i) => *i as f64,
                Value::Float(f) => *f,
                _ => 0.0,
            };
            let mut drag = egui::DragValue::new(&mut n);
            if let (Some(min), Some(max)) = (def.spec.min, def.spec.max) {
                drag = drag.range(min..=max);
            }
            ui.add(drag);
            *value = novo_workflow::expr::Val::Num(n).to_value();
        }
        _ => {
            let mut text = value_text(value);
            ui.add(egui::TextEdit::singleline(&mut text).desired_width(140.0));
            *value = Value::Text(text);
        }
    }
    *value != before
}

fn add_option_form(
    ui: &mut egui::Ui,
    form: &mut NewOption,
    define: &mut Option<OptionDef>,
    error: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(
            egui::TextEdit::singleline(&mut form.label)
                .hint_text("Conveyor")
                .desired_width(140.0),
        );
        ui.label("Kind");
        egui::ComboBox::from_id_salt("new_option_kind")
            .selected_text(match form.kind {
                NewKind::YesNo => "Yes or no",
                NewKind::Choice => "One of a list",
                NewKind::Number => "Number",
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut form.kind, NewKind::YesNo, "Yes or no");
                ui.selectable_value(&mut form.kind, NewKind::Choice, "One of a list");
                ui.selectable_value(&mut form.kind, NewKind::Number, "Number");
            });
    });
    match form.kind {
        NewKind::Choice => {
            ui.horizontal(|ui| {
                ui.label("Choices");
                ui.add(
                    egui::TextEdit::singleline(&mut form.choices)
                        .hint_text("400V, 480V")
                        .desired_width(220.0),
                );
            });
        }
        NewKind::Number => {
            ui.horizontal(|ui| {
                ui.label("Min");
                ui.add(egui::DragValue::new(&mut form.min));
                ui.label("Max");
                ui.add(egui::DragValue::new(&mut form.max));
                ui.label("Default");
                ui.add(egui::DragValue::new(&mut form.default_number));
            });
        }
        NewKind::YesNo => {}
    }
    let id = if form.id.trim().is_empty() {
        slug(&form.label)
    } else {
        form.id.trim().to_owned()
    };
    ui.horizontal(|ui| {
        ui.label("Used in conditions as");
        ui.add(
            egui::TextEdit::singleline(&mut form.id)
                .hint_text(slug(&form.label))
                .desired_width(120.0),
        );
        if ui
            .add_enabled(!id.is_empty(), egui::Button::new("Add option"))
            .clicked()
        {
            let label = form.label.trim();
            let def = match form.kind {
                NewKind::YesNo => OptionDef::yes_no(&id, label, false),
                NewKind::Choice => {
                    let choices: Vec<String> = form
                        .choices
                        .split(',')
                        .map(|c| c.trim().to_owned())
                        .filter(|c| !c.is_empty())
                        .collect();
                    if choices.len() < 2 {
                        *error = Some("A list option needs at least two choices".into());
                        return;
                    }
                    let refs: Vec<&str> = choices.iter().map(String::as_str).collect();
                    OptionDef::choice(&id, label, &refs, refs[0])
                }
                NewKind::Number => {
                    if form.min > form.max || !(form.min..=form.max).contains(&form.default_number)
                    {
                        *error = Some("Min, max and the default do not fit together".into());
                        return;
                    }
                    OptionDef::number(&id, label, form.min, form.max, form.default_number)
                }
            };
            *error = None;
            *define = Some(def);
        }
    });
}

/// `Export 480 V` becomes `export_480_v`.
fn slug(label: &str) -> String {
    let mut out = String::new();
    for c in label.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_owned();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("o_{out}")
    } else {
        out
    }
}

fn configuration_section(ui: &mut egui::Ui, shown: &Shown, compare: &str) {
    let c = &shown.configuration;
    let pages_on = c.pages.iter().filter(|p| p.active).count();
    novo_theme::widgets::section_label(ui, "What the current choice includes");
    ui.label(format!(
        "{} of {} pages · {} devices ({} left out) · {} order numbers, {} parts",
        pages_on,
        c.pages.len(),
        c.devices.len(),
        c.devices_off.len(),
        c.bom.len(),
        c.bom.iter().map(|l| l.quantity).sum::<u32>(),
    ));
    if !c.unassigned.is_empty() {
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Warning,
            &format!(
                "{} without a part: {}",
                c.unassigned.len(),
                c.unassigned.join(", ")
            ),
            |_| {},
        );
    }
    for (_, error) in &c.errors {
        novo_theme::widgets::banner(
            ui,
            novo_theme::widgets::Level::Warning,
            &format!("Condition: {error}"),
            |_| {},
        );
    }
    if let Some(h) = shown.heat {
        let mut text = format!(
            "Heat inside the cabinets: {:.0} W of device losses",
            h.watts
        );
        if let Some(other) = shown.heat_other {
            text.push_str(&format!(
                " ({:+.0} W against {compare})",
                h.watts - other.watts
            ));
        }
        ui.label(text).on_hover_text(
            "The sum of the loss figures of the parts in the cabinets, as the Cabinet sizing \
             reads them. It is what the devices put in, not a temperature: the Heat view \
             works that out.",
        );
        if h.without_figure > 0 {
            ui.weak(format!(
                "{} cabinet devices have no loss figure and count as 0 W",
                h.without_figure
            ));
        }
    }
    ui.collapsing("Pages", |ui| {
        for p in &c.pages {
            let mark = if p.active { "●" } else { "○" };
            let text = format!("{mark} {} {}", p.name, p.title);
            if p.active {
                ui.label(text);
            } else {
                ui.weak(text);
            }
        }
    });
    if let Some(d) = &shown.difference {
        ui.add_space(4.0);
        ui.strong(format!("Compared with {compare}"));
        if d.is_empty() {
            ui.weak("No difference in pages, devices or parts");
        }
        for (key, items, sign) in [
            ("Pages added", &d.pages_added, "+"),
            ("Pages removed", &d.pages_removed, "−"),
            ("Devices added", &d.devices_added, "+"),
            ("Devices removed", &d.devices_removed, "−"),
        ] {
            if !items.is_empty() {
                ui.label(format!("{key}: {sign} {}", items.join(", ")));
            }
        }
        for ((maker, order), before, after) in &d.bom_changes {
            ui.label(format!("{maker} {order}: {before} → {after}"));
        }
    }
}

/// The answers an object can be tied to: both answers of a yes/no option
/// and every choice; a number is typed as a condition.
fn tie_choices(def: &OptionDef) -> Vec<(String, Value)> {
    match def.spec.kind {
        ParamKind::Bool => vec![
            (def.label().to_owned(), Value::Bool(true)),
            (format!("no {}", def.label()), Value::Bool(false)),
        ],
        ParamKind::Choice => def
            .spec
            .choices
            .iter()
            .map(|c| (format!("{} {c}", def.label()), Value::Text(c.clone())))
            .collect(),
        _ => Vec::new(),
    }
}

/// The rule of one page or device as lines to pick: option, is / is not,
/// value. Returns the new condition when a line was changed (`Some(None)`
/// removes it). A condition typed by hand that the lines cannot say stays as
/// text under Advanced, where it can also be edited.
fn condition_editor(
    ui: &mut egui::Ui,
    key: &str,
    defs: &[OptionDef],
    current: Option<String>,
    raw: &mut ConditionText,
) -> Option<Option<String>> {
    let mut result = None;
    let lines = match current.as_deref() {
        None => Some(Vec::new()),
        Some(c) if c.trim().is_empty() => Some(Vec::new()),
        Some(c) => clauses_of(defs, c),
    };
    match lines {
        None => {
            ui.horizontal_wrapped(|ui| {
                ui.weak(format!(
                    "Written as an expression: {}.",
                    current.as_deref().unwrap_or_default()
                ));
                if ui
                    .small_button("Use the picker instead")
                    .on_hover_text("Remove this rule, then pick it line by line")
                    .clicked()
                {
                    result = Some(None);
                }
            });
        }
        Some(_) if defs.is_empty() => {
            ui.weak("Add an option above first, then a rule can use it.");
        }
        Some(mut lines) => {
            let mut changed = false;
            let mut remove = None;
            if lines.is_empty() {
                ui.weak("Always appears. No rule yet.");
            }
            for (i, line) in lines.iter_mut().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    let def = defs
                        .iter()
                        .find(|d| d.id == line.option)
                        .unwrap_or(&defs[0]);
                    let mut chosen = def.id.clone();
                    egui::ComboBox::from_id_salt((key, i, "option"))
                        .selected_text(def.label())
                        .show_ui(ui, |ui| {
                            for d in defs {
                                ui.selectable_value(&mut chosen, d.id.clone(), d.label());
                            }
                        });
                    let mut def = def;
                    if let Some(d) = defs.iter().find(|d| d.id == chosen && d.id != def.id) {
                        def = d;
                        *line = Clause::first_for(d);
                        changed = true;
                    }
                    let tests = tests_for(def);
                    if tests.len() > 1 {
                        egui::ComboBox::from_id_salt((key, i, "test"))
                            .selected_text(line.test.words())
                            .show_ui(ui, |ui| {
                                for t in tests {
                                    changed |=
                                        ui.selectable_value(&mut line.test, t, t.words()).changed();
                                }
                            });
                    } else {
                        ui.label(line.test.words());
                    }
                    match def.spec.kind {
                        ParamKind::Bool => {
                            let mut yes = line.value == Value::Bool(true);
                            egui::ComboBox::from_id_salt((key, i, "value"))
                                .selected_text(if yes { "yes" } else { "no" })
                                .show_ui(ui, |ui| {
                                    changed |= ui.selectable_value(&mut yes, true, "yes").changed();
                                    changed |= ui.selectable_value(&mut yes, false, "no").changed();
                                });
                            line.value = Value::Bool(yes);
                        }
                        ParamKind::Choice => {
                            let mut text = match &line.value {
                                Value::Text(t) => t.clone(),
                                _ => String::new(),
                            };
                            egui::ComboBox::from_id_salt((key, i, "value"))
                                .selected_text(text.clone())
                                .show_ui(ui, |ui| {
                                    for c in &def.spec.choices {
                                        changed |=
                                            ui.selectable_value(&mut text, c.clone(), c).changed();
                                    }
                                });
                            line.value = Value::Text(text);
                        }
                        _ => {
                            let mut n = match &line.value {
                                Value::Int(i) => *i as f64,
                                Value::Float(f) => *f,
                                _ => 0.0,
                            };
                            let mut drag = egui::DragValue::new(&mut n).speed(0.1);
                            if let (Some(lo), Some(hi)) = (def.spec.min, def.spec.max) {
                                drag = drag.range(lo..=hi);
                            }
                            if ui.add(drag).changed() {
                                line.value = novo_workflow::expr::Val::Num(n).to_value();
                                changed = true;
                            }
                        }
                    }
                    if ui
                        .small_button(novo_theme::icons::X)
                        .on_hover_text("Remove this line")
                        .clicked()
                    {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                lines.remove(i);
                changed = true;
            }
            if ui
                .small_button("+ Add a rule")
                .on_hover_text("All the lines must hold for it to appear")
                .clicked()
            {
                let used: Vec<&str> = lines.iter().map(|l| l.option.as_str()).collect();
                let def = defs
                    .iter()
                    .find(|d| !used.contains(&d.id.as_str()))
                    .unwrap_or(&defs[0]);
                lines.push(Clause::first_for(def));
                changed = true;
            }
            if let (true, Ok(condition)) = (changed, condition_from_clauses(defs, &lines)) {
                result = Some(condition);
            }
        }
    }

    let sync = (key.to_owned(), current.clone());
    if raw.synced.as_ref() != Some(&sync) {
        raw.text = current.unwrap_or_default();
        raw.synced = Some(sync);
    }
    egui::CollapsingHeader::new("Advanced: type the expression")
        .id_salt((key, "advanced"))
        .show(ui, |ui| {
            ui.weak(
                "For example opt.conveyor || opt.compact, or opt.supply == '480V' && !opt.compact.",
            );
            ui.horizontal(|ui| {
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut raw.text)
                        .hint_text("always")
                        .desired_width(260.0),
                );
                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                if ui.button("Set").clicked() || enter {
                    let c = raw.text.trim();
                    result = Some((!c.is_empty()).then(|| c.to_owned()));
                }
            });
        });
    result
}

fn finding_text(project: &Project, f: &Finding) -> String {
    let name = |id: AnyId| match id {
        AnyId::Page(p) => project
            .page(p)
            .map(|p| format!("page {}", p.name))
            .unwrap_or_default(),
        AnyId::Device(d) => project
            .device(d)
            .map(|d| d.tag.to_string())
            .unwrap_or_default(),
        other => format!("{other:?}"),
    };
    match f {
        Finding::BadCondition { object, error } => {
            format!(
                "Condition on {} cannot be evaluated: {error}",
                name(*object)
            )
        }
        Finding::UnusedOption(id) => format!("Option '{id}' is not used by any condition or macro"),
        Finding::UnknownInVariant { variant, option } => {
            format!("Variant '{variant}' sets '{option}', which is not an option")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_become_ids() {
        assert_eq!(slug("Conveyor"), "conveyor");
        assert_eq!(slug("Export 480 V"), "export_480_v");
        assert_eq!(slug("480V supply"), "o_480v_supply");
        assert_eq!(slug("  "), "");
    }

    #[test]
    fn starter_macros_are_found_without_a_library_file() {
        let (found, missing) = load_macros(&["Motor starter".to_string(), "nope".to_string()]);
        assert!(missing.contains(&"nope".to_string()));
        // Starters ship in the app, so a sheet works on a fresh install.
        assert!(found.len() <= 1);
    }
}
