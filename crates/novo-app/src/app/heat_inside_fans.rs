//! Fans inside the cabinet (lane H8): circulation fans with no opening to the
//! outside. Listed in the 3D view's Vents and fans panel, saved with the
//! project as one text property on the enclosure (like the openings), and
//! added to the run's scenario by `HeatSettings::apply`.
//!
//! They work in the 2D thermal estimate; the 3D solver does not move air
//! with them yet, and the panel says so behind its "?".

use eframe::egui;
use novo_core::{Project, StructureId, Value};
use novo_thermal::model::{BlowDirection, CirculationFan, Enclosure};
use serde::{Deserialize, Serialize};

use super::heat_settings::HeatSettings;

/// The structure property that holds the fans inside.
pub(super) const KEY: &str = "heat.inside_fans";

/// Which way the fan blows, seen from the front of the cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) enum Dir {
    Right,
    Left,
    Up,
    Down,
    /// At the door, across the depth.
    ToDoor,
    /// At the mounting plate, across the depth.
    ToPlate,
}

impl Dir {
    pub(super) const ALL: [Dir; 6] = [
        Dir::Right,
        Dir::Left,
        Dir::Up,
        Dir::Down,
        Dir::ToPlate,
        Dir::ToDoor,
    ];

    pub(super) fn label(self) -> &'static str {
        match self {
            Dir::Right => "Right",
            Dir::Left => "Left",
            Dir::Up => "Up",
            Dir::Down => "Down",
            Dir::ToPlate => "At the plate",
            Dir::ToDoor => "At the door",
        }
    }

    fn model(self) -> BlowDirection {
        match self {
            Dir::Right => BlowDirection::Right,
            Dir::Left => BlowDirection::Left,
            Dir::Up => BlowDirection::Up,
            Dir::Down => BlowDirection::Down,
            Dir::ToPlate => BlowDirection::ToPlate,
            Dir::ToDoor => BlowDirection::ToDoor,
        }
    }
}

/// One fan inside, as the panel edits it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct InsideFan {
    /// Centre from the left edge of the inside, mm.
    pub x_mm: f64,
    /// Centre from the top edge of the inside, mm (y runs down).
    pub y_mm: f64,
    /// Distance of the fan from the mounting plate, mm; 0 = half the depth.
    pub off_plate_mm: f64,
    pub diameter_mm: f64,
    pub dir: Dir,
    /// The datasheet ends: flow in free air and shut-off pressure.
    pub flow_m3h: f64,
    pub pressure_pa: f64,
    /// 100 = as the datasheet; less when it sits behind a guard or a bend.
    pub installed_pct: f64,
}

impl Default for InsideFan {
    fn default() -> Self {
        InsideFan {
            x_mm: 0.0,
            y_mm: 0.0,
            off_plate_mm: 0.0,
            diameter_mm: 120.0,
            dir: Dir::Right,
            flow_m3h: 100.0,
            pressure_pa: 60.0,
            installed_pct: 100.0,
        }
    }
}

impl InsideFan {
    /// A fan in the middle of the mounting plate, blowing right.
    pub(super) fn start(e: &Enclosure) -> InsideFan {
        InsideFan {
            x_mm: e.plate.x + 0.5 * e.plate.w,
            y_mm: e.plate.y + 0.5 * e.plate.h,
            ..InsideFan::default()
        }
        .fitted(e)
    }

    /// Every number finite (a damaged file or a typed NaN falls back to the start).
    pub(super) fn finite(mut self) -> InsideFan {
        let d = InsideFan::default();
        for (v, s) in [
            (&mut self.x_mm, 0.0),
            (&mut self.y_mm, 0.0),
            (&mut self.off_plate_mm, d.off_plate_mm),
            (&mut self.diameter_mm, d.diameter_mm),
            (&mut self.flow_m3h, d.flow_m3h),
            (&mut self.pressure_pa, d.pressure_pa),
            (&mut self.installed_pct, d.installed_pct),
        ] {
            if !v.is_finite() {
                *v = s;
            }
        }
        self
    }

    /// Moved and cut down until it lies inside the cabinet.
    pub(super) fn fitted(self, e: &Enclosure) -> InsideFan {
        let mut f = self.finite();
        let (w, h, d) = (
            e.interior_width_mm(),
            e.interior_height_mm(),
            e.interior_depth_mm(),
        );
        f.diameter_mm = f.diameter_mm.clamp(30.0, w.min(h).max(30.0));
        f.x_mm = f.x_mm.clamp(0.0, w);
        f.y_mm = f.y_mm.clamp(0.0, h);
        f.off_plate_mm = f.off_plate_mm.clamp(0.0, d);
        f.flow_m3h = f.flow_m3h.max(1.0);
        f.pressure_pa = f.pressure_pa.max(1.0);
        f.installed_pct = f.installed_pct.clamp(10.0, 150.0);
        f
    }

    /// The fan in the solver's terms.
    pub(super) fn to_model(self, e: &Enclosure) -> CirculationFan {
        let f = self.fitted(e);
        CirculationFan {
            x_mm: f.x_mm,
            y_mm: f.y_mm,
            diameter_mm: f.diameter_mm,
            direction: f.dir.model(),
            free_flow_m3h: f.flow_m3h,
            max_pressure_pa: f.pressure_pa,
            installed_factor: f.installed_pct / 100.0,
            z_mm: (f.off_plate_mm > 0.0).then_some(f.off_plate_mm),
        }
    }
}

/// The text the fans are stored as; empty for none.
pub(super) fn text(fans: &[InsideFan]) -> String {
    if fans.is_empty() {
        String::new()
    } else {
        let clean: Vec<InsideFan> = fans.iter().map(|f| f.finite()).collect();
        serde_json::to_string(&clean).unwrap_or_default()
    }
}

/// What the project holds for `id` as text; empty for none.
pub(super) fn stored_text(project: &Project, id: StructureId) -> String {
    match project
        .get::<novo_core::Structure>(id)
        .and_then(|s| s.properties.get(KEY))
    {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// The fans in `stored`; a damaged text gives none (the file still opens).
pub(super) fn parse(stored: &str) -> Vec<InsideFan> {
    if stored.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(stored).unwrap_or_default()
}

fn is_damaged(stored: &str) -> bool {
    !stored.trim().is_empty() && serde_json::from_str::<Vec<InsideFan>>(stored).is_err()
}

impl super::NovoApp {
    /// Keeps the list of fans inside and the project's copy equal, the way
    /// `sync_openings_now` does for the openings: a change on the panel is
    /// written once the gesture ends; a project that changed under the panel
    /// (opened, undone) is read back.
    pub(super) fn sync_inside_fans_now(&mut self) {
        let Some(document) = self.backend.document() else {
            return;
        };
        let Some(id) = super::view3d::enclosure(document.project(), self.cabinet.location.as_ref())
        else {
            return;
        };
        let held = stored_text(document.project(), id);
        let settings = &mut self.cabinet.heat.settings;
        let known = settings
            .inside_stored
            .as_ref()
            .filter(|(s, _)| *s == id)
            .map(|(_, t)| t.clone());
        let Some(known) = known else {
            settings.inside = parse(&held);
            settings.inside_stored = Some((id, held));
            return;
        };
        let now = text(&settings.inside);
        if known == held {
            let keep_damaged = now.is_empty() && is_damaged(&held);
            if now != held && !settings.is_editing() && !keep_damaged {
                if let Some(doc) = self.backend.document_mut() {
                    let value = (!now.is_empty()).then(|| Value::Text(now.clone()));
                    let _ = doc.transact("Fans inside", |tx| tx.set_property(id, KEY, value));
                }
                self.cabinet.heat.settings.inside_stored = Some((id, now));
            }
        } else {
            settings.inside = parse(&held);
            settings.inside_stored = Some((id, held));
        }
    }
}

/// The "Fans inside" part of the Vents and fans panel.
pub(super) fn section(ui: &mut egui::Ui, settings: &mut HeatSettings, e: &Enclosure) {
    let pal = crate::theme::ui_palette();
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fans inside").strong());
        super::dialog_ui::help(
            ui,
            "Circulation fans with no opening to the outside: they stir the air and blow on \
             parts. Enter the maker's flow in free air and shut-off pressure. The 2D estimate \
             shows the air they move and the cooler film on parts in their stream; the \
             flow through the fan is set by the loop the air finds, not by a vent. Model, \
             not measured: how much a given fan lowers a given part. The 3D solver does not \
             move air with them yet.",
        );
        ui.label(
            egui::RichText::new("(model)")
                .small()
                .color(pal.text_secondary),
        );
        if ui
            .button("Add fan inside")
            .on_hover_text("A fan on the mounting plate; move it with the fields below")
            .clicked()
        {
            settings.inside.push(InsideFan::start(e));
            settings.inside_selected = Some(settings.inside.len() - 1);
        }
    });
    if !settings.inside.is_empty() {
        ui.label(
            egui::RichText::new(
                "Model, not measurement. Flow and pressure are the maker's free-air values; \
                 Installed lowers them. A new fan starts with assumed values.",
            )
            .small()
            .color(pal.text_secondary),
        );
    }
    let mut remove = None;
    for i in 0..settings.inside.len() {
        let f = settings.inside[i];
        let open = settings.inside_selected == Some(i);
        ui.horizontal(|ui| {
            let title = format!(
                "{}  Fan inside, {} (model)",
                i + 1,
                f.dir.label().to_lowercase()
            );
            if ui.selectable_label(open, title).clicked() {
                settings.inside_selected = if open { None } else { Some(i) };
            }
        });
        if open {
            let mut edited = f;
            egui::Frame::group(ui.style()).show(ui, |ui| {
                editor(ui, &mut edited, i, e);
                if ui.button("Remove").clicked() {
                    remove = Some(i);
                }
            });
            let edited = edited.fitted(e);
            if edited != f && remove.is_none() {
                settings.begin_inside_edit();
                settings.inside[i] = edited;
            }
        }
    }
    if let Some(i) = remove {
        settings.inside.remove(i);
        settings.inside_selected = None;
    }
}

fn editor(ui: &mut egui::Ui, f: &mut InsideFan, i: usize, e: &Enclosure) {
    let (w, h, d) = (
        e.interior_width_mm(),
        e.interior_height_mm(),
        e.interior_depth_mm(),
    );
    egui::Grid::new(("inside_fan", i))
        .num_columns(2)
        .spacing([8.0, 4.0])
        .show(ui, |ui| {
            ui.label("Blows");
            egui::ComboBox::from_id_salt(("inside_fan_dir", i))
                .selected_text(f.dir.label())
                .show_ui(ui, |ui| {
                    for dir in Dir::ALL {
                        ui.selectable_value(&mut f.dir, dir, dir.label());
                    }
                });
            ui.end_row();
            ui.label("From the left");
            ui.add(
                egui::DragValue::new(&mut f.x_mm)
                    .range(0.0..=w)
                    .speed(2.0)
                    .max_decimals(0)
                    .suffix(" mm"),
            )
            .on_hover_text("Centre of the fan from the left edge of the inside");
            ui.end_row();
            ui.label("Height above floor");
            let mut up = h - f.y_mm;
            if ui
                .add(
                    egui::DragValue::new(&mut up)
                        .range(0.0..=h)
                        .speed(2.0)
                        .max_decimals(0)
                        .suffix(" mm"),
                )
                .on_hover_text("Centre of the fan above the cabinet floor")
                .changed()
            {
                f.y_mm = h - up;
            }
            ui.end_row();
            ui.label("Off the plate");
            ui.add(
                egui::DragValue::new(&mut f.off_plate_mm)
                    .range(0.0..=d)
                    .speed(1.0)
                    .max_decimals(0)
                    .suffix(" mm"),
            )
            .on_hover_text(
                "Distance of the fan from the mounting plate. 0 = half the depth. A fan whose jet \
                 skims the fronts of the devices cools them more than one far from them.",
            );
            ui.end_row();
            ui.label("Diameter");
            ui.add(
                egui::DragValue::new(&mut f.diameter_mm)
                    .range(30.0..=w.min(h).max(30.0))
                    .speed(1.0)
                    .max_decimals(0)
                    .suffix(" mm"),
            );
            ui.end_row();
            ui.label("Flow, pressure");
            ui.horizontal(|ui| {
                ui.add(
                    egui::DragValue::new(&mut f.flow_m3h)
                        .range(1.0..=5000.0)
                        .speed(1.0)
                        .max_decimals(0)
                        .suffix(" m³/h"),
                )
                .on_hover_text("Flow in free air, from the maker's datasheet");
                ui.add(
                    egui::DragValue::new(&mut f.pressure_pa)
                        .range(1.0..=2000.0)
                        .speed(1.0)
                        .max_decimals(0)
                        .suffix(" Pa"),
                )
                .on_hover_text("Maximum pressure: where the fan delivers nothing (shut-off)");
            });
            ui.end_row();
            ui.label("Installed");
            ui.add(
                egui::DragValue::new(&mut f.installed_pct)
                    .range(10.0..=150.0)
                    .speed(1.0)
                    .max_decimals(0)
                    .suffix(" %"),
            )
            .on_hover_text(
                "100 % = the datasheet curve. Lower it for a guard, a filter or a tight spot: \
                 both ends of the curve are scaled.",
            );
            ui.end_row();
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fans_round_trip_through_text() {
        let a = InsideFan {
            x_mm: 321.5,
            dir: Dir::ToPlate,
            ..InsideFan::default()
        };
        let b = InsideFan::default();
        assert_eq!(parse(&text(&[a, b])), vec![a, b]);
        assert_eq!(text(&[]), "");
        assert!(parse("").is_empty());
        assert!(parse("{broken").is_empty());
    }

    #[test]
    fn an_unknown_or_missing_field_does_not_stop_the_project_opening() {
        let v = parse(r#"[{"dir":"Left","future":1}]"#);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].dir, Dir::Left);
        assert_eq!(v[0].diameter_mm, InsideFan::default().diameter_mm);
    }

    #[test]
    fn a_nan_is_not_stored() {
        let f = InsideFan {
            x_mm: f64::NAN,
            ..InsideFan::default()
        };
        let t = text(&[f]);
        assert_eq!(parse(&t).len(), 1);
        assert!(!t.contains("null"));
    }
}
