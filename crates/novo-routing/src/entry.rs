//! Where wires enter an enclosure: one point on the mounting plate, kept as
//! two properties of the enclosure (`docs/design/cabinet-entry.md`).
//!
//! Both keys are set or neither, the point is finite and inside the plate,
//! and it is written through the transaction so it undoes like any edit.

use novo_core::{ModelError, Project, Result, StructureId, Tx, Value};

use crate::Point;

/// Wire length outside the entry for wires that have no cable, mm: from the
/// entry to where the wire ends, service loop included.
pub const ENTRY_OUTSIDE: &str = "entry.outside_mm";
/// The largest stated outside length, mm; a larger stored figure reads as not stated.
pub const MAX_OUTSIDE_MM: f64 = 100_000.0;
/// Plate x of the entry point, mm.
pub const ENTRY_X: &str = "entry.x_mm";
/// Plate y of the entry point, mm.
pub const ENTRY_Y: &str = "entry.y_mm";

/// The wall of the enclosure the gland is in, as a side of the plate:
/// `top`, `bottom`, `left` or `right`. Not stated reads as the plate edge
/// nearest to the entry point (assumed, and labelled so).
pub const ENTRY_WALL: &str = "entry.wall";

/// A side of the mounting plate, plate coordinates (origin top left, y down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wall {
    Top,
    Bottom,
    Left,
    Right,
}

impl Wall {
    pub const ALL: [Wall; 4] = [Wall::Top, Wall::Bottom, Wall::Left, Wall::Right];

    pub fn key(self) -> &'static str {
        match self {
            Wall::Top => "top",
            Wall::Bottom => "bottom",
            Wall::Left => "left",
            Wall::Right => "right",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Wall::Top => "Top",
            Wall::Bottom => "Bottom",
            Wall::Left => "Left",
            Wall::Right => "Right",
        }
    }

    fn from_key(key: &str) -> Option<Wall> {
        Wall::ALL.into_iter().find(|w| w.key() == key)
    }

    /// Unit vector pointing out of the plate through this side.
    pub fn outward(self) -> (f64, f64) {
        match self {
            Wall::Top => (0.0, -1.0),
            Wall::Bottom => (0.0, 1.0),
            Wall::Left => (-1.0, 0.0),
            Wall::Right => (1.0, 0.0),
        }
    }
}

/// The wall an entry leaves through and whether the designer stated it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EntryWall {
    pub wall: Wall,
    /// False: the plate edge nearest to the entry point, a guess.
    pub stated: bool,
}

/// The plate edge nearest to `at` (ties: bottom, top, left, right, the
/// order a gland is usually put in).
pub fn nearest_wall(plate: &novo_core::MountingPlate, at: Point) -> Wall {
    [
        (Wall::Bottom, plate.height - at.y),
        (Wall::Top, at.y),
        (Wall::Left, at.x),
        (Wall::Right, plate.width - at.x),
    ]
    .into_iter()
    .min_by(|a, b| a.1.total_cmp(&b.1))
    .map_or(Wall::Bottom, |(w, _)| w)
}

/// The wall of the entry point: stated, else the nearest plate edge. `None`
/// without a valid entry point.
pub fn entry_wall(project: &Project, structure: StructureId) -> Option<EntryWall> {
    let at = entry_point(project, structure)?;
    let s = project.get::<novo_core::Structure>(structure)?;
    let plate = s.plate?;
    let stated = match s.properties.get(ENTRY_WALL) {
        Some(Value::Text(t)) => Wall::from_key(t.trim()),
        _ => None,
    };
    Some(match stated {
        Some(wall) => EntryWall { wall, stated: true },
        None => EntryWall {
            wall: nearest_wall(&plate, at),
            stated: false,
        },
    })
}

/// Sets the wall of the entry, or clears it with `None` (back to the
/// nearest plate edge).
pub fn set_wall(tx: &mut Tx<'_>, structure: StructureId, wall: Option<Wall>) -> Result<()> {
    tx.set_property(
        structure,
        ENTRY_WALL,
        wall.map(|w| Value::Text(w.key().into())),
    )
}

/// Where a wire from the entry reaches the plate edge of `wall`.
pub fn edge_point(plate: &novo_core::MountingPlate, at: Point, wall: Wall) -> Point {
    match wall {
        Wall::Top => Point::new(at.x, 0.0),
        Wall::Bottom => Point::new(at.x, plate.height),
        Wall::Left => Point::new(0.0, at.y),
        Wall::Right => Point::new(plate.width, at.y),
    }
}

fn number(value: &Value) -> Option<f64> {
    match value {
        Value::Float(v) if v.is_finite() => Some(*v),
        Value::Int(v) => Some(*v as f64),
        _ => None,
    }
}

/// The entry point of an enclosure's plate, if it has a valid one: both keys
/// present and numbers, inside the plate. A half-set or out-of-plate pair
/// reads as none (never a guess).
pub fn entry_point(project: &Project, structure: StructureId) -> Option<Point> {
    let s = project.get::<novo_core::Structure>(structure)?;
    let plate = s.plate?;
    let x = number(s.properties.get(ENTRY_X)?)?;
    let y = number(s.properties.get(ENTRY_Y)?)?;
    ((0.0..=plate.width).contains(&x) && (0.0..=plate.height).contains(&y))
        .then(|| Point::new(x, y))
}

/// The stated length of wire outside the entry (see [`ENTRY_OUTSIDE`]):
/// finite and positive, or `None` when not stated.
pub fn outside_mm(project: &Project, structure: StructureId) -> Option<f64> {
    let s = project.get::<novo_core::Structure>(structure)?;
    number(s.properties.get(ENTRY_OUTSIDE)?).filter(|v| *v > 0.0 && *v <= MAX_OUTSIDE_MM)
}

/// Sets the stated length of wire outside the entry, or clears it with
/// `None`. Refused when not finite and positive.
pub fn set_outside_mm(tx: &mut Tx<'_>, structure: StructureId, mm: Option<f64>) -> Result<()> {
    let value = match mm {
        Some(v) if v.is_finite() && v > 0.0 && v <= MAX_OUTSIDE_MM => Some(Value::Float(v)),
        Some(_) => {
            return Err(ModelError::Refused {
                what: "Wire outside the entry".into(),
                reason: "must be a length between 0 and 100000 mm".into(),
            });
        }
        None => None,
    };
    tx.set_property(structure, ENTRY_OUTSIDE, value)
}

/// Whether the enclosure has an entry point stored, valid or not. True while
/// [`entry_point`] is `None` means the stored point is hidden: half set, or
/// outside a plate that was made smaller.
pub fn has_stored_point(project: &Project, structure: StructureId) -> bool {
    project
        .get::<novo_core::Structure>(structure)
        .is_some_and(|s| s.properties.contains_key(ENTRY_X) || s.properties.contains_key(ENTRY_Y))
}

/// Sets the entry point of an enclosure's plate, or clears it with `None`.
/// Refused when there is no plate or the point is not finite and inside it.
pub fn set_entry_point(
    tx: &mut Tx<'_>,
    structure: StructureId,
    point: Option<Point>,
) -> Result<()> {
    let refuse = |reason: &str| ModelError::Refused {
        what: "Entry point".into(),
        reason: reason.into(),
    };
    let Some(point) = point else {
        tx.set_property(structure, ENTRY_X, None)?;
        return tx.set_property(structure, ENTRY_Y, None);
    };
    let plate = tx
        .project()
        .get::<novo_core::Structure>(structure)
        .ok_or(ModelError::NotFound(structure.into()))?
        .plate
        .ok_or_else(|| refuse("the enclosure has no mounting plate yet"))?;
    if !(point.x.is_finite() && point.y.is_finite())
        || !(0.0..=plate.width).contains(&point.x)
        || !(0.0..=plate.height).contains(&point.y)
    {
        return Err(refuse("the point must lie on the mounting plate"));
    }
    tx.set_property(structure, ENTRY_X, Some(Value::Float(point.x)))?;
    tx.set_property(structure, ENTRY_Y, Some(Value::Float(point.y)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{DeviceTag, Document, MountingPlate};

    fn plate_doc() -> (Document, StructureId) {
        let mut doc = Document::default();
        let (id, _) = doc
            .transact("Enclosure", |tx| {
                let id = tx.add_structure(DeviceTag::parse("+CAB").unwrap(), "Cabinet", true)?;
                tx.update::<novo_core::Structure>(id, |s| {
                    s.plate = Some(MountingPlate {
                        width: 600.0,
                        height: 800.0,
                        margin: 20.0,
                    });
                })?;
                Ok(id)
            })
            .unwrap();
        (doc, id)
    }

    #[test]
    fn a_valid_point_is_stored_read_back_and_undone() {
        let (mut doc, id) = plate_doc();
        assert_eq!(entry_point(doc.project(), id), None);
        doc.transact("Entry", |tx| {
            set_entry_point(tx, id, Some(Point::new(300.0, 800.0)))
        })
        .unwrap();
        assert_eq!(
            entry_point(doc.project(), id),
            Some(Point::new(300.0, 800.0))
        );
        doc.undo().unwrap();
        assert_eq!(entry_point(doc.project(), id), None);
    }

    #[test]
    fn points_off_the_plate_or_not_finite_are_refused_and_change_nothing() {
        let (mut doc, id) = plate_doc();
        let depth = doc.undo_depth();
        for p in [
            Point::new(-1.0, 10.0),
            Point::new(601.0, 10.0),
            Point::new(10.0, 801.0),
            Point::new(f64::NAN, 10.0),
            Point::new(10.0, f64::INFINITY),
        ] {
            assert!(
                doc.transact("Entry", |tx| set_entry_point(tx, id, Some(p)))
                    .is_err(),
                "{p:?}"
            );
        }
        assert_eq!(entry_point(doc.project(), id), None);
        assert_eq!(doc.undo_depth(), depth);
    }

    #[test]
    fn half_set_or_stale_values_read_as_none_and_clearing_removes_both() {
        let (mut doc, id) = plate_doc();
        doc.transact("Half", |tx| {
            tx.set_property(id, ENTRY_X, Some(Value::Float(10.0)))
        })
        .unwrap();
        assert_eq!(entry_point(doc.project(), id), None, "only x is set");
        doc.transact("Entry", |tx| {
            set_entry_point(tx, id, Some(Point::new(10.0, 10.0)))
        })
        .unwrap();
        // The plate shrinks below the point: it no longer counts.
        doc.transact("Shrink", |tx| {
            tx.update::<novo_core::Structure>(id, |s| {
                s.plate = Some(MountingPlate {
                    width: 5.0,
                    height: 5.0,
                    margin: 0.0,
                });
            })
        })
        .unwrap();
        assert_eq!(entry_point(doc.project(), id), None);
        doc.transact("Clear", |tx| set_entry_point(tx, id, None))
            .unwrap();
        let s = doc.project().get::<novo_core::Structure>(id).unwrap();
        assert!(!s.properties.contains_key(ENTRY_X) && !s.properties.contains_key(ENTRY_Y));
    }

    #[test]
    fn the_wall_is_the_nearest_edge_until_it_is_stated() {
        let (mut doc, id) = plate_doc();
        assert_eq!(entry_wall(doc.project(), id), None, "no entry point");
        doc.transact("Entry", |tx| {
            set_entry_point(tx, id, Some(Point::new(300.0, 790.0)))
        })
        .unwrap();
        assert_eq!(
            entry_wall(doc.project(), id),
            Some(EntryWall {
                wall: Wall::Bottom,
                stated: false
            })
        );
        doc.transact("Wall", |tx| set_wall(tx, id, Some(Wall::Left)))
            .unwrap();
        assert_eq!(
            entry_wall(doc.project(), id),
            Some(EntryWall {
                wall: Wall::Left,
                stated: true
            })
        );
        doc.transact("Garbage", |tx| {
            tx.set_property(id, ENTRY_WALL, Some(Value::Text("sideways".into())))
        })
        .unwrap();
        assert!(
            !entry_wall(doc.project(), id).unwrap().stated,
            "a bad value reads as not stated"
        );
        doc.undo().unwrap();
        doc.undo().unwrap();
        assert!(!entry_wall(doc.project(), id).unwrap().stated, "undone");
    }

    #[test]
    fn the_stub_reaches_the_edge_of_its_wall() {
        let plate = MountingPlate {
            width: 600.0,
            height: 800.0,
            margin: 20.0,
        };
        let at = Point::new(100.0, 700.0);
        assert_eq!(nearest_wall(&plate, at), Wall::Bottom);
        assert_eq!(
            edge_point(&plate, at, Wall::Bottom),
            Point::new(100.0, 800.0)
        );
        assert_eq!(edge_point(&plate, at, Wall::Left), Point::new(0.0, 700.0));
        assert_eq!(edge_point(&plate, at, Wall::Top), Point::new(100.0, 0.0));
        assert_eq!(
            edge_point(&plate, at, Wall::Right),
            Point::new(600.0, 700.0)
        );
        assert_eq!(Wall::Left.outward(), (-1.0, 0.0));
    }
}
