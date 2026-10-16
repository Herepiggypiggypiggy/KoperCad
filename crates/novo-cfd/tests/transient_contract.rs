//! The transient part of the result contract: the spec helpers and the stub
//! that lets the app lane build the time slider before the 3D march exists.

use novo_cfd::*;
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario() -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: 600.0,
            height_mm: 800.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 30.0,
                y: 30.0,
                w: 540.0,
                h: 740.0,
            },
        },
        ambient_c: 25.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: 200.0,
                y: 300.0,
                w: 120.0,
                h: 160.0,
            },
            height_mm: Some(60.0),
            loss_w: 60.0,
            rated_max_ambient_c: Some(55.0),
            rated_max_surface_c: Some(70.0),
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

#[test]
fn spec_helpers() {
    let s = TransientSpec {
        duration_s: 100.0,
        frame_interval_s: 30.0,
        load_schedule: vec![(10.0, 0.5), (50.0, 2.0)],
        start_from_steady: false,
    };
    assert_eq!(s.frame_count(), 5);
    assert_eq!(s.load_at(0.0), 1.0, "full load before the first entry");
    assert_eq!(s.load_at(10.0), 0.5);
    assert_eq!(s.load_at(60.0), 1.0, "clamped to 1");
    assert!(s.is_valid());
    let bad = TransientSpec {
        duration_s: 0.0,
        ..s.clone()
    };
    assert!(!bad.is_valid());
    assert_eq!(bad.frame_count(), 0);
    let unsorted = TransientSpec {
        load_schedule: vec![(50.0, 1.0), (10.0, 0.5)],
        ..s.clone()
    };
    assert!(!unsorted.is_valid(), "the schedule must be sorted by time");
    let nan = TransientSpec {
        frame_interval_s: f64::NAN,
        ..s
    };
    assert!(!nan.is_valid());
}

#[test]
fn the_stub_streams_consistent_frames_in_time_order() {
    let spec = TransientSpec {
        duration_s: 1800.0,
        frame_interval_s: 300.0,
        load_schedule: vec![(0.0, 1.0), (900.0, 0.0)],
        ..TransientSpec::default()
    };
    let mut frames = Vec::new();
    let sum = StubSolver
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            &spec,
            &Control::new(),
            &mut |f| frames.push(f.clone()),
        )
        .unwrap();
    assert!(frames.len() >= 3, "{} frames", frames.len());
    assert!(frames.iter().all(|f| f.snapshot.is_consistent()));
    assert!(frames.windows(2).all(|w| w[1].time_s > w[0].time_s));
    assert!(frames.last().unwrap().snapshot.progress.converged);
    // loads follow the schedule
    for f in &frames {
        assert_eq!(f.load_factor, spec.load_at(f.time_s));
    }
    assert_eq!(sum.fidelity, Fidelity::Coarse);
    assert_eq!(sum.times_s.len(), sum.mean_air_c.len());
    assert_eq!(sum.times_s.len(), sum.devices[0].surface_c.len());
    assert_eq!(sum.devices[0].id, 7);
    assert!(
        sum.devices[0].air_c.is_empty(),
        "the stub has no air series"
    );
    // the device heats up then cools after the load goes off
    let dev = &sum.devices[0].surface_c;
    let peak = dev.iter().cloned().fold(f64::MIN, f64::max);
    assert!(peak > dev[0] + 1.0);
    assert!(*dev.last().unwrap() < peak - 0.5, "{dev:?}");
}

#[test]
fn stub_transient_reports_unsupported_levels_bad_specs_and_cancel() {
    let spec = TransientSpec::default();
    let e = StubSolver
        .solve_transient(
            &scenario(),
            Fidelity::Fine,
            &spec,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(e, CfdError::Unsupported(Fidelity::Fine)));
    let bad = TransientSpec {
        frame_interval_s: 0.0,
        ..spec.clone()
    };
    let e = StubSolver
        .solve_transient(
            &scenario(),
            Fidelity::Coarse,
            &bad,
            &Control::new(),
            &mut |_| {},
        )
        .unwrap_err();
    assert!(matches!(e, CfdError::Scenario(_)));
    let c = Control::new();
    c.cancel();
    let e = StubSolver
        .solve_transient(&scenario(), Fidelity::Coarse, &spec, &c, &mut |_| {})
        .unwrap_err();
    assert!(matches!(e, CfdError::Cancelled));
}
