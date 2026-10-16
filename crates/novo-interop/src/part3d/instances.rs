//! Many parts, few meshes: instancing and the scene triangle budget.
//!
//! A cabinet has hundreds of terminals that share one part and so one mesh.
//! The renderer keeps one vertex buffer per part and one instance buffer of
//! transforms; this module does the grouping and decides how many triangles
//! each part's mesh may have so the whole scene stays within a budget.

use std::collections::HashMap;

/// One placed device, drawn from the mesh of `part`.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedPart {
    /// Identifies the mesh: the content hash of the part's model, or a
    /// size key for box parts (see [`box_key`]).
    pub part: String,
    /// The caller's handle for picking (an index into its device list).
    pub tag: u32,
    /// Column-major 4x4, part space to cabinet space.
    pub transform: [f32; 16],
}

/// All placements of one part, in the order they first appeared.
#[derive(Debug, Clone, PartialEq)]
pub struct InstanceBatch {
    pub part: String,
    pub tags: Vec<u32>,
    pub transforms: Vec<[f32; 16]>,
}

/// Groups placements by part. Batches keep the order of their first use,
/// so the draw order is stable between frames.
pub fn group_instances(placed: &[PlacedPart]) -> Vec<InstanceBatch> {
    let mut order: HashMap<&str, usize> = HashMap::new();
    let mut out: Vec<InstanceBatch> = Vec::new();
    for p in placed {
        let i = *order.entry(p.part.as_str()).or_insert_with(|| {
            out.push(InstanceBatch {
                part: p.part.clone(),
                tags: Vec::new(),
                transforms: Vec::new(),
            });
            out.len() - 1
        });
        out[i].tags.push(p.tag);
        out[i].transforms.push(p.transform);
    }
    out
}

/// Key for the box of a part without a model: every part of the same size
/// shares one mesh, so 200 relays without models are one batch.
pub fn box_key(size_mm: [f64; 3]) -> String {
    format!("box-{:.1}x{:.1}x{:.1}", size_mm[0], size_mm[1], size_mm[2])
}

/// A part in the scene, for choosing levels of detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartLods {
    pub instances: u32,
    /// Triangles of each level, most detailed first
    /// (`PartGeometry::lod_triangles`).
    pub triangles: Vec<u32>,
}

/// The level of detail of every part so that
/// `sum(instances * triangles)` stays within `scene_triangles` where the
/// levels allow: all parts start at their most detailed level, then the
/// part that costs the most in the scene drops one level at a time. So 400
/// copies of a detailed terminal get coarser before a rare large part does.
/// A part never drops below its last level (the box).
pub fn pick_lods(parts: &[PartLods], scene_triangles: u64) -> Vec<usize> {
    use std::collections::BinaryHeap;
    let cost = |p: &PartLods, l: usize| u64::from(p.instances.max(1)) * u64::from(p.triangles[l]);
    let mut level = vec![0usize; parts.len()];
    let mut total: u64 = parts
        .iter()
        .filter(|p| !p.triangles.is_empty())
        .map(|p| cost(p, 0))
        .sum();
    let mut heap: BinaryHeap<(u64, usize)> = parts
        .iter()
        .enumerate()
        .filter(|(_, p)| p.triangles.len() > 1)
        .map(|(i, p)| (cost(p, 0), i))
        .collect();
    while total > scene_triangles {
        let Some((c, i)) = heap.pop() else { break };
        let p = &parts[i];
        let next = level[i] + 1;
        level[i] = next;
        total = total - c + cost(p, next);
        if next + 1 < p.triangles.len() {
            heap.push((cost(p, next), i));
        }
    }
    level
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placed(part: &str, tag: u32) -> PlacedPart {
        PlacedPart {
            part: part.into(),
            tag,
            transform: [0.0; 16],
        }
    }

    #[test]
    fn identical_parts_share_a_batch_in_first_use_order() {
        let p = [
            placed("b", 0),
            placed("a", 1),
            placed("b", 2),
            placed("b", 3),
        ];
        let g = group_instances(&p);
        assert_eq!(g.len(), 2);
        assert_eq!(
            (g[0].part.as_str(), g[0].tags.clone()),
            ("b", vec![0, 2, 3])
        );
        assert_eq!((g[1].part.as_str(), g[1].tags.clone()), ("a", vec![1]));
    }

    #[test]
    fn box_parts_of_one_size_are_one_mesh() {
        assert_eq!(box_key([17.5, 90.0, 60.0]), box_key([17.5, 90.0, 60.0]));
        assert_ne!(box_key([17.5, 90.0, 60.0]), box_key([22.5, 90.0, 60.0]));
    }

    #[test]
    fn the_costliest_parts_drop_levels_first() {
        let parts = vec![
            // 400 terminals: 400 x 1200 = 480 000 at full detail.
            PartLods {
                instances: 400,
                triangles: vec![1_200, 300, 12],
            },
            // 2 PLCs: 2 x 2000.
            PartLods {
                instances: 2,
                triangles: vec![2_000, 300, 12],
            },
        ];
        assert_eq!(pick_lods(&parts, 10_000_000), vec![0, 0]);
        // Budget below the terminals' full cost: they coarsen, the PLCs stay.
        assert_eq!(pick_lods(&parts, 200_000), vec![1, 0]);
        // Tighter: terminals go to the box, then the PLCs follow.
        assert_eq!(pick_lods(&parts, 6_000), vec![2, 1]);
        // Nothing fits: every part ends at its last level, never beyond.
        assert_eq!(pick_lods(&parts, 0), vec![2, 2]);
        // A part with one level has nowhere to go.
        let one = vec![PartLods {
            instances: 9,
            triangles: vec![12],
        }];
        assert_eq!(pick_lods(&one, 0), vec![0]);
    }
}
