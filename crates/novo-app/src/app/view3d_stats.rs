//! Frame statistics of the 3D view: the adapter, how long a frame takes
//! while the view turns, what the renderer draws and what a scene rebuild
//! costs. It is there to measure the 3D view on a real machine without a
//! build tool: switch it on, let the view spin, copy the lines.

use std::collections::VecDeque;
use std::fmt::Write;

use eframe::egui;
use novo_view3d::widget::{RenderStats, View3d, percentile, render_stats};

/// Frame intervals kept for the percentiles.
const WINDOW: usize = 240;
/// A gap longer than this is the view standing still, not a slow frame.
const IDLE_GAP_MS: f32 = 250.0;
/// Margin from the view's corner, in points.
const MARGIN: f32 = 12.0;
/// How fast the view turns while measuring: a full turn in 12 seconds.
const SPIN_RAD_PER_S: f32 = std::f32::consts::TAU / 12.0;

/// What the last scene rebuild cost, in milliseconds.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(super) struct Build {
    /// Building the scene from the project.
    pub(super) scene_ms: f32,
    /// Routing the wires, when the rebuild did it.
    pub(super) routing_ms: Option<f32>,
}

/// The statistics panel's state.
#[derive(Debug, Default)]
pub(crate) struct Stats {
    pub(super) shown: bool,
    /// The view turns by itself, so every frame is a moving one.
    pub(super) spin: bool,
    intervals: VecDeque<f32>,
    pub(super) build: Build,
}

impl Stats {
    /// Shows or hides the panel; hiding stops the spin and forgets the
    /// samples.
    pub(super) fn toggle(&mut self) {
        self.shown = !self.shown;
        if !self.shown {
            self.spin = false;
        }
        self.intervals.clear();
    }

    /// One frame took `ms` since the one before. Gaps from the view standing
    /// still are not frames and are dropped.
    pub(super) fn push_interval(&mut self, ms: f32) {
        if !(0.0..IDLE_GAP_MS).contains(&ms) {
            return;
        }
        if self.intervals.len() == WINDOW {
            self.intervals.pop_front();
        }
        self.intervals.push_back(ms);
    }

    pub(super) fn samples(&self) -> Vec<f32> {
        self.intervals.iter().copied().collect()
    }

    /// Runs the panel for this frame: records the interval, turns the view
    /// when asked to and draws the panel in the bottom left of `view_rect`.
    /// Call it before the view is shown.
    pub(super) fn frame(&mut self, ctx: &egui::Context, view_rect: egui::Rect, view: &mut View3d) {
        if !self.shown {
            return;
        }
        self.push_interval(ctx.input(|i| i.unstable_dt) * 1000.0);
        if self.spin {
            view.spin(ctx, SPIN_RAD_PER_S);
        }
        let text = report(render_stats().as_ref(), &self.samples(), self.build);
        egui::Area::new(egui::Id::new("view3d_stats"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(view_rect.left_bottom() + egui::vec2(MARGIN, -MARGIN))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(egui::RichText::new(&text).monospace().small());
                    ui.horizontal(|ui| {
                        let label = if self.spin {
                            "✔ Spin the view"
                        } else {
                            "Spin the view"
                        };
                        if ui
                            .selectable_label(self.spin, label)
                            .on_hover_text("Turn the view steadily so every frame is a moving one")
                            .clicked()
                        {
                            self.spin = !self.spin;
                            self.intervals.clear();
                        }
                        if ui
                            .button("Copy")
                            .on_hover_text("Copy these lines to paste into a message")
                            .clicked()
                        {
                            ctx.copy_text(text.clone());
                        }
                        if ui.button("Close").clicked() {
                            self.toggle();
                        }
                    });
                });
            });
    }
}

/// The lines of the panel, also what Copy puts on the clipboard.
pub(super) fn report(stats: Option<&RenderStats>, intervals: &[f32], build: Build) -> String {
    let mut out = String::from("KoperCAD 3D frame statistics\n");
    match stats {
        Some(s) => {
            let _ = write!(
                out,
                "GPU: {} ({}, {:?})",
                s.adapter, s.backend, s.adapter_tier
            );
            if s.tier != s.adapter_tier {
                let _ = write!(
                    out,
                    ", quality stepped down to {:?} after slow frames",
                    s.tier
                );
            }
            out.push('\n');
        }
        None => out.push_str("GPU: none (the 3D view is not drawn)\n"),
    }
    if intervals.is_empty() {
        out.push_str("Frame interval: no samples yet, turn Spin on\n");
    } else {
        let p50 = percentile(intervals, 0.5);
        let p95 = percentile(intervals, 0.95);
        let _ = writeln!(
            out,
            "Frame interval: {p50:.1} ms median, {p95:.1} ms 95th percentile, {:.0} fps ({} frames)",
            1000.0 / p50.max(0.01),
            intervals.len()
        );
    }
    if let Some(s) = stats {
        let _ = writeln!(
            out,
            "Renderer CPU: {:.2} ms median, {:.2} ms 95th percentile",
            s.cpu_p50_ms, s.cpu_p95_ms
        );
        let _ = writeln!(
            out,
            "Drawn: {} items, {} draw calls, {} triangles, {} x {} px, MSAA {}",
            s.visible_items, s.draw_calls, s.triangles, s.rendered_px.0, s.rendered_px.1, s.msaa
        );
    }
    let routing = build
        .routing_ms
        .map_or_else(String::new, |r| format!(", routing {r:.1} ms"));
    let _ = write!(out, "Scene build: {:.1} ms{routing}", build.scene_ms);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_view3d::render::Tier;

    fn stats() -> RenderStats {
        RenderStats {
            adapter: "Test GPU".into(),
            backend: "Vulkan".into(),
            tier: Tier::Discrete,
            adapter_tier: Tier::Discrete,
            renders: 10,
            visible_items: 1016,
            draw_calls: 11,
            triangles: 12_700,
            rendered_px: (1280, 720),
            msaa: 1,
            cpu_p50_ms: 0.18,
            cpu_p95_ms: 0.4,
        }
    }

    #[test]
    fn the_report_names_the_adapter_the_frame_times_and_the_build() {
        let intervals: Vec<f32> = (0..100).map(|i| 16.0 + (i % 5) as f32).collect();
        let build = Build {
            scene_ms: 3.2,
            routing_ms: Some(41.0),
        };
        let text = report(Some(&stats()), &intervals, build);
        assert!(text.contains("GPU: Test GPU (Vulkan, Discrete)"), "{text}");
        assert!(text.contains("18.0 ms median"), "{text}");
        assert!(text.contains("56 fps (100 frames)"), "{text}");
        assert!(text.contains("Renderer CPU: 0.18 ms median"), "{text}");
        assert!(
            text.contains("1016 items, 11 draw calls, 12700 triangles"),
            "{text}"
        );
        assert!(
            text.ends_with("Scene build: 3.2 ms, routing 41.0 ms"),
            "{text}"
        );
    }

    #[test]
    fn the_report_says_when_the_governor_stepped_the_quality_down() {
        let stepped = RenderStats {
            tier: Tier::Integrated,
            ..stats()
        };
        let text = report(Some(&stepped), &[16.0], Build::default());
        assert!(
            text.contains(
                "GPU: Test GPU (Vulkan, Discrete), quality stepped down to Integrated after slow frames"
            ),
            "{text}"
        );
    }

    #[test]
    fn without_samples_or_an_adapter_the_report_says_so() {
        let text = report(None, &[], Build::default());
        assert!(text.contains("GPU: none"), "{text}");
        assert!(text.contains("no samples yet"), "{text}");
        assert!(text.ends_with("Scene build: 0.0 ms"), "{text}");
    }

    #[test]
    fn idle_gaps_are_not_frames_and_the_window_is_bounded() {
        let mut s = Stats::default();
        s.push_interval(16.0);
        s.push_interval(900.0); // the view stood still
        s.push_interval(-1.0);
        assert_eq!(s.samples(), vec![16.0]);
        for _ in 0..1000 {
            s.push_interval(10.0);
        }
        assert_eq!(s.samples().len(), WINDOW);
    }

    #[test]
    fn hiding_the_panel_stops_the_spin_and_forgets_the_samples() {
        let mut s = Stats::default();
        s.toggle();
        s.spin = true;
        s.push_interval(10.0);
        s.toggle();
        assert!(!s.shown && !s.spin);
        assert!(s.samples().is_empty());
    }
}
