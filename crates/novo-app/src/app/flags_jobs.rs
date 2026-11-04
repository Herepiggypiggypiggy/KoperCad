//! Failed jobs as flags (docs/design/problem-triangles.md section 7, lane T12).
//!
//! An export, import, save or report that fails used to raise a red or yellow
//! toast that faded away. It now becomes a flag on the job: a triangle in the
//! top bar next to the Issues pill, a popover with the plain words and the
//! actions Retry, Open folder, Copy details and Dismiss. The flag stays until
//! the same job succeeds or it is dismissed. The message still goes to the log.

use std::path::{Path, PathBuf};

use eframe::egui;
use novo_flags::{Action, Anchor, Detail, FlagDraft, FlagId, Life, Severity, Source};

use super::{Level, NovoApp};

/// The `Action::Open` key of "Open folder".
pub(super) const OPEN_FOLDER: &str = "job.folder";
/// The `Action::Open` key of "Dismiss".
pub(super) const DISMISS: &str = "job.dismiss";

/// What the app remembers about a failed job besides its flag.
#[derive(Clone, Debug, Default)]
pub(super) struct JobInfo {
    /// The folder to open: where the file should have gone.
    pub(super) folder: Option<PathBuf>,
    /// The command that runs the job again.
    pub(super) retry: Option<&'static str>,
}

/// The command that repeats a job, by job key. Jobs that ask for a file name
/// open their dialog again; that is the point of Retry.
pub(super) fn retry_command(job: &str) -> Option<&'static str> {
    Some(match job {
        "export.pdf" => "export.pdf",
        "export.dxf" => "export.dxf_page",
        "import.dxf" => "file.import_dxf_background",
        "export.step" => "export.step",
        "export.aml" => "export.aml",
        "export.eplan" => "export.eplan",
        "export.spreadsheet" => "export.sheets",
        "export.workpack" => "export.workpack",
        "export.bench_packet" => "export.bench_packet",
        "export.bench_cut" => "export.bench_cut",
        "export.ferrules" => "export.ferrules",
        "export.labels" => "export.labels",
        "export.change_report" => "export.change_report",
        "export.variant_pdf" => "export.variant_pdf",
        _ => return None,
    })
}

/// QA pictures of refusals the app cannot reach in a short script: in a
/// debug build, `KOPERCAD_QA_FAIL=export.change_report,manual.open` makes
/// those jobs refuse with a fixed sentence. Release builds ignore it.
pub(super) fn qa_forced_error(job: &str) -> Option<String> {
    if !cfg!(debug_assertions) {
        return None;
    }
    let keys = std::env::var("KOPERCAD_QA_FAIL").ok()?;
    keys.split(',')
        .any(|k| k.trim() == job)
        .then(|| format!("Forced for the QA picture ({job})"))
}

impl NovoApp {
    /// A job failed: the flag on the job, and the plain words in the log.
    /// `Info` is not a problem and stays a plain message.
    pub(super) fn job_fail(
        &mut self,
        job: &'static str,
        title: &'static str,
        level: Level,
        text: impl Into<String>,
    ) {
        self.job_fail_in(job, title, level, text, None);
    }

    /// [`job_fail`](Self::job_fail) for a job that writes `path`: the popover
    /// offers to open the folder it was going to.
    pub(super) fn job_fail_at(
        &mut self,
        job: &'static str,
        title: &'static str,
        level: Level,
        text: impl Into<String>,
        path: &Path,
    ) {
        let folder = if path.extension().is_some() {
            path.parent().map(Path::to_owned)
        } else {
            Some(path.to_owned())
        };
        self.job_fail_in(job, title, level, text, folder);
    }

    /// [`job_fail`](Self::job_fail) with the severity as a flag colour: red or yellow.
    pub(super) fn job_fail_red(
        &mut self,
        job: &str,
        title: &str,
        red: bool,
        text: impl Into<String>,
    ) {
        let level = if red { Level::Error } else { Level::Warning };
        self.job_fail_in(job, title, level, text, None);
    }

    pub(super) fn job_fail_in(
        &mut self,
        job: &str,
        title: &str,
        level: Level,
        text: impl Into<String>,
        folder: Option<PathBuf>,
    ) {
        let text = text.into();
        let severity = match level {
            Level::Error => Severity::Red,
            Level::Warning => Severity::Yellow,
            Level::Info => {
                self.message(level, text);
                return;
            }
        };
        // The flag is the visible part; the log keeps the message. Backticks in
        // an engine message are code quoting: plain quotes here.
        let words = self.log_only(level, text).replace('`', "'");
        let retry = retry_command(job);
        let mut actions = Vec::new();
        if retry.is_some() || self.wb.flags.retry.contains_key(&job_flag_id(job)) {
            actions.push(Action::Retry);
        }
        if folder.is_some() {
            actions.push(Action::Open(OPEN_FOLDER.into()));
        }
        actions.extend([Action::Copy, Action::Open(DISMISS.into())]);
        let flags = &mut self.wb.flags;
        let id = flags.store.set(FlagDraft {
            severity,
            unchecked: false,
            source: Source::Job,
            rule: job.to_owned(),
            anchors: vec![Anchor::Job(job.to_owned())],
            title: title.to_owned(),
            detail: Detail::new(words.lines().map(str::to_owned)),
            actions,
            waiver_key: None,
            life: Life::Until,
        });
        flags.jobs.insert(id, JobInfo { folder, retry });
        flags.job_order.retain(|i| *i != id);
        flags.job_order.push(id);
    }

    /// A project was closed or replaced: its failed jobs go.
    pub(super) fn forget_jobs(&mut self) {
        let flags = &mut self.wb.flags;
        for id in std::mem::take(&mut flags.job_order) {
            flags.store.clear(id);
        }
        flags.jobs.clear();
        flags.retry.clear();
        if flags
            .open
            .is_some_and(|(id, _)| !flags.store.get(id).is_some())
        {
            flags.open = None;
        }
    }

    /// The job worked: its flag goes.
    pub(super) fn job_ok(&mut self, job: &'static str) {
        let id = job_flag_id(job);
        let flags = &mut self.wb.flags;
        flags.retry.remove(&id);
        if flags.store.clear(id) {
            flags.jobs.remove(&id);
            flags.job_order.retain(|i| *i != id);
            if flags.open.is_some_and(|(open, _)| open == id) {
                flags.open = None;
            }
        }
    }

    /// The job flags still up, newest last.
    pub(super) fn job_flags(&self) -> Vec<novo_flags::Flag> {
        self.wb
            .flags
            .job_order
            .iter()
            .filter_map(|id| self.wb.flags.store.get(*id).cloned())
            .collect()
    }

    /// The triangle in the top bar: the worst failed job, a count when there
    /// are several. A click opens the popover of the newest one.
    pub(super) fn job_pill(&mut self, ui: &mut egui::Ui) {
        let flags = self.job_flags();
        let Some(newest) = flags.last() else {
            self.wb.job_pill_rect = egui::Rect::NOTHING;
            return;
        };
        let red = flags.iter().any(|f| f.severity == Severity::Red);
        let pal = crate::theme::ui_palette();
        let galley = (flags.len() > 1).then(|| {
            ui.painter().layout_no_wrap(
                flags.len().to_string(),
                egui::TextStyle::Small.resolve(ui.style()),
                if red { pal.error } else { pal.warning },
            )
        });
        let width = 26.0 + galley.as_ref().map_or(0.0, |g| g.size().x + 2.0);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click());
        let label = if flags.len() == 1 {
            newest.accessible_name()
        } else {
            format!(
                "{} failed jobs, newest: {}",
                flags.len(),
                newest.accessible_name()
            )
        };
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        if ui.is_rect_visible(rect) {
            novo_theme::widgets::flag_triangle(
                ui.painter(),
                egui::pos2(rect.left() + 13.0, rect.center().y),
                14.0,
                red,
                false,
                pal,
            );
            if let Some(g) = galley {
                let at = egui::pos2(rect.left() + 24.0, rect.center().y - g.size().y / 2.0);
                ui.painter().galley(at, g, pal.text_secondary);
            }
        }
        let hover = flags
            .iter()
            .rev()
            .map(|f| f.accessible_name())
            .collect::<Vec<_>>()
            .join("\n");
        let response = response.on_hover_text(hover);
        self.wb.job_pill_rect = response.rect;
        self.anchor_response("job.pill", &response, "Failed jobs");
        if response.clicked() {
            let at = ui
                .ctx()
                .pointer_interact_pos()
                .unwrap_or(rect.left_bottom());
            self.wb.flags.open = Some((newest.id, at));
        }
    }

    /// Runs the popover action of a job flag. `true` when it was one.
    pub(super) fn run_job_action(&mut self, id: FlagId, action: &Action) -> bool {
        let Some(info) = self.wb.flags.jobs.get(&id).cloned() else {
            return false;
        };
        match action {
            Action::Retry => {
                if let Some(again) = self.wb.flags.retry.remove(&id) {
                    self.wb.flags.store.clear(id);
                    self.wb.flags.jobs.remove(&id);
                    self.wb.flags.job_order.retain(|i| *i != id);
                    self.retry_job(again);
                } else if let Some(command) = info.retry {
                    self.wb.flags.store.clear(id);
                    self.wb.flags.jobs.remove(&id);
                    self.wb.flags.job_order.retain(|i| *i != id);
                    self.wb.queued.push(command);
                }
            }
            Action::Open(key) if key == OPEN_FOLDER => {
                if let Some(folder) = info.folder
                    && let Err(e) = super::parts_bulk::open_folder(&folder)
                {
                    self.message(Level::Error, format!("Could not open the folder: {e}"));
                }
            }
            Action::Open(key) if key == DISMISS => {
                self.wb.flags.retry.remove(&id);
                self.wb.flags.store.clear(id);
                self.wb.flags.jobs.remove(&id);
                self.wb.flags.job_order.retain(|i| *i != id);
            }
            _ => return false,
        }
        true
    }
}

/// The id of the flag a job key makes.
pub(super) fn job_flag_id(job: &str) -> FlagId {
    novo_flags::Flag::from_draft(FlagDraft {
        severity: Severity::Red,
        unchecked: false,
        source: Source::Job,
        rule: job.to_owned(),
        anchors: vec![Anchor::Job(job.to_owned())],
        title: String::new(),
        detail: Detail::default(),
        actions: Vec::new(),
        waiver_key: None,
        life: Life::Until,
    })
    .id
}
