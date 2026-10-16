//! Numbers about a grid for the UI, before or after classification.

use super::{BodyResolution, Mesh};
use crate::result::Grid3;

/// Cell counts and sizes of a mesh; shown in the Simulate dialog.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshPreview {
    pub cells: usize,
    pub n: [usize; 3],
    /// Smallest and largest cell edge over all axes, mm.
    pub min_size_mm: f64,
    pub max_size_mm: f64,
    /// Largest size ratio of two neighbouring cells along an axis.
    pub max_grading_ratio: f64,
    /// Largest ratio of the longest to the shortest edge of one cell.
    pub max_aspect_ratio: f64,
    /// Rough memory of the solver fields, MB.
    pub memory_mb: f64,
    /// `(fluid, solid, cut)` cells; `None` before classification.
    pub class_counts: Option<(usize, usize, usize)>,
    /// Solid bodies the grid does not resolve (see
    /// [`BodyResolution::is_resolved`]); `None` before classification.
    pub unresolved_bodies: Option<Vec<String>>,
    /// The bodies alone need more cells than the target.
    pub over_budget: bool,
}

/// Bytes per cell the flow core is expected to hold (fields, matrices).
const BYTES_PER_CELL: f64 = 400.0;

impl MeshPreview {
    pub fn from_grid(grid: &Grid3) -> MeshPreview {
        let axes = [&grid.x_edges_mm, &grid.y_edges_mm, &grid.z_edges_mm];
        let widths: Vec<Vec<f64>> = axes
            .iter()
            .map(|e| e.windows(2).map(|w| w[1] - w[0]).collect())
            .collect();
        let all = widths.iter().flatten().copied();
        let min_size_mm = all.clone().fold(f64::INFINITY, f64::min);
        let max_size_mm = all.fold(0.0, f64::max);
        let max_grading_ratio = widths
            .iter()
            .flat_map(|w| w.windows(2).map(|p| (p[0] / p[1]).max(p[1] / p[0])))
            .fold(1.0, f64::max);
        // Aspect: per cell, longest over shortest edge; the extremes come
        // from the widest and narrowest along each axis at the same index
        // combination, bounded here by the global extremes per axis pair.
        let (mut amax, mut amin) = ([0.0f64; 3], [f64::INFINITY; 3]);
        for (a, w) in widths.iter().enumerate() {
            for &v in w {
                amax[a] = amax[a].max(v);
                amin[a] = amin[a].min(v);
            }
        }
        let longest = amax.iter().cloned().fold(0.0, f64::max);
        let shortest = amin.iter().cloned().fold(f64::INFINITY, f64::min);
        let cells = grid.cells();
        MeshPreview {
            cells,
            n: [grid.nx, grid.ny, grid.nz],
            min_size_mm,
            max_size_mm,
            max_grading_ratio,
            max_aspect_ratio: longest / shortest,
            memory_mb: cells as f64 * BYTES_PER_CELL / 1.0e6,
            class_counts: None,
            unresolved_bodies: None,
            over_budget: false,
        }
    }

    /// One line for the Simulate dialog.
    pub fn summary(&self) -> String {
        format!(
            "{} cells ({} x {} x {}), {:.1} to {:.1} mm, about {:.0} MB",
            group(self.cells),
            self.n[0],
            self.n[1],
            self.n[2],
            self.min_size_mm,
            self.max_size_mm,
            self.memory_mb
        )
    }
}

impl Mesh {
    pub fn preview(&self) -> MeshPreview {
        let mut p = MeshPreview::from_grid(&self.grid);
        p.class_counts = Some(self.class_counts());
        p.unresolved_bodies = Some(
            self.bodies
                .iter()
                .filter(|b| !BodyResolution::is_resolved(b))
                .map(|b| b.name.clone())
                .collect(),
        );
        p.over_budget = self.over_budget;
        p
    }
}

fn group(n: usize) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(' ');
        }
        out.push(c);
    }
    out
}
