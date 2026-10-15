//! Routing a whole set of jobs with duct fill: order, penalty, problems.

use crate::graph::Load;
use crate::{DEFAULT_WIRE_DIAMETER_MM, Job, Kind, Network, Route, Rules};

/// How full one duct is after routing.
#[derive(Debug, Clone, PartialEq)]
pub struct DuctFill {
    pub duct: u64,
    /// Cross-section taken by wires, mm squared.
    pub used_mm2: f64,
    /// Allowed cross-section: duct area times the fill limit.
    pub capacity_mm2: f64,
    /// `used / capacity`; above 1.0 the duct is overfull.
    pub ratio: f64,
    /// Jobs routed through it, in routing order.
    pub jobs: Vec<u64>,
}

/// Something the designer has to look at.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// No duct path: the wire runs in free air.
    FreeAir { job: u64 },
    /// The wire has no known size; a default was used.
    GuessedWire { job: u64 },
    /// A duct carries more than its fill limit.
    OverfullDuct {
        duct: u64,
        ratio: f64,
        jobs: Vec<u64>,
    },
}

/// Result of routing every job.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// One route per job, in the order the jobs were given.
    pub routes: Vec<Route>,
    /// Every duct, by id.
    pub fill: Vec<DuctFill>,
    /// Free-air wires, guessed wires, then overfull ducts.
    pub problems: Vec<Problem>,
}

fn wire_area(job: &Job) -> (f64, bool) {
    match job.diameter_mm {
        Some(d) if d.is_finite() && d > 0.0 => (std::f64::consts::FRAC_PI_4 * d * d, false),
        _ => (
            std::f64::consts::FRAC_PI_4 * DEFAULT_WIRE_DIAMETER_MM * DEFAULT_WIRE_DIAMETER_MM,
            true,
        ),
    }
}

/// Routes all jobs, longest direct distance first (then job id), so the long
/// wires get the short paths and the fill steers the later ones. The result
/// does not depend on the order the jobs are given in, except that routes
/// come back in that order.
pub fn plan(network: &Network, jobs: &[Job], rules: &Rules) -> Plan {
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by(|&a, &b| {
        let (da, db) = (direct(&jobs[a]), direct(&jobs[b]));
        db.total_cmp(&da).then(jobs[a].id.cmp(&jobs[b].id))
    });

    let n = network.duct_count();
    let mut used = vec![0.0; n];
    let mut through: Vec<Vec<u64>> = vec![Vec::new(); n];
    let mut routes: Vec<Option<Route>> = vec![None; jobs.len()];
    let mut guessed = Vec::new();

    for i in order {
        let job = &jobs[i];
        let (area, was_guessed) = wire_area(job);
        if was_guessed {
            guessed.push(i);
        }
        let route = network.route_loaded(
            job,
            rules,
            Load {
                used: &used,
                wire_area: area,
            },
        );
        let mut seen = Vec::new();
        for d in &route.ducts {
            if let Some(ix) = network.run_index(*d)
                && !seen.contains(&ix)
            {
                seen.push(ix);
                used[ix] += area;
                through[ix].push(job.id);
            }
        }
        routes[i] = Some(route);
    }
    let routes: Vec<Route> = routes.into_iter().flatten().collect();

    let fill: Vec<DuctFill> = (0..n)
        .map(|ix| {
            let (duct, area) = network.run_info(ix);
            let capacity = area * rules.fill_limit;
            DuctFill {
                duct,
                used_mm2: used[ix],
                capacity_mm2: capacity,
                ratio: if capacity > 0.0 {
                    used[ix] / capacity
                } else if used[ix] > 0.0 {
                    f64::INFINITY
                } else {
                    0.0
                },
                jobs: through[ix].clone(),
            }
        })
        .collect();

    let mut problems: Vec<Problem> = routes
        .iter()
        .filter(|r| r.kind == Kind::FreeAir)
        .map(|r| Problem::FreeAir { job: r.job })
        .collect();
    guessed.sort();
    problems.extend(
        guessed
            .into_iter()
            .map(|i| Problem::GuessedWire { job: jobs[i].id }),
    );
    problems.extend(
        fill.iter()
            .filter(|f| f.ratio > 1.0)
            .map(|f| Problem::OverfullDuct {
                duct: f.duct,
                ratio: f.ratio,
                jobs: f.jobs.clone(),
            }),
    );

    Plan {
        routes,
        fill,
        problems,
    }
}

fn direct(job: &Job) -> f64 {
    job.from.pos.manhattan(&job.to.pos)
}
