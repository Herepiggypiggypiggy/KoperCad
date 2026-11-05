//! Problem triangles for the parts catalogue (docs/design/problem-triangles.md,
//! lane T9).
//!
//! Two shapes live here. A **row triangle** sits in a leading slot of a list
//! row (the import preview, a pick-list entry, the quality list) and opens a
//! popover with the cell detail. A **job flag** is what a failed catalogue
//! action leaves behind instead of a red toast: it lives in the flag store
//! under [`Source::Parts`], is drawn as a triangle in the Parts window, and
//! goes when the same action succeeds.

use eframe::egui;
use novo_flags::{Action, Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity, Source};

use super::NovoApp;

/// Side of a triangle in a row; the slot around it is a little larger so the
/// text beside it keeps its column.
const SIDE: f32 = 15.0;
const SLOT: f32 = 20.0;

/// What a row triangle says.
pub(super) struct RowFlag {
    pub(super) severity: Severity,
    /// Could not be judged (missing data): drawn dashed.
    pub(super) unchecked: bool,
    pub(super) title: String,
    pub(super) lines: Vec<String>,
}

impl RowFlag {
    pub(super) fn new(
        severity: Severity,
        title: impl Into<String>,
        lines: impl IntoIterator<Item = String>,
    ) -> Self {
        Self {
            severity,
            unchecked: false,
            title: title.into(),
            lines: lines.into_iter().collect(),
        }
    }

    pub(super) fn unchecked(mut self) -> Self {
        self.unchecked = true;
        self
    }

    /// "Error: Not imported" for screen readers.
    pub(super) fn accessible_name(&self) -> String {
        let level = match (self.severity, self.unchecked) {
            (Severity::Red, _) => "Error",
            (Severity::Yellow, true) => "Not checked",
            (Severity::Yellow, false) => "Warning",
        };
        format!("{level}: {}", self.title)
    }
}

/// Maps the parts engine's severity into the one flag vocabulary; a note is
/// not a flag.
pub(super) fn from_parts_severity(s: novo_parts::Severity) -> Option<Severity> {
    match s {
        novo_parts::Severity::Error => Some(Severity::Red),
        novo_parts::Severity::Warning => Some(Severity::Yellow),
        novo_parts::Severity::Note => None,
    }
}

/// Draws the triangle in a fixed slot and opens its popover on a click. The
/// slot is reserved even when `flag` is `None`, so the columns of a list stay
/// aligned.
pub(super) fn slot(ui: &mut egui::Ui, flag: Option<&RowFlag>) -> Option<egui::Response> {
    slot_with(ui, flag, None)
}

/// [`slot`] with a Dismiss button in the popover (job flags).
fn slot_with(
    ui: &mut egui::Ui,
    flag: Option<&RowFlag>,
    dismiss: Option<&mut bool>,
) -> Option<egui::Response> {
    let sense = if flag.is_some() {
        egui::Sense::click()
    } else {
        egui::Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(SLOT, SLOT), sense);
    let flag = flag?;
    if ui.is_rect_visible(rect) {
        novo_theme::widgets::flag_triangle(
            ui.painter(),
            rect.center(),
            SIDE,
            flag.severity == Severity::Red,
            flag.unchecked,
            novo_theme::Palette::of(ui),
        );
    }
    let name = flag.accessible_name();
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    egui::Popup::menu(&response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| popover(ui, flag, dismiss));
    Some(response)
}

/// The popover content: title, the detail lines, Copy.
fn popover(ui: &mut egui::Ui, flag: &RowFlag, dismiss: Option<&mut bool>) {
    ui.set_max_width(320.0);
    ui.label(egui::RichText::new(&flag.title).strong());
    for line in &flag.lines {
        ui.label(line);
    }
    ui.horizontal(|ui| copy_and_dismiss(ui, flag, dismiss));
}

fn copy_and_dismiss(ui: &mut egui::Ui, flag: &RowFlag, dismiss: Option<&mut bool>) {
    if let Some(gone) = dismiss
        && ui.small_button("Dismiss").clicked()
    {
        *gone = true;
        ui.close();
    }
    if ui.small_button("Copy").clicked() {
        let text = std::iter::once(flag.title.as_str())
            .chain(flag.lines.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        ui.ctx().copy_text(text);
        ui.close();
    }
}

/// A triangle without a row around it (a header, a note): the flag and
/// nothing else.
pub(super) fn inline(ui: &mut egui::Ui, flag: &RowFlag) -> egui::Response {
    slot(ui, Some(flag)).expect("a flag was given")
}

/// A triangle followed by a short plain label; the long text is the popover.
pub(super) fn labelled(
    ui: &mut egui::Ui,
    severity: Severity,
    label: &str,
    lines: impl IntoIterator<Item = String>,
) {
    let flag = RowFlag::new(severity, label, lines);
    ui.horizontal(|ui| {
        inline(ui, &flag);
        ui.label(label);
    });
}

/// The rule key of a job flag.
const JOB_RULE: &str = "job";

fn job_anchor(job: &str) -> Anchor {
    Anchor::Job(format!("parts:{job}"))
}

fn is_parts_job(flag: &Flag) -> bool {
    flag.source == Source::Parts && flag.anchors.iter().any(|a| matches!(a, Anchor::Job(_)))
}

impl NovoApp {
    /// A catalogue action failed: leave a triangle where the action lives
    /// (and in the Flags store) instead of a toast. The same action failing
    /// again renews the one flag; the same action succeeding clears it.
    pub(super) fn parts_job_failed(
        &mut self,
        job: &str,
        severity: Severity,
        title: &str,
        detail: impl AsRef<str>,
    ) {
        let raw = detail.as_ref();
        log::warn!("{title}: {raw}");
        // With the Parts window closed there is no place for the triangle yet:
        // say it once in words until the job tray lands.
        if self.wb.parts.is_none() {
            self.message(
                if severity == Severity::Red {
                    super::Level::Error
                } else {
                    super::Level::Warning
                },
                format!("{title}: {raw}"),
            );
        }
        let plain = super::error_words::plain(raw);
        self.wb.flags.store.set(FlagDraft {
            severity,
            unchecked: false,
            source: Source::Parts,
            rule: format!("{JOB_RULE}.{job}"),
            anchors: vec![job_anchor(job)],
            title: title.to_owned(),
            detail: Detail::new([plain]),
            actions: vec![Action::Copy],
            waiver_key: None,
            life: Life::Until,
        });
    }

    /// The action worked: its flag goes.
    pub(super) fn parts_job_ok(&mut self, job: &str) {
        let id = Flag::from_draft(FlagDraft {
            severity: Severity::Red,
            unchecked: false,
            source: Source::Parts,
            rule: format!("{JOB_RULE}.{job}"),
            anchors: vec![job_anchor(job)],
            title: String::new(),
            detail: Detail::default(),
            actions: Vec::new(),
            waiver_key: None,
            life: Life::Until,
        })
        .id;
        self.wb.flags.store.clear(id);
    }

    /// The job flags of the catalogue, oldest first by id.
    pub(super) fn parts_job_flags(&self) -> Vec<Flag> {
        self.wb
            .flags
            .store
            .list(false)
            .into_iter()
            .filter(|f| is_parts_job(f))
            .cloned()
            .collect()
    }

    /// Dismisses job flags the person has read.
    pub(super) fn parts_job_dismiss(&mut self, ids: &[FlagId]) {
        for id in ids {
            self.wb.flags.store.clear(*id);
        }
    }
}

/// The job flags as a row of triangles; returns the ones the person
/// dismissed from the popover.
pub(super) fn job_strip(ui: &mut egui::Ui, flags: &[Flag]) -> Vec<FlagId> {
    let mut dismissed = Vec::new();
    for flag in flags {
        let row = RowFlag {
            severity: flag.severity,
            unchecked: flag.unchecked,
            title: flag.title.clone(),
            lines: flag.detail.0.clone(),
        };
        let mut gone = false;
        slot_with(ui, Some(&row), Some(&mut gone));
        if gone {
            dismissed.push(flag.id);
        }
    }
    dismissed
}
