//! Compare two heat results: the difference map B minus A, the device table
//! and the one-line verdict (spec `docs/design/thermal-visuals.md` section 7;
//! the Δ mode). Engine in `novo-heatview::compare`, drawing here is thin.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Shape, Vec2};
use novo_heatview::Scalar;
use novo_heatview::cfd::{self, Window};
use novo_heatview::compare::{self, DeviceRow};
use novo_theme::heat;
use novo_theme::tokens as t;
use novo_thermal::Status;

use super::heat_run::Solution;

/// B minus A on the plate, with the numbers the legend quotes.
#[derive(Debug, Clone)]
pub(super) struct Diff {
    pub field: Scalar,
    /// Symmetric range of the map, K.
    pub extent: f32,
    /// Devices in both results, biggest change first.
    pub rows: Vec<DeviceRow>,
    pub verdict: String,
}

/// The difference map of `b` against `a`; `b_plate` is B's temperature on
/// the plate (the cache already has it). `None` when A cannot be sliced.
pub(super) fn diff(a: &Solution, b: &Solution, b_plate: &Scalar, cell_mm: f64) -> Option<Diff> {
    let [x, y, w, h] = a.plate;
    let a_plate = cfd::as_2d(&a.snapshot, Window { u0: x, v0: y, w, h }, cell_mm)?.temperature;
    let field = compare::difference(&a_plate, b_plate);
    // The map is on A's grid; B-A on B's grid would be the same picture.
    let extent = compare::symmetric_extent(&field);
    let mut rows: Vec<DeviceRow> = b
        .snapshot
        .devices
        .iter()
        .filter_map(|d| {
            let before = a.snapshot.devices.iter().find(|o| o.id == d.id)?;
            Some(DeviceRow {
                id: d.id,
                a: before.surface_c,
                b: d.surface_c,
            })
        })
        .collect();
    compare::device_table(&mut rows);
    let peak = |s: &Solution| {
        s.snapshot
            .devices
            .iter()
            .map(|d| d.surface_c)
            .fold(f64::NEG_INFINITY, f64::max)
    };
    let over = |s: &Solution| {
        s.snapshot
            .devices
            .iter()
            .filter(|d| d.status == Status::Over)
            .count()
    };
    let verdict = if rows.is_empty() {
        "No device is in both results.".to_owned()
    } else {
        compare::verdict_line(peak(a), peak(b), over(a), over(b))
    };
    Some(Diff {
        field,
        extent,
        rows,
        verdict,
    })
}

/// Colour of a difference value on the diverging ramp, as RGB.
pub(super) fn colour(value: f32, extent: f32) -> [u8; 3] {
    let t = if value.is_nan() {
        0.5
    } else {
        0.5 + 0.5 * (value / extent).clamp(-1.0, 1.0)
    };
    heat::diverging(t)
}

/// The legend of the Δ mode: ramp, verdict and the biggest changes.
pub(super) fn legend(ctx: &egui::Context, rect: Rect, d: &Diff, tag_of: &dyn Fn(u64) -> String) {
    egui::Area::new(egui::Id::new("heat_legend"))
        .fixed_pos(rect.right_top() + Vec2::new(-196.0, 12.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(160.0);
                ui.label(
                    egui::RichText::new("CHANGE B − A")
                        .small()
                        .strong()
                        .color(t::TEXT_MUTED),
                );
                let height = 120.0;
                let (bar, _) =
                    ui.allocate_exact_size(Vec2::new(14.0, height), egui::Sense::hover());
                let mut mesh = egui::Mesh::default();
                let steps = 24;
                for s in 0..=steps {
                    let f = s as f32 / steps as f32;
                    let c = heat::diverging(f);
                    let c = egui::Color32::from_rgb(c[0], c[1], c[2]);
                    let y = bar.bottom() - f * height;
                    mesh.colored_vertex(Pos2::new(bar.left(), y), c);
                    mesh.colored_vertex(Pos2::new(bar.right(), y), c);
                }
                for s in 0..steps as u32 {
                    let i = s * 2;
                    mesh.add_triangle(i, i + 1, i + 2);
                    mesh.add_triangle(i + 1, i + 3, i + 2);
                }
                let p = ui.painter();
                p.add(Shape::mesh(mesh));
                let font = FontId::monospace(t::FONT_SMALL);
                for (f, sign) in [(1.0, "+"), (0.5, ""), (0.0, "−")] {
                    let v = (f - 0.5) * 2.0 * d.extent;
                    p.text(
                        Pos2::new(bar.right() + 8.0, bar.bottom() - f * height),
                        Align2::LEFT_CENTER,
                        format!("{sign}{:.1} K", v.abs()),
                        font.clone(),
                        t::TEXT_MUTED,
                    );
                }
                ui.add_space(4.0);
                ui.label(egui::RichText::new(&d.verdict).small().color(t::TEXT));
                for r in d.rows.iter().take(5) {
                    ui.label(
                        egui::RichText::new(format!(
                            "{}  {:.0} → {:.0} °C  ({:+.1} K)",
                            tag_of(r.id),
                            r.a,
                            r.b,
                            r.delta()
                        ))
                        .small()
                        .monospace()
                        .color(t::TEXT_MUTED),
                    );
                }
            });
        });
}
