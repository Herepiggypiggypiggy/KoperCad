//! Hostile cabinets for the heat model: zero and negative sizes, NaN and
//! infinite numbers, devices off the plate or on top of each other, huge
//! losses, no devices at all. The model answers with a result or an error;
//! it never panics, hangs or returns non-finite temperatures as fact.

use std::sync::mpsc;
use std::time::Duration;

use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn pick(&mut self, v: &[f64]) -> f64 {
        v[(self.next() % v.len() as u64) as usize]
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

const ODD: [f64; 14] = [
    0.0,
    -1.0,
    -1e6,
    1e-9,
    0.001,
    1.0,
    50.0,
    800.0,
    1e5,
    1e9,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::MIN_POSITIVE,
];
const SANE: [f64; 8] = [10.0, 50.0, 120.0, 300.0, 600.0, 800.0, 1200.0, 2000.0];

fn rect(r: &mut Rng, odd: bool) -> Rect {
    let p = |r: &mut Rng| if odd { r.pick(&ODD) } else { r.pick(&SANE) };
    Rect {
        x: p(r),
        y: p(r),
        w: p(r),
        h: p(r),
    }
}

fn scenario(r: &mut Rng, odd: bool) -> Scenario {
    let p = |r: &mut Rng| if odd { r.pick(&ODD) } else { r.pick(&SANE) };
    let mut devices = Vec::new();
    for i in 0..r.below(8) {
        devices.push(Device {
            id: i as u64,
            footprint: rect(r, odd),
            height_mm: if r.below(2) == 0 { None } else { Some(p(r)) },
            loss_w: if odd {
                r.pick(&ODD)
            } else {
                r.pick(&[0.0, 1.0, 20.0, 150.0])
            },
            rated_max_ambient_c: if r.below(2) == 0 { None } else { Some(p(r)) },
            rated_max_surface_c: if r.below(2) == 0 { None } else { Some(p(r)) },
            heat_capacity_j_per_k: if r.below(3) == 0 { Some(p(r)) } else { None },
        });
    }
    let obstacles = (0..r.below(4))
        .map(|_| Obstacle {
            kind: [ObstacleKind::Duct, ObstacleKind::Rail, ObstacleKind::Solid][r.below(3)],
            rect: rect(r, odd),
            blockage: if r.below(2) == 0 { None } else { Some(p(r)) },
            loss_w: if odd { r.pick(&ODD) } else { 0.0 },
        })
        .collect();
    let opening = |r: &mut Rng| Opening {
        placement: match r.below(5) {
            0 => Placement::Top { x_mm: p(r) },
            1 => Placement::Bottom { x_mm: p(r) },
            2 => Placement::Left { y_mm: p(r) },
            3 => Placement::Right { y_mm: p(r) },
            _ => Placement::Front {
                x_mm: p(r),
                y_mm: p(r),
            },
        },
        width_mm: p(r),
        height_mm: p(r),
    };
    let vents = (0..r.below(4))
        .map(|_| Vent {
            opening: opening(r),
            free_area_fraction: if odd { r.pick(&ODD) } else { 0.6 },
        })
        .collect();
    let fans = (0..r.below(3))
        .map(|_| Fan {
            opening: opening(r),
            mode: if r.below(2) == 0 {
                FanMode::Intake
            } else {
                FanMode::Exhaust
            },
            free_flow_m3h: p(r),
            max_pressure_pa: p(r),
        })
        .collect();
    Scenario {
        enclosure: Enclosure {
            width_mm: p(r),
            height_mm: p(r),
            depth_mm: p(r),
            wall_mm: if odd {
                r.pick(&ODD)
            } else {
                r.pick(&[1.5, 2.0, 5.0])
            },
            material: Material::Steel,
            installation: Installation::default(),
            plate: rect(r, odd),
        },
        ambient_c: if odd { r.pick(&ODD) } else { 35.0 },
        devices,
        obstacles,
        vents,
        fans,
        params: Params::default(),
    }
}

/// Runs `f` on a thread; fails when it panics or takes longer than `limit`.
fn guarded<T: Send + 'static>(
    what: String,
    limit: Duration,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)));
    });
    match rx.recv_timeout(limit) {
        Ok(Ok(v)) => v,
        Ok(Err(_)) => panic!("PANIC: {what}"),
        Err(_) => panic!("HANG (over {limit:?}): {what}"),
    }
}

fn check_result(what: &str, res: &Result<ThermalResult, Error>) {
    if let Ok(res) = res {
        for d in &res.devices {
            // A temperature that is not a number must not be shown as a result.
            assert!(
                d.surface_c.is_finite() && d.air_c.is_finite(),
                "{what}: device {} has non-finite temperature (surface {}, air {})",
                d.id,
                d.surface_c,
                d.air_c
            );
        }
    }
}

#[test]
fn hostile_numbers_never_panic_or_hang_the_heat_model() {
    let mut rng = Rng(0x00c0_ffee_1234_5678);
    let mut errors = 0;
    let mut oks = 0;
    for round in 0..120 {
        let scn = scenario(&mut rng, round % 2 == 0);
        let what = format!("round {round}: {scn:?}");
        let r = guarded(what.clone(), Duration::from_secs(60), move || {
            simulate_steady(&scn)
        });
        check_result(&what, &r);
        match r {
            Ok(_) => oks += 1,
            Err(_) => errors += 1,
        }
    }
    eprintln!("heat model: {oks} results, {errors} refusals");
}

#[test]
fn an_empty_cabinet_and_a_single_zero_loss_device_give_a_plain_answer() {
    let mut rng = Rng(7);
    let mut scn = scenario(&mut rng, false);
    scn.devices.clear();
    scn.vents.clear();
    scn.fans.clear();
    scn.obstacles.clear();
    let r = guarded("empty".into(), Duration::from_secs(60), move || {
        simulate_steady(&scn)
    });
    check_result("empty", &r);
}

#[test]
fn hostile_numbers_never_panic_the_transient_run_or_advice() {
    let mut rng = Rng(99);
    for round in 0..30 {
        let scn = scenario(&mut rng, round % 3 != 0);
        let what = format!("advice round {round}: {scn:?}");
        let r = guarded(what.clone(), Duration::from_secs(120), move || {
            simulate_steady_with_advice(&scn)
        });
        check_result(&what, &r);
    }
}

/// A sound cabinet with exactly one thing made hostile: the cases a user
/// reaches by typing one bad value into a sound project.
fn sound(r: &mut Rng) -> Scenario {
    let devices = (0..1 + r.below(5))
        .map(|i| Device {
            id: i as u64,
            footprint: Rect {
                x: 100.0 + 120.0 * i as f64,
                y: 150.0 + 40.0 * (i % 2) as f64,
                w: 90.0,
                h: 110.0,
            },
            height_mm: Some(70.0),
            loss_w: r.pick(&[0.0, 5.0, 40.0, 120.0]),
            rated_max_ambient_c: Some(55.0),
            rated_max_surface_c: Some(80.0),
            heat_capacity_j_per_k: None,
        })
        .collect();
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1000.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::default(),
            plate: Rect {
                x: 20.0,
                y: 20.0,
                w: 740.0,
                h: 940.0,
            },
        },
        ambient_c: 35.0,
        devices,
        obstacles: vec![Obstacle {
            kind: ObstacleKind::Duct,
            rect: Rect {
                x: 20.0,
                y: 400.0,
                w: 740.0,
                h: 60.0,
            },
            blockage: None,
            loss_w: 0.0,
        }],
        vents: vec![Vent {
            opening: Opening {
                placement: Placement::Bottom { x_mm: 400.0 },
                width_mm: 150.0,
                height_mm: 100.0,
            },
            free_area_fraction: 0.6,
        }],
        fans: vec![Fan {
            opening: Opening {
                placement: Placement::Top { x_mm: 400.0 },
                width_mm: 120.0,
                height_mm: 120.0,
            },
            mode: FanMode::Exhaust,
            free_flow_m3h: 100.0,
            max_pressure_pa: 40.0,
        }],
        params: Params::default(),
    }
}

#[test]
fn one_bad_value_in_a_sound_cabinet_never_panics_or_hangs() {
    let mut rng = Rng(4242);
    let mut oks = 0;
    for round in 0..48 {
        let mut scn = sound(&mut rng);
        let bad = rng.pick(&ODD);
        let field = round % 16;
        let d = scn.devices.len() - 1;
        match field {
            0 => scn.devices[d].loss_w = bad,
            1 => scn.devices[d].footprint.w = bad,
            2 => scn.devices[d].footprint.x = bad,
            3 => scn.devices[d].height_mm = Some(bad),
            4 => scn.enclosure.depth_mm = bad,
            5 => scn.enclosure.wall_mm = bad,
            6 => scn.ambient_c = bad,
            7 => scn.vents[0].free_area_fraction = bad,
            8 => scn.vents[0].opening.width_mm = bad,
            9 => scn.fans[0].free_flow_m3h = bad,
            10 => scn.fans[0].max_pressure_pa = bad,
            11 => scn.obstacles[0].blockage = Some(bad),
            12 => scn.obstacles[0].rect.h = bad,
            // A tiny cell size is a legal fine grid (cells are capped at max_cells), just
            // slow in a debug build, so the fuzz hurts the cell cap instead.
            13 => scn.params.max_cells = [0usize, 1, 10, 100][(round / 16) % 4],
            14 => {
                scn.params.max_iterations = if (0.0..1e6).contains(&bad) {
                    bad as usize
                } else {
                    0
                }
            }
            _ => scn.devices[d].rated_max_surface_c = Some(bad),
        }
        let what = format!("single fault round {round} field {field} value {bad}");
        let w = what.clone();
        let r = guarded(what, Duration::from_secs(120), move || {
            let r = simulate_steady(&scn);
            let _ = &w;
            r
        });
        check_result(
            &format!("single fault round {round} field {field} value {bad}"),
            &r,
        );
        if r.is_ok() {
            oks += 1;
        }
    }
    eprintln!("single faults: {oks} of 48 gave a result");
}

/// A negative wall thickness or a cabinet 1000 m wide is refused at once;
/// before the check the grid grew and the run seemed to hang.
#[test]
fn a_negative_wall_or_an_absurd_size_is_refused_quickly() {
    for (wall, w) in [(-1.0e6, 800.0), (-500.0, 800.0), (2.0, 1.0e6)] {
        let mut scn = sound(&mut Rng(1));
        scn.enclosure.wall_mm = wall;
        scn.enclosure.width_mm = w;
        let r = guarded(
            format!("wall {wall} width {w}"),
            Duration::from_secs(10),
            move || simulate_steady(&scn),
        );
        assert!(r.is_err(), "wall {wall} width {w} should be refused");
    }
}
