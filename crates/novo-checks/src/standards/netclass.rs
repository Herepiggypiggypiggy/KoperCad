//! Which circuit each net belongs to, and how the supply reaches it.
//!
//! The standards rules need two things the netlist does not state outright:
//! whether a wire is in a power or a control circuit (conductor colours,
//! voltage limits), and which devices sit between the supply and a load
//! (disconnecting, protection). Both are derived from what is drawn, never
//! guessed:
//!
//! - A net is a *protective-earth* or *neutral* net by the potentials it
//!   carries, otherwise a *power* net when a power load (motor, drive,
//!   heater) or a switching pole of three or more poles ends on it, and a
//!   *control* net when a coil, contact, operator, lamp, sensor or PLC
//!   channel does. A net that shows both, or neither, is *unknown* and the
//!   rules say so instead of judging it. A net with neither marker takes the
//!   class of its neighbours across switched poles when they agree.
//! - The supply graph runs over the switched poles the symbols declare
//!   (`SymbolInstance::pass_through`), starting at the nets that define a
//!   potential. "Reachable without a disconnector" is a search that does not
//!   cross the poles of disconnectors, so a bypass is found as well as a
//!   missing device.

use std::collections::{BTreeSet, HashMap, VecDeque};

use novo_core::{Endpoint, NetId, Netlist, PotentialKind, Project, SymbolId, SymbolInstance};

use super::symbols::{Class, class_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Circuit {
    ProtectiveEarth,
    Neutral,
    Power,
    Control,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Current {
    Ac,
    Dc,
    Unknown,
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    symbol: SymbolId,
    to: usize,
}

/// The circuit class of every net and the supply graph over switched poles.
pub(crate) struct Nets<'a> {
    pub(crate) netlist: &'a Netlist,
    circuit: Vec<Circuit>,
    current: Vec<Current>,
    /// Nets a control device (coil, contact, operator, lamp ...) ends on.
    control_marker: Vec<bool>,
    adj: Vec<Vec<Edge>>,
    dist: Vec<Option<u32>>,
    sources: Vec<usize>,
}

fn is_ground_pin(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "PE" | "PEN" | "N" | "GND" | "EARTH"
    )
}

impl<'a> Nets<'a> {
    pub(crate) fn new(project: &'a Project, netlist: &'a Netlist) -> Nets<'a> {
        let n = netlist.nets().len();
        let mut circuit = vec![Circuit::Unknown; n];
        let mut current = vec![Current::Unknown; n];
        let mut power = vec![false; n];
        let mut control = vec![false; n];
        for net in netlist.nets() {
            let i = net.id.0 as usize;
            let kinds: Vec<PotentialKind> = net
                .potentials
                .iter()
                .filter_map(|p| project.potential(*p))
                .map(|p| p.kind)
                .collect();
            let has = |k: PotentialKind| kinds.contains(&k);
            let earth = has(PotentialKind::ProtectiveEarth) || has(PotentialKind::Pen);
            let neutral = has(PotentialKind::Neutral);
            let live = has(PotentialKind::Line)
                || has(PotentialKind::DcPositive)
                || has(PotentialKind::DcNegative);
            if earth && !neutral && !live {
                circuit[i] = Circuit::ProtectiveEarth;
            } else if neutral && !earth && !live {
                circuit[i] = Circuit::Neutral;
            }
            current[i] = if has(PotentialKind::Line) && !has(PotentialKind::DcPositive) {
                Current::Ac
            } else if has(PotentialKind::DcPositive) || has(PotentialKind::DcNegative) {
                Current::Dc
            } else {
                Current::Unknown
            };
            for end in &net.endpoints {
                let Endpoint::Pin { symbol, pin } = end else {
                    continue;
                };
                let Some(s) = project.symbol(*symbol) else {
                    continue;
                };
                let class = class_of(s);
                let pin_name = s
                    .pins
                    .get(usize::from(*pin))
                    .map_or("", |p| p.name.as_str());
                if is_ground_pin(pin_name) {
                    continue;
                }
                let poles = s.pass_through.len();
                if class.is_power_load()
                    || (poles >= 3
                        && matches!(
                            class,
                            Class::Contactor
                                | Class::Breaker
                                | Class::Fuse
                                | Class::Disconnector
                                | Class::FuseSwitch
                                | Class::OverloadRelay
                                | Class::MotorProtectionBreaker
                                | Class::MotorCircuitProtector
                        ))
                {
                    power[i] = true;
                } else if class.is_control() {
                    control[i] = true;
                }
            }
        }
        for i in 0..n {
            if circuit[i] == Circuit::Unknown {
                circuit[i] = match (power[i], control[i]) {
                    (true, false) => Circuit::Power,
                    (false, true) => Circuit::Control,
                    _ => Circuit::Unknown,
                };
            }
        }

        // Switched-pole edges.
        let mut adj: Vec<Vec<Edge>> = vec![Vec::new(); n];
        for symbol in project.symbols() {
            for group in &symbol.pass_through {
                let nets: Vec<usize> = group
                    .iter()
                    .filter_map(|&pin| netlist.net_of(Endpoint::pin(symbol.id, pin)))
                    .map(|id| id.0 as usize)
                    .collect();
                for &a in &nets {
                    for &b in &nets {
                        if a != b {
                            adj[a].push(Edge {
                                symbol: symbol.id,
                                to: b,
                            });
                        }
                    }
                }
            }
        }

        // A net with no marker takes the class its neighbours agree on.
        for _ in 0..8 {
            let mut changed = false;
            for i in 0..n {
                if circuit[i] != Circuit::Unknown || power[i] || control[i] {
                    continue;
                }
                let seen: BTreeSet<u8> = adj[i]
                    .iter()
                    .filter_map(|e| match circuit[e.to] {
                        Circuit::Power => Some(0),
                        Circuit::Control => Some(1),
                        _ => None,
                    })
                    .collect();
                let class = match seen.iter().copied().collect::<Vec<_>>().as_slice() {
                    [0] => Circuit::Power,
                    [1] => Circuit::Control,
                    _ => continue,
                };
                circuit[i] = class;
                changed = true;
            }
            if !changed {
                break;
            }
        }

        // Distance from the nets that define a potential.
        let sources: Vec<usize> = netlist
            .nets()
            .iter()
            .filter(|net| !net.defined.is_empty())
            .map(|net| net.id.0 as usize)
            .collect();
        let mut dist: Vec<Option<u32>> = vec![None; n];
        let mut queue: VecDeque<usize> = VecDeque::new();
        for &s in &sources {
            dist[s] = Some(0);
            queue.push_back(s);
        }
        while let Some(a) = queue.pop_front() {
            let d = dist[a].unwrap_or(0);
            for e in &adj[a] {
                if dist[e.to].is_none() {
                    dist[e.to] = Some(d + 1);
                    queue.push_back(e.to);
                }
            }
        }

        Nets {
            netlist,
            circuit,
            current,
            control_marker: control,
            adj,
            dist,
            sources,
        }
    }

    pub(crate) fn circuit(&self, net: NetId) -> Circuit {
        self.circuit[net.0 as usize]
    }

    /// Whether a control device ends on the net itself (as opposed to a net
    /// that only leads to control devices through switched poles).
    pub(crate) fn has_control_device(&self, net: NetId) -> bool {
        self.control_marker[net.0 as usize]
    }

    pub(crate) fn current(&self, net: NetId) -> Current {
        self.current[net.0 as usize]
    }

    /// The nets of a symbol's pins that are not earth or neutral
    /// connections, each once.
    pub(crate) fn live_nets_of(&self, symbol: &SymbolInstance) -> Vec<NetId> {
        let mut nets: Vec<NetId> = symbol
            .pins
            .iter()
            .enumerate()
            .filter(|(_, p)| !is_ground_pin(&p.name))
            .filter_map(|(i, _)| {
                let pin = u16::try_from(i).ok()?;
                self.netlist.net_of(Endpoint::pin(symbol.id, pin))
            })
            .filter(|n| {
                !matches!(
                    self.circuit(*n),
                    Circuit::ProtectiveEarth | Circuit::Neutral
                )
            })
            .collect();
        nets.sort();
        nets.dedup();
        nets
    }

    /// Poles from the nearest source to the net, when the supply reaches it.
    pub(crate) fn distance(&self, net: NetId) -> Option<u32> {
        self.dist[net.0 as usize]
    }

    /// Whether the supply reaches the net through switched poles.
    pub(crate) fn is_fed(&self, net: NetId) -> bool {
        self.dist[net.0 as usize].is_some()
    }

    /// Every net the supply reaches from `start` without crossing a pole of
    /// a symbol for which `blocked` is true. A net in the result can be
    /// powered although every path through a blocked symbol is cut; a net
    /// outside it is either not fed or fed only through a blocked symbol.
    pub(crate) fn reach(&self, start: &[NetId], blocked: impl Fn(SymbolId) -> bool) -> Reach {
        let mut seen: Vec<bool> = vec![false; self.adj.len()];
        let mut queue: VecDeque<usize> = VecDeque::new();
        for s in start {
            seen[s.0 as usize] = true;
            queue.push_back(s.0 as usize);
        }
        while let Some(a) = queue.pop_front() {
            for e in &self.adj[a] {
                if !seen[e.to] && !blocked(e.symbol) {
                    seen[e.to] = true;
                    queue.push_back(e.to);
                }
            }
        }
        Reach(seen)
    }

    /// The nets on the supply side of `net`: every net from which the
    /// supply's walk reaches `net` over poles one step farther from the
    /// source each time, with `net` itself. Also the symbols whose poles
    /// lie on that way and how many poles from the source they are.
    pub(crate) fn upstream(&self, net: NetId) -> Upstream {
        let mut nets = BTreeSet::new();
        let mut poles: HashMap<SymbolId, u32> = HashMap::new();
        let mut stack = vec![net.0 as usize];
        nets.insert(net);
        while let Some(b) = stack.pop() {
            let Some(db) = self.dist[b] else { continue };
            for e in &self.adj[b] {
                // The adjacency is symmetric; `e.to` is upstream when it is
                // one pole nearer the source.
                if self.dist[e.to].is_some_and(|da| da + 1 == db) {
                    poles
                        .entry(e.symbol)
                        .and_modify(|d| *d = (*d).min(db))
                        .or_insert(db);
                    if nets.insert(NetId(e.to as u32)) {
                        stack.push(e.to);
                    }
                }
            }
        }
        Upstream { nets, poles }
    }

    pub(crate) fn sources(&self) -> impl Iterator<Item = NetId> + '_ {
        self.sources.iter().map(|&s| NetId(s as u32))
    }
}

/// The nets a search reached, see [`Nets::reach`].
pub(crate) struct Reach(Vec<bool>);

impl Reach {
    pub(crate) fn contains(&self, net: NetId) -> bool {
        self.0.get(net.0 as usize).copied().unwrap_or(false)
    }

    /// Whether any of the nets was reached.
    pub(crate) fn any(&self, nets: &[NetId]) -> bool {
        nets.iter().any(|n| self.contains(*n))
    }
}

/// What lies on the supply side of a net, see [`Nets::upstream`].
pub(crate) struct Upstream {
    pub(crate) nets: BTreeSet<NetId>,
    /// Symbols with a pole between the supply and the net, with the
    /// distance of their load side from the source.
    pub(crate) poles: HashMap<SymbolId, u32>,
}
