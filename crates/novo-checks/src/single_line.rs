//! The single-line diagram model: the power distribution of a project as a
//! tree from the supply to its loads, with the numbers on every branch.
//!
//! Nothing is drawn by hand. The tree is derived from what the schematic
//! already says:
//!
//! - The **supply** is the set of nets that define a potential (potential
//!   feeds, supply terminals). Its voltage and fault level are the project's
//!   `supply.*` inputs.
//! - A **protective or switching device** is a symbol with declared switched
//!   poles (`pass_through`): fuse, circuit breaker, disconnector, contactor,
//!   motor protection breaker. Its line side is the net nearer the supply,
//!   its load side the nets farther away. A device whose poles join one net
//!   (an overload relay drawn with bridges) sits *in line* on a run and is
//!   listed on the cable it is part of.
//! - A **load** is a motor, heater, drive, soft starter, transformer or power
//!   supply. A drive, soft starter, transformer or power supply also feeds
//!   whatever is wired to its output (a motor behind a drive).
//! - A device is the child of the device whose load side shares its line-side
//!   net. A device with no such parent hangs on the supply.
//! - The **cable** between two boxes is the wires on the path between their
//!   pins in the netlist (through terminals, bridges and junctions): the
//!   cross-section, the length and the cable they belong to.
//!
//! # What is stated, calculated, proposed or assumed
//!
//! Every number carries an [`Origin`]. *Stated* is a property or cross-section
//! somebody entered. *Calculated* follows from stated inputs by a formula
//! (a motor current from its power, the sum of the currents below a device,
//! a voltage drop, a cable rating). *Proposed* is a cross-section the
//! Autopilot suggests for a wire that has none; it is not applied. *Assumed*
//! is a default the drawing does not state (power factor 1, installation
//! method C, 30 °C). Missing inputs stay missing and are named; nothing is
//! defaulted into a pass.
//!
//! The ampacity tables are planning values until the user loads their
//! licensed copy ([`TableSet::disclosure`]); the clause numbers in
//! [`crate::calc_view::clause`] are not verified against the standard text.
//!
//! The short-circuit current at each box is the stated supply fault level
//! through the cable impedance of every run on the way ([`Node::ik_ka`],
//! three-phase maximum, conductors at 20 °C); it is a ceiling where a run has
//! no data, absent past a converter, and a device whose rated breaking
//! capacity is below it fails.
//!
//! What it leaves out, and says so in [`SingleLine::notes`]: diversity (the
//! current of a distribution device is the arithmetic sum of what hangs on
//! it), the reactive part of the voltage drop when no power factor is
//! stated, impedance upstream of the first device, and single-phase
//! circuits whose only load is a symbol that is not a motor, heater, drive,
//! transformer or power supply.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use novo_core::{
    DeviceId, Endpoint, NetId, Netlist, PotentialKind, Project, SymbolId, SymbolInstance,
    SymbolRole, TerminalId, WireId, natural_cmp,
};

use crate::calc::Insulation;
use crate::calc::cable::{Circuit, Loaded, Method};
use crate::calc::circuit::Status;
use crate::calc::drop::{Run, Supply};
use crate::calc::motor::full_load_current;
use crate::calc::short_circuit::{Impedance, Source, cable_impedance, ik3_max};
use crate::calc::starter::{typical_cos_phi, typical_efficiency};
use crate::calc::tables::TableSet;
use crate::calc_checks::{
    AMBIENT_C, BREAKING_CAPACITY_KA, GROUPED_CIRCUITS, LOAD_CURRENT_A, TRIP_CURVE, device_number,
    device_text, installation_method_text, number, supply_voltage, wire_insulation, wire_length,
    wire_number, wire_section,
};
use crate::calc_view::{self, StarterChoice};
use crate::standards::keys;
use crate::standards::netclass::{Circuit as NetCircuit, Nets};
use crate::standards::symbols::{Class, class_of};

/// How a number came to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// Entered by somebody: a property, a cross-section, a rating.
    Stated,
    /// Worked out from stated inputs by a formula.
    Calculated,
    /// Suggested by the Autopilot for a wire that has none; not applied.
    Proposed,
    /// A default, because the drawing does not say.
    Assumed,
    /// Not known.
    Missing,
}

impl Origin {
    /// The word printed beside a number.
    pub fn label(self) -> &'static str {
        match self {
            Origin::Stated => "stated",
            Origin::Calculated => "calculated",
            Origin::Proposed => "proposed",
            Origin::Assumed => "assumed",
            Origin::Missing => "missing",
        }
    }
}

/// A number and where it came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Figure {
    pub value: Option<f64>,
    pub origin: Origin,
}

impl Figure {
    pub const MISSING: Figure = Figure {
        value: None,
        origin: Origin::Missing,
    };

    pub fn new(value: f64, origin: Origin) -> Figure {
        Figure {
            value: Some(value),
            origin,
        }
    }

    pub fn stated(value: f64) -> Figure {
        Figure::new(value, Origin::Stated)
    }

    pub fn calculated(value: f64) -> Figure {
        Figure::new(value, Origin::Calculated)
    }

    pub fn is_missing(&self) -> bool {
        self.value.is_none()
    }
}

/// What a box on the diagram is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Supply,
    Disconnector,
    FuseSwitch,
    Breaker,
    Fuse,
    MotorProtection,
    MotorCircuitProtector,
    Rcd,
    Contactor,
    OverloadRelay,
    Motor,
    Drive,
    SoftStarter,
    Heater,
    Lamp,
    Sounder,
    Transformer,
    PowerSupply,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Supply => "Supply",
            Kind::Disconnector => "Disconnector",
            Kind::FuseSwitch => "Fuse switch",
            Kind::Breaker => "Circuit breaker",
            Kind::Fuse => "Fuse",
            Kind::MotorProtection => "Motor protection breaker",
            Kind::MotorCircuitProtector => "Motor circuit protector",
            Kind::Rcd => "RCD",
            Kind::Contactor => "Contactor",
            Kind::OverloadRelay => "Overload relay",
            Kind::Motor => "Motor",
            Kind::Drive => "Drive",
            Kind::SoftStarter => "Soft starter",
            Kind::Heater => "Heater",
            Kind::Lamp => "Lamp",
            Kind::Sounder => "Sounder",
            Kind::Transformer => "Transformer",
            Kind::PowerSupply => "Power supply",
        }
    }

    /// A short name for a box on the drawing.
    pub fn short_label(self) -> &'static str {
        match self {
            Kind::Breaker => "Breaker",
            Kind::MotorProtection => "Motor breaker",
            Kind::MotorCircuitProtector => "Motor protector",
            other => other.label(),
        }
    }

    /// Devices that interrupt a short circuit: the ones a cable is
    /// protected by.
    pub fn is_protective(self) -> bool {
        matches!(
            self,
            Kind::FuseSwitch
                | Kind::Breaker
                | Kind::Fuse
                | Kind::MotorProtection
                | Kind::MotorCircuitProtector
        )
    }

    /// Ends a branch: draws power and turns it into work or another supply.
    pub fn is_load(self) -> bool {
        matches!(
            self,
            Kind::Motor
                | Kind::Drive
                | Kind::SoftStarter
                | Kind::Heater
                | Kind::Lamp
                | Kind::Sounder
                | Kind::Transformer
                | Kind::PowerSupply
        )
    }

    /// Signal devices, drawn only where they hang on a mains-level net (see
    /// [`mains_signal_nets`]).
    fn is_signal(self) -> bool {
        matches!(self, Kind::Lamp | Kind::Sounder)
    }

    /// A load that also feeds what is wired to its output.
    fn is_converter(self) -> bool {
        matches!(
            self,
            Kind::Drive | Kind::SoftStarter | Kind::Transformer | Kind::PowerSupply
        )
    }

    fn of_switching(class: Class) -> Option<Kind> {
        Some(match class {
            Class::Disconnector => Kind::Disconnector,
            Class::FuseSwitch => Kind::FuseSwitch,
            Class::Breaker => Kind::Breaker,
            Class::Fuse => Kind::Fuse,
            Class::MotorProtectionBreaker => Kind::MotorProtection,
            Class::MotorCircuitProtector => Kind::MotorCircuitProtector,
            Class::Rcd => Kind::Rcd,
            Class::Contactor => Kind::Contactor,
            Class::OverloadRelay => Kind::OverloadRelay,
            _ => return None,
        })
    }

    fn of_load(class: Class) -> Option<Kind> {
        Some(match class {
            Class::Motor => Kind::Motor,
            Class::Drive => Kind::Drive,
            Class::SoftStarter => Kind::SoftStarter,
            Class::Heater => Kind::Heater,
            Class::Lamp => Kind::Lamp,
            Class::Sounder => Kind::Sounder,
            Class::Transformer => Kind::Transformer,
            Class::PowerSupply => Kind::PowerSupply,
            _ => return None,
        })
    }
}

/// One wire of a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub wire: WireId,
    /// `wire 12`, or `unnumbered wire`.
    pub label: String,
    /// The cable the wire is a core of (`=A1+FLD-W1`).
    pub cable: Option<String>,
    pub section_mm2: Figure,
    pub length_m: Figure,
    pub insulation: Insulation,
    /// The drop over this wire at the run's current, percent of the nominal
    /// voltage. Missing while the length or the cross-section is.
    pub drop_percent: Figure,
}

/// The cable between a box and its parent.
#[derive(Debug, Clone, PartialEq)]
pub struct Link {
    pub segments: Vec<Segment>,
    /// Conductors in parallel that carry the load (3 for a three-phase run).
    pub poles: usize,
    /// The nets of the run carry a potential voltage.
    pub voltage_v: Figure,
    pub three_phase: bool,
    /// Sum of the stated lengths of the segments, m.
    pub length_m: Figure,
    /// The thinnest segment's cross-section, mm².
    pub section_mm2: Figure,
    pub insulation: Insulation,
    pub method: Method,
    pub method_origin: Origin,
    /// What `Iz` assumed because the wire does not state it: the
    /// installation method, the ambient temperature, the grouping.
    pub iz_defaults: Vec<&'static str>,
    /// The voltage drop used power factor 1 because no wire of the run
    /// states one.
    pub drop_pf_assumed: bool,
    /// Current the run carries, A (the child's current).
    pub current_a: Figure,
    pub power_factor: Figure,
    /// Drop over the whole run, percent of the nominal voltage. Segments
    /// without data are left out and [`Link::complete`] is false.
    pub drop_percent: Figure,
    /// Drop from the supply to the end of this run, percent.
    pub drop_total_percent: Figure,
    /// Whether every run from the supply to here is complete.
    pub total_complete: bool,
    /// Current-carrying capacity `Iz` of the weakest wire in these
    /// conditions, A.
    pub iz_a: Figure,
    /// The protective device above this run and its rating, A.
    pub protected_by: Option<(String, f64)>,
    /// Whether every wire of the run has a length and a cross-section.
    pub complete: bool,
    /// `false` when no path between the two boxes could be followed and
    /// every wire of the net is listed instead.
    pub traced: bool,
    /// Devices wired into the run (an overload relay drawn with bridges).
    pub inline: Vec<String>,
    /// Overload verdict `Ib ≤ In ≤ Iz` and the drop limit.
    pub status: Status,
    /// Why the status is what it is, one sentence each.
    pub reasons: Vec<String>,
    /// The failing checks in a few characters each (`Ib > Iz`), for the
    /// drawing.
    pub fail_codes: Vec<&'static str>,
}

/// What the Calculations window says about a protective device.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    pub status: Status,
    /// Titles of the rows that fail.
    pub failing: Vec<String>,
    /// `status` without the "Breaking capacity" row.
    pub status_without_breaking: Status,
    /// The Calculations sheet fails Icu against the level at the device
    /// from the supply data alone. Where the diagram has an Ik along the run
    /// it judges Icu against that instead (`icu_status`) and the row is
    /// taken out of `status` and `failing`.
    pub breaking_row_failed: bool,
    /// Maximum three-phase short-circuit current at the device, kA.
    pub ik_max_ka: Option<f64>,
    /// Inputs the calculation lacks, named by property.
    pub missing: Vec<String>,
}

/// One box of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: usize,
    pub kind: Kind,
    pub device: Option<DeviceId>,
    /// `=A1+CAB-F1`, or the supply's name.
    pub tag: String,
    pub description: String,
    pub parent: Option<usize>,
    pub children: Vec<usize>,
    /// Rated current `In` of a protective device (or the top of an
    /// adjustable range), the nameplate current of a load, A.
    pub rated_a: Figure,
    /// Trip characteristic (`C`, `gG`); the origin says whether it is
    /// stated or the library's usual fuse class.
    pub characteristic: Option<(String, Origin)>,
    pub breaking_ka: Figure,
    /// Rated power of a load, kW.
    pub power_kw: Figure,
    /// The current through the box: stated on the incoming wire or on the
    /// load, calculated from power, or the sum of what hangs below it, A.
    pub current_a: Figure,
    /// What the current rests on, one phrase.
    pub current_basis: String,
    /// Loads below this box without a current (their current is left out of
    /// the sum).
    pub loads_without_current: usize,
    pub link: Option<Link>,
    pub verdict: Option<Verdict>,
    /// Maximum three-phase short-circuit current at the box's incoming
    /// terminals, kA: the supply's fault level through the cables of every
    /// run on the way (IEC 60909-0, low-voltage simplification, resistance at
    /// 20 °C). Missing past a transformer or power supply and on runs that are
    /// not three-phase.
    pub ik_ka: Figure,
    /// Whether every run from the supply has a length and a cross-section.
    /// When false, `ik_ka` is a ceiling: the cable impedance of the runs
    /// without data is left out.
    pub ik_exact: bool,
    /// The box cannot break the current there: `Fail` when the figure is
    /// exact and above the rated breaking capacity, `Warning` when only the
    /// ceiling is above it.
    pub icu_status: Status,
    /// Hops from the supply.
    pub depth: usize,
}

impl Node {
    /// Whether the box or the run into it fails a check.
    pub fn fails(&self) -> bool {
        self.link.as_ref().is_some_and(|l| l.status == Status::Fail)
            || self
                .verdict
                .as_ref()
                .is_some_and(|v| v.status == Status::Fail)
            || self.icu_status == Status::Fail
    }
}

/// The whole diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct SingleLine {
    /// Box 0 is the supply; the rest follow depth first, siblings in tag
    /// order.
    pub nodes: Vec<Node>,
    pub voltage_v: f64,
    /// Fault level at the supply, kA, and where it came from.
    pub supply_ik_ka: Figure,
    pub drop_limit_percent: f64,
    pub drop_limit_stated: bool,
    /// One line on the supply (`400 V, 10.0 kA ...`).
    pub supply_text: Option<String>,
    /// What the diagram leaves out or could not place, one sentence each.
    pub notes: Vec<String>,
    /// Inputs that are missing, named by property.
    pub missing: Vec<String>,
    /// Devices with power poles that no supply reaches.
    pub unfed: Vec<String>,
    pub tables: String,
    pub planning_values: bool,
}

impl SingleLine {
    /// The boxes under `node`, in order.
    pub fn children(&self, node: usize) -> &[usize] {
        &self.nodes[node].children
    }

    /// Boxes that end a branch.
    pub fn leaves(&self) -> usize {
        self.nodes.iter().filter(|n| n.children.is_empty()).count()
    }

    /// The deepest level (the supply is level 0).
    pub fn depth(&self) -> usize {
        self.nodes.iter().map(|n| n.depth).max().unwrap_or(0)
    }

    /// Boxes or runs that fail.
    pub fn failing(&self) -> usize {
        self.nodes.iter().filter(|n| n.fails()).count()
    }

    /// Whether there is anything below the supply.
    pub fn is_empty(&self) -> bool {
        self.nodes.len() <= 1
    }
}

// ---------------------------------------------------------------------------
// Building the tree

/// A switching or load device with the nets on its two sides.
struct Item {
    device: DeviceId,
    kind: Kind,
    symbols: Vec<SymbolId>,
    line: BTreeMap<NetId, Vec<Endpoint>>,
    load: BTreeMap<NetId, Vec<Endpoint>>,
    line_dist: u32,
}

fn is_ground_pin(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "PE" | "PEN" | "N" | "GND" | "EARTH"
    )
}

fn is_live(nets: &Nets<'_>, signal: &BTreeSet<NetId>, net: NetId) -> bool {
    signal.contains(&net)
        || !matches!(
            nets.circuit(net),
            NetCircuit::ProtectiveEarth | NetCircuit::Neutral | NetCircuit::Control
        )
}

/// Below this a Line potential is extra-low voltage, not mains.
const MAINS_MIN_V: f64 = 100.0;

/// Nets a stated AC line voltage of mains level reaches over the switched
/// poles of the drawing (a converter output is not reached: its pins are not
/// switched poles).
fn mains_reach(project: &Project, netlist: &Netlist) -> BTreeSet<NetId> {
    let mut reached: BTreeSet<NetId> = netlist
        .nets()
        .iter()
        .filter(|net| {
            net.potentials
                .iter()
                .filter_map(|p| project.potential(*p))
                .any(|p| {
                    p.kind == PotentialKind::Line && p.voltage.is_some_and(|v| v >= MAINS_MIN_V)
                })
        })
        .map(|net| net.id)
        .collect();
    let mut edges: Vec<(NetId, NetId)> = Vec::new();
    for symbol in project.symbols() {
        for group in &symbol.pass_through {
            let nets: Vec<NetId> = group
                .iter()
                .filter_map(|&p| netlist.net_of(Endpoint::pin(symbol.id, p)))
                .collect();
            for pair in nets.windows(2) {
                if pair[0] != pair[1] {
                    edges.push((pair[0], pair[1]));
                }
            }
        }
    }
    loop {
        let before = reached.len();
        for (a, b) in &edges {
            if reached.contains(a) {
                reached.insert(*b);
            }
            if reached.contains(b) {
                reached.insert(*a);
            }
        }
        if reached.len() == before {
            return reached;
        }
    }
}

/// Control-class nets with a lamp or sounder on them that mains reaches. A
/// lamp or sounder on any other net (24 V, or no mains voltage stated
/// upstream) is a control device and stays off the distribution diagram.
/// Contactor and relay coils are never drawn, wherever they hang.
fn mains_signal_nets(project: &Project, netlist: &Netlist, nets: &Nets<'_>) -> BTreeSet<NetId> {
    let mains = mains_reach(project, netlist);
    let mut out = BTreeSet::new();
    for net in netlist.nets() {
        if nets.circuit(net.id) != NetCircuit::Control || !mains.contains(&net.id) {
            continue;
        }
        let signal = net.endpoints.iter().any(|e| {
            let Endpoint::Pin { symbol, .. } = e else {
                return false;
            };
            project
                .symbol(*symbol)
                .is_some_and(|s| Kind::of_load(class_of(s)).is_some_and(Kind::is_signal))
        });
        if signal {
            out.insert(net.id);
        }
    }
    out
}

/// The device a power symbol belongs to.
fn device_of(symbol: &SymbolInstance) -> Option<DeviceId> {
    match symbol.role {
        SymbolRole::Device { device, .. } => Some(device),
        _ => None,
    }
}

fn tag_of(project: &Project, device: DeviceId) -> String {
    project
        .device(device)
        .map(|d| d.tag.to_string())
        .unwrap_or_default()
}

fn live_pins(
    netlist: &Netlist,
    nets: &Nets<'_>,
    signal: &BTreeSet<NetId>,
    symbol: &SymbolInstance,
) -> Vec<(NetId, Endpoint)> {
    let mut out = Vec::new();
    for (i, pin) in symbol.pins.iter().enumerate() {
        let Ok(i) = u16::try_from(i) else { break };
        if is_ground_pin(&pin.name) {
            continue;
        }
        let end = Endpoint::pin(symbol.id, i);
        let Some(net) = netlist.net_of(end) else {
            continue;
        };
        if is_live(nets, signal, net) {
            out.push((net, end));
        }
    }
    out
}

fn push_unique(map: &mut BTreeMap<NetId, Vec<Endpoint>>, net: NetId, end: Endpoint) {
    let list = map.entry(net).or_default();
    if !list.contains(&end) {
        list.push(end);
    }
}

/// Everything the tree is assembled from.
struct Collected {
    items: Vec<Item>,
    /// Devices wired into a run, by symbol.
    inline: HashMap<SymbolId, (DeviceId, Kind)>,
    unfed: BTreeSet<DeviceId>,
    /// Lamps and sounders on a control circuit: not part of the diagram.
    signals_left_out: BTreeSet<DeviceId>,
}

fn collect(project: &Project, netlist: &Netlist, nets: &Nets<'_>) -> Collected {
    let n = netlist.nets().len();
    let signal = mains_signal_nets(project, netlist, nets);
    let mut signals_left_out: BTreeSet<DeviceId> = BTreeSet::new();
    let basic: Vec<Option<u32>> = (0..n).map(|i| nets.distance(NetId(i as u32))).collect();
    let mut dist = basic.clone();
    let dist_of = |dist: &[Option<u32>], net: NetId| dist[net.0 as usize];

    let mut items: BTreeMap<DeviceId, Item> = BTreeMap::new();
    let mut loads_seen: BTreeSet<DeviceId> = BTreeSet::new();
    let mut unfed: BTreeSet<DeviceId> = BTreeSet::new();

    // Loads. Converters first as their output extends the distances.
    let mut pending: Vec<(&SymbolInstance, DeviceId, Kind)> = project
        .symbols()
        .filter_map(|s| {
            let kind = Kind::of_load(class_of(s))?;
            let device = device_of(s)?;
            if kind.is_signal() {
                let on_mains = s.pins.iter().enumerate().any(|(i, _)| {
                    u16::try_from(i)
                        .ok()
                        .and_then(|i| netlist.net_of(Endpoint::pin(s.id, i)))
                        .is_some_and(|n| signal.contains(&n) || is_live(nets, &signal, n))
                });
                if !on_mains {
                    signals_left_out.insert(device);
                    return None;
                }
            }
            Some((s, device, kind))
        })
        .collect();
    pending.sort_by_key(|(s, d, _)| (*d, s.id));
    // A converter's input is the pins the supply reaches; rounds let a drive
    // behind a transformer find its input once the transformer's output is
    // known.
    for _round in 0..4 {
        let mut extra: Vec<(NetId, u32)> = Vec::new();
        let mut progressed = false;
        for (symbol, device, kind) in &pending {
            if !kind.is_converter() {
                continue;
            }
            let live = live_pins(netlist, nets, &signal, symbol);
            let fed: Vec<(NetId, Endpoint, u32)> = live
                .iter()
                .filter_map(|(net, end)| Some((*net, *end, dist_of(&dist, *net)?)))
                .collect();
            let Some(line_dist) = fed.iter().map(|(_, _, d)| *d).max() else {
                continue;
            };
            let item = items.entry(*device).or_insert_with(|| Item {
                device: *device,
                kind: *kind,
                symbols: Vec::new(),
                line: BTreeMap::new(),
                load: BTreeMap::new(),
                line_dist,
            });
            if item.symbols.contains(&symbol.id) {
                continue;
            }
            item.symbols.push(symbol.id);
            item.line_dist = item.line_dist.max(line_dist);
            for (net, end, d) in &fed {
                if *d == line_dist {
                    push_unique(&mut item.line, *net, *end);
                }
            }
            for (net, end) in &live {
                if !item.line.contains_key(net) {
                    push_unique(&mut item.load, *net, *end);
                    if dist_of(&dist, *net).is_none() {
                        extra.push((*net, line_dist + 1));
                    }
                }
            }
            progressed = true;
        }
        if !progressed {
            break;
        }
        dist = nets.distances_with(&extra);
        // `distances_with` starts from the basic distances: keep earlier
        // extensions.
        for (i, d) in basic.iter().enumerate() {
            if dist[i].is_none() {
                dist[i] = *d;
            }
        }
    }

    // Plain loads, with the extended distances.
    for (symbol, device, kind) in &pending {
        if kind.is_converter() {
            continue;
        }
        let live = live_pins(netlist, nets, &signal, symbol);
        let fed: Vec<(NetId, Endpoint, u32)> = live
            .iter()
            .filter_map(|(net, end)| Some((*net, *end, dist_of(&dist, *net)?)))
            .collect();
        let Some(line_dist) = fed.iter().map(|(_, _, d)| *d).max() else {
            unfed.insert(*device);
            continue;
        };
        let item = items.entry(*device).or_insert_with(|| Item {
            device: *device,
            kind: *kind,
            symbols: Vec::new(),
            line: BTreeMap::new(),
            load: BTreeMap::new(),
            line_dist,
        });
        item.symbols.push(symbol.id);
        item.line_dist = item.line_dist.max(line_dist);
        for (net, end, d) in &fed {
            if *d == line_dist {
                push_unique(&mut item.line, *net, *end);
            }
        }
        loads_seen.insert(*device);
    }

    // Switching devices.
    let mut inline: HashMap<SymbolId, (DeviceId, Kind)> = HashMap::new();
    for symbol in project.symbols() {
        let Some(device) = device_of(symbol) else {
            continue;
        };
        let Some(kind) = Kind::of_switching(class_of(symbol)) else {
            continue;
        };
        let mut any_power = false;
        let mut line: BTreeMap<NetId, Vec<Endpoint>> = BTreeMap::new();
        let mut load: BTreeMap<NetId, Vec<Endpoint>> = BTreeMap::new();
        let mut line_dist: Option<u32> = None;
        let mut inline_here = false;
        for group in &symbol.pass_through {
            let pins: Vec<(NetId, Endpoint)> = group
                .iter()
                .filter_map(|&p| {
                    let end = Endpoint::pin(symbol.id, p);
                    Some((netlist.net_of(end)?, end))
                })
                .filter(|(net, _)| is_live(nets, &signal, *net))
                .collect();
            let distinct: BTreeSet<NetId> = pins.iter().map(|(n, _)| *n).collect();
            if distinct.is_empty() {
                continue;
            }
            // A switching device on a control circuit is not on the diagram.
            if !pins
                .iter()
                .any(|(n, _)| nets.circuit(*n) == NetCircuit::Power)
            {
                continue;
            }
            any_power = true;
            if distinct.len() == 1 {
                inline_here = true;
                continue;
            }
            let Some(min) = pins.iter().filter_map(|(n, _)| dist_of(&dist, *n)).min() else {
                continue;
            };
            line_dist = Some(line_dist.map_or(min, |d| d.min(min)));
            for (net, end) in pins {
                if dist_of(&dist, net) == Some(min) {
                    push_unique(&mut line, net, end);
                } else {
                    push_unique(&mut load, net, end);
                }
            }
        }
        if symbol.pass_through.is_empty() {
            // Poles joined by bridges only: the device sits in a run.
            let on_power = (0..symbol.pins.len()).any(|p| {
                u16::try_from(p)
                    .ok()
                    .and_then(|p| netlist.net_of(Endpoint::pin(symbol.id, p)))
                    .is_some_and(|n| nets.circuit(n) == NetCircuit::Power)
            });
            if on_power {
                inline.insert(symbol.id, (device, kind));
            }
            continue;
        }
        if !any_power {
            continue;
        }
        if line.is_empty() {
            if inline_here {
                inline.insert(symbol.id, (device, kind));
            } else {
                unfed.insert(device);
            }
            continue;
        }
        let item = items.entry(device).or_insert_with(|| Item {
            device,
            kind,
            symbols: Vec::new(),
            line: BTreeMap::new(),
            load: BTreeMap::new(),
            line_dist: line_dist.unwrap_or(0),
        });
        item.symbols.push(symbol.id);
        item.line_dist = item.line_dist.min(line_dist.unwrap_or(0));
        for (net, ends) in line {
            for end in ends {
                push_unique(&mut item.line, net, end);
            }
        }
        for (net, ends) in load {
            for end in ends {
                push_unique(&mut item.load, net, end);
            }
        }
    }
    for item in items.values() {
        unfed.remove(&item.device);
    }
    Collected {
        items: items.into_values().collect(),
        inline,
        unfed,
        signals_left_out,
    }
}

// ---------------------------------------------------------------------------
// Runs between boxes

struct Route {
    wires: Vec<WireId>,
    via: BTreeSet<SymbolId>,
    traced: bool,
}

#[derive(PartialEq, Eq, Hash)]
enum Join {
    Bridge(SymbolId, usize),
    Terminal(TerminalId),
    Interruption(String),
}

/// The wires on the way from `from` to `to` inside one net, crossing
/// bridges, terminals and interruption points at no cost. Falls back to
/// every wire of the net when no way is found.
fn route(
    project: &Project,
    netlist: &Netlist,
    net: NetId,
    from: &[Endpoint],
    to: &[Endpoint],
) -> Route {
    let net = netlist.net(net);
    let index: HashMap<Endpoint, usize> = net
        .endpoints
        .iter()
        .enumerate()
        .map(|(i, e)| (*e, i))
        .collect();
    let symbols_of = |ends: &mut dyn Iterator<Item = usize>| -> BTreeSet<SymbolId> {
        ends.filter_map(|i| match net.endpoints[i] {
            Endpoint::Pin { symbol, .. } => Some(symbol),
            _ => None,
        })
        .collect()
    };
    if from.iter().any(|e| to.contains(e)) {
        return Route {
            wires: Vec::new(),
            via: BTreeSet::new(),
            traced: true,
        };
    }
    let mut edges: Vec<Vec<(usize, Option<WireId>)>> = vec![Vec::new(); net.endpoints.len()];
    for id in &net.wires {
        let Some(wire) = project.wire(*id) else {
            continue;
        };
        if let (Some(&a), Some(&b)) = (index.get(&wire.from), index.get(&wire.to)) {
            edges[a].push((b, Some(*id)));
            edges[b].push((a, Some(*id)));
        }
    }
    let mut joins: HashMap<Join, Vec<usize>> = HashMap::new();
    for (i, end) in net.endpoints.iter().enumerate() {
        let Endpoint::Pin { symbol, pin } = end else {
            continue;
        };
        let Some(s) = project.symbol(*symbol) else {
            continue;
        };
        for (g, group) in s.bridges.iter().enumerate() {
            if group.contains(pin) {
                joins.entry(Join::Bridge(s.id, g)).or_default().push(i);
            }
        }
        if let Some(t) = s.terminal_of_pin(*pin) {
            joins.entry(Join::Terminal(t)).or_default().push(i);
        }
        if let SymbolRole::Interruption { name } = &s.role {
            joins
                .entry(Join::Interruption(name.clone()))
                .or_default()
                .push(i);
        }
    }
    for members in joins.values() {
        for pair in members.windows(2) {
            edges[pair[0]].push((pair[1], None));
            edges[pair[1]].push((pair[0], None));
        }
    }

    // 0-1 breadth first search: wires cost one, joins nothing.
    let starts: Vec<usize> = from.iter().filter_map(|e| index.get(e).copied()).collect();
    let goals: BTreeSet<usize> = to.iter().filter_map(|e| index.get(e).copied()).collect();
    let mut cost = vec![u32::MAX; edges.len()];
    let mut prev: Vec<Option<(usize, Option<WireId>)>> = vec![None; edges.len()];
    let mut queue: VecDeque<usize> = VecDeque::new();
    for &s in &starts {
        cost[s] = 0;
        queue.push_back(s);
    }
    while let Some(a) = queue.pop_front() {
        for &(b, wire) in &edges[a] {
            let c = cost[a] + u32::from(wire.is_some());
            if c < cost[b] {
                cost[b] = c;
                prev[b] = Some((a, wire));
                if wire.is_some() {
                    queue.push_back(b);
                } else {
                    queue.push_front(b);
                }
            }
        }
    }
    let best = goals
        .iter()
        .copied()
        .filter(|g| cost[*g] != u32::MAX)
        .min_by_key(|g| (cost[*g], *g));
    let Some(goal) = best else {
        return Route {
            wires: net.wires.clone(),
            via: symbols_of(&mut (0..net.endpoints.len())),
            traced: false,
        };
    };
    let mut wires = Vec::new();
    let mut walked = vec![goal];
    let mut at = goal;
    while let Some((p, wire)) = prev[at] {
        if let Some(w) = wire {
            wires.push(w);
        }
        walked.push(p);
        at = p;
    }
    wires.reverse();
    Route {
        wires,
        via: symbols_of(&mut walked.into_iter()),
        traced: true,
    }
}

// ---------------------------------------------------------------------------
// Numbers

struct Inputs<'a> {
    project: &'a Project,
    netlist: &'a Netlist,
    tables: &'a TableSet,
    proposed: &'a BTreeMap<WireId, f64>,
    voltage: f64,
}

struct RunFacts {
    voltage: Figure,
    three_phase: bool,
    /// One line conductor: the voltage is the stated line voltage over √3.
    phase_to_neutral: bool,
}

fn run_facts(inputs: &Inputs<'_>, net: NetId, poles: usize) -> RunFacts {
    let potential = inputs
        .netlist
        .net(net)
        .potential()
        .and_then(|p| inputs.project.potential(p));
    match potential {
        Some(p) if p.voltage.is_some_and(|v| v > 0.0) => {
            let v = p.voltage.unwrap_or(inputs.voltage);
            let line = p.kind == PotentialKind::Line && v >= 380.0;
            if line && poles == 1 {
                return RunFacts {
                    voltage: Figure::calculated(v / 3.0_f64.sqrt()),
                    three_phase: false,
                    phase_to_neutral: true,
                };
            }
            RunFacts {
                voltage: Figure::stated(v),
                three_phase: line && poles >= 3,
                phase_to_neutral: false,
            }
        }
        _ if poles == 1 && inputs.voltage >= 380.0 => RunFacts {
            voltage: Figure::new(inputs.voltage / 3.0_f64.sqrt(), Origin::Assumed),
            three_phase: false,
            phase_to_neutral: true,
        },
        _ => RunFacts {
            voltage: Figure::new(inputs.voltage, Origin::Assumed),
            three_phase: poles >= 3,
            phase_to_neutral: false,
        },
    }
}

fn section_of(inputs: &Inputs<'_>, wire: &novo_core::Wire) -> Figure {
    match wire_section(inputs.project, wire) {
        Some(s) => Figure::stated(s),
        None => inputs
            .proposed
            .get(&wire.id)
            .map_or(Figure::MISSING, |s| Figure::new(*s, Origin::Proposed)),
    }
}

fn wire_label(wire: &novo_core::Wire) -> String {
    wire.number
        .as_deref()
        .filter(|n| !n.is_empty())
        .map_or_else(|| "unnumbered wire".to_owned(), |n| format!("wire {n}"))
}

struct RunInput {
    route: Route,
    current: Figure,
    facts: RunFacts,
    poles: usize,
    protector: Option<(String, f64)>,
    inline: Vec<String>,
}

fn build_link(inputs: &Inputs<'_>, run: RunInput) -> Link {
    let project = inputs.project;
    let supply = if run.facts.three_phase {
        Supply::ThreePhase
    } else {
        Supply::TwoWire
    };
    let voltage = run.facts.voltage.value.unwrap_or(inputs.voltage);
    let mut segments = Vec::new();
    let mut complete = true;
    let mut reasons = Vec::new();
    let mut fail_codes: Vec<&'static str> = Vec::new();
    let mut method_origin = Origin::Assumed;
    let mut iz_defaults: Vec<&'static str> = Vec::new();
    let mut pf_stated = false;
    let mut method = Method::C;
    let mut insulation = Insulation::Pvc;
    let mut iz: Option<f64> = None;
    let mut thinnest: Option<f64> = None;
    let mut pf = Figure::new(1.0, Origin::Assumed);
    let mut sum_drop = 0.0;
    let mut sum_length = 0.0;
    let mut any_length = false;
    let mut any_drop = false;
    let mut weakest_label = String::new();
    for id in &run.route.wires {
        let Some(wire) = project.wire(*id) else {
            continue;
        };
        let section = section_of(inputs, wire);
        let length = wire_length(project, wire).map_or(Figure::MISSING, Figure::stated);
        let ins = wire_insulation(project, wire);
        let power_factor =
            wire_number(project, wire, "power_factor").filter(|pf| (0.1..=1.0).contains(pf));
        if let Some(f) = power_factor {
            pf = Figure::stated(f);
            pf_stated = true;
        }
        let drop = match (length.value, section.value, run.current.value) {
            (Some(l), Some(s), Some(i)) => {
                let mut r = Run::new(l, s, i, supply, voltage);
                r.power_factor = power_factor.unwrap_or(1.0);
                r.temperature_c = ins.max_temperature();
                r.drop_percent().map_or(Figure::MISSING, Figure::calculated)
            }
            _ => Figure::MISSING,
        };
        if length.is_missing() || section.is_missing() {
            complete = false;
        }
        if let Some(l) = length.value {
            sum_length += l;
            any_length = true;
        }
        if let Some(d) = drop.value {
            sum_drop += d;
            any_drop = true;
        }
        if let Some(s) = section.value {
            if thinnest.is_none_or(|t| s < t) {
                thinnest = Some(s);
                insulation = ins;
            }
            let m_text = installation_method_text(project, wire).or_else(|| {
                match project
                    .info()
                    .properties
                    .get(keys::DEFAULT_INSTALLATION_METHOD)
                {
                    Some(novo_core::Value::Text(t)) => Some(t.clone()),
                    _ => None,
                }
            });
            let parsed = m_text.as_deref().and_then(Method::parse);
            let m = parsed.unwrap_or(Method::C);
            let ambient = wire_number(project, wire, AMBIENT_C);
            let grouping = wire_number(project, wire, GROUPED_CIRCUITS).filter(|n| *n >= 1.0);
            let circuit = Circuit {
                design_current: run.current.value.unwrap_or(0.0),
                device_rating: run.protector.as_ref().map_or(0.0, |p| p.1),
                section_mm2: s,
                insulation: ins,
                method: m,
                loaded: if run.facts.three_phase {
                    Loaded::Three
                } else {
                    Loaded::Two
                },
                ambient_c: ambient.unwrap_or(30.0),
                grouped_circuits: grouping.map_or(1, |n| n as u32),
            };
            if let Some(z) = circuit.rating_with(inputs.tables)
                && iz.is_none_or(|old| z < old)
            {
                iz = Some(z);
                method = m;
                method_origin = if parsed.is_some() {
                    Origin::Stated
                } else {
                    Origin::Assumed
                };
                weakest_label = wire_label(wire);
                iz_defaults.clear();
                if parsed.is_none() {
                    iz_defaults.push("installation method C");
                }
                if ambient.is_none() {
                    iz_defaults.push("30 °C");
                }
                if grouping.is_none() {
                    iz_defaults.push("one circuit");
                }
            }
        }
        segments.push(Segment {
            wire: *id,
            label: wire_label(wire),
            cable: project
                .core_of_wire(*id)
                .and_then(|(c, _)| project.cable(c))
                .map(|c| c.tag.to_string()),
            section_mm2: section,
            length_m: length,
            insulation: ins,
            drop_percent: drop,
        });
    }
    if segments.is_empty() {
        complete = true;
    }
    let iz_fig = iz.map_or(Figure::MISSING, |z| {
        if iz_defaults.is_empty() {
            Figure::calculated(z)
        } else {
            Figure::new(z, Origin::Assumed)
        }
    });
    let drop_pf_assumed = !pf_stated && any_drop;
    let drop_origin = |v: f64| {
        if drop_pf_assumed {
            Figure::new(v, Origin::Assumed)
        } else {
            Figure::calculated(v)
        }
    };
    let mut status = Status::Pass;
    let worsen = |to: Status, status: &mut Status| {
        let rank = |s: Status| match s {
            Status::Pass => 0,
            Status::Unknown => 1,
            Status::Warning => 2,
            Status::Fail => 3,
        };
        if rank(to) > rank(*status) {
            *status = to;
        }
    };
    if let (Some(ib), Some(z)) = (run.current.value, iz) {
        if ib > z {
            worsen(Status::Fail, &mut status);
            fail_codes.push("Ib > Iz");
            reasons.push(format!(
                "load {} A is above the cable rating Iz {} A ({weakest_label})",
                fmt(ib),
                fmt(z)
            ));
        } else if let Some((tag, in_a)) = &run.protector
            && *in_a > z
        {
            worsen(Status::Fail, &mut status);
            fail_codes.push("In > Iz");
            reasons.push(format!(
                "{tag} {} A is above the cable rating Iz {} A: the cable is not protected against overload",
                fmt(*in_a),
                fmt(z)
            ));
        }
    } else if !segments.is_empty() {
        worsen(Status::Unknown, &mut status);
        reasons.push(
            "cable rating cannot be judged: the current or a cross-section is missing".to_owned(),
        );
    }
    if !complete {
        worsen(Status::Unknown, &mut status);
        reasons.push("a wire of the run has no length or no cross-section".to_owned());
    }
    if !run.route.traced {
        reasons.push(
            "the path between the two devices could not be followed: every wire of the net is counted"
                .to_owned(),
        );
    }
    Link {
        segments,
        poles: run.poles,
        voltage_v: run.facts.voltage,
        three_phase: run.facts.three_phase,
        length_m: if any_length {
            Figure::stated(sum_length)
        } else {
            Figure::MISSING
        },
        section_mm2: thinnest.map_or(Figure::MISSING, |s| {
            let proposed = run.route.wires.iter().any(|w| {
                inputs.proposed.contains_key(w)
                    && project
                        .wire(*w)
                        .is_some_and(|wire| wire_section(project, wire).is_none())
            });
            Figure::new(
                s,
                if proposed {
                    Origin::Proposed
                } else {
                    Origin::Stated
                },
            )
        }),
        insulation,
        method,
        method_origin,
        current_a: run.current,
        power_factor: pf,
        iz_defaults,
        drop_pf_assumed,
        drop_percent: if any_drop {
            drop_origin(sum_drop)
        } else {
            Figure::MISSING
        },
        drop_total_percent: Figure::MISSING,
        total_complete: complete,
        iz_a: iz_fig,
        protected_by: run.protector,
        complete,
        traced: run.route.traced,
        inline: run.inline,
        status,
        reasons,
        fail_codes,
    }
}

/// A number the way the diagram prints it: no trailing zeros.
pub fn fmt(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    if (rounded - rounded.round()).abs() < 1e-9 {
        format!("{}", rounded.round())
    } else if (rounded * 10.0 - (rounded * 10.0).round()).abs() < 1e-9 {
        format!("{rounded:.1}")
    } else {
        format!("{rounded:.2}")
    }
}

// ---------------------------------------------------------------------------
// Public entry

/// Builds the single-line diagram of the project.
///
/// `proposed` maps wires without a cross-section to the Autopilot's proposal
/// (empty when there is none); those numbers are used and marked
/// [`Origin::Proposed`].
pub fn single_line(
    project: &Project,
    netlist: &Netlist,
    tables: &TableSet,
    proposed: &BTreeMap<WireId, f64>,
) -> SingleLine {
    let nets = Nets::new(project, netlist);
    let sheet = calc_view::calc_sheet(project, netlist, StarterChoice::default(), tables);
    let voltage = supply_voltage(project);
    let props = &project.info().properties;
    let drop_limit_stated = number(props, crate::calc_checks::VOLTAGE_DROP_PCT)
        .filter(|l| *l > 0.0)
        .is_some();
    let drop_limit = sheet.drop_limit_percent;
    let inputs = Inputs {
        project,
        netlist,
        tables,
        proposed,
        voltage,
    };
    let Collected {
        items,
        inline,
        unfed,
        signals_left_out,
    } = collect(project, netlist, &nets);

    let mut notes = vec![
        "Currents above a distribution device are the arithmetic sum of the loads below it: no diversity or simultaneity factor is applied.".to_owned(),
        "The supply is taken at the first device; the impedance upstream of it is not included in the voltage drop.".to_owned(),
    ];
    if sheet.planning_values {
        notes.push(
            "Cable ratings use the built-in ampacity tables, which are planning values until the licensed tables are loaded."
                .to_owned(),
        );
    }

    // Parents.
    let by_load: Vec<(usize, &Item)> = items.iter().enumerate().collect();
    let mut parent: Vec<Option<usize>> = vec![None; items.len()];
    for (i, item) in &by_load {
        let mut best: Option<(u32, usize)> = None;
        for net in item.line.keys() {
            for (j, other) in &by_load {
                if j == i || !other.load.contains_key(net) {
                    continue;
                }
                let key = (other.line_dist, usize::MAX - *j);
                if best.is_none_or(|(d, jj)| key > (d, jj)) {
                    best = Some(key);
                }
            }
        }
        parent[*i] = best.map(|(_, inv)| usize::MAX - inv);
    }
    // No loops: a device that is its own ancestor hangs on the supply.
    for i in 0..items.len() {
        let mut at = parent[i];
        let mut steps = 0;
        while let Some(p) = at {
            if p == i || steps > items.len() {
                parent[i] = None;
                break;
            }
            at = parent[p];
            steps += 1;
        }
    }

    // Order: depth first from the supply, siblings by tag.
    let tags: Vec<String> = items.iter().map(|i| tag_of(project, i.device)).collect();
    let mut kids: Vec<Vec<usize>> = vec![Vec::new(); items.len()];
    let mut roots: Vec<usize> = Vec::new();
    for (i, p) in parent.iter().enumerate() {
        match p {
            Some(p) => kids[*p].push(i),
            None => roots.push(i),
        }
    }
    let sort = |v: &mut Vec<usize>| v.sort_by(|a, b| natural_cmp(&tags[*a], &tags[*b]));
    sort(&mut roots);
    for k in &mut kids {
        sort(k);
    }

    let supply_ik = crate::calc_checks::supply_max_ka(project);
    let supply_ik_fig = match supply_ik {
        Some(ik) => {
            let stated = number(props, crate::calc_checks::SUPPLY_SHORT_CIRCUIT_KA).is_some();
            Figure::new(
                ik,
                if stated {
                    Origin::Stated
                } else {
                    Origin::Calculated
                },
            )
        }
        None => Figure::MISSING,
    };
    let mut nodes = vec![Node {
        id: 0,
        kind: Kind::Supply,
        device: None,
        tag: if project.info().name.is_empty() {
            "Supply".to_owned()
        } else {
            format!("Supply {}", project.info().name)
        },
        description: String::new(),
        parent: None,
        children: Vec::new(),
        rated_a: Figure::MISSING,
        characteristic: None,
        breaking_ka: Figure::MISSING,
        power_kw: Figure::MISSING,
        current_a: Figure::MISSING,
        current_basis: String::new(),
        loads_without_current: 0,
        link: None,
        verdict: None,
        ik_ka: supply_ik_fig,
        ik_exact: true,
        icu_status: Status::Pass,
        depth: 0,
    }];
    let mut node_of_item: Vec<usize> = vec![usize::MAX; items.len()];
    let mut order: Vec<(usize, usize)> = Vec::new(); // (item, parent node)
    {
        fn walk(
            item: usize,
            parent_node: usize,
            kids: &[Vec<usize>],
            order: &mut Vec<(usize, usize)>,
            next: &mut usize,
        ) {
            let id = *next;
            *next += 1;
            order.push((item, parent_node));
            for &k in &kids[item] {
                walk(k, id, kids, order, next);
            }
        }
        let mut next = 1;
        for &r in &roots {
            walk(r, 0, &kids, &mut order, &mut next);
        }
    }
    for (n, (item, _)) in order.iter().enumerate() {
        node_of_item[*item] = n + 1;
    }

    // Boxes, with the facts that need no neighbour.
    let verdicts: HashMap<DeviceId, &calc_view::CircuitEntry> =
        sheet.circuits.iter().map(|c| (c.device, c)).collect();
    for (item_index, parent_node) in &order {
        let item = &items[*item_index];
        let device = project.device(item.device);
        let id = nodes.len();
        let rated = project
            .rated_current_a(item.device)
            .or_else(|| device_number(project, item.device, crate::calc_checks::SETTING_MAX_A))
            .or_else(|| device_number(project, item.device, keys::FLC_A))
            .or_else(|| {
                (item.kind == Kind::Contactor)
                    .then(|| device_number(project, item.device, crate::calc_checks::AC3_CURRENT_A))
                    .flatten()
            })
            .filter(|a| *a > 0.0);
        let power = device_number(project, item.device, "power_kw").filter(|p| *p > 0.0);
        let curve = device_text(project, item.device, TRIP_CURVE);
        let characteristic = match (curve, item.kind) {
            (Some(c), _) => Some((c, Origin::Stated)),
            (None, Kind::Fuse | Kind::FuseSwitch) => Some(("gG".to_owned(), Origin::Assumed)),
            _ => None,
        };
        let verdict = verdicts.get(&item.device).and_then(|entry| {
            let report = entry.report.as_ref()?;
            let fold = |skip_breaking: bool| {
                let mut status = Status::Pass;
                for row in report
                    .rows
                    .iter()
                    .filter(|r| !(skip_breaking && r.title == "Breaking capacity"))
                {
                    status = match (status, row.status) {
                        (Status::Fail, _) | (_, Status::Fail) => Status::Fail,
                        (Status::Warning, _) | (_, Status::Warning) => Status::Warning,
                        (Status::Unknown, _) | (_, Status::Unknown) => Status::Unknown,
                        _ => Status::Pass,
                    };
                }
                status
            };
            Some(Verdict {
                status: fold(false),
                status_without_breaking: fold(true),
                failing: report.failures().map(|r| r.title.to_owned()).collect(),
                breaking_row_failed: report
                    .rows
                    .iter()
                    .any(|r| r.title == "Breaking capacity" && r.status == Status::Fail),
                ik_max_ka: report.at_device.as_ref().map(|f| f.ik3_max_a / 1000.0),
                missing: entry.missing.clone(),
            })
        });
        nodes.push(Node {
            id,
            kind: item.kind,
            device: Some(item.device),
            tag: tags[*item_index].clone(),
            description: device.map(|d| d.description.clone()).unwrap_or_default(),
            parent: Some(*parent_node),
            children: Vec::new(),
            rated_a: rated.map_or(Figure::MISSING, Figure::stated),
            characteristic,
            breaking_ka: device_number(project, item.device, BREAKING_CAPACITY_KA)
                .map_or(Figure::MISSING, Figure::stated),
            power_kw: power.map_or(Figure::MISSING, Figure::stated),
            current_a: Figure::MISSING,
            current_basis: String::new(),
            loads_without_current: 0,
            link: None,
            verdict: if item.kind.is_protective() || item.kind == Kind::OverloadRelay {
                verdict
            } else {
                None
            },
            ik_ka: Figure::MISSING,
            ik_exact: false,
            icu_status: Status::Pass,
            depth: 0,
        });
    }
    for id in 1..nodes.len() {
        let p = nodes[id].parent.unwrap_or(0);
        nodes[id].depth = nodes[p].depth + 1;
        nodes[p].children.push(id);
    }

    // Runs.
    let node_item: HashMap<usize, usize> = order
        .iter()
        .enumerate()
        .map(|(n, (item, _))| (n + 1, *item))
        .collect();
    let mut stated_on_wire: HashMap<usize, f64> = HashMap::new();
    let mut runs: HashMap<usize, (Route, NetId, usize, RunFacts)> = HashMap::new();
    for id in 1..nodes.len() {
        let item = &items[node_item[&id]];
        let parent_id = nodes[id].parent.unwrap_or(0);
        let mut best: Option<(Route, NetId, f64)> = None;
        for (net, to) in &item.line {
            let from: Vec<Endpoint> = if parent_id == 0 {
                source_endpoints(project, netlist, *net)
            } else {
                let parent_item = &items[node_item[&parent_id]];
                match parent_item.load.get(net) {
                    Some(ends) => ends.clone(),
                    None => continue,
                }
            };
            let r = route(project, netlist, *net, &from, to);
            let rank: f64 = r.wires.len() as f64
                + r.wires
                    .iter()
                    .filter_map(|w| project.wire(*w))
                    .map(|w| 1.0 / (1.0 + wire_section(project, w).unwrap_or(0.0)))
                    .sum::<f64>();
            if best.as_ref().is_none_or(|(_, _, old)| rank > *old) {
                best = Some((r, *net, rank));
            }
        }
        let poles = item.line.len();
        if let Some((r, net, _)) = best {
            for w in &r.wires {
                if let Some(wire) = project.wire(*w)
                    && let Some(i) = wire_number(project, wire, LOAD_CURRENT_A).filter(|a| *a > 0.0)
                {
                    stated_on_wire
                        .entry(id)
                        .and_modify(|old| *old = old.max(i))
                        .or_insert(i);
                }
            }
            let facts = run_facts(&inputs, net, poles);
            runs.insert(id, (r, net, poles, facts));
        }
    }

    // Currents, from the leaves up.
    let mut order_up: Vec<usize> = (1..nodes.len()).collect();
    order_up.sort_by_key(|n| std::cmp::Reverse(nodes[*n].depth));
    for &id in &order_up {
        let item = &items[node_item[&id]];
        let facts_voltage = runs
            .get(&id)
            .and_then(|r| r.3.voltage.value)
            .unwrap_or(voltage);
        let three_phase = runs.get(&id).is_none_or(|r| r.3.three_phase);
        let kids: Vec<usize> = nodes[id].children.clone();
        let (own, basis): (Figure, String) = if let Some(a) = stated_on_wire.get(&id) {
            (
                Figure::stated(*a),
                "load_current_a stated on the wire".to_owned(),
            )
        } else if item.kind.is_load()
            && let Some(a) = nodes[id].rated_a.value
        {
            (Figure::stated(a), "rated current of the load".to_owned())
        } else if item.kind.is_load()
            && let Some(p) = nodes[id].power_kw.value
        {
            let current = if item.kind == Kind::Motor && three_phase {
                full_load_current(p, facts_voltage, typical_efficiency(p), typical_cos_phi(p))
            } else if item.kind == Kind::Motor {
                // One phase: the same typical efficiency and power factor.
                let (eta, cos) = (typical_efficiency(p), typical_cos_phi(p));
                (facts_voltage > 0.0 && eta > 0.0 && cos > 0.0)
                    .then(|| p * 1000.0 / (facts_voltage * eta * cos))
            } else if three_phase {
                Some(p * 1000.0 / (3.0_f64.sqrt() * facts_voltage))
            } else {
                Some(p * 1000.0 / facts_voltage)
            };
            match current {
                Some(a) if item.kind == Kind::Motor => (
                    Figure::calculated(a),
                    format!(
                        "calculated from {} kW with typical efficiency and power factor (replace with the nameplate current)",
                        fmt(p)
                    ),
                ),
                Some(a) => (
                    Figure::calculated(a),
                    format!("calculated from {} kW at power factor 1", fmt(p)),
                ),
                None => (Figure::MISSING, String::new()),
            }
        } else {
            (Figure::MISSING, String::new())
        };
        if !own.is_missing() {
            nodes[id].current_a = own;
            nodes[id].current_basis = basis;
            nodes[id].loads_without_current = 0;
            continue;
        }
        // The sum of what hangs below.
        let mut sum = 0.0;
        let mut counted = 0;
        let mut without = 0;
        // Behind a transformer or power supply the branches carry another
        // voltage: their power is carried to the input side, never their
        // amperes added.
        let converter = matches!(item.kind, Kind::Transformer | Kind::PowerSupply);
        let phases = |three: bool| if three { 3.0_f64.sqrt() } else { 1.0 };
        for k in &kids {
            let below = if converter {
                let out = runs.get(k).map(|r| &r.3);
                match (nodes[*k].current_a.value, out) {
                    (Some(a), Some(f))
                        if matches!(f.voltage.origin, Origin::Stated | Origin::Calculated)
                            && f.voltage.value.is_some_and(|v| v > 0.0)
                            && facts_voltage > 0.0 =>
                    {
                        let v_out = f.voltage.value.unwrap_or(0.0);
                        Some(
                            a * v_out * phases(f.three_phase)
                                / (facts_voltage * phases(three_phase)),
                        )
                    }
                    _ => None,
                }
            } else {
                nodes[*k].current_a.value
            };
            match below {
                Some(a) => {
                    sum += a;
                    counted += 1;
                }
                None => {
                    without += nodes[*k].loads_without_current.max(1);
                }
            }
            if nodes[*k].current_a.value.is_some() {
                without += nodes[*k].loads_without_current;
            }
        }
        if counted > 0 {
            nodes[id].current_a = Figure::calculated(sum);
            nodes[id].current_basis = format!(
                "{} {} branch{} below{}, no diversity",
                if converter {
                    "output power of"
                } else {
                    "sum of"
                },
                counted,
                if counted == 1 { "" } else { "es" },
                if converter {
                    " carried to the input voltage, no losses"
                } else {
                    ""
                },
            );
        } else if item.kind.is_load() {
            nodes[id].loads_without_current = 1;
        } else {
            nodes[id].loads_without_current = without;
        }
        if counted > 0 {
            nodes[id].loads_without_current = without;
        }
    }
    // The supply carries the sum of its branches.
    {
        let mut sum = 0.0;
        let mut counted = 0;
        let mut without = 0;
        for k in nodes[0].children.clone() {
            match nodes[k].current_a.value {
                Some(a) => {
                    sum += a;
                    counted += 1;
                    without += nodes[k].loads_without_current;
                }
                None => without += nodes[k].loads_without_current.max(1),
            }
        }
        if counted > 0 {
            nodes[0].current_a = Figure::calculated(sum);
            nodes[0].current_basis = "sum of the branches, no diversity".to_owned();
        }
        nodes[0].loads_without_current = without;
    }

    // Links, top down, so the cumulative drop and the protector are known.
    let mut phase_voltage_used = false;
    let mut order_down: Vec<usize> = (1..nodes.len()).collect();
    order_down.sort_by_key(|n| nodes[*n].depth);
    let mut protector_above: HashMap<usize, Option<(String, f64)>> = HashMap::new();
    let mut restarted: Vec<String> = Vec::new();
    protector_above.insert(0, None);
    // The cable impedance from the supply to each box, and whether every run
    // on the way is complete. `None` past a converter or without a source.
    let source = supply_ik.and_then(|ik| Source::from_ik(ik, voltage));
    let mut ik_state: HashMap<usize, Option<(Impedance, bool)>> = HashMap::new();
    ik_state.insert(0, source.map(|_| (Impedance::default(), true)));
    let mut ik_through_converter = false;
    let mut breaking_judged_along_run = false;
    for &id in &order_down {
        let parent_id = nodes[id].parent.unwrap_or(0);
        let above = protector_above.get(&parent_id).cloned().flatten();
        let own = if nodes[id].kind.is_protective() {
            nodes[id]
                .rated_a
                .value
                .map(|a| (nodes[id].tag.clone(), a))
                .or(above.clone())
        } else {
            above.clone()
        };
        protector_above.insert(id, own);
        let parent_is_converter =
            matches!(nodes[parent_id].kind, Kind::Transformer | Kind::PowerSupply);
        if parent_is_converter && nodes[parent_id].ik_ka.value.is_some() {
            ik_through_converter = true;
        }
        let parent_ik = if parent_is_converter {
            None
        } else {
            ik_state.get(&parent_id).copied().flatten()
        };
        let Some((route, _net, poles, facts)) = runs.remove(&id) else {
            ik_state.insert(id, parent_ik);
            continue;
        };
        phase_voltage_used |= facts.phase_to_neutral;
        let inline_tags: Vec<String> = route
            .via
            .iter()
            .filter_map(|s| inline.get(s))
            .map(|(d, k)| format!("{} {}", tag_of(project, *d), k.label().to_lowercase()))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut link = build_link(
            &inputs,
            RunInput {
                route,
                current: nodes[id].current_a,
                facts,
                poles,
                protector: above,
                inline: inline_tags,
            },
        );
        let mut above_link = nodes[parent_id].link.as_ref();
        let mut up = nodes[parent_id].parent;
        while above_link.is_none()
            && let Some(u) = up
        {
            above_link = nodes[u].link.as_ref();
            up = nodes[u].parent;
        }
        // The drop is counted again from the output of a transformer or
        // power supply: percentages of two voltages are not added.
        let from_converter = matches!(nodes[parent_id].kind, Kind::Transformer | Kind::PowerSupply);
        if from_converter {
            above_link = None;
            if let Some(d) = nodes[parent_id].device {
                restarted.push(tag_of(project, d));
            }
        }
        let parent_total = above_link
            .and_then(|l| l.drop_total_percent.value)
            .unwrap_or(0.0);
        link.total_complete = link.complete && above_link.is_none_or(|l| l.total_complete);
        link.drop_total_percent = match link.drop_percent.value {
            Some(d) => Figure::calculated(parent_total + d),
            None if parent_total > 0.0 => Figure::calculated(parent_total),
            None => Figure::MISSING,
        };
        if link.drop_pf_assumed
            || above_link.is_some_and(|l| l.drop_total_percent.origin == Origin::Assumed)
        {
            link.drop_total_percent.origin = Origin::Assumed;
        }
        if let Some(total) = link.drop_total_percent.value
            && total > drop_limit
        {
            link.status = Status::Fail;
            link.fail_codes.push("ΔU > limit");
            link.reasons.push(format!(
                "voltage drop {} % from {} is above the limit {} %",
                fmt(total),
                if from_converter {
                    "the converter output"
                } else {
                    "the supply"
                },
                fmt(drop_limit)
            ));
        }
        let state = match (source, parent_ik) {
            (Some(_), Some((mut z, mut exact))) if link.three_phase => {
                exact &= link.complete && link.traced;
                for seg in &link.segments {
                    match (seg.section_mm2.value, seg.length_m.value) {
                        (Some(sec), Some(len)) => match cable_impedance(sec, len, 20.0) {
                            Some(c) => z = z + c,
                            None => exact = false,
                        },
                        _ => exact = false,
                    }
                }
                Some((z, exact))
            }
            _ => None,
        };
        ik_state.insert(id, state);
        if let (Some(src), Some((z, exact))) = (source, state) {
            let ik = ik3_max(&src, &[z]) / 1000.0;
            nodes[id].ik_ka = Figure::calculated(ik);
            nodes[id].ik_exact = exact;
            if let Some(v) = nodes[id].verdict.as_mut()
                && v.breaking_row_failed
            {
                v.status = v.status_without_breaking;
                v.failing.retain(|t| t != "Breaking capacity");
                breaking_judged_along_run = true;
            }
            if let Some(icu) = nodes[id].breaking_ka.value
                && nodes[id].kind.is_protective()
                && ik > icu
            {
                nodes[id].icu_status = if exact { Status::Fail } else { Status::Warning };
            }
        }
        nodes[id].link = Some(link);
    }

    let mut unfed_tags: Vec<String> = unfed
        .iter()
        .map(|d| tag_of(project, *d))
        .filter(|t| !t.is_empty())
        .collect();
    unfed_tags.sort_by(|a, b| natural_cmp(a, b));
    let mut left_out: Vec<String> = signals_left_out
        .iter()
        .map(|d| tag_of(project, *d))
        .filter(|t| !t.is_empty())
        .collect();
    left_out.sort_by(|a, b| natural_cmp(a, b));
    if !left_out.is_empty() {
        let shown = left_out.len().min(4);
        notes.push(format!(
            "Not drawn, lamps and sounders on control circuits (no mains voltage stated on their net): {}{}.",
            left_out[..shown].join(", "),
            if left_out.len() > shown {
                format!(" and {} more", left_out.len() - shown)
            } else {
                String::new()
            }
        ));
    }
    if nodes.iter().skip(1).any(|n| n.kind.is_signal()) {
        notes.push(
            "Lamps and sounders are drawn from their stated power or current; contactor and relay coils on the same feeder are not counted in its current."
                .to_owned(),
        );
    }
    if phase_voltage_used {
        notes.push(
            "A load on one line conductor is taken phase to neutral, the line voltage (stated, or the project's) divided by √3 (400 V gives 231 V); a load between two lines keeps the line voltage."
                .to_owned(),
        );
    }
    if !unfed_tags.is_empty() {
        notes.push(format!(
            "Not on the diagram, no supply reaches their power poles: {}.",
            unfed_tags.join(", ")
        ));
    }
    restarted.sort_by(|a, b| natural_cmp(a, b));
    restarted.dedup();
    if !restarted.is_empty() {
        notes.push(format!(
            "ΔU Σ is counted again from the output of {}: percentages of two voltages are not added.",
            restarted.join(", ")
        ));
    }
    if breaking_judged_along_run {
        notes.push(
            "Icu is checked against Ik along the run, which the cable impedance lowers; the Calculations window compares with the supply fault level alone, so it can fail a device that passes here."
                .to_owned(),
        );
    }
    if ik_through_converter {
        notes.push(
            "Short-circuit current is not carried through a transformer or power supply: boxes behind one show no Ik."
                .to_owned(),
        );
    }
    if nodes.iter().skip(1).any(|n| n.ik_ka.value.is_some()) {
        notes.push(
            "Ik is the three-phase maximum (IEC 60909-0, low-voltage simplification): the stated supply fault level through the cable impedance of each run, conductors at 20 °C; motor contribution and busbar impedance are left out."
                .to_owned(),
        );
    }
    if nodes
        .iter()
        .skip(1)
        .any(|n| n.ik_ka.value.is_some() && !n.ik_exact)
    {
        notes.push(
            "Ik is a ceiling (shown as ≤) where a run above has no length or cross-section: its cable impedance is left out."
                .to_owned(),
        );
    }
    let links = || nodes.iter().filter_map(|n| n.link.as_ref());
    let mut assumed: Vec<&'static str> = links()
        .flat_map(|l| l.iz_defaults.iter().copied())
        .collect();
    assumed.sort_unstable();
    assumed.dedup();
    if !assumed.is_empty() {
        notes.push(
            "Iz assumes installation method C, 30 °C and one circuit where the wire does not state them (marked ?)."
                .to_owned(),
        );
    }
    if links().any(|l| l.drop_pf_assumed) {
        notes.push(
            "ΔU is calculated at power factor 1 where none is stated (marked ?); inductive loads drop more."
                .to_owned(),
        );
    }
    SingleLine {
        nodes,
        voltage_v: voltage,
        supply_ik_ka: supply_ik_fig,
        drop_limit_percent: drop_limit,
        drop_limit_stated,
        supply_text: sheet.supply.clone(),
        notes,
        missing: sheet.missing.clone(),
        unfed: unfed_tags,
        tables: tables.disclosure(),
        planning_values: sheet.planning_values,
    }
}

/// The pins and wire ends that define the potentials of a net.
fn source_endpoints(project: &Project, netlist: &Netlist, net: NetId) -> Vec<Endpoint> {
    use novo_core::AnyId;
    let net = netlist.net(net);
    let mut out: Vec<Endpoint> = Vec::new();
    for source in &net.sources {
        match source.by {
            AnyId::Symbol(s) => {
                out.extend(
                    net.endpoints
                        .iter()
                        .copied()
                        .filter(|e| matches!(e, Endpoint::Pin { symbol, .. } if *symbol == s)),
                );
            }
            AnyId::Wire(w) => {
                if let Some(wire) = project.wire(w) {
                    out.extend([wire.from, wire.to]);
                }
            }
            AnyId::Terminal(t) => {
                out.extend(net.endpoints.iter().copied().filter(|e| {
                    matches!(e, Endpoint::Pin { symbol, pin }
                        if project.symbol(*symbol).and_then(|s| s.terminal_of_pin(*pin)) == Some(t))
                }));
            }
            _ => {}
        }
    }
    out.retain(|e| net.endpoints.contains(e));
    out.sort();
    out.dedup();
    out
}
