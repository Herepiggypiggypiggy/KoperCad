//! How add-on commands look: their icons and short labels join the same tables
//! the built-in commands read (`novo_theme::drawn` for icons,
//! `novo_theme::presentation` for the command rows), so the rail, the menus and
//! Ctrl K draw an add-on's command exactly like a built-in one
//! (docs/design/visual-redesign-addons.md section 4).
//!
//! The add-on manager owns what add-ons contribute (`novo_addons::Extensions`,
//! rebuilt whenever one is installed, turned on or off or removed). This file is
//! the one place that copies it into the look tables, and it does so by
//! replacing, never appending: a disabled or removed add-on leaves nothing
//! behind.
//!
//! Icon resolution for a command: the add-on's own icon, then a built-in drawn
//! icon of that name, then the add-on glyph (`presentation::ADDON_ICON`). A
//! command whose icon name is unknown, or whose own icon the painter refuses, is
//! never left without a picture.
//!
//! Where the user sees it: Icon gallery (Ctrl K) lists the registered icons and
//! commands; the rail, app menu and palette lanes read the same tables.

use novo_addons::Extensions;
use novo_addons::icons::{IconDef, Shape as DefShape};
use novo_theme::drawn::{self, IconSpec, Seg, ShapeSpec};
use novo_theme::presentation;

/// An add-on's icon shape in the painter's terms. The add-on file gives arcs in
/// degrees, counter-clockwise on screen from the +x axis; the painter's angles
/// are radians that grow clockwise (y down), so the angle changes sign.
fn shape(def: &DefShape) -> ShapeSpec {
    match def {
        DefShape::Line { x1, y1, x2, y2 } => ShapeSpec::Line([*x1, *y1, *x2, *y2]),
        DefShape::Poly { points, closed } => ShapeSpec::Poly(points.clone(), *closed),
        DefShape::Rect { x, y, w, h, r } => ShapeSpec::Rect {
            x: *x,
            y: *y,
            w: *w,
            h: *h,
            r: *r,
        },
        DefShape::Circle { cx, cy, r, filled } => ShapeSpec::Circle {
            cx: *cx,
            cy: *cy,
            r: *r,
            filled: *filled,
        },
        DefShape::Arc {
            cx,
            cy,
            r,
            from,
            to,
        } => {
            let (from, to) = (-from.to_radians(), -to.to_radians());
            ShapeSpec::Path {
                start: [cx + r * from.cos(), cy + r * from.sin()],
                segs: vec![Seg::Arc {
                    cx: *cx,
                    cy: *cy,
                    rx: *r,
                    ry: *r,
                    from,
                    to,
                }],
                closed: false,
            }
        }
    }
}

/// The painter's description of an add-on icon.
pub(crate) fn icon_spec(def: &IconDef) -> IconSpec {
    IconSpec {
        name: def.name.clone(),
        title: if def.tooltip.is_empty() {
            def.name.clone()
        } else {
            def.tooltip.clone()
        },
        shapes: def.shape.iter().map(shape).collect(),
    }
}

/// What [`sync`] did, for the log and the tests.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Synced {
    pub icons: usize,
    pub commands: usize,
    /// Icons the painter refused (the add-on glyph stands in for them).
    pub refused: Vec<String>,
    /// Commands whose label the table refused.
    pub refused_commands: Vec<String>,
}

/// Replaces the add-on icons and command rows with those of `ext`.
pub(crate) fn sync(ext: &Extensions) -> Synced {
    drawn::clear();
    presentation::clear();
    let mut done = Synced::default();
    for def in ext.icons.values() {
        match drawn::register(icon_spec(def)) {
            Ok(_) => done.icons += 1,
            Err(e) => {
                log::warn!("add-on icon {} refused: {e}", def.name);
                done.refused.push(def.name.clone());
            }
        }
    }
    for c in &ext.commands {
        let icon = (!c.icon.is_empty()).then_some(c.icon.as_str());
        match presentation::register_extension(&c.id, icon, &c.label) {
            Ok(_) => done.commands += 1,
            Err(e) => {
                log::warn!("add-on command {} not shown with an icon: {e}", c.id);
                done.refused_commands.push(c.id.clone());
            }
        }
    }
    done
}

/// The tables are global, so the tests that fill and clear them take turns.
#[cfg(test)]
pub(crate) static TABLES: std::sync::Mutex<()> = std::sync::Mutex::new(());
