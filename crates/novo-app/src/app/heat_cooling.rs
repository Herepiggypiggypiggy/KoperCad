//! "Cooling selection" in the Heat panels: from the heat of the shown result,
//! a fan, filter fan or cooling unit that holds a target temperature inside.
//! The sizing itself is `novo_thermal::cooling` (a wall balance in the manner
//! of IEC 60890 plus typical derating); this file chooses the inputs from the
//! result and draws the answer. Products come from the parts of the project,
//! the person's own list (`cooling.csv` under `%APPDATA%`, never bundled from
//! makers) and, last, generic planning sizes that are labelled as such.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_thermal::cooling::{self, Basis, Product, Selection, Source};

use super::heat_run::Solution;

/// File name inside the KoperCAD folder under `%APPDATA%`.
pub const LIST_FILE: &str = "cooling.csv";

/// Target used when no device states a limit, degrees C.
pub const DEFAULT_TARGET_C: f64 = 40.0;

pub fn list_path() -> Option<PathBuf> {
    Some(
        novo_storage::profile::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(LIST_FILE),
    )
}

/// What the person chooses about the selection, and the product lists.
#[derive(Debug, Clone, Default)]
pub(super) struct CoolingSettings {
    /// `None` follows the lowest ambient limit among the devices.
    pub target_c: Option<f64>,
    /// The enclosure has to stay closed: no fans.
    pub closed: bool,
    /// Fans and units among the parts of the project, refreshed with it.
    pub parts: Vec<Product>,
    mine: Option<Vec<Product>>,
    /// What the last import or load said.
    pub notice: Option<String>,
}

impl CoolingSettings {
    /// The person's own list, read once.
    fn mine(&mut self) -> &[Product] {
        if self.mine.is_none() {
            let (list, errors) = list_path()
                .map(|p| load_list(&p))
                .unwrap_or((Vec::new(), Vec::new()));
            if let Some(first) = errors.first() {
                self.notice = Some(format!("{LIST_FILE}: {first}"));
            }
            self.mine = Some(list);
        }
        self.mine.as_deref().unwrap_or(&[])
    }

    /// Imports `text` as the list and keeps it for the next session.
    pub(super) fn import(&mut self, text: &str) -> String {
        self.import_to(text, list_path())
    }

    fn import_to(&mut self, text: &str, path: Option<PathBuf>) -> String {
        let (list, errors) = cooling::products_from_csv(text);
        let mut note = format!("Read {} product(s)", list.len());
        if let Some(first) = errors.first() {
            note.push_str(&format!(
                ", {} row(s) skipped (first: {first})",
                errors.len()
            ));
        }
        if !list.is_empty() {
            if let Some(path) = path {
                if let Some(dir) = path.parent() {
                    let _ = std::fs::create_dir_all(dir);
                }
                if let Err(e) = std::fs::write(&path, cooling::products_to_csv(&list)) {
                    note.push_str(&format!("; could not save the list: {e}"));
                }
            }
            self.mine = Some(list);
        }
        note.push('.');
        note
    }

    /// Every product, real data first in the sense that `select` prefers it.
    pub(super) fn products(&mut self) -> Vec<Product> {
        let mut all = self.parts.clone();
        all.extend(self.mine().iter().cloned());
        all.extend(cooling::planning_products());
        all
    }

    pub(super) fn real_counts(&mut self) -> (usize, usize) {
        let parts = self.parts.len();
        (parts, self.mine().len())
    }
}

fn load_list(path: &Path) -> (Vec<Product>, Vec<cooling::ListError>) {
    match std::fs::read_to_string(path) {
        Ok(text) => cooling::products_from_csv(&text),
        Err(_) => (Vec::new(), Vec::new()),
    }
}

/// The lowest ambient limit among the devices of the result, if any states one.
pub(super) fn lowest_limit_c(sol: &Solution) -> Option<f64> {
    sol.snapshot
        .devices
        .iter()
        .filter_map(|d| d.rated_max_ambient_c)
        .filter(|v| v.is_finite())
        .reduce(f64::min)
}

/// What the selection is made from; `None` for a result that did not come
/// with its enclosure (a stub) or that has no heat.
pub(super) fn basis_for(sol: &Solution, settings: &CoolingSettings) -> Option<Basis> {
    let e = sol.enclosure.as_ref()?;
    let loss_w: f64 = sol.snapshot.devices.iter().map(|d| d.loss_w).sum();
    if loss_w.is_nan() || loss_w <= 0.0 {
        return None;
    }
    Some(Basis {
        loss_w,
        ambient_c: sol.ambient,
        target_c: settings
            .target_c
            .or_else(|| lowest_limit_c(sol))
            .unwrap_or(DEFAULT_TARGET_C),
        width_mm: e.width_mm,
        height_mm: e.height_mm,
        depth_mm: e.depth_mm,
        material: e.material,
        installation: e.installation,
        closed_required: settings.closed,
    })
}

/// Mean air temperature of the result, degrees C.
fn model_mean_air_c(sol: &Solution) -> Option<f64> {
    let s = &sol.snapshot;
    let (mut sum, mut n) = (0.0_f64, 0usize);
    for (t, m) in s.temperature_c.iter().zip(&s.material) {
        if *m == 0 && t.is_finite() {
            sum += f64::from(*t);
            n += 1;
        }
    }
    (n > 0).then(|| sum / n as f64)
}

/// The basis and selection of a result, for the panels and the reports.
pub(super) fn selection_for(
    sol: &Solution,
    settings: &mut CoolingSettings,
) -> Option<(Basis, Selection)> {
    let basis = basis_for(sol, settings)?;
    let products = settings.products();
    let selection = cooling::select(&basis, &products);
    Some((basis, selection))
}

/// The lines a report prints for the shown result.
pub(super) fn report_lines(sol: &Solution, settings: &mut CoolingSettings) -> Vec<String> {
    let Some((basis, selection)) = selection_for(sol, settings) else {
        return Vec::new();
    };
    let mut lines = selection.report_lines(&basis);
    lines.push(
        "Sized for the closed enclosure: vents or fans the run had are not credited.".to_owned(),
    );
    lines
}

/// The section, for the 3D Heat panel and the 2D legend. Returns nothing:
/// the settings are edited in place.
pub(super) fn show(
    ui: &mut egui::Ui,
    sol: &Solution,
    settings: &mut CoolingSettings,
    stale: bool,
    running: bool,
) {
    let pal = crate::theme::ui_palette();
    let small = |text: &str, colour: egui::Color32| egui::RichText::new(text).small().color(colour);
    egui::CollapsingHeader::new("Cooling selection")
        .id_salt("heat_cooling")
        .default_open(false)
        .show(ui, |ui| {
            let auto_target = lowest_limit_c(sol);
            let Some((basis, selection)) = selection_for(sol, settings) else {
                ui.label(small(
                    "Needs a result with device losses and the enclosure size.",
                    pal.text_secondary,
                ));
                return;
            };
            let n = &selection.need;
            let colour = if stale || running {
                pal.text_secondary
            } else if n.natural_ok() {
                pal.success
            } else if selection.proposal().is_some() {
                pal.text_primary
            } else {
                pal.error
            };
            ui.label(egui::RichText::new(selection.sentence()).strong().color(colour));
            if running {
                ui.label(small(
                    "Preliminary: the run is still going, so this can change.",
                    pal.warning,
                ));
            } else if stale {
                ui.label(small(
                    "Out of date: the layout changed since this run.",
                    pal.warning,
                ));
            }
            ui.add_space(2.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(small("Hold inside at", pal.text_secondary));
                let mut target = basis.target_c;
                if ui
                    .add(
                        egui::DragValue::new(&mut target)
                            .range(10.0..=80.0)
                            .speed(0.5)
                            .suffix(" °C"),
                    )
                    .on_hover_text(match auto_target {
                        Some(t) => format!(
                            "Starts at the lowest ambient limit among the devices ({t:.0} °C)."
                        ),
                        None => format!(
                            "No device states an ambient limit, so it starts at {DEFAULT_TARGET_C:.0} °C."
                        ),
                    })
                    .changed()
                {
                    settings.target_c = Some(target);
                }
                if settings.target_c.is_some() && ui.small_button("Auto").clicked() {
                    settings.target_c = None;
                }
            });
            ui.checkbox(&mut settings.closed, "Must stay closed (no fans)")
                .on_hover_text(
                    "Dust, spray or the IP rating rule out an opening: only a cooling unit is proposed.",
                );
            ui.add_space(2.0);
            for (i, o) in selection.options.iter().enumerate() {
                let recommended = selection.recommended == Some(i);
                let text = match (&o.choice, &o.why_not) {
                    (Some(c), _) => format!(
                        "{}: {}{} ({}) → {:.0} °C inside",
                        o.kind.label(),
                        if c.count > 1 {
                            format!("{} × ", c.count)
                        } else {
                            String::new()
                        },
                        c.product.name,
                        source_word(&c.product.source),
                        c.inside_c
                    ),
                    (None, Some(why)) => format!("{}: {why}", o.kind.label()),
                    (None, None) => format!("{}: no listed product is large enough", o.kind.label()),
                };
                let c = if recommended { pal.text_primary } else { pal.text_secondary };
                let mut rt = egui::RichText::new(text).small().color(c);
                if recommended {
                    rt = rt.strong();
                }
                ui.label(rt);
            }
            ui.add_space(2.0);
            egui::CollapsingHeader::new(egui::RichText::new("The balance").small())
                .id_salt("heat_cooling_balance")
                .show(ui, |ui| {
                    let row = |ui: &mut egui::Ui, text: String| {
                        ui.label(small(&text, pal.text_secondary));
                    };
                    row(ui, format!("Loss to carry out: {:.0} W", n.loss_w));
                    row(
                        ui,
                        format!(
                            "Walls carry {:.0} W at {:.0} °C inside, {:.0} °C outside (k {:.1}, {:.2} m²)",
                            n.wall_w, basis.target_c, basis.ambient_c, n.k, n.surface_m2
                        ),
                    );
                    row(ui, format!("Left for cooling: {:.0} W", n.cooling_w));
                    if let Some(f) = n.airflow_m3h {
                        row(ui, format!("Fan flow needed once installed: {f:.0} m³/h"));
                    }
                    row(
                        ui,
                        format!(
                            "Without cooling the wall balance gives {:.0} °C inside",
                            n.natural_inside_c
                        ),
                    );
                    if let Some(m) = model_mean_air_c(sol) {
                        row(
                            ui,
                            format!(
                                "This run's mean air: {m:.0} °C. The two differ because the run \
                                 solves the flow and the wall balance is the standard's estimate."
                            ),
                        );
                    }
                    row(
                        ui,
                        "Sized for the closed enclosure: vents or fans the run had are not credited."
                            .to_owned(),
                    );
                });
            let (parts, mine) = settings.real_counts();
            ui.label(small(
                &format!(
                    "Products: {parts} from parts, {mine} from your list, then generic planning sizes."
                ),
                pal.text_secondary,
            ));
            if ui
                .small_button("Import my list…")
                .on_hover_text(
                    "CSV with name;kind;airflow_m3h;cooling_w (kind: fan, filter fan, cooling unit). \
                     Kept for next time. Parts with airflow_m3h or cooling_capacity_w also count.",
                )
                .clicked()
                && let Some(path) = crate::dialogs::FileDialog::new()
                    .set_title("Import fans and cooling units")
                    .add_filter("CSV", &["csv", "txt"])
                    .pick_file()
            {
                settings.notice = Some(match std::fs::read_to_string(&path) {
                    Ok(text) => settings.import(&text),
                    Err(e) => format!("Could not read {}: {e}", path.display()),
                });
            }
            if let Some(notice) = &settings.notice {
                ui.label(small(notice, pal.text_secondary));
            }
            ui.label(small(
                "Model estimate with planning values: the loss is the run's, the walls follow IEC 60890, \
                 filters take about a third of a fan's flow. The maker's selection tool decides.",
                pal.text_secondary,
            ));
        });
}

fn source_word(source: &Source) -> &'static str {
    match source {
        Source::Part(_) => "part data",
        Source::Mine => "your list",
        Source::Planning => "planning size",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn importing_a_list_reads_good_rows_and_counts_the_bad() {
        let mut s = CoolingSettings::default();
        let note = s.import_to(
            "name;kind;airflow_m3h;cooling_w\nFF 300;filter fan;300;\nX;pump;1;\n",
            None,
        );
        assert!(note.starts_with("Read 1 product"), "{note}");
        assert!(note.contains("1 row(s) skipped"), "{note}");
        let all = s.products();
        assert!(
            all.iter()
                .any(|p| p.name == "FF 300" && p.source == Source::Mine)
        );
        // The generic sizes come last.
        assert_eq!(all.last().unwrap().source, Source::Planning);
        assert!(all.iter().any(|p| p.kind == cooling::Kind::CoolingUnit));
    }
}
