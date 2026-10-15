//! Simulation engine (owned by WS-19, Phase 4): solves the connectivity graph
//! step by step for energized conductors, coil and contact states and timers.
//!
//! First piece: live potentials. The netlist shows the *declared* potentials
//! (what the drawing says each conductor carries, with every switched pole
//! counted as able to conduct). A [`PoleState`] says which switched poles are
//! closed right now, and [`solve`] returns the potential each net carries
//! under that state. Nothing here is stored in the project: the declared
//! potential stays what is drawn, the live one is a view on top of it.
//!
//! A potential flows from the nets that define it through closed poles into
//! nets that define nothing of their own; it never enters a net that has its
//! own definition, and a net reached by two different potentials is `Shorted`.
//! A pole is one group of `SymbolInstance::pass_through`: with the group
//! closed every pin of it conducts to every other.

mod coupling;
mod devices;
mod loads;
mod plc;

pub use coupling::{
    Clock, DEFAULT_DELAY, Settled, delay_seconds, is_coil, is_timer_contact, settle, settle_timed,
};
pub use devices::{
    DeviceStates, Direction, MotorRun, TimerKind, TimerProgress, device_states, is_lamp, is_motor,
    lamp_color, timer_progress,
};
pub use plc::{inputs_seen, is_plc_input, is_plc_output, output_drives};

pub use loads::{is_load, loads_on};

use std::collections::{HashSet, VecDeque};

use novo_core::{Endpoint, NetId, Netlist, PotentialId, Project, SymbolId};

/// Which switched poles are closed: `(symbol, group index into
/// `SymbolInstance::pass_through`)`. Everything not listed is open, so the
/// default state is a plant at rest with all contacts drawn open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PoleState {
    closed: HashSet<(SymbolId, usize)>,
}

impl PoleState {
    /// Every pole open.
    pub fn all_open() -> Self {
        Self::default()
    }

    /// Closes (`true`) or opens (`false`) one pole.
    pub fn set(&mut self, symbol: SymbolId, group: usize, closed: bool) {
        if closed {
            self.closed.insert((symbol, group));
        } else {
            self.closed.remove(&(symbol, group));
        }
    }

    /// Closes or opens every pole of a symbol (a contactor's main contacts).
    pub fn set_all(&mut self, project: &Project, symbol: SymbolId, closed: bool) {
        let groups = project.symbol(symbol).map_or(0, |s| s.pass_through.len());
        for group in 0..groups {
            self.set(symbol, group, closed);
        }
    }

    pub fn is_closed(&self, symbol: SymbolId, group: usize) -> bool {
        self.closed.contains(&(symbol, group))
    }
}

/// What one net carries under a [`PoleState`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Live {
    /// Nothing reaches this net.
    #[default]
    Dead,
    /// Exactly one potential.
    Carries(PotentialId),
    /// Two or more different potentials meet here.
    Shorted(Vec<PotentialId>),
}

impl Live {
    pub fn potential(&self) -> Option<PotentialId> {
        match self {
            Live::Carries(p) => Some(*p),
            _ => None,
        }
    }
}

/// The live potential of every net of one netlist.
#[derive(Debug, Clone)]
pub struct Solution {
    nets: Vec<Live>,
}

impl Solution {
    pub fn net(&self, net: NetId) -> &Live {
        &self.nets[net.0 as usize]
    }

    /// What the net of `end` carries; `Dead` for an endpoint in no net.
    pub fn of(&self, netlist: &Netlist, end: impl Into<Endpoint>) -> &Live {
        static DEAD: Live = Live::Dead;
        netlist.net_of(end).map_or(&DEAD, |n| self.net(n))
    }
}

/// Solves live potentials: each net that defines potentials keeps them; a
/// defined potential then flows breadth first through closed poles into
/// nets with no definition of their own.
pub fn solve(project: &Project, netlist: &Netlist, state: &PoleState) -> Solution {
    solve_driven(project, netlist, state, &[])
}

/// [`solve`] with outputs switched on: each `(pin, potential)` puts that
/// potential on the pin's net, as a source, unless the net defines potentials
/// of its own.
pub fn solve_driven(
    project: &Project,
    netlist: &Netlist,
    state: &PoleState,
    drives: &[(Endpoint, PotentialId)],
) -> Solution {
    let count = netlist.nets().len();
    // net -> (neighbour net) for every closed pole, both directions.
    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); count];
    for symbol in project.symbols() {
        for (g, group) in symbol.pass_through.iter().enumerate() {
            if !state.is_closed(symbol.id, g) {
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
                        edges[a].push(b);
                    }
                }
            }
        }
    }
    let defines: Vec<bool> = netlist
        .nets()
        .iter()
        .map(|n| !n.defined.is_empty())
        .collect();
    let mut found: Vec<Vec<PotentialId>> = vec![Vec::new(); count];
    for (i, net) in netlist.nets().iter().enumerate() {
        found[i].extend(net.defined.iter().copied());
    }
    // One walk per distinct (potential, source net); never enters a net
    // that defines its own potential.
    let mut sources: Vec<(PotentialId, usize)> = Vec::new();
    for (i, net) in netlist.nets().iter().enumerate() {
        sources.extend(net.defined.iter().map(|&p| (p, i)));
    }
    for &(pin, potential) in drives {
        if let Some(net) = netlist.net_of(pin).map(|n| n.0 as usize)
            && !defines[net]
        {
            found[net].push(potential);
            sources.push((potential, net));
        }
    }
    for (potential, start) in sources {
        let mut seen = vec![false; count];
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(net) = queue.pop_front() {
            for &next in &edges[net] {
                if seen[next] || defines[next] {
                    continue;
                }
                seen[next] = true;
                found[next].push(potential);
                queue.push_back(next);
            }
        }
    }
    let nets = found
        .into_iter()
        .map(|mut p| {
            p.sort();
            p.dedup();
            match p.len() {
                0 => Live::Dead,
                1 => Live::Carries(p[0]),
                _ => Live::Shorted(p),
            }
        })
        .collect();
    Solution { nets }
}
