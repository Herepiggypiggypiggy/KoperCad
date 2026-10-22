//! Small shapes for the overlays: strips along a path, glowing discs, rings,
//! gems and fans. World millimetres; everything is flat-shaded by the
//! renderer, so colour and alpha are per vertex.

use glam::Vec3;

pub fn rgba(c: [u8; 3], a: u8) -> [u8; 4] {
    [c[0], c[1], c[2], a]
}

/// A growing triangle mesh.
#[derive(Default)]
pub struct Mesh {
    pub positions: Vec<Vec3>,
    pub colors: Vec<[u8; 4]>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    pub fn vertex(&mut self, p: Vec3, c: [u8; 4]) -> u32 {
        self.positions.push(p);
        self.colors.push(c);
        (self.positions.len() - 1) as u32
    }

    pub fn tri(&mut self, a: u32, b: u32, c: u32) {
        self.indices.extend([a, b, c]);
    }

    pub fn quad(&mut self, a: u32, b: u32, c: u32, d: u32) {
        self.tri(a, b, c);
        self.tri(a, c, d);
    }
}

fn normal_xy(a: Vec3, b: Vec3) -> Option<Vec3> {
    let d = b - a;
    let l = (d.x * d.x + d.y * d.y).sqrt();
    (l > 1e-4).then(|| Vec3::new(-d.y / l, d.x / l, 0.0))
}

/// A flat strip of half-width `half` along `path`, in the plane of the path's
/// own heights. Joints use the mean normal, so a corner stays closed.
/// `edge_alpha` fades the strip to its edges (a glow) when below `alpha`.
pub fn strip(
    mesh: &mut Mesh,
    path: &[Vec3],
    half: f32,
    colour: [u8; 3],
    alpha: u8,
    edge_alpha: u8,
) {
    let mut pts: Vec<Vec3> = Vec::with_capacity(path.len());
    for &p in path {
        if pts.last().is_none_or(|l| (p - *l).length() > 1e-3) {
            pts.push(p);
        }
    }
    if pts.len() < 2 {
        return;
    }
    let mut prev_pair: Option<(u32, u32)> = None;
    for i in 0..pts.len() {
        let before = (i > 0).then(|| normal_xy(pts[i - 1], pts[i])).flatten();
        let after = (i + 1 < pts.len())
            .then(|| normal_xy(pts[i], pts[i + 1]))
            .flatten();
        let n = match (before, after) {
            (Some(a), Some(b)) => {
                let m = a + b;
                if m.length() < 1e-3 { a } else { m.normalize() }
            }
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => continue,
        };
        // Widen at a corner so the strip keeps its width.
        let scale = match (before, after) {
            (Some(a), Some(b)) => {
                let c = a.dot(b).clamp(-1.0, 1.0);
                (2.0 / (1.0 + c)).sqrt().min(2.0)
            }
            _ => 1.0,
        };
        let w = n * half * scale;
        let l = mesh.vertex(pts[i] + w, rgba(colour, edge_alpha));
        let r = mesh.vertex(pts[i] - w, rgba(colour, edge_alpha));
        let c = mesh.vertex(pts[i], rgba(colour, alpha));
        if let Some((pl, pr)) = prev_pair {
            // Previous centre vertex is the one before its pair's successor.
            let pc = pl + 2;
            mesh.quad(pl, pc, c, l);
            mesh.quad(pc, pr, r, c);
        }
        prev_pair = Some((l, r));
    }
}

/// A disc whose alpha falls from `centre_alpha` in the middle to nothing at
/// the rim, drawn in `rings` steps.
pub fn glow_disc(mesh: &mut Mesh, centre: Vec3, radius: f32, colour: [u8; 3], centre_alpha: u8) {
    const SEGS: u32 = 40;
    const RINGS: u32 = 6;
    let hub = mesh.vertex(centre, rgba(colour, centre_alpha));
    let mut prev: Vec<u32> = Vec::new();
    for ring in 1..=RINGS {
        let t = ring as f32 / RINGS as f32;
        let a = (f32::from(centre_alpha) * (1.0 - t).powi(2)).round() as u8;
        let mut this = Vec::new();
        for s in 0..SEGS {
            let ang = s as f32 / SEGS as f32 * std::f32::consts::TAU;
            let p = centre + Vec3::new(ang.cos(), ang.sin(), 0.0) * radius * t;
            this.push(mesh.vertex(p, rgba(colour, a)));
        }
        for s in 0..SEGS as usize {
            let n = (s + 1) % SEGS as usize;
            if prev.is_empty() {
                mesh.tri(hub, this[s], this[n]);
            } else {
                mesh.quad(prev[s], this[s], this[n], prev[n]);
            }
        }
        prev = this;
    }
}

/// A closed circle of points in the horizontal plane (the first point is
/// repeated at the end).
pub fn ring(centre: Vec3, radius: f32) -> Vec<Vec3> {
    const SEGS: u32 = 48;
    (0..=SEGS)
        .map(|s| {
            let a = s as f32 / SEGS as f32 * std::f32::consts::TAU;
            centre + Vec3::new(a.cos(), a.sin(), 0.0) * radius
        })
        .collect()
}

/// A diamond (octahedron) of half-size `r` at `centre`.
pub fn gem(mesh: &mut Mesh, centre: Vec3, r: f32, colour: [u8; 3]) {
    let top = mesh.vertex(centre + Vec3::Z * r * 1.3, rgba(colour, 255));
    let bottom = mesh.vertex(centre - Vec3::Z * r * 1.3, rgba(colour, 255));
    let ring: Vec<u32> = [Vec3::X, Vec3::Y, -Vec3::X, -Vec3::Y]
        .iter()
        .map(|d| mesh.vertex(centre + *d * r, rgba(colour, 255)))
        .collect();
    for i in 0..4 {
        let j = (i + 1) % 4;
        mesh.tri(top, ring[i], ring[j]);
        mesh.tri(bottom, ring[j], ring[i]);
    }
}

/// A filled polygon, as a fan from its middle. Right for the loops the stub
/// and the engine draw (rectangles and L shapes); a polygon that curls round
/// its middle would need splitting first.
pub fn fan(mesh: &mut Mesh, outline: &[Vec3], colour: [u8; 3], alpha: u8) {
    let mut pts: Vec<Vec3> = outline.to_vec();
    if pts.len() > 1 && (pts[0] - pts[pts.len() - 1]).length() < 1e-3 {
        pts.pop();
    }
    if pts.len() < 3 {
        return;
    }
    let centre = pts.iter().copied().sum::<Vec3>() / pts.len() as f32;
    let hub = mesh.vertex(centre, rgba(colour, alpha));
    let ids: Vec<u32> = pts
        .iter()
        .map(|p| mesh.vertex(*p, rgba(colour, alpha)))
        .collect();
    for i in 0..ids.len() {
        let j = (i + 1) % ids.len();
        mesh.tri(hub, ids[i], ids[j]);
    }
}

/// Points along a polyline every `step` mm, with the cumulative length.
pub fn dashes(path: &[Vec3], dash: f32) -> Vec<(Vec<Vec3>, usize)> {
    let mut out = Vec::new();
    let mut idx = 0usize;
    let mut carry = 0.0f32;
    let mut cur: Vec<Vec3> = Vec::new();
    for w in path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = (b - a).length();
        if len < 1e-4 {
            continue;
        }
        let dir = (b - a) / len;
        let mut at = 0.0f32;
        if cur.is_empty() {
            cur.push(a);
        }
        while at < len {
            let room = dash - carry;
            let take = room.min(len - at);
            at += take;
            carry += take;
            cur.push(a + dir * at);
            if carry >= dash - 1e-4 {
                out.push((std::mem::take(&mut cur), idx));
                idx += 1;
                carry = 0.0;
                if at < len {
                    cur.push(a + dir * at);
                }
            }
        }
    }
    if cur.len() > 1 {
        out.push((cur, idx));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid(m: &Mesh) -> bool {
        m.positions.len() == m.colors.len()
            && m.indices.len().is_multiple_of(3)
            && m.indices.iter().all(|&i| (i as usize) < m.positions.len())
    }

    #[test]
    fn a_strip_along_a_corner_has_whole_triangles_and_the_right_width() {
        let mut m = Mesh::default();
        strip(
            &mut m,
            &[
                Vec3::new(0.0, 0.0, 5.0),
                Vec3::new(100.0, 0.0, 5.0),
                Vec3::new(100.0, 100.0, 5.0),
            ],
            10.0,
            [255, 0, 0],
            80,
            0,
        );
        assert!(valid(&m));
        assert!(!m.is_empty());
        let max_y = m.positions.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        let min_y = m.positions.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        assert!(min_y < -9.0 && max_y > 99.0, "{min_y} {max_y}");
        assert!(m.positions.iter().all(|p| (p.z - 5.0).abs() < 1e-4));
    }

    #[test]
    fn a_strip_of_one_point_or_a_repeated_point_is_nothing() {
        let mut m = Mesh::default();
        strip(&mut m, &[Vec3::ZERO], 5.0, [0; 3], 1, 1);
        strip(&mut m, &[Vec3::ZERO, Vec3::ZERO], 5.0, [0; 3], 1, 1);
        assert!(m.is_empty());
    }

    #[test]
    fn shapes_are_valid_meshes() {
        let mut m = Mesh::default();
        glow_disc(&mut m, Vec3::new(10.0, 10.0, 1.0), 80.0, [200, 50, 50], 120);
        gem(&mut m, Vec3::new(0.0, 0.0, 50.0), 8.0, [255, 200, 100]);
        fan(
            &mut m,
            &[
                Vec3::ZERO,
                Vec3::new(10.0, 0.0, 0.0),
                Vec3::new(10.0, 10.0, 0.0),
                Vec3::new(0.0, 10.0, 0.0),
                Vec3::ZERO,
            ],
            [1, 2, 3],
            90,
        );
        assert!(valid(&m));
        assert_eq!(ring(Vec3::ZERO, 5.0).len(), 49);
    }

    #[test]
    fn the_glow_fades_to_nothing_at_the_rim() {
        let mut m = Mesh::default();
        glow_disc(&mut m, Vec3::ZERO, 50.0, [9, 9, 9], 200);
        assert_eq!(m.colors[0][3], 200);
        assert_eq!(m.colors.last().unwrap()[3], 0);
    }

    #[test]
    fn dashes_alternate_along_the_path() {
        let d = dashes(&[Vec3::ZERO, Vec3::new(100.0, 0.0, 0.0)], 10.0);
        assert_eq!(d.len(), 10);
        assert_eq!(d[3].1, 3);
        assert!(d.iter().all(|(p, _)| p.len() >= 2));
    }
}
