//! Assembly checklist: what the workshop does to build one cabinet, in
//! order, with a box to tick at every step (Pro Panel gap analysis, R8).
//!
//! One group of tables per enclosure with a mounting plate:
//!
//! 1. **Prepare**: the plate to check, and the rails and ducts to cut,
//!    grouped by length so one length is cut once.
//! 2. **Drill**: the plate drawing's drill table: one line per drill to fit,
//!    then every hole and cut-out with its position, smallest hole first.
//! 3. **Mount**: rails and ducts by position, then devices top to bottom and
//!    left to right, with part, rail, offset and position on the plate.
//! 4. **Wire**: the wires of the wire list that have an end in this cabinet,
//!    with colour, size and length, in the wire list's order.
//! 5. **Label**: one label per placed device.
//! 6. **Check**: what the model can say about the layout (devices not placed,
//!    overlaps, clearances, rails, sizes, guessed lengths) and the checks a
//!    person does, with lines to sign.
//!
//! It is a [`Report`] like the others, so it pages, exports to CSV and Excel
//! and prints through the stand-alone report PDF. It is not proposed as a
//! generated page by default: it is a document for the workshop, not a page
//! of the drawing set. Only the project is read; every table is a function
//! of it.

use std::collections::BTreeMap;

use novo_core::{
    AnyId, CabinetPlacement, Device, DeviceId, DeviceTag, EntityKind, Part, Project, Rail,
    RailKind, Structure, natural_cmp,
};
use novo_layout::drill::mm as drill_mm;
use novo_layout::model::{load, plate_documents};

use crate::WireList;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};

/// The box printed in the Done column.
pub const BOX: &str = "[ ]";

/// The assembly checklist.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssemblyChecklist;

impl AssemblyChecklist {
    pub const ID: &'static str = "assembly";
}

/// Rounds to a tenth of a millimetre, so floating point noise does not show.
fn mm(v: f64) -> Cell {
    Cell::Number((v * 10.0).round() / 10.0)
}

fn done() -> Cell {
    Cell::text(BOX)
}

fn columns(list: &[(&str, &str, f64, bool)]) -> Vec<Column> {
    list.iter()
        .map(|(key, header, width, right)| {
            let c = Column::new(key, header, *width);
            if *right { c.right() } else { c }
        })
        .collect()
}

/// A rail or duct on the plate, with its mark.
struct Piece<'a> {
    mark: String,
    rail: &'a Rail,
}

impl Piece<'_> {
    fn name(&self) -> String {
        match self.rail.kind {
            RailKind::DinRail => "DIN rail".into(),
            RailKind::Duct if self.rail.width > 0.0 && self.rail.height > 0.0 => format!(
                "Wiring duct {} x {}",
                Cell::Number(self.rail.width),
                Cell::Number(self.rail.height)
            ),
            RailKind::Duct => "Wiring duct".into(),
        }
    }
}

fn position_cmp(a: &Rail, b: &Rail) -> std::cmp::Ordering {
    a.y.total_cmp(&b.y)
        .then(a.x.total_cmp(&b.x))
        .then(a.id.raw().cmp(&b.id.raw()))
}

/// The marks of an enclosure's rails (`R1`...) and ducts (`D1`...), each in
/// plate reading order.
fn pieces<'a>(project: &'a Project, enclosure: &Structure) -> Vec<Piece<'a>> {
    let mut rails: Vec<&Rail> = project
        .all::<Rail>()
        .filter(|r| r.structure == enclosure.id)
        .collect();
    rails.sort_by(|a, b| position_cmp(a, b));
    let (mut din, mut duct) = (0, 0);
    rails
        .into_iter()
        .map(|rail| {
            let mark = match rail.kind {
                RailKind::DinRail => {
                    din += 1;
                    format!("R{din}")
                }
                RailKind::Duct => {
                    duct += 1;
                    format!("D{duct}")
                }
            };
            Piece { mark, rail }
        })
        .collect()
}

/// A tag's location, from a designation such as `=A1+CAB1-K1:13`.
fn location_of_designation(designation: &str) -> Option<Vec<String>> {
    let tag = designation.split(':').next()?;
    DeviceTag::parse(tag).ok().map(|t| t.location)
}

fn part_text(project: &Project, device: &Device) -> String {
    let parts: Vec<String> = device
        .parts
        .iter()
        .filter_map(|id| project.get::<Part>(*id))
        .map(|p| {
            let what = if p.type_number.is_empty() {
                &p.order_number
            } else {
                &p.type_number
            };
            format!("{} {}", p.manufacturer, what).trim().to_owned()
        })
        .collect();
    if parts.is_empty() {
        "(no part)".into()
    } else {
        parts.join(" + ")
    }
}

impl Report for AssemblyChecklist {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Assembly checklist".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        // The wire list's inputs plus the cabinet layout and the parts.
        Dependencies::on(&[
            Page,
            Device,
            Symbol,
            Junction,
            Wire,
            Potential,
            Terminal,
            Cable,
            Structure,
            Rail,
            CabinetPlacement,
            Part,
        ])
        .with_info()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let mut enclosures: Vec<&Structure> = project
            .structures()
            .filter(|s| s.enclosure && s.plate.is_some())
            .collect();
        enclosures.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));

        let wires = WireList.generate(project);
        let mut sections = Vec::new();
        for enclosure in &enclosures {
            sections.extend(cabinet(project, enclosure, &wires));
        }
        if sections.is_empty() {
            let mut none = Section::new(
                "none",
                "",
                columns(&[("note", "Assembly checklist", 80.0, false)]),
            );
            none.push(
                vec![Cell::text(
                    "No enclosure has a mounting plate yet. Create a cabinet and set its plate on the Cabinet canvas.",
                )],
                Vec::new(),
            );
            sections.push(none);
        }
        ReportOutput {
            report: Self::ID.into(),
            title: self.title(),
            revision: project.revision(),
            sections,
        }
    }
}

/// The five tables of one enclosure.
fn cabinet(project: &Project, enclosure: &Structure, wires: &ReportOutput) -> Vec<Section> {
    let tag = enclosure.tag.to_string();
    let Some(plate) = enclosure.plate else {
        return Vec::new();
    };
    let layout = load(project, enclosure.id);
    let pieces = pieces(project, enclosure);
    let key = |step: &str| format!("{tag}:{step}");
    let title = |n: usize, name: &str| format!("{tag}  {n} {name}");
    let source = AnyId::from(enclosure.id);

    // 1. Prepare: the plate, then the pieces to cut, grouped by length.
    let mut prepare = Section::new(
        &key("prepare"),
        &title(1, "Prepare"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("item", "Item", 34.0, false),
            ("length_mm", "Length mm", 12.0, true),
            ("qty", "Qty", 7.0, true),
            ("marks", "Marks", 20.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    let mut n = 1;
    prepare.push(
        vec![
            Cell::text(format!("1.{n}")),
            Cell::text(format!(
                "Mounting plate {} x {} mm, margin {} mm",
                Cell::Number(plate.width),
                Cell::Number(plate.height),
                Cell::Number(plate.margin)
            )),
            Cell::Empty,
            Cell::Int(1),
            Cell::Empty,
            done(),
        ],
        vec![source],
    );
    let mut groups: Vec<(String, f64, Vec<&Piece<'_>>)> = Vec::new();
    for piece in &pieces {
        let name = piece.name();
        let length = (piece.rail.length * 10.0).round() / 10.0;
        match groups
            .iter_mut()
            .find(|(n, l, _)| *n == name && l.to_bits() == length.to_bits())
        {
            Some((_, _, members)) => members.push(piece),
            None => groups.push((name, length, vec![piece])),
        }
    }
    groups.sort_by(|a, b| {
        kind_order(a.2[0].rail.kind)
            .cmp(&kind_order(b.2[0].rail.kind))
            .then_with(|| natural_cmp(&a.0, &b.0))
            .then(b.1.total_cmp(&a.1))
    });
    for (name, length, members) in groups {
        n += 1;
        prepare.push(
            vec![
                Cell::text(format!("1.{n}")),
                Cell::text(format!("Cut {name}")),
                mm(length),
                Cell::Int(members.len() as i64),
                Cell::text(
                    members
                        .iter()
                        .map(|p| p.mark.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
                done(),
            ],
            members
                .iter()
                .map(|p| AnyId::from(p.rail.id))
                .collect::<Vec<_>>(),
        );
    }

    // 2. Drill: the holes and cut-outs of the plate drawing, one tool at a
    // time (smallest hole first), then the cut-outs.
    let drilling = plate_documents(project, enclosure.id).map(|(_, table)| table);
    let mut drill = Section::new(
        &key("drill"),
        &title(2, "Drill"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("item", "Item", 34.0, false),
            ("surface", "Surface", 14.0, false),
            ("x_mm", "x mm", 9.0, true),
            ("y_mm", "y mm", 9.0, true),
            ("for", "For", 22.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    let table = drilling.unwrap_or_default();
    let mut n = 0;
    for (surface, dia, count) in table.summary() {
        n += 1;
        drill.push(
            vec![
                Cell::text(format!("2.{n}")),
                Cell::text(format!("Fit drill dia {}: {count} holes", drill_mm(dia))),
                Cell::text(surface.name()),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                done(),
            ],
            vec![source],
        );
    }
    for hole in &table.holes {
        n += 1;
        drill.push(
            vec![
                Cell::text(format!("2.{n}")),
                Cell::text(format!("Hole dia {}", drill_mm(hole.dia))),
                Cell::text(hole.surface.name()),
                mm(hole.x),
                mm(hole.y),
                Cell::text(hole.source.clone()),
                done(),
            ],
            vec![source],
        );
    }
    for cut in &table.cutouts {
        n += 1;
        drill.push(
            vec![
                Cell::text(format!("2.{n}")),
                Cell::text(format!("Cut-out {}", cut.shape.describe())),
                Cell::text(cut.surface.name()),
                mm(cut.x),
                mm(cut.y),
                Cell::text(cut.source.clone()),
                done(),
            ],
            vec![source],
        );
    }
    if n == 0 {
        drill.push(
            vec![
                Cell::text("2.1"),
                Cell::text("No holes or cut-outs to make"),
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
                Cell::Empty,
            ],
            vec![source],
        );
    }

    // 3. Mount: rails and ducts, then devices in reading order.
    let mut mount = Section::new(
        &key("mount"),
        &title(3, "Mount"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("item", "Item", 24.0, false),
            ("part", "Part", 30.0, false),
            ("on", "On", 16.0, false),
            ("x_mm", "x mm", 9.0, true),
            ("y_mm", "y mm", 9.0, true),
            ("turned", "Turned", 8.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    let mut n = 0;
    for piece in &pieces {
        n += 1;
        mount.push(
            vec![
                Cell::text(format!("3.{n}")),
                Cell::text(format!("{} {}", piece.mark, piece.name())),
                Cell::text(format!(
                    "{} mm {}",
                    Cell::Number(piece.rail.length),
                    if piece.rail.vertical {
                        "vertical"
                    } else {
                        "horizontal"
                    }
                )),
                Cell::text("plate"),
                mm(piece.rail.x),
                mm(piece.rail.y),
                Cell::Empty,
                done(),
            ],
            vec![AnyId::from(piece.rail.id)],
        );
    }
    let mark_of: BTreeMap<u64, &Piece<'_>> = pieces.iter().map(|p| (p.rail.id.raw(), p)).collect();
    let mut placed: Vec<(&CabinetPlacement, &Device)> = project
        .all::<CabinetPlacement>()
        .filter(|p| p.structure == enclosure.id)
        .filter_map(|p| Some((p, project.get::<Device>(p.device)?)))
        .collect();
    // Reading order: a device on a rail sits at its rail's row.
    let row = |p: &CabinetPlacement| {
        p.rail
            .and_then(|r| mark_of.get(&r.raw()))
            .map_or(p.y, |piece| piece.rail.y)
    };
    placed.sort_by(|(a, da), (b, db)| {
        row(a)
            .total_cmp(&row(b))
            .then(a.x.total_cmp(&b.x))
            .then_with(|| natural_cmp(&da.tag.to_string(), &db.tag.to_string()))
    });
    for (placement, device) in &placed {
        n += 1;
        let on = match placement
            .rail
            .and_then(|r| mark_of.get(&r.raw()).map(|p| (p, r)))
        {
            Some((piece, _)) => match placement.rail_offset {
                Some(offset) => format!("{} at {}", piece.mark, Cell::Number(offset.round())),
                None => piece.mark.clone(),
            },
            None => "plate".into(),
        };
        mount.push(
            vec![
                Cell::text(format!("3.{n}")),
                Cell::text(device.tag.to_string()),
                Cell::text(part_text(project, device)),
                Cell::text(on),
                mm(placement.x),
                mm(placement.y),
                Cell::text(if placement.rotated { "90°" } else { "" }),
                done(),
            ],
            vec![AnyId::from(device.id), AnyId::from(placement.id)],
        );
    }

    // 4. Wire: the wire list's connections with an end in this cabinet.
    let mut wire = Section::new(
        &key("wire"),
        &title(4, "Wire"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("number", "Wire no.", 12.0, false),
            ("from", "From", 28.0, false),
            ("to", "To", 28.0, false),
            ("colour", "Colour", 10.0, false),
            ("cross_section_mm2", "mm²", 7.0, true),
            ("length_mm", "Length mm", 10.0, true),
            ("basis", "Basis", 10.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    let mut estimated = 0;
    if let Some(list) = wires.sections.first() {
        let at = |row: &crate::Row, key: &str| {
            list.column(key)
                .and_then(|i| row.cells.get(i))
                .cloned()
                .unwrap_or(Cell::Empty)
        };
        let mut n = 0;
        for row in &list.rows {
            let (from, to) = (at(row, "from").to_string(), at(row, "to").to_string());
            let here = |d: &str| {
                !enclosure.tag.location.is_empty()
                    && location_of_designation(d).is_some_and(|l| l == enclosure.tag.location)
            };
            if !(here(&from) || here(&to)) {
                continue;
            }
            n += 1;
            let basis = at(row, "length_basis").to_string();
            if basis == "estimate" {
                estimated += 1;
            }
            wire.push(
                vec![
                    Cell::text(format!("4.{n}")),
                    at(row, "number"),
                    Cell::text(from),
                    Cell::text(to),
                    at(row, "color"),
                    at(row, "cross_section_mm2"),
                    at(row, "length_mm"),
                    Cell::text(basis),
                    done(),
                ],
                row.sources.clone(),
            );
        }
    }
    let wires_here = wire.rows.len();

    // 5. Label: one per placed device.
    let mut label = Section::new(
        &key("label"),
        &title(5, "Label"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("text", "Label text", 30.0, false),
            ("fix_at", "Fix at", 30.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    for (i, (placement, device)) in placed.iter().enumerate() {
        let rail = placement
            .rail
            .and_then(|r| mark_of.get(&r.raw()))
            .map_or_else(String::new, |p| format!(" on {}", p.mark));
        label.push(
            vec![
                Cell::text(format!("5.{}", i + 1)),
                Cell::text(device.tag.to_string()),
                Cell::text(format!("{}{rail}", part_text(project, device))),
                done(),
            ],
            vec![AnyId::from(device.id)],
        );
    }

    // 6. Check: what the model knows, then the checks a person does.
    let mut check = Section::new(
        &key("check"),
        &title(6, "Check"),
        columns(&[
            ("no", "No.", 8.0, false),
            ("check", "Check", 44.0, false),
            ("result", "Result", 36.0, false),
            ("done", "Done", 7.0, false),
        ]),
    );
    let tag_of = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .map_or_else(|| format!("#{id}"), |d| d.tag.to_string())
    };
    let list = |ids: Vec<u64>| -> String {
        let mut tags: Vec<String> = ids.into_iter().map(tag_of).collect();
        tags.sort_by(|a, b| natural_cmp(a, b));
        tags.join(", ")
    };
    let count_or = |count: usize, yes: &str, what: &str| {
        if count == 0 {
            yes.to_owned()
        } else {
            format!("{count} {what}")
        }
    };
    let mut rows: Vec<(String, String)> = Vec::new();
    let unplaced = novo_layout::model::candidates(project, enclosure.id)
        .into_iter()
        .filter(|c| !placed.iter().any(|(p, _)| p.device.raw() == c.id))
        .map(|c| c.id)
        .collect::<Vec<_>>();
    rows.push((
        "Every device of the cabinet is on the plate".into(),
        if unplaced.is_empty() {
            "yes".into()
        } else {
            format!("not placed: {}", list(unplaced))
        },
    ));
    if let Some(layout) = &layout {
        let report = layout.report();
        rows.push((
            "No device overlaps another".into(),
            count_or(report.collisions.len(), "yes", "overlapping pairs"),
        ));
        rows.push((
            "Devices keep their clearances".into(),
            count_or(report.too_close.len(), "yes", "pairs too close"),
        ));
        rows.push((
            "Devices are inside the plate margin".into(),
            count_or(report.outside.len(), "yes", "outside"),
        ));
        rows.push((
            "Rails are long enough".into(),
            count_or(report.rail_overflows.len(), "yes", "rails overflow"),
        ));
        rows.push((
            "Part sizes are known".into(),
            if report.guessed.is_empty() {
                "yes".into()
            } else {
                format!("guessed size: {}", list(report.guessed))
            },
        ));
    }
    rows.push((
        "Every drill pattern could be read".into(),
        if table.problems.is_empty() {
            "yes".into()
        } else {
            format!("unreadable: {}", table.problems.join("; "))
        },
    ));
    rows.push((
        "Wire lengths are measured, not estimated".into(),
        if wires_here == 0 {
            "no wires in this cabinet".into()
        } else {
            count_or(estimated, "yes", "wires use an estimated length")
        },
    ));
    for manual in [
        "Every terminal is tightened",
        "Wiring compared with the schematic",
        "Labels are fixed and readable",
    ] {
        rows.push((manual.into(), String::new()));
    }
    for (i, (what, result)) in rows.into_iter().enumerate() {
        check.push(
            vec![
                Cell::text(format!("6.{}", i + 1)),
                Cell::text(what),
                Cell::text(result),
                done(),
            ],
            vec![source],
        );
    }
    let at = check.rows.len();
    for (i, what) in ["Built by (name)", "Date"].into_iter().enumerate() {
        check.push(
            vec![
                Cell::text(format!("6.{}", at + i + 1)),
                Cell::text(what),
                Cell::Empty,
                Cell::Empty,
            ],
            vec![source],
        );
    }

    vec![prepare, drill, mount, wire, label, check]
}

/// Rails before ducts in the cutting group order.
fn kind_order(kind: RailKind) -> u8 {
    match kind {
        RailKind::DinRail => 0,
        RailKind::Duct => 1,
    }
}
