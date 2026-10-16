//! The run worker: snapshots arrive, cancel is prompt, a dead worker is an
//! error.

use std::sync::Arc;
use std::time::{Duration, Instant};

use novo_cfd::{CfdError, CfdSnapshot, Control, Fidelity, Solver};
use novo_thermal::Scenario;

use super::heat_run::{Event, Runner};
use novo_cfd::sched::Outcome;

struct Spin;

impl Solver for Spin {
    fn solve(
        &self,
        _: &Scenario,
        _: Fidelity,
        control: &Control,
        _: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        while !control.is_cancelled() {
            std::thread::sleep(Duration::from_millis(5));
        }
        Err(CfdError::Cancelled)
    }
}

struct Panics;

impl Solver for Panics {
    fn solve(
        &self,
        _: &Scenario,
        _: Fidelity,
        _: &Control,
        _: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        panic!("boom");
    }
}

fn scenario() -> Scenario {
    use novo_layout::Rect;
    use novo_layout::sizing::{Installation, Material};
    use novo_thermal::{Enclosure, Params};
    Scenario {
        enclosure: Enclosure {
            width_mm: 404.0,
            height_mm: 404.0,
            depth_mm: 204.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 20.0,
                y: 20.0,
                w: 360.0,
                h: 360.0,
            },
        },
        ambient_c: 35.0,
        devices: Vec::new(),
        obstacles: Vec::new(),
        vents: Vec::new(),
        fans: Vec::new(),
        params: Params::default(),
    }
}

fn drain(r: &mut Runner) -> Vec<Event> {
    let t0 = Instant::now();
    let mut all = Vec::new();
    while t0.elapsed() < Duration::from_secs(10) {
        all.extend(r.poll());
        if matches!(all.last(), Some(Event::Finished(_) | Event::Lost)) {
            return all;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!("no end event");
}

#[test]
fn cancel_ends_the_run_promptly() {
    let mut r = Runner::start(Arc::new(Spin), scenario(), Fidelity::Coarse);
    r.cancel();
    let events = drain(&mut r);
    assert!(matches!(
        events.last(),
        Some(Event::Finished(Outcome::Cancelled))
    ));
}

#[test]
fn a_panicking_solver_is_reported_as_a_failed_run() {
    let mut r = Runner::start(Arc::new(Panics), scenario(), Fidelity::Coarse);
    let events = drain(&mut r);
    assert!(matches!(
        events.last(),
        Some(Event::Finished(Outcome::Failed(_)))
    ));
}

#[test]
fn the_stub_solver_publishes_then_finishes() {
    let mut r = Runner::start(
        super::heat_run::default_solver(novo_cfd::turb::TurbModel::Laminar),
        scenario(),
        Fidelity::Coarse,
    );
    let events = drain(&mut r);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Snapshot(s) if s.is_consistent()))
    );
    assert!(matches!(
        events.last(),
        Some(Event::Finished(Outcome::Completed))
    ));
}

#[test]
fn the_demo_ladder_reports_progress_for_every_level() {
    let mut r = Runner::start(
        super::heat_run::default_solver(novo_cfd::turb::TurbModel::Laminar),
        scenario(),
        Fidelity::Fine,
    );
    let events = drain(&mut r);
    let levels: Vec<Fidelity> = events
        .iter()
        .filter_map(|e| match e {
            Event::Progress(p) => Some(p.fidelity),
            _ => None,
        })
        .collect();
    for f in [Fidelity::Coarse, Fidelity::Medium, Fidelity::Fine] {
        assert!(levels.contains(&f), "no progress for {f:?}");
    }
}
