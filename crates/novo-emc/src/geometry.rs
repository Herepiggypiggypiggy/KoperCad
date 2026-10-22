//! Geometric indices (level 1) and the wire-over-plane estimate (level 2).

use crate::input::{Duct, P, Run};

/// One stretch of a run in or out of a duct, axis-aligned.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Piece {
    pub a: P,
    pub b: P,
    /// The duct the stretch lies in.
    pub duct: Option<u64>,
    /// Half the duct's width, or the wire's radius out of a duct.
    pub half_mm: f64,
}

impl Piece {
    pub fn horizontal(&self) -> bool {
        (self.a.1 - self.b.1).abs() < 1e-6
    }
    pub fn vertical(&self) -> bool {
        (self.a.0 - self.b.0).abs() < 1e-6
    }
    /// The coordinate along the stretch, low to high.
    fn span(&self) -> (f64, f64) {
        if self.horizontal() {
            (self.a.0.min(self.b.0), self.a.0.max(self.b.0))
        } else {
            (self.a.1.min(self.b.1), self.a.1.max(self.b.1))
        }
    }
    /// The coordinate across the stretch.
    fn offset(&self) -> f64 {
        if self.horizontal() {
            self.a.1
        } else {
            self.a.0
        }
    }
}

fn lies_in(piece: (P, P), duct: &Duct) -> Option<(f64, f64)> {
    let tol = (duct.width_mm / 2.0).max(1.0) + 0.5;
    let horiz = |a: P, b: P| (a.1 - b.1).abs() < 1e-6;
    let vert = |a: P, b: P| (a.0 - b.0).abs() < 1e-6;
    if horiz(piece.0, piece.1) && horiz(duct.a, duct.b) && (piece.0.1 - duct.a.1).abs() <= tol {
        let (lo, hi) = (piece.0.0.min(piece.1.0), piece.0.0.max(piece.1.0));
        let (dlo, dhi) = (duct.a.0.min(duct.b.0), duct.a.0.max(duct.b.0));
        let (l, h) = (lo.max(dlo), hi.min(dhi));
        return (h - l > 1e-6).then_some((l, h));
    }
    if vert(piece.0, piece.1) && vert(duct.a, duct.b) && (piece.0.0 - duct.a.0).abs() <= tol {
        let (lo, hi) = (piece.0.1.min(piece.1.1), piece.0.1.max(piece.1.1));
        let (dlo, dhi) = (duct.a.1.min(duct.b.1), duct.a.1.max(duct.b.1));
        let (l, h) = (lo.max(dlo), hi.min(dhi));
        return (h - l > 1e-6).then_some((l, h));
    }
    None
}

/// Cuts a run into stretches, marking those that lie in one of its ducts.
pub fn pieces(run: &Run, ducts: &[Duct]) -> Vec<Piece> {
    let mut out = Vec::new();
    let radius = run.diameter_mm / 2.0;
    for w in run.path.windows(2) {
        let (a, b) = (w[0], w[1]);
        let horiz = (a.1 - b.1).abs() < 1e-6;
        let vert = (a.0 - b.0).abs() < 1e-6;
        if !horiz && !vert {
            continue; // Routes are orthogonal; a diagonal has no parallel part.
        }
        let along = |p: P| if horiz { p.0 } else { p.1 };
        let at = |t: f64| if horiz { (t, a.1) } else { (a.0, t) };
        let (s_lo, s_hi) = (along(a).min(along(b)), along(a).max(along(b)));
        let mut cuts: Vec<(f64, f64, &Duct)> = run
            .ducts
            .iter()
            .filter_map(|id| ducts.iter().find(|d| d.id == *id))
            .filter_map(|d| lies_in((a, b), d).map(|(l, h)| (l, h, d)))
            .collect();
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        let mut cursor = s_lo;
        for (l, h, d) in cuts {
            let l = l.max(cursor);
            if h <= l {
                continue;
            }
            if l > cursor + 1e-6 {
                out.push(Piece {
                    a: at(cursor),
                    b: at(l),
                    duct: None,
                    half_mm: radius,
                });
            }
            out.push(Piece {
                a: at(l),
                b: at(h),
                duct: Some(d.id),
                half_mm: d.width_mm / 2.0,
            });
            cursor = h;
        }
        if s_hi > cursor + 1e-6 {
            out.push(Piece {
                a: at(cursor),
                b: at(s_hi),
                duct: None,
                half_mm: radius,
            });
        }
    }
    out
}

/// One parallel stretch of two runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stretch {
    pub length_mm: f64,
    /// Edge to edge: 0 in one duct, else centre distance less both halves.
    pub distance_mm: f64,
    /// Centre to centre.
    pub centre_mm: f64,
    pub duct: Option<u64>,
    pub divider: bool,
    /// The stretch on the first run's line and on the second's, for drawing.
    pub on_a: (P, P),
    pub on_b: (P, P),
}

/// The parallel stretches of two runs closer than `reach_mm` edge to edge.
pub fn parallel(a: &[Piece], b: &[Piece], ducts: &[Duct], reach_mm: f64) -> Vec<Stretch> {
    let mut out = Vec::new();
    for pa in a {
        for pb in b {
            if pa.horizontal() != pb.horizontal() {
                continue;
            }
            let (alo, ahi) = pa.span();
            let (blo, bhi) = pb.span();
            let overlap = ahi.min(bhi) - alo.max(blo);
            if overlap < 1.0 {
                continue;
            }
            let centre = (pa.offset() - pb.offset()).abs();
            let same_duct = pa.duct.is_some() && pa.duct == pb.duct;
            let distance = if same_duct {
                0.0
            } else {
                (centre - pa.half_mm - pb.half_mm).max(0.0)
            };
            if distance > reach_mm {
                continue;
            }
            let divider = same_duct
                && ducts
                    .iter()
                    .find(|d| Some(d.id) == pa.duct)
                    .is_some_and(|d| d.divider);
            let (lo, hi) = (alo.max(blo), ahi.min(bhi));
            let line = |off: f64| {
                if pa.horizontal() {
                    ((lo, off), (hi, off))
                } else {
                    ((off, lo), (off, hi))
                }
            };
            out.push(Stretch {
                on_a: line(pa.offset()),
                on_b: line(pb.offset()),
                length_mm: overlap,
                distance_mm: distance,
                centre_mm: centre,
                duct: if same_duct { pa.duct } else { None },
                divider,
            });
        }
    }
    out
}

/// Area in cm2 between a go path and a return path that share their
/// end devices: the polygon of one path and the other reversed, by the
/// shoelace formula (a crossing makes the two lobes cancel, so this is a
/// lower bound for crossed pairs).
pub fn loop_area_cm2(go: &[P], back: &[P]) -> f64 {
    if go.len() < 2 || back.len() < 2 {
        return 0.0;
    }
    // The return may be drawn from either end.
    let d = |p: P, q: P| ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt();
    let flip = d(go[0], back[0]) + d(go[go.len() - 1], back[back.len() - 1])
        > d(go[0], back[back.len() - 1]) + d(go[go.len() - 1], back[0]);
    let mut poly: Vec<P> = go.to_vec();
    if flip {
        poly.extend(back.iter().copied());
    } else {
        poly.extend(back.iter().rev().copied());
    }
    let mut sum = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        sum += p.0 * q.1 - q.0 * p.1;
    }
    (sum.abs() / 2.0) / 100.0
}

const MU0: f64 = 4.0e-7 * std::f64::consts::PI;

/// Mutual inductance per metre in H/m of two parallel wires at height `h`
/// above a conducting plane, `d` apart (all mm), the plane carrying the
/// return current (image method): mu0/(4 pi) ln(1 + 4 h^2 / d^2). `d` is
/// kept at least a wire diameter apart so touching wires stay finite.
pub fn mutual_per_m(h_mm: f64, d_mm: f64, wire_d_mm: f64) -> f64 {
    let d = d_mm.max(wire_d_mm).max(0.5);
    MU0 / (4.0 * std::f64::consts::PI) * (1.0 + 4.0 * h_mm * h_mm / (d * d)).ln()
}

/// Inductance of a straight round wire's pigtail in nH, rule of thumb
/// 10 nH per cm (the figure in `docs/EMC.md`).
pub fn pigtail_nh(mm: f64) -> f64 {
    mm.max(0.0) // 1 nH per mm
}

/// Impedance in ohm of a pigtail of `mm` at `f_hz`.
pub fn pigtail_ohm(mm: f64, f_hz: f64) -> f64 {
    2.0 * std::f64::consts::PI * f_hz * pigtail_nh(mm) * 1e-9
}

/// Gap in mm between two rectangles (x, y, w, h); 0 when they touch or overlap.
pub fn gap_mm(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> f64 {
    let dx = (a.0 - (b.0 + b.2)).max(b.0 - (a.0 + a.2)).max(0.0);
    let dy = (a.1 - (b.1 + b.3)).max(b.1 - (a.1 + a.3)).max(0.0);
    (dx * dx + dy * dy).sqrt()
}
