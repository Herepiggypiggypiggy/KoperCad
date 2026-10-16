//! Mounting surfaces of an enclosure (panel layout lane B): the plate, the
//! door, the side walls, roof, floor and mounting profiles.
//!
//! Every surface is a rectangle in the 3D frame of `enclosure.rs`: `x`
//! across the front from the left, `y` down, `z` from the back wall towards
//! the door; millimetres. A surface has its own 2D frame (`u`, `v`) so the
//! 2D layout engine works on it unchanged, and a `normal` that points into
//! the enclosure, the way a device mounted on it stands out.
//!
//! No data model change: a placement names its surface with the property
//! [`SURFACE_PROPERTY`] (absent means the main plate) and surface sizes are
//! structure properties `surface.<name>` = `"w h margin"`, optionally
//! followed by an origin `"w h margin x y z"` for surfaces whose place the
//! enclosure cannot derive (extra plates, profiles).
//!
//! Planning geometry, like `enclosure.rs`: walls have no thickness and the
//! plate is centred in the interior.

use crate::collide::{Body, Mesh};
use crate::enclosure::Enclosure;
use crate::{Plate, Rect};

/// Placement property that names the surface a device is mounted on.
pub const SURFACE_PROPERTY: &str = "surface";
/// Prefix of the structure properties that carry surface sizes.
pub const SURFACE_SIZE_PREFIX: &str = "surface.";
/// Name of the main mounting plate, also the surface when none is named.
pub const MAIN_PLATE: &str = "plate";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SurfaceKind {
    Plate,
    Door,
    LeftWall,
    RightWall,
    Roof,
    Floor,
    Profile,
}

impl SurfaceKind {
    pub fn label(self) -> &'static str {
        match self {
            SurfaceKind::Plate => "plate",
            SurfaceKind::Door => "door",
            SurfaceKind::LeftWall => "left wall",
            SurfaceKind::RightWall => "right wall",
            SurfaceKind::Roof => "roof",
            SurfaceKind::Floor => "floor",
            SurfaceKind::Profile => "profile",
        }
    }

    /// Kind of a surface name: `plate`, `plate2`, `door`, `left`, `right`,
    /// `roof`, `floor`, `profile`, `profile3`. `None` for anything else.
    pub fn from_name(name: &str) -> Option<SurfaceKind> {
        let name = name.trim().to_ascii_lowercase();
        let stem = name.trim_end_matches(|c: char| c.is_ascii_digit());
        let has_number = stem.len() != name.len();
        match stem {
            "plate" => Some(SurfaceKind::Plate),
            "profile" => Some(SurfaceKind::Profile),
            "door" | "left" | "right" | "roof" | "floor" if !has_number => match stem {
                "door" => Some(SurfaceKind::Door),
                "left" => Some(SurfaceKind::LeftWall),
                "right" => Some(SurfaceKind::RightWall),
                "roof" => Some(SurfaceKind::Roof),
                _ => Some(SurfaceKind::Floor),
            },
            _ => None,
        }
    }
}

/// A mounting rectangle in the enclosure's 3D frame.
#[derive(Debug, Clone, PartialEq)]
pub struct Surface {
    pub name: String,
    pub kind: SurfaceKind,
    /// The corner where `u` and `v` are 0.
    pub origin: [f64; 3],
    /// Unit vector of the surface's x direction.
    pub u: [f64; 3],
    /// Unit vector of the surface's y direction.
    pub v: [f64; 3],
    /// Unit vector pointing into the enclosure, away from the surface.
    pub normal: [f64; 3],
    pub width: f64,
    pub height: f64,
    /// Margin kept free along the edges, as for `Plate`.
    pub margin: f64,
    /// The size or origin was not given and was assumed.
    pub assumed: bool,
}

impl Surface {
    /// The surface's 2D plate, so the 2D engine and checks work on it.
    pub fn plate(&self) -> Plate {
        Plate {
            width: self.width,
            height: self.height,
            margin: self.margin,
        }
    }

    pub fn usable(&self) -> Rect {
        self.plate().usable()
    }

    /// The 3D point of surface coordinates (`x`, `y`) lifted `height` mm
    /// off the surface.
    pub fn point(&self, x: f64, y: f64, height: f64) -> [f64; 3] {
        let mut p = self.origin;
        for (i, c) in p.iter_mut().enumerate() {
            *c += self.u[i] * x + self.v[i] * y + self.normal[i] * height;
        }
        p
    }

    /// The box of a device standing on this surface: footprint `rect` and
    /// `depth` mm out along the normal.
    pub fn device_box(&self, rect: &Rect, depth: f64) -> ([f64; 3], [f64; 3]) {
        let a = self.point(rect.x, rect.y, 0.0);
        let b = self.point(rect.right(), rect.bottom(), depth.max(0.0));
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for i in 0..3 {
            min[i] = a[i].min(b[i]);
            max[i] = a[i].max(b[i]);
        }
        (min, max)
    }

    /// A collision body for a device on this surface: its mesh (already in
    /// the enclosure frame) or, without one, the box from footprint and
    /// depth, flagged as a fallback.
    pub fn device_body(&self, rect: &Rect, depth: f64, mesh: Option<Mesh>) -> Body {
        let (min, max) = self.device_box(rect, depth);
        Body::from_mesh_or_box(mesh, min, max)
    }

    /// The wall itself as a thin slab, for collision with things that swing
    /// or stand out. `thickness` goes outwards, away from the interior.
    pub fn slab(&self, thickness: f64) -> Body {
        let a = self.point(0.0, 0.0, 0.0);
        let b = self.point(self.width, self.height, -thickness.max(0.1));
        let mut min = [0.0; 3];
        let mut max = [0.0; 3];
        for i in 0..3 {
            min[i] = a[i].min(b[i]);
            max[i] = a[i].max(b[i]);
        }
        Body::fallback_box(min, max)
    }
}

/// Size (and optional origin) read from a `surface.<name>` property.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceSize {
    pub width: f64,
    pub height: f64,
    pub margin: f64,
    pub origin: Option<[f64; 3]>,
}

impl SurfaceSize {
    /// `"w h margin"` or `"w h margin x y z"`; sizes must be positive and
    /// the margin non-negative.
    pub fn parse(text: &str) -> Option<SurfaceSize> {
        let n: Vec<f64> = text
            .split_whitespace()
            .map(|t| t.replace(',', ".").parse::<f64>().ok())
            .collect::<Option<_>>()?;
        if !(n.len() == 3 || n.len() == 6) || n.iter().any(|v| !v.is_finite()) {
            return None;
        }
        if n[0] <= 0.0 || n[1] <= 0.0 || n[2] < 0.0 {
            return None;
        }
        Some(SurfaceSize {
            width: n[0],
            height: n[1],
            margin: n[2],
            origin: (n.len() == 6).then(|| [n[3], n[4], n[5]]),
        })
    }

    pub fn format(&self) -> String {
        match self.origin {
            Some(o) => format!(
                "{} {} {} {} {} {}",
                self.width, self.height, self.margin, o[0], o[1], o[2]
            ),
            None => format!("{} {} {}", self.width, self.height, self.margin),
        }
    }
}

/// The property key that carries the size of the surface called `name`.
pub fn size_key(name: &str) -> String {
    format!("{SURFACE_SIZE_PREFIX}{}", name.trim().to_ascii_lowercase())
}

/// A surface a placement asked for that could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SurfaceProblem {
    /// The name is not a known surface name.
    UnknownName(String),
    /// The `surface.<name>` property is not a valid size.
    BadSize(String),
    /// A profile needs its size and origin.
    ProfileNeedsOrigin(String),
}

impl SurfaceProblem {
    pub fn message(&self) -> String {
        match self {
            SurfaceProblem::UnknownName(n) => format!(
                "Surface '{n}' is not one of plate, plateN, door, left, right, roof, floor, profileN."
            ),
            SurfaceProblem::BadSize(n) => format!(
                "Property '{SURFACE_SIZE_PREFIX}{n}' should read 'width height margin' in mm."
            ),
            SurfaceProblem::ProfileNeedsOrigin(n) => {
                format!("Profile '{n}' needs 'width height margin x y z' so it can be placed.")
            }
        }
    }
}

/// All surfaces of one enclosure.
#[derive(Debug, Clone, PartialEq)]
pub struct Surfaces {
    pub list: Vec<Surface>,
}

impl Surfaces {
    /// The standard surfaces of `enclosure` with the mounting `plate`: the
    /// plate, the walls, roof and floor, and the door when there is one.
    /// `extra` are `(name, property text)` pairs of the `surface.<name>`
    /// structure properties; they add plates and profiles and override the
    /// size of the standard surfaces. Problems are returned, never dropped
    /// silently.
    pub fn from_enclosure(
        enclosure: &Enclosure,
        plate: &Plate,
        extra: &[(String, String)],
    ) -> (Surfaces, Vec<SurfaceProblem>) {
        let (w, h, d) = (enclosure.width, enclosure.height, enclosure.depth);
        let plate_x = enclosure.plate_left(plate);
        let plate_y = ((h - plate.height) / 2.0).max(0.0);
        let mut list = vec![Surface {
            name: MAIN_PLATE.into(),
            kind: SurfaceKind::Plate,
            origin: [plate_x, plate_y, enclosure.plate_setback],
            u: [1.0, 0.0, 0.0],
            v: [0.0, 1.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            width: plate.width,
            height: plate.height,
            margin: plate.margin,
            assumed: false,
        }];
        let wall = |name: &str, kind, origin, u, v, normal, width, height| Surface {
            name: name.into(),
            kind,
            origin,
            u,
            v,
            normal,
            width,
            height,
            margin: plate.margin,
            assumed: true,
        };
        if enclosure.door.is_some() {
            // Door seen from the front; its inner face looks at the plate.
            list.push(wall(
                "door",
                SurfaceKind::Door,
                [0.0, 0.0, d],
                [1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, -1.0],
                w,
                h,
            ));
        }
        list.push(wall(
            "left",
            SurfaceKind::LeftWall,
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            d,
            h,
        ));
        list.push(wall(
            "right",
            SurfaceKind::RightWall,
            [w, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
            d,
            h,
        ));
        list.push(wall(
            "roof",
            SurfaceKind::Roof,
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0],
            w,
            d,
        ));
        list.push(wall(
            "floor",
            SurfaceKind::Floor,
            [0.0, h, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            w,
            d,
        ));

        let mut problems = Vec::new();
        // Next free x for extra plates without an origin.
        let mut next_x = plate_x + plate.width;
        for (raw_name, text) in extra {
            let name = raw_name.trim().to_ascii_lowercase();
            let Some(kind) = SurfaceKind::from_name(&name) else {
                problems.push(SurfaceProblem::UnknownName(name));
                continue;
            };
            let Some(size) = SurfaceSize::parse(text) else {
                problems.push(SurfaceProblem::BadSize(name));
                continue;
            };
            if let Some(existing) = list.iter_mut().find(|s| s.name == name) {
                existing.width = size.width;
                existing.height = size.height;
                existing.margin = size.margin;
                existing.assumed = false;
                if let Some(o) = size.origin {
                    existing.origin = o;
                }
                continue;
            }
            match kind {
                SurfaceKind::Plate => {
                    let assumed = size.origin.is_none();
                    let origin = size
                        .origin
                        .unwrap_or([next_x, plate_y, enclosure.plate_setback]);
                    next_x = origin[0] + size.width;
                    list.push(Surface {
                        name,
                        kind,
                        origin,
                        u: [1.0, 0.0, 0.0],
                        v: [0.0, 1.0, 0.0],
                        normal: [0.0, 0.0, 1.0],
                        width: size.width,
                        height: size.height,
                        margin: size.margin,
                        assumed,
                    });
                }
                SurfaceKind::Profile => {
                    let Some(origin) = size.origin else {
                        problems.push(SurfaceProblem::ProfileNeedsOrigin(name));
                        continue;
                    };
                    // A profile is a vertical bar facing the door.
                    list.push(Surface {
                        name,
                        kind,
                        origin,
                        u: [1.0, 0.0, 0.0],
                        v: [0.0, 1.0, 0.0],
                        normal: [0.0, 0.0, 1.0],
                        width: size.width,
                        height: size.height,
                        margin: size.margin,
                        assumed: false,
                    });
                }
                // Door, walls, roof and floor exist already (or the door
                // does not exist on a door-less enclosure).
                _ => problems.push(SurfaceProblem::UnknownName(name)),
            }
        }
        (Surfaces { list }, problems)
    }

    /// The surface a placement names; the main plate when the property is
    /// absent or empty. `None` when the name is not among the surfaces.
    pub fn find(&self, name: Option<&str>) -> Option<&Surface> {
        let name = name
            .map(|n| n.trim().to_ascii_lowercase())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| MAIN_PLATE.to_string());
        self.list.iter().find(|s| s.name == name)
    }

    pub fn of_kind(&self, kind: SurfaceKind) -> impl Iterator<Item = &Surface> {
        self.list.iter().filter(move |s| s.kind == kind)
    }
}
