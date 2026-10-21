//! Wire routes on the Cabinet canvas (WS-16): every routed wire as a line
//! over the plate and every duct tinted by how full it is, with the fill as a
//! percentage so colour is never the only cue. The numbers come from
//! novo-routing; this file only turns them into lines and rectangles.

use eframe::egui;

use super::cabinet::Ask;
use novo_canvas::Viewport;
use novo_core::{Project, Rail, RailKind, StructureId};
use novo_routing::model::route_project;
use novo_routing::{Kind, WireRules, project_rules};
use novo_theme::tokens;

/// One routed wire, as a polyline in plate millimetres.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RouteLine {
    /// The wire's id.
    pub(super) wire: u64,
    pub(super) points: Vec<(f32, f32)>,
    /// No duct reaches both ends: drawn dashed and red.
    pub(super) free_air: bool,
    pub(super) length_mm: f64,
}

/// A duct and how full it is.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DuctFill {
    pub(super) rail: u64,
    /// Top left and size on the plate, in mm.
    pub(super) rect: (f32, f32, f32, f32),
    /// Used over allowed fill, 0 for a duct nothing runs in; `None` when the
    /// duct has no cross-section yet.
    pub(super) ratio: Option<f64>,
    pub(super) wires: usize,
}

/// One row of the route table: a wire, where it goes and how long it is.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct RouteRow {
    pub(super) wire: u64,
    /// The wire number, or `#id` while it has none.
    pub(super) label: String,
    /// `-K1:2 > -K2:1`.
    pub(super) ends: String,
    /// The cut length, rounded up to a whole millimetre.
    pub(super) length_mm: f64,
    /// `routed`, `routed, no depth` or `free air`.
    pub(super) basis: &'static str,
    pub(super) ducts: usize,
}

/// Everything drawn over the plate for the routes.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Overlay {
    pub(super) routes: Vec<RouteLine>,
    pub(super) ducts: Vec<DuctFill>,
    /// The routes as a table, longest first.
    pub(super) rows: Vec<RouteRow>,
    /// What the designer has to look at, in words.
    pub(super) notes: Vec<String>,
    /// The project's wire production rules the lengths were made with.
    pub(super) rules: WireRules,
    /// The plate's size and its entry point (where outside wires arrive).
    pub(super) plate: Option<(f64, f64)>,
    pub(super) entry: Option<(f64, f64)>,
    /// A point is stored but cannot be used (outside a smaller plate, or
    /// only one number).
    pub(super) entry_hidden: bool,
}

impl Overlay {
    /// One line for the header strip: routed wires, those in free air, the
    /// fullest duct.
    pub(super) fn summary(&self) -> String {
        let free = self.routes.iter().filter(|r| r.free_air).count();
        let mut text = format!(
            "{} {} routed",
            self.routes.len(),
            if self.routes.len() == 1 {
                "wire"
            } else {
                "wires"
            }
        );
        if free > 0 {
            text.push_str(&format!(", {free} in free air"));
        }
        let fullest = self
            .ducts
            .iter()
            .filter_map(|d| d.ratio)
            .fold(None, |m: Option<f64>, r| Some(m.map_or(r, |m| m.max(r))));
        if let Some(r) = fullest {
            text.push_str(&format!(", fullest duct {:.0}%", r * 100.0));
        }
        text
    }
}

/// Routes the wires of `structure`'s devices and reads the result as an
/// overlay. Empty when no wire has both ends placed on this plate.
pub(super) fn overlay(project: &Project, structure: StructureId) -> Overlay {
    let routed = route_project(project);
    let rules = WireRules::of(project);
    let plate = project
        .get::<novo_core::Structure>(structure)
        .and_then(|s| s.plate)
        .map(|p| (p.width, p.height));
    let entry = novo_routing::entry::entry_point(project, structure).map(|p| (p.x, p.y));
    let entry_hidden = entry.is_none() && novo_routing::entry::has_stored_point(project, structure);
    let Some((_, plan)) = routed.plans.iter().find(|(s, _)| *s == structure) else {
        return Overlay {
            rules,
            plate,
            entry,
            entry_hidden,
            ..Overlay::default()
        };
    };
    let rows = rows(project, &routed, plan);
    let notes = notes(plan, &routed, &rows);
    let routes = plan
        .routes
        .iter()
        .map(|r| RouteLine {
            wire: r.job,
            points: r.points.iter().map(|p| (p.x as f32, p.y as f32)).collect(),
            free_air: r.kind == Kind::FreeAir,
            length_mm: r.length_mm,
        })
        .collect();
    let ducts = duct_fills(project, structure, plan);
    Overlay {
        routes,
        ducts,
        rows,
        notes,
        rules,
        plate,
        entry,
        entry_hidden,
    }
}

/// Every duct of `structure` with how full the plan leaves it.
pub(super) fn duct_fills(
    project: &Project,
    structure: StructureId,
    plan: &novo_routing::Plan,
) -> Vec<DuctFill> {
    project
        .all::<Rail>()
        .filter(|r| r.structure == structure && r.kind == RailKind::Duct)
        .map(|r| {
            let fill = plan.fill.iter().find(|f| f.duct == r.id.raw());
            let sized = r.width > 0.0 && r.height > 0.0;
            let (w, h) = if r.vertical {
                (r.width, r.length)
            } else {
                (r.length, r.width)
            };
            // The duct's line runs down its middle.
            let (x, y) = if r.vertical {
                (r.x - r.width / 2.0, r.y)
            } else {
                (r.x, r.y - r.width / 2.0)
            };
            DuctFill {
                rail: r.id.raw(),
                rect: (x as f32, y as f32, w as f32, h as f32),
                ratio: sized.then(|| fill.map_or(0.0, |f| f.ratio)),
                wires: fill.map_or(0, |f| f.jobs.len()),
            }
        })
        .collect()
}

/// The route table: longest wire first, then by wire id.
fn rows(
    project: &Project,
    routed: &novo_routing::model::Routed,
    plan: &novo_routing::Plan,
) -> Vec<RouteRow> {
    let by_id: std::collections::HashMap<u64, &novo_core::Wire> =
        project.wires().map(|w| (w.id.raw(), w)).collect();
    let mut rows: Vec<RouteRow> = plan
        .routes
        .iter()
        .map(|r| {
            let wire = by_id.get(&r.job);
            let label = wire
                .and_then(|w| w.number.clone())
                .unwrap_or_else(|| format!("#{}", r.job));
            let ends = wire
                .and_then(|w| Some((w.from.as_pin()?, w.to.as_pin()?)))
                .map(|(a, b)| {
                    format!(
                        "{} > {}",
                        project.pin_designation(a).unwrap_or_default(),
                        project.pin_designation(b).unwrap_or_default()
                    )
                })
                .unwrap_or_default();
            let to_door = routed
                .surface_legs
                .get(&r.job)
                .is_some_and(|leg| leg.opening.is_some());
            let basis = match r.kind {
                _ if to_door => "door, no gap",
                Kind::FreeAir => "free air",
                Kind::Ducted if routed.unknown_height.contains(&r.job) => "routed, no depth",
                Kind::Ducted => "routed",
            };
            RouteRow {
                wire: r.job,
                label,
                ends,
                length_mm: r.length_mm.ceil().max(1.0),
                basis,
                ducts: r.ducts.len(),
            }
        })
        .collect();
    rows.sort_by(|a, b| {
        b.length_mm
            .total_cmp(&a.length_mm)
            .then(a.wire.cmp(&b.wire))
    });
    rows
}

/// The plan's problems and the unknown depths, as sentences.
fn notes(
    plan: &novo_routing::Plan,
    routed: &novo_routing::model::Routed,
    rows: &[RouteRow],
) -> Vec<String> {
    use novo_routing::Problem;
    let plural =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let count = |f: &dyn Fn(&Problem) -> bool| plan.problems.iter().filter(|p| f(p)).count();
    let mut notes = Vec::new();
    let free = count(&|p| matches!(p, Problem::FreeAir { .. }));
    if free > 0 {
        notes.push(format!(
            "{} no duct reaches: they run in free air",
            plural(free, "wire", "wires")
        ));
    }
    let overfull = count(&|p| matches!(p, Problem::OverfullDuct { .. }));
    if overfull > 0 {
        notes.push(format!(
            "{} over the fill limit",
            plural(overfull, "duct is", "ducts are")
        ));
    }
    let no_size = count(&|p| matches!(p, Problem::UnsizedDuct { .. }));
    if no_size > 0 {
        notes.push(format!(
            "{} no size, so its fill is not checked",
            plural(no_size, "duct has", "ducts have")
        ));
    }
    let guessed = count(&|p| matches!(p, Problem::GuessedWire { .. }));
    if guessed > 0 {
        notes.push(format!(
            "{} no size: duct fill assumes 3 mm",
            plural(guessed, "wire has", "wires have")
        ));
    }
    let flat = rows
        .iter()
        .filter(|r| routed.unknown_height.contains(&r.wire))
        .count();
    if flat > 0 {
        notes.push(format!(
            "{} measured from the plate: a part has no depth",
            plural(flat, "wire is", "wires are")
        ));
    }
    notes
}

/// How a duct reads on the plate. The colour is never the only cue: each
/// tone has its own label ([`duct_label`]) and the over-full one says so in
/// words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tone {
    /// Nothing runs in it: drawn faintly, no label.
    Idle,
    /// Up to 80% of the allowed fill.
    Ok,
    /// Above 80%, up to the limit.
    Busy,
    /// Above the allowed fill.
    Over,
    /// Wires run in it but it has no cross-section yet.
    NoSize,
}

pub(super) fn tone(d: &DuctFill) -> Tone {
    match d.ratio {
        _ if d.wires == 0 => Tone::Idle,
        None => Tone::NoSize,
        Some(r) if r > 1.0 => Tone::Over,
        Some(r) if r > 0.8 => Tone::Busy,
        Some(_) => Tone::Ok,
    }
}

fn colour(tone: Tone) -> egui::Color32 {
    match tone {
        Tone::Idle | Tone::NoSize => crate::theme::ui_palette().text_secondary,
        Tone::Ok => crate::theme::ui_palette().success,
        Tone::Busy => crate::theme::ui_palette().warning,
        Tone::Over => crate::theme::ui_palette().error,
    }
}

/// The text on a duct, longest form first so the caller can take the first
/// that fits: the fill and the wires in it (`62% · 4`), then the fill alone
/// (`62%`); `over 112% · 9` past the limit, `no size · 3` without a
/// cross-section. Empty for a duct nothing runs in.
pub(super) fn duct_labels(d: &DuctFill) -> Vec<String> {
    let percent = |r: f64| (r * 100.0).round();
    match (tone(d), d.ratio) {
        (Tone::Idle, _) => Vec::new(),
        (Tone::NoSize, _) | (_, None) => vec![format!("no size · {}", d.wires), "no size".into()],
        (Tone::Over, Some(r)) => vec![
            format!("over {:.0}% · {}", percent(r), d.wires),
            format!("over {:.0}%", percent(r)),
            format!("{:.0}%", percent(r)),
        ],
        (_, Some(r)) => vec![
            format!("{:.0}% · {}", percent(r), d.wires),
            format!("{:.0}%", percent(r)),
        ],
    }
}

/// The overlay of the panel shown, rebuilt only when the project changed or
/// another panel is shown: routing walks every wire, which is too much work
/// for every frame.
#[derive(Default)]
pub(super) struct OverlayCache {
    key: Option<(u64, StructureId)>,
    overlay: Overlay,
}

impl OverlayCache {
    pub(super) fn get(&mut self, project: &Project, structure: StructureId) -> &Overlay {
        let key = (project.revision(), structure);
        if self.key != Some(key) {
            self.overlay = overlay(project, structure);
            self.key = Some(key);
        }
        &self.overlay
    }
}

/// Room (points) a label of `chars` characters needs on screen: about 6
/// points a character and a line of text high; too small and the tint and
/// outline alone show the duct. A narrow vertical duct has no room for text
/// at all (nothing is rotated).
fn fits(area: egui::Rect, label: &str) -> bool {
    area.width() >= 6.0 * label.chars().count() as f32 + 6.0 && area.height() >= 14.0
}

/// The route table beside the plate: the summary, what to look at, the
/// export buttons and every wire with its length. Returns what was clicked.
pub(super) fn panel(ui: &mut egui::Ui, overlay: &Overlay, plate: StructureId) -> Option<Ask> {
    let mut ask = None;
    let panel = egui::Panel::right("cabinet_routes")
        .exact_size(340.0)
        .resizable(false)
        .show(ui, |ui| {
            ui.add_space(6.0);
            ui.strong("Routes");
            if let Some(a) = rules_section(ui, &overlay.rules) {
                ask = Some(a);
            }
            if let Some(a) = entry_section(ui, overlay, plate) {
                ask = Some(a);
            }
            if overlay.rows.is_empty() {
                ui.weak("No wire has both ends placed on this plate");
                return;
            }
            ui.weak(overlay.summary());
            for note in &overlay.notes {
                ui.colored_label(crate::theme::ui_palette().warning, note);
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Export wire cut")
                    .on_hover_text("Cut list for a wire machine: CSV next to the project")
                    .clicked()
                {
                    ask = Some(Ask::ExportWireCut);
                }
                if ui
                    .button("Workbook")
                    .on_hover_text("Cut list, wire stock, ferrules and duct fill as .xlsx")
                    .clicked()
                {
                    ask = Some(Ask::ExportWireProduction);
                }
                if ui
                    .button("Bench output")
                    .on_hover_text("Bench cut list, ferrule table and label sheets to print")
                    .clicked()
                {
                    ask = Some(Ask::OpenBench);
                }
                if ui
                    .button("DXF plan")
                    .on_hover_text("The plate with ducts, fill and routes as a DXF drawing")
                    .clicked()
                {
                    ask = Some(Ask::ExportWiringDxf(plate));
                }
            });
            ui.add_space(4.0);
            let row_height = ui.text_style_height(&egui::TextStyle::Body) * 2.0 + 8.0;
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(
                ui,
                row_height,
                overlay.rows.len(),
                |ui, range| {
                    for row in &overlay.rows[range] {
                        ui.horizontal(|ui| {
                            ui.label(&row.label);
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(format!("{:.0} mm", row.length_mm));
                                },
                            );
                        });
                        ui.weak(format!(
                            "{} · {}{}",
                            row.ends,
                            row.basis,
                            if row.ducts > 0 {
                                format!(" · {} duct(s)", row.ducts)
                            } else {
                                String::new()
                            }
                        ));
                        ui.add_space(2.0);
                    }
                },
            );
        });
    super::cabinet_bar::note_rect(
        ui.ctx(),
        "cabinet.routes_panel",
        "Routes",
        panel.response.rect,
        false,
    );
    ask
}

/// The cabinet entry: the point on the plate where wires from outside (a
/// supply feed, a cable to the field) will come in. It is kept in the
/// project, but no length uses it yet. Unset shows "not set" and a button, so
/// no default looks like a choice; both numbers are kept together.
fn entry_section(ui: &mut egui::Ui, overlay: &Overlay, plate: StructureId) -> Option<Ask> {
    let (width, height) = overlay.plate?;
    let mut ask = None;
    ui.horizontal(|ui| {
        ui.label("Cabinet entry");
        let Some(set) = overlay.entry else {
            ui.weak("not set");
            if ui
                .small_button("Set")
                .on_hover_text("Start at the bottom middle of the plate")
                .clicked()
            {
                ask = Some(Ask::SetEntryPoint(plate, Some((width / 2.0, height))));
            }
            return;
        };
        let id = egui::Id::new(("cabinet_entry", plate));
        let (mut x, mut y) = ui.data(|d| d.get_temp::<(f64, f64)>(id)).unwrap_or(set);
        let rx = ui.add(
            egui::DragValue::new(&mut x)
                .range(0.0..=width)
                .prefix("x ")
                .suffix(" mm"),
        );
        let ry = ui.add(
            egui::DragValue::new(&mut y)
                .range(0.0..=height)
                .prefix("y ")
                .suffix(" mm"),
        );
        if rx.changed() || ry.changed() {
            ui.data_mut(|d| d.insert_temp(id, (x, y)));
        }
        let done = rx.drag_stopped() || rx.lost_focus() || ry.drag_stopped() || ry.lost_focus();
        if done && ui.data(|d| d.get_temp::<(f64, f64)>(id)).is_some() {
            ui.data_mut(|d| d.remove_temp::<(f64, f64)>(id));
            if (x, y) != set {
                ask = Some(Ask::SetEntryPoint(plate, Some((x, y))));
            }
        }
        if ui
            .small_button("\u{21ba}")
            .on_hover_text("Remove the entry point")
            .clicked()
        {
            ask = Some(Ask::SetEntryPoint(plate, None));
        }
    });
    if overlay.entry_hidden {
        ui.colored_label(
            crate::theme::ui_palette().warning,
            "A stored entry point lies outside the plate and is ignored. Set a new one.",
        );
    }
    ui.weak(
        "Where wires from outside will come in (origin top left). Not used for wire lengths yet.",
    );
    ask
}

/// The wire production rules, kept in the project: service loop, duct
/// reach and fill limit, end allowances, estimate constants. Collapsed by
/// default; a changed value is kept when the drag ends or the entry loses
/// focus, so one edit is one undo step. A reset arrow puts a rule back to
/// its default.
fn rules_section(ui: &mut egui::Ui, rules: &WireRules) -> Option<Ask> {
    let mut ask = None;
    let custom = project_rules::RULES
        .iter()
        .filter(|r| rules.value(r.key).is_some_and(|v| v != r.default))
        .count();
    let title = if custom == 0 {
        "Wire rules".to_owned()
    } else {
        format!("Wire rules ({custom} changed)")
    };
    let section = egui::CollapsingHeader::new(title)
        .id_salt("cabinet_wire_rules")
        .default_open(false)
        .show(ui, |ui| {
            ui.weak(
                "Kept in the project. They set cut lengths, the wire list and the wire cut file.",
            );
            for r in &project_rules::RULES {
                let Some(current) = rules.value(r.key) else {
                    continue;
                };
                let id = egui::Id::new(("wire_rule", r.key));
                ui.horizontal(|ui| {
                    ui.label(r.label);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if current != r.default
                            && ui
                                .small_button("\u{21ba}")
                                .on_hover_text(format!("Back to {} {}", r.default, r.unit))
                                .clicked()
                        {
                            ask = Some(Ask::SetWireRule(r.key, None));
                        }
                        let mut value = ui.data(|d| d.get_temp::<f64>(id)).unwrap_or(current);
                        let speed = ((r.max - r.min) / 400.0).clamp(0.01, 5.0);
                        let response = ui.add(
                            egui::DragValue::new(&mut value)
                                .range(r.min..=r.max)
                                .speed(speed)
                                .suffix(format!(" {}", r.unit)),
                        );
                        if response.changed() {
                            ui.data_mut(|d| d.insert_temp(id, value));
                        }
                        if (response.drag_stopped() || response.lost_focus())
                            && ui.data(|d| d.get_temp::<f64>(id)).is_some()
                        {
                            ui.data_mut(|d| d.remove_temp::<f64>(id));
                            if value != current {
                                ask = Some(Ask::SetWireRule(r.key, Some(value)));
                            }
                        }
                    });
                });
            }
            if !rules.ignored.is_empty() {
                ui.colored_label(
                    crate::theme::ui_palette().warning,
                    format!(
                        "{} in the file could not be read and use the default",
                        rules.ignored.join(", ")
                    ),
                );
            }
        });
    let rect = section
        .body_response
        .as_ref()
        .map_or(section.header_response.rect, |b| {
            section.header_response.rect.union(b.rect)
        });
    super::cabinet_bar::note_rect(ui.ctx(), "cabinet.wire_rules", "Wire rules", rect, false);
    ask
}

/// Paints the overlay. `rect` is the canvas widget's screen rectangle.
pub(super) fn draw(painter: &egui::Painter, rect: egui::Rect, view: &Viewport, overlay: &Overlay) {
    let to_screen = |x: f32, y: f32| {
        let s = view.world_to_screen(novo_canvas::vec2(x, y));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    for d in &overlay.ducts {
        let (x, y, w, h) = d.rect;
        let area = egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + w, y + h));
        let tone = tone(d);
        let c = colour(tone);
        let wash = if tone == Tone::Idle { 0.1 } else { 0.25 };
        painter.rect_filled(area, 0.0, c.gamma_multiply(wash));
        if tone != Tone::Idle {
            painter.rect_stroke(
                area,
                0.0,
                egui::Stroke::new(1.0, c.gamma_multiply(0.7)),
                egui::StrokeKind::Inside,
            );
        }
        if let Some(label) = duct_labels(d).into_iter().find(|l| fits(area, l)) {
            painter.text(
                area.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(tokens::FONT_SMALL),
                c,
            );
        }
    }
    for r in &overlay.routes {
        let points: Vec<egui::Pos2> = r.points.iter().map(|&(x, y)| to_screen(x, y)).collect();
        if r.free_air {
            let stroke = egui::Stroke::new(1.5, crate::theme::ui_palette().error);
            painter.extend(egui::Shape::dashed_line(&points, stroke, 6.0, 4.0));
        } else {
            let stroke = egui::Stroke::new(1.5, crate::theme::ui_palette().info);
            painter.add(egui::Shape::line(points, stroke));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{
        DeviceTag, Direction, MountingPlate, Pin, PinRef, Placement, Point, SymbolRole, Value,
    };

    /// Two devices on a plate joined by a wire, with a vertical duct of the
    /// given size between them (none when `duct` is `None`).
    fn cabinet(duct: Option<(f64, f64)>) -> (Project, StructureId) {
        let mut p = Project::new("t");
        let (cab, _) = p
            .transact("setup", |tx| {
                let cab = tx.add_structure(DeviceTag::parse("+CAB1")?, "", true)?;
                tx.set_plate(
                    cab,
                    Some(MountingPlate {
                        width: 400.0,
                        height: 400.0,
                        margin: 10.0,
                    }),
                )?;
                if let Some((w, h)) = duct {
                    let d = tx.add_rail(cab, 150.0, 100.0, 200.0, RailKind::Duct)?;
                    tx.update::<Rail>(d, |r| r.vertical = true)?;
                    tx.set_rail_size(d, 200.0, w, h)?;
                }
                let part = tx.add_part("ACME", "C1")?;
                tx.set_property(part, "width_mm", Some(Value::Float(45.0)))?;
                tx.set_property(part, "height_mm", Some(Value::Float(90.0)))?;
                let page = tx.add_page("1")?;
                let mut pins = Vec::new();
                for (tag, y, dir) in [
                    ("=A1+CAB1-K1", 50.0, Direction::Down),
                    ("=A1+CAB1-K2", 250.0, Direction::Up),
                ] {
                    let d = tx.add_device(DeviceTag::parse(tag)?, "")?;
                    tx.assign_part(d, part)?;
                    tx.place_device(d, cab, Point::new(50.0, y), false, None)?;
                    let symbol = tx.place_symbol(
                        page,
                        "iec/contacts/no",
                        Placement::at(Point::new(0.0, 0.0)),
                        vec![Pin::new("1", 0.0, 0.0).with_direction(dir)],
                        SymbolRole::Device {
                            device: d,
                            main: true,
                        },
                    )?;
                    pins.push(PinRef::new(symbol, 0));
                }
                tx.connect(pins[0], pins[1])?;
                Ok(cab)
            })
            .unwrap();
        (p, cab)
    }

    #[test]
    fn a_wire_through_a_duct_is_a_route_and_a_fill() {
        let (p, cab) = cabinet(Some((40.0, 60.0)));
        let o = overlay(&p, cab);
        assert_eq!(o.routes.len(), 1);
        assert!(!o.routes[0].free_air);
        assert!(o.routes[0].points.len() >= 2);
        assert_eq!(o.ducts.len(), 1);
        let ratio = o.ducts[0].ratio.unwrap();
        assert!(ratio > 0.0 && ratio < 0.1, "{ratio}");
        assert_eq!(o.ducts[0].wires, 1);
        // The duct line is x=150, 40 mm wide: its rectangle spans 130..170.
        assert_eq!(o.ducts[0].rect, (130.0, 100.0, 40.0, 200.0));
        assert!(o.summary().starts_with("1 wire routed, fullest duct"));
    }

    #[test]
    fn without_a_duct_the_wire_is_drawn_as_free_air() {
        let (p, cab) = cabinet(None);
        let o = overlay(&p, cab);
        assert_eq!(o.routes.len(), 1);
        assert!(o.routes[0].free_air);
        assert!(o.ducts.is_empty());
        assert_eq!(o.summary(), "1 wire routed, 1 in free air");
    }

    #[test]
    fn a_duct_without_a_size_has_no_ratio() {
        let (p, cab) = cabinet(Some((0.0, 0.0)));
        let o = overlay(&p, cab);
        assert_eq!(o.ducts[0].ratio, None);
        assert_eq!(o.ducts[0].wires, 1);
    }

    #[test]
    fn a_plate_nothing_is_routed_on_has_an_empty_overlay() {
        let (p, _) = cabinet(None);
        assert_eq!(overlay(&p, StructureId::from(9999)), Overlay::default());
    }

    #[test]
    fn the_cache_rebuilds_only_when_the_project_or_panel_changes() {
        let (mut p, cab) = cabinet(Some((40.0, 60.0)));
        let mut cache = OverlayCache::default();
        assert_eq!(cache.get(&p, cab).routes.len(), 1);
        // Poison the stored copy: an unchanged project must not rebuild it.
        cache.overlay.routes.clear();
        assert!(cache.get(&p, cab).routes.is_empty());
        // Any edit bumps the revision and the overlay is built again.
        p.transact("edit", |tx| tx.add_page("2")).unwrap();
        assert_eq!(cache.get(&p, cab).routes.len(), 1);
        // Another panel is another key.
        assert!(cache.get(&p, StructureId::from(9999)).routes.is_empty());
    }

    #[test]
    fn the_panel_fits_its_width_and_leaves_the_rest_to_the_plate() {
        let ctx = egui::Context::default();
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1340.0, 700.0));
        let input = egui::RawInput {
            screen_rect: Some(screen),
            ..Default::default()
        };
        let overlay = Overlay {
            rows: vec![RouteRow {
                wire: 1,
                label: "#1".into(),
                ends: "=A1+CAB-F2:2 > =A1+CAB-X1:1".into(),
                length_mm: 120.0,
                basis: "routed, no depth",
                ducts: 2,
            }],
            ..Overlay::default()
        };
        let mut left = 0.0;
        for _ in 0..3 {
            let mut output = ctx.run_ui(input.clone(), |ui| {
                panel(ui, &overlay, StructureId(1));
                left = ui.available_rect_before_wrap().right();
            });
            output.textures_delta.clear();
        }
        // The panel is its 340 points wide, inside the window, and the
        // buttons wrap instead of widening it.
        assert_eq!(left, 1000.0);
    }
}
