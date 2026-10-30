//! Commands from outside the drawing: ports of the module instances (lane L5b).
//!
//! A virtual controller (`novo-controller`) can bind a signal to a *port* of a module instance:
//! `up`, `down` and `fast` of a hoist, `forward`, `reverse` and `fast` of a travel axis, and so on.
//! This file turns the level of such a port into what the loops already read from the circuit:
//! the motor feed of a contactor hoist, the energised state of the device tags that carry a
//! drive's digital inputs, the brake feed. The loops (`hoist_loop`, `travel_loop`,
//! `drive_hoist_loop`, `cl_winch_loop`) stay as they are: [`CommandedCircuit`] wraps the
//! [`CircuitPort`] they step against and answers for the tags a bound port owns.
//!
//! Rules, the same for every module:
//! - a port that nothing is bound to changes nothing: the module reads its drawn circuit;
//! - a bound port replaces the drawn input for that function while it is bound (the drawing no
//!   longer decides it), so a wiring fault on that input is not simulated for it;
//! - a *permit* port is an E-stop or safety chain: high means the module may run, low removes every
//!   command, drops the motor feed and the brake feed and opens the main contactor and safety
//!   torque off channels that the module has;
//! - an *enable* port (dead-man bar) works like a permit that the operator holds: the module only
//!   moves while it is high. Both ports act only when something is bound to them;
//! - two commands that cannot both be true (up and down, forward and reverse) leave a contactor
//!   hoist unfed, as a real interlock does; a drive sees both inputs and does what its macro says.
//!
//! This simulates wiring and logic. It is not a safety function and says nothing about PL or SIL.
//! Names of the modules are the device tags of their motors (the ids the loops use).

use std::collections::{BTreeMap, BTreeSet};

use novo_drive::hoist_unit::BrakeFeed;

use crate::cl_pair_loop::ClPairModule;
use crate::cl_winch_loop::ClWinchModule;
use crate::drive_hoist_loop::DriveHoistModule;
use crate::hoist_loop::HoistModule;
use crate::port::{CircuitPort, Direction, MotorFeed, WindingSel};
use crate::travel_loop::TravelModule;

/// A function a module offers to a controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Port {
    Up,
    Down,
    /// Step 2: the coarse winding, the high speed input, the second speed step.
    Fast,
    Forward,
    Reverse,
    /// Lifts the brake (a contactor hoist with its own brake feed).
    BrakeRelease,
    /// Resets a tripped drive (a rising edge).
    Reset,
    /// The E-stop or safety chain: low stops the module.
    Permit,
    /// The dead-man bar: the module moves only while it is high.
    Deadman,
}

/// How a port treats the level it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortClass {
    Command,
    Chain,
    Enable,
}

impl Port {
    pub const ALL: [Port; 9] = [
        Port::Up,
        Port::Down,
        Port::Fast,
        Port::Forward,
        Port::Reverse,
        Port::BrakeRelease,
        Port::Reset,
        Port::Permit,
        Port::Deadman,
    ];

    /// The name in a target text (`hoist/=A-M1@up`).
    pub const fn name(self) -> &'static str {
        match self {
            Port::Up => "up",
            Port::Down => "down",
            Port::Fast => "fast",
            Port::Forward => "forward",
            Port::Reverse => "reverse",
            Port::BrakeRelease => "brake_release",
            Port::Reset => "reset",
            Port::Permit => "permit",
            Port::Deadman => "deadman",
        }
    }

    pub fn parse(name: &str) -> Option<Port> {
        Port::ALL.into_iter().find(|p| p.name() == name)
    }

    /// Words for a picker.
    pub const fn label(self) -> &'static str {
        match self {
            Port::Up => "Up (raise the hook)",
            Port::Down => "Down (lower the hook)",
            Port::Fast => "Fast (step 2)",
            Port::Forward => "Forward (right, positive)",
            Port::Reverse => "Reverse (left, negative)",
            Port::BrakeRelease => "Brake release",
            Port::Reset => "Fault reset",
            Port::Permit => "Permit (E-stop chain, low stops)",
            Port::Deadman => "Dead-man (moves only while held)",
        }
    }

    pub const fn class(self) -> PortClass {
        match self {
            Port::Permit => PortClass::Chain,
            Port::Deadman => PortClass::Enable,
            _ => PortClass::Command,
        }
    }
}

/// Which engine a module is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    /// A two-speed contactor hoist (`hoist_loop`).
    Hoist,
    /// A travel axis behind a drive (`travel_loop`).
    Travel,
    /// An open-loop drive hoist (`drive_hoist_loop`).
    DriveHoist,
    /// A closed-loop winch (`cl_winch_loop`).
    ClosedWinch,
    /// Two closed-loop winches on one beam (`cl_pair_loop`).
    WinchPair,
}

impl ModuleKind {
    /// First segment of the canonical path of a module of this kind.
    pub const fn prefix(self) -> &'static str {
        match self {
            ModuleKind::Hoist | ModuleKind::DriveHoist => "hoist",
            ModuleKind::Travel => "travel",
            ModuleKind::ClosedWinch | ModuleKind::WinchPair => "winch",
        }
    }

    pub const fn words(self) -> &'static str {
        match self {
            ModuleKind::Hoist => "two-speed contactor hoist",
            ModuleKind::Travel => "travel axis on a drive",
            ModuleKind::DriveHoist => "drive hoist",
            ModuleKind::ClosedWinch => "closed-loop winch",
            ModuleKind::WinchPair => "pair of closed-loop winches",
        }
    }
}

/// The device tag a port owns when the drawing has no device for it. The circuit does not know it
/// (it reads as not energised), [`CommandedCircuit`] answers for it.
pub fn virtual_tag(module: &str, port: Port) -> String {
    format!("{VIRTUAL}{module}:{}", port.name())
}

const VIRTUAL: &str = "~cmd:";
const VIRTUAL_ON: &str = "~on:";

/// The tag of a slot where "no device" means "always on" (main contactor, safety torque off).
fn on_tag(module: &str, port: Port) -> String {
    format!("{VIRTUAL_ON}{module}:{}", port.name())
}

fn is_virtual(tag: &str) -> bool {
    tag.starts_with(VIRTUAL) || tag.starts_with(VIRTUAL_ON)
}

/// The levels of the bound ports, by module id (the device tag of the module's motor).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Commands {
    levels: BTreeMap<(String, Port), bool>,
}

impl Commands {
    pub fn new() -> Self {
        Self::default()
    }

    /// A port is bound and has this level.
    pub fn set(&mut self, module: &str, port: Port, high: bool) {
        self.levels.insert((module.to_owned(), port), high);
    }

    pub fn level(&self, module: &str, port: Port) -> Option<bool> {
        self.levels.get(&(module.to_owned(), port)).copied()
    }

    /// The ports of a module that are bound.
    pub fn bound(&self, module: &str) -> BTreeSet<Port> {
        self.levels
            .keys()
            .filter(|(m, _)| m == module)
            .map(|(_, p)| *p)
            .collect()
    }

    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    fn high(&self, module: &str, port: Port) -> bool {
        self.level(module, port).unwrap_or(false)
    }

    /// Whether the module may move: every bound permit and enable port is high.
    pub fn may_run(&self, module: &str) -> bool {
        [Port::Permit, Port::Deadman]
            .into_iter()
            .all(|p| self.level(module, p).unwrap_or(true))
    }
}

/// Where the ports of one module meet the circuit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wiring {
    pub id: String,
    pub kind: ModuleKind,
    /// The ports this module offers.
    pub ports: Vec<Port>,
    /// The device tag each command port owns (a drawn device, or a virtual tag).
    tags: BTreeMap<Port, String>,
    /// Hoist: the motor symbol whose feed the up, down and fast ports decide.
    motor: Option<String>,
    /// Brake feed tag, dropped when the permit is lost.
    brake_feed: Option<String>,
    /// Slots where an absent tag means "always on" (main contactor, safety torque off): masked by
    /// the permit. `true` marks a tag that is virtual.
    masks: Vec<(String, bool)>,
    /// Command tags the drawing has, forced off while the permit is lost.
    drawn_commands: Vec<String>,
}

impl Wiring {
    fn new(id: &str, kind: ModuleKind) -> Self {
        Wiring {
            id: id.to_owned(),
            kind,
            ports: Vec::new(),
            tags: BTreeMap::new(),
            motor: None,
            brake_feed: None,
            masks: Vec::new(),
            drawn_commands: Vec::new(),
        }
    }

    pub fn has(&self, port: Port) -> bool {
        self.ports.contains(&port)
    }

    /// The tag a slot reads: the drawn one, or a virtual one put in the slot when the port is
    /// `bound` (an unbound port leaves the slot as the drawing has it).
    fn slot(
        &mut self,
        port: Port,
        bound: &BTreeSet<Port>,
        slot: &mut Option<String>,
    ) -> Option<String> {
        if slot.is_none() && bound.contains(&port) {
            *slot = Some(virtual_tag(&self.id, port));
        }
        if let Some(tag) = slot.clone() {
            self.drawn_commands.push(tag.clone());
            self.tags.insert(port, tag.clone());
            Some(tag)
        } else {
            None
        }
    }

    /// A slot where no tag means the input is always on: with a permit or enable bound it gets a
    /// virtual tag, so the permit can switch it.
    fn mask(&mut self, bound: &BTreeSet<Port>, slot: &mut Option<String>, port: Port) {
        let gated = bound.contains(&Port::Permit) || bound.contains(&Port::Deadman);
        if slot.is_none() && gated {
            *slot = Some(on_tag(&self.id, port));
        }
        if let Some(tag) = slot.clone() {
            let virtual_ = is_virtual(&tag);
            self.masks.push((tag, virtual_));
        }
    }

    /// A two-speed contactor hoist. The module id is its motor tag.
    pub fn hoist(m: &mut HoistModule, bound: &BTreeSet<Port>) -> Wiring {
        let mut w = Wiring::new(&m.id, ModuleKind::Hoist);
        w.ports = vec![Port::Up, Port::Down, Port::Fast];
        w.motor = Some(m.binding.motor.clone());
        if m.unit.brake_feed() == BrakeFeed::Separate {
            w.ports.push(Port::BrakeRelease);
            let mut slot = m.binding.brake_feed.take();
            let tag = w.slot(Port::BrakeRelease, bound, &mut slot);
            m.binding.brake_feed = slot;
            w.brake_feed = tag;
        }
        w.ports.extend([Port::Permit, Port::Deadman]);
        let mut mains = m.binding.mains.take();
        w.mask(bound, &mut mains, Port::Permit);
        m.binding.mains = mains;
        w
    }

    /// A travel axis behind a drive. The macro order of the inputs is forward, reverse, step 2.
    pub fn travel(m: &mut TravelModule, bound: &BTreeSet<Port>) -> Wiring {
        let mut w = Wiring::new(&m.id, ModuleKind::Travel);
        w.ports = vec![
            Port::Forward,
            Port::Reverse,
            Port::Fast,
            Port::Permit,
            Port::Deadman,
        ];
        if m.binding.inputs.len() < 3 {
            m.binding.inputs.resize(3, None);
        }
        for (i, port) in [Port::Forward, Port::Reverse, Port::Fast]
            .into_iter()
            .enumerate()
        {
            let mut slot = m.binding.inputs[i].take();
            w.slot(port, bound, &mut slot);
            m.binding.inputs[i] = slot;
        }
        let mut mains = m.binding.mains.take();
        w.mask(bound, &mut mains, Port::Permit);
        m.binding.mains = mains;
        w
    }

    /// An open-loop drive hoist.
    pub fn drive_hoist(m: &mut DriveHoistModule, bound: &BTreeSet<Port>) -> Wiring {
        let mut w = Wiring::new(&m.id, ModuleKind::DriveHoist);
        w.ports = vec![Port::Up, Port::Down];
        for (port, field) in [(Port::Up, 0usize), (Port::Down, 1)] {
            let mut slot = if field == 0 {
                m.binding.up.take()
            } else {
                m.binding.down.take()
            };
            w.slot(port, bound, &mut slot);
            if field == 0 {
                m.binding.up = slot;
            } else {
                m.binding.down = slot;
            }
        }
        if !m.binding.speed_inputs.is_empty() {
            w.ports.push(Port::Fast);
            let mut slot = m.binding.speed_inputs[0].take();
            w.slot(Port::Fast, bound, &mut slot);
            m.binding.speed_inputs[0] = slot;
        }
        w.ports.push(Port::Reset);
        let mut reset = m.binding.reset.take();
        w.slot(Port::Reset, bound, &mut reset);
        m.binding.reset = reset;
        w.ports.extend([Port::Permit, Port::Deadman]);
        let mut mains = m.binding.mains.take();
        w.mask(bound, &mut mains, Port::Permit);
        m.binding.mains = mains;
        w
    }

    /// A closed-loop winch.
    pub fn closed_winch(m: &mut ClWinchModule, bound: &BTreeSet<Port>) -> Wiring {
        let mut w = Wiring::new(&m.id, ModuleKind::ClosedWinch);
        w.ports = vec![Port::Up, Port::Down, Port::Fast, Port::Reset];
        for port in [Port::Up, Port::Down, Port::Fast, Port::Reset] {
            let mut slot = match port {
                Port::Up => m.binding.up.take(),
                Port::Down => m.binding.down.take(),
                Port::Fast => m.binding.fast.take(),
                _ => m.binding.reset.take(),
            };
            w.slot(port, bound, &mut slot);
            match port {
                Port::Up => m.binding.up = slot,
                Port::Down => m.binding.down = slot,
                Port::Fast => m.binding.fast = slot,
                _ => m.binding.reset = slot,
            }
        }
        w.ports.extend([Port::Permit, Port::Deadman]);
        for i in 0..2 {
            let mut sto = m.binding.sto[i].take();
            w.mask(bound, &mut sto, Port::Permit);
            m.binding.sto[i] = sto;
        }
        let mut mains = m.binding.mains.take();
        w.mask(bound, &mut mains, Port::Permit);
        m.binding.mains = mains;
        w
    }
}

impl Wiring {
    /// Two closed-loop winches on one beam. Winch A's up, down and fast command the pair.
    pub fn closed_pair(m: &mut ClPairModule, bound: &BTreeSet<Port>) -> Wiring {
        let mut w = Wiring::new(&m.id, ModuleKind::WinchPair);
        w.ports = vec![Port::Up, Port::Down, Port::Fast];
        for port in [Port::Up, Port::Down, Port::Fast] {
            let b = &mut m.bindings[0];
            let mut slot = match port {
                Port::Up => b.up.take(),
                Port::Down => b.down.take(),
                _ => b.fast.take(),
            };
            w.slot(port, bound, &mut slot);
            match port {
                Port::Up => b.up = slot,
                Port::Down => b.down = slot,
                _ => b.fast = slot,
            }
        }
        w.ports.extend([Port::Permit, Port::Deadman]);
        for b in &mut m.bindings {
            for i in 0..2 {
                let mut sto = b.sto[i].take();
                w.mask(bound, &mut sto, Port::Permit);
                b.sto[i] = sto;
            }
            let mut mains = b.mains.take();
            w.mask(bound, &mut mains, Port::Permit);
            b.mains = mains;
        }
        w
    }
}

/// The feed a contactor hoist motor gets from its commands: up and down together are an interlock
/// fault and feed nothing; the winding is the coarse one with `fast`, the fine one without.
pub fn hoist_feed(up: bool, down: bool, fast: bool) -> MotorFeed {
    if up == down {
        return MotorFeed::Off;
    }
    MotorFeed::Fed {
        winding: if fast {
            WindingSel::Coarse
        } else {
            WindingSel::Fine
        },
        direction: if up {
            Direction::Positive
        } else {
            Direction::Negative
        },
    }
}

/// The circuit the loops step against, with the bound ports answering for the tags they own.
pub struct CommandedCircuit<'a, C: CircuitPort> {
    inner: &'a mut C,
    forced: BTreeMap<String, bool>,
    masks: BTreeMap<String, (bool, bool)>,
    feeds: BTreeMap<String, MotorFeed>,
}

impl<'a, C: CircuitPort> CommandedCircuit<'a, C> {
    /// `wirings` are those of the modules with a bound port (see the constructors of [`Wiring`]),
    /// `commands` the levels now.
    pub fn new(inner: &'a mut C, wirings: &[Wiring], commands: &Commands) -> Self {
        let mut forced = BTreeMap::new();
        let mut masks = BTreeMap::new();
        let mut feeds = BTreeMap::new();
        for w in wirings {
            let run = commands.may_run(&w.id);
            let bound = commands.bound(&w.id);
            for (port, tag) in &w.tags {
                if let Some(level) = commands.level(&w.id, *port) {
                    forced.insert(tag.clone(), level && run);
                }
            }
            // With a permit or enable bound, the drawn command inputs are held off while it is lost.
            if !run {
                for tag in &w.drawn_commands {
                    forced.insert(tag.clone(), false);
                }
                if let Some(tag) = &w.brake_feed {
                    forced.insert(tag.clone(), false);
                }
            }
            for (tag, virtual_) in &w.masks {
                masks.insert(tag.clone(), (run, *virtual_));
            }
            if let Some(motor) = &w.motor {
                if !run {
                    feeds.insert(motor.clone(), MotorFeed::Off);
                } else if bound.contains(&Port::Up) || bound.contains(&Port::Down) {
                    feeds.insert(
                        motor.clone(),
                        hoist_feed(
                            commands.high(&w.id, Port::Up),
                            commands.high(&w.id, Port::Down),
                            commands.high(&w.id, Port::Fast),
                        ),
                    );
                }
            }
        }
        Self {
            inner,
            forced,
            masks,
            feeds,
        }
    }
}

impl<C: CircuitPort> CircuitPort for CommandedCircuit<'_, C> {
    fn settle(&mut self, dt_s: f64) {
        self.inner.settle(dt_s);
    }

    fn motor_feed(&self, motor: &str) -> MotorFeed {
        self.feeds
            .get(motor)
            .copied()
            .unwrap_or_else(|| self.inner.motor_feed(motor))
    }

    fn is_energised(&self, device: &str) -> bool {
        if let Some(level) = self.forced.get(device) {
            return *level;
        }
        match self.masks.get(device) {
            Some((ok, true)) => *ok,
            Some((ok, false)) => *ok && self.inner.is_energised(device),
            // A virtual tag left in a slot after its port was unbound: an input reads off, a
            // "no device" slot of the main contactor or safety channel reads on, as before.
            None if device.starts_with(VIRTUAL_ON) => true,
            None if device.starts_with(VIRTUAL) => false,
            None => self.inner.is_energised(device),
        }
    }

    fn set_contact(&mut self, device: &str, closed: bool) {
        if !is_virtual(device) {
            self.inner.set_contact(device, closed);
        }
    }

    fn knows_device(&self, device: &str) -> bool {
        is_virtual(device) || self.inner.knows_device(device)
    }

    fn motor_humming(&self, motor: &str) -> bool {
        !self.feeds.contains_key(motor) && self.inner.motor_humming(motor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cosim::StepConfig;
    use crate::hoist_loop::HoistLoop;
    use crate::port::FakeCircuit;
    use crate::spec::hoist_from_properties;
    use crate::travel_loop::TravelLoop;
    use crate::travel_spec::travel_from_properties;

    const DT: f64 = 0.005;

    fn props(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: BTreeMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        move |k| map.get(k).cloned()
    }

    fn hoist_loop(extra: &[(&str, &str)]) -> HoistLoop {
        let mut pairs = vec![("mech_mass_kg", "100")];
        pairs.extend_from_slice(extra);
        let get = props(&pairs);
        let module = hoist_from_properties("=M1", "=M1", &get).unwrap().unwrap();
        let mut lp = HoistLoop::new(StepConfig::new(DT).unwrap()).unwrap();
        lp.hoists.push(module);
        lp
    }

    /// Steps `seconds` with the commands, re-attaching the wiring every tick as the host does.
    fn run_hoist(lp: &mut HoistLoop, c: &Commands, seconds: f64) {
        let mut inner = FakeCircuit::new();
        for _ in 0..(seconds / DT).round() as usize {
            let wirings: Vec<Wiring> = lp
                .hoists
                .iter_mut()
                .filter(|h| !c.bound(&h.id).is_empty())
                .map(|h| Wiring::hoist(h, &c.bound(&h.id)))
                .collect();
            let mut circuit = CommandedCircuit::new(&mut inner, &wirings, c);
            lp.step(&mut circuit);
        }
    }

    fn cmd(pairs: &[(Port, bool)]) -> Commands {
        let mut c = Commands::new();
        for (p, v) in pairs {
            c.set("=M1", *p, *v);
        }
        c
    }

    fn height(lp: &HoistLoop) -> f64 {
        lp.hoists[0].height_m()
    }

    #[test]
    fn port_names_round_trip() {
        for p in Port::ALL {
            assert_eq!(Port::parse(p.name()), Some(p));
        }
        assert_eq!(Port::parse("sideways"), None);
    }

    #[test]
    fn a_hoist_with_no_command_stays_put() {
        let mut lp = hoist_loop(&[]);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, false), (Port::Down, false)]),
            2.0,
        );
        assert_eq!(height(&lp), 0.0);
    }

    #[test]
    fn up_lifts_the_hook_release_stops_it_and_down_lowers_it() {
        let mut lp = hoist_loop(&[]);
        run_hoist(&mut lp, &cmd(&[(Port::Up, true)]), 3.0);
        let up = height(&lp);
        assert!(up > 0.05, "the hook rose {up} m");
        // released: the brake applies and the hook comes to rest
        run_hoist(&mut lp, &cmd(&[(Port::Up, false)]), 3.0);
        let rest = height(&lp);
        run_hoist(&mut lp, &cmd(&[(Port::Up, false)]), 1.0);
        assert_eq!(height(&lp), rest, "the hook holds after release");
        assert!(rest >= up);
        // down
        run_hoist(&mut lp, &cmd(&[(Port::Down, true)]), 3.0);
        assert!(height(&lp) < rest - 0.05);
    }

    #[test]
    fn fast_selects_the_coarse_winding_and_goes_faster() {
        let mut slow = hoist_loop(&[]);
        run_hoist(
            &mut slow,
            &cmd(&[(Port::Up, true), (Port::Fast, false)]),
            4.0,
        );
        let mut fast = hoist_loop(&[]);
        run_hoist(
            &mut fast,
            &cmd(&[(Port::Up, true), (Port::Fast, true)]),
            4.0,
        );
        assert!(
            height(&fast) > 3.0 * height(&slow),
            "{} vs {}",
            height(&fast),
            height(&slow)
        );
    }

    #[test]
    fn up_and_down_together_feed_nothing() {
        let mut lp = hoist_loop(&[]);
        run_hoist(&mut lp, &cmd(&[(Port::Up, true), (Port::Down, true)]), 2.0);
        assert_eq!(height(&lp), 0.0);
    }

    #[test]
    fn a_lost_permit_stops_the_hoist_and_it_runs_again_when_the_chain_closes() {
        let mut lp = hoist_loop(&[]);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Permit, true)]),
            2.0,
        );
        let before = height(&lp);
        assert!(before > 0.0);
        // E-stop: permit low while up is still held
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Permit, false)]),
            3.0,
        );
        let stopped = height(&lp);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Permit, false)]),
            2.0,
        );
        assert_eq!(height(&lp), stopped, "no movement while the chain is open");
        assert!(stopped - before < 0.3, "it only coasts a little");
        // chain closed again
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Permit, true)]),
            2.0,
        );
        assert!(height(&lp) > stopped + 0.05);
    }

    #[test]
    fn the_deadman_must_be_held_for_any_motion() {
        let mut lp = hoist_loop(&[]);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Deadman, false)]),
            3.0,
        );
        assert_eq!(height(&lp), 0.0, "no motion without the bar");
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Deadman, true)]),
            3.0,
        );
        assert!(height(&lp) > 0.05);
        let moving = height(&lp);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Deadman, false)]),
            3.0,
        );
        let rest = height(&lp);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::Deadman, false)]),
            1.0,
        );
        assert_eq!(height(&lp), rest);
        assert!(rest - moving < 0.3);
    }

    #[test]
    fn the_run_is_deterministic() {
        let go = || {
            let mut lp = hoist_loop(&[]);
            run_hoist(&mut lp, &cmd(&[(Port::Up, true), (Port::Fast, true)]), 2.5);
            run_hoist(&mut lp, &cmd(&[(Port::Down, true)]), 1.5);
            height(&lp).to_bits()
        };
        assert_eq!(go(), go());
    }

    #[test]
    fn a_separate_brake_feed_is_a_port_and_a_motor_terminal_brake_is_not() {
        let mut plain = hoist_loop(&[]);
        let w = Wiring::hoist(&mut plain.hoists[0], &BTreeSet::new());
        assert!(!w.has(Port::BrakeRelease));
        let mut separate = hoist_loop(&[("mech_brake_feed", "=K9")]);
        let w = Wiring::hoist(&mut separate.hoists[0], &BTreeSet::new());
        assert!(w.has(Port::BrakeRelease));
        assert_eq!(
            separate.hoists[0].binding.brake_feed.as_deref(),
            Some("=K9")
        );
    }

    #[test]
    fn a_bound_brake_release_on_a_separate_feed_lifts_the_brake() {
        let mut lp = hoist_loop(&[("mech_brake_feed", "=K9")]);
        // motor fed, brake not released: the hook does not move
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::BrakeRelease, false)]),
            2.0,
        );
        let stuck = height(&lp);
        run_hoist(
            &mut lp,
            &cmd(&[(Port::Up, true), (Port::BrakeRelease, true)]),
            3.0,
        );
        assert!(height(&lp) > stuck + 0.05);
    }

    #[test]
    fn unbound_wiring_changes_nothing_in_the_module() {
        let mut lp = hoist_loop(&[]);
        let before = lp.hoists[0].binding.clone();
        let _ = Wiring::hoist(&mut lp.hoists[0], &BTreeSet::new());
        assert_eq!(lp.hoists[0].binding, before);
    }

    fn travel_loop() -> TravelLoop {
        let get = props(&[("trav_mass_kg", "2000")]);
        let (module, _) = travel_from_properties("=M2", &get).unwrap().unwrap();
        let mut lp = TravelLoop::new(StepConfig::new(DT).unwrap()).unwrap();
        lp.modules.push(module);
        lp
    }

    fn run_travel(lp: &mut TravelLoop, c: &Commands, seconds: f64) {
        let mut inner = FakeCircuit::new();
        for _ in 0..(seconds / DT).round() as usize {
            let wirings: Vec<Wiring> = lp
                .modules
                .iter_mut()
                .filter(|m| !c.bound(&m.id).is_empty())
                .map(|m| Wiring::travel(m, &c.bound(&m.id)))
                .collect();
            let mut circuit = CommandedCircuit::new(&mut inner, &wirings, c);
            lp.step(&mut circuit);
        }
    }

    fn tcmd(pairs: &[(Port, bool)]) -> Commands {
        let mut c = Commands::new();
        for (p, v) in pairs {
            c.set("=M2", *p, *v);
        }
        c
    }

    fn position(lp: &TravelLoop) -> f64 {
        lp.modules[0].position_m()
    }

    #[test]
    fn forward_drives_the_axis_reverse_brings_it_back_and_release_stops_it() {
        let mut lp = travel_loop();
        run_travel(&mut lp, &tcmd(&[(Port::Forward, true)]), 6.0);
        let there = position(&lp);
        assert!(there > 0.1, "{there}");
        run_travel(&mut lp, &tcmd(&[(Port::Forward, false)]), 12.0);
        let rest = position(&lp);
        run_travel(&mut lp, &tcmd(&[(Port::Forward, false)]), 2.0);
        assert!(
            (position(&lp) - rest).abs() < 1e-3,
            "it has come to rest after the ramp"
        );
        run_travel(&mut lp, &tcmd(&[(Port::Reverse, true)]), 6.0);
        assert!(position(&lp) < rest - 0.1);
    }

    #[test]
    fn step_2_is_faster_than_step_1_on_a_travel_axis() {
        let mut a = travel_loop();
        run_travel(
            &mut a,
            &tcmd(&[(Port::Forward, true), (Port::Fast, false)]),
            10.0,
        );
        let mut b = travel_loop();
        run_travel(
            &mut b,
            &tcmd(&[(Port::Forward, true), (Port::Fast, true)]),
            10.0,
        );
        assert!(
            position(&b) > 2.0 * position(&a),
            "{} {}",
            position(&b),
            position(&a)
        );
    }

    #[test]
    fn a_lost_permit_or_deadman_stops_a_travel_axis_and_it_waits_until_both_are_back() {
        for gate in [Port::Permit, Port::Deadman] {
            let mut lp = travel_loop();
            run_travel(&mut lp, &tcmd(&[(Port::Forward, true), (gate, false)]), 5.0);
            assert_eq!(
                position(&lp),
                0.0,
                "{gate:?}: no motion while the gate is low"
            );
            run_travel(&mut lp, &tcmd(&[(Port::Forward, true), (gate, true)]), 5.0);
            let moved = position(&lp);
            assert!(moved > 0.05, "{gate:?}: {moved}");
            run_travel(
                &mut lp,
                &tcmd(&[(Port::Forward, true), (gate, false)]),
                15.0,
            );
            let stopped = position(&lp);
            run_travel(&mut lp, &tcmd(&[(Port::Forward, true), (gate, false)]), 3.0);
            assert!((position(&lp) - stopped).abs() < 1e-3, "{gate:?}");
        }
    }

    #[test]
    fn the_virtual_tags_are_known_and_never_reach_the_drawing() {
        let mut inner = FakeCircuit::new();
        let mut c = Commands::new();
        c.set("=M2", Port::Forward, true);
        let mut lp = travel_loop();
        let w = Wiring::travel(&mut lp.modules[0], &c.bound("=M2"));
        let circuit = CommandedCircuit::new(&mut inner, &[w], &c);
        let tag = virtual_tag("=M2", Port::Forward);
        assert!(circuit.knows_device(&tag));
        assert!(circuit.is_energised(&tag));
        assert!(
            lp.check_bindings(&circuit)
                .iter()
                .all(|f| !f.message.contains("~cmd"))
        );
    }
}
