//! The Mechanical tab of the Calculations window (Check > Calculations):
//! busbar short-circuit force with support spacing, and deflection of a
//! DIN rail or a mounting plate under device load. The numbers come from
//! [`novo_checks::calc::mechanical`]; this file holds the inputs and draws
//! the results.
//!
//! The inputs live in the window, not in the project: a busbar and the
//! weights on a rail are not in the data model yet. Every result carries the
//! "model" label (Help > About the numbers).

use std::fmt::Write as _;

use eframe::egui;
use novo_checks::calc::mechanical::{
    ALUMINIUM, BarMaterial, BeamInput, BeamResult, BusFault, BusbarInput, BusbarResult, COPPER,
    Hold, Load, PlateInput, PlateResult, STEEL_E_PA, Spans, Verdict, check_beam, check_busbar,
    check_plate, top_hat_35,
};

use super::model_labels::{Model, badge};

/// What the tab remembers between frames.
#[derive(Debug, Clone)]
pub(super) struct MechState {
    bus: BusbarInput,
    /// Rail profile: 0 is TH35×7.5, 1 is TH35×15.
    rail_profile: usize,
    /// Load lies in the plate plane (devices hanging on a horizontal rail).
    in_plate: bool,
    rail_span_mm: f64,
    rail_hold: Hold,
    /// Weight of the devices on the rail, kg, spread over the span.
    rail_kg: f64,
    /// A heavy device at the middle, kg.
    rail_point_kg: f64,
    rail_limit: f64,
    plate: PlateInput,
    /// Weight spread over the plate, kg.
    plate_kg: f64,
    /// Weight at the plate centre, kg.
    plate_centre_kg: f64,
}

impl Default for MechState {
    fn default() -> Self {
        MechState {
            bus: BusbarInput::typical(),
            rail_profile: 0,
            in_plate: true,
            rail_span_mm: 400.0,
            rail_hold: Hold::Simple,
            rail_kg: 2.0,
            rail_point_kg: 0.0,
            rail_limit: 200.0,
            plate: PlateInput {
                a_mm: 600.0,
                b_mm: 400.0,
                thickness_mm: 2.0,
                e_pa: STEEL_E_PA,
                poisson: 0.3,
                spread_n: 0.0,
                centre_n: 0.0,
                limit_ratio: 200.0,
            },
            plate_kg: 8.0,
            plate_centre_kg: 0.0,
        }
    }
}

const G: f64 = 9.806_65;

impl MechState {
    fn rail_input(&self) -> BeamInput {
        let (h, t) = if self.rail_profile == 0 {
            (7.5, 1.0)
        } else {
            (15.0, 1.5)
        };
        let s = top_hat_35(h, t);
        let mut loads = Vec::new();
        if self.rail_kg > 0.0 {
            loads.push(Load::Spread(self.rail_kg * G));
        }
        if self.rail_point_kg > 0.0 {
            loads.push(Load::Point {
                pos_mm: self.rail_span_mm / 2.0,
                newtons: self.rail_point_kg * G,
            });
        }
        BeamInput {
            span_mm: self.rail_span_mm,
            hold: self.rail_hold,
            e_pa: STEEL_E_PA,
            inertia_m4: if self.in_plate {
                s.i_in_plate
            } else {
                s.i_out_of_plate
            },
            loads,
            limit_ratio: self.rail_limit,
        }
    }

    fn plate_input(&self) -> PlateInput {
        PlateInput {
            spread_n: self.plate_kg * G,
            centre_n: self.plate_centre_kg * G,
            ..self.plate
        }
    }
}

fn verdict_label(v: Verdict) -> (&'static str, egui::Color32) {
    let p = crate::theme::ui_palette();
    match v {
        Verdict::Pass => ("Pass", p.success),
        Verdict::Fail => ("Fail", p.error),
        Verdict::Note => ("Check", p.warning),
    }
}

fn drag(ui: &mut egui::Ui, value: &mut f64, unit: &str, min: f64, max: f64, speed: f64) {
    ui.add(
        egui::DragValue::new(value)
            .suffix(format!(" {unit}"))
            .range(min..=max)
            .speed(speed),
    );
}

/// A limit given as span over a number: shown as "1/200".
fn drag_ratio(ui: &mut egui::Ui, value: &mut f64) {
    ui.add(
        egui::DragValue::new(value)
            .prefix("1/")
            .range(50.0..=1000.0)
            .speed(5.0),
    );
}

fn row(ui: &mut egui::Ui, label: &str, help: &str, add: impl FnOnce(&mut egui::Ui)) {
    super::dialog_ui::explained(ui, label, help);
    add(ui);
    ui.end_row();
}

fn result_line(ui: &mut egui::Ui, v: Verdict, text: String) {
    let (label, color) = verdict_label(v);
    ui.horizontal_wrapped(|ui| {
        ui.colored_label(color, label);
        ui.label(text);
    });
}

fn fmt(value: f64) -> String {
    if value >= 100.0 {
        format!("{value:.0}")
    } else if value >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    }
}

fn busbar_lines(r: &BusbarResult, i: &BusbarInput) -> Vec<(Verdict, String)> {
    let mpa = |pa: f64| fmt(pa / 1.0e6);
    let mut out = vec![
        (
            Verdict::Note,
            format!(
                "Peak current {} kA (peak factor {:.2}); force {} N per metre between the conductors",
                fmt(r.peak_a / 1000.0),
                r.kappa,
                fmt(r.force_per_m)
            ),
        ),
        (
            r.bending,
            format!(
                "Bending stress {} MPa of {} MPa allowed ({:.0} %). Longest span that holds: {} mm",
                mpa(r.stress_pa),
                mpa(r.allowed_pa),
                r.utilisation * 100.0,
                fmt(r.max_span_mm)
            ),
        ),
        (
            r.support,
            match i.support_rating_n {
                Some(rated) => format!(
                    "Force on one support {} N, rated {} N",
                    fmt(r.support_force_n),
                    fmt(rated)
                ),
                None => format!(
                    "Force on one support {} N. Compare it with the cantilever rating of the insulator",
                    fmt(r.support_force_n)
                ),
            },
        ),
    ];
    out.push((
        r.resonance,
        if r.resonance_risk {
            format!(
                "The span swings at {} Hz, near {} Hz or {} Hz: the force can build up. Shorten the span or run a dynamic check",
                fmt(r.natural_hz),
                fmt(i.frequency_hz),
                fmt(2.0 * i.frequency_hz)
            )
        } else {
            format!(
                "The span swings at {} Hz, away from {} Hz and {} Hz",
                fmt(r.natural_hz),
                fmt(i.frequency_hz),
                fmt(2.0 * i.frequency_hz)
            )
        },
    ));
    out
}

fn beam_line(r: &BeamResult) -> (Verdict, String) {
    (
        r.verdict,
        format!(
            "Deflection {:.2} mm at {} mm from the left end; allowed {:.2} mm",
            r.deflection_mm,
            fmt(r.at_mm),
            r.limit_mm
        ),
    )
}

fn plate_line(r: &PlateResult) -> (Verdict, String) {
    (
        r.verdict,
        format!(
            "Deflection {:.2} mm at the centre; allowed {:.2} mm",
            r.deflection_mm, r.limit_mm
        ),
    )
}

/// The tab as plain text, for Copy.
pub(super) fn mechanical_text(s: &MechState) -> String {
    let mut out = String::from("Mechanical estimates (model, not a certificate)\n");
    let i = &s.bus;
    let _ = writeln!(
        out,
        "\nBusbar: {} kA, {} mm spacing, {} mm span ({}), bar {} x {} mm {}",
        fmt(i.ik_a / 1000.0),
        fmt(i.spacing_mm),
        fmt(i.span_mm),
        i.spans.label(),
        fmt(i.breadth_mm),
        fmt(i.depth_mm),
        i.material.name
    );
    match check_busbar(i) {
        Ok(r) => {
            for (v, t) in busbar_lines(&r, i) {
                let _ = writeln!(out, "  {}: {t}", verdict_label(v).0);
            }
        }
        Err(e) => {
            let _ = writeln!(out, "  Not calculated: {e}");
        }
    }
    let _ = writeln!(out, "\nDIN rail over {} mm", fmt(s.rail_span_mm));
    match check_beam(&s.rail_input()) {
        Ok(r) => {
            let (v, t) = beam_line(&r);
            let _ = writeln!(out, "  {}: {t}", verdict_label(v).0);
        }
        Err(e) => {
            let _ = writeln!(out, "  Not calculated: {e}");
        }
    }
    let p = s.plate_input();
    let _ = writeln!(
        out,
        "\nMounting plate {} x {} x {} mm",
        fmt(p.a_mm),
        fmt(p.b_mm),
        fmt(p.thickness_mm)
    );
    match check_plate(&p) {
        Ok(r) => {
            let (v, t) = plate_line(&r);
            let _ = writeln!(out, "  {}: {t}", verdict_label(v).0);
        }
        Err(e) => {
            let _ = writeln!(out, "  Not calculated: {e}");
        }
    }
    out
}

fn material_choice(ui: &mut egui::Ui, m: &mut BarMaterial) {
    ui.horizontal(|ui| {
        ui.selectable_value(m, COPPER, COPPER.name);
        ui.selectable_value(m, ALUMINIUM, ALUMINIUM.name);
    });
}

/// Draws the tab. `available_ka` is the supply's short-circuit current when
/// the project states one; `copy` is set when the user asks for text.
pub(super) fn mechanical(
    ui: &mut egui::Ui,
    s: &mut MechState,
    available_ka: Option<f64>,
    copy: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        badge(ui, Model::Structural);
        if ui.button("Copy as text").clicked() {
            *copy = Some(mechanical_text(s));
        }
    });
    ui.colored_label(
        crate::theme::ui_palette().warning,
        "Estimate, not a verification of the busbar or support rating. Not a safety function.",
    );
    super::dialog_ui::explained(
        ui,
        "Method and limits",
        "After the IEC 60865-1 method, not checked against the standard text or a test. Deflection assumes a simple beam and ignores stiffening by devices or ducts.",
    );
    egui::CollapsingHeader::new("Busbar short circuit")
        .default_open(true)
        .show(ui, |ui| busbar(ui, s, available_ka));
    egui::CollapsingHeader::new("DIN rail under device weight")
        .default_open(true)
        .show(ui, |ui| rail(ui, s));
    egui::CollapsingHeader::new("Mounting plate under device weight")
        .default_open(false)
        .show(ui, |ui| plate(ui, s));
}

fn busbar(ui: &mut egui::Ui, s: &mut MechState, available_ka: Option<f64>) {
    let i = &mut s.bus;
    match available_ka {
        Some(ka) => {
            if ui
                .button(format!("Use the {} kA at the supply", fmt(ka)))
                .on_hover_text("From Check > Calculations > Panel SCCR")
                .clicked()
            {
                i.ik_a = ka * 1000.0;
            }
            ui.weak("Supply current taken from Check > Calculations > Panel SCCR.");
        }
        None => {
            ui.add_enabled(false, egui::Button::new("Use the current at the supply"));
            ui.weak(
                "Supply current not stated. Enter it in Electrical inputs; it then shows under Check > Calculations > Panel SCCR.",
            );
        }
    }
    egui::Grid::new("mech_bus")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            row(
                ui,
                "Short-circuit current",
                "Initial symmetrical short-circuit current Ik'' at the busbar, from the supply data (Check > Calculations > Panel SCCR).",
                |ui| {
                    let mut ka = i.ik_a / 1000.0;
                    drag(ui, &mut ka, "kA", 0.1, 500.0, 0.5);
                    i.ik_a = ka * 1000.0;
                },
            );
            row(
                ui,
                "R/X of the fault path",
                "Sets the peak factor. 0.1 is typical near a transformer; 0.3 and above for long cables.",
                |ui| {
                    drag(ui, &mut i.r_over_x, "", 0.0, 5.0, 0.01);
                },
            );
            row(
                ui,
                "Fault",
                "Three-phase: three bars in one plane, equal spacing, the middle bar takes the most. Two conductors: single phase, DC or a two-phase fault.",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut i.fault, BusFault::ThreePhase, "Three-phase");
                        ui.selectable_value(&mut i.fault, BusFault::TwoConductor, "Two conductors");
                    });
                },
            );
            row(
                ui,
                "Spacing between bar centres",
                "Centre to centre. Wide bars at small spacing see a smaller force than this thin-wire value, so the result is on the safe side.",
                |ui| drag(ui, &mut i.spacing_mm, "mm", 1.0, 2000.0, 1.0),
            );
            row(
                ui,
                "Distance between supports",
                "Support centre to support centre.",
                |ui| drag(ui, &mut i.span_mm, "mm", 10.0, 6000.0, 5.0),
            );
            row(
                ui,
                "Supports",
                "Insulators that can tilt act as free ends. Clamped ends need rigid clamps. A bar over three supports takes 2.5 times the force on the middle support.",
                |ui| {
                    ui.horizontal(|ui| {
                        for sp in [Spans::Simple, Spans::FixedBoth, Spans::TwoSpan] {
                            ui.selectable_value(&mut i.spans, sp, sp.label());
                        }
                    });
                },
            );
            row(
                ui,
                "Bar size in the force direction",
                "A bar on edge (tall side toward the force) is far stiffer than the same bar flat.",
                |ui| drag(ui, &mut i.depth_mm, "mm", 1.0, 300.0, 0.5),
            );
            row(
                ui,
                "Bar size across the force",
                "The other side of the bar section.",
                |ui| drag(ui, &mut i.breadth_mm, "mm", 1.0, 300.0, 0.5),
            );
            row(
                ui,
                "Material",
                "Typical minimum yield stress. Use the value on the maker's certificate if it differs.",
                |ui| material_choice(ui, &mut i.material),
            );
            row(
                ui,
                "Yield stress Rp0.2",
                "Overrides the preset.",
                |ui| {
                    let mut mpa = i.material.rp02_pa / 1.0e6;
                    drag(ui, &mut mpa, "MPa", 20.0, 600.0, 1.0);
                    i.material.rp02_pa = mpa * 1.0e6;
                },
            );
            row(
                ui,
                "Dynamic factor",
                "1 treats the force as a steady load. The swinging of the bar after the first peak can raise stress and support force; the natural frequency below shows when that is likely. A value above 1 scales both.",
                |ui| drag(ui, &mut i.dynamic_factor, "", 0.5, 5.0, 0.05),
            );
            row(
                ui,
                "Support rating",
                "Cantilever force the insulator may take, from its datasheet. Zero means not known.",
                |ui| {
                    let mut n = i.support_rating_n.unwrap_or(0.0);
                    drag(ui, &mut n, "N", 0.0, 100_000.0, 10.0);
                    i.support_rating_n = (n > 0.0).then_some(n);
                },
            );
            row(
                ui,
                "System frequency",
                "50 Hz in the Netherlands.",
                |ui| drag(ui, &mut i.frequency_hz, "Hz", 16.0, 400.0, 1.0),
            );
        });
    ui.add_space(4.0);
    match check_busbar(i) {
        Ok(r) => {
            for (v, t) in busbar_lines(&r, i) {
                result_line(ui, v, t);
            }
        }
        Err(e) => {
            result_line(ui, Verdict::Note, format!("Not calculated: {e}"));
        }
    }
}

fn rail(ui: &mut egui::Ui, s: &mut MechState) {
    egui::Grid::new("mech_rail")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            row(
                ui,
                "Rail",
                "Idealised steel top-hat rail, 35 mm wide: TH35 x 7.5 is 1 mm thick, TH35 x 15 is 1.5 mm thick. Sharp corners, so the stiffness is a little high.",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut s.rail_profile, 0, "TH35 x 7.5");
                        ui.selectable_value(&mut s.rail_profile, 1, "TH35 x 15");
                    });
                },
            );
            row(
                ui,
                "Load direction",
                "In the plate plane: devices weighing on a horizontal rail on an upright plate. Out of the plate: a pull on a wire or a plate laid flat.",
                |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut s.in_plate, true, "In the plate plane");
                        ui.selectable_value(&mut s.in_plate, false, "Out of the plate");
                    });
                },
            );
            row(
                ui,
                "Distance between fixing screws",
                "The rail is checked between two fixings.",
                |ui| drag(ui, &mut s.rail_span_mm, "mm", 20.0, 2000.0, 5.0),
            );
            row(
                ui,
                "Held",
                "Screws that do not clamp the rail act as free ends.",
                |ui| {
                    ui.horizontal(|ui| {
                        for h in [Hold::Simple, Hold::FixedBoth, Hold::Cantilever] {
                            ui.selectable_value(&mut s.rail_hold, h, h.label());
                        }
                    });
                },
            );
            row(
                ui,
                "Devices spread along the span",
                "Total weight of the devices on this stretch of rail.",
                |ui| drag(ui, &mut s.rail_kg, "kg", 0.0, 200.0, 0.1),
            );
            row(
                ui,
                "Heavy device at the middle",
                "One device that is much heavier than the rest.",
                |ui| drag(ui, &mut s.rail_point_kg, "kg", 0.0, 200.0, 0.1),
            );
            row(
                ui,
                "Allowed deflection",
                "Span divided by this number. 200 is a common planning limit.",
                |ui| drag_ratio(ui, &mut s.rail_limit),
            );
        });
    ui.add_space(4.0);
    match check_beam(&s.rail_input()) {
        Ok(r) => {
            let (v, t) = beam_line(&r);
            result_line(ui, v, t);
        }
        Err(e) => result_line(ui, Verdict::Note, format!("Not calculated: {e}")),
    }
    super::dialog_ui::explained(
        ui,
        "Fixing screws and clips",
        "The fixing screws and the device clips usually fail before the rail bends.",
    );
}

fn plate(ui: &mut egui::Ui, s: &mut MechState) {
    egui::Grid::new("mech_plate")
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, |ui| {
            row(
                ui,
                "Plate length",
                "Plate supported along all four edges. A plate on four corner posts bends more than this.",
                |ui| drag(ui, &mut s.plate.a_mm, "mm", 50.0, 3000.0, 5.0),
            );
            row(ui, "Plate width", "", |ui| {
                drag(ui, &mut s.plate.b_mm, "mm", 50.0, 3000.0, 5.0)
            });
            row(
                ui,
                "Thickness",
                "Steel, 210 GPa, Poisson 0.3. Deflection falls with the cube of the thickness.",
                |ui| drag(ui, &mut s.plate.thickness_mm, "mm", 0.5, 20.0, 0.1),
            );
            row(
                ui,
                "Devices spread over the plate",
                "Total weight, counted as an even load. Only for a plate that lies flat or swings out; a plate upright in the cabinet carries weight as shear, not bending.",
                |ui| drag(ui, &mut s.plate_kg, "kg", 0.0, 500.0, 0.5),
            );
            row(
                ui,
                "Heavy device at the centre",
                "One concentrated weight in the middle of the plate.",
                |ui| drag(ui, &mut s.plate_centre_kg, "kg", 0.0, 500.0, 0.5),
            );
            row(
                ui,
                "Allowed deflection",
                "Shorter side divided by this number.",
                |ui| drag_ratio(ui, &mut s.plate.limit_ratio),
            );
        });
    ui.add_space(4.0);
    match check_plate(&s.plate_input()) {
        Ok(r) => {
            let (v, t) = plate_line(&r);
            result_line(ui, v, t);
        }
        Err(e) => result_line(ui, Verdict::Note, format!("Not calculated: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_inputs_calculate_and_copy_as_text() {
        let s = MechState::default();
        let text = mechanical_text(&s);
        assert!(text.contains("Busbar:"), "{text}");
        assert!(text.contains("Bending stress"), "{text}");
        assert!(text.contains("DIN rail"), "{text}");
        assert!(text.contains("Mounting plate"), "{text}");
        assert!(!text.contains("Not calculated"), "{text}");
    }

    #[test]
    fn a_rail_without_load_says_so() {
        let s = MechState {
            rail_kg: 0.0,
            rail_point_kg: 0.0,
            ..MechState::default()
        };
        assert!(mechanical_text(&s).contains("Enter at least one load"));
    }

    #[test]
    fn a_pull_out_of_the_plate_deflects_more_than_a_weight_in_it() {
        let mut s = MechState::default();
        let a = check_beam(&s.rail_input()).unwrap().deflection_mm;
        s.in_plate = false;
        let b = check_beam(&s.rail_input()).unwrap().deflection_mm;
        assert!(b > 10.0 * a, "{b} against {a}");
    }
}
