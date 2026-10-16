//! Radiating surfaces: exposed cell faces grouped into patches.

use std::ops::Range;

use crate::mesh::Mesh;
use crate::result::{Grid3, MaterialInfo, MaterialKind};

/// Planning emissivities (total hemispherical, grey). They are typical
/// values, not measurements: painted steel and plastics are close to 0.9,
/// bare galvanised steel is much lower. Override per material id where the
/// project knows better.
#[derive(Debug, Clone, PartialEq)]
pub struct Emissivity {
    pub plate: f64,
    pub device: f64,
    pub partition: f64,
    pub rail: f64,
    pub duct: f64,
    /// Inner side of the enclosure walls.
    pub enclosure_wall: f64,
    /// `(material id, emissivity)` taking precedence over the kinds above.
    pub overrides: Vec<(u16, f64)>,
}

impl Default for Emissivity {
    fn default() -> Self {
        Emissivity {
            plate: 0.30,
            device: 0.90,
            partition: 0.30,
            rail: 0.30,
            duct: 0.90,
            enclosure_wall: 0.90,
            overrides: Vec::new(),
        }
    }
}

impl Emissivity {
    /// Emissivity of a solid material, kept inside `[0.02, 1]`.
    pub fn of(&self, m: &MaterialInfo) -> f64 {
        if let Some(&(_, e)) = self.overrides.iter().find(|o| o.0 == m.id) {
            return clamp(e);
        }
        clamp(match m.kind {
            MaterialKind::Air => 1.0,
            MaterialKind::Device(_) => self.device,
            MaterialKind::Plate => self.plate,
            MaterialKind::Wall => self.partition,
            MaterialKind::Rail => self.rail,
            MaterialKind::Duct => self.duct,
        })
    }
}

fn clamp(e: f64) -> f64 {
    e.clamp(0.02, 1.0)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PatchSettings {
    /// Upper bound on the number of patches (the view factor matrix is
    /// dense: n x n).
    pub max_patches: usize,
    /// Fixed block size in cells per tangential axis instead of the
    /// automatic one (tests and fine control).
    pub block: Option<usize>,
}

impl Default for PatchSettings {
    fn default() -> Self {
        PatchSettings {
            max_patches: 1500,
            block: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PatchKind {
    /// Face of a solid body, looking into the air.
    Solid,
    /// Inner face of the enclosure wall.
    Wall,
}

/// A cell face. `dir` is `2 * axis + (1 if the face is on the high side)`.
/// For a solid patch the cell is the solid one and the face looks into the
/// air; for a wall patch the cell is the air cell at the domain boundary
/// and the face is the boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceRef {
    pub cell: u32,
    pub dir: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub kind: PatchKind,
    /// Mesh material id of the solid (0 for the enclosure wall).
    pub material: u16,
    /// Unit normal pointing into the air.
    pub normal: [f64; 3],
    pub area_m2: f64,
    pub centroid_mm: [f64; 3],
    pub emissivity: f64,
    /// Range into [`PatchSet::faces`].
    pub faces: Range<usize>,
}

#[derive(Debug, Clone)]
pub struct PatchSet {
    pub patches: Vec<Patch>,
    pub faces: Vec<FaceRef>,
    /// Cumulative face area within the patch, mm2 (same order as `faces`).
    pub(crate) cum_area_mm2: Vec<f64>,
    lookup: Vec<u32>,
    /// Block size that was used, cells.
    pub block: usize,
}

const NONE: u32 = u32::MAX;

/// Faces share a patch when kind, material, direction, plane and block agree.
type GroupKey = (u8, u16, u8, u32, u32, u32);

pub(crate) fn axis_of(dir: u8) -> usize {
    usize::from(dir / 2)
}

pub(crate) fn sign_of(dir: u8) -> isize {
    if dir % 2 == 1 { 1 } else { -1 }
}

pub(crate) fn edges(g: &Grid3, a: usize) -> &[f64] {
    match a {
        0 => &g.x_edges_mm,
        1 => &g.y_edges_mm,
        _ => &g.z_edges_mm,
    }
}

fn dims(g: &Grid3) -> [usize; 3] {
    [g.nx, g.ny, g.nz]
}

fn split(g: &Grid3, cell: usize) -> [usize; 3] {
    [cell % g.nx, (cell / g.nx) % g.ny, cell / (g.nx * g.ny)]
}

/// Area of a face, mm2.
pub(crate) fn face_area_mm2(g: &Grid3, f: FaceRef) -> f64 {
    let a = axis_of(f.dir);
    let idx = split(g, f.cell as usize);
    let w = |ax: usize| edges(g, ax)[idx[ax] + 1] - edges(g, ax)[idx[ax]];
    w((a + 1) % 3) * w((a + 2) % 3)
}

/// Lower and upper corner of a face (equal on the normal axis), mm.
pub(crate) fn face_box_mm(g: &Grid3, f: FaceRef) -> ([f64; 3], [f64; 3]) {
    let a = axis_of(f.dir);
    let idx = split(g, f.cell as usize);
    let mut lo = [0.0; 3];
    let mut hi = [0.0; 3];
    for ax in 0..3 {
        let e = edges(g, ax);
        lo[ax] = e[idx[ax]];
        hi[ax] = e[idx[ax] + 1];
    }
    let plane = if f.dir % 2 == 1 { hi[a] } else { lo[a] };
    lo[a] = plane;
    hi[a] = plane;
    (lo, hi)
}

impl PatchSet {
    pub fn from_mesh(mesh: &Mesh, emis: &Emissivity, st: &PatchSettings) -> PatchSet {
        let g = &mesh.grid;
        let n = dims(g);
        let opaque: Vec<bool> = mesh.material.iter().map(|&m| m != 0).collect();
        // All exposed faces in cell order.
        let mut raw: Vec<(FaceRef, PatchKind)> = Vec::new();
        for cell in 0..g.cells() {
            let idx = split(g, cell);
            for dir in 0..6u8 {
                let a = axis_of(dir);
                let nb = idx[a] as isize + sign_of(dir);
                let inside = nb >= 0 && (nb as usize) < n[a];
                let face = FaceRef {
                    cell: cell as u32,
                    dir,
                };
                if opaque[cell] {
                    if inside {
                        let mut j = idx;
                        j[a] = nb as usize;
                        if !opaque[(j[2] * g.ny + j[1]) * g.nx + j[0]] {
                            raw.push((face, PatchKind::Solid));
                        }
                    }
                } else if !inside {
                    raw.push((face, PatchKind::Wall));
                }
            }
        }

        let group = |bs: usize| -> Vec<(GroupKey, usize)> {
            let mut keyed: Vec<_> = raw
                .iter()
                .enumerate()
                .map(|(r, (f, kind))| {
                    let idx = split(g, f.cell as usize);
                    let a = axis_of(f.dir);
                    let mat = if *kind == PatchKind::Solid {
                        mesh.material[f.cell as usize]
                    } else {
                        0
                    };
                    let key = (
                        u8::from(*kind == PatchKind::Wall),
                        mat,
                        f.dir,
                        idx[a] as u32,
                        (idx[(a + 1) % 3] / bs) as u32,
                        (idx[(a + 2) % 3] / bs) as u32,
                    );
                    (key, r)
                })
                .collect();
            keyed.sort_unstable();
            keyed
        };
        let count = |k: &[(GroupKey, usize)]| -> usize {
            let mut c = 0;
            let mut prev = None;
            for (key, _) in k {
                if prev != Some(key) {
                    c += 1;
                    prev = Some(key);
                }
            }
            c
        };
        let max_dim = n.iter().copied().max().unwrap_or(1).max(1);
        let (bs, keyed) = match st.block {
            Some(b) => {
                let b = b.max(1);
                (b, group(b))
            }
            None => {
                let mut bs = 1usize;
                loop {
                    let k = group(bs);
                    if count(&k) <= st.max_patches.max(2) || bs >= max_dim {
                        break (bs, k);
                    }
                    bs = if bs < 4 { bs + 1 } else { bs + bs / 2 };
                }
            }
        };

        let mut patches = Vec::new();
        let mut faces = Vec::with_capacity(raw.len());
        let mut cum = Vec::with_capacity(raw.len());
        let mut lookup = vec![NONE; g.cells() * 6];
        let mut s = 0;
        while s < keyed.len() {
            let key = keyed[s].0;
            let mut e = s;
            while e < keyed.len() && keyed[e].0 == key {
                e += 1;
            }
            let id = patches.len() as u32;
            let first = faces.len();
            let kind = raw[keyed[s].1].1;
            let mut area = 0.0;
            let mut c = [0.0; 3];
            for &(_, r) in &keyed[s..e] {
                let f = raw[r].0;
                let a_mm2 = face_area_mm2(g, f);
                let (lo, hi) = face_box_mm(g, f);
                for ax in 0..3 {
                    c[ax] += a_mm2 * 0.5 * (lo[ax] + hi[ax]);
                }
                area += a_mm2;
                cum.push(area);
                lookup[f.cell as usize * 6 + usize::from(f.dir)] = id;
                faces.push(f);
            }
            let d = key.2;
            let mut normal = [0.0; 3];
            let towards_air = if kind == PatchKind::Solid {
                sign_of(d)
            } else {
                -sign_of(d)
            };
            normal[axis_of(d)] = towards_air as f64;
            let emissivity = match kind {
                PatchKind::Wall => clamp(emis.enclosure_wall),
                PatchKind::Solid => mesh
                    .materials
                    .iter()
                    .find(|m| m.id == key.1)
                    .map_or(0.9, |m| emis.of(m)),
            };
            patches.push(Patch {
                kind,
                material: key.1,
                normal,
                area_m2: area * 1e-6,
                centroid_mm: c.map(|v| v / area),
                emissivity,
                faces: first..faces.len(),
            });
            s = e;
        }
        PatchSet {
            patches,
            faces,
            cum_area_mm2: cum,
            lookup,
            block: bs,
        }
    }

    pub fn len(&self) -> usize {
        self.patches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.patches.is_empty()
    }

    /// Patch that owns a face (see [`FaceRef`] for the conventions).
    pub fn patch_of(&self, cell: usize, dir: u8) -> Option<usize> {
        match self.lookup.get(cell * 6 + usize::from(dir)) {
            Some(&p) if p != NONE => Some(p as usize),
            _ => None,
        }
    }

    pub fn areas_m2(&self) -> Vec<f64> {
        self.patches.iter().map(|p| p.area_m2).collect()
    }

    /// Area-weighted mean of a face quantity (for example the surface
    /// temperature of the solid cell) per patch.
    pub fn average(&self, grid: &Grid3, f: impl Fn(FaceRef) -> f64) -> Vec<f64> {
        self.patches
            .iter()
            .map(|p| {
                let mut num = 0.0;
                for i in p.faces.clone() {
                    num += f(self.faces[i]) * face_area_mm2(grid, self.faces[i]);
                }
                num / (p.area_m2 * 1e6)
            })
            .collect()
    }

    /// Radiative flux per patch spread back to its faces: `(face, W/m2)`.
    pub fn scatter(&self, flux_w_m2: &[f64]) -> Vec<(FaceRef, f64)> {
        let mut out = Vec::with_capacity(self.faces.len());
        for (p, q) in self.patches.iter().zip(flux_w_m2) {
            for i in p.faces.clone() {
                out.push((self.faces[i], *q));
            }
        }
        out
    }
}
