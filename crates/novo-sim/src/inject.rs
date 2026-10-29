//! Fault injection for the live circuit (lane F1, design `docs/design/fault-injection.md`).
//!
//! A [`Defect`] says what has gone wrong in the plant, in the words an electrician uses and by
//! the printed names of the drawing: a contactor main contact that welded (`-K1`), a coil that
//! is open (`-K2`), a conductor that broke at `-S0:2`, a phase that is gone (`L2`), a pin that
//! leaks to earth. A [`DefectSet`] is what is wrong right now. [`DefectSet::prepare`] resolves
//! the names against the drawing once and gives a [`Prepared`] that settles the circuit with the
//! defects in it, with the same coupling of coils, contacts, seal-ins and timers as the healthy
//! bench ([`crate::settle_faulted`]); nothing is stored in the project.
//!
//! This models wiring and logic. It is not a safety analysis: it does not know failure rates,
//! diagnostic coverage or what a real device does after it failed, only what the drawn circuit
//! does when this one thing is broken.

use std::collections::BTreeSet;
use std::sync::Arc;

use novo_core::{AnyId, Endpoint, Netlist, PotentialId, PotentialKind, Project, SymbolId};
use serde::{Deserialize, Serialize};

use crate::pins::{
    PinProblem, PoleRef, device_by_text as device_exact, poles_of_device as poles_exact,
};
use crate::{Clock, Faults, PoleState, Settled, is_coil, settle_faulted};

/// One thing that is wrong. Devices are named by tag (`-K1`, `=A+B-K1`), pins as `tag:pin`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "defect", rename_all = "snake_case")]
pub enum Defect {
    /// A contact welded closed: it conducts whatever its coil or hand does. With `pin` only the
    /// pole that pin belongs to (also a break contact, which then never opens: a defeated
    /// interlock); without, every make contact of the device (a contactor's mains and its
    /// auxiliary make contacts).
    ContactWelded {
        device: String,
        #[serde(default)]
        pin: Option<String>,
        /// The break contacts of the device are positively guided (mirror contacts): a welded
        /// make contact keeps them open. Off for a plain auxiliary block, where a welded main
        /// contact leaves the break contacts to close as drawn. Only without `pin`.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        mirror: bool,
    },
    /// A contact that no longer closes (burnt, dirty, broken spring): stays open.
    ContactStuckOpen {
        device: String,
        #[serde(default)]
        pin: Option<String>,
    },
    /// A protective device has tripped (overload relay, motor protection switch, drive fault
    /// relay): all its switched contacts are open and stay open.
    Tripped { device: String },
    /// The winding of a coil is open: the coil never operates (a brake that does not release,
    /// a contactor that does not pull in).
    CoilOpen { device: String },
    /// The coil operates whatever feeds it (jammed armature, a driver output that failed on).
    CoilStuckOn { device: String },
    /// The conductor is broken at this pin: every wire ending on it is cut (a loose terminal,
    /// an E-stop chain that opened, a lost phase at one device).
    ConductorOpen { at: String },
    /// The wire with this wire number is broken.
    WireBroken { number: String },
    /// A supply is gone: the potential (`L2`, `+24V`) reaches nothing (a fuse or a phase of the
    /// grid lost).
    PhaseLost { potential: String },
    /// The pin touches protective earth. A conductor that carries a potential is then shorted to
    /// earth; a coil with one side earthed in a control circuit may pull in on its own.
    EarthFault { at: String },
    /// The pin touches this potential (insulation breakdown between two conductors).
    ShortTo { at: String, potential: String },
}

impl Defect {
    /// A short name for lists: "Contact welded -K1".
    pub fn label(&self) -> String {
        match self {
            Defect::ContactWelded { device, pin, .. } => {
                format!("Contact welded {}", with_pin(device, pin))
            }
            Defect::ContactStuckOpen { device, pin } => {
                format!("Contact stuck open {}", with_pin(device, pin))
            }
            Defect::Tripped { device } => format!("Tripped {device}"),
            Defect::CoilOpen { device } => format!("Coil open {device}"),
            Defect::CoilStuckOn { device } => format!("Coil stuck on {device}"),
            Defect::ConductorOpen { at } => format!("Conductor open at {at}"),
            Defect::WireBroken { number } => format!("Wire {number} broken"),
            Defect::PhaseLost { potential } => format!("Supply {potential} lost"),
            Defect::EarthFault { at } => format!("Earth fault at {at}"),
            Defect::ShortTo { at, potential } => format!("{at} shorted to {potential}"),
        }
    }

    /// What the defect is, one sentence, for the explanation list.
    pub fn meaning(&self) -> String {
        match self {
            Defect::ContactWelded { device, pin, .. } => format!(
                "{} conducts all the time, whatever its coil or the operator does.",
                with_pin(device, pin)
            ),
            Defect::ContactStuckOpen { device, pin } => format!(
                "{} never conducts, even when its coil is on.",
                with_pin(device, pin)
            ),
            Defect::Tripped { device } => {
                format!("{device} has tripped: its contacts stay open until it is reset.")
            }
            Defect::CoilOpen { device } => {
                format!("The winding of {device} is open: it never operates.")
            }
            Defect::CoilStuckOn { device } => {
                format!("{device} operates all the time, whatever feeds its coil.")
            }
            Defect::ConductorOpen { at } => format!("Every wire at {at} is cut."),
            Defect::WireBroken { number } => format!("Wire {number} is cut."),
            Defect::PhaseLost { potential } => {
                format!("{potential} is gone: nothing is fed from it.")
            }
            Defect::EarthFault { at } => format!("{at} touches protective earth."),
            Defect::ShortTo { at, potential } => format!("{at} touches {potential}."),
        }
    }

    /// The name of the device this defect sits on, when it has one.
    pub fn device(&self) -> Option<&str> {
        match self {
            Defect::ContactWelded { device, .. }
            | Defect::ContactStuckOpen { device, .. }
            | Defect::Tripped { device }
            | Defect::CoilOpen { device }
            | Defect::CoilStuckOn { device } => Some(device),
            _ => None,
        }
    }
}

fn with_pin(device: &str, pin: &Option<String>) -> String {
    match pin {
        Some(p) => format!("{device}:{p}"),
        None => device.to_owned(),
    }
}

/// The defects that are in the plant right now.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefectSet {
    defects: Vec<Defect>,
}

impl DefectSet {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn defects(&self) -> &[Defect] {
        &self.defects
    }

    pub fn is_empty(&self) -> bool {
        self.defects.is_empty()
    }

    /// Adds a defect (once).
    pub fn inject(&mut self, defect: Defect) {
        if !self.defects.contains(&defect) {
            self.defects.push(defect);
        }
    }

    /// Removes one defect; `true` when it was there.
    pub fn clear(&mut self, defect: &Defect) -> bool {
        let before = self.defects.len();
        self.defects.retain(|d| d != defect);
        before != self.defects.len()
    }

    pub fn clear_all(&mut self) {
        self.defects.clear();
    }

    /// Resolves the names against the drawing. A defect whose name leads nowhere is reported in
    /// [`Prepared::problems`] and has no effect; it is never dropped silently.
    pub fn prepare(&self, project: &Project) -> Prepared {
        let mut faults = Faults::default();
        let mut problems = Vec::new();
        let mut cut: BTreeSet<novo_core::WireId> = BTreeSet::new();
        for defect in &self.defects {
            if let Err(why) = resolve(project, defect, &mut faults, &mut cut) {
                problems.push(format!("{}: {why}", defect.label()));
            }
        }
        let mut cut_project = None;
        if !cut.is_empty() {
            let mut copy = project.clone();
            let removed = copy.transact("Inject wire break", |tx| {
                for wire in &cut {
                    tx.remove(AnyId::from(*wire))?;
                }
                Ok(())
            });
            match removed {
                Ok(_) => cut_project = Some(copy),
                Err(err) => problems.push(format!("a wire could not be cut: {err}")),
            }
        }
        let netlist = cut_project.as_ref().unwrap_or(project).netlist();
        Prepared {
            cut_project,
            netlist,
            faults,
            problems,
        }
    }
}

/// A defect set resolved against one drawing, ready to settle. It holds no borrow of the
/// drawing, so it can be kept between frames; it is valid for the revision it was prepared on.
pub struct Prepared {
    /// The drawing with the broken wires removed; `None` when no wire is cut.
    cut_project: Option<Project>,
    netlist: Arc<Netlist>,
    faults: Faults,
    /// Defects whose names did not lead to anything in the drawing.
    pub problems: Vec<String>,
}

impl Prepared {
    /// The drawing the faulted plant is solved on: `original` with the broken wires removed
    /// (`original` itself when none are).
    pub fn project<'a>(&'a self, original: &'a Project) -> &'a Project {
        self.cut_project.as_ref().unwrap_or(original)
    }

    /// The netlist of the faulted drawing.
    pub fn netlist(&self) -> &Arc<Netlist> {
        &self.netlist
    }

    /// Whether any wire is cut (then the netlist is not the original's).
    pub fn cuts_wires(&self) -> bool {
        self.cut_project.is_some()
    }

    /// Settles the circuit with the defects in it. `original` is the drawing the set was
    /// prepared on; other arguments as [`crate::settle_timed`].
    pub fn settle(
        &self,
        original: &Project,
        manual: &PoleState,
        forced: &BTreeSet<SymbolId>,
        held: &BTreeSet<SymbolId>,
        clock: &mut Clock,
    ) -> Settled {
        settle_faulted(
            self.project(original),
            &self.netlist,
            manual,
            forced,
            held,
            clock,
            &self.faults,
        )
    }
}

type Resolved = Result<(), String>;

/// The full tag text of the device a person typed: `-K1` finds `=A1+CAB-K1` when no other
/// device ends the same way. Text that matches nothing, or more than one device, is returned as
/// typed, so the usual "no device" or "more than one" message follows.
pub fn full_name(project: &Project, text: &str) -> String {
    let text = text.trim();
    if device_exact(project, text).is_ok() || !text.starts_with('-') {
        return text.to_owned();
    }
    let mut hits = project
        .devices()
        .map(|d| d.tag.to_string())
        .filter(|t| t.ends_with(text));
    match (hits.next(), hits.next()) {
        (Some(one), None) => one,
        _ => text.to_owned(),
    }
}

/// [`full_name`] for a `tag:pin` designation.
pub fn full_pin_name(project: &Project, designation: &str) -> String {
    match designation.split_once(':') {
        Some((tag, pin)) => format!("{}:{}", full_name(project, tag), pin.trim()),
        None => designation.trim().to_owned(),
    }
}

/// The device with this tag text, accepting the short form of [`full_name`].
pub fn device_by_text(project: &Project, text: &str) -> Result<novo_core::DeviceId, PinProblem> {
    device_exact(project, &full_name(project, text))
}

/// Every switched pole of the device, accepting the short form of [`full_name`].
pub fn poles_of_device(project: &Project, text: &str) -> Result<Vec<PoleRef>, PinProblem> {
    poles_exact(project, &full_name(project, text))
}

fn resolve(
    project: &Project,
    defect: &Defect,
    faults: &mut Faults,
    cut: &mut BTreeSet<novo_core::WireId>,
) -> Resolved {
    match defect {
        Defect::ContactWelded {
            device,
            pin,
            mirror,
        } => {
            let rest = crate::rest_state(project);
            for p in contact_poles(project, device, pin)? {
                // Without a pin only the make contacts weld; a break contact is named by its pin.
                let break_contact = rest.is_closed(p.symbol, p.group);
                if pin.is_some() || !break_contact {
                    faults.welded.insert((p.symbol, p.group));
                } else if *mirror {
                    faults.stuck_open.insert((p.symbol, p.group));
                }
            }
        }
        Defect::ContactStuckOpen { device, pin } => {
            for p in contact_poles(project, device, pin)? {
                faults.stuck_open.insert((p.symbol, p.group));
            }
        }
        Defect::Tripped { device } => {
            for p in contact_poles(project, device, &None)? {
                faults.stuck_open.insert((p.symbol, p.group));
            }
        }
        Defect::CoilOpen { device } => faults.coil_open.extend(coils_of(project, device)?),
        Defect::CoilStuckOn { device } => faults.coil_stuck_on.extend(coils_of(project, device)?),
        Defect::ConductorOpen { at } => {
            let ends = pin_endpoints(project, at)?;
            let before = cut.len();
            for end in ends {
                cut.extend(project.wires_at(end));
            }
            if cut.len() == before {
                return Err(format!("no wire is connected at {at}"));
            }
        }
        Defect::WireBroken { number } => {
            let found: Vec<_> = project
                .wires()
                .filter(|w| w.number.as_deref().map(str::trim) == Some(number.trim()))
                .map(|w| w.id)
                .collect();
            if found.is_empty() {
                return Err(format!("no wire is numbered {number}"));
            }
            cut.extend(found);
        }
        Defect::PhaseLost { potential } => {
            faults
                .defects
                .lost
                .insert(potential_named(project, potential)?);
        }
        Defect::EarthFault { at } => {
            let earth = project
                .potentials()
                .find(|p| p.kind == PotentialKind::ProtectiveEarth)
                .map(|p| p.id)
                .ok_or("the project has no protective earth potential")?;
            for end in pin_endpoints(project, at)? {
                faults.defects.leaks.push((end, earth));
            }
        }
        Defect::ShortTo { at, potential } => {
            let target = potential_named(project, potential)?;
            for end in pin_endpoints(project, at)? {
                faults.defects.leaks.push((end, target));
            }
        }
    }
    Ok(())
}

fn potential_named(project: &Project, name: &str) -> Result<PotentialId, String> {
    let name = name.trim();
    project
        .potentials()
        .find(|p| p.name.eq_ignore_ascii_case(name))
        .map(|p| p.id)
        .ok_or_else(|| format!("no potential named {name}"))
}

fn problem(p: PinProblem) -> String {
    p.to_string()
}

/// The switched poles a contact defect applies to.
fn contact_poles(
    project: &Project,
    device: &str,
    pin: &Option<String>,
) -> Result<Vec<PoleRef>, String> {
    match pin {
        Some(pin) => crate::pins::pole_of_pin(
            project,
            &full_pin_name(project, &format!("{}:{pin}", device.trim())),
        )
        .map(|p| vec![p])
        .map_err(problem),
        None => poles_of_device(project, device).map_err(problem),
    }
}

/// The coil symbols of the device with this tag.
fn coils_of(project: &Project, device: &str) -> Result<Vec<SymbolId>, String> {
    let id = device_by_text(project, device).map_err(problem)?;
    let coils: Vec<SymbolId> = project
        .symbols_of_device(id)
        .into_iter()
        .filter(|s| is_coil(s))
        .map(|s| s.id)
        .collect();
    if coils.is_empty() {
        Err(format!("{} has no coil", device.trim()))
    } else {
        Ok(coils)
    }
}

/// Every endpoint of the pin `tag:pin` (a device may draw the pin on more than one symbol).
pub fn pin_endpoints(project: &Project, designation: &str) -> Result<Vec<Endpoint>, String> {
    let (tag, pin) = designation
        .split_once(':')
        .ok_or_else(|| PinProblem::NotAPin(designation.to_owned()).to_string())?;
    let (tag, pin) = (tag.trim(), pin.trim());
    let device = device_by_text(project, tag).map_err(problem)?;
    let mut found = Vec::new();
    for symbol in project.symbols_of_device(device) {
        for (index, p) in symbol.pins.iter().enumerate() {
            if p.name == pin {
                found.push(Endpoint::pin(
                    symbol.id,
                    u16::try_from(index).expect("pin count fits u16"),
                ));
            }
        }
    }
    if found.is_empty() {
        Err(PinProblem::NoPin {
            device: tag.to_owned(),
            pin: pin.to_owned(),
        }
        .to_string())
    } else {
        Ok(found)
    }
}

/// What a person picks to build a [`Defect`]: the kind, then one or two targets from the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefectKind {
    ContactWelded,
    ContactStuckOpen,
    Tripped,
    CoilOpen,
    CoilStuckOn,
    ConductorOpen,
    WireBroken,
    PhaseLost,
    EarthFault,
    ShortTo,
}

/// What a [`DefectKind`] needs to be pointed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// A device with switched contacts.
    Contacts,
    /// A device with a coil.
    Coil,
    /// A pin, `tag:pin`.
    Pin,
    /// A numbered wire.
    Wire,
    /// A potential.
    Potential,
    /// A pin and a potential.
    PinAndPotential,
}

impl DefectKind {
    pub const ALL: [DefectKind; 10] = [
        DefectKind::ContactWelded,
        DefectKind::ContactStuckOpen,
        DefectKind::Tripped,
        DefectKind::CoilOpen,
        DefectKind::CoilStuckOn,
        DefectKind::ConductorOpen,
        DefectKind::WireBroken,
        DefectKind::PhaseLost,
        DefectKind::EarthFault,
        DefectKind::ShortTo,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DefectKind::ContactWelded => "Contact welded closed",
            DefectKind::ContactStuckOpen => "Contact stuck open",
            DefectKind::Tripped => "Protective device tripped",
            DefectKind::CoilOpen => "Coil open (never operates)",
            DefectKind::CoilStuckOn => "Coil stuck on",
            DefectKind::ConductorOpen => "Conductor broken at a pin",
            DefectKind::WireBroken => "Wire broken (by number)",
            DefectKind::PhaseLost => "Supply lost (phase, fuse)",
            DefectKind::EarthFault => "Earth fault at a pin",
            DefectKind::ShortTo => "Pin shorted to a potential",
        }
    }

    pub fn target(self) -> Target {
        match self {
            DefectKind::ContactWelded | DefectKind::ContactStuckOpen | DefectKind::Tripped => {
                Target::Contacts
            }
            DefectKind::CoilOpen | DefectKind::CoilStuckOn => Target::Coil,
            DefectKind::ConductorOpen | DefectKind::EarthFault => Target::Pin,
            DefectKind::WireBroken => Target::Wire,
            DefectKind::PhaseLost => Target::Potential,
            DefectKind::ShortTo => Target::PinAndPotential,
        }
    }

    /// Builds the defect. `target` is the device, pin or wire number; `potential` the second
    /// target of [`Target::PinAndPotential`] and the only one of [`Target::Potential`].
    pub fn build(self, target: &str, potential: &str) -> Defect {
        let (t, p) = (target.trim().to_owned(), potential.trim().to_owned());
        match self {
            DefectKind::ContactWelded => Defect::ContactWelded {
                device: t,
                pin: None,
                mirror: false,
            },
            DefectKind::ContactStuckOpen => Defect::ContactStuckOpen {
                device: t,
                pin: None,
            },
            DefectKind::Tripped => Defect::Tripped { device: t },
            DefectKind::CoilOpen => Defect::CoilOpen { device: t },
            DefectKind::CoilStuckOn => Defect::CoilStuckOn { device: t },
            DefectKind::ConductorOpen => Defect::ConductorOpen { at: t },
            DefectKind::WireBroken => Defect::WireBroken { number: t },
            DefectKind::PhaseLost => Defect::PhaseLost { potential: p },
            DefectKind::EarthFault => Defect::EarthFault { at: t },
            DefectKind::ShortTo => Defect::ShortTo {
                at: t,
                potential: p,
            },
        }
    }
}

/// The tags of the devices a kind can be pointed at, sorted: those with a coil or with switched
/// contacts. For pin targets use [`devices_with_pins`] and [`pins_of`].
pub fn devices_for(project: &Project, target: Target) -> Vec<String> {
    let mut out: Vec<String> = project
        .devices()
        .filter(|d| {
            let symbols = project.symbols_of_device(d.id);
            match target {
                Target::Coil => symbols.iter().any(|s| is_coil(s)),
                Target::Contacts => symbols.iter().any(|s| !s.pass_through.is_empty()),
                _ => !symbols.is_empty(),
            }
        })
        .map(|d| d.tag.to_string())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The designations of the pins of one device, as printed, in drawing order and without repeats.
pub fn pins_of(project: &Project, device: &str) -> Vec<String> {
    let Ok(id) = device_by_text(project, device) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for symbol in project.symbols_of_device(id) {
        for pin in &symbol.pins {
            if !pin.name.is_empty() && !out.contains(&pin.name) {
                out.push(pin.name.clone());
            }
        }
    }
    out
}

/// The wire numbers in use, sorted.
pub fn wire_numbers(project: &Project) -> Vec<String> {
    let mut out: Vec<String> = project
        .wires()
        .filter_map(|w| w.number.clone())
        .filter(|n| !n.trim().is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The names of the potentials of the project, in the order they were made.
pub fn potential_names(project: &Project) -> Vec<String> {
    project.potentials().map(|p| p.name.clone()).collect()
}
