//! The Simulate progress bar and the residual sparkline.

use eframe::egui;
use novo_cfd::{Fidelity, Progress};
use novo_theme::tokens as t;

/// Residual the bar counts as "done" for a level that has not flagged
/// convergence yet.
const TARGET: f64 = 1e-4;
/// Most residuals kept for the sparkline.
const KEEP: usize = 400;

/// Adds the worst residual of `p` to the sparkline history (log10).
pub(super) fn push(history: &mut Vec<f64>, p: &Progress) {
    let r = p.residuals.max();
    if r.is_finite() && r > 0.0 {
        history.push(r.log10());
        if history.len() > KEEP {
            history.remove(0);
        }
    }
}

/// How far the whole run is, 0 to 1: finished levels count in full, the
/// running one by how far its residual has fallen towards [`TARGET`].
pub(super) fn fraction(levels: &[Fidelity], p: &Progress) -> f32 {
    let n = levels.len().max(1) as f64;
    let at = levels.iter().position(|f| *f == p.fidelity).unwrap_or(0) as f64;
    let within = if p.converged {
        1.0
    } else {
        let r = p.residuals.max();
        if r.is_finite() && r > 0.0 {
            (-r.log10() / -TARGET.log10()).clamp(0.0, 1.0)
        } else {
            0.0
        }
    };
    (((at + within) / n) as f32).clamp(0.0, 1.0)
}

pub(super) fn label(p: &Progress, secs: f64) -> String {
    format!("{} · it {} · {secs:.0} s", p.fidelity.label(), p.iteration)
}

/// Points of the sparkline in a `w` by `h` box: oldest left, lowest
/// residual at the bottom.
pub(super) fn spark_points(history: &[f64], w: f32, h: f32) -> Vec<(f32, f32)> {
    if history.len() < 2 {
        return Vec::new();
    }
    let hi = history.iter().cloned().fold(f64::MIN, f64::max);
    let lo = history.iter().cloned().fold(f64::MAX, f64::min);
    let span = (hi - lo).max(1e-9);
    let last = (history.len() - 1) as f32;
    history
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let y = ((v - lo) / span) as f32;
            (w * i as f32 / last, h * (1.0 - y))
        })
        .collect()
}

pub(super) fn sparkline(ui: &mut egui::Ui, history: &[f64]) {
    let size = egui::vec2(90.0, 16.0);
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
    let pts: Vec<egui::Pos2> = spark_points(history, size.x, size.y)
        .into_iter()
        .map(|(x, y)| rect.min + egui::vec2(x, y))
        .collect();
    if pts.len() >= 2 {
        ui.painter().add(egui::Shape::line(
            pts,
            egui::Stroke::new(1.2, t::TEXT_MUTED),
        ));
    }
    resp.on_hover_text("Residual of the running level (log scale); falling means converging");
}
