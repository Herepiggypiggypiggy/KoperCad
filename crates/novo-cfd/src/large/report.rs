//! What a reduced run reports, and the convergence verdict of any run.

use crate::result::CfdSnapshot;

/// How the reduction treated the cabinet, for the notes and the dialog.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LargeReport {
    /// Cells the full scenario would have needed at this level's settings.
    pub full_cells: usize,
    /// Cells of the reduced grid actually solved.
    pub reduced_cells: usize,
    pub target_cells: usize,
    /// The plane pitch chosen, mm (0 = lumping alone was enough).
    pub pitch_mm: f64,
    pub devices: usize,
    /// Devices that kept their own box.
    pub exact_devices: usize,
    /// Cold devices merged into blocks of two or more.
    pub lumped_devices: usize,
    pub blocks: usize,
    /// Heat in lumped blocks, W, and in the cabinet, W.
    pub lumped_heat_w: f64,
    pub total_heat_w: f64,
    /// Ducts and rails widened to the pitch (porous smearing).
    pub smeared_obstacles: usize,
    /// Even the reduced grid is over the budget.
    pub over_budget: bool,
    /// Largest move of an edge of a device that keeps its own box, mm.
    pub hot_shift_mm: f64,
}

impl LargeReport {
    /// One-line summary for the progress card.
    pub fn summary(&self) -> String {
        format!(
            "{REDUCED_SUMMARY_PREFIX}{} devices as {} exact and {} blocks, {} cells (full grid {})",
            self.devices, self.exact_devices, self.blocks, self.reduced_cells, self.full_cells
        )
    }

    /// The honest notes a snapshot of a reduced run carries, the one-line
    /// summary first (see [`reduced_summary`]).
    pub fn notes(&self) -> Vec<String> {
        let mut v = vec![self.summary()];
        v.push(format!(
            "The full grid of this cabinet would have {} cells (budget {}), \
             so {} cold devices ({:.0} of {:.0} W) were lumped into {} row blocks, ducts and rails \
             were widened to {:.0} mm porous strips with the volume kept, and the edges of the \
             blocks were snapped to a {:.0} mm pitch. {} devices with real losses keep their own box; \
             their edges moved by at most {:.0} mm to meet a neighbour's. The solved grid has {} cells.",
            self.full_cells,
            self.target_cells,
            self.lumped_devices,
            self.lumped_heat_w,
            self.total_heat_w,
            self.blocks,
            self.pitch_mm,
            self.pitch_mm,
            self.exact_devices,
            self.hot_shift_mm,
            self.reduced_cells,
        ));
        v.push(
            "Devices inside a block show the air temperature of the block and a housing \
             temperature estimated from their own loss and the block's film coefficient; the \
             bands are wide. Read them as a screening, not as a surface temperature: a lumped device is shown \
             as marginal at most, never as over its limit, because its air temperature is the \
             block's. Run a finer level to resolve it."
                .into(),
        );
        if self.over_budget {
            v.push(
                "Even the reduced grid is over the cell budget: bodies alone need more cells than \
                 the level allows, so the run is slower than the level promises."
                    .into(),
            );
        }
        v
    }
}

/// Start of the one-line note every reduced run carries.
pub const REDUCED_SUMMARY_PREFIX: &str = "Reduced model: ";

/// The one-line summary of a reduced run, or `None` for a run of the full
/// scenario. The long explanation follows it in the snapshot's notes.
pub fn reduced_summary(s: &CfdSnapshot) -> Option<&str> {
    s.notes
        .iter()
        .map(String::as_str)
        .find(|n| n.starts_with(REDUCED_SUMMARY_PREFIX))
}

/// Whether a result can be trusted as a steady state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// Residuals, heat balance and drift are within the limits.
    Converged,
    /// Stopped by its budget and still moving, or the balance is off.
    NotConverged,
    /// The numbers show it has hardly started.
    Early,
}

/// The convergence of a snapshot in plain terms.
#[derive(Debug, Clone, PartialEq)]
pub struct Convergence {
    pub verdict: Verdict,
    pub iterations: usize,
    pub residual: f64,
    /// Heat in versus out, share of the heat in.
    pub imbalance: f64,
    /// Change of the monitored device temperatures over the last window, K.
    pub drift_k: f64,
    /// One short line for a narrow panel.
    pub headline: String,
    /// Sentences for the dialog, most important first.
    pub lines: Vec<String>,
}

/// Limit on the heat imbalance of a trusted steady result.
pub const IMBALANCE_LIMIT: f64 = 0.05;
/// Limit on the temperature drift over the last window, K.
pub const DRIFT_LIMIT_K: f64 = 0.5;

/// Reads the convergence out of a snapshot. Pure; works for any run.
pub fn convergence(s: &CfdSnapshot) -> Convergence {
    let p = &s.progress;
    let residual = p.residuals.max();
    let imbalance = p.energy_balance_rel.abs();
    let drift = p.monitor_drift_k.abs();
    let early = p.iteration < 20;
    let verdict = if p.converged && imbalance <= IMBALANCE_LIMIT && drift <= DRIFT_LIMIT_K {
        Verdict::Converged
    } else if early {
        Verdict::Early
    } else {
        Verdict::NotConverged
    };
    let mut lines = Vec::new();
    lines.push(match verdict {
        Verdict::Converged => format!(
            "Converged after {} iterations: heat in and out agree within {:.1} % and the \
             monitored temperatures moved {:.2} K over the last window.",
            p.iteration,
            100.0 * imbalance,
            drift
        ),
        Verdict::Early => format!(
            "Just started ({} iterations): nothing to read yet.",
            p.iteration
        ),
        Verdict::NotConverged => format!(
            "Not converged at {} iterations: heat balance {:.1} % off, temperatures still \
             moving {:.2} K per window, residual {:.1e}. They are not final.",
            p.iteration,
            100.0 * imbalance,
            drift,
            residual
        ),
    });
    if p.converged && imbalance > IMBALANCE_LIMIT {
        lines.push(format!(
            "The iteration reports steady but the heat balance is off by {:.1} % (limit {:.0} %): \
             do not trust the absolute temperatures.",
            100.0 * imbalance,
            100.0 * IMBALANCE_LIMIT
        ));
    }
    if p.converged && drift > DRIFT_LIMIT_K {
        lines.push(format!(
            "Reported steady, yet the monitored temperatures moved {drift:.2} K over the last \
             window (limit {DRIFT_LIMIT_K} K)."
        ));
    }
    let headline = match verdict {
        Verdict::Converged if lines.len() > 1 => {
            format!(
                "Steady, but check: balance {:.1} % off, drift {drift:.2} K",
                100.0 * imbalance
            )
        }
        Verdict::Converged => format!("Converged at {} iterations", p.iteration),
        Verdict::Early => "Just started".to_string(),
        Verdict::NotConverged => format!(
            "Not converged: balance {:.1} % off, drift {drift:.2} K",
            100.0 * imbalance
        ),
    };
    Convergence {
        verdict,
        headline,
        iterations: p.iteration,
        residual,
        imbalance,
        drift_k: drift,
        lines,
    }
}
