//! The chosen enclosure ([`crate::spec::EnclosureSpec`]) as the room the
//! mechanical checks judge against (usable depth, plate size, door gap).
//!
//! Pure. The door hinge, angle and free room are not part of the record
//! yet: they come from the typed door form when there is one. The numbers
//! of the record win over typed inside sizes; a disagreement is reported
//! ([`Fit::differs`]) instead of being averaged away.

use crate::enclosure::{Door, Enclosure};
use crate::spec::{EnclosureSpec, Missing, Source, disagree};

/// The record read as an [`Enclosure`], with how far to trust it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fit {
    pub enclosure: Enclosure,
    /// The weakest source among the numbers used.
    pub weakest: Source,
    /// The typed inside size, when it disagrees with the record beyond the
    /// tolerance of [`disagree`]: width, height, depth.
    pub differs: Option<(f64, f64, f64)>,
}

/// Reads the record as the room for devices. `typed` is the door form's
/// enclosure, if any: it supplies the hinge, angle and free room, and its
/// inside size is only compared. Names the first missing field.
///
/// Depth is measured from the inside of the back wall to the inside of the
/// door; the plate setback is taken from the inside of the back wall.
pub fn fit(spec: &EnclosureSpec, typed: Option<&Enclosure>) -> Result<Fit, Missing> {
    let inside = spec.interior()?;
    let setback = spec.plate_setback.ok_or(Missing("plate setback"))?;
    let typed_door = typed.and_then(|e| e.door);
    let inner = spec
        .door_inner_depth
        .map(|f| f.value)
        .or(typed_door.map(|d| d.inner_depth))
        .unwrap_or(0.0);
    let mut weakest = inside.weakest.max(setback.source);
    if let Some(f) = spec.door_inner_depth {
        weakest = weakest.max(f.source);
    }
    let differs = typed
        .filter(|t| {
            disagree(t.width, inside.width)
                || disagree(t.height, inside.height)
                || disagree(t.depth, inside.depth)
        })
        .map(|t| (t.width, t.height, t.depth));
    Ok(Fit {
        enclosure: Enclosure {
            width: inside.width,
            height: inside.height,
            depth: inside.depth,
            plate_setback: setback.value,
            door: typed_door.map(|d| Door {
                inner_depth: inner,
                ..d
            }),
            front_clearance: typed.and_then(|e| e.front_clearance),
            hinge_side_clearance: typed.and_then(|e| e.hinge_side_clearance),
        },
        weakest,
        differs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plate;
    use crate::enclosure::{Hinge, check};
    use crate::spec::{CatalogueRow, Field};

    fn spec(depth: f64) -> EnclosureSpec {
        let mut s = CatalogueRow {
            article: "T-1".into(),
            series: "T".into(),
            width: 600.0,
            height: 800.0,
            depth,
            wall: 2.0,
            plate_article: String::new(),
            source: "test".into(),
            terms: "test".into(),
        }
        .spec();
        s.plate_setback = Some(Field::new(20.0, Source::Catalogue));
        s
    }

    #[test]
    fn missing_setback_is_named_not_defaulted() {
        let mut s = spec(300.0);
        s.plate_setback = None;
        assert_eq!(fit(&s, None).unwrap_err(), Missing("plate setback"));
    }

    #[test]
    fn a_deeper_enclosure_changes_the_door_report() {
        let plate = Plate {
            width: 500.0,
            height: 700.0,
            margin: 10.0,
        };
        let depths = [(1, Some(170.0))];
        let shallow = fit(&spec(200.0), None).unwrap().enclosure;
        let deep = fit(&spec(300.0), None).unwrap().enclosure;
        assert_eq!(shallow.usable_depth(), 196.0 - 20.0);
        assert_eq!(check(&shallow, &plate, &depths).tight.len(), 1);
        assert!(check(&deep, &plate, &depths).is_clean());
    }

    #[test]
    fn typed_size_only_compared_and_door_kept() {
        let typed = Enclosure {
            width: 500.0,
            height: 800.0,
            depth: 250.0,
            plate_setback: 0.0,
            door: Some(Door {
                hinge: Hinge::Right,
                max_angle: 100.0,
                inner_depth: 15.0,
            }),
            front_clearance: Some(900.0),
            hinge_side_clearance: None,
        };
        let f = fit(&spec(300.0), Some(&typed)).unwrap();
        assert_eq!(f.enclosure.width, 596.0);
        assert_eq!(f.differs, Some((500.0, 800.0, 250.0)));
        let door = f.enclosure.door.unwrap();
        assert_eq!(
            (door.hinge, door.max_angle, door.inner_depth),
            (Hinge::Right, 100.0, 15.0)
        );
        assert_eq!(f.weakest, Source::Catalogue);
    }
}
