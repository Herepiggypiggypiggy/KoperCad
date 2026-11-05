//! Heat-up of a whole cabinet with the heat capacity of its walls, mounting
//! plate and devices ([`crate::mass`] holds the arithmetic).
//!
//! The cabinet is a small network: the **air** (one mean temperature), the
//! **devices** as one lump that carries all the losses, the **plate** and the
//! six **wall faces** as slabs across their thickness. The conductances are
//! chosen so that, in steady state, the walls carry exactly the IEC 60890
//! overall coefficient `k b A` per face; the wall only adds *mass* and a
//! surface temperature, it does not change the steady total. The wall's
//! inner film follows the 3D solver (`1.5 k b`), the outer film is what is
//! left of the IEC coefficient.
//!
//! **Model, not exact.** One air temperature (no stratification), devices
//! lumped, rails, ducts and wires not counted as mass, no radiation between
//! walls, no door gasket, no sun. The values of the lumped pieces (wall
//! steel, plate) are the standard material constants.

use crate::Error;
use crate::mass::{Layer, Network, Phase, Slab, Solved, Trajectory};
use crate::model::Scenario;
use crate::setup::{REAR, surface_factors};
use novo_layout::sizing::Material;

/// Cells across a wall.
const WALL_CELLS: usize = 3;
/// Air at room temperature: density times specific heat, J/(m3 K).
const AIR_RHO_CP: f64 = 1.16 * 1007.0;
/// Steel: density times specific heat, J/(m3 K).
const STEEL_RHO_CP: f64 = 7850.0 * 490.0;
/// Mass left on a wall or plate in the comparison "as before".
const NO_MASS: f64 = 1.0e-6;

/// Conductivity (W/m K) and rho c (J/m3 K) of the wall material.
pub fn wall_properties(m: Material) -> (f64, f64) {
    match m {
        Material::Steel => (50.0, STEEL_RHO_CP),
        Material::Stainless => (15.0, 7900.0 * 500.0),
        Material::Aluminium => (160.0, 2700.0 * 900.0),
        Material::Plastic => (0.3, 1400.0 * 1300.0),
    }
}

/// How to run the heat-up.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatUpSettings {
    pub duration_s: f64,
    /// Spacing of the returned curves, s.
    pub sample_s: f64,
    /// Load and ambient over time. Empty = full load at the scenario ambient
    /// for `duration_s`.
    pub phases: Vec<Phase>,
    /// Start at the steady state of the first phase instead of a cold cabinet
    /// (everything at the ambient).
    pub start_steady: bool,
    /// Report when the mean air first reaches this, degrees C.
    pub limit_c: Option<f64>,
}

impl Default for HeatUpSettings {
    fn default() -> Self {
        HeatUpSettings {
            duration_s: 4.0 * 3600.0,
            sample_s: 60.0,
            phases: Vec::new(),
            start_steady: false,
            limit_c: None,
        }
    }
}

/// Where the heat capacity sits, J/K.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MassBudget {
    pub air_j_per_k: f64,
    pub devices_j_per_k: f64,
    pub plate_j_per_k: f64,
    pub walls_j_per_k: f64,
    /// Sum of the wall face conductances air to ambient, W/K (IEC `k A_eff`).
    pub wall_ua_w_per_k: f64,
}

/// Inner and outer surface temperature of a wall face, degrees C.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FaceTemps {
    pub inner_c: f64,
    pub outer_c: f64,
}

/// Result of [`heat_up`].
#[derive(Debug, Clone, PartialEq)]
pub struct HeatUp {
    pub budget: MassBudget,
    /// Time constants of the cabinet, s, longest first.
    pub time_constants_s: Vec<f64>,
    /// Mean air at full load once everything has settled, degrees C.
    pub steady_air_c: f64,
    pub times_s: Vec<f64>,
    pub air_c: Vec<f64>,
    pub devices_c: Vec<f64>,
    pub plate_c: Vec<f64>,
    /// Mean of the wall cells.
    pub walls_c: Vec<f64>,
    /// Surfaces at the end of the run, front, rear, left, right, top, bottom.
    pub faces_end: [FaceTemps; 6],
    /// Time for the mean air to rise 63 % and 90 % of the way to steady state
    /// from a cold start at full load, with and without wall and plate mass.
    pub rise: RiseTimes,
    /// First time the mean air reaches `limit_c`.
    pub time_to_limit_s: Option<f64>,
}

/// Cold-start step response of the mean air.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RiseTimes {
    pub t63_s: f64,
    pub t90_s: f64,
    /// The same without the mass of walls and plate (devices and air only,
    /// what the heat-up read before the walls had mass).
    pub t63_no_wall_mass_s: f64,
    pub t90_no_wall_mass_s: f64,
}

impl RiseTimes {
    /// How much too short the 90 % time reads without the wall mass,
    /// 0 to 1 (0.4 = 40 % too short).
    pub fn first_order_error(&self) -> f64 {
        if self.t90_s > 0.0 {
            1.0 - self.t90_no_wall_mass_s / self.t90_s
        } else {
            0.0
        }
    }
}

struct Built {
    net: Network,
    air: usize,
    devices: usize,
    plate: Option<usize>,
    slabs: [Slab; 6],
    h_in_wall: [f64; 6],
    h_out_wall: [f64; 6],
    power: Vec<f64>,
    budget: MassBudget,
}

fn build(scn: &Scenario, wall_mass: f64) -> Result<Built, Error> {
    let e = &scn.enclosure;
    let p = &scn.params;
    if !(e.width_mm > 0.0 && e.height_mm > 0.0 && e.depth_mm > 0.0 && e.wall_mm > 0.0) {
        return Err(Error::Invalid(
            "the enclosure needs a size and a wall".into(),
        ));
    }
    let (wo, ho, d_o) = (
        e.width_mm / 1000.0,
        e.height_mm / 1000.0,
        e.depth_mm / 1000.0,
    );
    let (lam, wall_rho_cp) = wall_properties(e.material);
    let t_wall = e.wall_mm / 1000.0;
    let k = e.material.k() * p.wall_k_scale;
    let b = surface_factors(e.installation);
    let areas = [wo * ho, wo * ho, d_o * ho, d_o * ho, wo * d_o, wo * d_o];

    let mut net = Network::new();
    let mut devices_loss = 0.0;
    let mut dev_cap = 0.0;
    let mut dev_vol = 0.0;
    let mut dev_exposed = 0.0;
    let mut dev_foot = 0.0;
    let mut dev_len = 0.0f64;
    for d in &scn.devices {
        let foot = d.footprint.w * d.footprint.h / 1.0e6;
        let height_m = d.height() / 1000.0;
        devices_loss += d.loss_w;
        dev_cap += d
            .heat_capacity_j_per_k
            .unwrap_or(500.0 * foot * 100.0)
            .max(1.0);
        dev_vol += foot * height_m;
        dev_foot += foot;
        dev_exposed += foot + 2.0 * (d.footprint.w + d.footprint.h) / 1000.0 * height_m;
        dev_len = dev_len.max(d.footprint.h / 1000.0);
    }
    let int_vol = e.interior_width_mm().max(1.0) / 1000.0 * e.interior_height_mm().max(1.0)
        / 1000.0
        * e.interior_depth_mm().max(1.0)
        / 1000.0;
    let air_c = AIR_RHO_CP * (int_vol - dev_vol).max(0.2 * int_vol);
    let air = net.add_node(air_c);
    let devices = net.add_node(dev_cap.max(1.0));
    // Devices to air: surface coefficient at a typical 20 K surface rise.
    let h_dev = crate::device_h(
        dev_len.max(0.02),
        scn.ambient_c + 20.0,
        scn.ambient_c,
        0.1,
        p.emissivity,
        p.radiation_view_factor,
    );
    net.link(devices, air, h_dev * dev_exposed.max(1.0e-4));

    // Plate.
    let plate_area = (e.plate.w * e.plate.h / 1.0e6).min(areas[REAR]);
    let plate = if plate_area > 1.0e-6 {
        let c = STEEL_RHO_CP * p.plate_thickness_mm / 1000.0 * plate_area * wall_mass;
        Some(net.add_node(c.max(1.0e-9)))
    } else {
        None
    };
    if let Some(pl) = plate {
        net.link(devices, pl, p.plate_contact_h * dev_foot.max(1.0e-4));
    }

    // Walls.
    let layers = [Layer {
        thickness_m: t_wall,
        k: lam,
        rho_cp: wall_rho_cp * wall_mass,
        cells: WALL_CELLS,
    }];
    let mut h_in_wall = [0.0; 6];
    let mut h_out_wall = [0.0; 6];
    let mut ua = 0.0;
    let mut walls_c = 0.0;
    let mut slabs = Vec::with_capacity(6);
    for f in 0..6 {
        let u = k * b[f];
        let slab = net.add_slab(&layers, areas[f]);
        walls_c += slab.nodes.iter().map(|&n| net.capacity(n)).sum::<f64>();
        if u > 0.0 {
            // 1/u = 1/h_in + t/lambda + 1/h_out
            let h_in = 1.5 * u;
            let rest = 1.0 / u - 1.0 / h_in - t_wall / lam;
            let h_out = if rest > 1.0e-9 { 1.0 / rest } else { 1.0e6 };
            h_in_wall[f] = h_in;
            h_out_wall[f] = h_out;
            ua += u * areas[f];
            slab.connect_outer(&mut net, None, h_out);
            if f == REAR {
                let ap = plate_area;
                slab.connect_inner_area(&mut net, Some(air), h_in, areas[f] - ap);
                if let Some(pl) = plate {
                    // Air to plate to wall in series equals the film h_in over
                    // the covered part, as in the 2D engine.
                    let g_air = p.h_in.max(1.2 * h_in) * ap;
                    let g_tot = h_in * ap;
                    let g_rear = 1.0 / (1.0 / g_tot - 1.0 / g_air);
                    net.link(air, pl, g_air);
                    slab.connect_inner_area(&mut net, Some(pl), g_rear / ap, ap);
                }
            } else {
                slab.connect_inner(&mut net, Some(air), h_in);
            }
        } else {
            // Face with no path outwards (standing on the floor): mass only.
            slab.connect_inner(&mut net, Some(air), 1.5 * k * 0.5);
        }
        slabs.push(slab);
    }
    if plate.is_none() {
        // The rear wall had its whole film to the air; nothing to add.
    }
    let mut power = vec![0.0; net.len()];
    power[devices] = devices_loss;
    let budget = MassBudget {
        air_j_per_k: air_c,
        devices_j_per_k: dev_cap,
        plate_j_per_k: plate.map_or(0.0, |n| net.capacity(n)),
        walls_j_per_k: walls_c,
        wall_ua_w_per_k: ua,
    };
    let slabs: [Slab; 6] = slabs
        .try_into()
        .map_err(|_| Error::Invalid("six wall faces expected".into()))?;
    Ok(Built {
        net,
        air,
        devices,
        plate,
        slabs,
        h_in_wall,
        h_out_wall,
        power,
        budget,
    })
}

/// Time for node `node` to rise `fraction` of the way from the ambient to
/// its steady value under constant `power` from a cold start.
fn rise_time(s: &Solved, node: usize, power: &[f64], amb: f64, fraction: f64) -> f64 {
    let steady = s.steady(power, amb)[node];
    let level = amb + fraction * (steady - amb);
    let horizon = 12.0 * s.slowest_s();
    let cold = vec![amb; s.len()];
    // Geometric scan, then bisection inside the bracket.
    let (mut lo, mut hi) = (0.0, horizon / 4000.0);
    while s.advance(&cold, power, amb, hi)[node] < level && hi < horizon {
        lo = hi;
        hi *= 1.15;
    }
    for _ in 0..60 {
        let mid = 0.5 * (lo + hi);
        if s.advance(&cold, power, amb, mid)[node] < level {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    0.5 * (lo + hi)
}

/// Runs the heat-up of the cabinet in `scn`.
pub fn heat_up(scn: &Scenario, set: &HeatUpSettings) -> Result<HeatUp, Error> {
    if !(set.duration_s > 0.0 && set.sample_s > 0.0) {
        return Err(Error::Invalid(
            "a heat-up run needs a duration and a sample spacing".into(),
        ));
    }
    let built = build(scn, 1.0)?;
    let solved = built.net.solve()?;
    let amb = scn.ambient_c;
    let phases = if set.phases.is_empty() {
        vec![Phase {
            duration_s: set.duration_s,
            power_scale: 1.0,
            ambient_c: amb,
        }]
    } else {
        set.phases.clone()
    };
    let n = built.net.len();
    let start = if set.start_steady {
        let p0: Vec<f64> = built
            .power
            .iter()
            .map(|w| w * phases[0].power_scale)
            .collect();
        solved.steady(&p0, phases[0].ambient_c)
    } else {
        vec![amb; n]
    };
    let traj: Trajectory = solved.run(&start, &built.power, &phases, set.sample_s);

    let wall_nodes: Vec<usize> = built.slabs.iter().flat_map(|s| s.nodes.clone()).collect();
    let mean_of = |t: &[f64], nodes: &[usize]| -> f64 {
        nodes.iter().map(|&i| t[i]).sum::<f64>() / nodes.len() as f64
    };
    let air_c = traj.node_series(built.air);
    let devices_c = traj.node_series(built.devices);
    let plate_c = match built.plate {
        Some(p) => traj.node_series(p),
        None => air_c.clone(),
    };
    let walls_c: Vec<f64> = traj.temps.iter().map(|t| mean_of(t, &wall_nodes)).collect();
    let last = traj.temps.last().cloned().unwrap_or_else(|| start.clone());
    let last_amb = phases.last().map_or(amb, |p| p.ambient_c);
    let air_end = last[built.air];
    let mut faces_end = [FaceTemps {
        inner_c: amb,
        outer_c: amb,
    }; 6];
    for (f, ft) in faces_end.iter_mut().enumerate() {
        if built.h_in_wall[f] > 0.0 {
            ft.inner_c = built.slabs[f].inner_surface(&last, air_end, built.h_in_wall[f]);
            ft.outer_c = built.slabs[f].outer_surface(&last, last_amb, built.h_out_wall[f]);
        } else {
            ft.inner_c = last[built.slabs[f].nodes[0]];
            ft.outer_c = ft.inner_c;
        }
    }

    // Step response with and without the mass of walls and plate.
    let bare = build(scn, NO_MASS)?;
    let bare_solved = bare.net.solve()?;
    let rise = RiseTimes {
        t63_s: rise_time(&solved, built.air, &built.power, amb, 0.632),
        t90_s: rise_time(&solved, built.air, &built.power, amb, 0.9),
        t63_no_wall_mass_s: rise_time(&bare_solved, bare.air, &bare.power, amb, 0.632),
        t90_no_wall_mass_s: rise_time(&bare_solved, bare.air, &bare.power, amb, 0.9),
    };

    let steady_air_c = solved.steady(&built.power, amb)[built.air];
    let time_to_limit_s = set.limit_c.and_then(|l| traj.first_reach(built.air, l));
    Ok(HeatUp {
        budget: built.budget,
        time_constants_s: solved.time_constants_s(),
        steady_air_c,
        times_s: traj.times_s,
        air_c,
        devices_c,
        plate_c,
        walls_c,
        faces_end,
        rise,
        time_to_limit_s,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Device, Enclosure, Params};
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, effective_surface};

    fn scenario(loss_w: f64, wall_mm: f64) -> Scenario {
        Scenario {
            enclosure: Enclosure {
                width_mm: 800.0,
                height_mm: 1200.0,
                depth_mm: 400.0,
                wall_mm,
                material: Material::Steel,
                installation: Installation::default(),
                plate: Rect {
                    x: 20.0,
                    y: 20.0,
                    w: 740.0,
                    h: 1140.0,
                },
            },
            ambient_c: 25.0,
            devices: (0..12)
                .map(|i| Device {
                    id: i as u64,
                    footprint: Rect {
                        x: 60.0 + 170.0 * (i % 4) as f64,
                        y: 100.0 + 300.0 * (i / 4) as f64,
                        w: 150.0,
                        h: 250.0,
                    },
                    height_mm: Some(80.0),
                    loss_w: loss_w / 12.0,
                    rated_max_ambient_c: None,
                    rated_max_surface_c: None,
                    heat_capacity_j_per_k: None,
                })
                .collect(),
            obstacles: vec![],
            vents: vec![],
            fans: vec![],
            params: Params::default(),
        }
    }

    #[test]
    fn the_walls_keep_the_iec_60890_total() {
        let s = scenario(150.0, 1.5);
        let h = heat_up(&s, &HeatUpSettings::default()).unwrap();
        let e = &s.enclosure;
        let want =
            e.material.k() * effective_surface(e.width_mm, e.height_mm, e.depth_mm, e.installation);
        assert!(
            (h.budget.wall_ua_w_per_k - want).abs() < 1e-9 * want,
            "{} vs {want}",
            h.budget.wall_ua_w_per_k
        );
    }

    #[test]
    fn steady_state_balances_the_loss_through_the_walls() {
        // All heat leaves through the six faces: sum of face conductance times
        // the outer surface rise equals the loss.
        let s = scenario(150.0, 1.5);
        let h = heat_up(
            &s,
            &HeatUpSettings {
                duration_s: 40.0 * 3600.0,
                sample_s: 3600.0,
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let e = &s.enclosure;
        let b = surface_factors(e.installation);
        let (wo, ho, d) = (0.8, 1.2, 0.4);
        let areas = [wo * ho, wo * ho, d * ho, d * ho, wo * d, wo * d];
        let mut out = 0.0;
        for f in 0..6 {
            let u = e.material.k() * b[f];
            // outer film h_out = 1/(1/u - 1/(1.5u) - t/lambda)
            let h_out = 1.0 / (1.0 / u - 1.0 / (1.5 * u) - 0.0015 / 50.0);
            out += h_out * areas[f] * (h.faces_end[f].outer_c - 25.0);
        }
        assert!((out - 150.0).abs() < 0.5, "{out}");
        assert!((h.air_c.last().unwrap() - h.steady_air_c).abs() < 0.05);
    }

    #[test]
    fn wall_mass_slows_the_heat_up_a_lot() {
        let s = scenario(150.0, 1.5);
        let h = heat_up(&s, &HeatUpSettings::default()).unwrap();
        // Walls and plate hold as much heat as twelve devices.
        let shell = h.budget.walls_j_per_k + h.budget.plate_j_per_k;
        assert!(shell > h.budget.devices_j_per_k, "{:?}", h.budget);
        // Thin sheet steel, warm only part of the way to the air: it adds
        // about an eighth to the time, and more the thicker the wall.
        assert!(
            h.rise.t90_s > 1.08 * h.rise.t90_no_wall_mass_s,
            "{:?}",
            h.rise
        );
        assert!(h.rise.first_order_error() > 0.07);
        let heavy = heat_up(&scenario(150.0, 4.0), &HeatUpSettings::default()).unwrap();
        assert!(heavy.rise.first_order_error() > 1.3 * h.rise.first_order_error());
        assert!(
            h.time_constants_s[0] > 1800.0,
            "{:?}",
            &h.time_constants_s[..3]
        );
        // The curve is monotone from a cold start.
        assert!(h.air_c.windows(2).all(|w| w[1] >= w[0] - 1e-9));
    }

    #[test]
    fn thicker_walls_take_longer() {
        let thin = heat_up(&scenario(150.0, 1.0), &HeatUpSettings::default()).unwrap();
        let thick = heat_up(&scenario(150.0, 2.0), &HeatUpSettings::default()).unwrap();
        assert!(thick.rise.t90_s > thin.rise.t90_s);
        // The steady value is the same: mass does not change the total.
        assert!((thick.steady_air_c - thin.steady_air_c).abs() < 1e-4);
    }

    #[test]
    fn wall_surface_is_between_air_and_ambient() {
        let s = scenario(150.0, 1.5);
        let h = heat_up(
            &s,
            &HeatUpSettings {
                duration_s: 3.0 * 3600.0,
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let air = *h.air_c.last().unwrap();
        for (f, ft) in h.faces_end.iter().enumerate() {
            if f == 5 {
                continue;
            }
            assert!(
                ft.outer_c > 25.0 && ft.inner_c >= ft.outer_c - 1e-9,
                "{f}: {ft:?}"
            );
            assert!(ft.inner_c < air + 1e-9, "{f}: {ft:?} air {air}");
        }
    }

    #[test]
    fn a_duty_cycle_swings_less_than_the_load() {
        // 15 min on, 45 min off: the cabinet sees roughly the mean load.
        let s = scenario(400.0, 1.5);
        let cycle = Phase::duty_cycle(900.0, 1.0, 2700.0, 0.0, 25.0, 6);
        let h = heat_up(
            &s,
            &HeatUpSettings {
                duration_s: 6.0 * 3600.0,
                sample_s: 60.0,
                phases: cycle,
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let mean_load = heat_up(
            &s,
            &HeatUpSettings {
                duration_s: 6.0 * 3600.0,
                sample_s: 60.0,
                phases: vec![Phase {
                    duration_s: 6.0 * 3600.0,
                    power_scale: 0.25,
                    ambient_c: 25.0,
                }],
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let peak = h.air_c.iter().cloned().fold(f64::MIN, f64::max);
        let steady_full = h.steady_air_c;
        let mean_end = *mean_load.air_c.last().unwrap();
        assert!(
            peak < 25.0 + 0.5 * (steady_full - 25.0),
            "{peak} {steady_full}"
        );
        assert!(peak > mean_end - 1.0, "{peak} {mean_end}");
    }

    #[test]
    fn day_night_ambient_lags_the_inside() {
        let s = scenario(100.0, 1.5);
        let days = Phase::daily_ambient(25.0, 8.0, 15.0, 900.0, 4.0, 1.0);
        let h = heat_up(
            &s,
            &HeatUpSettings {
                duration_s: 4.0 * 86_400.0,
                sample_s: 900.0,
                phases: days,
                start_steady: true,
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let day4: Vec<f64> = h
            .times_s
            .iter()
            .zip(&h.air_c)
            .filter(|(t, _)| **t >= 3.0 * 86_400.0)
            .map(|(_, a)| *a)
            .collect();
        let swing = day4.iter().cloned().fold(f64::MIN, f64::max)
            - day4.iter().cloned().fold(f64::MAX, f64::min);
        // The inside swings with the outside, damped but not by half or less
        // at the 24 h period of a steel box with hours of time constant.
        assert!(swing > 5.0 && swing < 16.5, "{swing}");
    }

    #[test]
    fn the_time_to_limit_is_reported() {
        let s = scenario(150.0, 1.5);
        let probe = heat_up(&s, &HeatUpSettings::default()).unwrap();
        let limit = 25.0 + 0.5 * (probe.steady_air_c - 25.0);
        let h = heat_up(
            &s,
            &HeatUpSettings {
                limit_c: Some(limit),
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        let t = h.time_to_limit_s.expect("reaches half the rise");
        assert!(t > 0.0 && t < h.rise.t90_s);
        let none = heat_up(
            &s,
            &HeatUpSettings {
                limit_c: Some(probe.steady_air_c + 5.0),
                ..HeatUpSettings::default()
            },
        )
        .unwrap();
        assert_eq!(none.time_to_limit_s, None);
    }

    #[test]
    fn bad_input_is_an_error() {
        let mut s = scenario(100.0, 1.5);
        s.enclosure.wall_mm = 0.0;
        assert!(heat_up(&s, &HeatUpSettings::default()).is_err());
        let ok = scenario(100.0, 1.5);
        assert!(
            heat_up(
                &ok,
                &HeatUpSettings {
                    duration_s: 0.0,
                    ..HeatUpSettings::default()
                }
            )
            .is_err()
        );
    }
}
