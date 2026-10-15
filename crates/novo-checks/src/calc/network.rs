//! Short-circuit currents at every node of a radial low-voltage network
//! (IEC 60909-0 and its low-voltage guide IEC 60909-1; earth faults for TN
//! systems as in IEC 60364-4-41).
//!
//! The network is a tree: a source (supply network, transformer, or both
//! in series) at the root, and nodes (boards, terminals) hung off it by
//! branches (cables, busbars). Motors may be attached to nodes. For every
//! node [`Network::fault`] returns the currents a protective device
//! there has to deal with:
//!
//! | Quantity | Used for | Conditions |
//! | --- | --- | --- |
//! | `ik3_max`, `ip` | breaking and making capacity, busbar forces | voltage factor `c_max`, strongest source, cold conductors, motors added |
//! | `ik2_max` | contact welding checks | `√3/2 · ik3_max` |
//! | `ik1_max` | breaking capacity on earth faults | as `ik3_max`, sequence networks |
//! | `ik3_min`, `ik2_min`, `ik1_min` | disconnection time, magnetic trip, thermal withstand | `c_min`, weakest source, conductors at operating temperature, motors left out |
//!
//! Earth faults use symmetrical components: `Ik1 = √3·c·Un / |2·Z1 + Z0|`
//! (negative sequence equals positive, far from generators). Where the
//! earth-return path is a protective conductor in the same cable, the
//! cable's zero-sequence impedance is `Z1 + 3·Zpe`, which makes the loop
//! `Z1 + Zpe` the textbook earth-fault loop. If a branch has no protective
//! conductor data its earth-fault results are `None`: unknown stays
//! unknown.
//!
//! Limits, stated so the report can repeat them: radial network only (no
//! meshes, no generators), copper conductors, the simple ratio method for
//! the peak factor, motor contributions added arithmetically (safe side),
//! no arc resistance.

use super::Insulation;
use super::copper_resistivity;
use super::drop::REACTANCE_OHM_PER_M;
use super::short_circuit::{C_MAX, C_MIN, Impedance};

/// A node of the network. Opaque; made by [`Network`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(usize);

impl NodeId {
    /// Position in creation order; the root is 0.
    pub fn index(self) -> usize {
        self.0
    }
}

/// What feeds the network.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceModel {
    /// Positive-sequence impedance of the strongest source seen at the
    /// low-voltage terminals, ohm, for maximum currents.
    pub z1_max: Impedance,
    /// Same for the weakest source, for minimum currents.
    pub z1_min: Impedance,
    /// Zero-sequence impedance at the terminals (strongest, weakest), when
    /// known.
    pub z0_max: Option<Impedance>,
    pub z0_min: Option<Impedance>,
    /// What was assumed to build this, one line each, for the report.
    pub assumptions: Vec<String>,
}

fn split(z: f64, r_over_x: f64) -> Impedance {
    let x = z / (1.0 + r_over_x * r_over_x).sqrt();
    Impedance::new(x * r_over_x, x)
}

impl SourceModel {
    /// From stated three-phase currents at the incoming terminals in kA:
    /// the maximum the supply can deliver and, when known, the minimum.
    /// Without a minimum the maximum is used for both, which makes the
    /// minimum currents optimistic and is said so. Zero sequence is
    /// assumed equal to positive.
    pub fn from_short_circuit(
        ik_max_ka: f64,
        ik_min_ka: Option<f64>,
        voltage: f64,
    ) -> Option<SourceModel> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(ik_max_ka) && ok(voltage)) || ik_min_ka.is_some_and(|m| !ok(m) || m > ik_max_ka) {
            return None;
        }
        let z_of = |c: f64, ik_ka: f64| c * voltage / (3.0_f64.sqrt() * ik_ka * 1000.0);
        let z1_max = split(z_of(C_MAX, ik_max_ka), 0.1);
        let mut assumptions = vec![
            "source R/X = 0.1 (IEC 60909-0 §3.2)".to_owned(),
            "source zero-sequence impedance taken equal to positive sequence".to_owned(),
        ];
        let z1_min = match ik_min_ka {
            Some(m) => split(z_of(C_MIN, m), 0.1),
            None => {
                assumptions.push(
                    "no minimum supply current given: the maximum is used, so minimum fault currents are optimistic"
                        .to_owned(),
                );
                z1_max
            }
        };
        Some(SourceModel {
            z1_max,
            z1_min,
            z0_max: Some(z1_max),
            z0_min: Some(z1_min),
            assumptions,
        })
    }

    /// A transformer on a stiff network, or behind a supply network of
    /// stated fault level.
    pub fn from_transformer(t: &Transformer) -> Option<SourceModel> {
        t.source()
    }
}

/// A distribution transformer feeding the network.
#[derive(Debug, Clone, PartialEq)]
pub struct Transformer {
    pub sn_kva: f64,
    /// Short-circuit voltage, percent.
    pub uk_percent: f64,
    /// Load losses at rated current, W. Without them `R/X = 0.2` is
    /// assumed.
    pub pk_w: Option<f64>,
    /// Low-voltage side nominal voltage, V.
    pub voltage: f64,
    /// Zero-sequence over positive-sequence impedance seen from the
    /// low-voltage side. 1.0 for a Dyn connection (the usual distribution
    /// transformer, IEC 60909-0 gives about 0.95 to 1.0); a Yzn connection
    /// is lower, typically 0.1 to 0.4 on the reactive part, so give the
    /// maker's figure.
    pub zero_ratio: f64,
    /// Fault level of the network behind the transformer, MVA, at maximum
    /// and minimum. `None` is an infinitely stiff network.
    pub network_sk_mva: Option<(f64, f64)>,
}

impl Transformer {
    pub fn new(sn_kva: f64, uk_percent: f64, voltage: f64) -> Transformer {
        Transformer {
            sn_kva,
            uk_percent,
            pk_w: None,
            voltage,
            zero_ratio: 1.0,
            network_sk_mva: None,
        }
    }

    fn source(&self) -> Option<SourceModel> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(self.sn_kva) && ok(self.uk_percent) && ok(self.voltage) && ok(self.zero_ratio)) {
            return None;
        }
        if self.pk_w.is_some_and(|p| !(p.is_finite() && p >= 0.0)) {
            return None;
        }
        let sn = self.sn_kva * 1000.0;
        let un2_over_sn = self.voltage * self.voltage / sn;
        let z_t = self.uk_percent / 100.0 * un2_over_sn;
        let mut assumptions = Vec::new();
        let (r_t, x_t) = match self.pk_w {
            Some(pk) => {
                let r = pk / sn * un2_over_sn;
                if r >= z_t {
                    return None;
                }
                (r, (z_t * z_t - r * r).sqrt())
            }
            None => {
                assumptions.push("transformer R/X = 0.2 (no load losses given)".to_owned());
                let z = split(z_t, 0.2);
                (z.r, z.x)
            }
        };
        // Correction factor KT (IEC 60909-0 §6.3.3), on the relative reactance.
        let x_rel = x_t / un2_over_sn;
        let kt = 0.95 * C_MAX / (1.0 + 0.6 * x_rel);
        assumptions.push(format!(
            "transformer impedance corrected with KT = {:.3} (IEC 60909-0 §6.3.3)",
            kt
        ));
        let zt = Impedance::new(r_t * kt, x_t * kt);
        let zt0 = zt.scaled(self.zero_ratio);
        if self.zero_ratio != 1.0 {
            assumptions.push(format!(
                "transformer zero-sequence impedance = {} × positive",
                self.zero_ratio
            ));
        } else {
            assumptions
                .push("transformer zero-sequence impedance = positive (Dyn connection)".to_owned());
        }
        let (zq_max, zq_min) = match self.network_sk_mva {
            None => {
                assumptions.push("supply network behind the transformer taken as stiff".to_owned());
                (Impedance::default(), Impedance::default())
            }
            Some((sk_max, sk_min)) => {
                if !(ok(sk_max) && ok(sk_min) && sk_min <= sk_max) {
                    return None;
                }
                let z = |c: f64, sk: f64| split(c * self.voltage * self.voltage / (sk * 1e6), 0.1);
                // c = 1.1 for the medium-voltage network at maximum, 1.0 at minimum.
                (z(1.1, sk_max), z(1.0, sk_min))
            }
        };
        Some(SourceModel {
            z1_max: zq_max + zt,
            z1_min: zq_min + zt,
            z0_max: Some(zt0),
            z0_min: Some(zt0),
            assumptions,
        })
    }
}

/// A conductor path between two nodes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Branch {
    /// Phase impedance with resistance at 20 °C (cold: maximum currents).
    pub cold: Impedance,
    /// Phase impedance with resistance at operating temperature (hot:
    /// minimum currents).
    pub hot: Impedance,
    /// Protective conductor impedance (cold, hot) in the same run, if its
    /// size is known.
    pub pe: Option<(Impedance, Impedance)>,
}

impl Branch {
    /// A copper cable of `length_m` and `section_mm2` per phase, `parallel`
    /// identical cables side by side, with a protective conductor of
    /// `pe_mm2` when known. Reactance 0.08 mΩ/m for the phase conductors
    /// (multicore, trefoil); the protective conductor is given the same
    /// per-metre reactance.
    pub fn cable(
        section_mm2: f64,
        length_m: f64,
        parallel: u32,
        insulation: Insulation,
        pe_mm2: Option<f64>,
    ) -> Option<Branch> {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        if !(ok(section_mm2) && length_m.is_finite() && length_m >= 0.0 && parallel >= 1) {
            return None;
        }
        if pe_mm2.is_some_and(|s| !ok(s)) {
            return None;
        }
        let n = f64::from(parallel);
        let hot_t = insulation.max_temperature();
        let z = |mm2: f64, t: f64, per: f64| {
            Impedance::new(
                copper_resistivity(t) * length_m / mm2 / per,
                REACTANCE_OHM_PER_M * length_m / per,
            )
        };
        Some(Branch {
            cold: z(section_mm2, 20.0, n),
            hot: z(section_mm2, hot_t, n),
            pe: pe_mm2.map(|s| (z(s, 20.0, 1.0), z(s, hot_t, 1.0))),
        })
    }

    /// A branch of a given impedance (busbar, reactor, tested assembly),
    /// the same cold and hot, with no protective conductor data.
    pub fn impedance(z: Impedance) -> Branch {
        Branch {
            cold: z,
            hot: z,
            pe: None,
        }
    }
}

/// A group of identical induction motors on a node. Only the contribution
/// to the maximum currents is counted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Motor {
    /// Rated current of one motor, A.
    pub rated_current_a: f64,
    /// Locked-rotor current over rated current (5 to 8 typical).
    pub locked_rotor_ratio: f64,
    pub count: u32,
}

#[derive(Debug, Clone)]
struct Node {
    name: String,
    parent: Option<(usize, Branch)>,
    motors: Vec<Motor>,
}

/// The tree.
#[derive(Debug, Clone)]
pub struct Network {
    /// Nominal line-to-line voltage, V.
    pub voltage: f64,
    /// Voltage factors; 1.05 and 0.95 for 400 V with a +6 % tolerance
    /// (IEC 60909-0 Table 1), 1.10 for a +10 % tolerance.
    pub c_max: f64,
    pub c_min: f64,
    source: SourceModel,
    nodes: Vec<Node>,
}

/// Fault currents at a node, A. `None` is unknown (missing data), never
/// zero.
#[derive(Debug, Clone, PartialEq)]
pub struct FaultLevels {
    pub node: NodeId,
    pub ik3_max_a: f64,
    /// Peak short-circuit current `ip`, A.
    pub ip_a: f64,
    pub ik2_max_a: f64,
    pub ik1_max_a: Option<f64>,
    pub ik3_min_a: f64,
    pub ik2_min_a: f64,
    pub ik1_min_a: Option<f64>,
    /// `R/X` of the network seen from the node (maximum case).
    pub r_over_x: f64,
    /// Share of the maximum current that comes from motors, 0 to 1.
    pub motor_share: f64,
    /// Earth-fault loop impedance at minimum conditions, ohm.
    pub loop_min: Option<Impedance>,
}

/// Peak factor `κ` by the ratio method (IEC 60909-0 eq. 55).
pub fn kappa(r_over_x: f64) -> f64 {
    1.02 + 0.98 * (-3.0 * r_over_x).exp()
}

impl Network {
    /// A network with the source at node 0.
    pub fn new(voltage: f64, source: SourceModel) -> Network {
        Network {
            voltage,
            c_max: C_MAX,
            c_min: C_MIN,
            source,
            nodes: vec![Node {
                name: "source".to_owned(),
                parent: None,
                motors: Vec::new(),
            }],
        }
    }

    pub fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub fn source(&self) -> &SourceModel {
        &self.source
    }

    pub fn name(&self, node: NodeId) -> &str {
        &self.nodes[node.0].name
    }

    pub fn parent(&self, node: NodeId) -> Option<NodeId> {
        self.nodes[node.0].parent.map(|(p, _)| NodeId(p))
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    pub fn add_node(&mut self, parent: NodeId, name: &str, branch: Branch) -> NodeId {
        self.nodes.push(Node {
            name: name.to_owned(),
            parent: Some((parent.0, branch)),
            motors: Vec::new(),
        });
        NodeId(self.nodes.len() - 1)
    }

    pub fn add_motors(&mut self, node: NodeId, motors: Motor) {
        self.nodes[node.0].motors.push(motors);
    }

    /// Branches from the root down to `node`, root side first.
    fn path(&self, node: NodeId) -> Vec<Branch> {
        let mut out = Vec::new();
        let mut at = node.0;
        while let Some((parent, branch)) = self.nodes[at].parent {
            out.push(branch);
            at = parent;
        }
        out.reverse();
        out
    }

    /// Impedance between a descendant `from` and its ancestor `to`, cold.
    fn cold_between(&self, from: usize, to: usize) -> Option<Impedance> {
        let mut z = Impedance::default();
        let mut at = from;
        while at != to {
            let (parent, branch) = self.nodes[at].parent?;
            z = z + branch.cold;
            at = parent;
        }
        Some(z)
    }

    /// Fault currents at `node`. `None` for a source with zero impedance.
    pub fn fault(&self, node: NodeId) -> Option<FaultLevels> {
        let path = self.path(node);
        let un = self.voltage;
        let root3 = 3.0_f64.sqrt();

        // Maximum: strongest source, cold conductors.
        let z_max = path.iter().fold(self.source.z1_max, |acc, b| acc + b.cold);
        let zmag = z_max.magnitude();
        if zmag <= 0.0 {
            return None;
        }
        let ik_net = self.c_max * un / (root3 * zmag);
        let r_over_x = if z_max.x > 0.0 {
            z_max.r / z_max.x
        } else {
            0.0
        };

        // Motors on this node and everything below it, through the cold
        // branches between them and the fault.
        let mut ik_motor = 0.0;
        for (index, n) in self.nodes.iter().enumerate() {
            if n.motors.is_empty() {
                continue;
            }
            let Some(between) = self.cold_between(index, node.0) else {
                continue;
            };
            for m in &n.motors {
                let ok = |v: f64| v.is_finite() && v > 0.0;
                if !(ok(m.rated_current_a) && ok(m.locked_rotor_ratio)) || m.count == 0 {
                    continue;
                }
                // ZM = Un / (√3 · ILR), R/X = 0.42 (IEC 60909-0 §3.8).
                let zm = split(
                    un / (root3 * m.locked_rotor_ratio * m.rated_current_a),
                    0.42,
                );
                let each = self.c_max * un / (root3 * (zm + between).magnitude());
                ik_motor += each * f64::from(m.count);
            }
        }
        // Motors are only counted when they matter (IEC 60909-0 §3.8.1:
        // total rated current above 1 % of Ik'' without motors).
        let motor_rated: f64 = self
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| self.cold_between(*i, node.0).is_some())
            .flat_map(|(_, n)| n.motors.iter())
            .map(|m| m.rated_current_a * f64::from(m.count))
            .sum();
        let motors_count = motor_rated > 0.01 * ik_net;
        let ik_motor = if motors_count { ik_motor } else { 0.0 };

        let ik3_max = ik_net + ik_motor;
        let ip = kappa(r_over_x) * 2.0_f64.sqrt() * ik_net + 1.65 * 2.0_f64.sqrt() * ik_motor;

        // Minimum: weakest source, hot conductors, no motors.
        let z_min = path.iter().fold(self.source.z1_min, |acc, b| acc + b.hot);
        let ik3_min = self.c_min * un / (root3 * z_min.magnitude());

        // Earth faults.
        let earth = |z1: Impedance, z0_src: Option<Impedance>, hot: bool, c: f64| {
            let mut z0 = z0_src?;
            for b in &path {
                let (phase, pe) = if hot {
                    (b.hot, b.pe?.1)
                } else {
                    (b.cold, b.pe?.0)
                };
                z0 = z0 + phase + pe.scaled(3.0);
            }
            let sum = z1.scaled(2.0) + z0;
            Some((
                root3 * c * un / sum.magnitude(),
                (z1.scaled(2.0) + z0).scaled(1.0 / 3.0),
            ))
        };
        let ik1_max = earth(z_max, self.source.z0_max, false, self.c_max).map(|(i, _)| i);
        let (ik1_min, loop_min) = match earth(z_min, self.source.z0_min, true, self.c_min) {
            Some((i, z)) => (Some(i), Some(z)),
            None => (None, None),
        };

        Some(FaultLevels {
            node,
            ik3_max_a: ik3_max,
            ip_a: ip,
            ik2_max_a: root3 / 2.0 * ik3_max,
            ik1_max_a: ik1_max,
            ik3_min_a: ik3_min,
            ik2_min_a: root3 / 2.0 * ik3_min,
            ik1_min_a: ik1_min,
            r_over_x,
            motor_share: if ik3_max > 0.0 {
                ik_motor / ik3_max
            } else {
                0.0
            },
            loop_min,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64, rel: f64) {
        assert!((a - b).abs() <= rel * b.abs().max(1e-12), "{a} vs {b}");
    }

    /// A phase-domain solver, independent of the sequence formulas: a
    /// 3×3 complex impedance matrix `Zs` on the diagonal and `Zm` off it
    /// (the same thing as Z1/Z0 expressed in phase quantities), a source of
    /// `c·Un/√3` on phase a, and a bolted fault from phase a to earth.
    /// `Ia = V / Zaa` for a single-line-to-ground fault with the other
    /// phases open, which is what the symmetrical-component formula must
    /// reproduce.
    fn phase_domain_slg(z1: Impedance, z0: Impedance, v_ph: f64) -> f64 {
        let zs = Impedance::new((z0.r + 2.0 * z1.r) / 3.0, (z0.x + 2.0 * z1.x) / 3.0);
        v_ph / zs.magnitude()
    }

    #[test]
    fn transformer_matches_the_textbook_definition() {
        // A stiff 630 kVA, 4 % transformer: In = 909.3 A and the definition
        // Ik = In/uk gives 22.73 kA before voltage factor and KT.
        let mut t = Transformer::new(630.0, 4.0, 400.0);
        t.pk_w = Some(6_500.0);
        let src = SourceModel::from_transformer(&t).unwrap();
        let net = Network::new(400.0, src);
        let f = net.fault(net.root()).unwrap();
        let textbook = 630_000.0 / (3.0_f64.sqrt() * 400.0 * 0.04);
        near(textbook, 22_730.0, 0.001);
        // c_max / KT: 1.05 / 0.9751 → 7.7 % above the textbook value.
        near(f.ik3_max_a / textbook, 1.05 / 0.9751, 0.002);
        // R/X from the losses: R = 6500/630000·0.25397 Ω... ukr 1.032 %.
        assert!(f.r_over_x > 0.25 && f.r_over_x < 0.30, "{}", f.r_over_x);
    }

    #[test]
    fn supply_current_is_reproduced_at_the_source() {
        let src = SourceModel::from_short_circuit(15.0, Some(10.0), 400.0).unwrap();
        let net = Network::new(400.0, src);
        let f = net.fault(net.root()).unwrap();
        near(f.ik3_max_a, 15_000.0, 1e-9);
        near(f.ik3_min_a, 10_000.0, 1e-9);
        near(f.ik2_max_a, 15_000.0 * 3.0_f64.sqrt() / 2.0, 1e-9);
    }

    #[test]
    fn peak_factor_follows_the_ratio() {
        near(kappa(0.0), 2.0, 1e-12);
        near(kappa(0.1), 1.02 + 0.98 * (-0.3_f64).exp(), 1e-12);
        assert!(kappa(1.0) < 1.07);
        // At R/X = 0.1 the supply source gives ip ≈ 2.7 · Ik.
        let src = SourceModel::from_short_circuit(10.0, None, 400.0).unwrap();
        let net = Network::new(400.0, src);
        let f = net.fault(net.root()).unwrap();
        near(f.ip_a / f.ik3_max_a, kappa(0.1) * 2.0_f64.sqrt(), 1e-9);
    }

    #[test]
    fn cable_current_falls_along_the_run_and_ratio_rises() {
        let src = SourceModel::from_short_circuit(20.0, Some(15.0), 400.0).unwrap();
        let mut net = Network::new(400.0, src);
        let cable = Branch::cable(16.0, 50.0, 1, Insulation::Pvc, Some(16.0)).unwrap();
        let board = net.add_node(net.root(), "DB1", cable);
        let f0 = net.fault(net.root()).unwrap();
        let f1 = net.fault(board).unwrap();
        assert!(f1.ik3_max_a < f0.ik3_max_a);
        assert!(f1.r_over_x > f0.r_over_x);
        assert!(f1.ik1_min_a.unwrap() < f1.ik3_min_a);
        assert!(f1.ik3_min_a < f1.ik3_max_a);
    }

    #[test]
    fn earth_fault_matches_a_hand_calculation_and_the_phase_domain() {
        // Source: 10 kA stated. 20 m of 4 mm² with 4 mm² PE, PVC, hot.
        let src = SourceModel::from_short_circuit(10.0, Some(10.0), 400.0).unwrap();
        let mut net = Network::new(400.0, src.clone());
        let b = Branch::cable(4.0, 20.0, 1, Insulation::Pvc, Some(4.0)).unwrap();
        let n = net.add_node(net.root(), "M1", b);
        let f = net.fault(n).unwrap();

        // Hand: loop = source + 20 m · 2 conductors at 70 °C.
        let rho70 = copper_resistivity(70.0);
        let r_cable = rho70 * 20.0 / 4.0; // one conductor
        let x_cable = 0.00008 * 20.0;
        let z1 = src.z1_min + Impedance::new(r_cable, x_cable);
        let z0 = src.z0_min.unwrap()
            + Impedance::new(r_cable, x_cable)
            + Impedance::new(3.0 * r_cable, 3.0 * x_cable);
        // Independent solution in phase quantities.
        let expected = phase_domain_slg(z1, z0, 0.95 * 400.0 / 3.0_f64.sqrt());
        near(f.ik1_min_a.unwrap(), expected, 1e-9);
        // The loop impedance seen by a fault: source part + 2 · 20 m of 4 mm².
        let loop_r = f.loop_min.unwrap().r;
        near(loop_r, src.z1_min.r + 2.0 * r_cable, 1e-9);
        // About 20 m of 4 mm² each way, hot: 0.2 Ω of copper.
        assert!(f.ik1_min_a.unwrap() > 800.0 && f.ik1_min_a.unwrap() < 1200.0);
    }

    #[test]
    fn unknown_pe_gives_unknown_earth_fault_not_zero() {
        let src = SourceModel::from_short_circuit(10.0, None, 400.0).unwrap();
        let mut net = Network::new(400.0, src);
        let b = Branch::cable(4.0, 20.0, 1, Insulation::Pvc, None).unwrap();
        let n = net.add_node(net.root(), "M1", b);
        let f = net.fault(n).unwrap();
        assert_eq!(f.ik1_min_a, None);
        assert_eq!(f.ik1_max_a, None);
        assert!(f.loop_min.is_none());
        assert!(f.ik3_min_a > 0.0);
    }

    #[test]
    fn parallel_cables_halve_the_impedance() {
        let one = Branch::cable(95.0, 30.0, 1, Insulation::Xlpe, Some(50.0)).unwrap();
        let two = Branch::cable(95.0, 30.0, 2, Insulation::Xlpe, Some(50.0)).unwrap();
        near(two.cold.r, one.cold.r / 2.0, 1e-12);
        near(two.cold.x, one.cold.x / 2.0, 1e-12);
        // The protective conductor is not duplicated.
        assert_eq!(two.pe, one.pe);
    }

    #[test]
    fn motors_add_to_the_maximum_only_when_they_matter() {
        let src = SourceModel::from_short_circuit(10.0, Some(8.0), 400.0).unwrap();
        let mut net = Network::new(400.0, src);
        let b = net.add_node(
            net.root(),
            "MCC",
            Branch::cable(50.0, 10.0, 1, Insulation::Pvc, Some(25.0)).unwrap(),
        );
        let base = net.fault(b).unwrap();
        // One small motor: rated current below 1 % of Ik'' is ignored.
        net.add_motors(
            b,
            Motor {
                rated_current_a: 10.0,
                locked_rotor_ratio: 6.0,
                count: 1,
            },
        );
        let small = net.fault(b).unwrap();
        near(small.ik3_max_a, base.ik3_max_a, 1e-12);
        // 20 motors of 20 A: counted. ILR = 6·20 = 120 A each → +c·...
        net.add_motors(
            b,
            Motor {
                rated_current_a: 20.0,
                locked_rotor_ratio: 6.0,
                count: 20,
            },
        );
        let big = net.fault(b).unwrap();
        assert!(big.ik3_max_a > base.ik3_max_a);
        assert!(big.motor_share > 0.0 && big.motor_share < 0.5);
        assert!(big.ip_a > base.ip_a);
        // Minimum currents never include motors.
        near(big.ik3_min_a, base.ik3_min_a, 1e-12);
        // A motor contributes less at the board above it, behind the cable.
        let root = net.fault(net.root()).unwrap();
        assert!(root.motor_share < big.motor_share);
    }

    #[test]
    fn transformer_behind_a_network_is_weaker_than_stiff() {
        let stiff = SourceModel::from_transformer(&Transformer::new(400.0, 4.0, 400.0)).unwrap();
        let mut t = Transformer::new(400.0, 4.0, 400.0);
        t.network_sk_mva = Some((250.0, 150.0));
        let weak = SourceModel::from_transformer(&t).unwrap();
        let a = Network::new(400.0, stiff);
        let b = Network::new(400.0, weak);
        let fa = a.fault(a.root()).unwrap();
        let fb = b.fault(b.root()).unwrap();
        assert!(fb.ik3_max_a < fa.ik3_max_a);
        assert!(fb.ik3_min_a < fb.ik3_max_a);
        // Network impedance: c·Un²/Sk'' = 1.1·0.16/250 = 0.704 mΩ.
        near(
            weak_z(&t).magnitude()
                - SourceModel::from_transformer(&Transformer::new(400.0, 4.0, 400.0))
                    .unwrap()
                    .z1_max
                    .magnitude(),
            0.001_1 * 0.16 / 0.25,
            0.01,
        );
    }

    fn weak_z(t: &Transformer) -> Impedance {
        SourceModel::from_transformer(t).unwrap().z1_max
    }

    #[test]
    fn invalid_data_is_refused() {
        assert!(SourceModel::from_short_circuit(0.0, None, 400.0).is_none());
        assert!(SourceModel::from_short_circuit(10.0, Some(12.0), 400.0).is_none());
        assert!(SourceModel::from_transformer(&Transformer::new(0.0, 4.0, 400.0)).is_none());
        let mut t = Transformer::new(630.0, 4.0, 400.0);
        t.pk_w = Some(1.0e9);
        assert!(SourceModel::from_transformer(&t).is_none());
        assert!(Branch::cable(0.0, 10.0, 1, Insulation::Pvc, None).is_none());
        assert!(Branch::cable(4.0, 10.0, 0, Insulation::Pvc, None).is_none());
    }

    #[test]
    fn assumptions_are_listed() {
        let src = SourceModel::from_short_circuit(10.0, None, 400.0).unwrap();
        assert!(src.assumptions.iter().any(|a| a.contains("optimistic")));
        let t = SourceModel::from_transformer(&Transformer::new(630.0, 4.0, 400.0)).unwrap();
        assert!(t.assumptions.iter().any(|a| a.contains("R/X = 0.2")));
        assert!(t.assumptions.iter().any(|a| a.contains("KT")));
    }
}
