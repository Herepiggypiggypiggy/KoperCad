//! The cabinet shell in the Heat panels (lane H7): the Shell section of the
//! Vents and fans panel (material, wall, paint, insulation, door gasket and
//! window, plate setback), each value with the word that says where it comes
//! from, and the shell results block of the Heat panel (heat through the
//! shell and the surface temperatures of the door, walls and plate).
//!
//! The setup is saved with the project as one text property on the enclosure
//! (like the openings) and becomes `Params::shell` in the run's scenario
//! through `HeatSettings::apply`. An untouched setup leaves `Params::shell`
//! empty, which is the shell calibrated to IEC 60890: results do not move.
//!
//! Honesty: every number from the shell is a model (`ModelStatus::Model`
//! until lane H6 has an exact or measured case); the words next to a value
//! are never colour alone.

use eframe::egui;
use novo_core::{Project, StructureId, Value};
use novo_layout::sizing::Material;
use novo_thermal::model::Enclosure;
use novo_thermal::shell::{
    DoorEdge, FACE_NAMES, Finish, LayerMaterial, ModelStatus, Provenance, ShellModel,
    ShellProvenance, ShellReport, Window,
};
use serde::{Deserialize, Serialize};

use super::heat_settings::HeatSettings;

/// The structure property that holds the shell setup.
pub(super) const KEY: &str = "heat.shell";

/// Where a value comes from, in the words the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    /// Today's default, nothing set for it.
    Assumed,
    /// From the chosen enclosure's record.
    Catalogue,
    /// Typed by the person.
    Typed,
    /// Typed and marked as measured on the real enclosure.
    Measured,
}

impl Source {
    pub(super) fn word(self) -> &'static str {
        match self {
            Source::Assumed => "assumed",
            Source::Catalogue => "catalogue",
            Source::Typed => "typed by you",
            Source::Measured => "measured",
        }
    }
}

/// The fields of the setup, for provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Field {
    Material,
    Wall,
    Finish,
    Insulation,
    Gasket,
    Window,
    Setback,
}

impl Field {
    const ALL: [Field; 7] = [
        Field::Material,
        Field::Wall,
        Field::Finish,
        Field::Insulation,
        Field::Gasket,
        Field::Window,
        Field::Setback,
    ];

    fn key(self) -> &'static str {
        match self {
            Field::Material => "material",
            Field::Wall => "wall",
            Field::Finish => "finish",
            Field::Insulation => "insulation",
            Field::Gasket => "gasket",
            Field::Window => "window",
            Field::Setback => "setback",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Mat {
    Steel,
    Stainless,
    Aluminium,
    Plastic,
}

impl Mat {
    const ALL: [Mat; 4] = [Mat::Steel, Mat::Stainless, Mat::Aluminium, Mat::Plastic];

    pub(super) fn material(self) -> Material {
        match self {
            Mat::Steel => Material::Steel,
            Mat::Stainless => Material::Stainless,
            Mat::Aluminium => Material::Aluminium,
            Mat::Plastic => Material::Plastic,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mat::Steel => "Steel",
            Mat::Stainless => "Stainless",
            Mat::Aluminium => "Aluminium",
            Mat::Plastic => "Plastic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Paint {
    PowderLight,
    PowderDark,
    Galvanised,
    StainlessBrushed,
    AluminiumNatural,
    Plastic,
}

impl Paint {
    const ALL: [Paint; 6] = [
        Paint::PowderLight,
        Paint::PowderDark,
        Paint::Galvanised,
        Paint::StainlessBrushed,
        Paint::AluminiumNatural,
        Paint::Plastic,
    ];

    fn finish(self) -> Finish {
        match self {
            Paint::PowderLight => Finish::PowderLight,
            Paint::PowderDark => Finish::PowderDark,
            Paint::Galvanised => Finish::Galvanised,
            Paint::StainlessBrushed => Finish::StainlessBrushed,
            Paint::AluminiumNatural => Finish::AluminiumNatural,
            Paint::Plastic => Finish::Plastic,
        }
    }
}

/// What the person set; `None` (or 0 for insulation and window) leaves the
/// default.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub(super) struct ShellSetup {
    pub material: Option<Mat>,
    /// Sheet thickness of walls and door, mm.
    pub wall_mm: Option<f64>,
    pub paint: Option<Paint>,
    /// Mineral wool on the inside of walls and door, mm; 0 = none.
    pub insulation_mm: f64,
    /// Door frame bridge, W per metre of door edge and kelvin.
    pub gasket_psi: Option<f64>,
    /// Door window, per cent of the door area; 0 = none.
    pub window_pct: f64,
    /// Air gap between the plate and the rear wall, mm.
    pub setback_mm: Option<f64>,
    /// Fields the person marked as measured.
    pub measured: Vec<String>,
    /// Fields the enclosure's record fills (set by the enclosure feed).
    pub catalogue: Vec<String>,
}

/// Planning value of the door frame bridge, W/mK, the panel starts from.
pub(super) const GASKET_START: f64 = 0.1;

impl ShellSetup {
    fn is_set(&self, f: Field) -> bool {
        match f {
            Field::Material => self.material.is_some(),
            Field::Wall => self.wall_mm.is_some(),
            Field::Finish => self.paint.is_some(),
            Field::Insulation => self.insulation_mm > 0.0,
            Field::Gasket => self.gasket_psi.is_some(),
            Field::Window => self.window_pct > 0.0,
            Field::Setback => self.setback_mm.is_some(),
        }
    }

    pub(super) fn source(&self, f: Field) -> Source {
        let key = f.key();
        if self.is_set(f) {
            if self.measured.iter().any(|m| m == key) {
                Source::Measured
            } else {
                Source::Typed
            }
        } else if self.catalogue.iter().any(|m| m == key) {
            Source::Catalogue
        } else {
            Source::Assumed
        }
    }

    fn reset(&mut self, f: Field) {
        match f {
            Field::Material => self.material = None,
            Field::Wall => self.wall_mm = None,
            Field::Finish => self.paint = None,
            Field::Insulation => self.insulation_mm = 0.0,
            Field::Gasket => self.gasket_psi = None,
            Field::Window => self.window_pct = 0.0,
            Field::Setback => self.setback_mm = None,
        }
        self.measured.retain(|m| m != f.key());
    }

    fn toggle_measured(&mut self, f: Field) {
        let key = f.key();
        if self.measured.iter().any(|m| m == key) {
            self.measured.retain(|m| m != key);
        } else {
            self.measured.push(key.to_owned());
        }
    }

    /// Nothing is set: the run uses the default shell.
    pub(super) fn is_default(&self) -> bool {
        Field::ALL.iter().all(|f| !self.is_set(*f))
    }

    /// The shell of the run: the default of `e` with what was set on top.
    pub(super) fn to_model(&self, e: &Enclosure) -> ShellModel {
        let mut shaped = e.clone();
        if let Some(m) = self.material {
            shaped.material = m.material();
        }
        if let Some(w) = self.wall_mm {
            shaped.wall_mm = w.clamp(0.5, 20.0);
        }
        let mut m = ShellModel::from_enclosure(&shaped);
        if let Some(p) = self.paint {
            for face in &mut m.faces {
                face.set_finish_out(p.finish());
            }
        }
        if self.insulation_mm > 0.0 {
            m.add_insulation(
                LayerMaterial::MineralWool,
                self.insulation_mm.min(200.0),
                true,
            );
        }
        if let Some(psi) = self.gasket_psi {
            m.door_edge = Some(DoorEdge {
                psi_w_per_mk: psi.max(0.0),
            });
        }
        if self.window_pct > 0.0 {
            m.window = Some(Window::single_glazed(
                (self.window_pct / 100.0).clamp(0.0, 0.9),
            ));
        }
        if let Some(s) = self.setback_mm {
            m.plate_setback_mm = Some(s.max(0.0));
        }
        let group = |fields: &[Field]| {
            if fields.iter().any(|f| self.source(*f) == Source::Measured) {
                Provenance::Measured
            } else if fields.iter().any(|f| self.source(*f) == Source::Catalogue) {
                Provenance::Catalogue
            } else {
                Provenance::Assumed
            }
        };
        m.provenance = ShellProvenance {
            walls: group(&[Field::Material, Field::Wall, Field::Insulation]),
            finish: group(&[Field::Finish]),
            door: group(&[Field::Gasket, Field::Window]),
            plate: group(&[Field::Setback]),
            ..m.provenance
        };
        m
    }

    /// Writes the setup into the scenario of a run.
    pub(super) fn apply(&self, s: &mut novo_thermal::Scenario) {
        if !self.is_default() {
            s.params.shell = Some(self.to_model(&s.enclosure));
        }
    }
}

/// The text the setup is stored as; empty for the default.
pub(super) fn text(s: &ShellSetup) -> String {
    if s.is_default() {
        return String::new();
    }
    // No NaN in the text: it would be stored as null and lose the value.
    let mut c = s.clone();
    for v in [&mut c.insulation_mm, &mut c.window_pct] {
        if !v.is_finite() {
            *v = 0.0;
        }
    }
    for v in [&mut c.wall_mm, &mut c.gasket_psi, &mut c.setback_mm] {
        if v.is_some_and(|x| !x.is_finite()) {
            *v = None;
        }
    }
    serde_json::to_string(&c).unwrap_or_default()
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

/// The setup in `stored`; a damaged text gives the default (the file still opens).
pub(super) fn parse(stored: &str) -> ShellSetup {
    if stored.trim().is_empty() {
        return ShellSetup::default();
    }
    serde_json::from_str(stored).unwrap_or_default()
}

fn is_damaged(stored: &str) -> bool {
    !stored.trim().is_empty() && serde_json::from_str::<ShellSetup>(stored).is_err()
}

impl super::NovoApp {
    /// Keeps the shell setup and the project's copy equal, the way
    /// `sync_inside_fans_now` does for the fans inside.
    pub(super) fn sync_shell_now(&mut self) {
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
            .shell_stored
            .as_ref()
            .filter(|(s, _)| *s == id)
            .map(|(_, t)| t.clone());
        let Some(known) = known else {
            settings.shell = parse(&held);
            settings.shell_stored = Some((id, held));
            return;
        };
        let now = text(&settings.shell);
        if known == held {
            let keep_damaged = now.is_empty() && is_damaged(&held);
            if now != held && !settings.is_editing() && !keep_damaged {
                if let Some(doc) = self.backend.document_mut() {
                    let value = (!now.is_empty()).then(|| Value::Text(now.clone()));
                    let _ = doc.transact("Shell setup", |tx| tx.set_property(id, KEY, value));
                }
                self.cabinet.heat.settings.shell_stored = Some((id, now));
            }
        } else {
            settings.shell = parse(&held);
            settings.shell_stored = Some((id, held));
        }
    }
}

// ---------------------------------------------------------------- the panel

fn source_colour(s: Source) -> egui::Color32 {
    let pal = crate::theme::ui_palette();
    match s {
        Source::Assumed => pal.warning,
        Source::Catalogue => pal.info,
        Source::Typed => pal.text_secondary,
        Source::Measured => pal.success,
    }
}

/// One row: the label, the control, and the word that says where the value
/// comes from. Clicking the word of a typed value marks it measured (and
/// back); the arrow puts the default back.
fn row(
    ui: &mut egui::Ui,
    setup: &mut ShellSetup,
    f: Field,
    label: &str,
    control: impl FnOnce(&mut egui::Ui, &mut ShellSetup) -> bool,
    editing: &mut bool,
) {
    ui.horizontal(|ui| {
        ui.label(label);
        let changed = control(ui, setup);
        if changed {
            *editing = true;
        }
        let src = setup.source(f);
        let word = egui::RichText::new(src.word())
            .small()
            .color(source_colour(src));
        if matches!(src, Source::Typed | Source::Measured) {
            if ui
                .add(egui::Button::new(word).frame(false))
                .on_hover_text(
                    "Click to mark this value as measured on the real enclosure, or back",
                )
                .clicked()
            {
                setup.toggle_measured(f);
            }
            if ui
                .add(egui::Button::new(egui::RichText::new("↺").small()).frame(false))
                .on_hover_text("Back to the default")
                .clicked()
            {
                setup.reset(f);
            }
        } else {
            ui.label(word);
        }
    });
}

/// The Shell section of the Vents and fans panel.
pub(super) fn section(ui: &mut egui::Ui, settings: &mut HeatSettings, e: &Enclosure) {
    let pal = crate::theme::ui_palette();
    ui.add_space(8.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Shell").strong());
        ui.label(
            egui::RichText::new("Model, not measurement")
                .small()
                .color(pal.text_secondary),
        );
    });
    let mut editing = false;
    let setup = &mut settings.shell;
    let steel_default = e.material;
    row(
        ui,
        setup,
        Field::Material,
        "Material",
        |ui, s| {
            let current = s.material.unwrap_or(match steel_default {
                Material::Steel => Mat::Steel,
                Material::Stainless => Mat::Stainless,
                Material::Aluminium => Mat::Aluminium,
                Material::Plastic => Mat::Plastic,
            });
            let mut chosen = current;
            egui::ComboBox::from_id_salt("shell_material")
                .selected_text(current.name())
                .show_ui(ui, |ui| {
                    for m in Mat::ALL {
                        ui.selectable_value(&mut chosen, m, m.name());
                    }
                });
            if chosen != current {
                s.material = Some(chosen);
                true
            } else {
                false
            }
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Wall,
        "Wall",
        |ui, s| {
            let mut v = s.wall_mm.unwrap_or(e.wall_mm);
            let r = ui.add(
                egui::DragValue::new(&mut v)
                    .range(0.5..=20.0)
                    .speed(0.1)
                    .suffix(" mm"),
            );
            if r.changed() {
                s.wall_mm = Some(v);
            }
            r.changed()
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Finish,
        "Paint",
        |ui, s| {
            let current = s.paint.unwrap_or(match steel_default {
                Material::Steel => Paint::PowderLight,
                Material::Stainless => Paint::StainlessBrushed,
                Material::Aluminium => Paint::AluminiumNatural,
                Material::Plastic => Paint::Plastic,
            });
            let mut chosen = current;
            egui::ComboBox::from_id_salt("shell_paint")
                .selected_text(current.finish().name())
                .show_ui(ui, |ui| {
                    for p in Paint::ALL {
                        ui.selectable_value(&mut chosen, p, p.finish().name());
                    }
                });
            if chosen != current {
                s.paint = Some(chosen);
                true
            } else {
                false
            }
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Insulation,
        "Insulation",
        |ui, s| {
            let r = ui.add(
                egui::DragValue::new(&mut s.insulation_mm)
                    .range(0.0..=200.0)
                    .speed(0.5)
                    .suffix(" mm")
                    .custom_formatter(|v, _| {
                        if v <= 0.0 {
                            "none".to_owned()
                        } else {
                            format!("{v:.0} mm")
                        }
                    }),
            );
            r.changed()
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Gasket,
        "Door frame",
        |ui, s| {
            let mut on = s.gasket_psi.is_some();
            let toggled = ui.checkbox(&mut on, "").changed();
            if toggled {
                s.gasket_psi = on.then_some(GASKET_START);
            }
            let mut changed = toggled;
            if let Some(psi) = s.gasket_psi.as_mut() {
                let r = ui.add(
                    egui::DragValue::new(psi)
                        .range(0.0..=1.0)
                        .speed(0.005)
                        .suffix(" W/mK"),
                );
                changed |= r.changed();
            }
            changed
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Window,
        "Door window",
        |ui, s| {
            ui.add(
                egui::DragValue::new(&mut s.window_pct)
                    .range(0.0..=90.0)
                    .speed(0.5)
                    .custom_formatter(|v, _| {
                        if v <= 0.0 {
                            "none".to_owned()
                        } else {
                            format!("{v:.0} % of the door")
                        }
                    }),
            )
            .changed()
        },
        &mut editing,
    );
    row(
        ui,
        setup,
        Field::Setback,
        "Plate gap",
        |ui, s| {
            let mut on = s.setback_mm.is_some();
            let toggled = ui.checkbox(&mut on, "").changed();
            if toggled {
                s.setback_mm = on.then_some(10.0);
            }
            let mut changed = toggled;
            if let Some(v) = s.setback_mm.as_mut() {
                let r = ui.add(egui::DragValue::new(v).range(0.0..=100.0).suffix(" mm"));
                changed |= r.changed();
            }
            changed
        },
        &mut editing,
    );
    if editing {
        settings.begin_shell_edit();
    }
    ui.label(
        egui::RichText::new("catalogue · typed by you · measured · assumed")
            .small()
            .color(pal.text_secondary),
    );
    super::window_help::add(
        ui.ctx(),
        "Shell: the walls, the door and the mounting plate as the heat run sees them. A value \
         with no setting is today's default for the enclosure (assumed). Typing changes it to \
         typed by you; click that word to mark it as measured on the real enclosure. The \
         result is a model: wall thickness and material change how the heat is shared between \
         the layers and the surfaces, not the IEC 60890 total, which stays the reference. \
         Door frame is a thermal bridge along the gasket (planning range 0.05 to 0.2 W/mK for \
         a foam gasket on a steel frame). Changes apply to the next run.",
    );
}

// ------------------------------------------------------------ the results

/// The paths the heat takes out through the shell, W.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Split {
    pub convection_w: f64,
    pub radiation_w: f64,
    /// Heat through the shell that is neither of the above (the part of the
    /// shell heat the outer film split does not cover).
    pub other_w: f64,
    /// Part of the door's heat that goes through the frame bridge.
    pub door_frame_w: f64,
    pub total_w: f64,
}

pub(super) fn split(r: &ShellReport) -> Split {
    let total = r.wall_heat_w();
    let convection: f64 = r.faces.iter().map(|f| f.outer_convection_w).sum();
    let radiation: f64 = r.faces.iter().map(|f| f.outer_radiation_w).sum();
    Split {
        convection_w: convection,
        radiation_w: radiation,
        other_w: (total - convection - radiation).max(0.0),
        door_frame_w: r.door().edge_heat_w,
        total_w: total,
    }
}

fn status_word(s: ModelStatus) -> &'static str {
    match s {
        ModelStatus::Model => "model",
        ModelStatus::Iec => "IEC",
        ModelStatus::Exact => "exact",
    }
}

/// The shell results: the split bar and the surface temperatures.
pub(super) fn result_block(ui: &mut egui::Ui, r: &ShellReport) {
    let pal = crate::theme::ui_palette();
    let sp = split(r);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Through the shell").small().strong());
        ui.label(
            egui::RichText::new(format!("{:.0} W · {}", sp.total_w, status_word(r.status)))
                .small()
                .color(pal.text_secondary),
        );
    });
    if sp.total_w > 0.5 {
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().min(260.0), 10.0),
            egui::Sense::hover(),
        );
        let segs = [
            (sp.convection_w, pal.info, "convection"),
            (sp.radiation_w, pal.warning, "radiation"),
            (sp.other_w, pal.text_secondary, "other"),
        ];
        let mut x = rect.left();
        for (w, colour, _) in segs {
            let width = rect.width() * (w / sp.total_w).clamp(0.0, 1.0) as f32;
            if width > 0.5 {
                ui.painter().rect_filled(
                    egui::Rect::from_min_size(
                        egui::pos2(x, rect.top()),
                        egui::vec2(width, rect.height()),
                    ),
                    0.0,
                    colour,
                );
            }
            x += width;
        }
        let mut words: Vec<String> = segs
            .iter()
            .filter(|(w, _, _)| *w >= 0.5)
            .map(|(w, _, name)| format!("{name} {w:.0} W"))
            .collect();
        if sp.door_frame_w >= 0.05 {
            words.push(format!("door frame {:.1} W", sp.door_frame_w));
        }
        ui.label(
            egui::RichText::new(words.join(" · "))
                .small()
                .color(pal.text_secondary),
        );
    }
    ui.add_space(4.0);
    egui::Grid::new("shell_faces")
        .num_columns(4)
        .spacing([10.0, 2.0])
        .show(ui, |ui| {
            for head in ["", "inside °C", "outside °C", "heat W"] {
                ui.label(egui::RichText::new(head).small().color(pal.text_secondary));
            }
            ui.end_row();
            for (i, f) in r.faces.iter().enumerate() {
                ui.label(egui::RichText::new(FACE_NAMES[i]).small());
                ui.label(egui::RichText::new(format!("{:.1}", f.inner_c)).small())
                    .on_hover_text(format!(
                        "{:.1} to {:.1} °C over the face",
                        f.inner_min_c, f.inner_max_c
                    ));
                ui.label(egui::RichText::new(format!("{:.1}", f.outer_c)).small())
                    .on_hover_text(format!(
                        "{:.1} to {:.1} °C over the face",
                        f.outer_min_c, f.outer_max_c
                    ));
                ui.label(egui::RichText::new(format!("{:.0}", f.heat_w)).small());
                ui.end_row();
            }
            if let Some(p) = r.plate_rear_c {
                ui.label(egui::RichText::new("plate rear").small());
                ui.label(egui::RichText::new(format!("{p:.1}")).small());
                ui.label("");
                ui.label("");
                ui.end_row();
            }
        });
    if let Some((inner, outer)) = r.door().window {
        ui.label(
            egui::RichText::new(format!("door window {inner:.1} / {outer:.1} °C"))
                .small()
                .color(pal.text_secondary),
        );
    }
    super::window_help::add(
        ui.ctx(),
        "Shell temperatures are a model: the inside is the air or plate next to the wall, \
         the outside is the surface the room sees. The split of the outside heat into \
         convection and radiation comes from a linearised film. The IEC 60890 total stays \
         the sealed-box reference; no value here is a measurement.",
    );
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;
    use novo_layout::Rect;
    use novo_layout::sizing::Installation;

    fn enclosure() -> Enclosure {
        Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 20.0,
                y: 20.0,
                w: 740.0,
                h: 1140.0,
            },
        }
    }

    #[test]
    fn an_untouched_setup_is_the_default_and_stores_nothing() {
        let s = ShellSetup::default();
        assert!(s.is_default());
        assert_eq!(text(&s), "");
        assert!(parse("").is_default());
        assert!(parse("not json").is_default());
        for f in Field::ALL {
            assert_eq!(s.source(f), Source::Assumed);
        }
    }

    #[test]
    fn typing_is_typed_and_measured_needs_the_mark() {
        let mut s = ShellSetup::default();
        s.wall_mm = Some(3.0);
        assert_eq!(s.source(Field::Wall), Source::Typed);
        assert_eq!(Source::Typed.word(), "typed by you");
        s.toggle_measured(Field::Wall);
        assert_eq!(s.source(Field::Wall), Source::Measured);
        s.toggle_measured(Field::Wall);
        assert_eq!(s.source(Field::Wall), Source::Typed);
        s.reset(Field::Wall);
        assert_eq!(s.source(Field::Wall), Source::Assumed);
        s.catalogue.push("wall".into());
        assert_eq!(s.source(Field::Wall), Source::Catalogue);
    }

    #[test]
    fn the_setup_round_trips_and_old_text_without_fields_opens() {
        let mut s = ShellSetup::default();
        s.material = Some(Mat::Aluminium);
        s.gasket_psi = Some(0.12);
        s.window_pct = 20.0;
        s.measured.push("gasket".into());
        assert_eq!(parse(&text(&s)), s);
        assert_eq!(parse("{\"wall_mm\":2.5}").wall_mm, Some(2.5));
    }

    #[test]
    fn the_default_leaves_the_scenario_alone_and_a_setting_fills_the_shell() {
        let e = enclosure();
        let mut scn = novo_thermal::Scenario {
            enclosure: e.clone(),
            ambient_c: 35.0,
            devices: Vec::new(),
            obstacles: Vec::new(),
            vents: Vec::new(),
            fans: Vec::new(),
            params: novo_thermal::model::Params::default(),
        };
        ShellSetup::default().apply(&mut scn);
        assert!(scn.params.shell.is_none());
        let mut s = ShellSetup::default();
        s.gasket_psi = Some(0.1);
        s.window_pct = 25.0;
        s.insulation_mm = 20.0;
        s.setback_mm = Some(15.0);
        s.apply(&mut scn);
        let m = scn.params.shell.expect("shell");
        assert_eq!(m.door_edge.map(|d| d.psi_w_per_mk), Some(0.1));
        assert!((m.window.expect("window").area_fraction - 0.25).abs() < 1e-9);
        assert_eq!(m.plate_setback_mm, Some(15.0));
        assert_eq!(m.faces[0].layers.len(), 2);
    }

    #[test]
    fn a_measured_mark_reaches_the_provenance_of_the_shell() {
        let mut s = ShellSetup::default();
        s.gasket_psi = Some(0.1);
        s.toggle_measured(Field::Gasket);
        let m = s.to_model(&enclosure());
        assert_eq!(m.provenance.door, Provenance::Measured);
        assert_eq!(m.provenance.walls, Provenance::Assumed);
    }
}
