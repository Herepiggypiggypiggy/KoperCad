//! Thermal and airflow simulation of an enclosure (design
//! `docs/design/thermal-simulation.md`).
//!
//! A reduced-order model, not CFD: a depth-averaged 2D slice of the interior
//! seen from the front. Heat from devices goes into the air and the mounting
//! plate; the plate conducts sideways; air moves by buoyancy, fans and vents
//! through a Darcy–Forchheimer flow field with obstacles; walls lose heat by
//! the IEC 60890 coefficient. Every result is an estimate with a stated
//! band, never an authority.
//!
//! Entry points: [`simulate_steady`], [`simulate_transient`] and
//! [`simulate_steady_with_advice`].

mod advice;
pub mod bridge;
mod flow;
mod linalg;
pub mod model;
mod report;
pub mod result;
mod setup;
mod thermal;
pub mod wires;

pub use model::*;
pub use result::*;
pub use setup::Error;
/// Surface coefficient of a device, shared with the 3D engine (`novo-cfd`).
pub use thermal::device_h;

use setup::Setup;

pub(crate) fn run_steady(scn: &Scenario) -> Result<(Setup, thermal::State), Error> {
    run_steady_from(scn, None)
}

pub(crate) fn run_steady_from(
    scn: &Scenario,
    init: Option<&thermal::State>,
) -> Result<(Setup, thermal::State), Error> {
    let s = Setup::new(scn)?;
    let st = thermal::steady_from(&s, 1.0, init);
    Ok((s, st))
}

/// Runs `f` for every item on its own thread (the items are few and each run
/// takes long enough to pay for it), results in input order.
pub(crate) fn par_map<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    std::thread::scope(|scope| {
        let handles: Vec<_> = items.iter().map(|it| scope.spawn(|| f(it))).collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("simulation thread panicked"))
            .collect()
    })
}

/// What a steady run does besides the central estimate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Repeat the run at both ends of the assumptions for the uncertainty
    /// band (three runs instead of one).
    pub bands: bool,
    /// Try move, fan and vent changes and report what they win.
    pub advice: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            bands: true,
            advice: false,
        }
    }
}

/// Steady state at full load, with the uncertainty band and hot spots.
pub fn simulate_steady(scn: &Scenario) -> Result<ThermalResult, Error> {
    simulate_steady_with(scn, Options::default())
}

/// [`simulate_steady`] plus tried recommendations (move a device, add fans,
/// add vents). Each recommendation costs a few more simulations.
pub fn simulate_steady_with_advice(scn: &Scenario) -> Result<ThermalResult, Error> {
    simulate_steady_with(
        scn,
        Options {
            bands: true,
            advice: true,
        },
    )
}

/// Steady state with the extras chosen in `options`. Without bands the
/// bands equal the central estimate.
pub fn simulate_steady_with(scn: &Scenario, options: Options) -> Result<ThermalResult, Error> {
    let (s, st) = run_steady(scn)?;
    let mut res = report::build(scn, &s, &st, 1.0, None);
    if options.bands {
        widen_bands(scn, &st, &mut res)?;
    }
    res.hot_spots = report::hot_spots(&res);
    if options.advice {
        res.recommendations = advice::recommend(scn, &res)?;
    }
    Ok(res)
}

/// Runs the two ends of the assumption range and widens each device's band.
fn widen_bands(
    scn: &Scenario,
    central: &thermal::State,
    res: &mut ThermalResult,
) -> Result<(), Error> {
    let ends = [
        scn.params.cooling_favourable(),
        scn.params.cooling_unfavourable(),
    ];
    let runs = par_map(&ends, |params| -> Result<ThermalResult, Error> {
        let other = Scenario {
            params: params.clone(),
            ..scn.clone()
        };
        let (s, st) = run_steady_from(&other, Some(central))?;
        Ok(report::build(&other, &s, &st, 1.0, None))
    });
    for r in runs {
        let r = r?;
        for (d, o) in res.devices.iter_mut().zip(&r.devices) {
            d.surface_band_c.0 = d.surface_band_c.0.min(o.surface_c);
            d.surface_band_c.1 = d.surface_band_c.1.max(o.surface_c);
            d.air_band_c.0 = d.air_band_c.0.min(o.air_c);
            d.air_band_c.1 = d.air_band_c.1.max(o.air_c);
        }
    }
    for d in &mut res.devices {
        report::apply_margins(d);
    }
    Ok(())
}

/// Heat-up (or load step) over time.
pub fn simulate_transient(
    scn: &Scenario,
    settings: &TransientSettings,
) -> Result<TransientResult, Error> {
    if !(settings.duration_s > 0.0 && settings.steps >= 1) {
        return Err(Error::Invalid(
            "a transient run needs a duration and steps".into(),
        ));
    }
    let s = Setup::new(scn)?;
    let load_at = |t: f64| -> f64 {
        settings
            .load_schedule
            .iter()
            .rfind(|(from, _)| *from <= t)
            .map(|(_, f)| f.clamp(0.0, 1.0))
            .unwrap_or(1.0)
    };
    let mut st = if settings.start_from_steady {
        thermal::steady(&s, load_at(0.0))
    } else {
        thermal::State::uniform(&s, s.ambient)
    };
    st.converged = true;
    let dt = settings.duration_s / settings.steps as f64;
    let snapshot_every = settings.snapshot_every_s;
    let mut next_snapshot = snapshot_every.unwrap_or(f64::INFINITY);
    let mut samples = Vec::with_capacity(settings.steps + 1);
    let mut snapshots = Vec::new();
    let sample_of = |s: &Setup, st: &thermal::State, t: f64, load: f64| Sample {
        t_s: t,
        mean_air_c: st.t_air.iter().sum::<f64>() / s.n() as f64,
        max_air_c: st.t_air.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        device_c: st.t_dev.clone(),
        loss_w: s.total_loss_w() * load,
    };
    samples.push(sample_of(&s, &st, 0.0, load_at(0.0)));
    let snapshot_of = |s: &Setup, st: &thermal::State, t: f64| {
        let (vx, vy) = flow::cell_velocity(s, &st.flow);
        let mut plate = st.t_air.clone();
        for &i in &s.plate_cells {
            plate[i] = st.t_plate[i];
        }
        Snapshot {
            t_s: t,
            air_temp: report::grid_field(s, st.t_air.clone()),
            plate_temp: report::grid_field(s, plate),
            velocity: VectorField {
                grid: s.grid,
                vx,
                vy,
            },
        }
    };
    let mut last_step_prev = st.clone();
    let mut last_load = load_at(0.0);
    for k in 0..settings.steps {
        let t0 = k as f64 * dt;
        let t1 = t0 + dt;
        let load = load_at(t0);
        let prev = st.clone();
        let tb = st.t_air.clone();
        flow::solve(&s, &tb, &mut st.flow, 3);
        for _ in 0..2 {
            thermal::update_device_h(&s, &mut st);
            thermal::solve_temperatures(
                &s,
                &mut st,
                load,
                Some(&thermal::Step { dt, prev: &prev }),
            );
        }
        samples.push(sample_of(&s, &st, t1, load));
        if snapshot_every.is_some() && t1 + 1e-9 >= next_snapshot {
            snapshots.push(snapshot_of(&s, &st, t1));
            next_snapshot += snapshot_every.unwrap_or(f64::INFINITY);
        }
        last_step_prev = prev;
        last_load = load;
    }
    if snapshot_every.is_none() {
        // First, middle and last.
        snapshots.push(snapshot_of(
            &s,
            &thermal::State::uniform(&s, s.ambient),
            0.0,
        ));
        snapshots.push(snapshot_of(&s, &st, settings.duration_s));
    }
    let step = thermal::Step {
        dt,
        prev: &last_step_prev,
    };
    let mut last = report::build(scn, &s, &st, last_load, Some(&step));
    last.hot_spots = report::hot_spots(&last);

    // Time constant from the mean air rise of a constant-load run.
    let constant = settings.load_schedule.is_empty();
    let time_constant_s = if constant {
        time_constant(&samples, s.ambient)
    } else {
        None
    };
    Ok(TransientResult {
        samples,
        snapshots,
        last,
        time_constant_s,
    })
}

fn time_constant(samples: &[Sample], ambient: f64) -> Option<f64> {
    let final_rise = samples.last()?.mean_air_c - ambient;
    if final_rise <= 0.0 {
        return None;
    }
    // Needs to be close to settled for the final value to mean something.
    let n = samples.len();
    let before = samples[n.saturating_sub(1 + n / 20)].mean_air_c - ambient;
    if (final_rise - before).abs() > 0.02 * final_rise {
        return None;
    }
    let target = 0.632 * final_rise;
    samples
        .windows(2)
        .find(|w| w[1].mean_air_c - ambient >= target)
        .map(|w| {
            let (a, b) = (w[0].mean_air_c - ambient, w[1].mean_air_c - ambient);
            let f = if b > a { (target - a) / (b - a) } else { 1.0 };
            w[0].t_s + f * (w[1].t_s - w[0].t_s)
        })
}
