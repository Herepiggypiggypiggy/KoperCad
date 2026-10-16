//! The door and depth side panel of the Cabinet canvas (Pro Panel gap
//! analysis, R2): the inside size and door of the enclosure as a form, the
//! depth and door-swing checks as lines, and a top view with the devices'
//! depth and the arc of the open door. All maths is `novo_layout::enclosure`.

use eframe::egui::{self, Color32, Pos2, Rect, Shape, Stroke, vec2};
use novo_layout::enclosure::{
    DEFAULT_DOOR_ANGLE_DEG, Door, DoorReport, Enclosure, Hinge, MIN_DOOR_GAP_MM,
};
use novo_layout::state::CabinetLayout;
use novo_theme::tokens;

use super::cabinet::Ask;
use crate::theme;

/// What the form starts with when an enclosure has no inside size yet.
const DEFAULT_DEPTH_MM: f64 = 250.0;
const DEFAULT_SETBACK_MM: f64 = 20.0;
/// Height of the top view drawing, in points.
const TOP_VIEW_HEIGHT: f32 = 280.0;

/// The form values, edited freely until Apply.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Draft {
    width: f64,
    height: f64,
    depth: f64,
    setback: f64,
    door: bool,
    hinge: Hinge,
    angle: f64,
    inner: f64,
    front: Option<f64>,
    hinge_side: Option<f64>,
}

impl Draft {
    fn new(enclosure: Option<&Enclosure>, layout: &CabinetLayout) -> Draft {
        match enclosure {
            Some(e) => {
                let door = e.door.unwrap_or_default();
                Draft {
                    width: e.width,
                    height: e.height,
                    depth: e.depth,
                    setback: e.plate_setback,
                    door: e.door.is_some(),
                    hinge: door.hinge,
                    angle: door.max_angle,
                    inner: door.inner_depth,
                    front: e.front_clearance,
                    hinge_side: e.hinge_side_clearance,
                }
            }
            None => Draft {
                width: layout.plate.width,
                height: layout.plate.height,
                depth: DEFAULT_DEPTH_MM,
                setback: DEFAULT_SETBACK_MM,
                door: true,
                hinge: Hinge::Left,
                angle: DEFAULT_DOOR_ANGLE_DEG,
                inner: 0.0,
                front: None,
                hinge_side: None,
            },
        }
    }

    fn enclosure(&self) -> Enclosure {
        Enclosure {
            width: self.width,
            height: self.height,
            depth: self.depth,
            plate_setback: self.setback,
            door: self.door.then_some(Door {
                hinge: self.hinge,
                max_angle: self.angle,
                inner_depth: self.inner,
            }),
            front_clearance: self.front,
            hinge_side_clearance: self.hinge_side,
        }
    }
}

/// The form with the model value it was filled from, to follow undo and
/// changes made elsewhere.
#[derive(Debug, Default)]
pub(super) struct DoorState {
    draft: Option<Draft>,
    source: Option<Option<Enclosure>>,
    /// Scroll the inspector to the form (`"cabinet.door"`) or to Apply
    /// (`"cabinet.door.apply"`) the next time the section is drawn: the
    /// form sits below the plate fields and can start out of view.
    pub(super) reveal: Option<&'static str>,
}

fn mm(ui: &mut egui::Ui, value: &mut f64, max: f64) {
    ui.add(
        egui::DragValue::new(value)
            .speed(5.0)
            .range(0.0..=max)
            .suffix(" mm"),
    );
}

/// A value that may be unknown: a check box and, when known, the number.
fn known(ui: &mut egui::Ui, value: &mut Option<f64>) {
    ui.horizontal(|ui| {
        let mut on = value.is_some();
        if ui.checkbox(&mut on, "").changed() {
            *value = on.then_some(value.unwrap_or(0.0));
        }
        if let Some(v) = value {
            mm(ui, v, 20_000.0);
        } else {
            ui.weak("unknown");
        }
    });
}

fn paint_color(c: novo_canvas::Color) -> Color32 {
    Color32::from_rgba_unmultiplied(c.0[0], c.0[1], c.0[2], c.0[3])
}

impl DoorState {
    /// The panel for the shown layout. Returns the enclosure to store when
    /// Apply was pressed.
    pub(super) fn show(
        &mut self,
        ui: &mut egui::Ui,
        layout: &CabinetLayout,
        tag_of: &impl Fn(u64) -> String,
    ) -> Option<Ask> {
        if self.source.as_ref() != Some(&layout.enclosure) || self.draft.is_none() {
            self.draft = Some(Draft::new(layout.enclosure.as_ref(), layout));
            self.source = Some(layout.enclosure);
        }
        let draft = self.draft.as_mut()?;
        let mut ask = None;
        let top = ui.cursor().left_top();

        ui.weak("Depth and door swing are checked against these. The plate is taken as centred.");
        ui.add_space(4.0);
        egui::Grid::new("cabinet_door_form")
            .num_columns(2)
            .spacing([8.0, 4.0])
            .show(ui, |ui| {
                ui.label("Inside width");
                mm(ui, &mut draft.width, 10_000.0);
                ui.end_row();
                ui.label("Inside height");
                mm(ui, &mut draft.height, 10_000.0);
                ui.end_row();
                ui.label("Inside depth");
                mm(ui, &mut draft.depth, 5_000.0);
                ui.end_row();
                ui.label("Plate setback");
                mm(ui, &mut draft.setback, 1_000.0);
                ui.end_row();
                ui.label("Door");
                ui.checkbox(&mut draft.door, "has a door");
                ui.end_row();
                if draft.door {
                    ui.label("Hinge");
                    egui::ComboBox::from_id_salt("cabinet_hinge")
                        .selected_text(draft.hinge.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut draft.hinge, Hinge::Left, "left");
                            ui.selectable_value(&mut draft.hinge, Hinge::Right, "right");
                        });
                    ui.end_row();
                    ui.label("Opens to");
                    ui.add(
                        egui::DragValue::new(&mut draft.angle)
                            .speed(1.0)
                            .range(0.0..=180.0)
                            .suffix(" °"),
                    );
                    ui.end_row();
                    ui.label("On the door");
                    mm(ui, &mut draft.inner, 500.0);
                    ui.end_row();
                    ui.label("Free in front");
                    known(ui, &mut draft.front);
                    ui.end_row();
                    ui.label("Free at hinge");
                    known(ui, &mut draft.hinge_side);
                    ui.end_row();
                }
            });
        let changed = layout.enclosure != Some(draft.enclosure());
        let valid = draft.width > 0.0 && draft.height > 0.0 && draft.depth > 0.0;
        ui.add_space(4.0);
        let apply = ui
            .add_enabled(changed && valid, egui::Button::new("Apply"))
            .on_disabled_hover_text(if valid {
                "Nothing changed"
            } else {
                "Width, height and depth are needed"
            });
        super::cabinet_bar::note_ctx(ui.ctx(), "cabinet.door.apply", "Apply", &apply);
        if apply.clicked() {
            ask = Some(Ask::SetEnclosure(draft.enclosure()));
        }
        match self.reveal.take() {
            Some("cabinet.door.apply") => {
                ui.scroll_to_rect(apply.rect, Some(egui::Align::Center));
            }
            Some(_) => {
                let form = Rect::from_min_max(top, apply.rect.right_bottom());
                ui.scroll_to_rect(form, Some(egui::Align::Min));
            }
            None => {}
        }
        ui.separator();

        match (layout.enclosure.as_ref(), layout.door_report()) {
            (Some(e), Some(report)) => {
                report_lines(ui, e, &report, tag_of);
                if let Some(swing) = layout.swing_report() {
                    for (bad, text) in swing_sentences(&swing, tag_of) {
                        line(ui, bad, text);
                    }
                }
                ui.add_space(6.0);
                top_view(ui, e, layout, &report);
            }
            _ => {
                ui.weak("Set the inside size to see the top view and the checks.");
            }
        }
        let card =
            egui::Rect::from_min_max(top, egui::pos2(ui.max_rect().right(), ui.cursor().top()));
        super::cabinet_bar::note_rect(ui.ctx(), "cabinet.door", "Door and depth", card, false);
        ask
    }
}

fn line(ui: &mut egui::Ui, bad: bool, text: String) {
    let style = theme::cabinet_style();
    let color = if bad {
        paint_color(style.error)
    } else {
        ui.visuals().weak_text_color()
    };
    ui.label(egui::RichText::new(text).color(color));
}

/// The report as sentences, problems first.
pub(super) fn sentences(
    e: &Enclosure,
    report: &DoorReport,
    tag_of: &impl Fn(u64) -> String,
) -> Vec<(bool, String)> {
    let mut out = Vec::new();
    if report.plate_too_big {
        out.push((true, "The mounting plate is bigger than the inside.".into()));
    }
    for (id, over) in &report.too_deep {
        out.push((
            true,
            format!(
                "{} is {over:.0} mm too deep: the door cannot close.",
                tag_of(*id)
            ),
        ));
    }
    for (id, gap) in &report.tight {
        out.push((
            true,
            format!(
                "{} leaves {gap:.0} mm to the door (at least {MIN_DOOR_GAP_MM:.0} mm).",
                tag_of(*id)
            ),
        ));
    }
    if let Some(swing) = report.swing {
        let need = format!("The open door needs {:.0} mm in front", swing.front);
        match (report.front_short, e.front_clearance) {
            (Some(short), Some(free)) => out.push((
                true,
                format!("{need}, {free:.0} mm are free ({short:.0} mm short)."),
            )),
            _ => out.push((false, format!("{need}."))),
        }
        if swing.hinge_side > 0.0 {
            let need = format!("It also needs {:.0} mm beside the hinge", swing.hinge_side);
            match (report.hinge_side_short, e.hinge_side_clearance) {
                (Some(short), Some(free)) => out.push((
                    true,
                    format!("{need}, {free:.0} mm are free ({short:.0} mm short)."),
                )),
                _ => out.push((false, format!("{need}."))),
            }
        }
    }
    if !report.unknown_depth.is_empty() {
        out.push((
            false,
            format!(
                "{} placed devices have no depth in their part and are not checked.",
                report.unknown_depth.len()
            ),
        ));
    }
    if report.is_clean() && report.unknown_depth.is_empty() {
        out.push((
            false,
            format!(
                "Every placed device fits behind the door ({:.0} mm usable depth).",
                e.usable_depth()
            ),
        ));
    }
    out
}

/// The door swing sweep as sentences: one per touch, or one line saying the
/// devices on the door clear everything. Nothing when no device is on the door.
pub(super) fn swing_sentences(
    report: &novo_layout::swing::SwingReport,
    tag_of: &impl Fn(u64) -> String,
) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = report
        .hits
        .iter()
        .map(|h| (true, h.message(tag_of)))
        .collect();
    for id in &report.unknown_depth {
        out.push((
            false,
            format!(
                "{} on the door has no depth and was not swept.",
                tag_of(*id)
            ),
        ));
    }
    if report.swept > 0 && !report.unchecked_plate.is_empty() {
        out.push((
            false,
            format!(
                "Not checked against the door devices: {} on the plate without a depth.",
                report.unchecked_plate.len()
            ),
        ));
    }
    if report.hits.is_empty() && report.swept > 0 && report.unchecked_plate.is_empty() {
        out.push((
            false,
            format!(
                "Devices on the door clear everything up to {:.0} degrees.",
                report.max_angle_deg
            ),
        ));
    }
    out
}

fn report_lines(
    ui: &mut egui::Ui,
    e: &Enclosure,
    report: &DoorReport,
    tag_of: &impl Fn(u64) -> String,
) {
    for (bad, text) in sentences(e, report, tag_of) {
        line(ui, bad, text);
    }
}

/// Top view: back wall at the top, the door at the bottom, the plate, the
/// devices as bars as deep as their parts, and the arc of the open door.
fn top_view(ui: &mut egui::Ui, e: &Enclosure, layout: &CabinetLayout, report: &DoorReport) {
    let style = theme::cabinet_style();
    let (error, warning) = (paint_color(style.error), paint_color(style.warning));
    let arc = e.arc(24);
    let (mut x0, mut x1) = (0.0f64, e.width);
    let mut y1 = e.depth;
    for (x, z) in &arc {
        x0 = x0.min(*x);
        x1 = x1.max(*x);
        y1 = y1.max(*z);
    }
    y1 = y1.max(e.depth + e.front_clearance.unwrap_or(0.0));
    let pad = 12.0;
    let width = ui.available_width();
    let (response, painter) =
        ui.allocate_painter(vec2(width, TOP_VIEW_HEIGHT), egui::Sense::hover());
    let area = response.rect.shrink(pad);
    let (sx, sy) = (
        f64::from(area.width()) / (x1 - x0),
        f64::from(area.height()) / y1,
    );
    let scale = sx.min(sy);
    let at = |x: f64, z: f64| -> Pos2 {
        Pos2::new(
            area.min.x + ((x - x0) * scale) as f32,
            area.min.y + (z * scale) as f32,
        )
    };
    let weak = ui.visuals().weak_text_color();
    let text = ui.visuals().text_color();
    let rect = |x: f64, z: f64, w: f64, d: f64| Rect::from_two_pos(at(x, z), at(x + w, z + d));

    // The inside, and the wall behind.
    painter.rect_stroke(
        rect(0.0, 0.0, e.width, e.depth),
        0.0,
        Stroke::new(1.0, weak),
        egui::StrokeKind::Inside,
    );
    painter.text(
        at(0.0, 0.0) + vec2(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        "back",
        egui::FontId::proportional(tokens::FONT_SMALL),
        weak,
    );

    // The plate and the devices on it.
    let left = e.plate_left(&layout.plate);
    painter.line_segment(
        [
            at(left, e.plate_setback),
            at(left + layout.plate.width, e.plate_setback),
        ],
        Stroke::new(2.0, text),
    );
    let usable = e.usable_depth();
    for p in &layout.placements {
        let f = p.footprint();
        let depth = layout.depths.get(&p.id).copied();
        let color = if report.too_deep.iter().any(|(id, _)| *id == p.id) {
            error
        } else if report.tight.iter().any(|(id, _)| *id == p.id) {
            warning
        } else {
            text
        };
        match depth {
            Some(d) => {
                let r = rect(left + f.x, e.plate_setback, f.w, d.min(usable + 40.0));
                painter.rect_filled(r, 0.0, color.gamma_multiply(0.25));
                // Not by colour alone: too deep is a thick outline, tight a
                // dashed one.
                if report.too_deep.iter().any(|(id, _)| *id == p.id) {
                    painter.rect_stroke(r, 0.0, Stroke::new(3.0, color), egui::StrokeKind::Inside);
                } else if report.tight.iter().any(|(id, _)| *id == p.id) {
                    painter.extend(Shape::dashed_line(
                        &[
                            r.left_top(),
                            r.right_top(),
                            r.right_bottom(),
                            r.left_bottom(),
                            r.left_top(),
                        ],
                        Stroke::new(1.5, color),
                        4.0,
                        3.0,
                    ));
                } else {
                    painter.rect_stroke(r, 0.0, Stroke::new(1.0, color), egui::StrokeKind::Inside);
                }
            }
            None => {
                // No depth known: a stub, so the device is still seen.
                let r = rect(left + f.x, e.plate_setback, f.w, 3.0);
                painter.rect_filled(r, 0.0, weak);
            }
        }
    }

    // The door: closed, the arc of its free edge, and open.
    let Some(door) = e.door else {
        return;
    };
    painter.line_segment(
        [at(0.0, e.depth), at(e.width, e.depth)],
        Stroke::new(3.0, text),
    );
    let points: Vec<Pos2> = arc.iter().map(|(x, z)| at(*x, *z)).collect();
    let hx = match door.hinge {
        Hinge::Left => 0.0,
        Hinge::Right => e.width,
    };
    painter.extend(Shape::dashed_line(
        &points,
        Stroke::new(1.0, weak),
        4.0,
        3.0,
    ));
    if let Some(tip) = points.last() {
        painter.line_segment([at(hx, e.depth), *tip], Stroke::new(2.0, weak));
    }
    painter.circle_filled(at(hx, e.depth), 3.0, text);

    // Free room in front, if known.
    if let Some(free) = e.front_clearance {
        let color = if report.front_short.is_some() {
            error
        } else {
            weak
        };
        let y = e.depth + free;
        painter.extend(Shape::dashed_line(
            &[at(x0, y), at(x1, y)],
            Stroke::new(1.0, color),
            3.0,
            3.0,
        ));
        painter.text(
            at(x0, y) + vec2(2.0, -2.0),
            egui::Align2::LEFT_BOTTOM,
            format!("free {free:.0} mm"),
            egui::FontId::proportional(tokens::FONT_SMALL),
            color,
        );
    }
}
