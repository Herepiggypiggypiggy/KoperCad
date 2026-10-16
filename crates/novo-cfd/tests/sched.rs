use std::sync::Arc;
use std::time::{Duration, Instant};

use novo_cfd::sched::{Event, Outcome, Plan, Scheduler, Threads};
use novo_cfd::*;
use novo_layout::Rect;
use novo_layout::sizing::{Installation, Material};
use novo_thermal::*;

fn scenario() -> Scenario {
    Scenario {
        enclosure: Enclosure {
            width_mm: 800.0,
            height_mm: 1200.0,
            depth_mm: 300.0,
            wall_mm: 2.0,
            material: Material::Steel,
            installation: Installation::Wall,
            plate: Rect {
                x: 50.0,
                y: 50.0,
                w: 690.0,
                h: 1090.0,
            },
        },
        ambient_c: 25.0,
        devices: vec![Device {
            id: 7,
            footprint: Rect {
                x: 200.0,
                y: 300.0,
                w: 120.0,
                h: 200.0,
            },
            height_mm: Some(60.0),
            loss_w: 80.0,
            rated_max_ambient_c: Some(55.0),
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }],
        obstacles: vec![],
        vents: vec![],
        fans: vec![],
        params: Params::default(),
    }
}

/// Iterates until cancelled; publishes every iteration. Medium panics, Fine is unsupported.
struct Fake(CfdSnapshot);

impl Fake {
    fn new() -> Fake {
        let base = StubSolver.solve(&scenario(), Fidelity::Coarse, &Control::new(), &mut |_| {});
        Fake(base.unwrap())
    }
}

impl Solver for Fake {
    fn solve(
        &self,
        scn: &Scenario,
        fidelity: Fidelity,
        control: &Control,
        publish: &mut dyn FnMut(&CfdSnapshot),
    ) -> Result<CfdSnapshot, CfdError> {
        let base = self.0.clone();
        match fidelity {
            Fidelity::Fine => Err(CfdError::Unsupported(fidelity)),
            Fidelity::Medium if scn.ambient_c < 0.0 => panic!("boom"),
            Fidelity::Medium => loop {
                if control.is_cancelled() {
                    return Err(CfdError::Cancelled);
                }
                publish(&base);
                std::thread::sleep(Duration::from_millis(2));
            },
            _ => {
                publish(&base);
                Ok(base)
            }
        }
    }
}

fn plan() -> Plan {
    let mut p = Plan::up_to(Fidelity::Fine);
    p.threads = Threads::Fixed(2);
    p.min_publish_interval = Duration::ZERO;
    p
}

#[test]
fn stub_runs_to_completion_and_skips_unsupported_levels() {
    let mut s = Scheduler::new(Arc::new(StubSolver));
    let job = s.start(scenario(), plan());
    let t = Instant::now();
    while !job.is_finished() {
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(5));
    }
    let ev = job.poll();
    assert!(
        ev.iter()
            .any(|e| matches!(e, Event::LevelSkipped(Fidelity::Medium)))
    );
    assert!(matches!(
        ev.last(),
        Some(Event::Finished(Outcome::Completed))
    ));
    let snap = job.latest().expect("snapshot");
    assert!(snap.is_consistent());
}

#[test]
fn cancel_is_prompt_and_keeps_the_last_snapshot() {
    let mut s = Scheduler::new(Arc::new(Fake::new()));
    let job = s.start(scenario(), plan());
    let t = Instant::now();
    while job.latest().is_none()
        || !matches!(
            job.status(),
            novo_cfd::sched::JobStatus::Running {
                fidelity: Fidelity::Medium
            }
        )
    {
        assert!(t.elapsed() < Duration::from_secs(60));
        std::thread::sleep(Duration::from_millis(2));
    }
    let c = Instant::now();
    s.cancel();
    while !s.current().unwrap().is_finished() {
        assert!(c.elapsed() < Duration::from_secs(2), "cancel too slow");
        std::thread::sleep(Duration::from_millis(1));
    }
    let ev = s.current().unwrap().poll();
    assert!(matches!(
        ev.last(),
        Some(Event::Finished(Outcome::Cancelled))
    ));
    assert!(s.current().unwrap().latest().is_some());
}

#[test]
fn a_new_start_cancels_the_old_job() {
    let mut s = Scheduler::new(Arc::new(Fake::new()));
    s.start(scenario(), plan());
    let second = s.start(scenario(), Plan::single(Fidelity::Coarse));
    let t = Instant::now();
    while !second.is_finished() {
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[test]
fn a_panicking_solver_becomes_a_failure_not_a_crash() {
    let mut s = Scheduler::new(Arc::new(Fake::new()));
    let mut scn = scenario();
    scn.ambient_c = -5.0;
    let job = s.start(scn, plan());
    let t = Instant::now();
    while !job.is_finished() {
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    let ev = job.poll();
    assert!(matches!(
        ev.last(),
        Some(Event::Finished(Outcome::Failed(_)))
    ));
    assert!(job.latest().is_some(), "coarse result survives");
}

#[test]
fn throttle_drops_intermediate_snapshots_but_not_the_final() {
    let mut s = Scheduler::new(Arc::new(Fake::new()));
    let mut p = Plan::single(Fidelity::Coarse);
    p.min_publish_interval = Duration::from_secs(3600);
    let job = s.start(scenario(), p);
    let t = Instant::now();
    while !job.is_finished() {
        assert!(t.elapsed() < Duration::from_secs(30));
        std::thread::sleep(Duration::from_millis(2));
    }
    let n = job
        .poll()
        .iter()
        .filter(|e| matches!(e, Event::Snapshot(_)))
        .count();
    assert_eq!(n, 2, "first publish plus final");
}
