//! The Quality tab of the Parts window: how complete each part's data is,
//! what is missing, and an export of that list. A thin view over
//! `novo_parts::quality` (lane PARTS-Q).
//!
//! The tab sits next to the part list: Parts window > Quality. Nothing here
//! changes the catalog; the score only reports what is recorded.

use eframe::egui;
use novo_parts::{Catalog, Criterion, Fill, Grade, PartKey, PartQuality, QualityReport};

/// Which side of the Parts window is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Side {
    #[default]
    Parts,
    Quality,
}

/// State of the Quality tab.
#[derive(Default)]
pub(crate) struct QualityTab {
    side: Side,
    /// Only parts with a gap on this line; `None` lists every incomplete part.
    filter: Option<Criterion>,
    picked: Option<PartKey>,
    /// The scores and the catalog size they were computed for.
    report: Option<(usize, QualityReport)>,
    /// Result of the last export, shown beside the button.
    note: Option<String>,
}

impl QualityTab {
    /// Scores again when the catalog changed size or the user asked.
    fn ensure(&mut self, catalog: &Catalog) {
        let stale = self
            .report
            .as_ref()
            .is_none_or(|(len, _)| *len != catalog.len());
        if stale {
            self.report = Some((catalog.len(), novo_parts::quality_report(catalog)));
        }
    }
}

/// Draws the Parts / Quality switch. While Quality is showing it draws the
/// tab and returns `Some`; the Parts window then skips its own list. The
/// inner value is the part to open in the list when the user asked for one.
pub(crate) fn tab(
    ui: &mut egui::Ui,
    state: &mut QualityTab,
    catalog: &Catalog,
) -> Option<Option<PartKey>> {
    ui.horizontal(|ui| {
        ui.selectable_value(&mut state.side, Side::Parts, "Parts");
        ui.selectable_value(&mut state.side, Side::Quality, "Quality");
    });
    if state.side != Side::Quality {
        return None;
    }
    ui.separator();
    let jump = quality_ui(ui, state, catalog);
    if jump.is_some() {
        state.side = Side::Parts;
    }
    Some(jump)
}

fn grade_color(grade: Grade) -> egui::Color32 {
    match grade {
        Grade::Good => novo_theme::tokens::SUCCESS,
        Grade::Fair => novo_theme::tokens::WARNING,
        Grade::Poor => novo_theme::tokens::ERROR,
    }
}

fn fill_color(fill: Fill) -> egui::Color32 {
    match fill {
        Fill::Complete => novo_theme::tokens::SUCCESS,
        Fill::Partial => novo_theme::tokens::WARNING,
        Fill::Missing => novo_theme::tokens::ERROR,
    }
}

fn quality_ui(ui: &mut egui::Ui, state: &mut QualityTab, catalog: &Catalog) -> Option<PartKey> {
    let mut refresh = false;
    let mut export = false;
    let mut jump = None;
    state.ensure(catalog);
    let (_, report) = state.report.as_ref()?;
    let filter = state.filter;
    let mut next_filter = filter;
    let mut picked = state.picked.clone();

    ui.horizontal_wrapped(|ui| {
        if report.is_empty() {
            ui.weak("The catalog has no parts.");
        } else {
            let avg = report.average_percent().unwrap_or(0.0);
            ui.strong(format!("{avg:.0} % complete on average"));
            for grade in [Grade::Good, Grade::Fair, Grade::Poor] {
                ui.colored_label(
                    grade_color(grade),
                    format!(
                        "{} {}",
                        report.count_grade(grade),
                        grade.label().to_lowercase()
                    ),
                );
            }
            ui.weak(format!("of {} parts", report.len()));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button("Export missing data")
                .on_hover_text("Saves a CSV of every part with a gap and what is missing")
                .clicked()
            {
                export = true;
            }
            if ui.small_button("Refresh").clicked() {
                refresh = true;
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.weak("Gaps:");
        if ui
            .add(egui::Button::selectable(filter.is_none(), "All"))
            .clicked()
        {
            next_filter = None;
        }
        for c in Criterion::ALL {
            let text = format!("{} {}", c.label(), report.count_gaps(c));
            if ui
                .add(egui::Button::selectable(filter == Some(c), text))
                .clicked()
            {
                next_filter = Some(c);
            }
        }
    });
    ui.separator();

    let rows: Vec<&PartQuality> = match filter {
        Some(c) => report.with_gap(c),
        None => report.incomplete(),
    };
    let detail = picked
        .as_ref()
        .and_then(|k| report.rows.iter().find(|r| &r.key == k));
    egui::Panel::right("parts_quality_detail")
        .exact_size(300.0)
        .resizable(false)
        .show(ui, |ui| match detail {
            Some(row) => {
                if let Some(key) = detail_ui(ui, row) {
                    jump = Some(key);
                }
            }
            None => {
                ui.weak("Pick a part to see what is missing.");
            }
        });
    egui::CentralPanel::default().show(ui, |ui| {
        if rows.is_empty() {
            ui.weak(if report.is_empty() {
                ""
            } else {
                "Nothing is missing here."
            });
            return;
        }
        egui::ScrollArea::vertical().show_rows(ui, 24.0, rows.len(), |ui, range| {
            for row in &rows[range] {
                let done = &row.completeness;
                ui.horizontal(|ui| {
                    ui.colored_label(grade_color(done.grade()), format!("{:>3} %", done.percent));
                    let selected = picked.as_ref() == Some(&row.key);
                    let text = format!(
                        "{}  {}  {}",
                        row.manufacturer, row.order_number, row.description
                    );
                    if ui
                        .add(egui::Button::selectable(selected, text).truncate())
                        .clicked()
                    {
                        picked = Some(row.key.clone());
                    }
                });
            }
        });
    });

    if export {
        state.note = Some(export_csv(&report.missing_csv()));
    }
    state.filter = next_filter;
    state.picked = picked;
    if refresh {
        state.report = None;
    }
    if let Some(note) = &state.note {
        ui.weak(note);
    }
    jump
}

/// The right-hand panel: one line per criterion with the reason, and the
/// button that opens the part in the list. Returns the part to open.
fn detail_ui(ui: &mut egui::Ui, row: &PartQuality) -> Option<PartKey> {
    let mut open = None;
    ui.heading(&row.order_number);
    ui.label(&row.manufacturer);
    if !row.description.is_empty() {
        ui.weak(&row.description);
    }
    ui.add_space(6.0);
    ui.colored_label(
        grade_color(row.completeness.grade()),
        format!("{} % complete", row.completeness.percent),
    );
    ui.add_space(6.0);
    for f in &row.completeness.findings {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(fill_color(f.fill), format!("{:<10}", f.fill.label()));
            ui.strong(f.criterion.label());
        });
        if !f.note.is_empty() {
            ui.weak(&f.note);
        }
    }
    ui.add_space(8.0);
    if ui.button("Show in Parts").clicked() {
        open = Some(row.key.clone());
    }
    open
}

/// Asks where to save and writes the report; returns the line to show.
fn export_csv(csv: &str) -> String {
    let Some(path) = rfd::FileDialog::new()
        .set_title("Save the missing-data report")
        .set_file_name("parts-missing-data.csv")
        .add_filter("CSV", &["csv"])
        .save_file()
    else {
        return "Export cancelled".into();
    };
    match std::fs::write(&path, csv) {
        Ok(()) => format!("Saved {}", path.display()),
        Err(e) => format!("Could not save {}: {e}", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_parts::CatalogPart;

    #[test]
    fn the_scores_follow_the_catalog_size() {
        let mut catalog = Catalog::new();
        let mut tab = QualityTab::default();
        tab.ensure(&catalog);
        assert!(tab.report.as_ref().unwrap().1.is_empty());
        catalog.upsert(CatalogPart::new("ABB", "S201-B16"));
        tab.ensure(&catalog);
        assert_eq!(tab.report.as_ref().unwrap().1.len(), 1);
    }
}
