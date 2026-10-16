//! Door swing sweep (panel layout lane B, PANEL-B2): devices mounted on the
//! inside of the door are turned about the hinge from closed to the door's
//! largest opening angle and tested against the devices on the plate and the
//! enclosure walls. The first angle at which a door device touches
//! something is reported, with the rule that was broken.
//!
//! Planning geometry like `enclosure.rs`: the door opens outwards about a
//! vertical hinge axis at the inside edge of the hinge-side wall, or
//! `hinge_offset` mm outside it ([`SweepSettings`]). Walls are thin slabs, so
//! only a device that reaches past the strike-side wall edge or the hinge
//! side wall is caught there; the door leaf itself is not swept.

use crate::Rect;
use crate::collide::{Body, Mesh, collide};
use crate::enclosure::{Enclosure, Hinge};
use crate::surfaces::{Surface, SurfaceKind, Surfaces};

/// Rule name of a door device that touches something while the door moves.
pub const RULE_DOOR_SWING: &str = "door.swing";
/// Rule name of a door device that touches something with the door closed.
pub const RULE_DOOR_CLOSED: &str = "door.closed";

/// Thickness given to the wall slabs that door devices are tested against.
const WALL_SLAB_MM: f64 = 2.0;
/// Angle step of the coarse sweep, in degrees.
const STEP_DEG: f64 = 1.0;
/// Bisection stops when the touch angle is known to this many degrees.
const ANGLE_TOLERANCE_DEG: f64 = 0.01;

/// A device on the inside of the door. `rect` is in door coordinates seen
/// from the front (x from the left edge, y down); `depth` stands out into
/// the enclosure. Without a `mesh` the box from rect and depth is used.
#[derive(Debug, Clone, PartialEq)]
pub struct DoorDevice {
    pub id: u64,
    pub rect: Rect,
    pub depth: f64,
    pub mesh: Option<Mesh>,
}

/// A device on the mounting plate (or another surface), as an obstacle.
#[derive(Debug, Clone, PartialEq)]
pub struct PlateDevice {
    pub id: u64,
    pub surface: String,
    pub rect: Rect,
    pub depth: f64,
    pub mesh: Option<Mesh>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepSettings {
    /// Distance of the hinge axis outside the hinge-side wall, in mm.
    pub hinge_offset: f64,
}

impl Default for SweepSettings {
    fn default() -> Self {
        SweepSettings { hinge_offset: 0.0 }
    }
}

/// What a door device touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Obstacle {
    Device(u64),
    /// A wall, roof or floor, by surface name.
    Wall(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SwingHit {
    pub door_device: u64,
    pub obstacle: Obstacle,
    /// Door angle in degrees at which they first touch; 0 means they
    /// already touch with the door closed.
    pub angle_deg: f64,
    /// A box stood in for a missing mesh of the door device or the device
    /// it touched, so the angle is a worst case.
    pub approximate: bool,
}

impl SwingHit {
    pub fn rule(&self) -> &'static str {
        if self.angle_deg <= 0.0 {
            RULE_DOOR_CLOSED
        } else {
            RULE_DOOR_SWING
        }
    }

    /// One sentence for the Issues list; `tag_of` names a device id.
    pub fn message(&self, tag_of: &impl Fn(u64) -> String) -> String {
        let what = match &self.obstacle {
            Obstacle::Device(id) => tag_of(*id),
            Obstacle::Wall(name) => format!("the {name}"),
        };
        let door = tag_of(self.door_device);
        let worst = if self.approximate {
            " (worst case, a part has no 3D shape)"
        } else {
            ""
        };
        if self.angle_deg <= 0.0 {
            format!(
                "{door} on the door touches {what} with the door closed{worst} [{}].",
                self.rule()
            )
        } else {
            format!(
                "{door} on the door touches {what} when the door is open {:.1} degrees{worst} [{}].",
                self.angle_deg,
                self.rule()
            )
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SwingReport {
    /// The largest opening angle swept; 0 without a door.
    pub max_angle_deg: f64,
    /// First touch per door device and obstacle, earliest angle first.
    pub hits: Vec<SwingHit>,
    /// Door devices without a positive depth; they were not swept.
    pub unknown_depth: Vec<u64>,
    /// How many door devices were swept.
    pub swept: usize,
    /// Placed plate devices with no known depth: they could not be used as
    /// obstacles, so a clear result does not cover them.
    pub unchecked_plate: Vec<u64>,
}

impl SwingReport {
    pub fn problems(&self) -> usize {
        self.hits.len()
    }

    pub fn is_clean(&self) -> bool {
        self.hits.is_empty()
    }
}

struct Target {
    obstacle: Obstacle,
    body: Body,
}

/// Sweeps `door_devices` through the door's opening range against
/// `plate_devices` and the walls. No door, or no door devices, gives an
/// empty clean report.
pub fn sweep(
    enclosure: &Enclosure,
    surfaces: &Surfaces,
    door_devices: &[DoorDevice],
    plate_devices: &[PlateDevice],
    settings: &SweepSettings,
) -> SwingReport {
    let mut report = SwingReport::default();
    let (Some(door), Some(door_surface)) =
        (enclosure.door, surfaces.of_kind(SurfaceKind::Door).next())
    else {
        return report;
    };
    report.max_angle_deg = door.max_angle.clamp(0.0, 180.0);

    let mut targets: Vec<Target> = Vec::new();
    for d in plate_devices {
        let Some(s) = surfaces.find(Some(&d.surface)) else {
            continue;
        };
        if s.kind == SurfaceKind::Door || d.depth <= 0.0 {
            continue;
        }
        targets.push(Target {
            obstacle: Obstacle::Device(d.id),
            body: s.device_body(&d.rect, d.depth, d.mesh.clone()),
        });
    }
    for s in &surfaces.list {
        if matches!(
            s.kind,
            SurfaceKind::LeftWall | SurfaceKind::RightWall | SurfaceKind::Roof | SurfaceKind::Floor
        ) {
            targets.push(Target {
                obstacle: Obstacle::Wall(s.name.clone()),
                body: s.slab(WALL_SLAB_MM),
            });
        }
    }

    let (pivot_x, sign) = match door.hinge {
        Hinge::Left => (-settings.hinge_offset, 1.0),
        Hinge::Right => (enclosure.width + settings.hinge_offset, -1.0),
    };
    let pivot_z = enclosure.depth;

    for dd in door_devices {
        if dd.depth <= 0.0 {
            report.unknown_depth.push(dd.id);
            continue;
        }
        report.swept += 1;
        let closed = door_surface.device_body(&dd.rect, dd.depth, dd.mesh.clone());
        for t in &targets {
            let at = |deg: f64| -> bool {
                let body = if deg == 0.0 {
                    closed.clone()
                } else {
                    closed.rotated_y(pivot_x, pivot_z, sign * deg.to_radians())
                };
                collide(&body, &t.body).is_some()
            };
            if let Some(angle) = first_touch(&at, report.max_angle_deg) {
                let device_fallback =
                    matches!(t.obstacle, Obstacle::Device(_)) && t.body.is_fallback();
                report.hits.push(SwingHit {
                    door_device: dd.id,
                    obstacle: t.obstacle.clone(),
                    angle_deg: angle,
                    approximate: closed.is_fallback() || device_fallback,
                });
            }
        }
    }
    report
        .hits
        .sort_by(|a, b| a.angle_deg.total_cmp(&b.angle_deg));
    report
}

/// Smallest angle in `0..=max` where `touches` holds, found with a coarse
/// sweep and then bisection. `None` when it never does.
fn first_touch(touches: &impl Fn(f64) -> bool, max: f64) -> Option<f64> {
    if touches(0.0) {
        return Some(0.0);
    }
    let mut prev = 0.0;
    let mut deg = STEP_DEG.min(max);
    loop {
        if deg > 0.0 && touches(deg) {
            let (mut lo, mut hi) = (prev, deg);
            while hi - lo > ANGLE_TOLERANCE_DEG {
                let mid = (lo + hi) / 2.0;
                if touches(mid) {
                    hi = mid;
                } else {
                    lo = mid;
                }
            }
            return Some(hi);
        }
        if deg >= max {
            return None;
        }
        prev = deg;
        deg = (deg + STEP_DEG).min(max);
    }
}

/// The surface a [`DoorDevice`] sits on, for callers that build rects.
pub fn door_surface(surfaces: &Surfaces) -> Option<&Surface> {
    surfaces.of_kind(SurfaceKind::Door).next()
}
