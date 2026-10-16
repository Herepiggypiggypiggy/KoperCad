//! The inside of an enclosure and its door (Pro Panel gap analysis, R2):
//! interior size, depth, door hinge and opening angle, and the 2D checks
//! that follow from them: device depth against the room between plate and
//! door, and the room the swinging door needs in front of the cabinet.
//!
//! Top view coordinates: `x` runs across the front from its left edge, `z`
//! runs from the back wall towards the door (the door's closed position is
//! `z = depth`). Millimetres.
//!
//! Planning values, not a maker's drawing: the door is taken as wide as the
//! interior, hinge play and frame thickness are ignored, and the mounting
//! plate sits in the middle of the interior across its width.

use crate::Plate;

/// A device must leave at least this much between its front and the door
/// inside, for cables and for the door to close without touching.
pub const MIN_DOOR_GAP_MM: f64 = 10.0;
/// Opening angle used when none is given.
pub const DEFAULT_DOOR_ANGLE_DEG: f64 = 90.0;

/// Property key of a device's depth in millimetres. On a part it is the
/// catalog value; on a device it overrides the part (a mounted add-on, a
/// deeper plug), so the door check and the thermal model read one number.
pub const DEPTH_KEY: &str = "depth_mm";

/// The one device-depth rule: a positive value on the device wins over the
/// first positive value on its parts (`part_depths` in part order). `None`
/// when neither has a depth.
pub fn device_depth(
    device: Option<f64>,
    part_depths: impl IntoIterator<Item = Option<f64>>,
) -> Option<f64> {
    device
        .filter(|d| *d > 0.0)
        .or_else(|| part_depths.into_iter().flatten().find(|d| *d > 0.0))
}

/// Which side of the door the hinge is on, seen from the front.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Hinge {
    #[default]
    Left,
    Right,
}

impl Hinge {
    pub fn as_str(self) -> &'static str {
        match self {
            Hinge::Left => "left",
            Hinge::Right => "right",
        }
    }

    pub fn parse(text: &str) -> Option<Hinge> {
        match text.trim().to_ascii_lowercase().as_str() {
            "left" => Some(Hinge::Left),
            "right" => Some(Hinge::Right),
            _ => None,
        }
    }
}

/// The door of an enclosure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Door {
    pub hinge: Hinge,
    /// How far the door opens, in degrees (0 to 180).
    pub max_angle: f64,
    /// How far things mounted on the inside of the door stand out.
    pub inner_depth: f64,
}

impl Default for Door {
    fn default() -> Self {
        Door {
            hinge: Hinge::Left,
            max_angle: DEFAULT_DOOR_ANGLE_DEG,
            inner_depth: 0.0,
        }
    }
}

/// Room the open door needs outside the enclosure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Swing {
    /// In front of the closed door, at the door's furthest point.
    pub front: f64,
    /// Beyond the hinge side wall, for doors that open past 90 degrees.
    pub hinge_side: f64,
}

/// Interior size, plate setback and door of an enclosure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Enclosure {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    /// Distance from the back wall to the surface of the mounting plate.
    pub plate_setback: f64,
    pub door: Option<Door>,
    /// Free room in front of the enclosure, if known.
    pub front_clearance: Option<f64>,
    /// Free room beside the hinge side, if known.
    pub hinge_side_clearance: Option<f64>,
}

impl Enclosure {
    /// Room for devices, from the plate surface to the door inside.
    pub fn usable_depth(&self) -> f64 {
        let inner = self.door.map_or(0.0, |d| d.inner_depth);
        (self.depth - self.plate_setback - inner).max(0.0)
    }

    /// What the open door needs; `None` without a door.
    pub fn swing(&self) -> Option<Swing> {
        let door = self.door?;
        let angle = door.max_angle.clamp(0.0, 180.0).to_radians();
        Some(Swing {
            front: self.width
                * if angle >= std::f64::consts::FRAC_PI_2 {
                    1.0
                } else {
                    angle.sin()
                },
            hinge_side: self.width * (-angle.cos()).max(0.0),
        })
    }

    /// Points of the door's free edge from closed to fully open, in top
    /// view coordinates, `steps` + 1 of them.
    pub fn arc(&self, steps: usize) -> Vec<(f64, f64)> {
        let Some(door) = self.door else {
            return Vec::new();
        };
        let max = door.max_angle.clamp(0.0, 180.0).to_radians();
        let (hx, dir) = match door.hinge {
            Hinge::Left => (0.0, 1.0),
            Hinge::Right => (self.width, -1.0),
        };
        let steps = steps.max(1);
        (0..=steps)
            .map(|i| {
                let a = max * i as f64 / steps as f64;
                (
                    hx + dir * self.width * a.cos(),
                    self.depth + self.width * a.sin(),
                )
            })
            .collect()
    }

    /// The left edge of the mounting plate in the interior (plate centred).
    pub fn plate_left(&self, plate: &Plate) -> f64 {
        ((self.width - plate.width) / 2.0).max(0.0)
    }
}

/// How a device's depth compares with the room in front of the plate.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DoorReport {
    /// Devices deeper than the usable depth, with the excess: the door
    /// cannot close.
    pub too_deep: Vec<(u64, f64)>,
    /// Devices that fit but leave less than [`MIN_DOOR_GAP_MM`], with the gap.
    pub tight: Vec<(u64, f64)>,
    /// Devices whose part has no depth, so nothing was checked.
    pub unknown_depth: Vec<u64>,
    /// The plate is wider or higher than the interior.
    pub plate_too_big: bool,
    /// What the open door needs outside.
    pub swing: Option<Swing>,
    /// By how much the free room in front falls short.
    pub front_short: Option<f64>,
    /// By how much the free room beside the hinge side falls short.
    pub hinge_side_short: Option<f64>,
}

impl DoorReport {
    pub fn problems(&self) -> usize {
        self.too_deep.len()
            + self.tight.len()
            + usize::from(self.plate_too_big)
            + usize::from(self.front_short.is_some())
            + usize::from(self.hinge_side_short.is_some())
    }

    pub fn is_clean(&self) -> bool {
        self.problems() == 0
    }
}

fn shortfall(have: Option<f64>, need: f64) -> Option<f64> {
    have.filter(|have| *have < need).map(|have| need - have)
}

/// Checks the devices of a plate (`depths`: id and depth, `None` when the
/// part has none) and the door against the enclosure.
pub fn check(enclosure: &Enclosure, plate: &Plate, depths: &[(u64, Option<f64>)]) -> DoorReport {
    let usable = enclosure.usable_depth();
    let mut report = DoorReport {
        plate_too_big: plate.width > enclosure.width || plate.height > enclosure.height,
        swing: enclosure.swing(),
        ..DoorReport::default()
    };
    for &(id, depth) in depths {
        match depth.filter(|d| *d > 0.0) {
            None => report.unknown_depth.push(id),
            Some(d) if d > usable => report.too_deep.push((id, d - usable)),
            Some(d) if usable - d < MIN_DOOR_GAP_MM => report.tight.push((id, usable - d)),
            Some(_) => {}
        }
    }
    if let Some(swing) = report.swing {
        report.front_short = shortfall(enclosure.front_clearance, swing.front);
        report.hinge_side_short = shortfall(enclosure.hinge_side_clearance, swing.hinge_side);
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enclosure() -> Enclosure {
        Enclosure {
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            plate_setback: 20.0,
            door: Some(Door::default()),
            front_clearance: None,
            hinge_side_clearance: None,
        }
    }

    fn plate() -> Plate {
        Plate {
            width: 560.0,
            height: 760.0,
            margin: 10.0,
        }
    }

    #[test]
    fn usable_depth_is_between_plate_and_door() {
        let mut e = enclosure();
        assert_eq!(e.usable_depth(), 230.0);
        e.door = Some(Door {
            inner_depth: 30.0,
            ..Door::default()
        });
        assert_eq!(e.usable_depth(), 200.0);
    }

    #[test]
    fn depth_is_judged_against_usable_depth() {
        let depths = [
            (1, Some(100.0)),
            (2, Some(225.0)),
            (3, Some(260.0)),
            (4, None),
            (5, Some(0.0)),
        ];
        let r = check(&enclosure(), &plate(), &depths);
        assert_eq!(r.too_deep, vec![(3, 30.0)]);
        assert_eq!(r.tight, vec![(2, 5.0)]);
        assert_eq!(r.unknown_depth, vec![4, 5]);
        assert_eq!(r.problems(), 2);
    }

    #[test]
    fn a_door_at_ninety_degrees_needs_its_width_in_front() {
        let s = enclosure().swing().unwrap();
        assert!((s.front - 600.0).abs() < 1e-9);
        assert!(s.hinge_side.abs() < 1e-9);
    }

    #[test]
    fn a_door_past_ninety_needs_room_beside_the_hinge() {
        let mut e = enclosure();
        e.door = Some(Door {
            max_angle: 120.0,
            ..Door::default()
        });
        let s = e.swing().unwrap();
        assert!((s.front - 600.0).abs() < 1e-9);
        assert!((s.hinge_side - 300.0).abs() < 1e-6);
    }

    #[test]
    fn a_small_opening_angle_needs_less_in_front() {
        let mut e = enclosure();
        e.door = Some(Door {
            max_angle: 30.0,
            ..Door::default()
        });
        assert!((e.swing().unwrap().front - 300.0).abs() < 1e-6);
    }

    #[test]
    fn clearances_report_the_shortfall() {
        let mut e = enclosure();
        e.front_clearance = Some(450.0);
        e.hinge_side_clearance = Some(0.0);
        let r = check(&e, &plate(), &[]);
        assert_eq!(r.front_short, Some(150.0));
        assert_eq!(r.hinge_side_short, None);
        e.front_clearance = Some(600.0);
        assert!(check(&e, &plate(), &[]).is_clean());
    }

    #[test]
    fn no_door_means_no_swing_and_no_arc() {
        let mut e = enclosure();
        e.door = None;
        e.front_clearance = Some(0.0);
        assert!(e.swing().is_none());
        assert!(e.arc(8).is_empty());
        assert!(check(&e, &plate(), &[]).is_clean());
    }

    #[test]
    fn the_arc_starts_at_the_closed_door_and_ends_at_the_open_one() {
        let mut e = enclosure();
        let a = e.arc(4);
        assert_eq!(a.len(), 5);
        // Hinge left: the free edge starts at the right wall, ends in front of the hinge.
        assert!((a[0].0 - 600.0).abs() < 1e-9 && (a[0].1 - 250.0).abs() < 1e-9);
        assert!(a[4].0.abs() < 1e-9 && (a[4].1 - 850.0).abs() < 1e-9);
        e.door = Some(Door {
            hinge: Hinge::Right,
            ..Door::default()
        });
        let a = e.arc(4);
        assert!(a[0].0.abs() < 1e-9);
        assert!((a[4].0 - 600.0).abs() < 1e-9);
    }

    #[test]
    fn a_plate_bigger_than_the_interior_is_a_problem() {
        let mut p = plate();
        p.width = 620.0;
        assert!(check(&enclosure(), &p, &[]).plate_too_big);
    }

    #[test]
    fn hinge_text_round_trips() {
        assert_eq!(Hinge::parse(Hinge::Right.as_str()), Some(Hinge::Right));
        assert_eq!(Hinge::parse(" LEFT "), Some(Hinge::Left));
        assert_eq!(Hinge::parse("top"), None);
    }
}
