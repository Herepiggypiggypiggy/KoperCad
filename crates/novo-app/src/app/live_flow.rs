//! Current paths in the Live view: which nets lie on a route that carries a
//! load current. This is a model of where current is expected under the
//! switched poles, never a measured or per-wire amount, and it has no
//! direction (AC has none).
//!
//! A path runs from a net that defines a potential (a supply) through closed
//! poles to the pins of a device that is drawing current. Dead ends are
//! removed: a net that carries a potential only because a closed pole leads
//! to a stub (an open branch beyond it) is not on any route.

use std::collections::{BTreeSet, VecDeque};

use eframe::egui;
use novo_core::{Endpoint, NetId, Netlist, PotentialKind, Project, SymbolId};
use novo_sim::{Live, PoleState, Solution};

/// The nets on a current path to the pins of `sinks` (loads and energised
/// coils). Protective earth is never a current path here.
pub(super) fn flow_nets(
    project: &Project,
    netlist: &Netlist,
    solution: &Solution,
    poles: &PoleState,
    sinks: &BTreeSet<SymbolId>,
) -> BTreeSet<NetId> {
    let count = netlist.nets().len();
    let carries = |n: usize| match solution.net(NetId(n as u32)) {
        Live::Carries(p) => project
            .potential(*p)
            .is_some_and(|p| p.kind != PotentialKind::ProtectiveEarth),
        _ => false,
    };
    let defines: Vec<bool> = netlist
        .nets()
        .iter()
        .map(|n| !n.defined.is_empty())
        .collect();
    // Net to net through every closed pole.
    let mut edges: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); count];
    for symbol in project.symbols() {
        for (g, group) in symbol.pass_through.iter().enumerate() {
            if !poles.is_closed(symbol.id, g) {
                continue;
            }
            let nets: Vec<usize> = group
                .iter()
                .filter_map(|&pin| netlist.net_of(Endpoint::pin(symbol.id, pin)))
                .map(|n| n.0 as usize)
                .collect();
            for &a in &nets {
                for &b in &nets {
                    if a != b {
                        edges[a].insert(b);
                    }
                }
            }
        }
    }
    // The nets on the pins of a device that is drawing current.
    let mut terminals: BTreeSet<usize> = BTreeSet::new();
    for symbol in project.symbols().filter(|s| sinks.contains(&s.id)) {
        for pin in 0..symbol.pins.len() {
            if let Some(n) = netlist.net_of(Endpoint::pin(symbol.id, pin as u16)) {
                let n = n.0 as usize;
                if carries(n) {
                    terminals.insert(n);
                }
            }
        }
    }
    // Everything a terminal reaches through closed poles, stopping at supplies.
    let mut kept: BTreeSet<usize> = terminals.clone();
    let mut queue: VecDeque<usize> = terminals.iter().copied().collect();
    while let Some(net) = queue.pop_front() {
        if defines[net] && !terminals.contains(&net) {
            continue;
        }
        for &next in &edges[net] {
            if carries(next) && kept.insert(next) {
                queue.push_back(next);
            }
        }
    }
    // Strip dead-end stubs: a net that is neither a supply nor a terminal and
    // touches at most one other kept net leads nowhere.
    loop {
        let stubs: Vec<usize> = kept
            .iter()
            .copied()
            .filter(|n| !defines[*n] && !terminals.contains(n))
            .filter(|n| edges[*n].iter().filter(|m| kept.contains(m)).count() <= 1)
            .collect();
        if stubs.is_empty() {
            break;
        }
        for n in stubs {
            kept.remove(&n);
        }
    }
    // A supply that ended up touching nothing kept is not a path either.
    let alone: Vec<usize> = kept
        .iter()
        .copied()
        .filter(|n| {
            defines[*n] && !terminals.contains(n) && !edges[*n].iter().any(|m| kept.contains(m))
        })
        .collect();
    for n in alone {
        kept.remove(&n);
    }
    kept.into_iter().map(|n| NetId(n as u32)).collect()
}

/// Length of a dash and of the gap after it, in screen points.
const DASH: f32 = 5.0;
const GAP: f32 = 7.0;
/// Seconds for one slow breath of the dashes.
const BREATH_SECS: f64 = 3.2;

/// Opacity of the dashes at `time` seconds; `None` (a still picture or
/// Reduce motion) is a steady 0.85.
pub(super) fn dash_alpha(time: Option<f64>) -> f32 {
    time.map_or(0.85, |t| {
        let phase = (t / BREATH_SECS * std::f64::consts::TAU).sin() as f32;
        0.8 + 0.15 * phase
    })
}

/// Dash segments along a screen polyline: pairs of points, in order, every
/// dash `DASH` long with `GAP` between, restarting at each wire's start.
pub(super) fn dashes(points: &[egui::Pos2]) -> Vec<[egui::Pos2; 2]> {
    let mut out = Vec::new();
    let mut run = 0.0f32;
    for seg in points.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let len = (b - a).length();
        if len < 0.01 {
            continue;
        }
        let dir = (b - a) / len;
        let mut at = 0.0f32;
        while at < len {
            let period = DASH + GAP;
            let into = run % period;
            if into < DASH {
                let end = (at + (DASH - into)).min(len);
                out.push([a + dir * at, a + dir * end]);
                run += end - at;
                at = end;
            } else {
                let skip = (period - into).min(len - at);
                run += skip;
                at += skip;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashes_are_short_and_evenly_spaced() {
        let line = [egui::pos2(0.0, 0.0), egui::pos2(36.0, 0.0)];
        let d = dashes(&line);
        assert_eq!(d.len(), 3);
        assert_eq!(d[0], [egui::pos2(0.0, 0.0), egui::pos2(5.0, 0.0)]);
        assert_eq!(d[1][0], egui::pos2(12.0, 0.0));
        assert_eq!(d[2][0], egui::pos2(24.0, 0.0));
    }

    #[test]
    fn a_dash_carries_on_round_a_corner() {
        let line = [
            egui::pos2(0.0, 0.0),
            egui::pos2(3.0, 0.0),
            egui::pos2(3.0, 10.0),
        ];
        let d = dashes(&line);
        // 3 in the first leg, the remaining 2 of the dash in the second.
        assert_eq!(d[0], [egui::pos2(0.0, 0.0), egui::pos2(3.0, 0.0)]);
        assert_eq!(d[1], [egui::pos2(3.0, 0.0), egui::pos2(3.0, 2.0)]);
    }

    #[test]
    fn a_still_picture_has_a_steady_quiet_alpha_and_motion_stays_gentle() {
        assert_eq!(dash_alpha(None), 0.85);
        for i in 0..64 {
            let a = dash_alpha(Some(f64::from(i) * 0.1));
            assert!((0.65..=0.95).contains(&a), "{a}");
        }
    }
}
