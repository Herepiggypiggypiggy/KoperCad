//! File > Import > Controller profile from I/O table (lane C4): a wizard that turns a standard
//! I/O page into a controller profile and a binding table (`plans/controller-live-view.md`,
//! section 3). Four ways in, all ending in the same editable table: paste from Excel, the
//! pins of the selected device in the open drawing, a table or EPLAN page from a file, and the
//! generic library. The preview shows what would be created and a warning for every row the
//! guess is unsure about; nothing is written until Save.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_controller::import::{
    self, DevicePin, Options, Outcome, Proposal, Row, Severity, Table, WordList, library,
    rows_to_text,
};
use novo_core::{Endpoint, PinRef, Project};
use novo_interop::eplan::pxf::{PxfFile, read_pxf};

use super::inspector::selected_device;
use super::{Level, NovoApp};

/// Header that tells a file the wizard wrote itself.
const OURS: &str = "# Made by the KoperCAD controller import wizard";

/// The wizard window's state.
pub(super) struct ControllerWizard {
    text: String,
    name: String,
    id: String,
    id_edited: bool,
    step2_includes_step1: bool,
    gate_on_main: bool,
    words: WordList,
    /// Problems in the person's own word list, shown once.
    word_problems: Vec<String>,
    proposal: Proposal,
    /// Where the table came from, for the heading of the preview.
    source: String,
    /// An EPLAN export that was read, with the page to take rows from.
    pxf: Option<(String, PxfFile, usize)>,
    saved: Option<String>,
    /// The table box takes the keyboard when the wizard opens, so a paste goes straight in.
    focus_table: bool,
    /// Set while the table is a generic library one: its kind and flags are applied to the
    /// proposal. Editing the table by hand clears it.
    library: Option<&'static str>,
}

/// The folder generic and personal controller profiles live in.
pub(crate) fn controllers_dir() -> Option<PathBuf> {
    novo_storage::profile::config_dir().map(|d| d.join("KoperCAD").join("controllers"))
}

fn user_words() -> (WordList, Vec<String>) {
    let mut words = WordList::builtin();
    let mut problems = Vec::new();
    if let Some(path) = controllers_dir().map(|d| d.join("words.txt"))
        && let Ok(text) = std::fs::read_to_string(&path)
    {
        problems = words
            .extend(&text)
            .into_iter()
            .map(|p| format!("{}: {p}", path.display()))
            .collect();
    }
    (words, problems)
}

impl ControllerWizard {
    fn new() -> ControllerWizard {
        let (words, word_problems) = user_words();
        let options = Options::default();
        let mut w = ControllerWizard {
            text: String::new(),
            name: options.name.clone(),
            id: options.id.clone(),
            id_edited: false,
            step2_includes_step1: true,
            gate_on_main: true,
            words,
            word_problems,
            proposal: import::propose(&Table::default(), &WordList::default(), &options),
            source: "Pasted table".to_owned(),
            pxf: None,
            saved: None,
            focus_table: true,
            library: None,
        };
        w.refresh();
        w
    }

    fn refresh(&mut self) {
        let table = Table::from_text(&self.text);
        let options = Options {
            id: import::slug(&self.id),
            name: self.name.clone(),
            step2_includes_step1: self.step2_includes_step1,
            gate_on_main: self.gate_on_main,
        };
        self.proposal = import::propose(&table, &self.words, &options);
        if let Some(id) = self.library {
            library::finish(id, &mut self.proposal);
        }
        self.saved = None;
    }

    fn set_text(&mut self, text: String, source: impl Into<String>) {
        self.library = None;
        self.text = text;
        self.source = source.into();
        self.refresh();
    }
}

/// The pins of `device` with what each is wired to: the label of the far end, and the far
/// device's description as the function text when the pin has none of its own.
pub(super) fn device_pins(project: &Project, device: novo_core::DeviceId) -> Vec<DevicePin> {
    let mut out = Vec::new();
    for symbol in project.symbols_of_device(device) {
        for (i, pin) in symbol.pins.iter().enumerate() {
            let Ok(index) = u16::try_from(i) else {
                continue;
            };
            let here = Endpoint::pin(symbol.id, index);
            let mut target = String::new();
            let mut text = String::new();
            for wire in project.wires_at(here) {
                let Some(wire) = project.wire(wire) else {
                    continue;
                };
                let far = if wire.from == here {
                    wire.to
                } else {
                    wire.from
                };
                if let Endpoint::Pin { symbol: s, pin: p } = far {
                    if let Some(d) = project.pin_designation(PinRef::new(s, p)) {
                        target = d;
                    }
                    if let Some(far_device) = project.symbol(s).and_then(|s| s.device())
                        && let Some(d) = project.device(far_device)
                    {
                        text = d.description.clone();
                    }
                }
                break;
            }
            out.push(DevicePin {
                name: pin.name.clone(),
                text,
                target,
            });
        }
    }
    out
}

/// Rows of one page of an EPLAN export: every terminal text with the nearest free text on the
/// same line as its function. A guess, shown in the table for the person to correct.
pub(crate) fn pxf_page_rows(file: &PxfFile, page: usize, lang: &str) -> Vec<Row> {
    use novo_interop::eplan::pxf::DeviceRole;
    let mut rows = Vec::new();
    for d in &file.devices {
        if d.page != Some(page) || d.role != DeviceRole::Terminal || d.terminal_text.is_empty() {
            continue;
        }
        let function = d.position.and_then(|(x, y)| {
            file.texts
                .iter()
                .filter(|t| t.page == Some(page))
                .filter_map(|t| {
                    let (tx, ty) = t.position?;
                    ((ty - y).abs() <= 3.0 && !t.text.get(lang).trim().is_empty())
                        .then(|| ((tx - x).abs(), t.text.get(lang).trim().to_owned()))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, text)| text)
        });
        rows.push(Row {
            line: rows.len() + 1,
            terminal: d.terminal_text.clone(),
            function: function.unwrap_or_default(),
            ..Row::default()
        });
    }
    rows
}

fn read_table_file(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    // Spreadsheets saved as CSV from Excel are often Windows-1252, not UTF-8.
    Ok(match String::from_utf8(bytes) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_owned(),
        Err(e) => e.into_bytes().iter().map(|&b| char::from(b)).collect(),
    })
}

/// `dir/<id>.controller.toml`, or the first `<id>-2`... when that name holds a file the wizard
/// did not write.
fn target_path(dir: &Path, id: &str) -> PathBuf {
    let mut n = 1;
    loop {
        let stem = if n == 1 {
            id.to_owned()
        } else {
            format!("{id}-{n}")
        };
        let path = dir.join(format!("{stem}.controller.toml"));
        let ours = std::fs::read_to_string(&path).is_ok_and(|t| t.contains(OURS));
        if !path.exists() || ours {
            return path;
        }
        n += 1;
    }
}

impl NovoApp {
    /// Opens the wizard, empty.
    pub(super) fn open_controller_import(&mut self) {
        self.dismiss_toasts();
        self.wb.controller_wizard = Some(Box::new(ControllerWizard::new()));
    }

    /// Opens the wizard with `text` already in the table (scripts and tests).
    pub fn open_controller_import_with(&mut self, text: &str) {
        let mut w = ControllerWizard::new();
        w.set_text(text.to_owned(), "Pasted table");
        self.wb.controller_wizard = Some(Box::new(w));
    }

    fn controller_from_selection(&mut self) -> Option<(String, String)> {
        let selection = self.selection();
        let doc = self.backend.document()?;
        let project = doc.project();
        let device = selected_device(project, &selection)?;
        let pins = device_pins(project, device);
        let tag = project.device(device)?.tag.to_string();
        Some((tag, rows_to_text(&Table::from_pins(&pins).rows)))
    }

    /// Writes the profile and its binding table; returns the profile's path.
    fn save_controller_profile(&mut self) -> Result<PathBuf, String> {
        let Some(w) = &mut self.wb.controller_wizard else {
            return Err("The wizard is closed".to_owned());
        };
        let dir = controllers_dir().ok_or("No folder for controller profiles")?;
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let path = target_path(&dir, &import::slug(&w.id));
        let toml = w.proposal.profile.to_toml().replacen(
            "# Made by the KoperCAD controller import wizard from an I/O table.",
            &format!("{OURS} from an I/O table."),
            1,
        );
        std::fs::write(&path, toml).map_err(|e| format!("{}: {e}", path.display()))?;
        let csv = path.with_extension("").with_extension("bindings.csv");
        std::fs::write(&csv, w.proposal.bindings_csv())
            .map_err(|e| format!("{}: {e}", csv.display()))?;
        w.saved = Some(path.display().to_string());
        Ok(path)
    }

    pub(super) fn controller_import_dialog(&mut self, ctx: &egui::Context) {
        if self.wb.controller_wizard.is_none() {
            return;
        }
        let selection_ready = self
            .backend
            .document()
            .is_some_and(|d| selected_device(d.project(), &self.selection()).is_some());
        let palette = crate::theme::ui_palette();
        // Small windows (1366x768) keep Save, Copy and Close on screen: the boxes shrink.
        let screen_h = ctx.content_rect().height();
        let compact = screen_h < 900.0;
        let preview_height = (screen_h - 560.0).clamp(90.0, 230.0);
        let mut action = Action::None;
        let w = self.wb.controller_wizard.as_mut().expect("checked above");
        let mut changed = false;
        let modal = super::panel::modal(
            ctx,
            "controller_import",
            "Controller profile from an I/O table",
            [820.0, 640.0],
            |ui| {
                ui.weak(
                    "Paste the terminal table of a receiver or pendant page (terminal, function, \
                 step; Excel columns work). Dutch and English words are understood. Nothing is \
                 saved until you press Save.",
                );
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                ui.add_enabled_ui(selection_ready, |ui| {
                    if ui
                        .button("From the selected device")
                        .on_hover_text("Reads the pins of the device selected in the drawing and what they are wired to")
                        .clicked()
                    {
                        action = Action::FromDevice;
                    }
                });
                if ui.button("Open table or EPLAN file...").clicked() {
                    action = Action::OpenFile;
                }
                egui::ComboBox::from_id_salt("controller_library")
                    .selected_text("Generic library")
                    .show_ui(ui, |ui| {
                        for (id, name) in library::list() {
                            if ui.selectable_label(false, name).clicked() {
                                action = Action::Library(id);
                            }
                        }
                    });
            });
                if let Some((file, data, page)) = &mut w.pxf {
                    ui.horizontal(|ui| {
                        ui.label(format!("EPLAN page of {file}"));
                        let label = |i: usize| {
                            let p = &data.pages[i];
                            format!("{} {}{}", p.function, p.number, p.suffix)
                                .trim()
                                .to_owned()
                        };
                        egui::ComboBox::from_id_salt("controller_pxf_page")
                            .selected_text(label(*page))
                            .show_ui(ui, |ui| {
                                for i in 0..data.pages.len() {
                                    ui.selectable_value(page, i, label(i));
                                }
                            });
                        if ui.button("Use this page").clicked() {
                            action = Action::UsePage;
                        }
                    });
                }
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .id_salt("controller_text")
                    .max_height(if compact { 90.0 } else { 150.0 })
                    .show(ui, |ui| {
                        let table_box = ui.add(
                            egui::TextEdit::multiline(&mut w.text)
                                .font(egui::TextStyle::Monospace)
                                .desired_rows(if compact { 4 } else { 7 })
                                .desired_width(f32::INFINITY)
                                .hint_text(
                                    "Terminal\tFunction\tStep\nCOM\tCommon\t\n1\tHoist up\t1",
                                ),
                        );
                        if std::mem::take(&mut w.focus_table) {
                            table_box.request_focus();
                        }
                        if table_box.changed() {
                            w.library = None;
                            changed = true;
                        }
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let name = ui.add(egui::TextEdit::singleline(&mut w.name).desired_width(240.0));
                    if name.changed() {
                        if !w.id_edited {
                            w.id = import::slug(&w.name);
                        }
                        changed = true;
                    }
                    ui.label("File name");
                    if ui
                        .add(egui::TextEdit::singleline(&mut w.id).desired_width(180.0))
                        .changed()
                    {
                        w.id_edited = true;
                        changed = true;
                    }
                });
                changed |= ui
                    .checkbox(
                        &mut w.step2_includes_step1,
                        "Step 2 keeps the step 1 contact closed (usual for two-step switches)",
                    )
                    .changed();
                changed |= ui
                    .checkbox(
                        &mut w.gate_on_main,
                        "Motions only work after main/start is on (when the table has a main row)",
                    )
                    .changed();
                ui.separator();
                preview(ui, w, palette, preview_height);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    let can_save = !w.proposal.profile.controls.is_empty();
                    if ui
                        .add_enabled(can_save, egui::Button::new("Save profile"))
                        .on_hover_text(
                            "Writes the profile and its binding table to your controller folder",
                        )
                        .clicked()
                    {
                        action = Action::Save;
                    }
                    if ui
                        .add_enabled(can_save, egui::Button::new("Copy profile text"))
                        .clicked()
                    {
                        action = Action::Copy;
                    }
                    if ui.button("Close").clicked() {
                        action = Action::Close;
                    }
                    if let Some(saved) = &w.saved {
                        ui.weak(format!("Saved {saved}"));
                    }
                });
            },
        );
        if changed {
            w.refresh();
        }
        if modal.should_close() {
            action = Action::Close;
        }
        match action {
            Action::None => {}
            Action::Close => self.wb.controller_wizard = None,
            Action::FromDevice => match self.controller_from_selection() {
                Some((tag, text)) => {
                    if let Some(w) = &mut self.wb.controller_wizard {
                        w.pxf = None;
                        w.set_text(text, format!("Pins of {tag}"));
                    }
                }
                None => self.message(Level::Info, "Select one device in the drawing first"),
            },
            Action::OpenFile => self.controller_open_file(),
            Action::UsePage => {
                if let Some(w) = &mut self.wb.controller_wizard
                    && let Some((file, data, page)) = &w.pxf
                {
                    let lang = data
                        .texts
                        .iter()
                        .find_map(|t| t.text.entries.first().map(|(l, _)| l.clone()))
                        .unwrap_or_default();
                    let rows = pxf_page_rows(data, *page, &lang);
                    let source = format!("EPLAN page of {file}");
                    let text = rows_to_text(&rows);
                    w.set_text(text, source);
                }
            }
            Action::Library(id) => {
                if let Some(w) = &mut self.wb.controller_wizard {
                    w.pxf = None;
                    w.name = library::list()
                        .iter()
                        .find(|(i, _)| *i == id)
                        .map_or(String::new(), |(_, n)| (*n).to_owned());
                    w.id = id.to_owned();
                    w.id_edited = true;
                    w.set_text(
                        library::table(id).unwrap_or_default().to_owned(),
                        "Generic library",
                    );
                    w.library = Some(id);
                    w.refresh();
                }
            }
            Action::Save => match self.save_controller_profile() {
                Ok(path) => self.message_file("Controller profile saved", &path),
                Err(e) => self.message(Level::Error, format!("Could not save the profile: {e}")),
            },
            Action::Copy => {
                if let Some(w) = &self.wb.controller_wizard {
                    ctx.copy_text(w.proposal.profile.to_toml());
                    self.message(Level::Info, "Profile text copied");
                }
            }
        }
    }

    fn controller_open_file(&mut self) {
        let dialog = crate::dialogs::FileDialog::new()
            .set_title("Open an I/O table or an EPLAN export")
            .add_filter(
                "Table or EPLAN export",
                &["csv", "tsv", "txt", "epj", "xml"],
            );
        let Some(path) = dialog.pick_file() else {
            return;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if matches!(ext.as_str(), "epj" | "xml") {
            let read = std::fs::File::open(&path)
                .map_err(|e| e.to_string())
                .and_then(|f| {
                    read_pxf(std::io::BufReader::with_capacity(1 << 20, f))
                        .map_err(|e| e.to_string())
                });
            match read {
                Ok(file) if file.pages.is_empty() => {
                    self.message(Level::Info, format!("{name} has no pages"));
                }
                Ok(file) => {
                    if let Some(w) = &mut self.wb.controller_wizard {
                        w.pxf = Some((name, file, 0));
                    }
                }
                Err(e) => self.message(Level::Error, format!("Could not read {name}: {e}")),
            }
        } else {
            match read_table_file(&path) {
                Ok(text) => {
                    if let Some(w) = &mut self.wb.controller_wizard {
                        w.pxf = None;
                        w.set_text(text, name);
                    }
                }
                Err(e) => self.message(Level::Error, format!("Could not read {name}: {e}")),
            }
        }
    }
}

enum Action {
    None,
    Close,
    FromDevice,
    OpenFile,
    UsePage,
    Library(&'static str),
    Save,
    Copy,
}

/// `1 control`, `2 controls`.
fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn preview(ui: &mut egui::Ui, w: &ControllerWizard, palette: &novo_theme::Palette, height: f32) {
    let p = &w.proposal;
    ui.strong(format!("Preview: {}", w.source));
    for n in &p.notes {
        ui.weak(n);
    }
    for problem in &w.word_problems {
        ui.colored_label(palette.warning, problem);
    }
    ui.add_space(4.0);
    let profile = &p.profile;
    if profile.controls.is_empty() {
        ui.label("Nothing would be created yet.");
    } else {
        ui.label(format!(
            "Would create the {} controller \u{201c}{}\u{201d} with {} and {}{}{}.",
            profile.kind,
            profile.name,
            plural(profile.controls.len(), "control"),
            plural(profile.signals.len(), "signal"),
            profile
                .common
                .as_ref()
                .map_or(String::new(), |c| format!(", common {c}")),
            profile.supply.map_or(String::new(), |s| format!(
                ", {} V {}",
                s.volts,
                if s.ac { "AC" } else { "DC" }
            )),
        ));
    }
    egui::ScrollArea::vertical()
        .id_salt("controller_preview")
        .max_height(height)
        .show(ui, |ui| {
            if !p.warnings.is_empty() {
                ui.strong(format!("{} to check", p.warnings.len()));
                for wn in &p.warnings {
                    let line = if wn.line == 0 {
                        String::new()
                    } else {
                        format!("line {}: ", wn.line)
                    };
                    let text = format!("{line}{}", wn.message);
                    match wn.severity {
                        Severity::Warning => ui.colored_label(palette.warning, text),
                        Severity::Info => ui.weak(text),
                    };
                }
            }
            ui.add_space(6.0);
            for c in &profile.controls {
                ui.label(format!(
                    "{}: {}, positions {}",
                    c.label,
                    c.kind,
                    c.position_text()
                ));
            }
            if !p.rows.is_empty() {
                ui.add_space(4.0);
                egui::Grid::new("controller_rows")
                    .num_columns(4)
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Terminal");
                        ui.strong("Text");
                        ui.strong("Becomes");
                        ui.strong("Wired to");
                        ui.end_row();
                        for r in &p.rows {
                            ui.label(&r.row.terminal);
                            ui.label(&r.row.function);
                            match &r.outcome {
                                Outcome::Signal(s) => ui.label(s),
                                Outcome::Common => ui.label("common pin"),
                                Outcome::Skipped(why) => {
                                    ui.colored_label(palette.warning, format!("left out: {why}"))
                                }
                            };
                            ui.label(&r.row.target);
                            ui.end_row();
                        }
                    });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_interop::eplan::pxf::read_pxf_str;

    const SMALL: &str =
        include_str!("../../../novo-interop/tests/fixtures/pxf/synthetic-small.epj");

    #[test]
    fn pxf_page_rows_pair_a_terminal_with_the_text_on_its_line() {
        let mut file = read_pxf_str(SMALL).unwrap();
        let terminal = file
            .devices
            .iter()
            .position(|d| d.terminal_text == "1")
            .expect("fixture has a terminal 1");
        let page = file.devices[terminal].page.unwrap();
        let (x, y) = file.devices[terminal].position.unwrap();
        file.texts[0].page = Some(page);
        file.texts[0].position = Some((x + 20.0, y + 1.0));
        let rows = pxf_page_rows(&file, page, "en_US");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].terminal, "1");
        assert_eq!(rows[0].function, "Sheet note");
        // Too far above the terminal: no function is guessed.
        file.texts[0].position = Some((x + 20.0, y + 30.0));
        assert_eq!(pxf_page_rows(&file, page, "en_US")[0].function, "");
    }

    #[test]
    fn device_pins_carry_the_wire_target_and_the_far_devices_description() {
        use novo_core::{DeviceTag, Pin, Placement, Point, SymbolRole};
        let mut project = Project::new("Plant");
        let ((rx, hoist_up), _) = project
            .transact("Add", |tx| {
                let page = tx.add_page("1")?;
                let rx = tx.add_device(DeviceTag::parse("-A1")?, "Receiver")?;
                let k = tx.add_device(DeviceTag::parse("-K61")?, "Hoist up slow")?;
                let a = tx.place_symbol(
                    page,
                    "iec/misc/box",
                    Placement::at(Point::new(10.0, 10.0)),
                    vec![Pin::new("COM", 0.0, 0.0), Pin::new("13", 0.0, 5.0)],
                    SymbolRole::Device {
                        device: rx,
                        main: true,
                    },
                )?;
                let c = tx.place_symbol(
                    page,
                    "iec/coils/coil",
                    Placement::at(Point::new(60.0, 10.0)),
                    vec![Pin::new("A1", 0.0, 0.0)],
                    SymbolRole::Device {
                        device: k,
                        main: true,
                    },
                )?;
                tx.connect(Endpoint::pin(a, 1), Endpoint::pin(c, 0))?;
                Ok((rx, k))
            })
            .unwrap();
        let _ = hoist_up;
        let pins = device_pins(&project, rx);
        assert_eq!(pins.len(), 2);
        assert_eq!(pins[0].name, "COM");
        assert_eq!(pins[0].target, "");
        assert_eq!(pins[1].name, "13");
        assert_eq!(pins[1].target, "-K61:A1");
        assert_eq!(pins[1].text, "Hoist up slow");
        // The words of the far device name the motion, so the wizard reads the row.
        let text = rows_to_text(&Table::from_pins(&pins).rows);
        let p = import::propose(
            &Table::from_text(&text),
            &WordList::builtin(),
            &Options::default(),
        );
        assert!(
            p.bindings
                .iter()
                .any(|b| b.pin == "13" && b.target == "-K61:A1"),
            "{p:?}"
        );
    }

    #[test]
    fn ids_for_files_are_slugs() {
        assert_eq!(
            import::slug("Radio, 3 motions (2-step)"),
            "radio-3-motions-2-step"
        );
    }

    #[test]
    fn a_saved_name_is_not_taken_by_a_file_we_did_not_write() {
        let dir = std::env::temp_dir().join(format!("kc-ctrl-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let first = target_path(&dir, "radio");
        assert_eq!(first.file_name().unwrap(), "radio.controller.toml");
        std::fs::write(&first, "someone else's file").unwrap();
        assert_eq!(
            target_path(&dir, "radio").file_name().unwrap(),
            "radio-2.controller.toml"
        );
        std::fs::write(&first, format!("{OURS} from an I/O table.\n")).unwrap();
        assert_eq!(target_path(&dir, "radio"), first);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn windows_1252_csv_is_read() {
        let dir = std::env::temp_dir().join(format!("kc-ctrl-csv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.csv");
        std::fs::write(&path, b"klem;functie\n1;Hijsen omhoog caf\xe9\n").unwrap();
        assert!(read_table_file(&path).unwrap().contains("caf\u{e9}"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod plural_tests {
    #[test]
    fn counts_say_one_control_and_two_controls() {
        assert_eq!(super::plural(1, "control"), "1 control");
        assert_eq!(super::plural(2, "signal"), "2 signals");
        assert_eq!(super::plural(0, "signal"), "0 signals");
    }
}
