//! The Busbars tab of the Calculations window (Check > Calculations >
//! Busbars): resistance, reactance, skin and proximity effect and the share
//! of the current each parallel bar carries, from [`novo_emc::busbar`].
//!
//! The numbers are a model (quasi-static, non-magnetic, straight parallel
//! conductors); the badge and the "?" say what is not seen.

use std::fmt::Write as _;

use eframe::egui;
use novo_emc::busbar::{self, Material, NOT_MODELLED, Setup, Solution};

use super::model_labels::{Model, badge};

const SWEEP_HZ: [f64; 6] = [0.0, 50.0, 150.0, 250.0, 500.0, 1000.0];

/// What the tab remembers between frames.
#[derive(Debug)]
pub(super) struct BusbarState {
    aluminium: bool,
    w_mm: f64,
    t_mm: f64,
    bars: usize,
    stacked: bool,
    gap_mm: f64,
    pitch_mm: f64,
    length_m: f64,
    hz: f64,
    amps: f64,
    temp_c: f64,
    line_volts: f64,
    cache: Option<(Setup, Option<Solution>)>,
    sweep: Option<(Setup, Vec<Solution>)>,
}

impl Default for BusbarState {
    fn default() -> BusbarState {
        BusbarState {
            aluminium: false,
            w_mm: 80.0,
            t_mm: 10.0,
            bars: 1,
            stacked: true,
            gap_mm: 10.0,
            pitch_mm: 200.0,
            length_m: 1.0,
            hz: 50.0,
            amps: 800.0,
            temp_c: 70.0,
            line_volts: 400.0,
            cache: None,
            sweep: None,
        }
    }
}

impl BusbarState {
    fn setup(&self) -> Setup {
        let material = if self.aluminium {
            Material::Aluminium
        } else {
            Material::Copper
        };
        Setup::three_phase(
            material,
            self.w_mm,
            self.t_mm,
            self.bars,
            self.gap_mm,
            self.stacked,
            self.pitch_mm,
            self.length_m * 1000.0,
            self.hz,
            self.amps,
            self.temp_c,
        )
    }

    /// The solution for the current inputs (solved again only when they change).
    #[cfg(test)]
    pub(super) fn solution(&mut self) -> Option<&Solution> {
        self.refresh(false);
        self.cache.as_ref().and_then(|(_, s)| s.as_ref())
    }

    /// Solve again when an input changed. While a field is being dragged
    /// (`busy`) the last result stays, so dragging does not stutter.
    fn refresh(&mut self, busy: bool) {
        let setup = self.setup();
        if busy && self.cache.is_some() {
            return;
        }
        if self.cache.as_ref().is_none_or(|(s, _)| *s != setup) {
            let solved = busbar::solve(&setup);
            self.cache = Some((setup, solved));
        }
    }
}

/// One choice of a group: a button that shows it is the chosen one.
fn pick<T: PartialEq + Copy>(ui: &mut egui::Ui, value: &mut T, this: T, label: &str) {
    if ui
        .add(egui::Button::new(label).selected(*value == this))
        .clicked()
    {
        *value = this;
    }
}

fn milli(ohm: f64) -> String {
    format!("{:.4}", ohm * 1e3)
}

/// The result as plain text, for "Copy as text".
pub(super) fn text_of(setup: &Setup, sol: &Solution) -> String {
    let mut out = String::from("Busbar impedance\n");
    let _ = writeln!(out, "{}", busbar::MODEL_LABEL);
    let _ = writeln!(
        out,
        "Run {:.2} m, {} conductors, {} Hz, {:.0} A per phase",
        setup.length_mm / 1000.0,
        setup.conductors.len(),
        sol.hz,
        setup.group_current.first().map_or(0.0, |c| c.abs())
    );
    for (g, z) in sol.group_zeff.iter().enumerate() {
        let _ = writeln!(
            out,
            "{}: R {} mohm, X {} mohm (with coupling), drop {:.3} V",
            setup.group_name[g],
            milli(sol.group_r_ac[g]),
            milli(z.im),
            sol.group_v[g].abs()
        );
    }
    for c in &sol.conductors {
        let _ = writeln!(
            out,
            "{}: {:.0} A ({:.1} % of its phase), {:.2} W, AC factor {:.3}",
            c.name,
            c.current.abs(),
            c.share * 100.0,
            c.loss_w,
            c.ac_factor
        );
    }
    let _ = writeln!(out, "Total loss {:.2} W", sol.total_loss_w);
    for w in &sol.warnings {
        let _ = writeln!(out, "Note: {w}");
    }
    out
}

pub(super) fn busbar_tab(ui: &mut egui::Ui, st: &mut BusbarState) {
    let p = crate::theme::ui_palette();
    ui.horizontal_wrapped(|ui| {
        ui.strong("Three-phase bars");
        badge(ui, Model::Busbar);
        let hover = NOT_MODELLED.join("\n");
        ui.label("?")
            .on_hover_text(format!("{}\n\nNot modelled:\n{hover}", busbar::MODEL_LABEL));
    });
    ui.weak("Straight parallel bars, no steel nearby, no joints or bends. Planning numbers.");
    egui::Grid::new("busbar_inputs")
        .num_columns(4)
        .spacing([12.0, 4.0])
        .show(ui, |ui| {
            ui.label("Material");
            ui.horizontal(|ui| {
                pick(ui, &mut st.aluminium, false, "Copper");
                pick(ui, &mut st.aluminium, true, "Aluminium");
            });
            ui.label("Temperature");
            ui.add(
                egui::DragValue::new(&mut st.temp_c)
                    .range(0.0..=200.0)
                    .suffix(" °C"),
            );
            ui.end_row();
            ui.label("Bar width");
            ui.add(
                egui::DragValue::new(&mut st.w_mm)
                    .range(5.0..=400.0)
                    .suffix(" mm"),
            );
            ui.label("Bar thickness");
            ui.add(
                egui::DragValue::new(&mut st.t_mm)
                    .range(1.0..=60.0)
                    .suffix(" mm"),
            );
            ui.end_row();
            ui.label("Bars per phase");
            ui.horizontal(|ui| {
                for (n, label) in [(1, "1 bar"), (2, "2 bars"), (3, "3 bars"), (4, "4 bars")] {
                    pick(ui, &mut st.bars, n, label);
                }
            });
            ui.label("Bars of a phase");
            ui.horizontal(|ui| {
                pick(ui, &mut st.stacked, true, "stacked");
                pick(ui, &mut st.stacked, false, "side by side");
            });
            ui.end_row();
            ui.label("Gap between bars");
            ui.add(
                egui::DragValue::new(&mut st.gap_mm)
                    .range(0.0..=100.0)
                    .suffix(" mm"),
            );
            ui.label("Phase pitch");
            ui.add(
                egui::DragValue::new(&mut st.pitch_mm)
                    .range(20.0..=600.0)
                    .suffix(" mm"),
            );
            ui.end_row();
            ui.label("Length");
            ui.add(
                egui::DragValue::new(&mut st.length_m)
                    .range(0.05..=30.0)
                    .speed(0.05)
                    .suffix(" m"),
            );
            ui.label("Frequency");
            ui.add(
                egui::DragValue::new(&mut st.hz)
                    .range(0.0..=5000.0)
                    .suffix(" Hz"),
            );
            ui.end_row();
            ui.label("Current per phase");
            ui.add(
                egui::DragValue::new(&mut st.amps)
                    .range(1.0..=20000.0)
                    .suffix(" A"),
            );
            ui.label("Line voltage");
            ui.add(
                egui::DragValue::new(&mut st.line_volts)
                    .range(24.0..=1000.0)
                    .suffix(" V"),
            );
            ui.end_row();
        });
    let busy = ui.input(|i| i.pointer.any_down());
    st.refresh(busy);
    ui.separator();
    let Some((setup, sol)) = st.cache.clone() else {
        return;
    };
    let Some(sol) = sol else {
        let why = match busbar::overlap(&setup) {
            Some((a, b)) => format!("Bars {a} and {b} overlap: raise the phase pitch or the gap."),
            None => "These inputs cannot be solved.".to_owned(),
        };
        ui.colored_label(p.warning, why);
        return;
    };
    for w in &sol.warnings {
        ui.colored_label(p.warning, w);
    }
    ui.strong("Per phase (balanced three-phase current)");
    egui::Grid::new("busbar_phases")
        .striped(true)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            for h in [
                "Phase",
                "R AC (mΩ)",
                "X with coupling (mΩ)",
                "Drop (V)",
                "Drop (%)",
            ] {
                ui.strong(h);
            }
            ui.end_row();
            let phase_v = st.line_volts / 3f64.sqrt();
            for (g, z) in sol.group_zeff.iter().enumerate() {
                ui.label(&setup.group_name[g]);
                ui.label(milli(sol.group_r_ac[g]));
                ui.label(milli(z.im));
                let v = sol.group_v[g].abs();
                ui.label(format!("{v:.3}"));
                ui.label(format!("{:.2}", v / phase_v * 100.0));
                ui.end_row();
            }
        });
    ui.add_space(6.0);
    ui.strong("Per bar");
    egui::Grid::new("busbar_bars")
        .striped(true)
        .spacing([14.0, 3.0])
        .show(ui, |ui| {
            for h in [
                "Bar",
                "Current (A)",
                "Share",
                "A/mm²",
                "Loss (W)",
                "AC factor",
            ] {
                ui.strong(h);
            }
            ui.end_row();
            for (i, c) in sol.conductors.iter().enumerate() {
                let hot = sol.hottest == Some(i);
                let label = if hot {
                    format!("{} (hottest)", c.name)
                } else {
                    c.name.clone()
                };
                ui.label(label);
                ui.label(format!("{:.0}", c.current.abs()));
                ui.label(format!("{:.1} %", c.share * 100.0));
                ui.label(format!("{:.2}", c.amps_per_mm2));
                ui.label(format!("{:.2}", c.loss_w));
                ui.label(format!("{:.3}", c.ac_factor));
                ui.end_row();
            }
        });
    ui.add_space(4.0);
    let skin = sol.conductors.first().map_or(0.0, |c| c.skin_depth_mm);
    let dc: f64 = sol
        .conductors
        .iter()
        .map(|c| c.r_dc_ohm * c.current.norm_sq())
        .sum();
    ui.label(format!(
        "Total loss {:.1} W ({:.1} W/m), {:.3} times the loss at DC resistance. Skin depth {} mm.",
        sol.total_loss_w,
        sol.total_loss_w / setup.length_mm * 1000.0,
        if dc > 0.0 { sol.total_loss_w / dc } else { 1.0 },
        if skin.is_finite() {
            format!("{skin:.1}")
        } else {
            "infinite".to_owned()
        }
    ));
    ui.horizontal(|ui| {
        if ui.button("Frequency sweep").clicked() {
            let mut s = setup.clone();
            s.mesh = busbar::Mesh {
                max_per_side: 8,
                ..s.mesh
            };
            let sols = busbar::sweep(&s, &SWEEP_HZ);
            st.sweep = Some((setup.clone(), sols));
        }
        if ui.button("Copy as text").clicked() {
            ui.ctx().copy_text(text_of(&setup, &sol));
        }
    });
    if let Some((s, sols)) = &st.sweep
        && s.conductors == setup.conductors
        && s.length_mm == setup.length_mm
        && s.group_current == setup.group_current
    {
        {
            egui::Grid::new("busbar_sweep")
                .striped(true)
                .spacing([14.0, 3.0])
                .show(ui, |ui| {
                    for h in ["Hz", "R AC of L1 (mΩ)", "X of L1 (mΩ)", "Total loss (W)"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for r in sols {
                        ui.label(format!("{:.0}", r.hz));
                        ui.label(milli(r.group_r_ac[0]));
                        ui.label(milli(r.group_zeff[0].im));
                        ui.label(format!("{:.1}", r.total_loss_w));
                        ui.end_row();
                    }
                });
            ui.weak("Sweep uses a coarser mesh (8 per side) and the inputs shown when it was run.");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_run_solves_and_shares_add_up() {
        let mut st = BusbarState::default();
        let sol = st.solution().expect("solves").clone();
        assert_eq!(sol.conductors.len(), 3);
        assert!(sol.total_loss_w > 0.0);
        assert!(
            sol.group_zeff
                .iter()
                .all(|z| z.re > 0.0 && z.im.abs() > 0.0)
        );
    }

    #[test]
    fn two_bars_per_phase_share_to_one() {
        let mut st = BusbarState {
            bars: 2,
            stacked: false,
            ..BusbarState::default()
        };
        let sol = st.solution().expect("solves").clone();
        assert_eq!(sol.conductors.len(), 6);
        for g in 0..3 {
            let s: f64 = sol
                .conductors
                .iter()
                .filter(|c| c.group == g)
                .map(|c| c.share)
                .sum();
            assert!((s - 1.0).abs() < 1e-9);
        }
        let setup = st.setup();
        assert!(text_of(&setup, &sol).contains("Total loss"));
    }

    #[test]
    fn the_tab_is_registered_with_its_label() {
        assert!(super::super::learn::ANCHORS.contains(&"calc.tab.busbars"));
        assert!(Model::Busbar.label().contains("model"));
    }
}
