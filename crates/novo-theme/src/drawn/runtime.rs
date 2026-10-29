//! Icons registered while the program runs: the extension point for add-ons.
//!
//! The 40 built-in icons are static data. An add-on (the add-on manager, the
//! controller panel) brings its own icons and registers them here at load;
//! after that they are found by name exactly like the built-in ones
//! ([`Icon::by_name`]), paint with the same engine ([`paint`]) and can be the
//! picture of a command ([`crate::presentation::register`]).
//!
//! Rules, so an add-on cannot break the look of the set:
//! - the name is namespaced, `<addon-id>.<name>` (lower case, digits, hyphens), so it
//!   can never be one of the built-in names, which have no colon;
//! - every centre line lies inside the 24 grid (a stroke on the edge reaches
//!   0.75 past it);
//! - the group is always [`Group::Addon`].
//!
//! Registered icons live until the program ends: the data is leaked so the
//! painter can keep handing out `&'static Icon`. [`unregister_prefix`] makes an
//! unloaded add-on's icons unfindable; the few bytes stay. An add-on that
//! reloads registers the same names again, which replaces the old entry.

use std::sync::RwLock;

use super::{GRID, Group, Icon, Seg, Shape};

/// A shape as an add-on describes it: owned, so it can be built at run time.
/// Same meaning as [`Shape`], same 24 grid.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeSpec {
    Line([f32; 4]),
    Poly(Vec<[f32; 2]>, bool),
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        filled: bool,
    },
    Path {
        start: [f32; 2],
        segs: Vec<Seg>,
        closed: bool,
    },
    Dashed {
        shape: Box<ShapeSpec>,
        on: f32,
        off: f32,
    },
}

/// The most shapes an add-on icon may have (the busiest built-in has 9).
pub const MAX_SHAPES: usize = 48;

/// What an add-on registers for one icon.
#[derive(Debug, Clone, PartialEq)]
pub struct IconSpec {
    /// `<addon-id>.<name>`.
    pub name: String,
    /// The tooltip name.
    pub title: String,
    pub shapes: Vec<ShapeSpec>,
}

/// Why an icon was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconError {
    /// Not `<addon-id>.<name>` in lower case, digits, hyphens and underscores.
    BadName,
    NoTitle,
    NoShapes,
    /// More than [`MAX_SHAPES`] shapes.
    TooManyShapes,
    /// A coordinate is not a number, or the icon leaves the 24 grid.
    OutOfGrid,
    /// A dash, radius or size that cannot be drawn.
    BadShape,
}

impl std::fmt::Display for IconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            IconError::BadName => "the icon name must look like <addon-id>.<name> (lower case)",
            IconError::NoTitle => "the icon needs a title",
            IconError::NoShapes => "the icon draws nothing",
            IconError::TooManyShapes => "the icon has more than 48 shapes",
            IconError::OutOfGrid => "the icon leaves the 24 by 24 grid",
            IconError::BadShape => "a shape of the icon cannot be drawn",
        })
    }
}

impl std::error::Error for IconError {}

static REGISTERED: RwLock<Vec<&'static Icon>> = RwLock::new(Vec::new());

fn leak<T>(items: Vec<T>) -> &'static [T] {
    Box::leak(items.into_boxed_slice())
}

fn leak_shape(spec: &ShapeSpec) -> Shape {
    match spec {
        ShapeSpec::Line(l) => Shape::Line(*l),
        ShapeSpec::Poly(points, closed) => Shape::Poly(leak(points.clone()), *closed),
        ShapeSpec::Rect { x, y, w, h, r } => Shape::Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            r: *r,
        },
        ShapeSpec::Circle { cx, cy, r, filled } => Shape::Circle {
            cx: *cx,
            cy: *cy,
            r: *r,
            filled: *filled,
        },
        ShapeSpec::Path {
            start,
            segs,
            closed,
        } => Shape::Path {
            start: *start,
            segs: leak(segs.clone()),
            closed: *closed,
        },
        ShapeSpec::Dashed { shape, on, off } => Shape::Dashed {
            shape: Box::leak(Box::new(leak_shape(shape))),
            on: *on,
            off: *off,
        },
    }
}

/// `<addon-id>.<name>`, the form the add-on manager validates: both parts
/// non-empty, lower case letters, digits, `-` and `_`. A built-in icon name has
/// no dot, so the two can never meet.
pub fn valid_name(name: &str) -> bool {
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    matches!(name.split_once('.'), Some((a, b)) if part(a) && part(b))
}

/// Every number of the shape is a real number (egui's bounds ignore NaN, so the
/// grid check alone would let one through).
fn finite(spec: &ShapeSpec) -> bool {
    let pt = |p: &[f32; 2]| p.iter().all(|v| v.is_finite());
    match spec {
        ShapeSpec::Line(l) => l.iter().all(|v| v.is_finite()),
        ShapeSpec::Poly(points, _) => points.iter().all(pt),
        ShapeSpec::Rect { x, y, w, h, r } => [x, y, w, h, r].iter().all(|v| v.is_finite()),
        ShapeSpec::Circle { cx, cy, r, .. } => [cx, cy, r].iter().all(|v| v.is_finite()),
        ShapeSpec::Path { start, segs, .. } => {
            pt(start)
                && segs.iter().all(|seg| match seg {
                    Seg::Line(p) => pt(p),
                    Seg::Arc {
                        cx,
                        cy,
                        rx,
                        ry,
                        from,
                        to,
                    } => [cx, cy, rx, ry, from, to].iter().all(|v| v.is_finite()),
                    Seg::Cubic(c) => c.iter().all(pt),
                })
        }
        ShapeSpec::Dashed { shape, on, off } => on.is_finite() && off.is_finite() && finite(shape),
    }
}

fn check_shape(spec: &ShapeSpec) -> Result<(), IconError> {
    let ok_dash = |on: f32, off: f32| on.is_finite() && off.is_finite() && on > 0.0 && off > 0.0;
    match spec {
        ShapeSpec::Rect { w, h, r, .. } if *w <= 0.0 || *h <= 0.0 || *r < 0.0 => {
            Err(IconError::BadShape)
        }
        ShapeSpec::Circle { r, .. } if *r <= 0.0 => Err(IconError::BadShape),
        ShapeSpec::Poly(points, _) if points.len() < 2 => Err(IconError::BadShape),
        ShapeSpec::Dashed { on, off, .. } if !ok_dash(*on, *off) => Err(IconError::BadShape),
        ShapeSpec::Dashed { shape, .. } => check_shape(shape),
        _ => Ok(()),
    }
}

/// Registers an icon (or replaces the one of the same name) and returns it.
pub fn register(spec: IconSpec) -> Result<&'static Icon, IconError> {
    if !valid_name(&spec.name) {
        return Err(IconError::BadName);
    }
    if spec.title.trim().is_empty() {
        return Err(IconError::NoTitle);
    }
    if spec.shapes.is_empty() {
        return Err(IconError::NoShapes);
    }
    if spec.shapes.len() > MAX_SHAPES {
        return Err(IconError::TooManyShapes);
    }
    for shape in &spec.shapes {
        if !finite(shape) {
            return Err(IconError::OutOfGrid);
        }
        check_shape(shape)?;
    }
    let shapes: Vec<Shape> = spec.shapes.iter().map(leak_shape).collect();
    let probe = Icon {
        name: "probe",
        group: Group::Addon,
        title: "probe",
        shapes: leak(shapes.clone()),
    };
    let bounds = probe.bounds();
    // Every centre line inside the 24 grid, the rule the add-on manager checks
    // a manifest against (a stroke on the edge reaches 0.75 past it, which the
    // painter allows).
    let finite = [bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y]
        .iter()
        .all(|v| v.is_finite());
    let eps = 1e-2;
    if !finite
        || bounds.min.x < -eps
        || bounds.min.y < -eps
        || bounds.max.x > GRID + eps
        || bounds.max.y > GRID + eps
    {
        return Err(IconError::OutOfGrid);
    }
    let icon: &'static Icon = Box::leak(Box::new(Icon {
        name: Box::leak(spec.name.into_boxed_str()),
        group: Group::Addon,
        title: Box::leak(spec.title.into_boxed_str()),
        shapes: leak(shapes),
    }));
    let mut all = REGISTERED.write().unwrap_or_else(|e| e.into_inner());
    all.retain(|i| i.name != icon.name);
    all.push(icon);
    Ok(icon)
}

/// Forgets every registered icon (the add-on set changed: the caller registers
/// the running add-ons' icons again). Returns how many were removed.
pub fn clear() -> usize {
    let mut all = REGISTERED.write().unwrap_or_else(|e| e.into_inner());
    let n = all.len();
    all.clear();
    n
}

/// Forgets every registered icon whose name starts with `prefix` (an add-on
/// unloading passes `"myaddon:"`). Returns how many were removed.
pub fn unregister_prefix(prefix: &str) -> usize {
    let mut all = REGISTERED.write().unwrap_or_else(|e| e.into_inner());
    let before = all.len();
    all.retain(|i| !i.name.starts_with(prefix));
    before - all.len()
}

/// The registered icon called `name`.
pub fn lookup(name: &str) -> Option<&'static Icon> {
    let all = REGISTERED.read().unwrap_or_else(|e| e.into_inner());
    all.iter().copied().find(|i| i.name == name)
}

/// Every registered icon, in registration order.
pub fn registered() -> Vec<&'static Icon> {
    REGISTERED.read().unwrap_or_else(|e| e.into_inner()).clone()
}
