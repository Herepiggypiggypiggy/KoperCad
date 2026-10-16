//! Short-circuit current rating (SCCR) of a control panel, after the method of
//! UL 508A Supplement SB in its simplified form.
//!
//! The panel's rating is the lowest rating among the components of the power
//! circuit. A component downstream of a current-limiting protective device
//! whose let-through current is within the component's own rating is covered
//! up to the limiter's rating, so it no longer lowers the panel's.
//!
//! This is a model, and the report says so. What it does:
//!
//! - Components are the devices on power circuits: disconnecting devices,
//!   breakers, fuses, contactors, overload relays, motor protectors, drives,
//!   transformers and terminal strips.
//! - A component's rating is the `sccr_ka` marked on the device or its part;
//!   for a protective device else its interrupting rating
//!   (`breaking_capacity_ka`); else the default the project enters for its
//!   type under `standards.ul508a.default_sccr_ka.<type>` (copied from the
//!   engineer's own copy of the unmarked-component table, which is not built
//!   in); else unknown.
//! - A limiter is a protective device with `current_limiting = true` and its
//!   `let_through_ka`, from the maker's let-through data for the available
//!   fault current.
//!
//! What it does not do: combination ratings of motor controllers tested
//! together, rating reductions behind transformers, and the special cases of
//! the supplement. A panel that needs them is rated by the engineer.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use novo_core::{DeviceId, DeviceKind, SymbolId, SymbolRole};

use super::netclass::Circuit;
use super::symbols::{Class, class_of};
use super::{Env, as_bool, as_f64, device_property, keys};

/// Where a component's rating comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SccrSource {
    /// Marked on the device or its part (`sccr_ka`).
    Marked,
    /// A protective device's interrupting rating (`breaking_capacity_ka`).
    Interrupting,
    /// The project's entry for the component type.
    Default,
    /// Nothing states it.
    Unknown,
}

impl SccrSource {
    pub fn label(self) -> &'static str {
        match self {
            SccrSource::Marked => "marked",
            SccrSource::Interrupting => "interrupting rating",
            SccrSource::Default => "project default",
            SccrSource::Unknown => "unknown",
        }
    }
}

/// One component of the power circuit and what limits the panel through it.
#[derive(Debug, Clone, PartialEq)]
pub struct SccrComponent {
    pub device: DeviceId,
    pub tag: String,
    /// The type name used for defaults: `breaker`, `fuse`, `disconnect`,
    /// `contactor`, `overload`, `motor_protector`, `drive`, `transformer`,
    /// `terminal`.
    pub kind: &'static str,
    pub rating_ka: Option<f64>,
    pub source: SccrSource,
    /// What the component contributes to the panel after current-limiting
    /// protection upstream. `None` while its rating is unknown.
    pub effective_ka: Option<f64>,
    /// The limiter that raised `effective_ka` above `rating_ka`.
    pub limited_by: Option<DeviceId>,
    pub symbols: Vec<SymbolId>,
}

/// The result of the calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct SccrReport {
    /// Available fault current at the supply, kA, when the project states it.
    pub available_ka: Option<f64>,
    /// The lowest effective rating among components whose rating is known.
    /// The panel's rating when [`complete`](SccrReport::complete); an upper
    /// bound when not.
    pub panel_ka: Option<f64>,
    /// Whether every component has a rating.
    pub complete: bool,
    /// The components that set `panel_ka`.
    pub limiting: Vec<DeviceId>,
    /// Components in supply order, nearest the source first.
    pub components: Vec<SccrComponent>,
}

fn kind_of(class: Class) -> Option<&'static str> {
    Some(match class {
        Class::Disconnector | Class::FuseSwitch => "disconnect",
        Class::Breaker => "breaker",
        Class::Fuse => "fuse",
        Class::Contactor => "contactor",
        Class::OverloadRelay => "overload",
        Class::MotorProtectionBreaker | Class::MotorCircuitProtector => "motor_protector",
        Class::Drive | Class::SoftStarter => "drive",
        Class::Transformer => "transformer",
        _ => return None,
    })
}

fn is_protector(kind: &str) -> bool {
    matches!(kind, "breaker" | "fuse" | "disconnect" | "motor_protector")
}

/// Calculates the panel's short-circuit current rating.
pub(super) fn compute(e: &Env<'_>) -> Option<SccrReport> {
    let project = e.project;
    let nets = &e.nets;
    // device -> (kind, symbols on power circuits)
    let mut found: BTreeMap<DeviceId, (&'static str, Vec<SymbolId>)> = BTreeMap::new();
    for s in project.symbols() {
        let SymbolRole::Device { device, .. } = s.role else {
            continue;
        };
        let Some(kind) = kind_of(class_of(s)) else {
            continue;
        };
        let live = nets.live_nets_of(s);
        let on_power = live.iter().any(|n| nets.circuit(*n) == Circuit::Power);
        if !on_power {
            continue;
        }
        found
            .entry(device)
            .or_insert((kind, Vec::new()))
            .1
            .push(s.id);
    }
    // Terminal strips with a terminal on a power net.
    for s in project.symbols() {
        let SymbolRole::Terminal { terminals } = &s.role else {
            continue;
        };
        if !nets
            .live_nets_of(s)
            .iter()
            .any(|n| nets.circuit(*n) == Circuit::Power)
        {
            continue;
        }
        for t in terminals {
            if let Some(strip) = project.terminal(*t).map(|t| t.strip)
                && project
                    .device(strip)
                    .is_some_and(|d| d.kind == DeviceKind::TerminalStrip)
            {
                found
                    .entry(strip)
                    .or_insert(("terminal", Vec::new()))
                    .1
                    .push(s.id);
            }
        }
    }
    if found.is_empty() {
        return None;
    }

    let default_for = |kind: &str| {
        e.params
            .number(&format!("standards.ul508a.default_sccr_ka.{kind}"))
    };
    let num = |device: DeviceId, key: &str| device_property(project, device, key).and_then(as_f64);

    struct Work {
        component: SccrComponent,
        dist: u32,
        up: BTreeSet<SymbolId>,
        let_through: Option<f64>,
    }
    let mut work: Vec<Work> = Vec::new();
    for (device, (kind, symbols)) in found {
        let (rating_ka, source) = if let Some(v) = num(device, keys::SCCR_KA) {
            (Some(v), SccrSource::Marked)
        } else if let Some(v) = is_protector(kind)
            .then(|| num(device, keys::BREAKING_CAPACITY_KA))
            .flatten()
        {
            (Some(v), SccrSource::Interrupting)
        } else if let Some(v) = default_for(kind) {
            (Some(v), SccrSource::Default)
        } else {
            (None, SccrSource::Unknown)
        };
        let mut dist = u32::MAX;
        let mut up: BTreeSet<SymbolId> = BTreeSet::new();
        for sym in &symbols {
            let Some(s) = project.symbol(*sym) else {
                continue;
            };
            for n in nets.live_nets_of(s) {
                if nets.circuit(n) != Circuit::Power {
                    continue;
                }
                if let Some(d) = nets.distance(n) {
                    dist = dist.min(d);
                }
                up.extend(nets.upstream(n).poles.keys().copied());
            }
        }
        let limiting = is_protector(kind)
            && device_property(project, device, keys::CURRENT_LIMITING)
                .and_then(as_bool)
                .unwrap_or(false);
        let let_through = limiting
            .then(|| num(device, keys::LET_THROUGH_KA))
            .flatten();
        work.push(Work {
            component: SccrComponent {
                device,
                tag: e.tag(device),
                kind,
                rating_ka,
                source,
                effective_ka: None,
                limited_by: None,
                symbols,
            },
            dist,
            up,
            let_through,
        });
    }
    work.sort_by(|a, b| {
        a.dist
            .cmp(&b.dist)
            .then(a.component.tag.cmp(&b.component.tag))
            .then(a.component.device.cmp(&b.component.device))
    });

    // Effective ratings, nearest the source first, so a limiter's own
    // effective rating is known when the components behind it are rated.
    let mut effective: HashMap<DeviceId, f64> = HashMap::new();
    let limiters: Vec<usize> = (0..work.len())
        .filter(|i| work[*i].let_through.is_some())
        .collect();
    for i in 0..work.len() {
        let Some(rating) = work[i].component.rating_ka else {
            continue;
        };
        let mut best = rating;
        let mut by = None;
        for &l in &limiters {
            if l == i {
                continue;
            }
            let lim = &work[l];
            let behind = lim.component.symbols.iter().any(|s| work[i].up.contains(s));
            let Some(lt) = lim.let_through else { continue };
            let Some(lim_eff) = effective.get(&lim.component.device).copied() else {
                continue;
            };
            if behind && lt <= rating + 1e-9 && lim_eff > best {
                best = lim_eff;
                by = Some(lim.component.device);
            }
        }
        effective.insert(work[i].component.device, best);
        work[i].component.effective_ka = Some(best);
        work[i].component.limited_by = by;
    }

    let components: Vec<SccrComponent> = work.into_iter().map(|w| w.component).collect();
    let panel_ka = components
        .iter()
        .filter_map(|c| c.effective_ka)
        .reduce(f64::min);
    let limiting = match panel_ka {
        Some(p) => components
            .iter()
            .filter(|c| c.effective_ka.is_some_and(|v| (v - p).abs() < 1e-9))
            .map(|c| c.device)
            .collect(),
        None => Vec::new(),
    };
    Some(SccrReport {
        available_ka: e.params.number(keys::SUPPLY_SHORT_CIRCUIT_KA),
        panel_ka,
        complete: components.iter().all(|c| c.rating_ka.is_some()),
        limiting,
        components,
    })
}
