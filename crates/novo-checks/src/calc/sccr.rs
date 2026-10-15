//! Short-circuit current rating of an assembly: the UL 508A procedure
//! (Supplement SB) for industrial control panels, and the IEC 61439-1
//! short-circuit verification for low-voltage assemblies.
//!
//! # UL 508A SB
//!
//! The rating of a panel is the lowest rating of the power-circuit
//! components in it, unless a current-limiting device upstream of a branch
//! lets the components below it be rated at the (lower) current that gets
//! through. [`panel_sccr`] does exactly that:
//!
//! 1. every power-circuit component has a rating: the one marked on it, else
//!    the default for its kind from [`DefaultTable`] (SB4.1), else it is
//!    *unknown* and the panel rating is unknown;
//! 2. the *feeder* is everything in series before the branch protective
//!    devices (main disconnect, feeder breaker, busbar, distribution
//!    blocks); the panel is limited by the lowest of them;
//! 3. each *branch* is limited by its protective device and everything
//!    below it; a branch with a current-limiting device ([`Limiter`])
//!    only has to survive what that device lets through at the panel's
//!    fault level, so its components may be rated lower;
//! 4. the panel rating is the lowest of the feeder and every branch.
//!
//! What needs your licensed copy: the **default table** (SB4.1 values here
//! are planning values, [`DefaultTable::builtin`] is unverified), and the
//! **limiter let-through** table, which is the maker's data for the specific
//! fuse class or breaker (UL 248 / UL 489 limits). Nothing is credited for a
//! limiter without data. A control circuit of 1 kVA or less (SB5) is not a
//! power-circuit component and does not take part; mark it
//! `power_circuit: false`.
//!
//! # IEC 61439-1
//!
//! [`check_iec61439`] compares the prospective current at the incoming
//! terminals of the assembly with its rated short-time withstand current
//! `Icw` (with its time), or with the conditional rating `Icc` when a
//! short-circuit protective device upstream is named, and the prospective
//! peak with the peak withstand `n · Icw` (IEC 60947-1 Table 16 factors).
//! Assemblies with a prospective current of 10 kA rms or less need no
//! verification of short-circuit withstand for the main circuits (IEC
//! 61439-1 §10.11.2, as recalled; verify), which the check reports.

use std::collections::HashMap;

/// A kind of power-circuit component for the default table.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Kind {
    BusBar,
    CircuitBreaker,
    CurrentMeter,
    CurrentShunt,
    FuseHolder,
    OverloadRelay,
    MotorController,
    Switch,
    TerminalBlock,
    DistributionBlock,
    SupplementaryProtector,
    /// Anything else: has no default; needs a marked rating.
    Other(String),
}

impl Kind {
    pub fn key(&self) -> String {
        match self {
            Kind::BusBar => "busbar".into(),
            Kind::CircuitBreaker => "circuit_breaker".into(),
            Kind::CurrentMeter => "current_meter".into(),
            Kind::CurrentShunt => "current_shunt".into(),
            Kind::FuseHolder => "fuse_holder".into(),
            Kind::OverloadRelay => "overload_relay".into(),
            Kind::MotorController => "motor_controller".into(),
            Kind::Switch => "switch".into(),
            Kind::TerminalBlock => "terminal_block".into(),
            Kind::DistributionBlock => "distribution_block".into(),
            Kind::SupplementaryProtector => "supplementary_protector".into(),
            Kind::Other(s) => s.clone(),
        }
    }

    pub fn from_key(key: &str) -> Kind {
        match key.trim().to_ascii_lowercase().as_str() {
            "busbar" => Kind::BusBar,
            "circuit_breaker" => Kind::CircuitBreaker,
            "current_meter" => Kind::CurrentMeter,
            "current_shunt" => Kind::CurrentShunt,
            "fuse_holder" => Kind::FuseHolder,
            "overload_relay" => Kind::OverloadRelay,
            "motor_controller" => Kind::MotorController,
            "switch" => Kind::Switch,
            "terminal_block" => Kind::TerminalBlock,
            "distribution_block" => Kind::DistributionBlock,
            "supplementary_protector" => Kind::SupplementaryProtector,
            other => Kind::Other(other.to_owned()),
        }
    }
}

/// Default ratings in kA for components with no marked SCCR (UL 508A
/// Table SB4.1).
#[derive(Debug, Clone)]
pub struct DefaultTable {
    pub name: String,
    values: HashMap<String, f64>,
    pub licensed: bool,
}

impl DefaultTable {
    /// Planning values, entered without access to the standard. Replace
    /// them with [`DefaultTable::from_csv`] from your licensed copy.
    pub fn builtin() -> DefaultTable {
        let values = [
            ("busbar", 10.0),
            ("circuit_breaker", 5.0),
            ("current_meter", 1.0),
            ("current_shunt", 10.0),
            ("fuse_holder", 10.0),
            ("overload_relay", 5.0),
            ("motor_controller", 5.0),
            ("switch", 5.0),
            ("terminal_block", 10.0),
            ("distribution_block", 10.0),
            ("supplementary_protector", 0.2),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        DefaultTable {
            name: "UL 508A Table SB4.1 (built-in planning values, unverified)".into(),
            values,
            licensed: false,
        }
    }

    pub fn default_ka(&self, kind: &Kind) -> Option<f64> {
        self.values.get(&kind.key()).copied()
    }

    /// Lines `<kind>,<kA>`; `#` starts a comment. The result replaces the
    /// built-in table completely and is marked licensed.
    pub fn from_csv(name: &str, csv: &str) -> Result<DefaultTable, String> {
        let mut values = HashMap::new();
        for (i, raw) in csv.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let f: Vec<&str> = line.split(',').map(str::trim).collect();
            match f.as_slice() {
                [kind, ka] => {
                    let ka: f64 = ka
                        .parse()
                        .ok()
                        .filter(|v: &f64| v.is_finite() && *v > 0.0)
                        .ok_or(format!("line {}: kA must be a positive number", i + 1))?;
                    values.insert(Kind::from_key(kind).key(), ka);
                }
                _ => return Err(format!("line {}: expected <kind>,<kA>", i + 1)),
            }
        }
        Ok(DefaultTable {
            name: name.to_owned(),
            values,
            licensed: true,
        })
    }
}

/// A power-circuit component.
#[derive(Debug, Clone, PartialEq)]
pub struct Component {
    pub name: String,
    pub kind: Kind,
    /// SCCR marked on it, kA.
    pub marked_ka: Option<f64>,
    /// False for a control circuit of 1 kVA or less (SB5): it takes no part.
    pub power_circuit: bool,
}

impl Component {
    pub fn new(name: &str, kind: Kind, marked_ka: Option<f64>) -> Component {
        Component {
            name: name.to_owned(),
            kind,
            marked_ka,
            power_circuit: true,
        }
    }
}

/// A current-limiting branch protective device and what it lets through.
#[derive(Debug, Clone, PartialEq)]
pub struct Limiter {
    /// `(prospective kA, equivalent rms kA let through)`, ascending in the
    /// first. Maker or standard data; outside the range nothing is credited.
    pub let_through: Vec<(f64, f64)>,
}

impl Limiter {
    /// What reaches the branch components when `prospective_ka` is
    /// available upstream. `None` outside the table.
    pub fn limited_ka(&self, prospective_ka: f64) -> Option<f64> {
        let t = &self.let_through;
        let (first, last) = (t.first()?, t.last()?);
        if prospective_ka < first.0 || prospective_ka > last.0 {
            return None;
        }
        let upper = t.iter().position(|p| p.0 >= prospective_ka)?;
        if upper == 0 {
            return Some(first.1);
        }
        let ((x0, y0), (x1, y1)) = (t[upper - 1], t[upper]);
        Some(y0 + (y1 - y0) * (prospective_ka - x0) / (x1 - x0))
    }
}

/// One branch: its protective device, an optional limiter (the device
/// itself, when it is current limiting) and the components below.
#[derive(Debug, Clone, PartialEq)]
pub struct Branch {
    pub name: String,
    pub protective_device: Component,
    pub limiter: Option<Limiter>,
    pub components: Vec<Component>,
}

/// The assembly.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Panel {
    /// Components in series before the branches.
    pub feeder: Vec<Component>,
    pub branches: Vec<Branch>,
}

/// What limits the panel.
#[derive(Debug, Clone, PartialEq)]
pub struct Limit {
    pub ka: f64,
    pub component: String,
    /// `marked`, `default` (SB4.1), or `limiter` (a branch rated at what its
    /// limiter lets through).
    pub basis: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SccrResult {
    /// The panel rating in kA, or `None` if any component's rating is
    /// unknown.
    pub panel_ka: Option<f64>,
    /// The component that sets it.
    pub limiting: Option<Limit>,
    /// Components with no marked rating and no default; the rating is
    /// unknown until they are given one.
    pub unknown: Vec<String>,
    /// Components rated by default, which the designer should replace by
    /// marked values to raise the rating.
    pub defaulted: Vec<String>,
    /// What the table is, for the report.
    pub table: String,
    /// Plain-language lines for the report.
    pub notes: Vec<String>,
}

fn rating(c: &Component, defaults: &DefaultTable) -> Option<(f64, &'static str)> {
    if let Some(k) = c.marked_ka {
        return Some((k, "marked"));
    }
    defaults.default_ka(&c.kind).map(|k| (k, "default"))
}

/// Computes the panel's SCCR, UL 508A SB4 style.
pub fn panel_sccr(panel: &Panel, defaults: &DefaultTable) -> SccrResult {
    let mut unknown = Vec::new();
    let mut defaulted = Vec::new();
    let mut notes = Vec::new();

    // Collects the rating of a component into the running minimum.
    let mut min_of = |c: &Component, acc: &mut Option<Limit>, unknown: &mut Vec<String>| {
        if !c.power_circuit {
            return;
        }
        match rating(c, defaults) {
            None => unknown.push(c.name.clone()),
            Some((ka, basis)) => {
                if basis == "default" {
                    defaulted.push(c.name.clone());
                }
                if acc.as_ref().is_none_or(|l| ka < l.ka) {
                    *acc = Some(Limit {
                        ka,
                        component: c.name.clone(),
                        basis,
                    });
                }
            }
        }
    };

    // Feeder and branch protective devices cap the panel.
    let mut base: Option<Limit> = None;
    for c in &panel.feeder {
        min_of(c, &mut base, &mut unknown);
    }
    for b in &panel.branches {
        min_of(&b.protective_device, &mut base, &mut unknown);
    }
    // Branch components: those below a limiter are judged at what gets
    // through; the others cap the panel directly.
    let mut limited_branches: Vec<(&Branch, Limit)> = Vec::new();
    for b in &panel.branches {
        let mut lowest: Option<Limit> = None;
        for c in &b.components {
            min_of(c, &mut lowest, &mut unknown);
        }
        match (&b.limiter, lowest) {
            (_, None) => {}
            (None, Some(l)) => {
                if base.as_ref().is_none_or(|x| l.ka < x.ka) {
                    base = Some(l);
                }
            }
            (Some(_), Some(l)) => limited_branches.push((b, l)),
        }
    }

    if !unknown.is_empty() {
        unknown.sort();
        unknown.dedup();
        notes.push(format!(
            "no marked rating and no default for: {}",
            unknown.join(", ")
        ));
        return SccrResult {
            panel_ka: None,
            limiting: None,
            unknown,
            defaulted,
            table: defaults.name.clone(),
            notes,
        };
    }

    // Candidate panel rating before limiter credit. Without a power-circuit
    // component at all there is nothing to rate.
    let Some(mut limit) = base else {
        notes.push("no power-circuit components".into());
        return SccrResult {
            panel_ka: None,
            limiting: None,
            unknown,
            defaulted,
            table: defaults.name.clone(),
            notes,
        };
    };

    // Lower the panel rating until every limited branch survives what its
    // limiter lets through at that rating. The let-through rises with the
    // prospective current, so step down from the candidate by bisection on
    // the monotone table.
    for (branch, lowest) in &limited_branches {
        let limiter = branch.limiter.as_ref().expect("limited branch");
        let survives = |p: f64| limiter.limited_ka(p).is_some_and(|l| l <= lowest.ka + 1e-9);
        if survives(limit.ka) {
            notes.push(format!(
                "branch {}: its limiter lets through {:.1} kA at {:.1} kA, so the components below (lowest {:.1} kA, {}) are enough",
                branch.name,
                limiter.limited_ka(limit.ka).unwrap_or(0.0),
                limit.ka,
                lowest.ka,
                lowest.component
            ));
            continue;
        }
        // Largest p ≤ limit.ka that survives, found on the table's range.
        let lo_bound = limiter.let_through.first().map_or(0.0, |p| p.0);
        let (mut lo, mut hi) = (lo_bound, limit.ka);
        if limiter.limited_ka(limit.ka).is_none() || !survives(lo) {
            // Not even the smallest tabulated current is enough, or the
            // table does not cover the range: no credit, the lowest component
            // caps the panel.
            notes.push(format!(
                "branch {}: no credit for the limiter (no data at this level); the lowest component {} caps the panel at {:.1} kA",
                branch.name, lowest.component, lowest.ka
            ));
            if lowest.ka < limit.ka {
                limit = lowest.clone();
            }
            continue;
        }
        for _ in 0..60 {
            let mid = 0.5 * (lo + hi);
            if survives(mid) {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        if lo < limit.ka {
            notes.push(format!(
                "branch {}: the limiter lets {:.1} kA through at {:.1} kA; the components below hold only {:.1} kA ({}), so the panel is rated {:.1} kA",
                branch.name,
                limiter.limited_ka(limit.ka).unwrap_or(0.0),
                limit.ka,
                lowest.ka,
                lowest.component,
                lo
            ));
            limit = Limit {
                ka: lo,
                component: lowest.component.clone(),
                basis: "limiter",
            };
        }
    }

    if !defaulted.is_empty() {
        defaulted.sort();
        defaulted.dedup();
        notes.push(format!(
            "rated by default, not by marking: {}; a marked rating may raise the panel",
            defaulted.join(", ")
        ));
    }
    SccrResult {
        panel_ka: Some(limit.ka),
        limiting: Some(limit),
        unknown,
        defaulted,
        table: defaults.name.clone(),
        notes,
    }
}

/// Whether the panel is fit for the available fault current.
pub fn check_available(result: &SccrResult, available_ka: f64) -> Option<bool> {
    result.panel_ka.map(|p| p + 1e-9 >= available_ka)
}

/// Peak factor `n` for the short-circuit current (IEC 60947-1 Table 16):
/// the ratio of peak to rms current by the rms value, with the associated
/// power factors 0.7, 0.5, 0.3, 0.25 and 0.2.
pub fn peak_factor_n(ik_ka: f64) -> f64 {
    if ik_ka <= 5.0 {
        1.5
    } else if ik_ka <= 10.0 {
        1.7
    } else if ik_ka <= 20.0 {
        2.0
    } else if ik_ka <= 50.0 {
        2.1
    } else {
        2.2
    }
}

/// How an assembly's short-circuit withstand is stated.
#[derive(Debug, Clone, PartialEq)]
pub struct Assembly61439 {
    /// Rated short-time withstand current `Icw` in kA rms, with its time
    /// `tcw` in seconds (normally 1 s, or 3 s).
    pub icw_ka: Option<f64>,
    pub tcw_s: f64,
    /// Rated conditional short-circuit current `Icc` in kA, valid only
    /// with the named upstream short-circuit protective device.
    pub icc_ka: Option<f64>,
    pub icc_scpd: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Verification {
    pub ok: Option<bool>,
    pub text: String,
}

/// Checks an assembly against the prospective current `ik_ka` (rms) and
/// peak `ip_ka` at its incoming terminals. `upstream_scpd` is the id of the
/// protective device actually installed upstream, for the `Icc` route;
/// `clearing_s` the time that device lets the fault flow, for the `Icw`
/// route (must not exceed `tcw`).
pub fn check_iec61439(
    assembly: &Assembly61439,
    ik_ka: f64,
    ip_ka: f64,
    upstream_scpd: Option<&str>,
    clearing_s: Option<f64>,
) -> Verification {
    if ik_ka <= 10.0 {
        return Verification {
            ok: Some(true),
            text: format!(
                "the prospective current {:.1} kA is 10 kA or less: no short-circuit verification is required for the main circuits (IEC 61439-1)",
                ik_ka
            ),
        };
    }
    // Conditional rating with a named upstream device.
    if let (Some(icc), Some(want), Some(have)) =
        (assembly.icc_ka, assembly.icc_scpd.as_deref(), upstream_scpd)
        && want == have
    {
        let ok = icc + 1e-9 >= ik_ka;
        return Verification {
            ok: Some(ok),
            text: format!(
                "conditional rating {:.1} kA with {want} against a prospective {:.1} kA",
                icc, ik_ka
            ),
        };
    }
    // Rated short-time withstand.
    if let Some(icw) = assembly.icw_ka {
        let n = peak_factor_n(icw);
        let peak_ok = n * icw + 1e-9 >= ip_ka;
        let rms_ok = icw + 1e-9 >= ik_ka;
        let time_ok = clearing_s.map(|t| t <= assembly.tcw_s + 1e-9);
        return match time_ok {
            None => Verification {
                ok: None,
                text: format!(
                    "Icw {:.1} kA / {} s covers {:.1} kA rms and {:.1} kA peak (limit {:.1}); the clearing time of the upstream device is not known, so the {} s duration cannot be confirmed",
                    icw,
                    assembly.tcw_s,
                    ik_ka,
                    ip_ka,
                    n * icw,
                    assembly.tcw_s
                ),
            },
            Some(t_ok) => Verification {
                ok: Some(rms_ok && peak_ok && t_ok),
                text: format!(
                    "Icw {:.1} kA for {} s against {:.1} kA rms (needs ≤ Icw: {}), peak {:.1} kA (limit n·Icw = {:.1}: {}), clearing within the rated time: {}",
                    icw,
                    assembly.tcw_s,
                    ik_ka,
                    yes(rms_ok),
                    ip_ka,
                    n * icw,
                    yes(peak_ok),
                    yes(t_ok)
                ),
            },
        };
    }
    Verification {
        ok: None,
        text: "the assembly has neither a rated short-time withstand current nor a conditional rating for the upstream device: its short-circuit withstand is unknown"
            .to_owned(),
    }
}

fn yes(b: bool) -> &'static str {
    if b { "yes" } else { "no" }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &str, kind: Kind, ka: Option<f64>) -> Component {
        Component::new(name, kind, ka)
    }

    fn defaults() -> DefaultTable {
        DefaultTable::builtin()
    }

    #[test]
    fn the_lowest_component_sets_the_rating() {
        let panel = Panel {
            feeder: vec![
                comp("Q0 main switch", Kind::Switch, Some(65.0)),
                comp("X0 busbar", Kind::BusBar, Some(25.0)),
            ],
            branches: vec![Branch {
                name: "M1".into(),
                protective_device: comp("F1", Kind::CircuitBreaker, Some(65.0)),
                limiter: None,
                components: vec![
                    comp("K1", Kind::MotorController, None),
                    comp("X1", Kind::TerminalBlock, None),
                ],
            }],
        };
        let r = panel_sccr(&panel, &defaults());
        // The unmarked motor controller falls back to the 5 kA default.
        assert_eq!(r.panel_ka, Some(5.0));
        let l = r.limiting.unwrap();
        assert_eq!(l.component, "K1");
        assert_eq!(l.basis, "default");
        assert!(r.defaulted.contains(&"K1".to_owned()));
        assert!(r.notes.iter().any(|n| n.contains("rated by default")));
        assert!(r.table.contains("unverified"));
        // Marking the contactor raises the panel to the next lowest (busbar).
        let mut p2 = panel.clone();
        p2.branches[0].components[0].marked_ka = Some(65.0);
        p2.branches[0].components[1].marked_ka = Some(30.0);
        assert_eq!(panel_sccr(&p2, &defaults()).panel_ka, Some(25.0));
    }

    #[test]
    fn unknown_components_make_the_rating_unknown() {
        let panel = Panel {
            feeder: vec![comp("Q0", Kind::Switch, Some(65.0))],
            branches: vec![Branch {
                name: "A".into(),
                protective_device: comp("F1", Kind::CircuitBreaker, Some(65.0)),
                limiter: None,
                components: vec![comp("Drive", Kind::Other("vfd".into()), None)],
            }],
        };
        let r = panel_sccr(&panel, &defaults());
        assert_eq!(r.panel_ka, None);
        assert_eq!(r.unknown, vec!["Drive".to_owned()]);
        assert_eq!(check_available(&r, 10.0), None);
    }

    #[test]
    fn control_circuits_take_no_part() {
        let mut ctl = comp(
            "T1 control transformer",
            Kind::Other("transformer".into()),
            None,
        );
        ctl.power_circuit = false;
        let panel = Panel {
            feeder: vec![comp("Q0", Kind::Switch, Some(30.0)), ctl],
            branches: vec![],
        };
        assert_eq!(panel_sccr(&panel, &defaults()).panel_ka, Some(30.0));
    }

    #[test]
    fn a_limiter_lets_weaker_components_below_it_work() {
        // Feeder and device 65 kA. Components below are 5 kA by default.
        // Limiter: 10 kA available lets 3 kA through, 65 kA lets 6 kA.
        let limiter = Limiter {
            let_through: vec![(5.0, 2.0), (10.0, 3.0), (65.0, 6.0)],
        };
        let panel = |limiter: Option<Limiter>, below: Option<f64>| Panel {
            feeder: vec![comp("Q0", Kind::Switch, Some(65.0))],
            branches: vec![Branch {
                name: "M1".into(),
                protective_device: comp("F1", Kind::FuseHolder, Some(100.0)),
                limiter,
                components: vec![comp("K1", Kind::MotorController, below)],
            }],
        };
        // Without limiter data: the 5 kA contactor caps the panel.
        assert_eq!(
            panel_sccr(&panel(None, None), &defaults()).panel_ka,
            Some(5.0)
        );
        // With the limiter: at 65 kA 6 kA gets through > 5 kA, so the panel
        // is lowered until the let-through is 5 kA.
        let r = panel_sccr(&panel(Some(limiter.clone()), None), &defaults());
        let ka = r.panel_ka.unwrap();
        assert!(ka > 10.0 && ka < 65.0, "{ka}");
        // At that rating the limiter lets through exactly 5 kA.
        assert!((limiter.limited_ka(ka).unwrap() - 5.0).abs() < 1e-6);
        assert_eq!(r.limiting.unwrap().basis, "limiter");
        // A contactor marked 10 kA is enough for the whole table.
        let r = panel_sccr(&panel(Some(limiter), Some(10.0)), &defaults());
        assert_eq!(r.panel_ka, Some(65.0));
        assert!(r.notes.iter().any(|n| n.contains("lets through")));
    }

    #[test]
    fn a_limiter_without_data_at_the_level_earns_no_credit() {
        let limiter = Limiter {
            let_through: vec![(50.0, 4.0), (100.0, 8.0)],
        };
        let panel = Panel {
            feeder: vec![comp("Q0", Kind::Switch, Some(30.0))],
            branches: vec![Branch {
                name: "M1".into(),
                protective_device: comp("F1", Kind::FuseHolder, Some(100.0)),
                limiter: Some(limiter),
                components: vec![comp("K1", Kind::MotorController, None)],
            }],
        };
        // The feeder is 30 kA, outside the table's 50-100 kA range: no credit.
        let r = panel_sccr(&panel, &defaults());
        assert_eq!(r.panel_ka, Some(5.0));
        assert!(r.notes.iter().any(|n| n.contains("no credit")));
    }

    #[test]
    fn csv_replaces_the_defaults() {
        let t = DefaultTable::from_csv("my SB4.1", "# kind, kA\nswitch,10\nmotor_controller, 5\n")
            .unwrap();
        assert!(t.licensed);
        assert_eq!(t.default_ka(&Kind::Switch), Some(10.0));
        assert_eq!(t.default_ka(&Kind::BusBar), None);
        assert!(DefaultTable::from_csv("x", "switch\n").is_err());
        assert!(DefaultTable::from_csv("x", "switch,-1\n").is_err());
    }

    #[test]
    fn peak_factors_follow_the_table() {
        assert_eq!(peak_factor_n(4.0), 1.5);
        assert_eq!(peak_factor_n(8.0), 1.7);
        assert_eq!(peak_factor_n(15.0), 2.0);
        assert_eq!(peak_factor_n(30.0), 2.1);
        assert_eq!(peak_factor_n(80.0), 2.2);
    }

    #[test]
    fn iec_61439_routes() {
        let a = Assembly61439 {
            icw_ka: Some(25.0),
            tcw_s: 1.0,
            icc_ka: Some(50.0),
            icc_scpd: Some("NSX160".into()),
        };
        // Low prospective current: exempt.
        assert_eq!(check_iec61439(&a, 8.0, 14.0, None, None).ok, Some(true));
        // Conditional rating with the named device.
        let v = check_iec61439(&a, 40.0, 84.0, Some("NSX160"), Some(0.01));
        assert_eq!(v.ok, Some(true));
        assert!(v.text.contains("conditional"));
        assert_eq!(
            check_iec61439(&a, 55.0, 110.0, Some("NSX160"), Some(0.01)).ok,
            Some(false)
        );
        // Other device installed: falls to Icw, 25 kA / peak 52.5 kA: 40 kA fails.
        let v = check_iec61439(&a, 40.0, 84.0, Some("OTHER"), Some(0.01));
        assert_eq!(v.ok, Some(false));
        // Within Icw and peak and time.
        assert_eq!(
            check_iec61439(&a, 22.0, 46.0, None, Some(0.5)).ok,
            Some(true)
        );
        // Peak too high for Icw even though rms is fine.
        assert_eq!(
            check_iec61439(&a, 22.0, 60.0, None, Some(0.5)).ok,
            Some(false)
        );
        // Clearing time unknown: unknown, not a pass.
        assert_eq!(check_iec61439(&a, 22.0, 46.0, None, None).ok, None);
        // No rating at all: unknown.
        let none = Assembly61439 {
            icw_ka: None,
            tcw_s: 1.0,
            icc_ka: None,
            icc_scpd: None,
        };
        assert_eq!(check_iec61439(&none, 30.0, 63.0, None, None).ok, None);
    }
}
