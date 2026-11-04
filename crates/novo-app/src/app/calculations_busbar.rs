//! The "Busbar heat" tab of the Calculations window (Check > Calculations):
//! the bar's current, size and surroundings go in, the temperature along the
//! bar, its heat loss into the cabinet air, the largest current for a chosen
//! temperature and the time to reach it come out. The numbers come from
//! [`novo_thermal::busbar`]; this file only draws them.
//!
//! A model: exact against hand solutions in the unit tests, not validated
//! against a measured busbar.

use eframe::egui;
use novo_thermal::busbar::{
    self, Bar, End, Film, Input, Joint, Material, Output, max_current_for_limit, solve_steady,
    time_to_reach,
};

use super::model_labels::{Model, badge};

/// What the user types, in the units a workshop uses (mm, °C, µΩ).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct BusbarForm {
    pub aluminium: bool,
    pub current_a: f64,
    pub width_mm: f64,
    pub thickness_mm: f64,
    pub length_mm: f64,
    pub ambient_c: f64,
    pub frequency_hz: f64,
    pub proximity: f64,
    pub emissivity: f64,
    pub upright: bool,
    /// Share of the surface open to the air, percent.
    pub exposed_pct: f64,
    /// End temperature in °C when the end is tied to a cooled lug.
    pub end_c: [Option<f64>; 2],
    /// Contact resistance of one joint in the middle, µΩ (0 = none).
    pub joint_micro_ohm: f64,
    pub limit_c: f64,
}

impl Default for BusbarForm {
    fn default() -> Self {
        BusbarForm {
            aluminium: false,
            current_a: 400.0,
            width_mm: 50.0,
            thickness_mm: 5.0,
            length_mm: 1000.0,
            ambient_c: 35.0,
            frequency_hz: 50.0,
            proximity: 1.0,
            emissivity: 0.4,
            upright: false,
            exposed_pct: 100.0,
            end_c: [None, None],
            joint_micro_ohm: 0.0,
            limit_c: 105.0,
        }
    }
}

impl BusbarForm {
    pub(super) fn input(&self) -> Input {
        let bar = Bar {
            material: if self.aluminium {
                Material::ALUMINIUM
            } else {
                Material::COPPER
            },
            width_m: self.width_mm / 1000.0,
            thickness_m: self.thickness_mm / 1000.0,
            length_m: self.length_mm / 1000.0,
        };
        let mut i = Input::new(bar, self.current_a);
        i.frequency_hz = self.frequency_hz;
        i.proximity_factor = self.proximity.max(1.0);
        i.ambient_c = self.ambient_c;
        i.film = Film::Natural {
            emissivity: self.emissivity.clamp(0.0, 1.0),
            air_speed_m_s: 0.0,
            vertical: self.upright,
        };
        i.exposed_fraction = (self.exposed_pct / 100.0).clamp(0.0, 1.0);
        for (k, e) in self.end_c.iter().enumerate() {
            if let Some(c) = e {
                i.ends[k] = End::Fixed(*c);
            }
        }
        if self.joint_micro_ohm > 0.0 {
            i.joints.push(Joint {
                x_m: bar.length_m / 2.0,
                resistance_ohm: self.joint_micro_ohm * 1.0e-6,
            });
        }
        i
    }
}

/// Everything shown for one form.
#[derive(Debug, Clone)]
pub(super) struct BusbarReport {
    pub steady: Result<Output, busbar::Error>,
    /// Largest current that keeps the hottest point at the limit.
    pub max_current_a: Option<f64>,
    /// Seconds from ambient to the limit at the typed current.
    pub seconds_to_limit: Option<f64>,
}

pub(super) fn report(form: &BusbarForm) -> BusbarReport {
    let input = form.input();
    let steady = solve_steady(&input);
    let max_current_a = max_current_for_limit(&input, form.limit_c);
    // Heat-up over four time constants of the bar's own heat capacity.
    let b = input.bar;
    let tau = b.material.rho_cp * b.area_m2() / (10.0 * b.perimeter_m()).max(1.0e-9);
    let curve = busbar::heat_up(&input, None, 4.0 * tau, tau / 40.0).unwrap_or_default();
    let seconds_to_limit = time_to_reach(&curve, form.limit_c);
    BusbarReport {
        steady,
        max_current_a,
        seconds_to_limit,
    }
}

pub(super) fn duration(seconds: f64) -> String {
    if seconds < 90.0 {
        format!("{seconds:.0} s")
    } else if seconds < 5400.0 {
        format!("{:.0} min", seconds / 60.0)
    } else {
        format!("{:.1} h", seconds / 3600.0)
    }
}

/// The report as plain text, for pasting.
pub(super) fn report_text(form: &BusbarForm, r: &BusbarReport) -> String {
    let mut out = format!(
        "Busbar heat (model, not validated)\n{} {} x {} mm, {} mm long, {} A, {} Hz, air {} C\n",
        if form.aluminium {
            "Aluminium"
        } else {
            "Copper"
        },
        form.width_mm,
        form.thickness_mm,
        form.length_mm,
        form.current_a,
        form.frequency_hz,
        form.ambient_c
    );
    match &r.steady {
        Ok(o) => {
            out.push_str(&format!(
                "Hottest {:.1} C at {:.0} mm, mean {:.1} C\nLoss in the bar {:.1} W, in joints {:.1} W, into the air {:.1} W\n",
                o.peak_c,
                o.peak_x_m * 1000.0,
                o.mean_c,
                o.bar_loss_w,
                o.joint_loss_w,
                o.heat_to_air_w
            ));
        }
        Err(e) => out.push_str(&format!("{e}\n")),
    }
    match r.max_current_a {
        Some(a) => out.push_str(&format!("At most {a:.0} A for {} C\n", form.limit_c)),
        None => out.push_str("No current is allowed: the air is already above the limit\n"),
    }
    out
}

fn drag(
    ui: &mut egui::Ui,
    label: &str,
    v: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    unit: &str,
) {
    ui.label(label);
    ui.add(egui::DragValue::new(v).range(range).suffix(unit).speed(1.0));
}

pub(super) fn tab(
    ui: &mut egui::Ui,
    form: &mut BusbarForm,
    cache: &mut Option<(BusbarForm, BusbarReport)>,
    copy: &mut Option<String>,
) {
    ui.horizontal(|ui| {
        ui.label("Bar");
        badge(ui, Model::Heat);
    });
    egui::Grid::new("busbar_form")
        .num_columns(4)
        .show(ui, |ui| {
            drag(ui, "Current", &mut form.current_a, 0.0..=20000.0, " A");
            drag(ui, "Frequency", &mut form.frequency_hz, 0.0..=400.0, " Hz");
            ui.end_row();
            drag(ui, "Width", &mut form.width_mm, 5.0..=300.0, " mm");
            drag(ui, "Thickness", &mut form.thickness_mm, 1.0..=50.0, " mm");
            ui.end_row();
            drag(ui, "Length", &mut form.length_mm, 50.0..=10000.0, " mm");
            drag(ui, "Air", &mut form.ambient_c, -20.0..=120.0, " °C");
            ui.end_row();
            drag(
                ui,
                "Surface open to air",
                &mut form.exposed_pct,
                10.0..=100.0,
                " %",
            );
            drag(ui, "Emissivity", &mut form.emissivity, 0.0..=1.0, "");
            ui.end_row();
            drag(
                ui,
                "Neighbour loss factor",
                &mut form.proximity,
                1.0..=3.0,
                "",
            );
            drag(
                ui,
                "Joint in the middle",
                &mut form.joint_micro_ohm,
                0.0..=1000.0,
                " µΩ",
            );
            ui.end_row();
            drag(ui, "Limit", &mut form.limit_c, 40.0..=200.0, " °C");
            ui.end_row();
        });
    ui.horizontal(|ui| {
        ui.checkbox(&mut form.aluminium, "Aluminium");
        ui.checkbox(&mut form.upright, "Upright");
        for (k, name) in ["Cooled end A", "Cooled end B"].into_iter().enumerate() {
            let mut on = form.end_c[k].is_some();
            if ui.checkbox(&mut on, name).changed() {
                form.end_c[k] = on.then_some(form.ambient_c + 20.0);
            }
            if let Some(c) = &mut form.end_c[k] {
                ui.add(egui::DragValue::new(c).range(-20.0..=200.0).suffix(" °C"));
            }
        }
    });
    if cache.as_ref().is_none_or(|(f, _)| f != form) {
        *cache = Some((form.clone(), report(form)));
    }
    let Some((_, r)) = cache.as_ref() else {
        return;
    };
    ui.separator();
    match &r.steady {
        Ok(o) => {
            let over = o.peak_c > form.limit_c;
            let text = format!(
                "Hottest point {:.1} °C at {:.0} mm (mean {:.1} °C)",
                o.peak_c,
                o.peak_x_m * 1000.0,
                o.mean_c
            );
            ui.label(text);
            if over {
                ui.colored_label(
                    crate::theme::ui_palette().warning,
                    format!("Above the {} °C limit you typed.", form.limit_c),
                );
            }
            ui.label(format!(
                "Heat: bar {:.1} W, joints {:.1} W, into the cabinet air {:.1} W, out through the ends {:.1} W",
                o.bar_loss_w,
                o.joint_loss_w,
                o.heat_to_air_w,
                o.heat_to_ends_w[0] + o.heat_to_ends_w[1]
            ));
            if o.skin_factor > 1.001 {
                ui.label(format!(
                    "Skin effect raises the loss by {:.2}x",
                    o.skin_factor
                ));
            }
        }
        Err(e) => {
            ui.colored_label(crate::theme::ui_palette().error, e.to_string());
        }
    }
    ui.weak("Model, not a measurement. A planning estimate for the numbers typed here, not a pass or fail against a rating.");
    match r.max_current_a {
        Some(a) => ui.label(format!(
            "At most {a:.0} A keeps the hottest point at {} °C",
            form.limit_c
        )),
        None => ui.colored_label(
            crate::theme::ui_palette().error,
            "The air or a cooled end is already above the limit.",
        ),
    };
    match r.seconds_to_limit {
        Some(s) => ui.label(format!(
            "From cold it reaches {} °C after {}",
            form.limit_c,
            duration(s)
        )),
        None => ui.label(format!("From cold it stays under {} °C", form.limit_c)),
    };
    if let Ok(o) = &r.steady {
        profile(ui, o, form.limit_c);
    }
    ui.horizontal(|ui| {
        if ui.button("Copy as text").clicked() {
            *copy = Some(report_text(form, r));
        }
        ui.weak("Not covered: neighbouring phases, short-circuit heating, the standard's temperature limits.");
    });
}

/// The temperature along the bar as a small line graph with the limit.
fn profile(ui: &mut egui::Ui, o: &Output, limit_c: f64) {
    let size = egui::vec2(ui.available_width().min(520.0), 56.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    let p = crate::theme::ui_palette();
    let painter = ui.painter_at(rect);
    let lo = o
        .temp_c
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min)
        .min(limit_c)
        - 2.0;
    let hi = o
        .temp_c
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max)
        .max(limit_c)
        + 2.0;
    let len = o.x_m.last().copied().unwrap_or(1.0).max(1.0e-9);
    let at = |x: f64, t: f64| {
        egui::pos2(
            rect.left() + (x / len) as f32 * rect.width(),
            rect.bottom() - ((t - lo) / (hi - lo)) as f32 * rect.height(),
        )
    };
    painter.line_segment(
        [at(0.0, limit_c), at(len, limit_c)],
        egui::Stroke::new(1.0, p.error),
    );
    let pts: Vec<egui::Pos2> = o
        .x_m
        .iter()
        .zip(&o.temp_c)
        .map(|(x, t)| at(*x, *t))
        .collect();
    painter.text(
        at(0.0, limit_c) + egui::vec2(2.0, 1.0),
        egui::Align2::LEFT_TOP,
        format!("limit {limit_c:.0} °C"),
        egui::FontId::proportional(11.0),
        p.error,
    );
    painter.add(egui::Shape::line(pts, egui::Stroke::new(2.0, p.warning)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_bar_gives_a_sensible_answer() {
        let r = report(&BusbarForm::default());
        let o = r.steady.as_ref().unwrap();
        assert!(o.peak_c > 35.0 && o.peak_c < 105.0, "{}", o.peak_c);
        let amps = r.max_current_a.unwrap();
        assert!(amps > 400.0, "{amps}");
        assert!(report_text(&BusbarForm::default(), &r).contains("At most"));
    }

    #[test]
    fn current_over_the_limit_current_is_hot_and_reaches_the_limit_in_time() {
        let base = BusbarForm::default();
        let f = BusbarForm {
            current_a: report(&base).max_current_a.unwrap() * 1.3,
            ..base
        };
        let r = report(&f);
        assert!(r.steady.as_ref().unwrap().peak_c > f.limit_c);
        assert!(r.seconds_to_limit.is_some());
    }

    #[test]
    fn cooled_ends_and_a_joint_reach_the_solver() {
        let f = BusbarForm {
            end_c: [Some(40.0), None],
            joint_micro_ohm: 100.0,
            ..BusbarForm::default()
        };
        let i = f.input();
        assert_eq!(i.ends[0], End::Fixed(40.0));
        assert_eq!(i.joints.len(), 1);
        assert!(report(&f).steady.is_ok());
    }

    #[test]
    fn durations_read_in_a_natural_unit() {
        assert_eq!(duration(45.0), "45 s");
        assert_eq!(duration(600.0), "10 min");
        assert_eq!(duration(7200.0), "2.0 h");
    }
}
