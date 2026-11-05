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

/// Diameter assumed for a shield pigtail when the drawing does not say (mm):
/// the twisted-up braid of a small cable is about a 2 mm wire.
pub const PIGTAIL_WIRE_MM: f64 = 2.0;

/// Free-space permittivity, F/m.
const EPS0: f64 = 8.854_187_812_8e-12;

/// Self inductance in nH of a straight round wire, `len_mm` long and
/// `dia_mm` across, at high frequency (current on the surface, skin depth
/// far below the radius): mu0 l / (2 pi) (ln(2 l / r) - 1). Rosa and Grover
/// (Bureau of Standards, Formulas and Tables for the Calculation of Mutual
/// and Self Inductance). Valid for l well over r. This is the partial
/// inductance of the piece alone; the loop it closes adds the mutual term
/// with its return path, see [`loop_over_plane_nh`].
pub fn round_wire_nh(len_mm: f64, dia_mm: f64) -> f64 {
    let l = len_mm.max(0.0) / 1000.0;
    let r = (dia_mm.max(0.1) / 2.0) / 1000.0;
    if l <= r {
        return 0.0;
    }
    MU0 / (2.0 * std::f64::consts::PI) * l * ((2.0 * l / r).ln() - 1.0) * 1e9
}

/// Self inductance in nH of a flat strap (braid, bus strip), `len_mm` long,
/// `width_mm` wide and `thick_mm` thick, high-frequency current:
/// 0.2 l [ln(2 l / (w + t)) + 0.5005 + (w + t) / (3 l)] nH with l in mm
/// (Grover, Inductance Calculations, flat strip). A strap is much lower
/// than a round wire of the same length: width is what counts.
pub fn strap_nh(len_mm: f64, width_mm: f64, thick_mm: f64) -> f64 {
    let l = len_mm.max(0.0);
    let wt = (width_mm.max(0.1) + thick_mm.max(0.0)).max(0.1);
    if l <= wt {
        return 0.0;
    }
    0.2 * l * ((2.0 * l / wt).ln() + 0.5005 + wt / (3.0 * l))
}

/// Loop inductance per metre in nH/m of a round wire `dia_mm` across, its
/// axis `h_mm` above a conducting plane that carries the return:
/// mu0 / (2 pi) acosh(h / r). The wire has to be higher than its radius.
pub fn loop_over_plane_nh_per_m(h_mm: f64, dia_mm: f64) -> f64 {
    let r = dia_mm.max(0.1) / 2.0;
    let x = (h_mm / r).max(1.0);
    MU0 / (2.0 * std::f64::consts::PI) * x.acosh() * 1e9
}

/// Loop inductance in nH of a straight wire of `len_mm` over a plane (see
/// [`loop_over_plane_nh_per_m`]); end effects ignored, so it needs l >> h.
pub fn loop_over_plane_nh(len_mm: f64, h_mm: f64, dia_mm: f64) -> f64 {
    loop_over_plane_nh_per_m(h_mm, dia_mm) * len_mm.max(0.0) / 1000.0
}

/// Inductance of a shield pigtail in nH: the partial self inductance of a
/// [`PIGTAIL_WIRE_MM`] wire of that length ([`round_wire_nh`]). It replaces
/// the old rule of thumb of 1 nH per mm, which is the order of a pigtail
/// with its return path included; a pigtail alone computes lower (80 mm:
/// about 65 nH, not 80 nH).
pub fn pigtail_nh(mm: f64) -> f64 {
    round_wire_nh(mm, PIGTAIL_WIRE_MM)
}

/// Capacitance in pF per metre of a round wire `dia_mm` across, axis `h_mm`
/// above a plane, in air: 2 pi eps0 / acosh(h / r).
pub fn wire_over_plane_pf_per_m(h_mm: f64, dia_mm: f64) -> f64 {
    let r = dia_mm.max(0.1) / 2.0;
    let x = (h_mm / r).max(1.0001);
    2.0 * std::f64::consts::PI * EPS0 / x.acosh() * 1e12
}

/// Capacitance in pF per metre between a core and a concentric shield:
/// 2 pi eps0 eps_r / ln(D / d), with `ratio` = D / d, the inner diameter of
/// the shield over the diameter of the core. Only the ratio counts, not
/// the size.
pub fn coax_pf_per_m(ratio: f64, eps_r: f64) -> f64 {
    2.0 * std::f64::consts::PI * EPS0 * eps_r / ratio.max(1.0001).ln() * 1e12
}

/// Capacitance in pF per metre between two parallel round wires, centres
/// `d_mm` apart, `dia_mm` across, in a medium of relative permittivity
/// `eps_r`: pi eps0 eps_r / acosh(d / (2 r)).
pub fn two_wire_pf_per_m(d_mm: f64, dia_mm: f64, eps_r: f64) -> f64 {
    let r = dia_mm.max(0.1) / 2.0;
    let x = (d_mm / (2.0 * r)).max(1.0001);
    std::f64::consts::PI * EPS0 * eps_r / x.acosh() * 1e12
}

/// Current in mA through `c_pf` to earth at `v_rms` volts and `f_hz`
/// (I = 2 pi f C V, a sine; the cable's leakage to earth at mains
/// frequency).
pub fn leakage_ma(c_pf: f64, v_rms: f64, f_hz: f64) -> f64 {
    2.0 * std::f64::consts::PI * f_hz * c_pf * 1e-12 * v_rms * 1e3
}

/// Charging current in A that a voltage edge of `dv_dt_kv_per_us` pushes
/// through `c_pf` (I = C dV/dt): the short pulses a drive's output edges
/// send through the motor cable capacitance.
pub fn charging_current_a(c_pf: f64, dv_dt_kv_per_us: f64) -> f64 {
    c_pf * 1e-12 * dv_dt_kv_per_us * 1e9
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
