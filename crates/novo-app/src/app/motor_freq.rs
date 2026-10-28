//! Motor at variable frequency (`sim.motor_curves`, Simulate > Motor at variable frequency):
//! torque-speed curves of a motor of the drawing on a V/f drive, with the breakdown torque,
//! the torque within a current limit and the field weakening region (lane L2b, engine
//! `novo_drive::induction`).
//!
//! The motor's equivalent circuit is the one the Simulate motor start window fits to the
//! nameplate; this window runs it at other frequencies, with reactances scaled by frequency and
//! the voltage from the drive's V/f law (base frequency, boost, shape).

use eframe::egui;
use novo_core::{Project, SymbolId};
use novo_drive::drive::{VfCurve, VfShape};
use novo_drive::induction::{
    Circuit, FrequencyRow, InductionMotor, Region, SelfVentilation, Supply, TorqueLimitedBy,
};
use novo_drive::motor::Direction;
use novo_sim::analog::motor::{Connection, Motor};
use novo_sim::network::motor_of;
use novo_theme::tokens;

use super::model_labels::{self, Model};

/// What the window remembers.
pub(crate) struct MotorFreq {
    pub(super) open: bool,
    motor: Option<SymbolId>,
    /// Base frequency of the V/f law; 0 follows the motor's rated frequency.
    base_hz: f64,
    boost_v: f64,
    quadratic: bool,
    /// Current limit of the drive, percent of the motor's rated current.
    limit_percent: f64,
    max_hz: f64,
    /// Standstill cooling of a self-ventilated motor, percent of the rated losses.
    cooling_percent: f64,
    show_continuous: bool,
    cache: Option<(Key, Result<View, String>)>,
}

impl Default for MotorFreq {
    fn default() -> Self {
        Self {
            open: false,
            motor: None,
            base_hz: 0.0,
            boost_v: 0.0,
            quadratic: false,
            limit_percent: 150.0,
            max_hz: 100.0,
            cooling_percent: 40.0,
            show_continuous: true,
            cache: None,
        }
    }
}

#[derive(Clone, PartialEq)]
struct Key {
    revision: u64,
    motor: SymbolId,
    base: u32,
    boost: u32,
    quadratic: bool,
    limit: u32,
    max_hz: u32,
    cooling: u32,
}

/// Everything the window draws.
pub(crate) struct View {
    pub tag_note: Vec<String>,
    pub motor: InductionMotor,
    pub rated_current_a: f64,
    pub rows: Vec<FrequencyRow>,
    /// `(frequency, torque-speed curve in (rpm, N m))`.
    pub curves: Vec<(f64, Vec<(f64, f64)>)>,
}

/// The induction model for a motor of the drawing, from the circuit the motor start window
/// fits: per winding values become per phase of the equivalent star.
pub(crate) fn induction_of(m: &Motor) -> Result<InductionMotor, String> {
    let k = match m.connection {
        Connection::Star => 1.0,
        Connection::Delta => 1.0 / 3.0,
    };
    InductionMotor::new(
        Circuit::new(m.r1 * k, m.x1 * k, m.r2 * k, m.x2 * k, m.xm * k),
        2 * m.pole_pairs,
        m.rated_ll,
        m.hz,
        m.rated_rpm,
    )
    .map_err(|e| e.to_string())
}

/// The numbers for one motor and one V/f law (`None` base frequency = the motor's own).
pub(crate) fn compute(motor: &Motor, notes: &[String], p: &Params) -> Result<View, String> {
    let im = induction_of(motor)?;
    let base = if p.base_hz > 0.0 { p.base_hz } else { motor.hz };
    let mut vf = VfCurve::new(motor.rated_ll, base);
    vf.shape = if p.quadratic {
        VfShape::Quadratic
    } else {
        VfShape::ConstantTorque
    };
    vf.boost_v = p.boost_v;
    vf.boost_end_hz = base / 2.0;
    let supply =
        Supply::new(motor.hz, motor.rated_ll, Direction::Positive).map_err(|e| e.to_string())?;
    let rated = im
        .at_slip(&supply, im.rated_slip())
        .map_err(|e| e.to_string())?;
    let rated_current_a = rated.stator_current_a;
    let limit = p.limit_percent / 100.0 * rated_current_a;
    let cooling = SelfVentilation {
        standstill_loss_fraction: (p.cooling_percent / 100.0).clamp(0.0, 1.0),
    };
    let mut freqs: Vec<f64> = [
        2.0, 5.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0, 75.0, 87.0, 100.0,
    ]
    .into_iter()
    .chain([base, motor.hz])
    .filter(|f| *f > 0.0 && *f <= p.max_hz)
    .collect();
    freqs.sort_by(f64::total_cmp);
    freqs.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    let mut rows = Vec::new();
    for &f in &freqs {
        rows.push(
            im.frequency_row(&vf, f, limit, &cooling)
                .map_err(|e| e.to_string())?,
        );
    }
    let mut curves = Vec::new();
    for f in curve_frequencies(base, motor.hz, p.max_hz) {
        let s = Supply::new(f, vf.volts_for_frequency(f), Direction::Positive)
            .map_err(|e| e.to_string())?;
        curves.push((
            f,
            im.torque_speed_curve(&s, 160).map_err(|e| e.to_string())?,
        ));
    }
    Ok(View {
        tag_note: notes.to_vec(),
        motor: im,
        rated_current_a,
        rows,
        curves,
    })
}

/// The frequencies drawn as curves: a few below base, base, and the top of the range.
fn curve_frequencies(base: f64, rated_hz: f64, max_hz: f64) -> Vec<f64> {
    let mut f: Vec<f64> = [0.1, 0.5, 1.0]
        .into_iter()
        .map(|x| x * base.max(rated_hz))
        .chain([max_hz])
        .filter(|f| *f > 0.0 && *f <= max_hz)
        .collect();
    f.sort_by(f64::total_cmp);
    f.dedup_by(|a, b| (*a - *b).abs() < 0.05);
    f
}

/// The inputs of [`compute`].
#[derive(Clone, Copy)]
pub(crate) struct Params {
    pub base_hz: f64,
    pub boost_v: f64,
    pub quadratic: bool,
    pub limit_percent: f64,
    pub max_hz: f64,
    pub cooling_percent: f64,
}

impl MotorFreq {
    fn params(&self) -> Params {
        Params {
            base_hz: self.base_hz,
            boost_v: self.boost_v,
            quadratic: self.quadratic,
            limit_percent: self.limit_percent,
            max_hz: self.max_hz,
            cooling_percent: self.cooling_percent,
        }
    }
}

impl super::NovoApp {
    /// `sim.motor_curves`: opens or closes the window.
    pub(super) fn toggle_motor_curves(&mut self) {
        self.wb.motor_freq.open ^= true;
    }

    pub(super) fn motor_freq_window(&mut self, ctx: &egui::Context) {
        if !self.wb.motor_freq.open {
            return;
        }
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let motors: Vec<SymbolId> = project
            .symbols()
            .filter(|s| novo_sim::is_motor(s))
            .map(|s| s.id)
            .collect();
        let name = |id: SymbolId| symbol_tag(project, id);
        let state = &mut self.wb.motor_freq;
        if state.motor.is_none_or(|m| !motors.contains(&m)) {
            state.motor = motors.first().copied();
        }
        let mut open = true;
        super::panel::dialog(ctx, "Motor at variable frequency", [620.0, 600.0])
            .open(super::panel::esc(ctx, &mut open))
            .show(ctx, |ui| {
                let Some(symbol) = state.motor else {
                    ui.weak("The project has no motor. Place one and give it a power (kW).");
                    return;
                };
                controls(ui, state, &motors, &name);
                let key = Key {
                    revision: project.revision(),
                    motor: symbol,
                    base: (state.base_hz * 10.0) as u32,
                    boost: (state.boost_v * 10.0) as u32,
                    quadratic: state.quadratic,
                    limit: (state.limit_percent * 10.0) as u32,
                    max_hz: (state.max_hz * 10.0) as u32,
                    cooling: (state.cooling_percent * 10.0) as u32,
                };
                if state.cache.as_ref().is_none_or(|(k, _)| *k != key) {
                    let params = state.params();
                    let netlist = project.netlist();
                    let result = motor_of(project, &netlist, symbol).and_then(|(m, notes)| {
                        let texts: Vec<String> = notes.into_iter().map(|n| n.text).collect();
                        compute(&m, &texts, &params)
                    });
                    state.cache = Some((key, result));
                }
                match state.cache.as_ref().map(|(_, r)| r) {
                    Some(Ok(view)) => show(ui, view, state.show_continuous, state.cooling_percent),
                    Some(Err(why)) => {
                        ui.add_space(6.0);
                        ui.colored_label(crate::theme::ui_palette().warning, why);
                    }
                    None => {}
                }
            });
        if !open {
            self.wb.motor_freq.open = false;
        }
    }
}

fn controls(
    ui: &mut egui::Ui,
    state: &mut MotorFreq,
    motors: &[SymbolId],
    name: &dyn Fn(SymbolId) -> String,
) {
    ui.horizontal(|ui| {
        ui.label("Motor");
        let current = state.motor.map(name).unwrap_or_default();
        egui::ComboBox::from_id_salt("motor_freq_motor")
            .selected_text(current)
            .show_ui(ui, |ui| {
                for &m in motors {
                    ui.selectable_value(&mut state.motor, Some(m), name(m));
                }
            });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Base frequency, Hz (0 = use the motor's)");
        ui.add(
            egui::DragValue::new(&mut state.base_hz)
                .range(0.0..=400.0)
                .suffix(" Hz"),
        );
        ui.add(
            egui::DragValue::new(&mut state.boost_v)
                .range(0.0..=60.0)
                .suffix(" V boost"),
        );
        ui.checkbox(&mut state.quadratic, "V/f squared (fan, pump)");
    });
    ui.horizontal_wrapped(|ui| {
        ui.add(
            egui::DragValue::new(&mut state.limit_percent)
                .range(50.0..=400.0)
                .suffix(" % current limit"),
        );
        ui.add(
            egui::DragValue::new(&mut state.max_hz)
                .range(10.0..=400.0)
                .suffix(" Hz top"),
        );
        ui.checkbox(&mut state.show_continuous, "Self-ventilated limit");
        if state.show_continuous {
            ui.add(
                egui::DragValue::new(&mut state.cooling_percent)
                    .range(0.0..=100.0)
                    .suffix(" % cooling at standstill"),
            );
        }
    });
}

fn symbol_tag(project: &Project, id: SymbolId) -> String {
    project
        .symbol(id)
        .and_then(|s| s.device())
        .and_then(|d| project.device(d))
        .map_or_else(|| format!("Motor {}", id.0), |d| d.tag.to_string())
}

fn show(ui: &mut egui::Ui, v: &View, continuous: bool, cooling_percent: f64) {
    let m = &v.motor;
    let t_n = m.rated_torque_nm();
    ui.add_space(6.0);
    ui.label(format!(
        "{} poles · {:.0} V · {:.0} Hz · rated {:.1} N·m at {:.0} rpm · rated current {:.1} A",
        m.poles(),
        m.rated_v_ll(),
        m.rated_hz(),
        t_n,
        m.rated_rpm(),
        v.rated_current_a
    ));
    ui.add_space(4.0);
    chart(ui, v, continuous, cooling_percent);
    ui.add_space(4.0);
    table(ui, v);
    for n in &v.tag_note {
        ui.weak(n);
    }
    model_labels::badge(ui, Model::MotorFrequency);
    model_labels::note(ui, Model::MotorFrequency);
    ui.add_space(2.0);
    ui.collapsing("What this model leaves out", |ui| {
        for line in novo_drive::induction::LEAVES_OUT {
            ui.weak(format!("\u{2022} {line}"));
        }
    });
}

/// Colour of the n-th curve.
fn curve_colour(i: usize) -> egui::Color32 {
    let p = crate::theme::ui_palette();
    [p.info, p.success, p.accent, p.warning, p.error][i % 5]
}

fn chart(ui: &mut egui::Ui, v: &View, continuous: bool, cooling_percent: f64) {
    let height = (ui.available_height() - 150.0).clamp(150.0, 240.0);
    let size = egui::vec2(ui.available_width().max(320.0), height);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = ui.painter_at(rect);
    let pal = crate::theme::ui_palette();
    p.rect_filled(rect, 4.0, pal.panel);
    let plot = rect.shrink2(egui::vec2(46.0, 20.0));
    let n_max = v
        .curves
        .iter()
        .flat_map(|(_, c)| c.iter().map(|(n, _)| *n))
        .fold(1.0f64, f64::max);
    let (mut t_min, mut t_max) = (0.0f64, v.motor.rated_torque_nm());
    for (_, c) in &v.curves {
        for (_, t) in c {
            t_min = t_min.min(*t);
            t_max = t_max.max(*t);
        }
    }
    let t_max = t_max * 1.05;
    let t_min = t_min * 1.05;
    let at = |n: f64, t: f64| {
        egui::pos2(
            plot.left() + (n / n_max) as f32 * plot.width(),
            plot.bottom() - ((t - t_min) / (t_max - t_min)) as f32 * plot.height(),
        )
    };
    let font = egui::FontId::proportional(tokens::FONT_SMALL);
    for k in 0..=4 {
        let t = t_min + (t_max - t_min) * f64::from(k) / 4.0;
        let y = at(0.0, t).y;
        p.line_segment(
            [egui::pos2(plot.left(), y), egui::pos2(plot.right(), y)],
            egui::Stroke::new(1.0, pal.border_subtle),
        );
        p.text(
            egui::pos2(plot.left() - 4.0, y),
            egui::Align2::RIGHT_CENTER,
            format!("{:.0}", if t.abs() < 0.5 { 0.0 } else { t }),
            font.clone(),
            pal.text_secondary,
        );
    }
    p.text(
        egui::pos2(plot.left(), rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        "N·m",
        font.clone(),
        pal.text_secondary,
    );
    p.text(
        egui::pos2(plot.right(), plot.bottom() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{n_max:.0} rpm"),
        font.clone(),
        pal.text_secondary,
    );
    // Rated torque.
    let rated_y = at(0.0, v.motor.rated_torque_nm()).y;
    p.line_segment(
        [
            egui::pos2(plot.left(), rated_y),
            egui::pos2(plot.right(), rated_y),
        ],
        egui::Stroke::new(1.0, pal.text_tertiary),
    );
    p.text(
        egui::pos2(plot.right() - 4.0, rated_y - 2.0),
        egui::Align2::RIGHT_BOTTOM,
        "rated torque",
        font.clone(),
        pal.text_tertiary,
    );
    for (i, (f, curve)) in v.curves.iter().enumerate() {
        let points: Vec<_> = curve.iter().map(|(n, t)| at(*n, *t)).collect();
        let colour = curve_colour(i);
        p.add(egui::Shape::line(points, egui::Stroke::new(2.0, colour)));
        if let Some((n, t)) = curve.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)) {
            p.text(
                at(n, t) + egui::vec2(0.0, -3.0),
                egui::Align2::CENTER_BOTTOM,
                format!("{f:.0} Hz"),
                font.clone(),
                colour,
            );
        }
    }
    if continuous {
        // Continuous torque of a self-ventilated motor against speed.
        let cool = SelfVentilation {
            standstill_loss_fraction: (cooling_percent / 100.0).clamp(0.0, 1.0),
        };
        let pts: Vec<_> = (0..=40)
            .map(|k| {
                let n = n_max * f64::from(k) / 40.0;
                at(n, v.motor.continuous_torque_nm(n, &cool))
            })
            .collect();
        p.add(egui::Shape::line(
            pts,
            egui::Stroke::new(1.0, pal.text_secondary),
        ));
        p.text(
            at(0.0, v.motor.continuous_torque_nm(0.0, &cool)) + egui::vec2(4.0, 2.0),
            egui::Align2::LEFT_TOP,
            "continuous (self-ventilated)",
            font,
            pal.text_secondary,
        );
    }
    if let Some(pos) = response.hover_pos()
        && plot.contains(pos)
    {
        let n = f64::from((pos.x - plot.left()) / plot.width()) * n_max;
        let mut text = format!("{n:.0} rpm");
        for (f, curve) in &v.curves {
            if let Some((_, t)) = curve
                .iter()
                .min_by(|a, b| (a.0 - n).abs().total_cmp(&(b.0 - n).abs()))
            {
                text.push_str(&format!(" · {f:.0} Hz: {t:.1} N·m"));
            }
        }
        response.on_hover_text(text);
    }
}

fn table(ui: &mut egui::Ui, v: &View) {
    egui::Grid::new("motor_freq_rows")
        .striped(true)
        .num_columns(7)
        .show(ui, |ui| {
            for h in [
                "Hz",
                "Volts",
                "Rated torque at",
                "Breakdown N·m",
                "Within limit N·m",
                "Flux",
                "Region",
            ] {
                ui.strong(h);
            }
            ui.end_row();
            for r in &v.rows {
                ui.label(format!("{:.0}", r.frequency_hz));
                ui.label(format!("{:.0}", r.volts_ll));
                match (r.rated_torque_rpm, r.rated_torque_current_a) {
                    (Some(rpm), Some(a)) => ui.label(format!("{rpm:.0} rpm, {a:.1} A")),
                    _ => ui.colored_label(crate::theme::ui_palette().warning, "not reachable"),
                };
                let reach = if r.breakdown_reachable {
                    ""
                } else {
                    " (at standstill)"
                };
                ui.label(format!("{:.0}{reach}", r.breakdown_nm));
                let by = match r.limited_by {
                    TorqueLimitedBy::Current => "current",
                    TorqueLimitedBy::Breakdown => "breakdown",
                };
                ui.label(format!("{:.0} ({by})", r.current_limited_nm));
                ui.label(format!("{:.2}", r.flux_pu));
                ui.label(match r.region {
                    Region::ConstantFlux => "constant flux",
                    Region::FieldWeakening => "field weakening",
                });
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_sim::analog::motor::Motor;

    /// The 7.5 kW 400 V motor of the motor start tests, star connected.
    fn motor() -> Motor {
        Motor {
            r1: 0.9,
            x1: 1.3,
            r2: 0.6,
            x2: 1.5,
            xm: 40.0,
            hz: 50.0,
            pole_pairs: 2,
            rated_ll: 400.0,
            connection: Connection::Star,
            rated_power_w: 7500.0,
            rated_rpm: 1450.0,
        }
    }

    fn params() -> Params {
        Params {
            base_hz: 0.0,
            boost_v: 0.0,
            quadratic: false,
            limit_percent: 150.0,
            max_hz: 100.0,
            cooling_percent: 40.0,
        }
    }

    #[test]
    fn the_model_agrees_with_the_motor_start_circuit_at_the_rated_point() {
        let sim = motor();
        let im = induction_of(&sim).unwrap();
        let s = Supply::new(50.0, 400.0, Direction::Positive).unwrap();
        // Same circuit, same voltage: same air-gap torque at any slip (star: winding voltage is
        // the phase voltage).
        for slip in [0.02, 0.0333, 0.1, 0.5, 1.0] {
            let a = im.at_slip(&s, slip).unwrap().air_gap_torque_nm;
            let b = sim.torque(400.0 / 3f64.sqrt(), slip);
            assert!(
                (a - b).abs() < 1e-9 * b.abs().max(1.0),
                "slip {slip}: {a} vs {b}"
            );
        }
    }

    #[test]
    fn a_delta_motor_is_the_same_motor_at_a_third_of_the_impedance() {
        let mut d = motor();
        d.connection = Connection::Delta;
        // Winding impedance triples in delta for the same line behaviour: use 3x values.
        d.r1 *= 3.0;
        d.x1 *= 3.0;
        d.r2 *= 3.0;
        d.x2 *= 3.0;
        d.xm *= 3.0;
        let a = induction_of(&motor()).unwrap();
        let b = induction_of(&d).unwrap();
        let s = Supply::new(30.0, 240.0, Direction::Positive).unwrap();
        let (pa, pb) = (a.at_slip(&s, 0.04).unwrap(), b.at_slip(&s, 0.04).unwrap());
        assert!((pa.torque_nm - pb.torque_nm).abs() < 1e-9 * pa.torque_nm);
        assert!((pa.stator_current_a - pb.stator_current_a).abs() < 1e-9 * pa.stator_current_a);
    }

    #[test]
    fn the_table_covers_the_range_and_the_curves_are_drawn() {
        let v = compute(&motor(), &[], &params()).unwrap();
        assert!(v.rows.len() >= 8);
        assert!(
            v.rows
                .windows(2)
                .all(|w| w[0].frequency_hz < w[1].frequency_hz)
        );
        assert!(v.rows.iter().any(|r| r.region == Region::FieldWeakening));
        assert!(v.curves.len() >= 3);
        for (_, c) in &v.curves {
            assert!(c.len() >= 100 && c.iter().all(|(n, t)| n.is_finite() && t.is_finite()));
        }
        // Raising the boost raises the low-frequency torque within the current limit.
        let low = |p: &Params| {
            compute(&motor(), &[], p)
                .unwrap()
                .rows
                .iter()
                .find(|r| (r.frequency_hz - 5.0).abs() < 0.01)
                .unwrap()
                .breakdown_nm
        };
        let boosted = Params {
            boost_v: 20.0,
            ..params()
        };
        assert!(low(&boosted) > low(&params()));
    }
}
