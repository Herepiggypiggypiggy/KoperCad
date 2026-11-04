//! Bar and cable impedance with skin and proximity effect, and current
//! sharing between parallel conductors (quasi-static partial-element model).
//!
//! What it is. Straight, parallel conductors of equal length and a common
//! start and end plane (the usual busbar run or cable pair). Every
//! cross-section is cut into a graded grid of filaments. Each filament has a
//! DC resistance `rho l / a` and a partial inductance; the partial mutual
//! inductance of two parallel filaments of length `l` at geometric mean
//! distance `g` is the textbook
//!
//! ```text
//! M = mu0/(2 pi) * ( l asinh(l/g) - sqrt(l^2 + g^2) + g )
//! ```
//!
//! (Neumann integral of two parallel filaments, Grover). The geometric mean
//! distance of two rectangles is found by averaging `ln d` over sub-cells, a
//! cell with itself using the exact GMD of a rectangle, `0.2235 (a + b)`. The
//! conductors are grouped: all filaments of a group have the same voltage
//! drop and carry a stated total current (one group is one phase, or one
//! phase made of several parallel bars or cables). With the filament
//! impedance matrix `Z = R + j w L` and the group incidence matrix `B`, the
//! group admittance is `B^T Z^-1 B`; its inverse is the group impedance
//! matrix, and the stated group currents give the group voltages and every
//! filament current. That is the standard filament method (Ruehli's partial
//! elements, Kamon, Tsuk and White 1994 for the skin and proximity grid); the
//! code here is written from the formulas, not from any tool.
//!
//! What it is not. A quasi-static, non-magnetic model: a steel mounting
//! plate, a magnetic enclosure or a steel bolt changes the real inductance and
//! is not seen. Straight parallel runs only: bends, taps, bolted joints and
//! contact resistance are not modelled. The values are *partial* values: the
//! return current has to be among the groups (three phases summing to zero, or
//! a go and a return group), or the voltage of a single group is only its
//! partial impedance. Round cables are treated as solid conductors (strands
//! lower the skin effect, so the AC factor of a stranded cable is a little
//! lower than shown). No capacitance, no radiation, no frequency above the
//! quasi-static limit. Every number is a planning number ([`MODEL_LABEL`]),
//! never a measurement and never a compliance statement.
//!
//! Exact against closed forms (see the tests): partial mutual inductance of
//! two filaments, self inductance of a flat bar (Rosa), DC resistance,
//! skin depth, the skin effect of a wide slab, the two-branch current
//! sharing, and power balance of the solve. The mesh error against the
//! slab formula is reported in `docs/busbar-impedance.md`.

use std::f64::consts::PI;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Vacuum permeability, H/m.
pub const MU0: f64 = 4.0e-7 * PI;

/// The honesty label shown next to every result of this module.
pub const MODEL_LABEL: &str = "Model: quasi-static, non-magnetic, straight parallel conductors";

/// What the model does not see, one short line each (shown behind the "?").
pub const NOT_MODELLED: &[&str] = &[
    "Steel plate, steel enclosure and steel bolts (magnetic material)",
    "Bends, taps, bolted joints and contact resistance",
    "Capacitance and anything above the quasi-static limit",
    "Stranding of cables (they are solid conductors here)",
    "Different lengths or staggered start and end of the conductors",
];

// ---------------------------------------------------------------- complex

/// A complex number (the module needs nothing more than the four rules).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct C {
    pub re: f64,
    pub im: f64,
}

impl C {
    pub const ZERO: C = C { re: 0.0, im: 0.0 };
    pub const ONE: C = C { re: 1.0, im: 0.0 };

    pub fn new(re: f64, im: f64) -> C {
        C { re, im }
    }

    /// Magnitude and phase in degrees to a complex number.
    pub fn polar(mag: f64, deg: f64) -> C {
        let a = deg.to_radians();
        C::new(mag * a.cos(), mag * a.sin())
    }

    pub fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }

    /// Phase in degrees.
    pub fn deg(self) -> f64 {
        self.im.atan2(self.re).to_degrees()
    }

    pub fn conj(self) -> C {
        C::new(self.re, -self.im)
    }

    pub fn norm_sq(self) -> f64 {
        self.re * self.re + self.im * self.im
    }
}

impl Add for C {
    type Output = C;
    fn add(self, o: C) -> C {
        C::new(self.re + o.re, self.im + o.im)
    }
}
impl Sub for C {
    type Output = C;
    fn sub(self, o: C) -> C {
        C::new(self.re - o.re, self.im - o.im)
    }
}
impl Mul for C {
    type Output = C;
    fn mul(self, o: C) -> C {
        C::new(
            self.re * o.re - self.im * o.im,
            self.re * o.im + self.im * o.re,
        )
    }
}
impl Mul<f64> for C {
    type Output = C;
    fn mul(self, k: f64) -> C {
        C::new(self.re * k, self.im * k)
    }
}
impl Div for C {
    type Output = C;
    fn div(self, o: C) -> C {
        let d = o.norm_sq();
        let n = self * o.conj();
        C::new(n.re / d, n.im / d)
    }
}
impl Neg for C {
    type Output = C;
    fn neg(self) -> C {
        C::new(-self.re, -self.im)
    }
}

/// Solve `A X = B` for a dense complex `n x n` matrix `a` (row-major) and
/// `m` right-hand sides (`b[k]` is column `k`). LU with partial pivoting.
/// `None` when the matrix is singular.
pub fn solve_complex(a: &[C], n: usize, b: &[Vec<C>]) -> Option<Vec<Vec<C>>> {
    let mut lu = a.to_vec();
    let mut perm: Vec<usize> = (0..n).collect();
    for k in 0..n {
        let mut p = k;
        let mut best = lu[k * n + k].abs();
        for i in k + 1..n {
            let v = lu[i * n + k].abs();
            if v > best {
                best = v;
                p = i;
            }
        }
        if best < 1e-300 || !best.is_finite() {
            return None;
        }
        if p != k {
            for j in 0..n {
                lu.swap(k * n + j, p * n + j);
            }
            perm.swap(k, p);
        }
        let piv = lu[k * n + k];
        for i in k + 1..n {
            let f = lu[i * n + k] / piv;
            lu[i * n + k] = f;
            for j in k + 1..n {
                let t = f * lu[k * n + j];
                lu[i * n + j] = lu[i * n + j] - t;
            }
        }
    }
    let mut out = Vec::with_capacity(b.len());
    for col in b {
        let mut x: Vec<C> = perm.iter().map(|&p| col[p]).collect();
        for i in 0..n {
            let mut s = x[i];
            for j in 0..i {
                s = s - lu[i * n + j] * x[j];
            }
            x[i] = s;
        }
        for i in (0..n).rev() {
            let mut s = x[i];
            for j in i + 1..n {
                s = s - lu[i * n + j] * x[j];
            }
            x[i] = s / lu[i * n + i];
        }
        out.push(x);
    }
    Some(out)
}

// -------------------------------------------------------------- materials

/// Conductor material. Resistivity and temperature coefficient are the
/// IEC 60228 / IEC 60287 style planning values (clause numbers unverified).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Material {
    Copper,
    Aluminium,
    /// Resistivity at 20 degC (ohm m) and temperature coefficient (1/K).
    Custom {
        rho20: f64,
        alpha: f64,
    },
}

impl Material {
    pub fn rho20(self) -> f64 {
        match self {
            Material::Copper => 1.7241e-8,
            Material::Aluminium => 2.8264e-8,
            Material::Custom { rho20, .. } => rho20,
        }
    }

    pub fn alpha(self) -> f64 {
        match self {
            Material::Copper => 0.00393,
            Material::Aluminium => 0.00403,
            Material::Custom { alpha, .. } => alpha,
        }
    }

    /// Resistivity at a conductor temperature, ohm m.
    pub fn rho(self, temp_c: f64) -> f64 {
        self.rho20() * (1.0 + self.alpha() * (temp_c - 20.0))
    }

    pub fn name(self) -> &'static str {
        match self {
            Material::Copper => "copper",
            Material::Aluminium => "aluminium",
            Material::Custom { .. } => "custom",
        }
    }
}

/// Skin depth `sqrt(2 rho / (w mu0))`, m. Infinite at 0 Hz.
pub fn skin_depth_m(rho: f64, hz: f64) -> f64 {
    if hz <= 0.0 {
        return f64::INFINITY;
    }
    (2.0 * rho / (2.0 * PI * hz * MU0)).sqrt()
}

// --------------------------------------------------------- partial elements

/// Partial mutual inductance of two parallel filaments of length `l` at
/// distance `g` (both in m), H. The same formula serves as the partial self
/// inductance of a conductor when `g` is its geometric mean distance.
pub fn partial_mutual(l: f64, g: f64) -> f64 {
    let g = g.max(1e-12);
    MU0 / (2.0 * PI) * (l * (l / g).asinh() - (l * l + g * g).sqrt() + g)
}

/// Geometric mean distance of a rectangle with itself, `0.2235 (w + t)`.
pub fn rect_gmd(w: f64, t: f64) -> f64 {
    0.2235 * (w + t)
}

/// Partial self inductance of a flat bar, H (dimensions in m): the filament
/// formula at the bar's own GMD. Rosa's closed form for the same bar is
/// `2e-7 l (ln(2l/(w+t)) + 0.5 + 0.2235 (w+t)/l)`.
pub fn bar_self_inductance(l: f64, w: f64, t: f64) -> f64 {
    partial_mutual(l, rect_gmd(w, t))
}

// ------------------------------------------------------------------ input

/// The cross-section of a conductor.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Section {
    /// Flat bar, `w` along x and `t` along y, mm.
    Rect { w_mm: f64, t_mm: f64 },
    /// Solid round conductor, diameter mm.
    Round { d_mm: f64 },
}

impl Section {
    pub fn area_mm2(self) -> f64 {
        match self {
            Section::Rect { w_mm, t_mm } => w_mm * t_mm,
            Section::Round { d_mm } => PI * d_mm * d_mm / 4.0,
        }
    }

    /// Perimeter, mm (the surface that gives the heat away).
    pub fn perimeter_mm(self) -> f64 {
        match self {
            Section::Rect { w_mm, t_mm } => 2.0 * (w_mm + t_mm),
            Section::Round { d_mm } => PI * d_mm,
        }
    }

    /// A round conductor of the given cross-section area (mm2).
    pub fn round_of_area(mm2: f64) -> Section {
        Section::Round {
            d_mm: (4.0 * mm2 / PI).sqrt(),
        }
    }
}

/// One conductor of the run. Positions are the centre of the cross-section in
/// a plane across the run, x to the right and y up, mm.
#[derive(Debug, Clone, PartialEq)]
pub struct Conductor {
    pub name: String,
    pub section: Section,
    pub x_mm: f64,
    pub y_mm: f64,
    pub material: Material,
    /// Conductor temperature the resistivity is taken at, degC.
    pub temp_c: f64,
    /// Index into [`Setup::group_current`].
    pub group: usize,
}

/// How finely the cross-sections are cut.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mesh {
    /// The most filaments along one side of a conductor.
    pub max_per_side: usize,
    /// Wanted filaments per skin depth (the cut is graded, finer at the
    /// edges, so this is the average).
    pub per_skin_depth: f64,
}

impl Default for Mesh {
    fn default() -> Mesh {
        Mesh {
            max_per_side: 14,
            per_skin_depth: 1.5,
        }
    }
}

/// The most filaments one solve takes (the dense matrix grows with the
/// square; above this the mesh is made coarser and a warning says so).
pub const MAX_FILAMENTS: usize = 900;

/// A run to solve.
#[derive(Debug, Clone, PartialEq)]
pub struct Setup {
    pub conductors: Vec<Conductor>,
    /// Length of the run, mm (common to all conductors).
    pub length_mm: f64,
    pub hz: f64,
    /// RMS total current of each group, A (complex: amplitude and phase).
    pub group_current: Vec<C>,
    pub group_name: Vec<String>,
    pub mesh: Mesh,
}

impl Setup {
    /// Three phases of `bars_per_phase` identical flat bars. Phases stand
    /// `pitch_mm` apart along x; the bars of one phase stack along y with
    /// `gap_mm` between them (`stacked`) or lie side by side along x with
    /// `gap_mm` between them. `amps` is the RMS current per phase.
    #[allow(clippy::too_many_arguments)]
    pub fn three_phase(
        material: Material,
        w_mm: f64,
        t_mm: f64,
        bars_per_phase: usize,
        gap_mm: f64,
        stacked: bool,
        pitch_mm: f64,
        length_mm: f64,
        hz: f64,
        amps: f64,
        temp_c: f64,
    ) -> Setup {
        let mut conductors = Vec::new();
        let n = bars_per_phase.max(1);
        for p in 0..3 {
            for k in 0..n {
                let off = k as f64 - (n as f64 - 1.0) / 2.0;
                let (dx, dy) = if stacked {
                    (0.0, off * (t_mm + gap_mm))
                } else {
                    (off * (w_mm + gap_mm), 0.0)
                };
                conductors.push(Conductor {
                    name: format!(
                        "L{}{}",
                        p + 1,
                        if n > 1 {
                            format!(".{}", k + 1)
                        } else {
                            String::new()
                        }
                    ),
                    section: Section::Rect { w_mm, t_mm },
                    x_mm: (p as f64 - 1.0) * pitch_mm + dx,
                    y_mm: dy,
                    material,
                    temp_c,
                    group: p,
                });
            }
        }
        Setup {
            conductors,
            length_mm,
            hz,
            group_current: vec![
                C::polar(amps, 0.0),
                C::polar(amps, -120.0),
                C::polar(amps, 120.0),
            ],
            group_name: vec!["L1".into(), "L2".into(), "L3".into()],
            mesh: Mesh::default(),
        }
    }
}

// ----------------------------------------------------------------- output

/// One filament of the solved run (for a heat map or a force estimate).
#[derive(Debug, Clone, PartialEq)]
pub struct FilamentResult {
    pub conductor: usize,
    pub x_mm: f64,
    pub y_mm: f64,
    pub w_mm: f64,
    pub t_mm: f64,
    /// RMS current in the filament, A.
    pub current: C,
    /// Joule loss in the filament, W.
    pub loss_w: f64,
}

/// What one conductor carries.
#[derive(Debug, Clone, PartialEq)]
pub struct ConductorResult {
    pub name: String,
    pub group: usize,
    /// Total RMS current of the conductor, A.
    pub current: C,
    /// The part of the group current it carries, in phase with the group
    /// current (the shares of a group add up to 1).
    pub share: f64,
    /// DC resistance of the conductor at its temperature, ohm.
    pub r_dc_ohm: f64,
    /// Joule loss, W.
    pub loss_w: f64,
    /// Loss divided by `r_dc |I|^2`: how much skin and proximity add at this
    /// current (1.0 = none).
    pub ac_factor: f64,
    /// Mean current density, A/mm2.
    pub amps_per_mm2: f64,
    /// Loss per metre of run, W/m.
    pub loss_w_per_m: f64,
    /// Loss per surface area, W/m2: the number that says which conductor
    /// heats first when the surfaces are cooled alike.
    pub flux_w_per_m2: f64,
    /// Skin depth in this conductor, mm.
    pub skin_depth_mm: f64,
}

/// The solved run.
#[derive(Debug, Clone, PartialEq)]
pub struct Solution {
    pub hz: f64,
    /// Group impedance matrix, ohm (row i: voltage drop of group i per ampere
    /// in group j). Partial values over the run length.
    pub group_z: Vec<Vec<C>>,
    /// Voltage drop along the run of each group at the stated currents, V.
    pub group_v: Vec<C>,
    /// `V_g / I_g` of each group at the stated currents, ohm: the impedance of
    /// that phase *in this arrangement* (includes the others' mutual
    /// coupling). Resistance is the real part, reactance the imaginary part.
    pub group_zeff: Vec<C>,
    pub conductors: Vec<ConductorResult>,
    pub filaments: Vec<FilamentResult>,
    pub total_loss_w: f64,
    /// Index of the conductor with the highest loss per surface area.
    pub hottest: Option<usize>,
    pub warnings: Vec<String>,
}

impl Solution {
    /// Effective inductance of a group at the stated currents, H.
    pub fn group_l_eff(&self, g: usize) -> f64 {
        if self.hz > 0.0 {
            self.group_zeff[g].im / (2.0 * PI * self.hz)
        } else {
            0.0
        }
    }
}

// ----------------------------------------------------------------- engine

/// The network solve behind [`solve`], on bare numbers so it can be checked
/// by hand: filament resistances `r` (ohm), the symmetric partial inductance
/// matrix `l` (H), the group of each filament and the stated group currents.
/// Returns the group impedance matrix, the group voltages and the filament
/// currents.
#[allow(clippy::type_complexity)]
pub fn solve_network(
    r: &[f64],
    l: &[Vec<f64>],
    group_of: &[usize],
    group_current: &[C],
    hz: f64,
) -> Option<(Vec<Vec<C>>, Vec<C>, Vec<C>)> {
    let n = r.len();
    let g = group_current.len();
    let w = 2.0 * PI * hz;
    let mut z = vec![C::ZERO; n * n];
    for i in 0..n {
        for j in 0..n {
            z[i * n + j] = C::new(if i == j { r[i] } else { 0.0 }, w * l[i][j]);
        }
    }
    // Y = Z^-1 B, one column per group.
    let rhs: Vec<Vec<C>> = (0..g)
        .map(|k| {
            (0..n)
                .map(|i| if group_of[i] == k { C::ONE } else { C::ZERO })
                .collect()
        })
        .collect();
    let y = solve_complex(&z, n, &rhs)?;
    // Yg = B^T Y
    let mut yg = vec![C::ZERO; g * g];
    for a in 0..g {
        for b in 0..g {
            let mut s = C::ZERO;
            for i in 0..n {
                if group_of[i] == a {
                    s = s + y[b][i];
                }
            }
            yg[a * g + b] = s;
        }
    }
    // Zg = Yg^-1 (columns of the identity)
    let eye: Vec<Vec<C>> = (0..g)
        .map(|k| {
            (0..g)
                .map(|i| if i == k { C::ONE } else { C::ZERO })
                .collect()
        })
        .collect();
    let zg_cols = solve_complex(&yg, g, &eye)?;
    let zg: Vec<Vec<C>> = (0..g)
        .map(|a| (0..g).map(|b| zg_cols[b][a]).collect())
        .collect();
    let v: Vec<C> = (0..g)
        .map(|a| {
            let mut s = C::ZERO;
            for b in 0..g {
                s = s + zg[a][b] * group_current[b];
            }
            s
        })
        .collect();
    let mut cur = vec![C::ZERO; n];
    for (i, c) in cur.iter_mut().enumerate() {
        let mut s = C::ZERO;
        for k in 0..g {
            s = s + y[k][i] * v[k];
        }
        *c = s;
    }
    Some((zg, v, cur))
}

struct Fil {
    conductor: usize,
    x: f64,
    y: f64,
    w: f64,
    t: f64,
    area: f64,
}

/// Cell edges of one side: cosine graded (finer at the edges), `n` cells.
fn edges(len: f64, n: usize) -> Vec<f64> {
    (0..=n)
        .map(|k| len * 0.5 * (1.0 - (PI * k as f64 / n as f64).cos()))
        .collect()
}

fn cells_for(len_m: f64, delta: f64, mesh: Mesh) -> usize {
    if !delta.is_finite() {
        return 1;
    }
    let n = (len_m / delta * mesh.per_skin_depth).ceil() as usize;
    n.clamp(1, mesh.max_per_side.max(1))
}

fn mesh_conductor(ci: usize, c: &Conductor, delta: f64, mesh: Mesh) -> Vec<Fil> {
    let mut out = Vec::new();
    match c.section {
        Section::Rect { w_mm, t_mm } => {
            let (w, t) = (w_mm * 1e-3, t_mm * 1e-3);
            let (nx, ny) = (cells_for(w, delta, mesh), cells_for(t, delta, mesh));
            let (ex, ey) = (edges(w, nx), edges(t, ny));
            for i in 0..nx {
                for j in 0..ny {
                    let (cw, ct) = (ex[i + 1] - ex[i], ey[j + 1] - ey[j]);
                    out.push(Fil {
                        conductor: ci,
                        x: c.x_mm * 1e-3 - w / 2.0 + (ex[i] + ex[i + 1]) / 2.0,
                        y: c.y_mm * 1e-3 - t / 2.0 + (ey[j] + ey[j + 1]) / 2.0,
                        w: cw,
                        t: ct,
                        area: cw * ct,
                    });
                }
            }
        }
        Section::Round { d_mm } => {
            let d = d_mm * 1e-3;
            let n = cells_for(d, delta, mesh);
            let (ex, ey) = (edges(d, n), edges(d, n));
            let mut tmp = Vec::new();
            for i in 0..n {
                for j in 0..n {
                    let (cw, ct) = (ex[i + 1] - ex[i], ey[j + 1] - ey[j]);
                    let (px, py) = (
                        (ex[i] + ex[i + 1]) / 2.0 - d / 2.0,
                        (ey[j] + ey[j + 1]) / 2.0 - d / 2.0,
                    );
                    if n == 1 || px.hypot(py) <= d / 2.0 {
                        tmp.push((px, py, cw, ct));
                    }
                }
            }
            let covered: f64 = tmp.iter().map(|f| f.2 * f.3).sum();
            let k = (PI * d * d / 4.0) / covered.max(1e-30);
            for (px, py, cw, ct) in tmp {
                out.push(Fil {
                    conductor: ci,
                    x: c.x_mm * 1e-3 + px,
                    y: c.y_mm * 1e-3 + py,
                    w: cw,
                    t: ct,
                    area: cw * ct * k,
                });
            }
        }
    }
    out
}

/// Geometric mean distance of two rectangular filaments. A filament with
/// itself: the closed form [`rect_gmd`]. Two filaments close together:
/// `ln d` averaged over sub-cells; far apart: the centre distance.
fn gmd(a: &Fil, b: &Fil, same: bool) -> f64 {
    if same {
        return rect_gmd(a.w, a.t);
    }
    let d = (a.x - b.x).hypot(a.y - b.y);
    let size = a.w.max(a.t).max(b.w).max(b.t);
    if d > 4.0 * size {
        return d;
    }
    let m = 5usize;
    let sub = |f: &Fil, k: usize| -> (f64, f64) {
        let (i, j) = (k / m, k % m);
        (
            f.x - f.w / 2.0 + f.w * (i as f64 + 0.5) / m as f64,
            f.y - f.t / 2.0 + f.t * (j as f64 + 0.5) / m as f64,
        )
    };
    let mut s = 0.0;
    for p in 0..m * m {
        let (px, py) = sub(a, p);
        for q in 0..m * m {
            let (qx, qy) = sub(b, q);
            s += (px - qx).hypot(py - qy).max(1e-12).ln();
        }
    }
    (s / (m * m * m * m) as f64).exp()
}

/// Solve a run. `None` when the setup is empty or the matrix is singular.
pub fn solve(setup: &Setup) -> Option<Solution> {
    if setup.conductors.is_empty() || setup.group_current.is_empty() || setup.length_mm <= 0.0 {
        return None;
    }
    let len = setup.length_mm * 1e-3;
    let ng = setup.group_current.len();
    let mut warnings = Vec::new();

    let mut mesh = setup.mesh;
    let mut deltas = Vec::new();
    let fils = loop {
        let mut fils: Vec<Fil> = Vec::new();
        deltas.clear();
        for (ci, c) in setup.conductors.iter().enumerate() {
            if c.group >= ng {
                return None;
            }
            let delta = skin_depth_m(c.material.rho(c.temp_c), setup.hz);
            deltas.push(delta);
            fils.extend(mesh_conductor(ci, c, delta, mesh));
        }
        if fils.len() <= MAX_FILAMENTS || mesh.max_per_side <= 1 {
            break fils;
        }
        mesh.max_per_side = (mesh.max_per_side * 2 / 3).max(1);
    };
    let n = fils.len();
    let rho_of = |f: &Fil| {
        let c = &setup.conductors[f.conductor];
        c.material.rho(c.temp_c)
    };
    let r: Vec<f64> = fils.iter().map(|f| rho_of(f) * len / f.area).collect();
    let mut l = vec![vec![0.0; n]; n];
    for i in 0..n {
        for j in i..n {
            let g = gmd(&fils[i], &fils[j], i == j);
            let m = partial_mutual(len, g);
            l[i][j] = m;
            l[j][i] = m;
        }
    }
    let group_of: Vec<usize> = fils
        .iter()
        .map(|f| setup.conductors[f.conductor].group)
        .collect();
    let (zg, v, cur) = solve_network(&r, &l, &group_of, &setup.group_current, setup.hz)?;

    // mesh adequacy
    if mesh.max_per_side < setup.mesh.max_per_side {
        warnings.push(format!(
            "The run is large: the mesh was made coarser to {} filaments per side",
            mesh.max_per_side
        ));
    }
    for (ci, c) in setup.conductors.iter().enumerate() {
        let d = deltas[ci];
        if !d.is_finite() {
            continue;
        }
        let worst = fils
            .iter()
            .filter(|f| f.conductor == ci)
            .map(|f| f.w.max(f.t))
            .fold(0.0, f64::max);
        if worst > 1.6 * d {
            warnings.push(format!(
                "{}: the mesh is coarse for a skin depth of {:.2} mm (largest filament {:.1} mm); the AC factor is low",
                c.name,
                d * 1e3,
                worst * 1e3
            ));
            break;
        }
    }

    let nc = setup.conductors.len();
    let mut ccur = vec![C::ZERO; nc];
    let mut closs = vec![0.0; nc];
    let mut filaments = Vec::with_capacity(n);
    for (i, f) in fils.iter().enumerate() {
        ccur[f.conductor] = ccur[f.conductor] + cur[i];
        let loss = r[i] * cur[i].norm_sq();
        closs[f.conductor] += loss;
        filaments.push(FilamentResult {
            conductor: f.conductor,
            x_mm: f.x * 1e3,
            y_mm: f.y * 1e3,
            w_mm: f.w * 1e3,
            t_mm: f.t * 1e3,
            current: cur[i],
            loss_w: loss,
        });
    }
    let mut conductors = Vec::with_capacity(nc);
    for (ci, c) in setup.conductors.iter().enumerate() {
        let a_m2 = c.section.area_mm2() * 1e-6;
        let r_dc = c.material.rho(c.temp_c) * len / a_m2;
        let ig = setup.group_current[c.group];
        let share = if ig.norm_sq() > 0.0 {
            (ccur[ci] * ig.conj()).re / ig.norm_sq()
        } else {
            0.0
        };
        let i2 = ccur[ci].norm_sq();
        let surface = c.section.perimeter_mm() * 1e-3 * len;
        conductors.push(ConductorResult {
            name: c.name.clone(),
            group: c.group,
            current: ccur[ci],
            share,
            r_dc_ohm: r_dc,
            loss_w: closs[ci],
            ac_factor: if i2 > 0.0 {
                closs[ci] / (r_dc * i2)
            } else {
                1.0
            },
            amps_per_mm2: ccur[ci].abs() / c.section.area_mm2(),
            loss_w_per_m: closs[ci] / len,
            flux_w_per_m2: closs[ci] / surface,
            skin_depth_mm: deltas[ci] * 1e3,
        });
    }
    let hottest = conductors
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.flux_w_per_m2.total_cmp(&b.1.flux_w_per_m2))
        .map(|(i, _)| i);
    let group_zeff: Vec<C> = (0..ng)
        .map(|g| {
            if setup.group_current[g].norm_sq() > 0.0 {
                v[g] / setup.group_current[g]
            } else {
                C::ZERO
            }
        })
        .collect();
    let net = setup.group_current.iter().fold(C::ZERO, |a, &b| a + b);
    if net.abs()
        > 1e-6
            * setup
                .group_current
                .iter()
                .map(|c| c.abs())
                .fold(0.0, f64::max)
    {
        warnings.push(
            "The group currents do not add up to zero: the return path is outside this model, so the reactance is a partial value".into(),
        );
    }
    Some(Solution {
        hz: setup.hz,
        group_z: zg,
        group_v: v,
        group_zeff,
        total_loss_w: closs.iter().sum(),
        conductors,
        filaments,
        hottest,
        warnings,
    })
}

/// Solve the same run at several frequencies.
pub fn sweep(setup: &Setup, hz: &[f64]) -> Vec<Solution> {
    hz.iter()
        .filter_map(|&f| {
            let mut s = setup.clone();
            s.hz = f;
            solve(&s)
        })
        .collect()
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, rel: f64) -> bool {
        (a - b).abs() <= rel * b.abs().max(1e-300)
    }

    #[test]
    fn mutual_of_two_filaments_by_hand() {
        // l = 1 m, d = 0.1 m: ln(10 + sqrt(101)) = 2.99822, sqrt(1.01) =
        // 1.004988, + 0.1 -> 2.09323; M = 2e-7 * 2.09323 = 4.1865e-7 H.
        let m = partial_mutual(1.0, 0.1);
        assert!(close(m, 4.1865e-7, 2e-4), "{m}");
    }

    #[test]
    fn flat_bar_self_inductance_matches_rosa() {
        // 100 x 10 mm, 1 m: Rosa 6.8497e-7 H, GMD form 6.8461e-7 H.
        let (l, w, t) = (1.0_f64, 0.1_f64, 0.01_f64);
        let rosa = 2e-7 * l * ((2.0 * l / (w + t)).ln() + 0.5 + 0.2235 * (w + t) / l);
        assert!(close(rosa, 6.8497e-7, 1e-4));
        let ours = bar_self_inductance(l, w, t);
        assert!(close(ours, rosa, 2e-3), "{ours} vs {rosa}");
    }

    #[test]
    fn skin_depth_of_copper_at_50_hz() {
        let d = skin_depth_m(Material::Copper.rho(20.0), 50.0);
        assert!(close(d, 9.346e-3, 5e-3), "{d}");
    }

    fn single(w: f64, t: f64, hz: f64, temp: f64) -> Setup {
        Setup {
            conductors: vec![Conductor {
                name: "bar".into(),
                section: Section::Rect { w_mm: w, t_mm: t },
                x_mm: 0.0,
                y_mm: 0.0,
                material: Material::Copper,
                temp_c: temp,
                group: 0,
            }],
            length_mm: 1000.0,
            hz,
            group_current: vec![C::new(100.0, 0.0)],
            group_name: vec!["a".into()],
            mesh: Mesh::default(),
        }
    }

    #[test]
    fn dc_resistance_and_temperature() {
        // 100 x 10 mm copper, 1 m, 20 degC: 1.7241e-8 / 1e-3 = 1.7241e-5 ohm.
        let s = solve(&single(100.0, 10.0, 0.0, 20.0)).unwrap();
        assert!(
            close(s.group_zeff[0].re, 1.7241e-5, 1e-9),
            "{:?}",
            s.group_zeff
        );
        assert!(s.group_zeff[0].im.abs() < 1e-12);
        // 70 degC: * (1 + 0.00393 * 50) = 1.1965
        let s = solve(&single(100.0, 10.0, 0.0, 70.0)).unwrap();
        assert!(close(s.group_zeff[0].re, 1.7241e-5 * 1.1965, 1e-9));
        // loss = R I^2 = 1.7241e-5 * 1e4 = 0.17241 W
        let s = solve(&single(100.0, 10.0, 0.0, 20.0)).unwrap();
        assert!(close(s.total_loss_w, 0.17241, 1e-9));
        assert!(close(s.conductors[0].ac_factor, 1.0, 1e-9));
    }

    /// Solid round wire, isolated: R_ac / R_dc = Re[(z/2) J0(z) / J1(z)] with
    /// z = (1 - j) a / delta (Bessel series, summed here by hand).
    fn round_wire_factor(a_over_delta: f64) -> f64 {
        let z = C::new(a_over_delta, -a_over_delta);
        let h = z * 0.5;
        let h2 = h * h;
        let (mut j0, mut j1) = (C::ONE, h);
        let (mut t0, mut t1) = (C::ONE, h);
        for m in 1..60 {
            let mf = m as f64;
            t0 = -(t0 * h2) * (1.0 / (mf * mf));
            t1 = -(t1 * h2) * (1.0 / (mf * (mf + 1.0)));
            j0 = j0 + t0;
            j1 = j1 + t1;
        }
        ((z * 0.5) * j0 / j1).re
    }

    #[test]
    fn skin_effect_of_a_round_wire_follows_the_bessel_solution() {
        // Series check at small argument: 1 + (a/delta)^4 / 48 (x = a/delta).
        let x: f64 = 0.5;
        assert!(close(round_wire_factor(x), 1.0 + x.powi(4) / 48.0, 1e-3));
        for (d_mm, hz) in [(20.0, 50.0), (30.0, 50.0), (30.0, 200.0), (20.0, 500.0)] {
            let mut s = single(1.0, 1.0, hz, 20.0);
            s.conductors[0].section = Section::Round { d_mm };
            s.mesh = Mesh {
                max_per_side: 24,
                per_skin_depth: 3.0,
            };
            let sol = solve(&s).unwrap();
            let delta = skin_depth_m(Material::Copper.rho(20.0), hz) * 1e3;
            let want = round_wire_factor(d_mm / 2.0 / delta);
            let got = sol.conductors[0].ac_factor;
            assert!(
                close(got, want, 0.04),
                "d {d_mm} hz {hz}: got {got} want {want}"
            );
        }
    }

    #[test]
    fn wide_bar_at_50_hz_crowds_to_its_edges() {
        // An isolated wide bar: the current prefers the edges because the
        // reactance of the middle is higher (no return conductor nearby).
        let s = solve(&single(300.0, 10.0, 50.0, 20.0)).unwrap();
        assert!(s.conductors[0].ac_factor > 1.05);
        let mid = s
            .filaments
            .iter()
            .min_by(|a, b| a.x_mm.abs().total_cmp(&b.x_mm.abs()))
            .unwrap();
        let edge = s
            .filaments
            .iter()
            .max_by(|a, b| a.x_mm.abs().total_cmp(&b.x_mm.abs()))
            .unwrap();
        assert!(
            edge.current.abs() / (edge.w_mm * edge.t_mm)
                > mid.current.abs() / (mid.w_mm * mid.t_mm)
        );
    }

    #[test]
    fn two_branch_sharing_by_hand() {
        // R = 0.1 mohm each, L = [[3, 1], [1, 2]] uH, 50 Hz.
        // I_a / I_b = (R + jw(Lb - M)) / (R + jw(La - M)).
        let r = [1e-4, 1e-4];
        let l = vec![vec![3e-6, 1e-6], vec![1e-6, 2e-6]];
        let w = 2.0 * PI * 50.0;
        let (_, _, cur) = solve_network(&r, &l, &[0, 0], &[C::new(100.0, 0.0)], 50.0).unwrap();
        let ratio = C::new(1e-4, w * 1e-6) / C::new(1e-4, w * 2e-6);
        let got = cur[0] / cur[1];
        assert!(close(got.re, ratio.re, 1e-9) && close(got.im, ratio.im, 1e-9));
        assert!(close(ratio.abs(), 0.5176, 2e-3), "{}", ratio.abs());
        let sum = cur[0] + cur[1];
        assert!(close(sum.re, 100.0, 1e-9) && sum.im.abs() < 1e-9);
    }

    #[test]
    fn three_phase_single_filament_matches_direct_sum() {
        // The reference uses centre distances; the solver uses the true
        // rectangle GMD, which differs by a few tenths of a percent and shows
        // up amplified because the self and mutual terms nearly cancel.
        // One filament per bar, low frequency: V_a = (R + jw L) I_a +
        // jw (M_ab I_b + M_ac I_c) with the partial mutual formula.
        let mut s = Setup::three_phase(
            Material::Copper,
            50.0,
            5.0,
            1,
            0.0,
            false,
            100.0,
            1000.0,
            50.0,
            200.0,
            20.0,
        );
        s.mesh = Mesh {
            max_per_side: 1,
            per_skin_depth: 0.0,
        };
        let sol = solve(&s).unwrap();
        let (l, ls) = (1.0, bar_self_inductance(1.0, 0.05, 0.005));
        let m12 = partial_mutual(l, 0.1);
        let m13 = partial_mutual(l, 0.2);
        let w = 2.0 * PI * 50.0;
        let r = 1.7241e-8 * 1.0 / (50e-3 * 5e-3);
        let i = &s.group_current;
        let va =
            C::new(r, w * ls) * i[0] + C::new(0.0, w * m12) * i[1] + C::new(0.0, w * m13) * i[2];
        let vb = C::new(r, w * ls) * i[1] + C::new(0.0, w * m12) * (i[0] + i[2]);
        assert!(
            close(sol.group_v[0].re, va.re, 1.5e-2) && close(sol.group_v[0].im, va.im, 1.5e-2),
            "{:?} {:?}",
            sol.group_v[0],
            va
        );
        assert!(close(sol.group_v[1].abs(), vb.abs(), 1.5e-2));
        // the middle phase has the smaller reactance
        assert!(sol.group_zeff[1].im < sol.group_zeff[0].im);
        // outer phases mirror each other
        assert!(close(
            sol.conductors[0].loss_w,
            sol.conductors[2].loss_w,
            1e-6
        ));
    }

    #[test]
    fn power_balance_closes() {
        let s = Setup::three_phase(
            Material::Copper,
            80.0,
            10.0,
            2,
            10.0,
            true,
            120.0,
            2000.0,
            50.0,
            1200.0,
            60.0,
        );
        let sol = solve(&s).unwrap();
        let p: f64 = (0..3)
            .map(|g| (sol.group_v[g] * s.group_current[g].conj()).re)
            .sum();
        assert!(
            close(p, sol.total_loss_w, 1e-9),
            "{p} vs {}",
            sol.total_loss_w
        );
        let per: f64 = sol.conductors.iter().map(|c| c.loss_w).sum();
        assert!(close(per, sol.total_loss_w, 1e-12));
    }

    #[test]
    fn parallel_bars_split_evenly_at_dc_and_unevenly_at_50_hz() {
        // Two bars per phase side by side: the bar of L2 nearest to L1 and
        // the one nearest to L3 see different mutual inductance.
        let mut s = Setup::three_phase(
            Material::Copper,
            60.0,
            10.0,
            2,
            10.0,
            false,
            200.0,
            1500.0,
            0.0,
            800.0,
            20.0,
        );
        let dc = solve(&s).unwrap();
        for c in &dc.conductors {
            assert!(close(c.share, 0.5, 1e-9), "{}", c.share);
        }
        s.hz = 50.0;
        let ac = solve(&s).unwrap();
        for g in 0..3 {
            let sh: f64 = ac
                .conductors
                .iter()
                .filter(|c| c.group == g)
                .map(|c| c.share)
                .sum();
            assert!(close(sh, 1.0, 1e-9));
        }
        let (a, b) = (ac.conductors[0].share, ac.conductors[1].share);
        assert!((a - b).abs() > 1e-4, "{a} {b}");
        assert!(ac.hottest.is_some());
    }

    #[test]
    fn proximity_raises_loss_of_a_go_and_return_pair() {
        let pair = |gap: f64, ret: f64| {
            let mk = |x: f64, g: usize| Conductor {
                name: format!("c{g}"),
                section: Section::Rect {
                    w_mm: 100.0,
                    t_mm: 10.0,
                },
                x_mm: x,
                y_mm: 0.0,
                material: Material::Copper,
                temp_c: 20.0,
                group: g,
            };
            Setup {
                conductors: vec![mk(0.0, 0), mk(100.0 + gap, 1)],
                length_mm: 2000.0,
                hz: 500.0,
                group_current: vec![C::new(500.0, 0.0), C::new(-500.0 * ret, 0.0)],
                group_name: vec!["go".into(), "back".into()],
                mesh: Mesh::default(),
            }
        };
        let near = solve(&pair(5.0, 1.0)).unwrap().total_loss_w;
        let far = solve(&pair(300.0, 1.0)).unwrap().total_loss_w;
        assert!(near > far, "{near} {far}");
        // the pair at 0 Hz has factor 1, so the rise is skin and proximity
        let mut dc = pair(5.0, 1.0);
        dc.hz = 0.0;
        let d = solve(&dc).unwrap();
        assert!(close(d.conductors[0].ac_factor, 1.0, 1e-9));
        assert!(near > d.total_loss_w);
    }

    #[test]
    fn resistance_rises_with_frequency() {
        let s = single(100.0, 20.0, 0.0, 20.0);
        let r: Vec<f64> = sweep(&s, &[0.0, 50.0, 200.0, 1000.0])
            .iter()
            .map(|x| x.group_zeff[0].re)
            .collect();
        assert!(r.windows(2).all(|w| w[1] > w[0]), "{r:?}");
    }

    #[test]
    fn round_cable_at_dc_has_the_right_area() {
        let mut s = single(1.0, 1.0, 0.0, 20.0);
        s.conductors[0].section = Section::round_of_area(240.0);
        let r = solve(&s).unwrap().group_zeff[0].re;
        assert!(close(r, 1.7241e-8 * 1.0 / 240e-6, 1e-9), "{r}");
    }

    #[test]
    fn unbalanced_groups_warn() {
        let mut s = single(50.0, 5.0, 50.0, 20.0);
        s.mesh = Mesh::default();
        assert!(!solve(&s).unwrap().warnings.is_empty());
    }
}
