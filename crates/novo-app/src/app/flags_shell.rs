//! Problem triangles for the shell: settings, add-ons, learning, languages,
//! startup helpers (docs/design/problem-triangles.md, lane T14).
//!
//! Two kinds of flag live here.
//!
//! * A **job flag** (`Anchor::Job`) replaces a red or yellow toast: "Could not
//!   open the log folder". It stays in the store until the same job succeeds or
//!   the person dismisses it, and shows as a triangle chip at the right of the
//!   status bar.
//! * A **control flag** (`Anchor::Control`) sits on the control or window that
//!   caused it. [`NovoApp::flag_on`] draws it where the control is.
//!
//! A state that is recomputed every frame (an add-on that stopped, a language
//! file with gaps) uses [`inline_flag`] instead: the same triangle, drawn where
//! the sentence used to be, with the detail in a popup. No store, no state.

use eframe::egui;
use novo_flags::{Action, Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity, Source};

use super::NovoApp;

/// Commands a shell flag may offer as its Fix or Open button, with the label.
pub(super) const COMMANDS: &[(&str, &str)] = &[
    ("help.open_log_folder", "Open log folder"),
    ("help.keymap_file", "Open keymap file"),
    ("app.preferences", "Open Preferences"),
    ("addons.manage", "Open Add-ons"),
];

/// The static command id and button label for a flag action, when it is one of ours.
pub(super) fn shell_command(action: &Action) -> Option<(&'static str, &'static str)> {
    let name = match action {
        Action::Fix(c) | Action::Open(c) => c.as_str(),
        _ => return None,
    };
    COMMANDS.iter().copied().find(|(c, _)| *c == name)
}

fn draft(
    anchor: Anchor,
    rule: &str,
    red: bool,
    title: &str,
    detail: &str,
    fix: Option<&str>,
) -> FlagDraft {
    let mut actions = Vec::new();
    if let Some(cmd) = fix {
        actions.push(Action::Fix(cmd.to_owned()));
    }
    actions.extend([Action::Copy, Action::Explain]);
    FlagDraft {
        severity: if red { Severity::Red } else { Severity::Yellow },
        unchecked: false,
        source: Source::Shell,
        rule: rule.to_owned(),
        anchors: vec![anchor],
        title: title.to_owned(),
        detail: Detail::new(
            detail
                .lines()
                .map(|l| crate::app::error_words::plain(l).to_owned()),
        ),
        actions,
        waiver_key: None,
        life: Life::Until,
    }
}

fn id_of(anchor: &Anchor, rule: &str) -> FlagId {
    Flag::from_draft(draft(anchor.clone(), rule, false, "", "", None)).id
}

impl NovoApp {
    /// A job failed or was doubtful: a triangle on the job chip instead of a
    /// toast. `rule` names the job (`diag.log_folder`); raising it again with
    /// new words replaces the old one.
    pub(super) fn job_flag(
        &mut self,
        rule: &str,
        red: bool,
        title: &str,
        detail: impl AsRef<str>,
        fix: Option<&str>,
    ) {
        self.shell_flag(Anchor::Job(rule.to_owned()), rule, red, title, detail, fix);
    }

    /// The same job worked this time: its flag goes.
    pub(super) fn job_ok(&mut self, rule: &str) {
        self.shell_clear(&Anchor::Job(rule.to_owned()), rule);
    }

    /// A triangle on the control or window `key` (see [`NovoApp::flag_on`]).
    pub(super) fn control_flag(
        &mut self,
        key: &str,
        rule: &str,
        red: bool,
        title: &str,
        detail: impl AsRef<str>,
        fix: Option<&str>,
    ) {
        let anchor = Anchor::Control(key.to_owned());
        self.shell_flag(anchor, rule, red, title, detail, fix);
    }

    fn shell_flag(
        &mut self,
        anchor: Anchor,
        rule: &str,
        red: bool,
        title: &str,
        detail: impl AsRef<str>,
        fix: Option<&str>,
    ) {
        let detail = detail.as_ref();
        if red {
            log::error!("{title}: {detail}");
        } else {
            log::warn!("{title}: {detail}");
        }
        // The old words of the same job go first, so the flag shows the new ones.
        self.shell_clear(&anchor, rule);
        self.wb
            .flags
            .store
            .set(draft(anchor, rule, red, title, detail, fix));
    }

    fn shell_clear(&mut self, anchor: &Anchor, rule: &str) {
        self.wb.flags.store.clear(id_of(anchor, rule));
    }

    /// The shell flags that are not on the drawing: worst severity, count and
    /// the newest one (for the status bar chip).
    pub(super) fn shell_chip(&self) -> Option<(Severity, usize, FlagId)> {
        let mut found: Vec<&Flag> = self
            .wb
            .flags
            .store
            .list(false)
            .into_iter()
            .filter(|f| f.source == Source::Shell)
            .collect();
        if found.is_empty() {
            return None;
        }
        found.sort_by_key(|f| (f.severity == Severity::Yellow, f.id.0));
        let worst = found[0].severity;
        Some((worst, found.len(), found[0].id))
    }

    /// The flags on control `key`, ready to draw with [`flag_row`].
    pub(super) fn control_flags(&self, key: &str) -> Vec<ControlFlag> {
        self.wb
            .flags
            .store
            .at(&Anchor::Control(key.to_owned()), false)
            .into_iter()
            .map(|f| ControlFlag {
                id: f.id,
                red: f.severity == Severity::Red,
                name: f.accessible_name(),
            })
            .collect()
    }

    /// The window or control `key` is gone: its flags go with it.
    pub(super) fn control_clear_all(&mut self, key: &str) {
        let ids: Vec<FlagId> = self
            .wb
            .flags
            .store
            .at(&Anchor::Control(key.to_owned()), true)
            .into_iter()
            .map(|f| f.id)
            .collect();
        for id in ids {
            self.wb.flags.store.clear(id);
        }
    }
}

/// A flag on a control, as the dialog draws it.
pub(super) struct ControlFlag {
    id: FlagId,
    red: bool,
    name: String,
}

/// Draws the triangles of `flags` in a row at the cursor. A click returns the
/// flag to open and where.
pub(super) fn flag_row(ui: &mut egui::Ui, flags: &[ControlFlag]) -> Option<(FlagId, egui::Pos2)> {
    let mut asked = None;
    if flags.is_empty() {
        return None;
    }
    ui.horizontal(|ui| {
        for f in flags {
            let r = triangle_button(ui, f.red, false, &f.name);
            if r.clicked() {
                asked = Some((f.id, r.rect.left_bottom()));
            }
        }
        if let Some(first) = flags.first() {
            ui.label(
                first
                    .name
                    .split_once(": ")
                    .map_or(first.name.as_str(), |x| x.1),
            );
        }
    });
    asked
}

/// A 14 pt triangle with a 24 pt hit area and an accessible name.
pub(super) fn triangle_button(
    ui: &mut egui::Ui,
    red: bool,
    dashed: bool,
    name: &str,
) -> egui::Response {
    triangle_button_sized(ui, red, dashed, name, 24.0)
}

/// [`triangle_button`] with a smaller hit area, for the status bar.
fn triangle_button_sized(
    ui: &mut egui::Ui,
    red: bool,
    dashed: bool,
    name: &str,
    hit: f32,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(hit, hit), egui::Sense::click());
    novo_theme::widgets::flag_triangle(
        ui.painter(),
        rect.center(),
        14.0,
        red,
        dashed,
        novo_theme::Palette::of(ui),
    );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, name));
    response
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .on_hover_text(name)
}

/// A recomputed state as a triangle plus a short title; a click opens a popup
/// with the detail. Replaces a banner or a coloured sentence. Returns the
/// response of the triangle row.
pub(super) fn inline_flag(
    ui: &mut egui::Ui,
    red: bool,
    dashed: bool,
    title: &str,
    detail: &str,
) -> egui::Response {
    let level = if red {
        "Error"
    } else if dashed {
        "Not checked"
    } else {
        "Warning"
    };
    let name = format!("{level}: {title}");
    let response = ui
        .horizontal(|ui| {
            let t = triangle_button(ui, red, dashed, &name);
            ui.label(title);
            t
        })
        .inner;
    egui::Popup::menu(&response).show(|ui| {
        ui.set_max_width(320.0);
        ui.label(egui::RichText::new(title).strong());
        for line in detail.lines().filter(|l| !l.trim().is_empty()) {
            ui.label(line);
        }
        if ui.button("Copy").clicked() {
            ui.ctx().copy_text(format!("{title}\n{detail}"));
            ui.close();
        }
    });
    response
}

/// The status bar chip for the shell flags: a triangle, the count when there
/// are several. Returns the flag to open when it was clicked.
pub(super) fn chip(
    ui: &mut egui::Ui,
    chip: (Severity, usize, FlagId),
) -> Option<(FlagId, egui::Pos2)> {
    let (severity, count, id) = chip;
    let name = if count == 1 {
        "A job did not work: click for details".to_owned()
    } else {
        format!("{count} things did not work: click for details")
    };
    let mut asked = None;
    ui.horizontal(|ui| {
        let r = triangle_button_sized(ui, severity == Severity::Red, false, &name, 18.0);
        if count > 1 {
            ui.weak(count.to_string());
        }
        if r.clicked() {
            asked = Some((id, r.rect.left_top() - egui::vec2(0.0, 120.0)));
        }
    });
    asked
}
