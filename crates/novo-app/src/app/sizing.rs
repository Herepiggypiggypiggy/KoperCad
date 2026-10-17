//! "Size the enclosure": a side panel in the Cabinet view that recommends a
//! plate and enclosure from the panel's devices, and the "Enclosures..."
//! window where the person keeps the list of sizes to choose from. The list
//! is the person's own: typed in or imported from CSV, never bundled from
//! makers, saved as `enclosures.csv` in the KoperCAD folder under
//! `%APPDATA%` (no admin rights). The numbers are planning values.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_layout::sizing::{
    EnclosureCatalog, EnclosureSize, Installation, Material, Note, Recommendation, SizingDevice,
    SizingRequest, recommend,
};

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const CATALOG_FILE: &str = "enclosures.csv";

/// `%APPDATA%\KoperCAD\enclosures.csv` (the platform's data folder elsewhere).
pub fn catalog_path() -> Option<PathBuf> {
    Some(
        novo_storage::profile::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(CATALOG_FILE),
    )
}

/// The list in the file, and one line per row that could not be read. A
/// missing or unreadable file is an empty list.
pub fn load_catalog(path: &Path) -> (EnclosureCatalog, Vec<String>) {
    let Ok(text) = std::fs::read_to_string(path) else {
        return (EnclosureCatalog::new(), Vec::new());
    };
    let (catalog, errors) = EnclosureCatalog::from_csv(&text);
    (catalog, errors.iter().map(ToString::to_string).collect())
}

/// Writes the list to `path`, creating the folder.
pub fn save_catalog(catalog: &EnclosureCatalog, path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, catalog.to_csv())
}

/// How the enclosure stands; the choices of the panel.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Standing {
    Floor,
    Wall,
    WallInRow,
}

impl Standing {
    const ALL: [Standing; 3] = [Standing::Floor, Standing::Wall, Standing::WallInRow];

    fn label(self) -> &'static str {
        match self {
            Standing::Floor => "Floor standing, free",
            Standing::Wall => "Wall mounted",
            Standing::WallInRow => "Wall mounted, in a row",
        }
    }

    fn installation(self) -> Installation {
        match self {
            Standing::Floor => Installation::FreeStanding,
            Standing::Wall => Installation::Wall,
            Standing::WallInRow => Installation::WallInRow,
        }
    }
}

/// What the panel and the editor remember.
pub(super) struct SizingState {
    pub(super) catalog: EnclosureCatalog,
    /// Where the list is saved; `None` when the platform has no data folder.
    path: Option<PathBuf>,
    loaded: bool,
    /// Percent of the plate kept free, percent taken by ducts.
    free_space: f64,
    duct: f64,
    standing: Standing,
    ambient: f64,
    inside: f64,
    /// The "Size the enclosure" window is open.
    pub(super) panel_open: bool,
    pub(super) editor_open: bool,
    /// The list has edits not yet written to its file.
    dirty: bool,
    /// One line for the editor: what the last import or save did.
    notice: Option<String>,
}

impl SizingState {
    pub(super) fn new() -> Self {
        let defaults = SizingRequest::default();
        Self {
            catalog: EnclosureCatalog::new(),
            path: catalog_path(),
            loaded: false,
            free_space: defaults.free_space * 100.0,
            duct: defaults.duct_allowance * 100.0,
            standing: Standing::Floor,
            ambient: defaults.ambient_max,
            inside: defaults.inside_max,
            panel_open: false,
            editor_open: false,
            dirty: false,
            notice: None,
        }
    }

    /// Reads the list file once, when the panel is first shown.
    pub(super) fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        if let Some(path) = &self.path {
            let (catalog, errors) = load_catalog(path);
            self.catalog = catalog;
            if !errors.is_empty() {
                self.notice = Some(format!(
                    "{} in {CATALOG_FILE} could not be read: {}",
                    count(errors.len(), "row"),
                    errors.join("; ")
                ));
            }
        }
    }

    fn save(&mut self) {
        if let Some(path) = &self.path
            && let Err(e) = save_catalog(&self.catalog, path)
        {
            self.notice = Some(format!("Could not save the list: {e}"));
        }
    }

    pub(super) fn request(&self) -> SizingRequest {
        SizingRequest {
            free_space: (self.free_space / 100.0).clamp(0.0, 0.9),
            duct_allowance: (self.duct / 100.0).clamp(0.0, 0.9),
            installation: self.standing.installation(),
            ambient_max: self.ambient,
            inside_max: self.inside,
            ..SizingRequest::default()
        }
    }

    /// Merges a CSV file into the list: rows with a name already in the
    /// list replace it. Returns the notice to show.
    pub(super) fn import(&mut self, text: &str) -> String {
        let (incoming, errors) = EnclosureCatalog::from_csv(text);
        let added = incoming.items.len();
        for item in incoming.items {
            self.catalog.upsert(item);
        }
        self.save();
        let mut notice = format!("Imported {}.", count(added, "enclosure"));
        if !errors.is_empty() {
            notice.push_str(&format!(
                " {} skipped: {}",
                count(errors.len(), "row"),
                errors
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        notice
    }
}

fn count(n: usize, noun: &str) -> String {
    if n == 1 {
        format!("1 {noun}")
    } else {
        format!("{n} {noun}s")
    }
}

/// The sentence for one note of a recommendation.
pub(super) fn note_text(note: &Note) -> String {
    match note {
        Note::NoCatalog => {
            "Your enclosure list is empty. Add sizes under Enclosures... to get a recommendation."
                .to_owned()
        }
        Note::GuessedSizes(n) => format!(
            "{} no size in their part, so a default footprint is used.",
            if *n == 1 {
                "1 device has".to_owned()
            } else {
                format!("{n} devices have")
            }
        ),
        Note::NoLossData => {
            "No device lists a power loss, so the heat result is zero by omission.".to_owned()
        }
        Note::NothingFits => "No enclosure in your list holds these devices.".to_owned(),
        Note::LargerCoolsNaturally(name) => {
            format!("{name} is the smallest one that also cools without help.")
        }
        Note::NoneCoolsNaturally => {
            "No enclosure in your list cools naturally at this loss.".to_owned()
        }
        Note::DeviceTooBig(_) => {
            "A device is larger than the plate of every enclosure in your list.".to_owned()
        }
        Note::TooDeep(_) => {
            "A device is deeper than every enclosure in your list allows.".to_owned()
        }
    }
}

/// The lines of the result block, without styling: what is recommended and
/// what the heat balance says.
pub(super) fn result_lines(rec: &Recommendation) -> Vec<String> {
    let mut lines = vec![format!(
        "Plate needed: {:.0} cm2 with gaps, ducts and free space",
        rec.required_plate_area / 100.0
    )];
    match (&rec.enclosure, &rec.thermal) {
        (Some(e), Some(t)) => {
            lines.push(format!(
                "Enclosure: {} ({} x {} x {} mm), plate {} x {} mm",
                e.name, e.width, e.height, e.depth, e.plate_width, e.plate_height
            ));
            if let Some(fill) = rec.fill {
                lines.push(format!("Plate fill: {:.0}%", fill * 100.0));
            }
            lines.push(format!("Loss: {:.0} W", t.loss));
            lines.push(format!(
                "Inside at the highest ambient: {:.0} C",
                t.inside_temperature
            ));
            if t.natural_ok {
                lines.push("Natural cooling is enough.".to_owned());
            } else {
                let mut line = format!("Needs about {:.0} W of cooling", t.cooling_needed);
                if let Some(flow) = t.fan_airflow {
                    line.push_str(&format!(", or a fan of about {flow:.0} m3/h"));
                }
                line.push('.');
                lines.push(line);
            }
        }
        _ => lines.push(format!("Loss: {:.0} W", rec.loss)),
    }
    lines
}

/// The floating "Size the enclosure" window; the right side of the Cabinet
/// view belongs to the inspector.
pub(super) fn show_window(ctx: &egui::Context, state: &mut SizingState, devices: &[SizingDevice]) {
    if !state.panel_open {
        return;
    }
    let mut open = true;
    let window = super::panel::dialog(ctx, "Size the enclosure", [380.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| show_panel(ui, state, devices));
    if let Some(w) = window {
        super::cabinet_bar::note_rect(
            ctx,
            "cabinet.sizing",
            "Size enclosure",
            w.response.rect,
            false,
        );
    }
    state.panel_open = open;
}

/// The content of the window. `devices` are the panel's devices from the project.
fn show_panel(ui: &mut egui::Ui, state: &mut SizingState, devices: &[SizingDevice]) {
    state.ensure_loaded();
    ui.weak("Planning values. The enclosure maker's own sizing wins.");
    ui.add_space(6.0);
    novo_theme::widgets::section_label(ui, "Inputs");
    egui::Grid::new("sizing_inputs")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            ui.label("Free space");
            ui.add(
                egui::DragValue::new(&mut state.free_space)
                    .range(0.0..=80.0)
                    .suffix(" %"),
            );
            ui.end_row();
            ui.label("Ducts");
            ui.add(
                egui::DragValue::new(&mut state.duct)
                    .range(0.0..=80.0)
                    .suffix(" %"),
            );
            ui.end_row();
            ui.label("Ambient max");
            ui.add(
                egui::DragValue::new(&mut state.ambient)
                    .range(-20.0..=70.0)
                    .suffix(" C"),
            );
            ui.end_row();
            ui.label("Inside max");
            ui.add(
                egui::DragValue::new(&mut state.inside)
                    .range(-20.0..=90.0)
                    .suffix(" C"),
            );
            ui.end_row();
        });
    egui::ComboBox::from_id_salt("sizing_standing")
        .selected_text(state.standing.label())
        .show_ui(ui, |ui| {
            for s in Standing::ALL {
                ui.selectable_value(&mut state.standing, s, s.label());
            }
        });
    ui.add_space(10.0);
    novo_theme::widgets::section_label(ui, "Result");

    if devices.is_empty() {
        ui.weak("This panel has no devices yet.");
    } else {
        let rec = recommend(devices, &state.catalog, &state.request());
        for line in result_lines(&rec) {
            ui.label(line);
        }
        if !rec.notes.is_empty() {
            ui.add_space(4.0);
            for note in &rec.notes {
                ui.weak(note_text(note));
            }
        }
    }
    ui.add_space(8.0);
    if novo_theme::widgets::ghost_button(ui, "Enclosures...").clicked() {
        state.editor_open = true;
    }
}

/// The "Enclosures..." window: the person's list of sizes.
pub(super) fn show_editor(ctx: &egui::Context, state: &mut SizingState) {
    if !state.editor_open {
        return;
    }
    state.ensure_loaded();
    let mut open = true;
    let mut changed = false;
    let mut remove: Option<usize> = None;
    let mut import = false;
    let mut export = false;
    super::panel::dialog(ctx, "Enclosures", [620.0, 0.0])
        .open(&mut open)
        .show(ctx, |ui| {
            ui.weak("Your own list of enclosure sizes, in mm. Outside size sets the cooling surface; the plate is the mounting plate inside.");
            ui.add_space(4.0);
            egui::Grid::new("enclosure_list")
                .num_columns(9)
                .striped(true)
                .show(ui, |ui| {
                    for head in ["Name", "W (mm)", "H (mm)", "D (mm)", "Plate W (mm)", "Plate H (mm)", "Margin (mm)", "Material", ""] {
                        ui.label(egui::RichText::new(head).size(12.0).weak());
                    }
                    ui.end_row();
                    for (i, e) in state.catalog.items.iter_mut().enumerate() {
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut e.name).desired_width(120.0))
                            .changed();
                        for value in [
                            &mut e.width,
                            &mut e.height,
                            &mut e.depth,
                            &mut e.plate_width,
                            &mut e.plate_height,
                            &mut e.margin,
                        ] {
                            changed |= ui
                                .add(egui::DragValue::new(value).range(0.0..=5000.0).speed(1.0))
                                .changed();
                        }
                        egui::ComboBox::from_id_salt(("enclosure_material", i))
                            .selected_text(e.material.name())
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for m in [
                                    Material::Steel,
                                    Material::Stainless,
                                    Material::Aluminium,
                                    Material::Plastic,
                                ] {
                                    changed |= ui
                                        .selectable_value(&mut e.material, m, m.name())
                                        .changed();
                                }
                            });
                        if novo_theme::widgets::ghost_button(ui, "Remove").clicked() {
                            remove = Some(i);
                        }
                        ui.end_row();
                    }
                });
            if state.catalog.items.is_empty() {
                ui.weak("No enclosures yet. Add one, or import a CSV file.");
            }
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if novo_theme::widgets::primary_button(ui, "Add").clicked() {
                    state.catalog.items.push(EnclosureSize {
                        name: format!("Enclosure {}", state.catalog.items.len() + 1),
                        width: 600.0,
                        height: 800.0,
                        depth: 300.0,
                        plate_width: 550.0,
                        plate_height: 750.0,
                        margin: 10.0,
                        material: Material::Steel,
                    });
                    changed = true;
                }
                import = ui.button("Import CSV...").clicked();
                export = ui.button("Export CSV...").clicked();
            });
            if let Some(notice) = &state.notice {
                ui.add_space(4.0);
                ui.weak(notice.as_str());
            }
        });
    if let Some(i) = remove {
        state.catalog.items.remove(i);
        changed = true;
    }
    if import
        && let Some(path) = crate::dialogs::FileDialog::new()
            .set_title("Import enclosure list")
            .add_filter("CSV", &["csv", "txt"])
            .pick_file()
    {
        state.notice = Some(match std::fs::read_to_string(&path) {
            Ok(text) => state.import(&text),
            Err(e) => format!("Could not read {}: {e}", path.display()),
        });
    }
    if export
        && let Some(path) = crate::dialogs::FileDialog::new()
            .set_title("Export enclosure list")
            .set_file_name(CATALOG_FILE)
            .add_filter("CSV", &["csv"])
            .save_file()
    {
        state.notice = Some(match save_catalog(&state.catalog, &path) {
            Ok(()) => format!("Saved {}.", path.display()),
            Err(e) => format!("Could not save: {e}"),
        });
    }
    // Written when typing stops (no field has focus) or the window closes,
    // not on every keystroke.
    state.dirty |= changed;
    if state.dirty && (!open || !ctx.egui_wants_keyboard_input()) {
        state.save();
        state.dirty = false;
    }
    state.editor_open = open;
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::sizing::SizingDevice;

    fn device(loss: f64) -> SizingDevice {
        SizingDevice {
            width: 45.0,
            height: 90.0,
            depth: 0.0,
            power_loss: loss,
            load_factor: 1.0,
            clearance: None,
            guessed: false,
        }
    }

    #[test]
    fn the_list_round_trips_through_its_file() {
        let dir = std::env::temp_dir().join(format!("kopercad-enclosures-{}", std::process::id()));
        let path = dir.join(CATALOG_FILE);
        let mut state = SizingState::new();
        state.path = Some(path.clone());
        let notice = state.import("A;400;500;210;350;450;10;steel\nB;x\n");
        assert!(notice.starts_with("Imported 1 enclosure."), "{notice}");
        assert!(notice.contains("1 row skipped"), "{notice}");
        let (back, errors) = load_catalog(&path);
        assert!(errors.is_empty());
        assert_eq!(back, state.catalog);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_missing_file_is_an_empty_list() {
        let (catalog, errors) = load_catalog(Path::new("/nonexistent/enclosures.csv"));
        assert!(catalog.items.is_empty() && errors.is_empty());
    }

    #[test]
    fn result_lines_say_whether_cooling_is_needed() {
        let mut state = SizingState::new();
        state.path = None;
        state.import("Small;400;500;210;350;450;10;steel\n");
        let cool = recommend(&[device(1.0)], &state.catalog, &state.request());
        assert!(
            result_lines(&cool)
                .iter()
                .any(|l| l == "Natural cooling is enough.")
        );
        let hot = recommend(&[device(400.0)], &state.catalog, &state.request());
        let lines = result_lines(&hot).join("\n");
        assert!(lines.contains("Needs about"), "{lines}");
        assert!(lines.contains("m3/h"), "{lines}");
    }

    #[test]
    fn an_empty_list_explains_itself() {
        let state = SizingState::new();
        let rec = recommend(&[device(1.0)], &state.catalog, &state.request());
        let notes: Vec<String> = rec.notes.iter().map(note_text).collect();
        assert!(notes.iter().any(|n| n.contains("list is empty")));
    }
}
