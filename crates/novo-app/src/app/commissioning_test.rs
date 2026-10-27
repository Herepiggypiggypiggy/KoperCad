//! The Wire test tab of Commissioning mode: the point-to-point continuity
//! and insulation test sheet (novo-commissioning `test_sheet`), worked at the
//! bench in wiring order. A reading is typed against the expected value; a
//! miss opens a punch item and fails the wire's Continuity check, so the
//! Checks tab and the Punch tab follow without a second entry. The wires
//! nobody has measured stay listed in red: the dead wire cross-check.
//!
//! Expected values are planning values from the drawing, not a compliance
//! test record; the tab and the printed sheet say so.

use std::rc::Rc;

use eframe::egui;
use novo_commissioning::{
    BuildPlan, CheckKind, ContinuityInput, PointState, ReadingInput, Target, TestSheet, Verdict,
    point_state, record_reading, test_progress, untested,
};
use novo_core::{Project, WireId};
use novo_theme::{Palette, icons, space, widgets};

use super::cabinet_bar::note_ctx;
use super::commissioning::{CommissioningMode, now};
use super::{Level, NovoApp};

/// What the tab asks the app to do after the frame.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum TestAction {
    WritePdf,
}

/// State of the tab: the sheet kept for the revision, the wire being
/// measured and the two fields being typed.
#[derive(Default)]
pub(super) struct TestState {
    sheet: Option<(u64, Rc<TestSheet>)>,
    pub(super) selected: Option<WireId>,
    pub(super) continuity: String,
    pub(super) insulation: String,
    /// What the last Record did, shown under the form.
    pub(super) last: String,
}

/// A typed number: comma or point, no negative, finite.
pub(super) fn parse_number(text: &str) -> Option<f64> {
    let v: f64 = text.trim().replace(',', ".").parse().ok()?;
    (v.is_finite() && v >= 0.0).then_some(v)
}

impl CommissioningMode {
    /// The sheet for the project as it is now and the settings kept.
    pub(super) fn test_sheet(&mut self, project: &Project) -> Rc<TestSheet> {
        let revision = self.cache_revision(project);
        if let Some((r, sheet)) = &self.test.sheet
            && *r == revision
            && sheet.settings == self.store.test_settings()
        {
            return Rc::clone(sheet);
        }
        let plan = self.plan(project);
        let sheet = Rc::new(TestSheet::of(
            project,
            &BuildPlan::of(project),
            &plan,
            self.store.test_settings(),
        ));
        self.test.sheet = Some((revision, Rc::clone(&sheet)));
        sheet
    }

    /// Records what is typed for the selected wire. Returns false when
    /// nothing valid was typed.
    pub(super) fn record_typed(&mut self, project: &Project, open: bool) -> bool {
        let sheet = self.test_sheet(project);
        let Some(point) = self.test.selected.and_then(|w| sheet.point(w)) else {
            return false;
        };
        let typed_c = self.test.continuity.trim();
        let typed_i = self.test.insulation.trim();
        let continuity = if open {
            Some(ContinuityInput::Open)
        } else if typed_c.is_empty() {
            None
        } else {
            match parse_number(typed_c) {
                Some(v) => Some(ContinuityInput::Ohm(v)),
                None => {
                    self.test.last = "Type the ohms as a number, for example 0.12".to_owned();
                    return false;
                }
            }
        };
        let insulation_mohm = if typed_i.is_empty() {
            None
        } else {
            match parse_number(typed_i) {
                Some(v) => Some(v),
                None => {
                    self.test.last = "Type the megohms as a number, for example 50".to_owned();
                    return false;
                }
            }
        };
        if continuity.is_none() && insulation_mohm.is_none() {
            self.test.last = "Type a reading first".to_owned();
            return false;
        }
        let r = record_reading(
            &mut self.store,
            point,
            ReadingInput {
                continuity,
                insulation_mohm,
            },
            &self.who,
            &now(),
        );
        self.dirty = true;
        let missed = r.verdicts.continuity == Some(Verdict::Miss)
            || r.verdicts.insulation == Some(Verdict::Miss);
        self.test.last = if missed {
            "MISS: a punch item is open on the Punch tab".to_owned()
        } else {
            "Recorded".to_owned()
        };
        self.test.continuity.clear();
        self.test.insulation.clear();
        // Carry on with the next wire nobody has measured.
        let seq = point.seq;
        let next = untested(&sheet, &self.store)
            .into_iter()
            .find(|p| p.seq > seq)
            .or_else(|| untested(&sheet, &self.store).into_iter().next())
            .map(|p| p.wire);
        if let Some(next) = next {
            self.select_test_wire(project, next);
        }
        true
    }

    pub(super) fn select_test_wire(&mut self, project: &Project, wire: WireId) {
        self.test.selected = Some(wire);
        self.test.continuity.clear();
        self.test.insulation.clear();
        if let Some(w) = project.wire(wire) {
            self.reveal_wire = Some((w.page, wire));
        }
        self.selected = Some(Target::Wire(wire));
    }
}

/// A stored reading as a row says it: `open (max 0.23 ohm)` becomes
/// `open, limit 0.23 ohm`, `0.12 ohm (max 0.23)` becomes `0.12 ohm, limit 0.23 ohm`.
pub(super) fn reading_text(value: &str) -> String {
    let Some((head, tail)) = value.split_once(" (max ") else {
        return value.to_owned();
    };
    let limit = tail.trim_end_matches(')');
    let limit = if limit.ends_with("ohm") {
        limit.to_owned()
    } else {
        format!("{limit} ohm")
    };
    format!("{head}, limit {limit}")
}

fn state_mark(p: &Palette, state: PointState) -> (&'static str, egui::Color32) {
    match state {
        PointState::NotTested => (icons::CIRCLE, p.error),
        PointState::Partial => (icons::CHECK_CIRCLE, p.warning),
        PointState::Complete => (icons::CHECK_CIRCLE, p.success),
        PointState::Miss => (icons::WARNING_OCTAGON, p.error),
    }
}

pub(super) fn test_tab(
    ui: &mut egui::Ui,
    mode: &mut CommissioningMode,
    project: &Project,
) -> Option<TestAction> {
    let p = Palette::of(ui);
    let sheet = mode.test_sheet(project);
    let mut action = None;
    if sheet.points.is_empty() {
        ui.weak("This project has no wires to test yet");
        return None;
    }
    let progress = test_progress(&sheet, &mode.store);
    ui.label(
        egui::RichText::new(
            "Design aid: expected values are planning values from the drawing, not a \
             compliance test record.",
        )
        .small()
        .color(p.text_tertiary),
    );
    ui.add_space(space::S);
    ui.label(
        egui::RichText::new(format!(
            "{} wires in the drawing: {} tested, {} missed",
            progress.total,
            progress.total - progress.not_tested,
            progress.miss
        ))
        .strong(),
    );
    if progress.not_tested > 0 {
        ui.label(
            egui::RichText::new(format!(
                "{} wire(s) not tested yet (dead wire check)",
                progress.not_tested
            ))
            .color(p.error),
        );
    } else {
        ui.label(egui::RichText::new("Every wire of the drawing is tested").color(p.success));
    }
    ui.add_space(space::S);

    // Expected values.
    let mut settings = mode.store.test_settings();
    let before = settings;
    egui::CollapsingHeader::new("Expected values")
        .id_salt("test_settings")
        .show(ui, |ui| {
            ui.checkbox(&mut settings.insulation, "Also read insulation");
            ui.horizontal(|ui| {
                ui.label("Test voltage");
                ui.add(
                    egui::DragValue::new(&mut settings.insulation_volts)
                        .range(24..=1000)
                        .suffix(" V"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Least insulation");
                ui.add(
                    egui::DragValue::new(&mut settings.min_insulation_mohm)
                        .range(0.1..=1000.0)
                        .speed(0.1)
                        .suffix(" Mohm"),
                );
            });
            ui.horizontal(|ui| {
                ui.label("Contact allowance");
                ui.add(
                    egui::DragValue::new(&mut settings.contact_ohm)
                        .range(0.0..=1.0)
                        .speed(0.01)
                        .suffix(" ohm"),
                );
            });
            ui.weak(
                "Wire resistance is copper at 20 C from the drawn length, times a 1.25 margin.",
            );
        });
    if settings != before {
        mode.store.set_test_settings(settings);
        mode.dirty = true;
    }

    // The wire being measured.
    if mode.test.selected.is_none_or(|w| sheet.point(w).is_none()) {
        let first = untested(&sheet, &mode.store)
            .first()
            .map(|p| p.wire)
            .or_else(|| sheet.points.first().map(|p| p.wire));
        mode.test.selected = first;
        if let Some(w) = first {
            mode.selected = Some(Target::Wire(w));
        }
    }
    if let Some(point) = mode.test.selected.and_then(|w| sheet.point(w)) {
        ui.separator();
        let name = if point.number.is_empty() {
            point.kind.short()
        } else {
            point.number.clone()
        };
        ui.label(egui::RichText::new(format!("#{}  {name}", point.seq)).strong());
        ui.label(
            egui::RichText::new(format!("{} -> {}", point.from, point.to))
                .small()
                .color(p.text_secondary),
        );
        if !point.hint.is_empty() {
            ui.label(
                egui::RichText::new(&point.hint)
                    .small()
                    .color(p.text_tertiary),
            );
        }
        ui.weak("Zero the test leads first. For OL or over range press Open circuit.");
        let mut record = false;
        let mut open = false;
        ui.horizontal(|ui| {
            ui.label(format!("Continuity {}", point.expected.continuity_text()));
        });
        ui.horizontal(|ui| {
            let r = ui.add(
                egui::TextEdit::singleline(&mut mode.test.continuity)
                    .hint_text("ohm")
                    .desired_width(80.0),
            );
            note_ctx(ui.ctx(), "commissioning.test_ohm", "Continuity reading", &r);
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                record = true;
            }
            let o = ui
                .button("Open circuit")
                .on_hover_text("The meter showed no connection: a miss");
            note_ctx(ui.ctx(), "commissioning.test_open", "Open circuit", &o);
            open = o.clicked();
        });
        if point.expected.insulation.is_some() {
            ui.label(format!("Insulation {}", point.expected.insulation_text()));
            ui.horizontal(|ui| {
                let r = ui.add(
                    egui::TextEdit::singleline(&mut mode.test.insulation)
                        .hint_text("Mohm")
                        .desired_width(80.0),
                );
                note_ctx(
                    ui.ctx(),
                    "commissioning.test_mohm",
                    "Insulation reading",
                    &r,
                );
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    record = true;
                }
            });
            ui.weak("Disconnect electronics and surge arresters before an insulation test");
        }
        let b = ui.button("Record");
        note_ctx(ui.ctx(), "commissioning.test_record", "Record reading", &b);
        if b.clicked() {
            record = true;
        }
        if (record || open) && !mode.record_typed(project, open) && mode.test.last.is_empty() {
            mode.test.last = "Type a reading first".to_owned();
        }
        if !mode.test.last.is_empty() {
            let missed = mode.test.last.starts_with("MISS");
            ui.label(
                egui::RichText::new(&mode.test.last)
                    .small()
                    .color(if missed { p.error } else { p.text_secondary }),
            );
        }
    }

    ui.separator();
    widgets::section_label(ui, "Wiring order");
    let mut pick = None;
    egui::ScrollArea::vertical()
        .id_salt("test_list")
        .max_height(ui.available_height().max(120.0) - 40.0)
        .show(ui, |ui| {
            for point in &sheet.points {
                let state = point_state(&mode.store, point);
                let (icon, colour) = state_mark(p, state);
                let name = if point.number.is_empty() {
                    point.kind.short()
                } else {
                    point.number.clone()
                };
                let selected = mode.test.selected == Some(point.wire);
                let reading = mode
                    .store
                    .record(Target::Wire(point.wire), CheckKind::Continuity)
                    .map(|r| {
                        if r.value.is_empty() && r.state == novo_commissioning::CheckState::Passed {
                            "ticked on Checks".to_owned()
                        } else {
                            reading_text(&r.value)
                        }
                    })
                    .unwrap_or_default();
                let row = ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).color(colour));
                    let text = if reading.is_empty() {
                        format!("{}.  Wire {name}", point.seq)
                    } else {
                        format!("{}.  Wire {name}, {reading}", point.seq)
                    };
                    ui.selectable_label(selected, text).on_hover_text(format!(
                        "{} -> {}\nContinuity {}",
                        point.from,
                        point.to,
                        point.expected.continuity_text()
                    ))
                });
                if row.inner.clicked() {
                    pick = Some(point.wire);
                }
            }
        });
    if let Some(w) = pick {
        mode.test.last.clear();
        mode.select_test_wire(project, w);
    }
    let w = ui.button("Write test sheet PDF");
    note_ctx(
        ui.ctx(),
        "commissioning.test_pdf",
        "Write test sheet PDF",
        &w,
    );
    if w.clicked() {
        action = Some(TestAction::WritePdf);
    }
    action
}

impl NovoApp {
    /// Writes the test sheet PDF next to the project (`<project> test
    /// sheet.pdf`), replacing only a file of this program's own making.
    pub(super) fn export_test_sheet(&mut self) -> Option<std::path::PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = super::pdf::replaceable_path(&dir.join(format!("{stem} test sheet.pdf")));
        self.export_test_sheet_to(&path)
    }

    pub(super) fn export_test_sheet_to(
        &mut self,
        path: &std::path::Path,
    ) -> Option<std::path::PathBuf> {
        self.load_commissioning();
        let name = self.machine_title().unwrap_or_default();
        let choice = self.machine_variant();
        let project = self.backend.document().map(|d| d.project())?;
        let project: &Project = choice.as_deref().unwrap_or(project);
        let revision = project.revision();
        let mode = self.wb.commissioning.as_mut()?;
        let sheet = mode.test_sheet(project);
        let output = novo_commissioning::test_sheet_report(&sheet, &mode.store, revision);
        if sheet.points.is_empty() {
            self.message(Level::Warning, "There are no wires to put on a test sheet.");
            return None;
        }
        let options = novo_pdf::ReportPdfOptions {
            title: format!("{name}  Continuity and insulation test sheet"),
            row_mm: output
                .sections
                .iter()
                .map(|s| (s.key.clone(), novo_commissioning::build_sheet_row_mm(s)))
                .collect(),
            language: self.report_translator(),
            ..novo_pdf::ReportPdfOptions::default()
        };
        let bytes = novo_pdf::export_report_pdf(&[&output], &options);
        let replaced = path.exists();
        match super::pdf::write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {}", path.display()), path);
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the test sheet: {e}"),
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_number, reading_text};
    use crate::actions::Action;
    use crate::app::commissioning::Tab;
    use crate::app::gate1_tests::Drive;
    use crate::app::workspace::Workspace;
    use novo_commissioning::{CheckKind, CheckState, Target};

    fn bench() -> (tempfile::TempDir, Drive) {
        let dir = tempfile::tempdir().unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, Action::OpenExample);
        d.frame();
        d.app.switch_workspace(Workspace::Commissioning);
        for _ in 0..3 {
            d.frame();
        }
        d.app.wb.commissioning.as_mut().unwrap().tab = Tab::Test;
        for _ in 0..3 {
            d.frame();
        }
        (dir, d)
    }

    #[test]
    fn the_tab_selects_the_first_unmeasured_wire_and_a_typed_miss_opens_a_punch_item() {
        let (_dir, mut d) = bench();
        let project = d.doc().project().clone();
        let mode = d.app.wb.commissioning.as_mut().unwrap();
        let first = mode.test.selected.expect("a wire is selected");
        let sheet = mode.test_sheet(&project);
        assert_eq!(sheet.points[0].wire, first);
        mode.test.continuity = "99".into();
        assert!(mode.record_typed(&project, false));
        assert_eq!(
            mode.store.state(Target::Wire(first), CheckKind::Continuity),
            CheckState::Failed
        );
        assert_eq!(mode.store.open_punch_count(), 1);
        assert!(mode.test.last.starts_with("MISS"));
        // The tab moved on to the next wire nobody measured.
        assert_ne!(mode.test.selected, Some(first));
        assert!(mode.dirty);
    }

    #[test]
    fn nonsense_is_refused_and_records_nothing() {
        let (_dir, mut d) = bench();
        let project = d.doc().project().clone();
        let mode = d.app.wb.commissioning.as_mut().unwrap();
        mode.test.continuity = "abc".into();
        assert!(!mode.record_typed(&project, false));
        mode.test.continuity.clear();
        assert!(!mode.record_typed(&project, false));
        assert!(mode.store.is_empty());
    }

    #[test]
    fn the_exported_sheet_pdf_lands_next_to_the_project() {
        let (dir, mut d) = bench();
        let path = dir.path().join("sheet.pdf");
        let out = d.app.export_test_sheet_to(&path).expect("written");
        assert_eq!(out, path);
        assert!(std::fs::read(&path).unwrap().starts_with(b"%PDF"));
    }

    #[test]
    fn a_reading_row_spells_out_the_limit() {
        assert_eq!(reading_text("open (max 0.23 ohm)"), "open, limit 0.23 ohm");
        assert_eq!(
            reading_text("0.12 ohm (max 0.23)"),
            "0.12 ohm, limit 0.23 ohm"
        );
        assert_eq!(reading_text("fine"), "fine");
    }

    #[test]
    fn numbers_take_a_comma_or_a_point_and_refuse_nonsense() {
        assert_eq!(parse_number("0.12"), Some(0.12));
        assert_eq!(parse_number(" 0,12 "), Some(0.12));
        assert_eq!(parse_number("-1"), None);
        assert_eq!(parse_number("abc"), None);
        assert_eq!(parse_number("NaN"), None);
        assert_eq!(parse_number("inf"), None);
        assert_eq!(parse_number(""), None);
    }
}
