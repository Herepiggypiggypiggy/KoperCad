//! An add-on's own icons: shape data on the program's 24 x 24 grid, never pixels or SVG
//! (`docs/design/visual-redesign-addons.md` section 4). The host paints them with the same
//! icon engine as the built-in set, so stroke, colour, hover, light theme and High contrast need
//! nothing from the add-on.
//!
//! ```toml
//! [[icon]]
//! name = "controllers.pendant"      # <add-on id>.<name>
//! tooltip = "Pendant"
//! shape = [
//!   { kind = "line", x1 = 12, y1 = 2.5, x2 = 12, y2 = 7.5 },
//!   { kind = "rect", x = 6, y = 7.5, w = 12, h = 14, r = 2 },
//!   { kind = "circle", cx = 12, cy = 12.5, r = 2.3 },
//! ]
//! ```

use serde::{Deserialize, Serialize};

use crate::error::{AddonError, Result};
use crate::manifest::valid_icon_name;

/// Most shapes one icon may have.
pub const MAX_SHAPES: usize = 48;
/// Most icons one add-on may ship.
pub const MAX_ICONS: usize = 64;
/// Largest `icons.toml`.
pub const MAX_BYTES: usize = 128 * 1024;
/// The filled dot (a junction) may not be larger than this radius.
const MAX_DOT_RADIUS: f32 = 1.5;

/// One drawn element on the 24 grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Shape {
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
    },
    Poly {
        points: Vec<[f32; 2]>,
        #[serde(default)]
        closed: bool,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        #[serde(default)]
        r: f32,
    },
    Circle {
        cx: f32,
        cy: f32,
        r: f32,
        /// Only for a small junction dot.
        #[serde(default)]
        filled: bool,
    },
    /// An arc of a circle, angles in degrees, counter-clockwise from the +x axis.
    Arc {
        cx: f32,
        cy: f32,
        r: f32,
        from: f32,
        to: f32,
    },
}

/// One icon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IconDef {
    /// `<add-on id>.<name>`.
    pub name: String,
    #[serde(default)]
    pub tooltip: String,
    pub shape: Vec<Shape>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    #[serde(default)]
    icon: Vec<IconDef>,
}

/// Parses and validates `data/icons.toml` of add-on `addon_id`.
pub fn parse(addon_id: &str, bytes: &[u8]) -> Result<Vec<IconDef>> {
    let bad = |m: String| {
        Err(AddonError::Manifest(format!(
            "{}: {m}",
            crate::manifest::ICONS_FILE
        )))
    };
    if bytes.len() > MAX_BYTES {
        return bad("larger than 128 KiB".into());
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return bad("not UTF-8 text".into());
    };
    let file: File = toml::from_str(text)
        .map_err(|e| AddonError::Manifest(format!("{}: {e}", crate::manifest::ICONS_FILE)))?;
    if file.icon.len() > MAX_ICONS {
        return bad(format!("more than {MAX_ICONS} icons"));
    }
    let prefix = format!("{addon_id}.");
    let mut names = std::collections::BTreeSet::new();
    for icon in &file.icon {
        if !icon.name.starts_with(&prefix) || !valid_icon_name(&icon.name) {
            return bad(format!("icon name {:?} must be {prefix}<name>", icon.name));
        }
        if !names.insert(icon.name.as_str()) {
            return bad(format!("icon {} is defined twice", icon.name));
        }
        if icon.tooltip.chars().count() > 40 {
            return bad(format!(
                "the tooltip of {} is longer than 40 characters",
                icon.name
            ));
        }
        if icon.shape.is_empty() || icon.shape.len() > MAX_SHAPES {
            return bad(format!("{} needs 1 to {MAX_SHAPES} shapes", icon.name));
        }
        for shape in &icon.shape {
            if let Err(why) = check_shape(shape) {
                return bad(format!("{}: {why}", icon.name));
            }
        }
    }
    Ok(file.icon)
}

fn on_grid(v: f32) -> bool {
    v.is_finite() && (0.0..=24.0).contains(&v)
}

fn check_shape(shape: &Shape) -> std::result::Result<(), String> {
    let ok = match shape {
        Shape::Line { x1, y1, x2, y2 } => [x1, y1, x2, y2].iter().all(|v| on_grid(**v)),
        Shape::Poly { points, .. } => {
            if !(2..=64).contains(&points.len()) {
                return Err("a polyline needs 2 to 64 points".into());
            }
            points.iter().all(|p| on_grid(p[0]) && on_grid(p[1]))
        }
        Shape::Rect { x, y, w, h, r } => {
            on_grid(*x)
                && on_grid(*y)
                && *w > 0.0
                && *h > 0.0
                && on_grid(x + w)
                && on_grid(y + h)
                && (0.0..=12.0).contains(r)
        }
        Shape::Circle { cx, cy, r, filled } => {
            on_grid(*cx)
                && on_grid(*cy)
                && *r > 0.0
                && on_grid(cx - r)
                && on_grid(cx + r)
                && on_grid(cy - r)
                && on_grid(cy + r)
                && (!*filled || *r <= MAX_DOT_RADIUS)
        }
        Shape::Arc {
            cx,
            cy,
            r,
            from,
            to,
        } => {
            on_grid(*cx)
                && on_grid(*cy)
                && *r > 0.0
                && on_grid(cx - r)
                && on_grid(cx + r)
                && on_grid(cy - r)
                && on_grid(cy + r)
                && from.is_finite()
                && to.is_finite()
        }
    };
    if ok {
        Ok(())
    } else {
        Err(format!(
            "a shape leaves the 0 to 24 grid, has a non-positive size, or is a filled shape larger than a junction dot ({shape:?})"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
[[icon]]
name = "demo.radio"
tooltip = "Radio"
shape = [
  { kind = "rect", x = 6, y = 11.5, w = 12, h = 9.5, r = 2 },
  { kind = "circle", cx = 12, cy = 7, r = 1, filled = true },
  { kind = "poly", points = [[9.5, 16.2], [14.5, 16.2]] },
  { kind = "arc", cx = 12, cy = 7, r = 4.6, from = 135, to = 225 },
  { kind = "line", x1 = 12, y1 = 2.5, x2 = 12, y2 = 5 },
]
"#;

    #[test]
    fn good_icons_parse() {
        let icons = parse("demo", GOOD.as_bytes()).unwrap();
        assert_eq!(icons.len(), 1);
        assert_eq!(icons[0].shape.len(), 5);
    }

    fn rejected(from: &str, to: &str) -> String {
        assert!(GOOD.contains(from), "{from}");
        parse("demo", GOOD.replace(from, to).as_bytes())
            .expect_err("accepted")
            .to_string()
    }

    #[test]
    fn rules_hold() {
        assert!(rejected("demo.radio", "other.radio").contains("must be demo."));
        assert!(rejected("x = 6", "x = 20").contains("grid"));
        assert!(rejected("r = 1, filled = true", "r = 4, filled = true").contains("junction dot"));
        assert!(rejected("cy = 7, r = 1,", "cy = 7, r = -1,").contains("grid"));
        assert!(rejected("y1 = 2.5", "y1 = 25").contains("grid"));
        assert!(rejected("kind = \"line\"", "kind = \"image\"").contains("image"));
        assert!(
            rejected(
                "tooltip = \"Radio\"",
                "tooltip = \"Radio\"\ncolor = \"red\""
            )
            .contains("color")
        );
        assert!(rejected("[9.5, 16.2], [14.5, 16.2]", "[9.5, 16.2]").contains("2 to 64"));
        let many = format!(
            "[[icon]]\nname = \"demo.x\"\nshape = [{}]",
            vec!["{ kind = \"line\", x1 = 1, y1 = 1, x2 = 2, y2 = 2 }"; 49].join(",")
        );
        assert!(
            parse("demo", many.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("shapes")
        );
        let nan = "[[icon]]\nname = \"demo.x\"\nshape = [{ kind = \"line\", x1 = nan, y1 = 1, x2 = 2, y2 = 2 }]";
        assert!(parse("demo", nan.as_bytes()).is_err());
    }
}
