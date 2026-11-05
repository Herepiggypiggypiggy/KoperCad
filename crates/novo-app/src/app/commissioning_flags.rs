//! Commissioning as problem triangles (docs/design/problem-triangles.md, lane T13).
//!
//! Where the user finds it: the Commissioning workspace. Punch items that are
//! Blocking draw a red triangle and Major ones a yellow triangle, on the item's row and
//! on the wire, terminal or device it is about; a failed check draws a red one and an open
//! redline a yellow one. Minor items draw none. A failed export, import or photo copy puts a
//! triangle on a job strip at the top of the panel, with Retry. A triangle opens the popover
//! of the T0b foundation (Locate, Open, Retry, Copy).
//!
//! These are findings of a walk at the machine and the notes someone typed. Nothing here is
//! a compliance sign-off: closing every flag does not certify the machine.

use eframe::egui;
use novo_commissioning::{
    CheckKind, CheckState, CommissioningStore, PunchItem, PunchState, RedlineState, Severity,
    Target, TargetInfo,
};
use novo_core::AnyId;
use novo_flags::{
    Action, Anchor, Detail, Flag, FlagDraft, FlagId, Life, Severity as FlagSeverity, Source,
};

use super::flag_rows::flag_slot;
use super::{Level, NovoApp};

/// The panel key the popover's Open action brings up (the Punch tab).
pub(super) const OPEN_PUNCH: &str = "commissioning.punch";
/// Every job key of this lane starts with this.
pub(super) const JOB_PREFIX: &str = "commissioning.";

/// The anchor of a punch item's row.
pub(super) fn item_anchor(id: u64) -> Anchor {
    Anchor::Control(format!("commissioning.punch.{id}"))
}

/// The anchor of a check row (the target and the check).
pub(super) fn check_anchor(target: Target, check: CheckKind) -> Anchor {
    Anchor::Control(format!(
        "commissioning.check.{}.{check:?}",
        target_key(target)
    ))
}

fn target_key(target: Target) -> String {
    match target.any_id() {
        AnyId::Wire(i) => format!("wire:{}", i.raw()),
        AnyId::Terminal(i) => format!("terminal:{}", i.raw()),
        AnyId::Device(i) => format!("device:{}", i.raw()),
        other => format!("other:{}", other.raw()),
    }
}

/// The anchor of the drawn object a target is.
pub(super) fn target_anchor(target: Target) -> Anchor {
    match target {
        Target::Wire(id) => Anchor::Wire(id.raw().to_string()),
        Target::Terminal(id) => Anchor::Terminal(id.raw().to_string()),
        Target::Device(id) => Anchor::Device(id.raw().to_string()),
    }
}

/// The target a flag anchor stands for, when it is one of the three kinds.
pub(super) fn anchor_target(anchor: &Anchor) -> Option<Target> {
    use novo_core::{DeviceId, TerminalId, WireId};
    Some(match anchor {
        Anchor::Wire(raw) => Target::Wire(WireId(raw.parse().ok()?)),
        Anchor::Terminal(raw) => Target::Terminal(TerminalId(raw.parse().ok()?)),
        Anchor::Device(raw) => Target::Device(DeviceId(raw.parse().ok()?)),
        _ => return None,
    })
}

fn label_of(all: &[TargetInfo], target: Target) -> String {
    all.iter()
        .find(|i| i.target == target)
        .map_or_else(String::new, |i| i.label.clone())
}

/// What a punch item of this severity says: `None` for Minor (no triangle).
pub(super) fn punch_severity(s: Severity) -> Option<FlagSeverity> {
    match s {
        Severity::Blocking => Some(FlagSeverity::Red),
        Severity::Major => Some(FlagSeverity::Yellow),
        Severity::Minor => None,
    }
}

fn punch_title(item: &PunchItem) -> String {
    match item.severity {
        Severity::Blocking => "Blocking punch item".into(),
        _ => "Major punch item".into(),
    }
}

fn base_actions(has_target: bool) -> Vec<Action> {
    let mut actions = Vec::new();
    if has_target {
        actions.push(Action::Locate);
    }
    actions.extend([Action::Open(OPEN_PUNCH.into()), Action::Copy]);
    actions
}

/// Every flag of a commissioning store: open Blocking and Major punch items, failed checks
/// nobody has a punch item for, and open redlines. `all` gives the labels.
pub(super) fn drafts(store: &CommissioningStore, all: &[TargetInfo]) -> Vec<FlagDraft> {
    let mut out = Vec::new();
    let open: Vec<&PunchItem> = store
        .punch_items()
        .filter(|p| !p.state.is_closed())
        .collect();
    for item in &open {
        // An item a failed check opened is red while that check still fails: the machine
        // failed a test. Otherwise the item's own severity decides (Minor: no triangle).
        let failed_check = item
            .target
            .zip(item.check)
            .filter(|&(t, c)| store.state(t, c) == CheckState::Failed);
        let Some(severity) = failed_check
            .map(|_| FlagSeverity::Red)
            .or_else(|| punch_severity(item.severity))
        else {
            continue;
        };
        let mut anchors = vec![item_anchor(item.id)];
        if let (Some(target), Some(check)) = (item.target, item.check) {
            anchors.push(check_anchor(target, check));
        }
        let mut lines = vec![item.title.clone()];
        if let Some(target) = item.target {
            anchors.push(target_anchor(target));
            let label = label_of(all, target);
            if !label.is_empty() {
                lines.push(label);
            }
        }
        if item.state == PunchState::Fixed {
            lines.push("Fixed, waits for a re-test".into());
        } else if !item.raised_by.trim().is_empty() {
            lines.push(format!("Raised by {}", item.raised_by));
        }
        out.push(FlagDraft {
            severity,
            unchecked: false,
            source: Source::Commissioning,
            rule: format!("punch.{}", item.id),
            anchors,
            title: failed_check.map_or_else(
                || punch_title(item),
                |(_, c)| format!("{} check failed", c.label()),
            ),
            detail: Detail::new(lines),
            actions: base_actions(item.target.is_some()),
            waiver_key: None,
            life: Life::Live,
        });
    }
    for record in store.records().filter(|r| r.state == CheckState::Failed) {
        // The failure already opened a punch item: one triangle for it, not two.
        if open
            .iter()
            .any(|p| p.target == Some(record.target) && p.check == Some(record.check))
        {
            continue;
        }
        let mut lines = vec![format!(
            "{} {}",
            label_of(all, record.target),
            record.check.label().to_lowercase()
        )];
        if !record.note.trim().is_empty() {
            lines.push(record.note.trim().to_owned());
        }
        if !record.value.trim().is_empty() {
            lines.push(format!("Measured {}", record.value.trim()));
        }
        out.push(FlagDraft {
            severity: FlagSeverity::Red,
            unchecked: false,
            source: Source::Commissioning,
            rule: format!("check.{}", check_anchor(record.target, record.check).key()),
            anchors: vec![
                check_anchor(record.target, record.check),
                target_anchor(record.target),
            ],
            title: format!("{} check failed", record.check.label()),
            detail: Detail::new(lines),
            actions: base_actions(true),
            waiver_key: None,
            life: Life::Live,
        });
    }
    for r in store.redlines().filter(|r| r.state == RedlineState::Open) {
        out.push(FlagDraft {
            severity: FlagSeverity::Yellow,
            unchecked: false,
            source: Source::Commissioning,
            rule: format!("redline.{}", r.id),
            anchors: vec![
                Anchor::Control(format!("commissioning.redline.{}", r.id)),
                target_anchor(r.target),
            ],
            title: "Machine differs from drawing".into(),
            detail: Detail::new([r.summary(), format!("Marked by {}", r.by.trim())]),
            actions: base_actions(true),
            waiver_key: None,
            life: Life::Live,
        });
    }
    out
}

/// Whether a job flag belongs to this lane.
pub(super) fn is_commissioning_job(flag: &Flag) -> bool {
    flag.source == Source::Job && job_key(flag).is_some_and(|k| k.starts_with(JOB_PREFIX))
}

/// The flag on `anchor`, if any.
pub(super) fn at<'a>(flags: &'a [Flag], anchor: &Anchor) -> Option<&'a Flag> {
    flags.iter().find(|f| f.anchors.contains(anchor))
}

/// What the panel keeps about flags between frames.
#[derive(Default)]
pub(super) struct PanelFlags {
    /// The flags of the mode and its jobs, as of this frame.
    pub(super) flags: Vec<Flag>,
    /// A triangle that was clicked this frame, and where.
    pub(super) clicked: Option<(FlagId, egui::Pos2)>,
}

impl PanelFlags {
    /// Draws the triangle of `anchor` when there is one; a click asks for its popover.
    pub(super) fn slot(&mut self, ui: &mut egui::Ui, anchor: &Anchor) -> bool {
        let Some(flag) = at(&self.flags, anchor) else {
            return false;
        };
        let response = flag_slot(ui, flag);
        if response.clicked() {
            self.clicked = Some((flag.id, response.rect.center()));
        }
        true
    }
}

fn job_key(flag: &Flag) -> Option<&str> {
    flag.anchors.iter().find_map(|a| match a {
        Anchor::Job(k) => Some(k.as_str()),
        _ => None,
    })
}

/// Job keys.
pub(super) mod job {
    pub(in crate::app) const FIELD_EXPORT: &str = "commissioning.field_export";
    pub(in crate::app) const FIELD_IMPORT: &str = "commissioning.field_import";
    pub(in crate::app) const PUNCH_PDF: &str = "commissioning.punch_pdf";
    pub(in crate::app) const REPORT: &str = "commissioning.report";
    pub(in crate::app) const REDLINES: &str = "commissioning.redlines_apply";
    pub(in crate::app) const PHOTO: &str = "commissioning.photo";
    pub(in crate::app) const PHOTO_UNPACK: &str = "commissioning.photo_unpack";
    pub(in crate::app) const TEST_SHEET: &str = "commissioning.test_sheet";
    pub(in crate::app) const LOAD: &str = "commissioning.load";
}

impl NovoApp {
    /// A commissioning job failed with `err`: a job flag on #1910's path (the top bar pill, Dismiss,
    /// Copy), plus Retry when the job can be run again from here.
    pub(super) fn job_error(
        &mut self,
        job: &'static str,
        title: &'static str,
        err: &(impl std::fmt::Display + ?Sized),
        retry: bool,
    ) {
        self.job_fail(job, title, Level::Error, err.to_string());
        if retry {
            self.add_retry(job);
        }
    }

    /// A commissioning job went on but not fully (a redline refused): yellow.
    pub(super) fn job_doubtful(
        &mut self,
        job: &'static str,
        title: &'static str,
        detail: Vec<String>,
    ) {
        self.job_fail(job, title, Level::Warning, detail.join("\n"));
    }

    /// Puts Retry first in the actions of the job's flag.
    fn add_retry(&mut self, job: &'static str) {
        let id = super::flags_jobs::job_flag_id(job);
        let Some(f) = self.wb.flags.store.get(id).cloned() else {
            return;
        };
        let mut actions = f.actions;
        actions.insert(0, Action::Retry);
        self.wb.flags.store.set(FlagDraft {
            severity: f.severity,
            unchecked: f.unchecked,
            source: f.source,
            rule: job.to_owned(),
            anchors: f.anchors,
            title: f.title,
            detail: f.detail,
            actions,
            waiver_key: None,
            life: f.life,
        });
    }

    /// Brings the store up to the commissioning data and copies what the panel draws.
    pub(super) fn sync_commissioning_flags(&mut self) {
        let choice = self.machine_variant();
        let generation = self.machine_generation();
        let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
        else {
            return;
        };
        let project: &novo_core::Project = choice.as_deref().unwrap_or(doc.project());
        mode.generation = generation;
        let index = mode.index(project);
        let built = drafts(&mode.store, index.all());
        self.wb
            .flags
            .store
            .replace_source(&Source::Commissioning, built);
        self.sync_commissioning_panel_flags();
    }

    /// Copies the flags the panel draws out of the store.
    fn sync_commissioning_panel_flags(&mut self) {
        let list: Vec<Flag> = self
            .wb
            .flags
            .store
            .list(false)
            .into_iter()
            .filter(|f| f.source == Source::Commissioning)
            .cloned()
            .collect();
        if let Some(mode) = self.wb.commissioning.as_mut() {
            mode.panel_flags.flags = list;
        }
    }

    /// The mode is off: its flags go with it.
    pub(super) fn drop_commissioning_flags(&mut self) {
        self.wb
            .flags
            .store
            .replace_source(&Source::Commissioning, Vec::new());
    }

    /// Hands a clicked triangle to the popover.
    pub(super) fn open_clicked_commissioning_flag(&mut self) {
        if let Some(mode) = self.wb.commissioning.as_mut()
            && let Some(opened) = mode.panel_flags.clicked.take()
        {
            self.wb.flags.open = Some(opened);
        }
    }

    /// The popover's Locate for a commissioning flag: select the target and jump to it.
    pub(super) fn locate_commissioning_flag(&mut self, flag: &Flag) {
        let Some(target) = flag.anchors.iter().find_map(anchor_target) else {
            return;
        };
        let choice = self.machine_variant();
        let current = self.current_page;
        let place = {
            let (Some(mode), Some(doc)) = (self.wb.commissioning.as_mut(), self.backend.document())
            else {
                return;
            };
            let project: &novo_core::Project = choice.as_deref().unwrap_or(doc.project());
            mode.selected = Some(target);
            mode.tab = super::commissioning::Tab::Checks;
            let index = mode.index(project);
            index
                .all()
                .iter()
                .find(|i| i.target == target)
                .and_then(|info| super::commissioning::place_of(project, info, current))
        };
        if let Some(place) = place {
            self.jump(place, true);
        }
    }

    /// The popover's Open: the Punch tab.
    pub(super) fn open_commissioning_panel(&mut self, panel: &str) {
        if panel == OPEN_PUNCH
            && let Some(mode) = self.wb.commissioning.as_mut()
        {
            mode.tab = super::commissioning::Tab::Punch;
        }
    }

    /// The popover's Retry: the job again (the file dialog opens again where there is one).
    pub(super) fn retry_commissioning_job(&mut self, flag: &Flag) {
        let Some(key) = job_key(flag).map(str::to_owned) else {
            return;
        };
        match key.as_str() {
            job::FIELD_EXPORT => self.export_field_results(),
            job::FIELD_IMPORT => self.import_field_results(),
            job::PUNCH_PDF => self.export_punch_list_pdf(),
            job::REPORT => {
                self.export_commissioning_pdf();
            }
            job::REDLINES => self.apply_redlines(false),
            job::TEST_SHEET => {
                self.export_test_sheet();
            }
            _ => {}
        }
    }
}
