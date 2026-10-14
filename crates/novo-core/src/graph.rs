//! Connectivity: which endpoints are electrically one node.
//!
//! A [`Netlist`] is derived from the project, never stored. Two endpoints are
//! in the same net when they are joined by:
//! - a wire,
//! - a bridge inside a symbol (`SymbolInstance::bridges`),
//! - being pins of the same terminal (a terminal conducts end to end; each
//!   level of a multi-level terminal is its own terminal),
//! - a jumper between terminals of one strip (same `Terminal::jumper` group),
//! - interruption points with the same name.
//!
//! Contacts never join nets: whether a contact conducts is simulation state.
//! Potential definition points label the nets they touch, and a potential
//! carries on through switched poles (`SymbolInstance::pass_through`), so the
//! wire after a contactor or breaker still reads `L1`.

use std::collections::{BTreeMap, HashMap};

use crate::entity::{Endpoint, SymbolRole};
use crate::id::{DeviceId, PageId, PotentialId, SymbolId, WireId};
use crate::project::Project;

/// Index of a net within one [`Netlist`]. Only meaningful for the netlist
/// that produced it; nets are renumbered whenever the netlist is rebuilt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NetId(pub u32);

/// A set of endpoints that are electrically one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Net {
    pub id: NetId,
    /// Every pin and junction in the net, sorted.
    pub endpoints: Vec<Endpoint>,
    /// Wires inside the net, sorted.
    pub wires: Vec<WireId>,
    /// Potentials this net carries, sorted: those defined on it plus those
    /// passed through contacts, breakers and fuses. More than one means two
    /// potentials meet (a short, or one waiting behind an open contact).
    pub potentials: Vec<PotentialId>,
    /// Potentials defined by a potential definition point in this net itself.
    pub defined: Vec<PotentialId>,
}

impl Net {
    /// The net's potential when exactly one is assigned.
    pub fn potential(&self) -> Option<PotentialId> {
        match self.potentials.as_slice() {
            [single] => Some(*single),
            _ => None,
        }
    }
}

/// The connectivity graph of one project revision.
#[derive(Debug, Clone)]
pub struct Netlist {
    revision: u64,
    nets: Vec<Net>,
    by_endpoint: HashMap<Endpoint, NetId>,
}

/// One way across a switched pole: from `from_pin` (in the current net) to
/// `to_pin` (in net `to`) of pole `group` of `symbol`.
#[derive(Debug, Clone, Copy)]
struct PassEdge {
    symbol: SymbolId,
    group: usize,
    from_pin: u16,
    to_pin: u16,
    to: usize,
}

/// Nets a potential reaches from the nets that define it, walking outward
/// through switched poles. Having crossed a pole, the walk does not cross
/// back out of the pin it arrived on or through the same pole again, so the
/// break and make sides of a changeover contact stay apart (fed from the
/// common, the potential still reaches both), while poles wired in series
/// pass it on. Within one group every pin passes to every other, so a
/// changeover needs two groups (`[common, break]`, `[common, make]`), not one.
fn carried_nets(edges: &[Vec<PassEdge>], starts: &[usize]) -> Vec<usize> {
    type Arrival = Option<(SymbolId, usize, u16)>;
    let mut seen: std::collections::HashSet<(usize, Arrival)> = Default::default();
    let mut reached = vec![false; edges.len()];
    let mut stack: Vec<(usize, Arrival)> = starts.iter().map(|&n| (n, None)).collect();
    while let Some(state) = stack.pop() {
        if !seen.insert(state) {
            continue;
        }
        let (net, arrival) = state;
        reached[net] = true;
        for edge in &edges[net] {
            let blocked = arrival.is_some_and(|(symbol, group, pin)| {
                symbol == edge.symbol && (group == edge.group || pin == edge.from_pin)
            });
            if !blocked {
                stack.push((edge.to, Some((edge.symbol, edge.group, edge.to_pin))));
            }
        }
    }
    (0..edges.len()).filter(|&n| reached[n]).collect()
}

struct UnionFind {
    parent: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        UnionFind {
            parent: (0..n).collect(),
        }
    }

    fn find(&mut self, mut x: usize) -> usize {
        while self.parent[x] != x {
            self.parent[x] = self.parent[self.parent[x]];
            x = self.parent[x];
        }
        x
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            // Keep the smaller index as root so results do not depend on
            // union order.
            let (lo, hi) = if ra < rb { (ra, rb) } else { (rb, ra) };
            self.parent[hi] = lo;
        }
    }

    fn union_all(&mut self, nodes: impl IntoIterator<Item = usize>) {
        let mut nodes = nodes.into_iter();
        if let Some(first) = nodes.next() {
            for n in nodes {
                self.union(first, n);
            }
        }
    }
}

impl Netlist {
    /// Builds the netlist of a project. Linear in the size of the project.
    pub fn build(project: &Project) -> Netlist {
        // Number every endpoint: all pins in symbol order, then junctions.
        let mut endpoints: Vec<Endpoint> = Vec::new();
        let mut first_pin: HashMap<SymbolId, usize> = HashMap::new();
        for symbol in project.symbols() {
            first_pin.insert(symbol.id, endpoints.len());
            for pin in 0..symbol.pins.len() {
                let pin = u16::try_from(pin).expect("pin count fits u16");
                endpoints.push(Endpoint::pin(symbol.id, pin));
            }
        }
        for junction in project.junctions() {
            endpoints.push(Endpoint::junction(junction.id));
        }
        let index: HashMap<Endpoint, usize> =
            endpoints.iter().enumerate().map(|(i, &e)| (e, i)).collect();
        let pins_of = |symbol: SymbolId, count: usize| {
            let base = first_pin[&symbol];
            base..base + count
        };

        let mut uf = UnionFind::new(endpoints.len());
        for wire in project.wires() {
            uf.union(index[&wire.from], index[&wire.to]);
        }
        let mut by_terminal: HashMap<_, Vec<usize>> = HashMap::new();
        let mut by_interruption: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
        for symbol in project.symbols() {
            let base = first_pin[&symbol.id];
            for group in &symbol.bridges {
                uf.union_all(group.iter().map(|&p| base + usize::from(p)));
            }
            match &symbol.role {
                SymbolRole::Terminal { .. } => {
                    for pin in 0..symbol.pins.len() {
                        let pin16 = u16::try_from(pin).expect("pin count fits u16");
                        if let Some(terminal) = symbol.terminal_of_pin(pin16) {
                            by_terminal.entry(terminal).or_default().push(base + pin);
                        }
                    }
                }
                SymbolRole::Interruption { name } => {
                    by_interruption
                        .entry(name.as_str())
                        .or_default()
                        .extend(pins_of(symbol.id, symbol.pins.len()));
                }
                _ => {}
            }
        }
        for nodes in by_terminal.values().chain(by_interruption.values()) {
            uf.union_all(nodes.iter().copied());
        }
        // Jumpers: terminals of one strip sharing a jumper group.
        let mut jumpers: HashMap<(DeviceId, u32), Vec<usize>> = HashMap::new();
        for terminal in project.terminals() {
            if let (Some(group), Some(nodes)) = (terminal.jumper, by_terminal.get(&terminal.id))
                && let Some(&node) = nodes.first()
            {
                jumpers
                    .entry((terminal.strip, group))
                    .or_default()
                    .push(node);
            }
        }
        for nodes in jumpers.values() {
            uf.union_all(nodes.iter().copied());
        }

        // Collect nets in order of their first endpoint.
        let mut net_of_root: HashMap<usize, NetId> = HashMap::new();
        let mut nets: Vec<Net> = Vec::new();
        let mut by_endpoint = HashMap::with_capacity(endpoints.len());
        for (i, &end) in endpoints.iter().enumerate() {
            let root = uf.find(i);
            let id = *net_of_root.entry(root).or_insert_with(|| {
                let id = NetId(u32::try_from(nets.len()).expect("net count fits u32"));
                nets.push(Net {
                    id,
                    endpoints: Vec::new(),
                    wires: Vec::new(),
                    potentials: Vec::new(),
                    defined: Vec::new(),
                });
                id
            });
            nets[id.0 as usize].endpoints.push(end);
            by_endpoint.insert(end, id);
        }
        for wire in project.wires() {
            let net = by_endpoint[&wire.from];
            nets[net.0 as usize].wires.push(wire.id);
        }
        // Pass-through edges: a potential crosses a switched pole from any
        // of its pins to the others, without joining the nets.
        let mut edges: Vec<Vec<PassEdge>> = vec![Vec::new(); nets.len()];
        for symbol in project.symbols() {
            let base = first_pin[&symbol.id];
            let net_of_pin = |p: u16| by_endpoint[&endpoints[base + usize::from(p)]].0 as usize;
            for (group, pins) in symbol.pass_through.iter().enumerate() {
                for &from_pin in pins {
                    for &to_pin in pins.iter().filter(|&&p| p != from_pin) {
                        edges[net_of_pin(from_pin)].push(PassEdge {
                            symbol: symbol.id,
                            group,
                            from_pin,
                            to_pin,
                            to: net_of_pin(to_pin),
                        });
                    }
                }
            }
        }
        for symbol in project.symbols() {
            if let SymbolRole::PotentialFeed { potential } = symbol.role {
                for node in pins_of(symbol.id, symbol.pins.len()) {
                    let net = by_endpoint[&endpoints[node]].0 as usize;
                    nets[net].defined.push(potential);
                }
            }
        }
        let feeds: BTreeMap<PotentialId, Vec<usize>> =
            nets.iter()
                .enumerate()
                .fold(BTreeMap::new(), |mut feeds, (i, net)| {
                    for &potential in &net.defined {
                        feeds.entry(potential).or_default().push(i);
                    }
                    feeds
                });
        for (potential, starts) in feeds {
            for net in carried_nets(&edges, &starts) {
                nets[net].potentials.push(potential);
            }
        }
        for net in &mut nets {
            net.endpoints.sort();
            net.wires.sort();
            net.defined.sort();
            net.defined.dedup();
            net.potentials.sort();
            net.potentials.dedup();
        }
        Netlist {
            revision: project.revision(),
            nets,
            by_endpoint,
        }
    }

    /// The project revision this netlist was built from.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether this netlist still matches the project.
    pub fn is_current(&self, project: &Project) -> bool {
        self.revision == project.revision()
    }

    pub fn nets(&self) -> &[Net] {
        &self.nets
    }

    pub fn net(&self, id: NetId) -> &Net {
        &self.nets[id.0 as usize]
    }

    /// The net an endpoint belongs to.
    pub fn net_of(&self, end: impl Into<Endpoint>) -> Option<NetId> {
        self.by_endpoint.get(&end.into()).copied()
    }

    /// Whether two endpoints are electrically connected.
    pub fn connected(&self, a: impl Into<Endpoint>, b: impl Into<Endpoint>) -> bool {
        match (self.net_of(a), self.net_of(b)) {
            (Some(x), Some(y)) => x == y,
            _ => false,
        }
    }

    /// The potential of the net an endpoint is in, when exactly one is assigned.
    pub fn potential_of(&self, end: impl Into<Endpoint>) -> Option<PotentialId> {
        self.net_of(end).and_then(|n| self.net(n).potential())
    }

    /// Nets carrying a potential.
    pub fn nets_with_potential(&self, potential: PotentialId) -> impl Iterator<Item = &Net> {
        self.nets
            .iter()
            .filter(move |n| n.potentials.contains(&potential))
    }

    /// Nets where two or more potentials meet (a short between them).
    pub fn potential_conflicts(&self) -> impl Iterator<Item = &Net> {
        self.nets.iter().filter(|n| n.potentials.len() > 1)
    }

    /// Devices with a pin in the net, sorted, each once.
    pub fn devices_on_net(&self, project: &Project, net: NetId) -> Vec<DeviceId> {
        let mut devices: Vec<DeviceId> = self
            .net(net)
            .endpoints
            .iter()
            .filter_map(|e| e.as_pin())
            .filter_map(|p| project.symbol(p.symbol)?.device())
            .collect();
        devices.sort();
        devices.dedup();
        devices
    }

    /// Pages the net appears on, sorted, each once.
    pub fn pages_of_net(&self, project: &Project, net: NetId) -> Vec<PageId> {
        let mut pages: Vec<PageId> = self
            .net(net)
            .endpoints
            .iter()
            .filter_map(|&e| match e {
                Endpoint::Pin { symbol, .. } => project.symbol(symbol).map(|s| s.page),
                Endpoint::Junction { junction } => project.junction(junction).map(|j| j.page),
            })
            .collect();
        pages.sort();
        pages.dedup();
        pages
    }
}
