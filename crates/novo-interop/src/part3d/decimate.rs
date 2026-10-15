//! Welding and quadric edge-collapse simplification to a triangle budget.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use super::math::*;

/// Merges vertices closer than `eps` and drops triangles that collapse or
/// repeat. Returns compact positions and triangles.
pub fn weld(pos: &[V3], tris: &[[u32; 3]], eps: f64) -> (Vec<V3>, Vec<[u32; 3]>) {
    let cell = |p: V3| -> [i64; 3] {
        [
            (p[0] / eps).floor() as i64,
            (p[1] / eps).floor() as i64,
            (p[2] / eps).floor() as i64,
        ]
    };
    let mut grid: HashMap<[i64; 3], Vec<u32>> = HashMap::new();
    let mut out: Vec<V3> = Vec::new();
    let mut map: Vec<u32> = Vec::with_capacity(pos.len());
    for &p in pos {
        let c = cell(p);
        let mut found = None;
        'search: for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = grid.get(&[c[0] + dx, c[1] + dy, c[2] + dz]) {
                        for &i in list {
                            if dist(out[i as usize], p) <= eps {
                                found = Some(i);
                                break 'search;
                            }
                        }
                    }
                }
            }
        }
        let id = found.unwrap_or_else(|| {
            out.push(p);
            let id = (out.len() - 1) as u32;
            grid.entry(c).or_default().push(id);
            id
        });
        map.push(id);
    }
    let mut seen: std::collections::HashSet<[u32; 3]> = std::collections::HashSet::new();
    let mut t_out = Vec::with_capacity(tris.len());
    for t in tris {
        let t = [map[t[0] as usize], map[t[1] as usize], map[t[2] as usize]];
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        // Slivers with three distinct but collinear corners (a triangle
        // that spans a seam) have no area and only make the mesh
        // non-manifold.
        let (a, b, c) = (out[t[0] as usize], out[t[1] as usize], out[t[2] as usize]);
        if len(cross(sub(b, a), sub(c, a))) < 1e-8 {
            continue;
        }
        // Same triangle twice (coincident faces): keep one. Rotate so the
        // smallest index comes first, keeping the winding.
        let k = (0..3).min_by_key(|&i| t[i]).unwrap_or(0);
        let key = [t[k], t[(k + 1) % 3], t[(k + 2) % 3]];
        if seen.insert(key) {
            t_out.push(t);
        }
    }
    (out, t_out)
}

/// Vertex clustering to at most `target` triangles: the robust last resort
/// for meshes that edge collapse cannot reduce (cracks, non-manifold
/// edges from odd surfaces). Coarser and blunter than [`simplify`].
pub fn cluster_to(pos: &mut Vec<V3>, tris: &mut Vec<[u32; 3]>, target: usize) {
    if tris.len() <= target || pos.is_empty() {
        return;
    }
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in pos.iter() {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let diag = len(sub(hi, lo)).max(1e-9);
    let run = |cell: f64| -> (Vec<V3>, Vec<[u32; 3]>) {
        let mut ids: HashMap<[i64; 3], u32> = HashMap::new();
        let mut sums: Vec<(V3, u32)> = Vec::new();
        let map: Vec<u32> = pos
            .iter()
            .map(|p| {
                let k = [
                    ((p[0] - lo[0]) / cell).floor() as i64,
                    ((p[1] - lo[1]) / cell).floor() as i64,
                    ((p[2] - lo[2]) / cell).floor() as i64,
                ];
                let id = *ids.entry(k).or_insert_with(|| {
                    sums.push(([0.0; 3], 0));
                    (sums.len() - 1) as u32
                });
                let s = &mut sums[id as usize];
                s.0 = add(s.0, *p);
                s.1 += 1;
                id
            })
            .collect();
        let np: Vec<V3> = sums
            .iter()
            .map(|s| scale(s.0, 1.0 / f64::from(s.1)))
            .collect();
        let (np, nt) = weld_indexed(&np, tris, &map);
        (np, nt)
    };
    // Bisect the cell size: more triangles for small cells.
    let (mut a, mut b) = (diag * 1e-4, diag);
    let mut best = run(b);
    for _ in 0..24 {
        let m = (a * b).sqrt();
        let r = run(m);
        if r.1.len() <= target {
            best = r;
            b = m;
        } else {
            a = m;
        }
        if b / a < 1.02 {
            break;
        }
    }
    *pos = best.0;
    *tris = best.1;
}

/// Remaps triangles through `map` (old vertex to cluster) and compacts.
fn weld_indexed(np: &[V3], tris: &[[u32; 3]], map: &[u32]) -> (Vec<V3>, Vec<[u32; 3]>) {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for t in tris {
        let t = [map[t[0] as usize], map[t[1] as usize], map[t[2] as usize]];
        if t[0] == t[1] || t[1] == t[2] || t[0] == t[2] {
            continue;
        }
        let k = (0..3).min_by_key(|&i| t[i]).unwrap_or(0);
        if seen.insert([t[k], t[(k + 1) % 3], t[(k + 2) % 3]]) {
            out.push(t);
        }
    }
    let mut remap = vec![u32::MAX; np.len()];
    let mut pos = Vec::new();
    for t in &mut out {
        for v in t.iter_mut() {
            if remap[*v as usize] == u32::MAX {
                remap[*v as usize] = pos.len() as u32;
                pos.push(np[*v as usize]);
            }
            *v = remap[*v as usize];
        }
    }
    (pos, out)
}

/// Edges used by one triangle only (a watertight mesh has none) and edges
/// used by more than two (non-manifold).
pub fn edge_health(tris: &[[u32; 3]]) -> (usize, usize) {
    let mut c: HashMap<(u32, u32), u32> = HashMap::new();
    for t in tris {
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            *c.entry((a.min(b), a.max(b))).or_insert(0) += 1;
        }
    }
    (
        c.values().filter(|&&n| n == 1).count(),
        c.values().filter(|&&n| n > 2).count(),
    )
}

/// Directed edges used by more than one triangle: where neighbouring
/// triangles are wound against each other (a face turned inside out).
pub fn winding_conflicts(tris: &[[u32; 3]]) -> usize {
    let mut c: HashMap<(u32, u32), u32> = HashMap::new();
    for t in tris {
        for k in 0..3 {
            *c.entry((t[k], t[(k + 1) % 3])).or_insert(0) += 1;
        }
    }
    c.values().filter(|&&n| n > 1).count()
}

type Quadric = [f64; 10];

fn plane_quadric(n: V3, d: f64, w: f64) -> Quadric {
    [
        w * n[0] * n[0],
        w * n[0] * n[1],
        w * n[0] * n[2],
        w * n[0] * d,
        w * n[1] * n[1],
        w * n[1] * n[2],
        w * n[1] * d,
        w * n[2] * n[2],
        w * n[2] * d,
        w * d * d,
    ]
}

fn q_add(a: &Quadric, b: &Quadric) -> Quadric {
    let mut o = [0.0; 10];
    for i in 0..10 {
        o[i] = a[i] + b[i];
    }
    o
}

fn q_eval(q: &Quadric, p: V3) -> f64 {
    let [x, y, z] = p;
    q[0] * x * x
        + 2.0 * q[1] * x * y
        + 2.0 * q[2] * x * z
        + 2.0 * q[3] * x
        + q[4] * y * y
        + 2.0 * q[5] * y * z
        + 2.0 * q[6] * y
        + q[7] * z * z
        + 2.0 * q[8] * z
        + q[9]
}

struct Cand {
    cost: f64,
    a: u32,
    b: u32,
    va: u32,
    vb: u32,
    target: V3,
}

impl PartialEq for Cand {
    fn eq(&self, o: &Self) -> bool {
        self.cost == o.cost
    }
}
impl Eq for Cand {}
impl PartialOrd for Cand {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Cand {
    // Smallest cost first out of a max-heap.
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost.total_cmp(&self.cost)
    }
}

fn tri_normal(p: &[V3], t: [u32; 3]) -> V3 {
    cross(
        sub(p[t[1] as usize], p[t[0] as usize]),
        sub(p[t[2] as usize], p[t[0] as usize]),
    )
}

/// Collapses edges, cheapest first, until at most `target` triangles are
/// left (or nothing safe remains). Open boundaries are held in place.
pub fn simplify(pos: &mut Vec<V3>, tris: &mut Vec<[u32; 3]>, target: usize) {
    if tris.len() <= target {
        return;
    }
    let nv = pos.len();
    let mut alive = vec![true; tris.len()];
    let mut vt: Vec<Vec<u32>> = vec![Vec::new(); nv];
    for (i, t) in tris.iter().enumerate() {
        for &v in t {
            vt[v as usize].push(i as u32);
        }
    }
    // Quadrics: area-weighted planes of the incident triangles, plus stiff
    // planes along open boundary edges.
    let mut q: Vec<Quadric> = vec![[0.0; 10]; nv];
    let mut edge_count: HashMap<(u32, u32), (u32, u32)> = HashMap::new();
    for (i, t) in tris.iter().enumerate() {
        let n = tri_normal(pos, *t);
        let area2 = len(n);
        if area2 < 1e-18 {
            continue;
        }
        let u = scale(n, 1.0 / area2);
        let d = -dot(u, pos[t[0] as usize]);
        let pq = plane_quadric(u, d, area2 * 0.5);
        for &v in t {
            q[v as usize] = q_add(&q[v as usize], &pq);
        }
        for k in 0..3 {
            let (a, b) = (t[k], t[(k + 1) % 3]);
            let e = edge_count
                .entry((a.min(b), a.max(b)))
                .or_insert((0, i as u32));
            e.0 += 1;
        }
    }
    for (&(a, b), &(c, ti)) in &edge_count {
        if c == 1 {
            let t = tris[ti as usize];
            let n = unit(tri_normal(pos, t));
            let e = sub(pos[b as usize], pos[a as usize]);
            let bn = unit_or(cross(e, n), [0.0; 3]);
            let w = 1000.0 * dot(e, e);
            let bq = plane_quadric(bn, -dot(bn, pos[a as usize]), w);
            q[a as usize] = q_add(&q[a as usize], &bq);
            q[b as usize] = q_add(&q[b as usize], &bq);
        }
    }
    let mut ver = vec![0u32; nv];
    let mut vdead = vec![false; nv];
    let mut heap: BinaryHeap<Cand> = BinaryHeap::new();
    let cand = |a: u32, b: u32, pos: &[V3], q: &[Quadric], ver: &[u32]| -> Cand {
        let qs = q_add(&q[a as usize], &q[b as usize]);
        let (pa, pb) = (pos[a as usize], pos[b as usize]);
        let m = lerp(pa, pb, 0.5);
        let mut best = (q_eval(&qs, m), m);
        for p in [pa, pb] {
            let c = q_eval(&qs, p);
            if c < best.0 {
                best = (c, p);
            }
        }
        Cand {
            cost: best.0.max(0.0),
            a,
            b,
            va: ver[a as usize],
            vb: ver[b as usize],
            target: best.1,
        }
    };
    for &(a, b) in edge_count.keys() {
        heap.push(cand(a, b, pos, &q, &ver));
    }
    let mut count = tris.len();
    while count > target {
        let Some(c) = heap.pop() else { break };
        let (a, b) = (c.a as usize, c.b as usize);
        if vdead[a] || vdead[b] || ver[a] != c.va || ver[b] != c.vb {
            continue;
        }
        // Neighbour sets for the link condition.
        let mut shared = 0usize;
        let mut na: Vec<u32> = Vec::new();
        for &t in &vt[a] {
            if !alive[t as usize] {
                continue;
            }
            let tr = tris[t as usize];
            if tr.contains(&(b as u32)) {
                shared += 1;
            }
            na.extend(tr.iter().copied().filter(|&v| v != a as u32));
        }
        na.sort_unstable();
        na.dedup();
        let mut common = 0usize;
        let mut nb: Vec<u32> = Vec::new();
        for &t in &vt[b] {
            if alive[t as usize] {
                nb.extend(tris[t as usize].iter().copied().filter(|&v| v != b as u32));
            }
        }
        nb.sort_unstable();
        nb.dedup();
        for v in &nb {
            if *v != a as u32 && na.binary_search(v).is_ok() {
                common += 1;
            }
        }
        if shared == 0 || common != shared {
            continue;
        }
        // Reject collapses that fold a neighbouring triangle over.
        let old_a = pos[a];
        let mut ok = true;
        for &v in &[a, b] {
            for &t in &vt[v] {
                if !alive[t as usize] {
                    continue;
                }
                let tr = tris[t as usize];
                if tr.contains(&(a as u32)) && tr.contains(&(b as u32)) {
                    continue;
                }
                let before = tri_normal(pos, tr);
                let mut moved = tr;
                for x in &mut moved {
                    if *x == a as u32 || *x == b as u32 {
                        *x = a as u32;
                    }
                }
                pos[a] = c.target;
                let after = tri_normal(pos, moved);
                pos[a] = old_a;
                let (lb, la) = (len(before), len(after));
                if la < 1e-12 * (1.0 + lb) || (lb > 1e-18 && dot(before, after) < 0.2 * lb * la) {
                    ok = false;
                    break;
                }
            }
            if !ok {
                break;
            }
        }
        if !ok {
            continue;
        }
        // Commit: b merges into a at the target position.
        pos[a] = c.target;
        q[a] = q_add(&q[a], &q[b]);
        vdead[b] = true;
        ver[a] += 1;
        let moved: Vec<u32> = std::mem::take(&mut vt[b]);
        for t in moved {
            if !alive[t as usize] {
                continue;
            }
            let tr = &mut tris[t as usize];
            if tr.contains(&(a as u32)) {
                alive[t as usize] = false;
                count -= 1;
            } else {
                for x in tr.iter_mut() {
                    if *x == b as u32 {
                        *x = a as u32;
                    }
                }
                vt[a].push(t);
            }
        }
        vt[a].retain(|&t| alive[t as usize]);
        let mut around: Vec<u32> = Vec::new();
        for &t in &vt[a] {
            around.extend(tris[t as usize].iter().copied().filter(|&v| v != a as u32));
        }
        around.sort_unstable();
        around.dedup();
        for v in around {
            heap.push(cand(a as u32, v, pos, &q, &ver));
        }
    }
    // Compact.
    let mut remap = vec![u32::MAX; nv];
    let mut np: Vec<V3> = Vec::new();
    let mut nt: Vec<[u32; 3]> = Vec::new();
    for (i, t) in tris.iter().enumerate() {
        if !alive[i] {
            continue;
        }
        let mut r = [0u32; 3];
        for k in 0..3 {
            let v = t[k] as usize;
            if remap[v] == u32::MAX {
                remap[v] = np.len() as u32;
                np.push(pos[v]);
            }
            r[k] = remap[v];
        }
        nt.push(r);
    }
    *pos = np;
    *tris = nt;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clustering_always_reaches_the_budget() {
        let (mut p, mut t) = grid(30, true);
        cluster_to(&mut p, &mut t, 100);
        assert!(!t.is_empty() && t.len() <= 100, "{}", t.len());
    }

    /// A flat n x n grid of quads, two triangles each, with a bump so the
    /// middle matters.
    fn grid(n: usize, bump: bool) -> (Vec<V3>, Vec<[u32; 3]>) {
        let mut pos = Vec::new();
        for y in 0..=n {
            for x in 0..=n {
                let z = if bump && x == n / 2 && y == n / 2 {
                    3.0
                } else {
                    0.0
                };
                pos.push([x as f64, y as f64, z]);
            }
        }
        let mut tris = Vec::new();
        let w = (n + 1) as u32;
        for y in 0..n as u32 {
            for x in 0..n as u32 {
                let i = y * w + x;
                tris.push([i, i + 1, i + w + 1]);
                tris.push([i, i + w + 1, i + w]);
            }
        }
        (pos, tris)
    }

    #[test]
    fn a_flat_grid_collapses_to_its_corners() {
        let (mut p, mut t) = grid(10, false);
        assert_eq!(t.len(), 200);
        simplify(&mut p, &mut t, 2);
        assert!(t.len() <= 10, "left {}", t.len());
        // Boundary corners survive: the outline still spans 10 x 10.
        let max_x = p.iter().map(|v| v[0]).fold(f64::MIN, f64::max);
        let max_y = p.iter().map(|v| v[1]).fold(f64::MIN, f64::max);
        assert!((max_x - 10.0).abs() < 1e-9 && (max_y - 10.0).abs() < 1e-9);
    }

    #[test]
    fn a_bump_survives_while_the_flat_rest_goes() {
        let (mut p, mut t) = grid(10, true);
        simplify(&mut p, &mut t, 40);
        assert!(t.len() <= 40);
        let top = p.iter().map(|v| v[2]).fold(f64::MIN, f64::max);
        assert!(top > 2.9, "bump height kept: {top}");
    }

    #[test]
    fn weld_merges_close_points_and_drops_duplicates() {
        let pos = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [1e-6, 0.0, 0.0],
            [1.0, 1e-6, 0.0],
            [0.0, 1.0, 1e-6],
        ];
        let tris = vec![[0, 1, 2], [3, 4, 5], [0, 0, 1]];
        let (p, t) = weld(&pos, &tris, 1e-3);
        assert_eq!((p.len(), t.len()), (3, 1));
    }

    #[test]
    fn a_closed_box_stays_closed_and_never_inverts() {
        // 12-triangle cube subdivided by welding duplicates is trivially
        // above budget 12: nothing to do.
        let pos = vec![
            [0., 0., 0.],
            [1., 0., 0.],
            [1., 1., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            [1., 0., 1.],
            [1., 1., 1.],
            [0., 1., 1.],
        ];
        let tris = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [1, 2, 6],
            [1, 6, 5],
            [2, 3, 7],
            [2, 7, 6],
            [3, 0, 4],
            [3, 4, 7],
        ];
        let (mut p, mut t) = (pos.clone(), tris.clone());
        simplify(&mut p, &mut t, 12);
        assert_eq!((p.len(), t.len()), (8, 12));
        // Asking for fewer on a cube must not produce inside-out triangles.
        simplify(&mut p, &mut t, 8);
        let c = [0.5, 0.5, 0.5];
        for tr in &t {
            let n = tri_normal(&p, *tr);
            let m = scale(
                add(add(p[tr[0] as usize], p[tr[1] as usize]), p[tr[2] as usize]),
                1.0 / 3.0,
            );
            assert!(dot(n, sub(m, c)) > 0.0);
        }
    }
}
