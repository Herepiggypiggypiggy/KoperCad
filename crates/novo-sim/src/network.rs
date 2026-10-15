//! The drawing as an analog circuit: what voltage every net carries and what
//! current every load draws, solved by [`analog`](crate::analog) for the
//! poles closed right now.
//!
//! Sources come from the project's potentials (voltage, frequency, phase from
//! the L1/L2/L3 names), closed poles are switches, and the loads are built
//! from the device data: a motor from its nameplate, a lamp or valve from its
//! power. Everything the drawing does not say is either assumed with a
//! stated typical value or left out, and every such decision is listed in
//! [`Analysis::notes`] so the view can show it. Nothing here is stored.
//!
//! Limits, stated in the notes too: conductors are ideal (their length is
//! not known to the schematic), loads are linear (a motor at its rated
//! point), and only one AC frequency is solved.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use novo_core::{
    Endpoint, NetId, Netlist, PotentialId, PotentialKind, Project, SymbolId, SymbolInstance, Value,
};

use crate::analog::motor::{Connection, Motor, Nameplate};
use crate::analog::{self, Circuit, Complex, GROUND, Node, Wave};
use crate::{PoleState, is_coil, is_lamp, is_load, is_motor};

/// What a net carries: the DC voltage and the AC RMS phasor, both relative
/// to the earth reference (the PE/N potentials, else the supply midpoint).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Reading {
    pub dc: f64,
    pub ac: Complex,
}

impl Reading {
    /// Volts RMS (AC) or volts (DC), whichever the net carries.
    pub fn magnitude(&self) -> f64 {
        self.ac.abs().max(self.dc.abs())
    }
}

/// What a load draws.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LoadFlow {
    pub symbol: SymbolId,
    /// Line current, A (RMS for AC).
    pub current: f64,
    /// Active power, W.
    pub power_w: f64,
    /// Reactive power, var (0 for DC).
    pub reactive_var: f64,
}

/// What a supply potential delivers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SupplyFlow {
    pub potential: PotentialId,
    pub current: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoteKind {
    /// A value the drawing lacks was filled in with a typical one.
    Assumed,
    /// A device could not be simulated for lack of data.
    Missing,
    /// The drawing contradicts itself.
    Conflict,
    /// A modelling limit the user should know about.
    Limit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Note {
    pub kind: NoteKind,
    pub symbol: Option<SymbolId>,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    /// Indexed by `NetId.0`.
    pub nets: Vec<Reading>,
    pub loads: Vec<LoadFlow>,
    pub supplies: Vec<SupplyFlow>,
    pub notes: Vec<Note>,
    pub hz: f64,
}

impl Analysis {
    pub fn net(&self, net: NetId) -> Reading {
        self.nets.get(net.0 as usize).copied().unwrap_or_default()
    }

    pub fn load(&self, symbol: SymbolId) -> Option<&LoadFlow> {
        self.loads.iter().find(|l| l.symbol == symbol)
    }
}

/// A numeric property of a device, or of the first of its parts that has it.
fn number(project: &Project, symbol: &SymbolInstance, keys: &[&str]) -> Option<f64> {
    let device = project.device(symbol.device()?)?;
    let read = |p: &novo_core::Properties| {
        keys.iter().find_map(|k| match p.get(*k)? {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            Value::Text(t) => t.trim().replace(',', ".").parse().ok(),
            Value::Bool(_) => None,
        })
    };
    read(&device.properties).or_else(|| {
        device
            .parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| read(&p.properties))
    })
}

fn text(project: &Project, symbol: &SymbolInstance, key: &str) -> Option<String> {
    let device = project.device(symbol.device()?)?;
    match device.properties.get(key)? {
        Value::Text(t) => Some(t.trim().to_lowercase()),
        _ => None,
    }
}

/// Trailing digit of a potential's name (`L2` is 2).
fn phase_digit(name: &str) -> Option<u32> {
    name.chars().rev().find_map(|c| c.to_digit(10))
}

/// Typical efficiency and power factor of an IE3 four-pole motor at `kw`
/// (planning values for a motor whose nameplate is not in the drawing).
fn typical_motor(kw: f64) -> (f64, f64) {
    const TABLE: [(f64, f64, f64); 8] = [
        (0.75, 0.80, 0.75),
        (2.2, 0.865, 0.80),
        (4.0, 0.885, 0.82),
        (7.5, 0.90, 0.83),
        (11.0, 0.91, 0.84),
        (22.0, 0.93, 0.86),
        (45.0, 0.945, 0.87),
        (90.0, 0.955, 0.88),
    ];
    let k = kw.clamp(TABLE[0].0, TABLE[7].0);
    let i = TABLE.iter().position(|r| r.0 >= k).unwrap_or(7).max(1);
    let (a, b) = (TABLE[i - 1], TABLE[i]);
    let f = ((k / a.0).ln() / (b.0 / a.0).ln()).clamp(0.0, 1.0);
    (a.1 + f * (b.1 - a.1), a.2 + f * (b.2 - a.2))
}

/// Typical full-load slip of a four-pole motor at `kw`.
fn typical_slip(kw: f64) -> f64 {
    (0.06 - 0.0078 * (kw.max(0.75)).ln()).clamp(0.012, 0.06)
}

/// One branch of a load: its supply node, its return node and the first
/// element of the branch, whose current is the branch current.
type Branch = (Node, Node, usize);

struct Builder<'a> {
    project: &'a Project,
    netlist: &'a Netlist,
    circuit: Circuit,
    hz: f64,
    notes: Vec<Note>,
    three_phase: bool,
}

impl Builder<'_> {
    fn node(&self, net: NetId) -> Node {
        net.0 as usize + 1
    }

    fn note(&mut self, kind: NoteKind, symbol: Option<SymbolId>, text: impl Into<String>) {
        self.notes.push(Note {
            kind,
            symbol,
            text: text.into(),
        });
    }

    fn pin_node(&self, symbol: &SymbolInstance, pin: usize) -> Option<Node> {
        self.netlist
            .net_of(Endpoint::pin(symbol.id, pin as u16))
            .map(|n| self.node(n))
    }

    /// An impedance `z` at the circuit's frequency between two nodes.
    fn impedance(&mut self, a: Node, b: Node, z: Complex) {
        let w = 2.0 * PI * self.hz;
        if z.im.abs() < 1e-12 {
            self.circuit.resistor(a, b, z.re.max(1e-6));
        } else {
            let mid = self.circuit.node();
            self.circuit.resistor(a, mid, z.re.max(1e-6));
            if z.im > 0.0 {
                self.circuit.inductor(mid, b, z.im / w);
            } else {
                self.circuit.capacitor(mid, b, -1.0 / (z.im * w));
            }
        }
    }

    /// The nominal voltage between two nets from the potentials they carry:
    /// line-to-line for two lines, line-to-neutral for a line and a neutral
    /// or earth, the supply voltage for DC.
    fn nominal(&self, a: NetId, b: NetId) -> Option<f64> {
        let pot = |n: NetId| -> Option<(PotentialKind, Option<f64>)> {
            let net = self.netlist.net(n);
            net.potentials
                .iter()
                .chain(net.reachable.iter())
                .find_map(|&p| self.project.potential(p).map(|p| (p.kind, p.voltage)))
        };
        let ((ka, va), (kb, vb)) = (pot(a)?, pot(b)?);
        use PotentialKind::*;
        match (ka, kb) {
            (Line, Line) => va.or(vb),
            (Line, _) => va.map(|v| if self.three_phase { v / 3f64.sqrt() } else { v }),
            (_, Line) => vb.map(|v| if self.three_phase { v / 3f64.sqrt() } else { v }),
            _ => va.or(vb),
        }
    }
}

/// Solves the drawing under the poles in `state`.
pub fn analyse(project: &Project, netlist: &Netlist, state: &PoleState) -> Analysis {
    let net_count = netlist.nets().len();
    let mut circuit = Circuit::new();
    for _ in 0..net_count {
        circuit.node();
    }
    let lines: Vec<_> = project
        .potentials()
        .filter(|p| p.kind == PotentialKind::Line)
        .collect();
    let three_phase = lines
        .iter()
        .filter(|p| phase_digit(&p.name).is_some())
        .count()
        >= 2;
    let system_of = |id: PotentialId| {
        project
            .potential_systems()
            .find(|s| s.members.contains(&id))
    };
    let mut hz = 50.0;
    let mut hz_known = false;
    if let Some(f) = lines
        .iter()
        .find_map(|p| system_of(p.id).and_then(|s| s.frequency_hz))
    {
        hz = f;
        hz_known = true;
    }
    let mut b = Builder {
        project,
        netlist,
        circuit,
        hz,
        notes: Vec::new(),
        three_phase,
    };
    // Sources.
    let mut source_elements: Vec<(PotentialId, usize)> = Vec::new();
    let mut ac_present = false;
    let mut impedance_known = false;
    for net in netlist.nets() {
        let Some(&id) = net.defined.first() else {
            continue;
        };
        if net.defined.len() > 1 {
            b.note(
                NoteKind::Conflict,
                None,
                "two potentials are defined on one net: a short at the source",
            );
        }
        let Some(p) = project.potential(id) else {
            continue;
        };
        use PotentialKind::*;
        let wave = match p.kind {
            Neutral | ProtectiveEarth | Pen | DcNegative => Some(Wave::Dc(0.0)),
            DcPositive | Signal => p.voltage.map(Wave::Dc),
            Line => p.voltage.map(|v| {
                ac_present = true;
                if three_phase {
                    let d = phase_digit(&p.name).unwrap_or(1);
                    Wave::rms(v / 3f64.sqrt(), hz, -120.0 * f64::from(d.saturating_sub(1)))
                } else {
                    Wave::rms(v, hz, 0.0)
                }
            }),
        };
        let Some(wave) = wave else {
            b.note(
                NoteKind::Missing,
                None,
                format!("potential {} has no voltage: not simulated", p.name),
            );
            continue;
        };
        // Source impedance from the short-circuit current, when the supply
        // states one.
        let ik = [Some(&p.properties), system_of(id).map(|s| &s.properties)]
            .into_iter()
            .flatten()
            .find_map(|props| match props.get("short_circuit_ka")? {
                Value::Float(f) => Some(*f),
                Value::Int(i) => Some(*i as f64),
                _ => None,
            });
        let node = b.node(net.id);
        match (ik, p.kind, p.voltage) {
            (Some(ka), Line, Some(v)) if ka > 0.0 => {
                impedance_known = true;
                let vph = if three_phase { v / 3f64.sqrt() } else { v };
                let z = vph / (ka * 1000.0);
                // X/R = 10, the usual figure for a low-voltage transformer.
                let r = z / (1.0 + 100.0f64).sqrt();
                let src = b.circuit.node();
                let e = b.circuit.voltage(src, GROUND, wave);
                b.impedance(src, node, Complex::new(r, 10.0 * r));
                source_elements.push((id, e));
            }
            _ => {
                let e = b.circuit.voltage(node, GROUND, wave);
                source_elements.push((id, e));
            }
        }
    }
    if ac_present && !impedance_known {
        b.note(
            NoteKind::Limit,
            None,
            "supply impedance unknown (no short_circuit_ka): voltage drop and fault current are not shown",
        );
    }
    if ac_present && !hz_known {
        b.note(NoteKind::Assumed, None, "supply frequency assumed 50 Hz");
    }
    // Closed poles.
    for symbol in project.symbols() {
        for (g, group) in symbol.pass_through.iter().enumerate() {
            if !state.is_closed(symbol.id, g) {
                continue;
            }
            let nodes: Vec<Node> = group
                .iter()
                .filter_map(|&pin| b.pin_node(symbol, pin as usize))
                .collect();
            for w in nodes.windows(2) {
                if w[0] != w[1] {
                    b.circuit.switch(w[0], w[1], true);
                }
            }
        }
    }
    // Loads: (symbol, elements whose currents make up its draw, node pairs
    // for its voltage).
    let mut loads: Vec<(SymbolId, Vec<Branch>)> = Vec::new();
    for symbol in project.symbols() {
        if is_motor(symbol) {
            if let Some(elements) = motor_load(&mut b, symbol) {
                loads.push((symbol.id, elements));
            }
        } else if (is_load(symbol) || is_coil(symbol))
            && let Some(elements) = resistive_load(&mut b, symbol)
        {
            loads.push((symbol.id, elements));
        }
    }
    b.note(
        NoteKind::Limit,
        None,
        "conductors are ideal (cable length is not in the schematic); loads are linear",
    );
    // Solve both analyses; they share the circuit because AC and DC
    // sources do not drive each other.
    let mut out = Analysis {
        hz,
        notes: Vec::new(),
        ..Analysis::default()
    };
    let dc = analog::solve_dc(&b.circuit);
    let ac = analog::solve_ac(&b.circuit, hz);
    out.notes = std::mem::take(&mut b.notes);
    let (Ok(dc), Ok(ac)) = (dc, ac) else {
        out.notes.push(Note {
            kind: NoteKind::Conflict,
            symbol: None,
            text: "the circuit has no solution: sources contradict each other".into(),
        });
        out.nets = vec![Reading::default(); net_count];
        return out;
    };
    out.nets = (0..net_count)
        .map(|i| Reading {
            dc: dc.voltage(i + 1).re,
            ac: ac.voltage(i + 1),
        })
        .collect();
    let draws: BTreeMap<SymbolId, Vec<Branch>> = loads.into_iter().collect();
    for (symbol, branches) in draws {
        let (mut p, mut q, mut current) = (0.0, 0.0, 0.0f64);
        for &(a, c, e) in &branches {
            let s_ac = ac.between(a, c) * ac.current(e).conj();
            let p_dc = dc.between(a, c).re * dc.current(e).re;
            p += s_ac.re + p_dc;
            q += s_ac.im;
            current = current.max(ac.current(e).abs()).max(dc.current(e).re.abs());
        }
        out.loads.push(LoadFlow {
            symbol,
            current,
            power_w: p,
            reactive_var: q,
        });
    }
    for (potential, e) in source_elements {
        out.supplies.push(SupplyFlow {
            potential,
            current: ac.current(e).abs().max(dc.current(e).re.abs()),
        });
    }
    out
}

/// A three-phase motor at its rated point: three equal impedances to a
/// floating star point. Returns the elements to read the current from.
fn motor_load(b: &mut Builder<'_>, symbol: &SymbolInstance) -> Option<Vec<Branch>> {
    let windings: Vec<usize> = symbol
        .pins
        .iter()
        .enumerate()
        .filter(|(_, p)| !p.name.eq_ignore_ascii_case("pe"))
        .map(|(i, _)| i)
        .collect();
    if windings.len() != 3 {
        b.note(
            NoteKind::Limit,
            Some(symbol.id),
            "only three-winding motors are simulated",
        );
        return None;
    }
    let nodes: Vec<Node> = windings
        .iter()
        .filter_map(|&i| b.pin_node(symbol, i))
        .collect();
    if nodes.len() != 3 {
        return None;
    }
    let Some(kw) = number(b.project, symbol, &["power_kw", "rated_power_kw"]) else {
        b.note(
            NoteKind::Missing,
            Some(symbol.id),
            "motor has no power (power_kw): not simulated",
        );
        return None;
    };
    let net = |i: usize| b.netlist.net_of(Endpoint::pin(symbol.id, i as u16));
    let v_ll = number(b.project, symbol, &["voltage_v", "rated_voltage_v"])
        .or_else(|| {
            // Line-to-line voltage of the lines feeding it.
            let (a, c) = (net(windings[0])?, net(windings[1])?);
            b.nominal(a, c)
        })
        .unwrap_or_else(|| {
            b.note(
                NoteKind::Assumed,
                Some(symbol.id),
                "motor voltage assumed 400 V",
            );
            400.0
        });
    let (eff_t, pf_t) = typical_motor(kw);
    let mut assumed = Vec::new();
    let mut get = |keys: &[&str], default: f64, what: &str| {
        number(b.project, symbol, keys).unwrap_or_else(|| {
            assumed.push(what.to_string());
            default
        })
    };
    let eff = get(&["efficiency"], eff_t, "efficiency");
    let pf = get(&["power_factor", "cos_phi"], pf_t, "power factor");
    let slip = typical_slip(kw);
    let rpm = get(&["rpm", "rated_speed_rpm"], 1500.0 * (1.0 - slip), "speed");
    let ilr = get(
        &["lr_current_ratio", "ilr_ratio"],
        7.0,
        "locked-rotor current",
    );
    let amps_default = kw * 1000.0 / (3f64.sqrt() * v_ll * eff * pf);
    let amps = get(&["rated_current_a"], amps_default, "rated current");
    let connection = match text(b.project, symbol, "connection").as_deref() {
        Some("star" | "y") => Connection::Star,
        _ => Connection::Delta,
    };
    if !assumed.is_empty() {
        b.note(
            NoteKind::Assumed,
            Some(symbol.id),
            format!("typical IE3 values used for: {}", assumed.join(", ")),
        );
    }
    let motor = match Motor::from_nameplate(Nameplate {
        power_w: kw * 1000.0,
        v_ll,
        hz: b.hz,
        rpm,
        current: amps,
        power_factor: pf,
        efficiency: eff,
        lr_current_ratio: ilr,
        connection,
    }) {
        Ok(m) => m,
        Err(e) => {
            b.note(
                NoteKind::Missing,
                Some(symbol.id),
                format!("motor nameplate does not fit a motor model ({e:?}): not simulated"),
            );
            return None;
        }
    };
    let zw = motor.z_in(motor.rated_slip());
    let zy = match connection {
        Connection::Star => zw,
        Connection::Delta => zw / Complex::real(3.0),
    };
    let star = b.circuit.node();
    let mut elements: Vec<Branch> = Vec::new();
    for n in nodes {
        elements.push((n, star, b.circuit.elements().len()));
        b.impedance(n, star, zy);
    }
    Some(elements)
}

/// A load that is a plain resistance: a lamp, valve, horn or coil, from its
/// power and the nominal voltage across it.
fn resistive_load(b: &mut Builder<'_>, symbol: &SymbolInstance) -> Option<Vec<Branch>> {
    let nets = (
        b.netlist.net_of(Endpoint::pin(symbol.id, 0)),
        b.netlist.net_of(Endpoint::pin(symbol.id, 1)),
    );
    let (Some(na), Some(nb)) = nets else {
        return None;
    };
    let volts =
        number(b.project, symbol, &["voltage_v", "coil_voltage_v"]).or_else(|| b.nominal(na, nb));
    let Some(volts) = volts else {
        b.note(
            NoteKind::Missing,
            Some(symbol.id),
            "no voltage across the load: not simulated",
        );
        return None;
    };
    let watts = number(b.project, symbol, &["power_w", "coil_power_w"]).unwrap_or_else(|| {
        let w = if is_lamp(symbol) {
            2.0
        } else if is_coil(symbol) {
            3.0
        } else {
            5.0
        };
        b.note(
            NoteKind::Assumed,
            Some(symbol.id),
            format!("power assumed {w} W"),
        );
        w
    });
    let id = b.circuit.elements().len();
    b.circuit
        .resistor(b.node(na), b.node(nb), volts * volts / watts.max(1e-6));
    Some(vec![(b.node(na), b.node(nb), id)])
}
