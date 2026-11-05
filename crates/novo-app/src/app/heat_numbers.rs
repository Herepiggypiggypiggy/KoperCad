//! Numbers and plots from the solved air in the 3D Heat strip: the plot
//! along a line, the volume of air above a temperature, a histogram and the
//! pinned probes. The sums live in `novo_heatview::probes`; this file words
//! them and draws them. Find them in Cabinet > 3D > Heat strip: Probe, then
//! Pin and Line in the probe row, and the Numbers menu.

use eframe::egui;
use novo_cfd::CfdSnapshot;
use novo_heatview::probes::{self, AirNumbers, Profile};

use super::heat_probe::{self, ProbeState};

/// What the Numbers menu and the pinned points mean; behind the strip's "?".
pub(crate) const NUMBERS_HELP: &str = "Numbers: the volume of air (not devices or walls) hotter than the level, from the solved cells. A coarse grid counts a cell as a whole. Pinned probes: turn on Probe, click the air, press Pin. Model results, not measurements.";
/// What the plot along a line shows; behind the strip's "?" and the plot's.
pub(crate) const LINE_HELP: &str = "Plot along line: temperature along the line you clicked, in the solved air. Grey stretches run through a device or wall and show its own temperature. The dashed line is the ambient air. Click the air twice: the start and the end of the line.";

/// The plot window's panel name.
const PLOT_ID: &str = "heat_line_plot";
/// Samples along a plotted line.
const LINE_SAMPLES: usize = 120;
/// Bars in the histogram.
const BINS: usize = 8;

/// The temperature the air figures count above: the user's pick, otherwise
/// ambient plus 10 K.
pub(crate) fn level(probe: &ProbeState, ambient: f64) -> f64 {
    probe.level_c.unwrap_or(ambient + 10.0)
}

/// The profile of the line the user set, when both ends are set.
pub(crate) fn profile(probe: &ProbeState, snap: &CfdSnapshot) -> Option<Profile> {
    let (a, b) = probe.line_ends()?;
    probes::line_profile(snap, a, b, LINE_SAMPLES)
}

/// Litres, or cubic metres once it is large.
pub(crate) fn litres(l: f64) -> String {
    if l >= 1000.0 {
        format!("{:.2} m³", l / 1000.0)
    } else if l >= 10.0 {
        format!("{l:.0} L")
    } else {
        format!("{l:.2} L")
    }
}

/// The figures of the air in words.
pub(crate) fn air_text(n: &AirNumbers, level_c: f64) -> String {
    format!(
        "Air {}, of which {} ({:.0} %) is hotter than {level_c:.0} °C. Mean {:.1} °C, from {:.1} to {:.1} °C.",
        litres(n.air_l),
        litres(n.above_l),
        n.above_share() * 100.0,
        n.mean_c,
        n.min_c,
        n.max_c
    )
}

/// One line for a pinned point.
pub(crate) fn pin_text(n: usize, snap: &CfdSnapshot, ambient: f64, at: [f64; 3]) -> String {
    let r = heat_probe::read(snap, ambient, at);
    format!(
        "{}: {:.1} °C ({:+.1} K), {:.2} m/s at x {:.0}, y {:.0}, z {:.0} mm",
        n + 1,
        r.temperature_c,
        r.rise_k,
        r.speed,
        at[0],
        at[1],
        at[2]
    )
}

/// The Numbers menu of the Heat strip.
pub(crate) fn menu(ui: &mut egui::Ui, probe: &mut ProbeState, snap: &CfdSnapshot, ambient: f64) {
    ui.menu_button("Numbers", |ui| {
        ui.horizontal(|ui| {
            ui.label("Air hotter than");
            let mut lv = level(probe, ambient);
            if ui
                .add(
                    egui::DragValue::new(&mut lv)
                        .speed(0.5)
                        .range(-40.0..=300.0)
                        .suffix(" °C"),
                )
                .changed()
            {
                probe.level_c = Some(lv);
            }
        });
        let lv = level(probe, ambient);
        match probes::air_numbers(snap, lv, None) {
            Some(n) => {
                ui.label(egui::RichText::new(air_text(&n, lv)).small());
                histogram(ui, snap, lv);
            }
            None => {
                ui.label("This result holds no air.");
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("Pinned probes").small());
        if probe.pins.is_empty() {
            ui.label(egui::RichText::new("None").small());
        }
        let mut drop = None;
        for (n, at) in probe.pins.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(pin_text(n, snap, ambient, *at)).small());
                if ui.small_button("✕").on_hover_text("Remove").clicked() {
                    drop = Some(n);
                }
            });
        }
        if let Some(n) = drop {
            probe.unpin(n);
        }
    });
}

fn histogram(ui: &mut egui::Ui, snap: &CfdSnapshot, level_c: f64) {
    let bins = probes::air_histogram(snap, BINS);
    let top = bins.iter().map(|b| b.2).fold(0.0, f64::max);
    if top <= 0.0 {
        return;
    }
    let pal = crate::theme::ui_palette();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(240.0, 54.0), egui::Sense::hover());
    let p = ui.painter_at(rect);
    let w = rect.width() / bins.len() as f32;
    for (n, (lo, hi, l)) in bins.iter().enumerate() {
        let h = (l / top) as f32 * (rect.height() - 4.0);
        let x = rect.left() + w * n as f32;
        let hot = (lo + hi) / 2.0 > level_c;
        let color = if hot { pal.warning } else { pal.info };
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(x + 1.0, rect.bottom() - h),
                egui::pos2(x + w - 1.0, rect.bottom()),
            ),
            1.0,
            color,
        );
    }
    resp.on_hover_text("Air volume at each temperature, coldest to hottest air; bars above the level are the warm colour");
}

/// The plot of the line, a card that can be docked like the other tool
/// panes. Returns false when the user closed it.
pub(crate) fn plot_window(
    ctx: &egui::Context,
    profile: Option<&Profile>,
    ambient: f64,
    estimate: bool,
) -> bool {
    // Docked at the bottom unless the person floated it: it never covers the
    // 3D view on its own.
    if super::panel_dock::slot_of(ctx, PLOT_ID).is_none()
        && !super::panel_dock::was_floated(ctx, PLOT_ID)
    {
        super::panel_dock::set(ctx, PLOT_ID, Some(super::panel_dock::Slot::Bottom));
    }
    super::panel::Panel::new(
        PLOT_ID,
        "Plot along line",
        ctx.content_rect(),
        super::panel::Side::BottomLeft,
        370.0,
    )
    .closable()
    .dockable()
    .fit()
    .show(ctx, |ui| {
        super::window_help::add(ui.ctx(), LINE_HELP);
        if estimate {
            ui.label(
                egui::RichText::new(super::heat::ESTIMATE_TAG)
                    .small()
                    .color(crate::theme::ui_palette().warning),
            );
            super::window_help::add(ui.ctx(), novo_layout::spec::ESTIMATE_NOTICE);
        }
        let Some(p) = profile else {
            ui.label("Pick two points");
            return;
        };
        plot(ui, p, ambient);
        if let Some(s) = p.stats() {
            ui.label(
                egui::RichText::new(format!(
                    "Along {:.0} mm: {:.1} to {:.1} °C, mean {:.1} °C, fastest air {:.2} m/s{}",
                    p.length_mm(),
                    s.min_c,
                    s.max_c,
                    s.mean_c,
                    s.max_speed,
                    if s.solid_share > 0.0 {
                        format!(", {:.0} % inside solids", s.solid_share * 100.0)
                    } else {
                        String::new()
                    }
                ))
                .small(),
            );
        } else {
            ui.label("The whole line lies inside solids.");
        }
        if ui
            .small_button("Copy as CSV")
            .on_hover_text("Distance, temperature and speed per point, for a spreadsheet")
            .clicked()
        {
            ui.ctx().copy_text(p.csv());
        }
    });
    !super::panel::take_closed(ctx, PLOT_ID)
}

fn plot(ui: &mut egui::Ui, p: &Profile, ambient: f64) {
    let pal = crate::theme::ui_palette();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(340.0, 130.0), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 2.0, pal.sunken);
    let lo = p
        .samples
        .iter()
        .map(|s| s.temperature_c)
        .fold(ambient, f64::min);
    let hi = p
        .samples
        .iter()
        .map(|s| s.temperature_c)
        .fold(ambient + 1.0, f64::max);
    let inner = rect.shrink2(egui::vec2(34.0, 12.0));
    let len = p.length_mm().max(1e-9);
    let at = |along: f64, t: f64| {
        egui::pos2(
            inner.left() + (along / len) as f32 * inner.width(),
            inner.bottom() - ((t - lo) / (hi - lo)) as f32 * inner.height(),
        )
    };
    // Ambient, dashed.
    let y = at(0.0, ambient).y;
    let mut x = inner.left();
    while x < inner.right() {
        painter.line_segment(
            [
                egui::pos2(x, y),
                egui::pos2((x + 4.0).min(inner.right()), y),
            ],
            egui::Stroke::new(1.0, pal.text_secondary),
        );
        x += 8.0;
    }
    for w in p.samples.windows(2) {
        let solid = w[0].solid || w[1].solid;
        let color = if solid {
            pal.text_secondary
        } else {
            pal.warning
        };
        painter.line_segment(
            [
                at(w[0].along_mm, w[0].temperature_c),
                at(w[1].along_mm, w[1].temperature_c),
            ],
            egui::Stroke::new(if solid { 1.0 } else { 2.0 }, color),
        );
    }
    let font = egui::FontId::proportional(10.0);
    for (t, a) in [
        (hi, egui::Align2::RIGHT_TOP),
        (lo, egui::Align2::RIGHT_BOTTOM),
    ] {
        painter.text(
            egui::pos2(inner.left() - 4.0, at(0.0, t).y),
            a,
            format!("{t:.0}"),
            font.clone(),
            pal.text_secondary,
        );
    }
    painter.text(
        egui::pos2(inner.right(), inner.bottom() + 2.0),
        egui::Align2::RIGHT_TOP,
        format!("{:.0} mm", p.length_mm()),
        font,
        pal.text_secondary,
    );
    // Hover: the value under the pointer.
    if let Some(pos) = resp.hover_pos() {
        let f = ((pos.x - inner.left()) / inner.width()).clamp(0.0, 1.0) as f64;
        let n = ((f * (p.samples.len() - 1) as f64).round() as usize).min(p.samples.len() - 1);
        let s = p.samples[n];
        painter.circle_filled(at(s.along_mm, s.temperature_c), 3.0, pal.text_primary);
        resp.on_hover_text(format!(
            "{:.0} mm: {:.1} °C, {:.2} m/s{}",
            s.along_mm,
            s.temperature_c,
            s.speed,
            if s.solid { " (inside a solid)" } else { "" }
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::snapshot;

    #[test]
    fn the_level_defaults_to_ten_kelvin_over_ambient() {
        let mut p = ProbeState::default();
        assert_eq!(level(&p, 20.0), 30.0);
        p.level_c = Some(25.0);
        assert_eq!(level(&p, 20.0), 25.0);
    }

    #[test]
    fn volumes_are_worded_in_litres_and_cubic_metres() {
        assert_eq!(litres(0.472), "0.47 L");
        assert_eq!(litres(48.0), "48 L");
        assert_eq!(litres(1500.0), "1.50 m³");
    }

    #[test]
    fn the_air_text_gives_share_and_range() {
        let snap = snapshot();
        let n = probes::air_numbers(&snap, 25.0, None).unwrap();
        let t = air_text(&n, 25.0);
        assert!(t.contains("hotter than 25 °C") && t.contains("51 %"), "{t}");
    }

    #[test]
    fn a_pin_line_reads_the_field_at_the_point() {
        let snap = snapshot();
        let t = pin_text(0, &snap, 20.0, [60.0, 30.0, 40.0]);
        assert!(t.starts_with("1: 26.0 °C (+6.0 K), 1.00 m/s"), "{t}");
    }

    #[test]
    fn the_profile_needs_both_ends() {
        let snap = snapshot();
        let mut p = ProbeState::default();
        assert!(profile(&p, &snap).is_none());
        p.line = (Some([30.0, 50.0, 40.0]), Some([90.0, 50.0, 40.0]));
        let pr = profile(&p, &snap).unwrap();
        assert_eq!(pr.samples.len(), LINE_SAMPLES);
    }
}
