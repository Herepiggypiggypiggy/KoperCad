//! Why is this lit, why is that dead: the live view's answer in words.
//!
//! [`explain_net`] takes one net of a [`Settled`] state and says what it
//! carries and how the potential got there (a route of closed poles from the
//! supply), or, when it is dead, which open poles stand between it and the
//! nearest supply and why each is open (a push button nobody holds, a
//! contact whose coil is off, an interlock whose coil is on). [`explain_coil`]
//! and [`explain_symbol`] ask the same question of the pins of a device.
//!
//! This is a query on the state the solver already produced; it changes
//! nothing and stores nothing. The route is the one with the fewest open
//! poles, then the fewest poles, so the cause named for a dead net is the
//! cheapest repair, not the only possible one. It follows potentials, not
//! currents: no direction of flow is modelled, so a route reads from the
//! supply to the asked net and nothing more should be read into it. A net
//! switched on by a PLC output or a forced output has no route to show and
//! is reported as driven.

use std::cmp::Reverse;
use std::collections::{BTreeSet, BinaryHeap, HashMap};

use novo_core::{DeviceId, Endpoint, NetId, Netlist, PotentialId, Project, SymbolId};

use crate::{Live, Settled, is_coil, is_timer_contact};

/// One switched pole: group `group` of `SymbolInstance::pass_through`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pole {
    pub symbol: SymbolId,
    pub group: usize,
}

/// What kind of hand-operated device an open pole belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandKind {
    /// Springs back when let go.
    Pushbutton,
    /// A breaker, fuse or isolator.
    Protective,
    /// An emergency stop, which is open once pressed.
    EmergencyStop,
    /// A selector, limit switch or any other switch.
    Switch,
}

/// Why a pole is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenWhy {
    /// It follows the coil of its device. `on` is that coil's state; a
    /// normally closed contact is open because the coil is on, a normally
    /// open one because it is off, or `delayed` when the coil is on but the
    /// on-delay has not run out.
    Coil {
        coil: SymbolId,
        on: bool,
        delayed: bool,
    },
    /// It is operated by hand and sits open.
    Hand(HandKind),
}

/// One hop of a route: across `pole` into net `to`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Step {
    pub pole: Pole,
    pub closed: bool,
    pub to: NetId,
    /// Why the pole is open; `None` for a closed one.
    pub why: Option<OpenWhy>,
}

/// A way from a supply to a net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Route {
    pub potential: PotentialId,
    /// The net that defines the potential.
    pub source: NetId,
    pub steps: Vec<Step>,
}

impl Route {
    /// The open poles on the route: where it breaks.
    pub fn breaks(&self) -> impl Iterator<Item = &Step> {
        self.steps.iter().filter(|s| !s.closed)
    }

    /// Every net the route touches, the source first.
    pub fn nets(&self) -> Vec<NetId> {
        std::iter::once(self.source)
            .chain(self.steps.iter().map(|s| s.to))
            .collect()
    }
}

/// The answer for one net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Carries one potential, reached over the closed poles of the route
    /// (no steps when the net defines it itself).
    Carries(Route),
    /// Carries a potential that no route explains: an output switched on.
    Driven(PotentialId),
    /// Two or more potentials meet here.
    Shorted(Vec<PotentialId>),
    /// Dead; the route is the cheapest way a supply could reach it, and its
    /// open poles are what stops it.
    Cut(Route),
    /// Dead, and no supply reaches it even with every pole closed.
    Isolated,
}

/// What one net carries and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Explanation {
    pub net: NetId,
    pub verdict: Verdict,
}

/// One line of the answer; `symbol` is the device the line is about, which a
/// view can make clickable ("why is that one open?").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub symbol: Option<SymbolId>,
}

impl Line {
    fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            symbol: None,
        }
    }
    fn about(text: impl Into<String>, symbol: SymbolId) -> Self {
        Self {
            text: text.into(),
            symbol: Some(symbol),
        }
    }
}

impl Explanation {
    /// The route, if the answer has one.
    pub fn route(&self) -> Option<&Route> {
        match &self.verdict {
            Verdict::Carries(r) | Verdict::Cut(r) => Some(r),
            _ => None,
        }
    }

    /// Nets to highlight: the route, or just the asked net.
    pub fn nets(&self) -> Vec<NetId> {
        self.route().map_or_else(|| vec![self.net], Route::nets)
    }

    /// The answer in plain words, one hop per line. `name` gives a device
    /// its tag.
    pub fn lines(&self, project: &Project, name: &dyn Fn(SymbolId) -> String) -> Vec<Line> {
        let pot = |p: PotentialId| {
            project
                .potential(p)
                .map_or_else(|| "?".to_owned(), |p| p.name.clone())
        };
        match &self.verdict {
            Verdict::Carries(route) => {
                let mut out = vec![Line::plain(format!("It carries {}.", pot(route.potential)))];
                if route.steps.is_empty() {
                    out.push(Line::plain(format!(
                        "{} is connected here directly.",
                        pot(route.potential)
                    )));
                } else {
                    out.push(Line::plain(format!(
                        "{} comes from the supply and passes:",
                        pot(route.potential)
                    )));
                    for step in &route.steps {
                        out.push(Line::about(
                            format!("{} (closed)", name(step.pole.symbol)),
                            step.pole.symbol,
                        ));
                    }
                }
                out
            }
            Verdict::Driven(p) => vec![Line::plain(format!(
                "It carries {}, put there by an output that is switched on.",
                pot(*p)
            ))],
            Verdict::Shorted(ps) => vec![Line::plain(format!(
                "Short: {} meet here.",
                ps.iter().map(|&p| pot(p)).collect::<Vec<_>>().join(" and ")
            ))],
            Verdict::Isolated => vec![Line::plain(
                "It is dead: no supply reaches it through contacts. Check the wiring, or a device that is not a contact (a transformer winding) may feed it.",
            )],
            Verdict::Cut(route) => {
                let breaks: Vec<&Step> = route.breaks().collect();
                let mut out = vec![Line::plain(format!(
                    "It is dead. {} would reach it, but {} stop{} it:",
                    pot(route.potential),
                    if breaks.len() == 1 {
                        "one open contact".to_owned()
                    } else {
                        format!("{} open contacts", breaks.len())
                    },
                    if breaks.len() == 1 { "s" } else { "" },
                ))];
                for step in breaks {
                    let text = open_text(step.why, &name(step.pole.symbol), name);
                    out.push(Line::about(text, step.pole.symbol));
                }
                out
            }
        }
    }
}

/// A sentence for an open pole `n` and its cause.
fn open_text(why: Option<OpenWhy>, n: &str, name: &dyn Fn(SymbolId) -> String) -> String {
    match why {
        Some(OpenWhy::Coil {
            coil, on: false, ..
        }) => format!("{n} is open because its coil {} is off.", name(coil)),
        Some(OpenWhy::Coil {
            coil,
            on: true,
            delayed: true,
        }) => format!(
            "{n} is open: its coil {} is on, but the on-delay has not run out.",
            name(coil)
        ),
        Some(OpenWhy::Coil {
            coil,
            on: true,
            delayed: false,
        }) => format!(
            "{n} is open because its coil {} is on (it opens when pulled in).",
            name(coil)
        ),
        Some(OpenWhy::Hand(HandKind::Pushbutton)) => {
            format!("{n} is a push button and is not pressed. Hold it.")
        }
        Some(OpenWhy::Hand(HandKind::Protective)) => format!("{n} is switched off or has tripped."),
        Some(OpenWhy::Hand(HandKind::EmergencyStop)) => format!("{n} is pressed in. Release it."),
        Some(OpenWhy::Hand(HandKind::Switch)) | None => format!("{n} is open. Switch it on."),
    }
}

/// An edge of the net graph: a pole joining two nets.
struct Edge {
    to: usize,
    pole: Pole,
    closed: bool,
}

/// Explains `net`. `avoid` leaves one potential out of the search for a dead
/// net: the other pin of a coil or lamp already carries it, and the question
/// is what the pin needs to see a *different* one.
pub fn explain_net(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    net: NetId,
    avoid: Option<PotentialId>,
) -> Explanation {
    let target = net.0 as usize;
    let verdict = match settled.solution.net(net) {
        Live::Shorted(ps) => Verdict::Shorted(ps.clone()),
        Live::Carries(p) => {
            let p = *p;
            match route_to(project, netlist, settled, target, |q| q == p, false) {
                Some(route) => Verdict::Carries(route),
                None => Verdict::Driven(p),
            }
        }
        Live::Dead => match route_to(
            project,
            netlist,
            settled,
            target,
            |q| Some(q) != avoid,
            true,
        ) {
            Some(route) => Verdict::Cut(route),
            None => Verdict::Isolated,
        },
    };
    Explanation { net, verdict }
}

/// Explains what the net of `end` carries; `None` for an endpoint in no net.
pub fn explain_endpoint(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    end: Endpoint,
    avoid: Option<PotentialId>,
) -> Option<Explanation> {
    let net = netlist.net_of(end)?;
    Some(explain_net(project, netlist, settled, net, avoid))
}

/// Cheapest route from a net defining a wanted potential to `target`: the
/// fewest open poles, then the fewest poles. With `through_open` false only
/// closed poles may be crossed. Like the solver it never enters a net that
/// defines a potential of its own on the way.
fn route_to(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    target: usize,
    wanted: impl Fn(PotentialId) -> bool,
    through_open: bool,
) -> Option<Route> {
    let count = netlist.nets().len();
    let mut edges: Vec<Vec<Edge>> = (0..count).map(|_| Vec::new()).collect();
    for symbol in project.symbols() {
        for (group, pins) in symbol.pass_through.iter().enumerate() {
            let closed = settled.poles.is_closed(symbol.id, group);
            if !closed && !through_open {
                continue;
            }
            let nets: Vec<usize> = pins
                .iter()
                .filter_map(|&pin| netlist.net_of(Endpoint::pin(symbol.id, pin)))
                .map(|n| n.0 as usize)
                .collect();
            for &a in &nets {
                for &b in &nets {
                    if a != b {
                        edges[a].push(Edge {
                            to: b,
                            pole: Pole {
                                symbol: symbol.id,
                                group,
                            },
                            closed,
                        });
                    }
                }
            }
        }
    }
    let defines = |n: usize| !netlist.nets()[n].defined.is_empty();

    // (open poles, poles, net); best first.
    let mut best: HashMap<usize, (usize, usize)> = HashMap::new();
    let mut prev: HashMap<usize, (usize, Step)> = HashMap::new();
    let mut heap = BinaryHeap::new();
    let mut origin: HashMap<usize, PotentialId> = HashMap::new();
    for (i, net) in netlist.nets().iter().enumerate() {
        if let Some(&p) = net.defined.iter().find(|&&p| wanted(p)) {
            best.insert(i, (0, 0));
            origin.insert(i, p);
            heap.push(Reverse((0usize, 0usize, i)));
        }
    }
    let mut done = BTreeSet::new();
    while let Some(Reverse((open, hops, net))) = heap.pop() {
        if !done.insert(net) {
            continue;
        }
        if net == target {
            break;
        }
        for edge in &edges[net] {
            // A net with its own definition is a supply, not a conductor.
            if defines(edge.to) {
                continue;
            }
            let cost = (open + usize::from(!edge.closed), hops + 1);
            if best.get(&edge.to).is_none_or(|&b| cost < b) {
                best.insert(edge.to, cost);
                prev.insert(
                    edge.to,
                    (
                        net,
                        Step {
                            pole: edge.pole,
                            closed: edge.closed,
                            to: NetId(edge.to as u32),
                            why: None,
                        },
                    ),
                );
                heap.push(Reverse((cost.0, cost.1, edge.to)));
            }
        }
    }
    if !best.contains_key(&target) {
        return None;
    }
    let mut steps = Vec::new();
    let mut at = target;
    while let Some((from, step)) = prev.get(&at) {
        steps.push(*step);
        at = *from;
    }
    steps.reverse();
    let potential = *origin.get(&at)?;
    for step in &mut steps {
        if !step.closed {
            step.why = Some(open_why(project, settled, step.pole.symbol));
        }
    }
    Some(Route {
        potential,
        source: NetId(at as u32),
        steps,
    })
}

/// Why a pole of `symbol` is open.
fn open_why(project: &Project, settled: &Settled, symbol: SymbolId) -> OpenWhy {
    let Some(instance) = project.symbol(symbol) else {
        return OpenWhy::Hand(HandKind::Switch);
    };
    if settled.driven.contains(&symbol)
        && let Some(coil) = coil_of(project, instance.device())
    {
        let on = settled.energized.contains(&coil);
        return OpenWhy::Coil {
            coil,
            on,
            delayed: on && is_timer_contact(instance),
        };
    }
    OpenWhy::Hand(hand_kind(&instance.symbol.0))
}

/// The coil of a device, if it draws one.
fn coil_of(project: &Project, device: Option<DeviceId>) -> Option<SymbolId> {
    let device = device?;
    project
        .symbols_of_device(device)
        .into_iter()
        .find(|s| is_coil(s))
        .map(|s| s.id)
}

/// Classifies a hand-operated symbol by its library key.
pub fn hand_kind(key: &str) -> HandKind {
    let name = key.rsplit('/').next().unwrap_or("");
    if key.contains("/operators/") && name.contains("emergency") {
        HandKind::EmergencyStop
    } else if is_pushbutton_key(key) {
        HandKind::Pushbutton
    } else if key.contains("/protection/")
        || (key.contains("/switches/") && !key.contains("changeover"))
    {
        HandKind::Protective
    } else {
        HandKind::Switch
    }
}

/// A push button or foot switch: it springs back when let go.
pub fn is_pushbutton_key(key: &str) -> bool {
    let name = key.rsplit('/').next().unwrap_or("");
    key.contains("/operators/")
        && (name.starts_with("pushbutton") || name.starts_with("footswitch"))
}

/// Why a winding, lamp or valve is on or off: one explanation per pin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinWhy {
    pub pin: u16,
    pub explanation: Explanation,
}

/// The pins of a symbol, each explained. A two-pin load (coil, lamp, valve)
/// is on when its pins carry different potentials, so a dead pin is explained
/// against the potential on the other one. Pins named PE are left out.
pub fn explain_symbol(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    symbol: SymbolId,
) -> Vec<PinWhy> {
    let Some(s) = project.symbol(symbol) else {
        return Vec::new();
    };
    let pins: Vec<u16> = (0..s.pins.len() as u16)
        .filter(|&i| !s.pins[usize::from(i)].name.eq_ignore_ascii_case("pe"))
        .collect();
    let carried = |pin: u16| {
        settled
            .solution
            .of(netlist, Endpoint::pin(symbol, pin))
            .potential()
    };
    pins.iter()
        .filter_map(|&pin| {
            let avoid = match pins.as_slice() {
                [a, b] => carried(if pin == *a { *b } else { *a }),
                _ => None,
            };
            explain_endpoint(project, netlist, settled, Endpoint::pin(symbol, pin), avoid)
                .map(|explanation| PinWhy { pin, explanation })
        })
        .collect()
}

/// The explanation of a coil as a sentence plus its two pins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoilWhy {
    pub coil: SymbolId,
    pub on: bool,
    pub pins: Vec<PinWhy>,
}

/// Why a coil is pulled in or not.
pub fn explain_coil(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    coil: SymbolId,
) -> CoilWhy {
    CoilWhy {
        coil,
        on: settled.energized.contains(&coil),
        pins: explain_symbol(project, netlist, settled, coil),
    }
}

/// A finished answer for a view: a title, lines to read (each may lead to the
/// device it is about), and what to light up on the drawing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Answer {
    pub title: String,
    pub lines: Vec<Line>,
    /// Nets on the route (or the asked net itself).
    pub nets: Vec<NetId>,
    /// Symbols with an open pole in the way.
    pub breaks: Vec<SymbolId>,
    /// The thing asked about is dead or cut off.
    pub dead: bool,
}

/// The answer for one net.
pub fn answer_net(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    net: NetId,
    name: &dyn Fn(SymbolId) -> String,
) -> Answer {
    let e = explain_net(project, netlist, settled, net, None);
    Answer {
        title: "This wire".to_owned(),
        lines: e.lines(project, name),
        nets: e.nets(),
        breaks: breaks_of(&e),
        dead: matches!(e.verdict, Verdict::Cut(_) | Verdict::Isolated),
    }
}

fn breaks_of(e: &Explanation) -> Vec<SymbolId> {
    e.route()
        .map(|r| r.breaks().map(|s| s.pole.symbol).collect())
        .unwrap_or_default()
}

/// The answer for a device: a contact says why it is open or closed, anything
/// else is asked pin by pin. `forced` are the coils switched on by hand.
pub fn answer_symbol(
    project: &Project,
    netlist: &Netlist,
    settled: &Settled,
    forced: &BTreeSet<SymbolId>,
    symbol: SymbolId,
    name: &dyn Fn(SymbolId) -> String,
) -> Answer {
    let n = name(symbol);
    let Some(s) = project.symbol(symbol) else {
        return Answer {
            title: n,
            lines: Vec::new(),
            nets: Vec::new(),
            breaks: Vec::new(),
            dead: false,
        };
    };
    if !s.pass_through.is_empty() {
        let closed = (0..s.pass_through.len()).all(|g| settled.poles.is_closed(symbol, g));
        let mut lines = Vec::new();
        if let Some(coil) = settled
            .driven
            .contains(&symbol)
            .then(|| coil_of(project, s.device()))
            .flatten()
        {
            let on = settled.energized.contains(&coil);
            lines.push(Line::about(
                format!(
                    "{n} is {} because its coil {} is {}.",
                    if closed { "closed" } else { "open" },
                    name(coil),
                    if on { "on" } else { "off" }
                ),
                coil,
            ));
        } else if closed {
            lines.push(Line::plain(format!("{n} is closed.")));
        } else {
            lines.push(Line::about(
                open_text(Some(open_why(project, settled, symbol)), &n, name),
                symbol,
            ));
        }
        return Answer {
            title: format!("{n}: {}", if closed { "closed" } else { "open" }),
            lines,
            nets: Vec::new(),
            breaks: Vec::new(),
            dead: false,
        };
    }
    let pins = explain_symbol(project, netlist, settled, symbol);
    let on = settled.energized.contains(&symbol);
    let is_coil_symbol = is_coil(s);
    let mut lines = Vec::new();
    if is_coil_symbol && forced.contains(&symbol) {
        lines.push(Line::plain("Switched on by hand from the bench."));
    }
    let mut nets = Vec::new();
    let mut breaks = Vec::new();
    let mut dead = false;
    for p in &pins {
        let pin_name = &s.pins[usize::from(p.pin)].name;
        lines.push(Line::plain(format!("Pin {pin_name}:")));
        lines.extend(p.explanation.lines(project, name).into_iter().map(|mut l| {
            l.text = format!("  {}", l.text);
            l
        }));
        nets.extend(p.explanation.nets());
        breaks.extend(breaks_of(&p.explanation));
        dead |= matches!(p.explanation.verdict, Verdict::Cut(_) | Verdict::Isolated);
    }
    breaks.sort();
    breaks.dedup();
    nets.sort();
    nets.dedup();
    let title = if is_coil_symbol {
        format!("{n} is {}", if on { "on" } else { "off" })
    } else {
        n
    };
    Answer {
        title,
        lines,
        nets,
        breaks,
        dead: dead && !on,
    }
}

/// A short plain-words summary of what the circuit is doing, to replace a
/// count of closed poles: coils pulled in, motors running, lamps lit. `None`
/// when nothing is switched on.
pub fn story(
    project: &Project,
    settled: &Settled,
    states: &crate::DeviceStates,
    name: &dyn Fn(SymbolId) -> String,
) -> Option<String> {
    use crate::{Direction, MotorRun};
    let mut parts: Vec<String> = Vec::new();
    for &coil in &settled.energized {
        if project.symbol(coil).is_some_and(is_coil) {
            parts.push(format!("{} pulled in", name(coil)));
        }
    }
    for (&motor, run) in &states.motors {
        match run {
            MotorRun::Stopped => {}
            MotorRun::Running { direction, .. } => parts.push(format!(
                "{} runs{}",
                name(motor),
                if *direction == Direction::Reverse {
                    " in reverse"
                } else {
                    ""
                }
            )),
            MotorRun::PhaseLoss => parts.push(format!("{} hums (a phase is missing)", name(motor))),
            MotorRun::Fault => parts.push(format!("{} has a fault", name(motor))),
        }
    }
    let lit = states.lamps.values().filter(|&&on| on).count();
    if lit > 0 {
        parts.push(format!("{lit} lamp{} lit", if lit == 1 { "" } else { "s" }));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}
