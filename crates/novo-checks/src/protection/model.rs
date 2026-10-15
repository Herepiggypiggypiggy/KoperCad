//! Reads the project into the protection engine: which protective devices
//! there are, what each one is, and which device sits behind which, from
//! the netlist.
//!
//! Nothing here guesses silently. A value the project does not state is
//! either left unknown (the check then says nothing) or filled with a
//! stated default that is listed in [`ProjectDevice::assumed`], so the view
//! can show the designer what the analysis took for granted.
//!
//! Numbers come from properties on the device, else on its first part that
//! has them (the same convention as the calculation checks):
//!
//! | Key | Meaning |
//! | --- | --- |
//! | `rated_current_a` | rating `In` (a relay or motor breaker: top of its range) |
//! | `setting_a` | overload relay or motor breaker setting `Ir`, A |
//! | `breaking_capacity_ka` | `Icu`, kA |
//! | `trip_curve` | `B`, `C`, `D`, `K`, `Z` of a miniature breaker |
//! | `trip_factor` | instantaneous trip as a multiple of the rating (picks the curve letter when `trip_curve` is missing; sets the magnetic trip of a motor breaker) |
//! | `trip_class` | overload relay class: `10A`, `10`, `20`, `30` |
//! | `pre_arcing_i2t` | fuse melting energy, A²s (otherwise estimated) |
//! | `sccr_ka` | short-circuit current rating marked on the device, kA |
//! | `fuse_class` | UL fuse class (`J`, `CC`, `RK5`…), for SCCR combinations |
//! | `current_limiting` | breaker is a listed current-limiting type |
//! | `supply.short_circuit_ka` | project: prospective current at the supply, kA |
//!
//! Which devices are protective, and which side is the line side, follows
//! the symbol library (`iec/protection/…`, odd pins on the line side, IEC
//! 60445). A contactor between two protective devices (a motor starter) is
//! passed through, its pole pins pairing 1-2, 3-4, 5-6.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    DeviceId, Endpoint, Netlist, Project, Properties, SymbolInstance, SymbolRole, Value,
};

use super::coordination::{self, Finding, Node};
use super::curves::{McbCurve, RelayClass, TripCurve};
use super::devices::{Device, MakerTables};
use super::sccr::{self, Branch, Class, Component, Kind, Panel, Protector};
use super::selectivity::{Backup, Selectivity, Settings};

/// The project property for the prospective current at the supply, kA.
pub const SUPPLY_SHORT_CIRCUIT_KA: &str = "supply.short_circuit_ka";

/// What kind of protective device a symbol is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceType {
    /// Miniature breaker (also RCBO).
    Mcb,
    /// Fuse.
    Fuse,
    /// Motor protection breaker.
    MotorBreaker,
    /// Overload relay.
    OverloadRelay,
}

fn device_type(symbol: &SymbolInstance) -> Option<DeviceType> {
    let key = symbol.symbol.0.as_str();
    if !key.contains("/protection/") {
        return None;
    }
    let name = key.rsplit('/').next()?;
    if name.starts_with("mcb") || name.starts_with("rcbo") {
        Some(DeviceType::Mcb)
    } else if name.starts_with("fuse") {
        Some(DeviceType::Fuse)
    } else if name.starts_with("motor_protection_breaker") {
        Some(DeviceType::MotorBreaker)
    } else if name.starts_with("overload_relay") {
        Some(DeviceType::OverloadRelay)
    } else {
        None
    }
}

fn number(properties: &Properties, key: &str) -> Option<f64> {
    match properties.get(key)? {
        Value::Int(i) => Some(*i as f64),
        Value::Float(f) => Some(*f),
        Value::Text(t) => t.trim().parse().ok(),
        Value::Bool(_) => None,
    }
}

fn text(properties: &Properties, key: &str) -> Option<String> {
    match properties.get(key)? {
        Value::Text(t) => Some(t.trim().to_string()),
        Value::Int(i) => Some(i.to_string()),
        Value::Float(f) => Some(f.to_string()),
        Value::Bool(_) => None,
    }
    .filter(|t| !t.is_empty())
}

/// A property of a device, else of its first part that has it.
fn lookup<T>(
    project: &Project,
    device: DeviceId,
    read: impl Fn(&Properties) -> Option<T>,
) -> Option<T> {
    let d = project.device(device)?;
    read(&d.properties).or_else(|| {
        d.parts
            .iter()
            .filter_map(|p| project.part(*p))
            .find_map(|p| read(&p.properties))
    })
}

/// One protective device of the project, as the engine sees it.
#[derive(Debug, Clone)]
pub struct ProjectDevice {
    /// The model device.
    pub id: DeviceId,
    /// Its tag (`-Q1`).
    pub tag: String,
    /// What it is.
    pub kind: DeviceType,
    /// The engine's device: curve, breaking capacity.
    pub device: Device,
    /// What the analysis took for granted because the project does not
    /// say, in plain language.
    pub assumed: Vec<String>,
    /// Marked short-circuit rating, kA.
    pub sccr_ka: Option<f64>,
    /// Devices directly behind it (indices into [`Study::devices`]).
    pub downstream: Vec<usize>,
    /// Contactors and other pass-through power parts directly behind it:
    /// `(tag, marked SCCR kA)`.
    pub passes: Vec<(String, Option<f64>)>,
}

/// The analysis of a pair of neighbouring protective devices of the project.
#[derive(Debug, Clone, PartialEq)]
pub struct StudyPair {
    /// Index of the upstream device in [`Study::devices`].
    pub upstream: usize,
    /// Index of the downstream device.
    pub downstream: usize,
    /// Selectivity between them.
    pub selectivity: Selectivity,
    /// Back-up protection of the downstream device.
    pub backup: Backup,
}

/// The chain analysis of a study.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StudyReport {
    /// Findings, each once however many chains it lies on.
    pub findings: Vec<Finding>,
    /// One entry per neighbouring pair.
    pub pairs: Vec<StudyPair>,
}

/// The protective devices of a project and how they sit behind each other.
#[derive(Debug, Clone, Default)]
pub struct Study {
    /// The devices, in tag order.
    pub devices: Vec<ProjectDevice>,
    /// Prospective short-circuit current at the supply, kA.
    pub supply_ka: Option<f64>,
}

fn read_device(project: &Project, id: DeviceId, kind: DeviceType) -> Option<ProjectDevice> {
    let d = project.device(id)?;
    let rated = project.rated_current_a(id).filter(|a| *a > 0.0)?;
    let mut assumed = Vec::new();
    let num = |key: &str| lookup(project, id, |p| number(p, key));
    let txt = |key: &str| lookup(project, id, |p| text(p, key));
    let icu_ka = num("breaking_capacity_ka").filter(|k| *k > 0.0);
    // An overload relay does not break a fault: no capacity to enter.
    if icu_ka.is_none() && kind != DeviceType::OverloadRelay {
        assumed.push("breaking capacity not entered, not checked".to_string());
    }
    let factor = num("trip_factor");
    let curve = match kind {
        DeviceType::Mcb => {
            let letter = txt("trip_curve")
                .and_then(|t| McbCurve::parse(&t))
                .or_else(|| {
                    // The letter whose upper instantaneous limit is nearest.
                    factor.and_then(|f| {
                        [
                            McbCurve::Z,
                            McbCurve::B,
                            McbCurve::C,
                            McbCurve::K,
                            McbCurve::D,
                        ]
                        .into_iter()
                        .min_by(|a, b| {
                            let da = (a.instantaneous().hi - f).abs();
                            let db = (b.instantaneous().hi - f).abs();
                            da.total_cmp(&db)
                        })
                    })
                });
            let letter = letter.unwrap_or_else(|| {
                assumed.push("trip curve not entered, curve C assumed".to_string());
                McbCurve::C
            });
            TripCurve::mcb(rated, letter).ok()?
        }
        DeviceType::Fuse => {
            if num("pre_arcing_i2t").is_none() {
                assumed.push(
                    "gG fuse with estimated melting energy (enter the maker's I²t for accuracy)"
                        .to_string(),
                );
            }
            TripCurve::fuse_gg(rated, num("pre_arcing_i2t").filter(|e| *e > 0.0)).ok()?
        }
        DeviceType::OverloadRelay | DeviceType::MotorBreaker => {
            let ir = num("setting_a").filter(|a| *a > 0.0).unwrap_or_else(|| {
                assumed.push("setting not entered, set at the rated current".to_string());
                rated
            });
            let class = txt("trip_class")
                .and_then(|t| RelayClass::parse(&t))
                .unwrap_or_else(|| {
                    assumed.push("trip class not entered, class 10 assumed".to_string());
                    RelayClass::C10
                });
            if kind == DeviceType::OverloadRelay {
                TripCurve::overload_relay(ir, class).ok()?
            } else {
                let magnetic = factor.filter(|f| *f > 7.2).unwrap_or_else(|| {
                    assumed.push(
                        "magnetic trip not entered, 13 times the setting assumed".to_string(),
                    );
                    13.0
                });
                TripCurve::mpcb(ir, class, magnetic).ok()?
            }
        }
    };
    let tag = d.tag.to_string();
    let mut device = Device::new(tag.clone(), curve, icu_ka.unwrap_or(f64::INFINITY));
    if icu_ka.is_none() {
        device.icu_a = f64::INFINITY;
    }
    Some(ProjectDevice {
        id,
        tag,
        kind,
        device,
        assumed,
        sccr_ka: num("sccr_ka").filter(|k| *k > 0.0),
        downstream: Vec::new(),
        passes: Vec::new(),
    })
}

fn pin_number(symbol: &SymbolInstance, pin: usize) -> Option<u32> {
    symbol.pins.get(pin)?.name.trim().parse().ok()
}

impl Study {
    /// Reads the project.
    pub fn read(project: &Project, netlist: &Netlist) -> Study {
        let mut devices: Vec<ProjectDevice> = Vec::new();
        let mut index: BTreeMap<DeviceId, usize> = BTreeMap::new();
        // One symbol per device decides what the device is.
        let mut symbol_of: BTreeMap<DeviceId, &SymbolInstance> = BTreeMap::new();
        for s in project.symbols() {
            let SymbolRole::Device { device, .. } = s.role else {
                continue;
            };
            if let Some(kind) = device_type(s)
                && !index.contains_key(&device)
                && let Some(pd) = read_device(project, device, kind)
            {
                index.insert(device, devices.len());
                devices.push(pd);
                symbol_of.insert(device, s);
            }
        }
        // Tag order, natural.
        let mut order: Vec<usize> = (0..devices.len()).collect();
        order.sort_by(|&a, &b| novo_core::natural_cmp(&devices[a].tag, &devices[b].tag));
        let mut sorted: Vec<ProjectDevice> = order.iter().map(|&i| devices[i].clone()).collect();
        let mut index: BTreeMap<DeviceId, usize> = BTreeMap::new();
        for (i, d) in sorted.iter().enumerate() {
            index.insert(d.id, i);
        }
        for entry in &mut sorted {
            let id = entry.id;
            let Some(symbol) = symbol_of.get(&id) else {
                continue;
            };
            let (down, passes) = downstream_of(project, netlist, symbol, id, &index);
            entry.downstream = down;
            entry.passes = passes
                .into_iter()
                .map(|d| {
                    let tag = project
                        .device(d)
                        .map_or_else(String::new, |x| x.tag.to_string());
                    (tag, lookup(project, d, |p| number(p, "sccr_ka")))
                })
                .collect();
        }
        Study {
            devices: sorted,
            supply_ka: number(&project.info().properties, SUPPLY_SHORT_CIRCUIT_KA)
                .filter(|k| *k > 0.0),
        }
    }

    /// Whether any device is upstream of device `i`.
    pub fn has_upstream(&self, i: usize) -> bool {
        self.devices.iter().any(|d| d.downstream.contains(&i))
    }

    /// Every path from a device with nothing upstream down to a device with
    /// nothing behind it, as indices, upstream first. A device behind
    /// several upstream ones appears in a path for each.
    pub fn chains(&self) -> Vec<Vec<usize>> {
        let mut out = Vec::new();
        for root in (0..self.devices.len()).filter(|&i| !self.has_upstream(i)) {
            let mut path = vec![root];
            self.walk(&mut path, &mut out);
        }
        out
    }

    fn walk(&self, path: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        let last = *path.last().unwrap_or(&0);
        let next: Vec<usize> = self.devices[last]
            .downstream
            .iter()
            .copied()
            .filter(|n| !path.contains(n))
            .collect();
        if next.is_empty() {
            out.push(path.clone());
            return;
        }
        for n in next {
            path.push(n);
            self.walk(path, out);
            path.pop();
        }
    }

    /// The chain analysis over every path, each finding once. The fault
    /// current at every device is taken as the supply's: an upper bound,
    /// since cable impedance along the way lowers it.
    pub fn report(&self, tables: &MakerTables, settings: &Settings) -> StudyReport {
        let ik = self.supply_ka.map_or(f64::NAN, |k| k * 1000.0);
        let mut merged = StudyReport::default();
        let mut seen_pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
        let mut seen_findings: BTreeSet<(String, String)> = BTreeSet::new();
        for path in self.chains() {
            let nodes: Vec<Node> = path
                .iter()
                .map(|&i| Node {
                    label: self.devices[i].tag.clone(),
                    device: self.devices[i].device.clone(),
                    ik_max_a: ik,
                    ik_min_a: None,
                    conductor: None,
                    max_disconnect_s: None,
                })
                .collect();
            let r = coordination::analyse_chain(&nodes, tables, settings);
            for f in r.findings {
                if seen_findings.insert((f.rule.key().to_string(), f.message.clone())) {
                    merged.findings.push(f);
                }
            }
            for p in r.pairs {
                let key = (path[p.upstream], path[p.upstream + 1]);
                if seen_pairs.insert(key) {
                    merged.pairs.push(StudyPair {
                        upstream: key.0,
                        downstream: key.1,
                        selectivity: p.selectivity,
                        backup: p.backup,
                    });
                }
            }
        }
        merged
    }

    /// The panel's power circuit for the SCCR analysis: protective devices
    /// nest as the chains do, the contactors between them are components of
    /// the branch they hang on. Devices with a marked `sccr_ka` use it, the
    /// others the default for their kind.
    pub fn sccr_panel(&self, name: &str) -> Panel {
        let mut root = Branch {
            name: name.to_string(),
            ..Default::default()
        };
        let roots: Vec<usize> = (0..self.devices.len())
            .filter(|&i| !self.has_upstream(i))
            .collect();
        let mut placed: BTreeSet<usize> = BTreeSet::new();
        for r in roots {
            if let Some(b) = self.branch(r, &mut placed) {
                root.branches.push(b);
            }
        }
        Panel {
            name: name.to_string(),
            root,
            available_ka: self.supply_ka,
        }
    }

    fn branch(&self, i: usize, placed: &mut BTreeSet<usize>) -> Option<Branch> {
        if !placed.insert(i) {
            return None;
        }
        let d = &self.devices[i];
        let kind = match d.kind {
            DeviceType::Fuse => Kind::FuseHolder,
            DeviceType::OverloadRelay => Kind::OverloadRelay,
            DeviceType::Mcb | DeviceType::MotorBreaker => Kind::CircuitBreaker,
        };
        let class = match d.kind {
            DeviceType::Fuse => Class::Fuse("gG".into()),
            _ => Class::Breaker,
        };
        let mut branch = Branch {
            name: d.tag.clone(),
            protector: Some(Protector {
                component: Component::new(d.tag.clone(), kind, d.sccr_ka),
                class,
                rating_a: d.device.rated_a(),
                let_through: None,
            }),
            ..Default::default()
        };
        for (tag, sccr) in &d.passes {
            branch
                .components
                .push(Component::new(tag.clone(), Kind::MotorController, *sccr));
        }
        for &n in &d.downstream {
            if let Some(b) = self.branch(n, placed) {
                branch.branches.push(b);
            }
        }
        Some(branch)
    }

    /// The SCCR of the panel read from the project.
    pub fn sccr(&self, name: &str) -> sccr::Report {
        sccr::analyse(&self.sccr_panel(name))
    }
}

/// The protective devices directly behind `symbol` (through its load-side
/// pins, odd-even pole pass-through of other devices) and the pass-through
/// devices met on the way.
fn downstream_of(
    project: &Project,
    netlist: &Netlist,
    symbol: &SymbolInstance,
    device: DeviceId,
    index: &BTreeMap<DeviceId, usize>,
) -> (Vec<usize>, Vec<DeviceId>) {
    let mut found: BTreeSet<usize> = BTreeSet::new();
    let mut passes: BTreeSet<DeviceId> = BTreeSet::new();
    let mut seen_nets: BTreeSet<usize> = BTreeSet::new();
    // (symbol, pin) pairs to start from.
    let mut work: Vec<Endpoint> = Vec::new();
    for pin in 0..symbol.pins.len() {
        if pin_number(symbol, pin).is_some_and(|n| n % 2 == 0)
            && let Ok(p) = u16::try_from(pin)
        {
            work.push(Endpoint::pin(symbol.id, p));
        }
    }
    let mut depth = 0;
    while !work.is_empty() && depth < 6 {
        depth += 1;
        let mut next: Vec<Endpoint> = Vec::new();
        for end in work {
            let Some(net) = netlist.net_of(end) else {
                continue;
            };
            if !seen_nets.insert(net.0 as usize) {
                continue;
            }
            for e in &netlist.net(net).endpoints {
                let Endpoint::Pin { symbol: s2, pin } = *e else {
                    continue;
                };
                let Some(other) = project.symbol(s2) else {
                    continue;
                };
                let SymbolRole::Device { device: d2, .. } = other.role else {
                    continue;
                };
                if d2 == device {
                    continue;
                }
                let Some(n) = pin_number(other, pin as usize).filter(|n| n % 2 == 1) else {
                    continue;
                };
                if let Some(&i) = index.get(&d2) {
                    found.insert(i);
                } else if device_type(other).is_none() {
                    // A pole: 1-2, 3-4, 5-6.
                    let mate = other
                        .pins
                        .iter()
                        .position(|p| p.name.trim() == (n + 1).to_string());
                    if let Some(m) = mate.and_then(|m| u16::try_from(m).ok()) {
                        passes.insert(d2);
                        next.push(Endpoint::pin(s2, m));
                    }
                }
            }
        }
        work = next;
    }
    (found.into_iter().collect(), passes.into_iter().collect())
}
