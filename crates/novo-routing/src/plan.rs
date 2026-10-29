//! Routing a whole set of jobs with duct fill: order, penalty, problems.

use std::collections::BTreeMap;

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
    /// No way from a terminal to its duct avoids the body of a device: the
    /// wire is drawn across it. Move the duct or the device.
    StubThroughDevice { job: u64 },
    /// The wire has no known size; a default was used.
    GuessedWire { job: u64 },
    /// A duct without a known cross-section carries wires, so its fill
    /// cannot be checked.
    UnsizedDuct { duct: u64, jobs: Vec<u64> },
    /// A duct carries more than its fill limit.
    OverfullDuct {
        duct: u64,
        ratio: f64,
        jobs: Vec<u64>,
    },
    /// A wire asked to keep `mm` clear of another wire could not: the ducts
    /// that would do reach no way between its ends, so it was routed as if it
    /// had not asked. `gap_mm` is the closest the two wires' ducts come.
    ClearanceUnmet {
        job: u64,
        other: u64,
        mm: f64,
        gap_mm: f64,
    },
}

/// A hard clearance one job asks for: its ducts keep at least `mm` (edge to
/// edge) from the ducts of job `other`. Measured between ducts only; the
/// stubs from the terminals to the duct are not counted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clearance {
    pub other: u64,
    pub mm: f64,
}

/// Result of routing every job.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    /// One route per job, in the order the jobs were given.
    pub routes: Vec<Route>,
    /// Every duct, by id.
    pub fill: Vec<DuctFill>,
    /// Free-air wires, guessed wires, unsized ducts, then overfull ducts.
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
    plan_avoiding(network, jobs, rules, &BTreeMap::new())
}

/// Like [`plan`], with some jobs asked to keep out of some ducts (job id to
/// duct ids). A duct to avoid costs a lot more but is still used when it is
/// the only way, so the wire is never left unrouted for it.
pub fn plan_avoiding(
    network: &Network,
    jobs: &[Job],
    rules: &Rules,
    avoid: &BTreeMap<u64, Vec<u64>>,
) -> Plan {
    plan_constrained(network, jobs, rules, avoid, &BTreeMap::new())
}

/// Like [`plan_avoiding`], with hard clearances (job id to what it asks for).
/// A clearance counts for both wires, whoever of them is routed second keeps
/// out of the ducts too close to the first. When that leaves a wire no way
/// between its ends it is routed as if it had not asked and the plan says so
/// with [`Problem::ClearanceUnmet`]; no wire is left unrouted for it.
pub fn plan_constrained(
    network: &Network,
    jobs: &[Job],
    rules: &Rules,
    avoid: &BTreeMap<u64, Vec<u64>>,
    keep_clear: &BTreeMap<u64, Vec<Clearance>>,
) -> Plan {
    // Both partners carry the clearance.
    let mut pairs: BTreeMap<u64, Vec<Clearance>> = BTreeMap::new();
    for (job, list) in keep_clear {
        for c in list {
            pairs.entry(*job).or_default().push(*c);
            pairs.entry(c.other).or_default().push(Clearance {
                other: *job,
                mm: c.mm,
            });
        }
    }
    let mut order: Vec<usize> = (0..jobs.len()).collect();
    order.sort_by(|&a, &b| {
        let (da, db) = (direct(&jobs[a]), direct(&jobs[b]));
        db.total_cmp(&da).then(jobs[a].id.cmp(&jobs[b].id))
    });

    let n = network.run_count();
    let mut used = vec![0.0; n];
    let mut through: Vec<Vec<u64>> = vec![Vec::new(); n];
    let mut routes: Vec<Option<Route>> = vec![None; jobs.len()];
    let mut guessed = Vec::new();
    // Ducts of the jobs routed so far, for the clearances.
    let mut done: BTreeMap<u64, Vec<u64>> = BTreeMap::new();

    for i in order {
        let job = &jobs[i];
        let (area, was_guessed) = wire_area(job);
        if was_guessed {
            guessed.push(i);
        }
        let avoided = avoid.get(&job.id).map_or(&[][..], Vec::as_slice);
        // Ducts too close to a partner that is already routed.
        let mut forbid: Vec<u64> = Vec::new();
        for c in pairs.get(&job.id).into_iter().flatten() {
            if let Some(ducts) = done.get(&c.other) {
                forbid.extend(network.ducts_within(ducts, c.mm));
            }
        }
        forbid.sort_unstable();
        forbid.dedup();
        let load = |forbid| Load {
            used: &used,
            wire_area: area,
            avoid: avoided,
            forbid,
        };
        let mut route = network.route_loaded(job, rules, load(&forbid));
        if route.kind == Kind::FreeAir && !forbid.is_empty() {
            let open = network.route_loaded(job, rules, load(&[]));
            if open.kind != Kind::FreeAir {
                route = open;
            }
        }
        if route.kind == Kind::Ducted {
            done.insert(job.id, route.ducts.clone());
        }
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

    let fill_at: Vec<(usize, DuctFill)> = (0..n)
        .filter(|&ix| !network.is_rail(ix))
        .map(|ix| {
            let (duct, area) = network.run_info(ix);
            let capacity = area * rules.fill_limit;
            (
                ix,
                DuctFill {
                    duct,
                    used_mm2: used[ix],
                    capacity_mm2: capacity,
                    ratio: if network.sized(ix) {
                        used[ix] / capacity
                    } else {
                        0.0
                    },
                    jobs: through[ix].clone(),
                },
            )
        })
        .collect();
    let fill: Vec<DuctFill> = fill_at.iter().map(|(_, f)| f.clone()).collect();

    let mut problems: Vec<Problem> = routes
        .iter()
        .filter(|r| r.kind == Kind::FreeAir)
        .map(|r| Problem::FreeAir { job: r.job })
        .collect();
    problems.extend(
        routes
            .iter()
            .filter(|r| r.kind == Kind::Ducted && r.blocked)
            .map(|r| Problem::StubThroughDevice { job: r.job }),
    );
    guessed.sort();
    problems.extend(
        guessed
            .into_iter()
            .map(|i| Problem::GuessedWire { job: jobs[i].id }),
    );
    problems.extend(
        fill_at
            .iter()
            .filter(|(ix, f)| !network.sized(*ix) && !f.jobs.is_empty())
            .map(|(_, f)| Problem::UnsizedDuct {
                duct: f.duct,
                jobs: f.jobs.clone(),
            }),
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

    // A clearance that is not kept, whatever the reason, is said once.
    for (job, list) in keep_clear {
        for c in list {
            let (Some(a), Some(b)) = (done.get(job), done.get(&c.other)) else {
                continue;
            };
            let gap = a
                .iter()
                .flat_map(|x| b.iter().filter_map(|y| network.duct_gap(*x, *y)))
                .fold(f64::INFINITY, f64::min);
            if gap < c.mm {
                problems.push(Problem::ClearanceUnmet {
                    job: *job,
                    other: c.other,
                    mm: c.mm,
                    gap_mm: gap,
                });
            }
        }
    }

    Plan {
        routes,
        fill,
        problems,
    }
}

fn direct(job: &Job) -> f64 {
    job.from.pos.manhattan(&job.to.pos)
}
