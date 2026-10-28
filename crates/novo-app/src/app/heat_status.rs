//! One simulation state for every view. The Cabinet strip, the 3D Heat panel
//! and the 3D status chip all read [`HeatState::sim_status`] and press Cancel
//! through [`HeatState::cancel`], so the person never has to go back to the
//! Cabinet view to see whether the heat run is going, done, out of date or
//! has failed.

use super::heat::{self, HeatState};
use super::heat_progress;

/// Where the heat simulation stands.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum SimStatus {
    /// No result and nothing running.
    NotRun,
    /// A run is going (or ending, after Cancel).
    Running {
        /// 0 to 1 over the whole run; `None` while the solver is still setting
        /// up and has no iteration to measure by.
        fraction: Option<f32>,
        /// "Coarse 3D · it 120 · 14 s", or the setup stage ("meshing · 4 s").
        label: String,
        /// Cancel was pressed; the run is ending.
        stopping: bool,
    },
    /// A result of the current cabinet.
    Done {
        /// "Coarse 3D · not validated" and the like.
        label: String,
        converged: bool,
    },
    /// A result of an earlier cabinet: something changed since.
    OutOfDate,
    /// The last run (or the start of one) failed; the sentence says why.
    Failed(String),
}

impl SimStatus {
    /// One short line for a chip.
    pub(super) fn text(&self) -> String {
        match self {
            SimStatus::NotRun => "Heat: not simulated".into(),
            SimStatus::Running {
                label,
                stopping: true,
                ..
            } => format!("Heat: stopping · {label}"),
            SimStatus::Running { label, .. } => format!("Heat: {label}"),
            SimStatus::Done { label, converged } => {
                if *converged {
                    format!("Heat: {label}")
                } else {
                    format!("Heat: {label} (not converged)")
                }
            }
            SimStatus::OutOfDate => "Heat: out of date, simulate again".into(),
            SimStatus::Failed(why) => format!("Heat: {why}"),
        }
    }
}

impl HeatState {
    /// The state of the heat simulation as the views show it.
    pub(super) fn sim_status(&self) -> SimStatus {
        if self.running() {
            let (fraction, label) = match (&self.progress, &self.run) {
                (Some(p), Some(r)) => {
                    let secs = r.elapsed_s();
                    (
                        Some(heat_progress::fraction(&r.levels, p)),
                        heat_progress::label(p, secs),
                    )
                }
                (None, Some(r)) => (
                    None,
                    heat_progress::preparing_label(r.phase(), r.elapsed_s()),
                ),
                _ => (None, "heat-up".into()),
            };
            return SimStatus::Running {
                fraction,
                label,
                stopping: self.stopping(),
            };
        }
        if let Some(why) = self.failure() {
            return SimStatus::Failed(why.to_owned());
        }
        match self.solution() {
            Some(_) if self.live.is_stale() => SimStatus::OutOfDate,
            Some(sol) => {
                let p = &sol.snapshot.progress;
                SimStatus::Done {
                    label: heat::status_label(p, sol.snapshot.accuracy.validated),
                    converged: p.converged,
                }
            }
            None => SimStatus::NotRun,
        }
    }

    /// The last notice when it is a failure (not "Stopped", which keeps its
    /// result).
    fn failure(&self) -> Option<&str> {
        self.notice.as_deref().filter(|n| {
            n.starts_with("Cannot simulate")
                || n.starts_with("The simulation")
                || n.starts_with("The heat-up")
                || n.starts_with("Open an enclosure")
        })
    }

    /// Whether a Cancel button should be live: a run goes and Cancel was not pressed yet.
    pub(super) fn can_cancel(&self) -> bool {
        self.running() && !self.stopping()
    }
}
