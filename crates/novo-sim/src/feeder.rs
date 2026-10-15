//! A radial feeder network: a source, then a tree of cable and busbar
//! segments, with a protective device at the head of any segment.
//!
//! This is the network model for the scenario runner when the plant is a
//! feeder tree (which is most of a control cabinet and a distribution board).
//! Short-circuit currents follow IEC 60909 with the voltage factor `c`:
//! `Ik3 = c·Un / (√3·|Z1|)` for the three-phase fault, `Ik2 = (√3/2)·Ik3`
//! phase to phase, and `Ik1 = c·Un/√3 / |Zloop|` for phase to neutral and
//! phase to earth with the return conductor in the same cable (TN-S). Zloop
//! is the sum of the phase and return impedances of every segment from the
//! source, which is why the lowest fault current is at the far end of the
//! longest cable. The source, and each segment, carries its positive-sequence
//! impedance and a loop impedance (by default twice the segment's, a return
//! conductor of the same cross section).
//!
//! What it leaves out, and says so: motor contribution to the fault current,
//! voltage drop of the load current while the fault lasts, decay of the
//! generator-fed AC component, and meshed networks. The analog solver covers
//! those; both implement [`CircuitModel`].

use std::collections::{BTreeMap, BTreeSet};

use crate::fault::{FaultCurrent, FaultKind};
use crate::scenario::{ActiveFault, CircuitModel, Conditions};

/// A complex impedance in ohm.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Impedance {
    pub r: f64,
    pub x: f64,
}

impl Impedance {
    pub fn new(r: f64, x: f64) -> Self {
        Self { r, x }
    }

    pub fn abs(self) -> f64 {
        self.r.hypot(self.x)
    }

    fn plus(self, o: Impedance) -> Impedance {
        Impedance::new(self.r + o.r, self.x + o.x)
    }

    fn times(self, k: f64) -> Impedance {
        Impedance::new(self.r * k, self.x * k)
    }
}

#[derive(Debug, Clone)]
struct Node {
    name: String,
    parent: Option<usize>,
    z: Impedance,
    z_loop: Impedance,
    device: Option<String>,
}

/// The tree. Node 0 is the busbar fed by the source.
#[derive(Debug, Clone)]
pub struct RadialNetwork {
    /// Line-to-line voltage in V.
    un: f64,
    /// Voltage factor c (1.05 for the maximum, 0.95 for the minimum short
    /// circuit current of IEC 60909 at low voltage).
    c: f64,
    frequency: f64,
    nodes: Vec<Node>,
}

impl RadialNetwork {
    /// A network fed through `source` (the upstream grid and transformer
    /// seen from the busbar), named `busbar`.
    pub fn new(busbar: &str, un: f64, frequency: f64, source: Impedance) -> Self {
        Self {
            un,
            c: 1.05,
            frequency,
            nodes: vec![Node {
                name: busbar.into(),
                parent: None,
                z: source,
                z_loop: source.times(2.0),
                device: None,
            }],
        }
    }

    /// Voltage factor, to compute the minimum current (0.95).
    pub fn with_voltage_factor(mut self, c: f64) -> Self {
        self.c = c;
        self
    }

    /// Adds a segment from `parent` to a new node, optionally protected at
    /// its head by `device`. The loop impedance is twice the segment's.
    pub fn segment(mut self, name: &str, parent: &str, z: Impedance, device: Option<&str>) -> Self {
        self.add(name, parent, z, z.times(2.0), device);
        self
    }

    /// [`Self::segment`] with an explicit loop impedance.
    pub fn segment_loop(
        mut self,
        name: &str,
        parent: &str,
        z: Impedance,
        z_loop: Impedance,
        device: Option<&str>,
    ) -> Self {
        self.add(name, parent, z, z_loop, device);
        self
    }

    fn add(&mut self, name: &str, parent: &str, z: Impedance, z_loop: Impedance, d: Option<&str>) {
        let parent = self
            .index(parent)
            .unwrap_or_else(|| panic!("unknown parent node {parent}"));
        self.nodes.push(Node {
            name: name.into(),
            parent: Some(parent),
            z,
            z_loop,
            device: d.map(str::to_string),
        });
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.nodes.iter().position(|n| n.name == name)
    }

    /// Node indexes from the busbar down to `i`.
    fn path(&self, mut i: usize) -> Vec<usize> {
        let mut p = vec![i];
        while let Some(parent) = self.nodes[i].parent {
            p.push(parent);
            i = parent;
        }
        p.reverse();
        p
    }

    /// The path to `node` has no open device on it.
    fn fed(&self, node: usize, open: &BTreeSet<String>) -> bool {
        self.path(node).iter().all(|&i| {
            self.nodes[i]
                .device
                .as_ref()
                .is_none_or(|d| !open.contains(d))
        })
    }

    fn is_below(&self, node: usize, ancestor: usize) -> bool {
        self.path(node).contains(&ancestor)
    }

    /// Prospective short-circuit current at a node for a kind of fault,
    /// regardless of which devices are open.
    pub fn prospective(&self, node: &str, kind: FaultKind, fault_ohm: f64) -> Option<FaultCurrent> {
        let i = self.index(node)?;
        let path = self.path(i);
        let positive = path
            .iter()
            .fold(Impedance::default(), |a, &n| a.plus(self.nodes[n].z));
        let looped = path
            .iter()
            .fold(Impedance::default(), |a, &n| a.plus(self.nodes[n].z_loop));
        let with_fault = |z: Impedance| Impedance::new(z.r + fault_ohm, z.x);
        let phase_v = self.c * self.un / 3f64.sqrt();
        let (ik, z) = match kind {
            FaultKind::ThreePhase => {
                let z = with_fault(positive);
                (phase_v / z.abs(), z)
            }
            FaultKind::PhasePhase => {
                let z = with_fault(positive);
                (phase_v / z.abs() * 3f64.sqrt() / 2.0, z)
            }
            FaultKind::PhaseNeutral | FaultKind::PhaseEarth => {
                let z = with_fault(looped);
                (phase_v / z.abs(), z)
            }
            FaultKind::OpenPhase => return None,
        };
        Some(FaultCurrent::new(ik, z.r / z.x.max(1e-9), self.frequency))
    }
}

impl CircuitModel for RadialNetwork {
    fn device_currents(&self, c: &Conditions) -> BTreeMap<String, f64> {
        // Single phasing: a motor on two phases draws √3 as much on them.
        let open_phase: Vec<usize> = c
            .faults
            .iter()
            .filter(|f| f.kind == FaultKind::OpenPhase)
            .filter_map(|f| self.index(&f.node))
            .filter(|&i| self.fed(i, c.open))
            .collect();
        let mut out = BTreeMap::new();
        for (d, node) in self.nodes.iter().enumerate() {
            let Some(device) = &node.device else { continue };
            if !self.fed(d, c.open) {
                out.insert(device.clone(), 0.0);
                continue;
            }
            let mut i = 0.0;
            for (name, &amps) in c.loads {
                let Some(m) = self.index(name) else { continue };
                if self.is_below(m, d) && self.fed(m, c.open) {
                    let single = open_phase.iter().any(|&o| self.is_below(m, o));
                    i += amps * if single { 3f64.sqrt() } else { 1.0 };
                }
            }
            if let Some((_, fc)) = self.fault_through(device, c) {
                // The fault current dwarfs the load; they do not add as
                // phasors in general, the larger one decides.
                i = i.max(fc.ik);
            }
            out.insert(device.clone(), i);
        }
        out
    }

    fn fault_current(&self, fault: &ActiveFault, c: &Conditions) -> Option<FaultCurrent> {
        let i = self.index(&fault.node)?;
        if !self.fed(i, c.open) {
            return None;
        }
        self.prospective(&fault.node, fault.kind, fault.fault_ohm)
    }

    fn fault_through(&self, device: &str, c: &Conditions) -> Option<(String, FaultCurrent)> {
        let d = self
            .nodes
            .iter()
            .position(|n| n.device.as_deref() == Some(device))?;
        if !self.fed(d, c.open) {
            return None;
        }
        c.faults
            .iter()
            .filter_map(|f| {
                let m = self.index(&f.node)?;
                if !self.is_below(m, d) {
                    return None;
                }
                self.fault_current(f, c).map(|fc| (f.id.clone(), fc))
            })
            .max_by(|a, b| a.1.ik.total_cmp(&b.1.ik))
    }

    fn upstream_devices(&self, node: &str) -> Vec<String> {
        let Some(i) = self.index(node) else {
            return Vec::new();
        };
        self.path(i)
            .iter()
            .rev()
            .filter_map(|&n| self.nodes[n].device.clone())
            .collect()
    }
}
