//! A running PLC wired to the drawing: inputs read from the circuit, outputs
//! put back on it, one scan per step.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{Netlist, Project, SymbolId};

use super::address::{Address, Space};
use super::model::Program;
use super::value::Value;
use super::vm::{Runtime, RuntimeError};
use crate::{Clock, PoleState, Settled, inputs_seen, settle_timed};

/// Which drawn channel sits at which address. Built from the I/O list: the
/// address column of each row and the symbol it is drawn at.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IoMap {
    inputs: BTreeMap<Address, SymbolId>,
    outputs: BTreeMap<Address, SymbolId>,
}

impl IoMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// A digital input channel. `false` when `address` is not an address
    /// (or not an input one), so the caller can report the row.
    pub fn add_input(&mut self, address: &str, symbol: SymbolId) -> bool {
        match Address::parse(address) {
            Some(a) if a.space == Space::Input => {
                self.inputs.insert(a, symbol);
                true
            }
            _ => false,
        }
    }

    pub fn add_output(&mut self, address: &str, symbol: SymbolId) -> bool {
        match Address::parse(address) {
            Some(a) if a.space == Space::Output => {
                self.outputs.insert(a, symbol);
                true
            }
            _ => false,
        }
    }

    pub fn input_symbol(&self, address: Address) -> Option<SymbolId> {
        self.inputs.get(&address).copied()
    }

    pub fn output_symbol(&self, address: Address) -> Option<SymbolId> {
        self.outputs.get(&address).copied()
    }

    /// The address a drawn channel is wired to, input or output.
    pub fn address_of(&self, symbol: SymbolId) -> Option<Address> {
        self.inputs
            .iter()
            .chain(&self.outputs)
            .find(|(_, s)| **s == symbol)
            .map(|(a, _)| *a)
    }
}

/// A program the drawing does not provide for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unbound {
    pub address: Address,
    /// An input nothing is drawn at reads 0 forever; an output nothing is
    /// drawn at drives nothing.
    pub is_input: bool,
}

/// The program, its state and the last result.
#[derive(Debug, Clone)]
pub struct Bench {
    runtime: Runtime,
    map: IoMap,
    on: BTreeSet<SymbolId>,
    fault: Option<RuntimeError>,
}

impl Bench {
    pub fn new(program: Program, map: IoMap) -> Bench {
        Bench {
            runtime: Runtime::new(program),
            map,
            on: BTreeSet::new(),
            fault: None,
        }
    }

    pub fn runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// For forcing a value or reading a variable on the monitor.
    pub fn runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    pub fn map(&self) -> &IoMap {
        &self.map
    }

    /// Wires the program to a changed drawing without losing its state.
    pub fn set_map(&mut self, map: IoMap) {
        self.map = map;
    }

    /// Addresses the program names that no drawn channel answers.
    pub fn unbound(&self) -> Vec<Unbound> {
        self.runtime
            .program()
            .addresses
            .iter()
            .filter_map(|&address| match address.space {
                Space::Input if address.is_bit() && self.map.input_symbol(address).is_none() => {
                    Some(Unbound {
                        address,
                        is_input: true,
                    })
                }
                Space::Output if address.is_bit() && self.map.output_symbol(address).is_none() => {
                    Some(Unbound {
                        address,
                        is_input: false,
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// The outputs that are on after the last scan.
    pub fn outputs_on(&self) -> &BTreeSet<SymbolId> {
        &self.on
    }

    /// The fault that stopped the last scan, if any. A faulted program
    /// keeps its outputs and tries again on the next step.
    pub fn fault(&self) -> Option<&RuntimeError> {
        self.fault.as_ref()
    }

    /// One scan: `seen` are the input channels whose pin carries a potential;
    /// `dt` is the controller time in seconds.
    pub fn step(&mut self, seen: &BTreeSet<SymbolId>, dt: f64) -> &BTreeSet<SymbolId> {
        for (&address, symbol) in &self.map.inputs {
            self.runtime
                .set_input(address, Value::Bool(seen.contains(symbol)));
        }
        match self.runtime.scan((dt * 1e6).round() as i64) {
            Ok(()) => {
                self.fault = None;
                self.on = self
                    .map
                    .outputs
                    .iter()
                    .filter(|(a, _)| self.runtime.output_bool(**a))
                    .map(|(_, s)| *s)
                    .collect();
            }
            Err(e) => self.fault = Some(e),
        }
        &self.on
    }
}

/// The program running against a drawing: the PLC, the coils and contacts
/// of the circuit, and the clock they share.
#[derive(Debug, Clone)]
pub struct ClosedLoop {
    pub bench: Bench,
    pub clock: Clock,
    settled: Option<Settled>,
}

impl ClosedLoop {
    pub fn new(bench: Bench) -> Self {
        ClosedLoop {
            bench,
            clock: Clock::new(),
            settled: None,
        }
    }

    pub fn settled(&self) -> Option<&Settled> {
        self.settled.as_ref()
    }

    /// Advances `dt` seconds: the PLC reads the inputs the circuit showed at
    /// the end of the last step, scans, and the circuit is solved again with
    /// its outputs. `manual` are the hand-set poles (buttons, switches);
    /// `forced` are outputs and coils held on by hand. An output that is a
    /// relay (it has switched poles) closes its pole; any other puts its
    /// supply on its pin.
    pub fn step(
        &mut self,
        project: &Project,
        netlist: &Netlist,
        manual: &PoleState,
        forced: &BTreeSet<SymbolId>,
        dt: f64,
    ) -> &Settled {
        if self.settled.is_none() {
            self.settled = Some(settle_timed(
                project,
                netlist,
                manual,
                forced,
                &BTreeSet::new(),
                &mut self.clock,
            ));
        }
        let previous = self.settled.take().expect("settled above");
        let seen = inputs_seen(project, netlist, &previous.solution);
        let on = self.bench.step(&seen, dt).clone();
        self.clock.advance(dt);
        let mut poles = manual.clone();
        let mut drive = forced.clone();
        for symbol in on {
            match project.symbol(symbol) {
                Some(s) if !s.pass_through.is_empty() => poles.set_all(project, symbol, true),
                _ => {
                    drive.insert(symbol);
                }
            }
        }
        self.settled = Some(settle_timed(
            project,
            netlist,
            &poles,
            &drive,
            &previous.energized,
            &mut self.clock,
        ));
        self.settled.as_ref().expect("just set")
    }
}
