//! A user's own crane model as the pieces the crane picture swaps in.
//!
//! The parts of a STEP crane model (see [`super::crane_roles`]) are sorted by their guessed role
//! into two pieces: the hook (block, hook) that hangs on the rope, and the hoist (drum, motor,
//! gearbox) that sits on the trolley. Each piece is one mesh at the size the file gives, in the
//! frame the 3D scene uses for a part: x and y centred, z up from 0. The rest of the model
//! (girders, rails, wheels, anything unnamed) is counted and left out for now. Nothing here
//! saves the model: it is read from the file the user points at, and the file stays where it is.

use super::build::PlacedPart;
use super::crane_roles::{CraneRole, role_of};
use super::math::V3;
use super::mesh::{MeshBudget, MeshData, Quality, levels};

/// Which way is up in the file. CAD programs differ: most mechanical ones use Z, some Y.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    #[default]
    Z,
    Y,
}

/// How the model sits in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Placement {
    pub up: UpAxis,
    /// Turn the model a quarter turn about the vertical (the drum axis runs the other way).
    pub quarter_turn: bool,
}

/// One swapped-in piece: its mesh levels (frame: x, y centred, z from 0 up) and size in mm.
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    pub lods: Vec<MeshData>,
    pub size_mm: [f32; 3],
    /// How many parts of the file went into it.
    pub parts: usize,
}

/// What a model gives the picture.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CraneModel {
    pub hook: Option<Piece>,
    pub hoist: Option<Piece>,
    /// Parts of the file that went into no piece (not used yet).
    pub left_out: usize,
    /// Total parts in the file.
    pub total: usize,
}

impl CraneModel {
    /// Whether the model changes the picture at all.
    pub fn is_usable(&self) -> bool {
        self.hook.is_some() || self.hoist.is_some()
    }
}

fn in_hoist(role: CraneRole) -> bool {
    matches!(
        role,
        CraneRole::Drum | CraneRole::Motor | CraneRole::Gearbox
    )
}

fn turned(p: V3, placement: Placement) -> V3 {
    // Y up to Z up is a quarter turn about x, which keeps the handedness (and the winding).
    let p = match placement.up {
        UpAxis::Z => p,
        UpAxis::Y => [p[0], -p[2], p[1]],
    };
    if placement.quarter_turn {
        [-p[1], p[0], p[2]]
    } else {
        p
    }
}

fn piece_of(parts: &[&PlacedPart], placement: Placement) -> Option<Piece> {
    let mut pos: Vec<V3> = Vec::new();
    let mut tris: Vec<[u32; 3]> = Vec::new();
    for part in parts {
        let base = pos.len() as u32;
        pos.extend(part.soup.pos.iter().map(|&p| turned(p, placement)));
        tris.extend(
            part.soup
                .tris
                .iter()
                .map(|t| [t[0] + base, t[1] + base, t[2] + base]),
        );
    }
    if tris.is_empty() {
        return None;
    }
    let mut lo = [f64::MAX; 3];
    let mut hi = [f64::MIN; 3];
    for p in &pos {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    if (0..3).any(|k| !lo[k].is_finite() || !hi[k].is_finite()) {
        return None;
    }
    let shift = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5, lo[2]];
    for p in &mut pos {
        for k in 0..3 {
            p[k] -= shift[k];
        }
    }
    let size = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
    let longest = size.iter().copied().fold(0.0f64, f64::max);
    let lods = levels(pos, tris, MeshBudget::for_size(longest, Quality::Detailed));
    Some(Piece {
        lods,
        size_mm: size.map(|v| v as f32),
        parts: parts.len(),
    })
}

/// Sorts the parts into the hook piece and the hoist piece.
pub fn crane_model(parts: &[PlacedPart], placement: Placement) -> CraneModel {
    let mut hook = Vec::new();
    let mut hoist = Vec::new();
    for p in parts {
        let role = role_of(&p.name);
        if role.follows_hook() {
            hook.push(p);
        } else if in_hoist(role) {
            hoist.push(p);
        }
    }
    let used = hook.len() + hoist.len();
    CraneModel {
        hook: piece_of(&hook, placement),
        hoist: piece_of(&hoist, placement),
        left_out: parts.len() - used,
        total: parts.len(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::part3d::tess::Soup;

    fn block(name: &str, lo: V3, hi: V3) -> PlacedPart {
        let pos = vec![
            [lo[0], lo[1], lo[2]],
            [hi[0], lo[1], lo[2]],
            [hi[0], hi[1], lo[2]],
            [lo[0], hi[1], hi[2]],
        ];
        PlacedPart {
            name: name.into(),
            soup: Soup {
                pos,
                tris: vec![[0, 1, 2], [0, 2, 3]],
                features: Vec::new(),
            },
        }
    }

    #[test]
    fn parts_go_to_the_hook_or_the_hoist_by_role_and_the_rest_is_counted() {
        let parts = vec![
            block("Hook block", [0.0; 3], [100.0, 80.0, 300.0]),
            block("Hook", [0.0, 0.0, -200.0], [60.0, 60.0, 0.0]),
            block("Drum", [1000.0; 3], [1400.0, 1400.0, 1300.0]),
            block("Bridge girder", [0.0; 3], [9000.0, 500.0, 900.0]),
        ];
        let m = crane_model(&parts, Placement::default());
        assert_eq!(m.total, 4);
        assert_eq!(m.left_out, 1);
        let hook = m.hook.expect("a hook piece");
        assert_eq!(hook.parts, 2);
        assert!((hook.size_mm[2] - 500.0).abs() < 0.01, "{:?}", hook.size_mm);
        assert_eq!(m.hoist.expect("a hoist piece").parts, 1);
    }

    #[test]
    fn a_piece_is_centred_in_x_and_y_and_stands_on_z_zero() {
        let parts = vec![block("Drum", [500.0, -300.0, 40.0], [700.0, -100.0, 240.0])];
        let m = crane_model(&parts, Placement::default());
        let b = m.hoist.unwrap().lods[0].bounds;
        assert!((b.min[0] + b.max[0]).abs() < 0.01);
        assert!((b.min[1] + b.max[1]).abs() < 0.01);
        assert!(b.min[2].abs() < 0.01);
    }

    #[test]
    fn y_up_files_stand_up_and_a_quarter_turn_swaps_the_plan_extents() {
        // 400 along file-y is the height in a Y-up file; 100 along x and 200 along z are the plan.
        let parts = vec![block("Drum", [0.0; 3], [100.0, 400.0, 200.0])];
        let y_up = Placement {
            up: UpAxis::Y,
            quarter_turn: false,
        };
        let s = crane_model(&parts, y_up).hoist.unwrap().size_mm;
        assert!((s[2] - 400.0).abs() < 0.01, "{s:?}");
        let turned = Placement {
            up: UpAxis::Y,
            quarter_turn: true,
        };
        let t = crane_model(&parts, turned).hoist.unwrap().size_mm;
        assert!(
            (s[0] - t[1]).abs() < 0.01 && (s[1] - t[0]).abs() < 0.01,
            "{s:?} {t:?}"
        );
    }

    #[test]
    fn a_model_without_known_parts_is_not_usable() {
        let m = crane_model(&[block("Plate", [0.0; 3], [1.0; 3])], Placement::default());
        assert!(!m.is_usable());
        assert_eq!(m.left_out, 1);
    }
}
