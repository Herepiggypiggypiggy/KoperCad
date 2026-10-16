//! The grey diffuse radiosity system.

use super::patches::PatchSet;
use super::viewfactor::ViewFactors;

/// Stefan-Boltzmann constant, W/(m2 K4).
pub const SIGMA: f64 = 5.670_374_419e-8;

#[derive(Debug, Clone, PartialEq)]
pub struct RadResult {
    /// Radiosity `J` per patch, W/m2.
    pub radiosity_w_m2: Vec<f64>,
    /// Irradiation `G` per patch, W/m2.
    pub irradiation_w_m2: Vec<f64>,
    /// Net radiative flux leaving the surface, `J - G`, W/m2. Positive cools
    /// the patch, negative heats it.
    pub net_flux_w_m2: Vec<f64>,
    /// `4 e sigma T^3`, W/(m2 K): the slope of the flux against the patch's
    /// own temperature, to keep an implicit coupling with the conduction in
    /// the solid stable: `q(T) ~ q_old + h (T - T_old)`.
    pub h_lin_w_m2k: Vec<f64>,
    pub iterations: usize,
    /// Largest change of `J` in the last sweep, relative to the largest `J`.
    pub residual: f64,
    /// Sum of `A q` over all patches, W: zero up to the solver tolerance.
    pub imbalance_w: f64,
    /// Sum of the heat leaving patches that lose heat, W.
    pub exchanged_w: f64,
}

impl RadResult {
    /// Net power of a patch, W.
    pub fn power_w(&self, areas_m2: &[f64], i: usize) -> f64 {
        self.net_flux_w_m2[i] * areas_m2[i]
    }

    /// Net radiative power per mesh material id (0 is the enclosure wall),
    /// ascending ids, W. Positive means heat given off by that material.
    pub fn power_by_material(&self, patches: &PatchSet) -> Vec<(u16, f64)> {
        let mut out: Vec<(u16, f64)> = Vec::new();
        for (p, q) in patches.patches.iter().zip(&self.net_flux_w_m2) {
            let w = q * p.area_m2;
            match out.iter_mut().find(|e| e.0 == p.material) {
                Some(e) => e.1 += w,
                None => out.push((p.material, w)),
            }
        }
        out.sort_by_key(|e| e.0);
        out
    }
}

/// Solves for the patch temperatures `temperature_k` (kelvin).
pub fn solve(patches: &PatchSet, vf: &ViewFactors, temperature_k: &[f64]) -> RadResult {
    let eps: Vec<f64> = patches.patches.iter().map(|p| p.emissivity).collect();
    let areas: Vec<f64> = patches.patches.iter().map(|p| p.area_m2).collect();
    solve_with(&eps, &areas, vf, temperature_k)
}

/// Same for plain slices; used directly by tests with analytic factors.
pub fn solve_with(eps: &[f64], areas: &[f64], vf: &ViewFactors, t_k: &[f64]) -> RadResult {
    let n = vf.len();
    assert!(eps.len() == n && areas.len() == n && t_k.len() == n);
    let emit: Vec<f64> = (0..n).map(|i| eps[i] * SIGMA * t_k[i].powi(4)).collect();
    let mut j = emit.clone();
    let mut residual = f64::INFINITY;
    let mut iterations = 0;
    while iterations < 3000 {
        iterations += 1;
        let mut change: f64 = 0.0;
        for i in 0..n {
            let g: f64 = vf.row(i).iter().zip(&j).map(|(f, jj)| f * jj).sum();
            let new = emit[i] + (1.0 - eps[i]) * g;
            change = change.max((new - j[i]).abs());
            j[i] = new;
        }
        let top = j.iter().cloned().fold(0.0, f64::max).max(1e-30);
        residual = change / top;
        if residual < 1e-12 {
            break;
        }
    }
    let g: Vec<f64> = (0..n)
        .map(|i| vf.row(i).iter().zip(&j).map(|(f, jj)| f * jj).sum())
        .collect();
    let net: Vec<f64> = (0..n).map(|i| j[i] - g[i]).collect();
    let imbalance_w = (0..n).map(|i| net[i] * areas[i]).sum();
    let exchanged_w = (0..n).map(|i| (net[i] * areas[i]).max(0.0)).sum();
    RadResult {
        h_lin_w_m2k: (0..n)
            .map(|i| 4.0 * eps[i] * SIGMA * t_k[i].powi(3))
            .collect(),
        radiosity_w_m2: j,
        irradiation_w_m2: g,
        net_flux_w_m2: net,
        iterations,
        residual,
        imbalance_w,
        exchanged_w,
    }
}
