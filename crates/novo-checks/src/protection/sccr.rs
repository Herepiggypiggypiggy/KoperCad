//! Short-circuit current rating (SCCR) of an industrial control panel, in
//! the way UL 508A Supplement SB sets it out.
//!
//! The SCCR of a panel is the highest fault current it may be connected to
//! without a component being destroyed or a person endangered. It is
//! decided by the weakest power-circuit component, where "weakest" takes
//! into account what protects it:
//!
//! 1. A component has its **own** rating: the SCCR marked on it, or, when
//!    it carries none, the default value for its kind ([`default_sccr_ka`]).
//! 2. A **tested combination** raises it: a maker may rate a component for
//!    a higher SCCR when it sits behind a stated type of protective device
//!    up to a stated size (user-supplied data).
//! 3. A **current-limiting protector** (fuse or limiting breaker) upstream
//!    raises it as far as the energy it lets through stays within what the
//!    component withstands (needs the let-through table of the protector
//!    and the withstand of the component; user-supplied data).
//! 4. A **transformer** in the path lowers the fault current seen on its
//!    secondary to what the transformer allows; components behind it then
//!    count against the line-side current that gives them that current.
//!
//! The panel's SCCR is the lowest effective rating over every power-circuit
//! component, protector and conductor-bearing part in the tree. It is then
//! set against the fault current available where the panel is installed
//! (NEC 110.10 and 409.110 require the rating to be marked and to be at
//! least the available current).
//!
//! # Limits
//!
//! The default values are the ones UL 508A gives for components without a
//! marked rating; they are stored here as planning values and must be
//! checked against the edition in force. Control circuits (a control
//! transformer's secondary, signal wiring) are not power circuits and are
//! not part of the tree. The result is guidance for the designer; the
//! panel's marking is the manufacturer's responsibility.

use serde::{Deserialize, Serialize};

use super::devices::{LetThrough, Withstand};

/// What kind of component it is, for the default rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    /// Bus bar or power distribution bar.
    BusBar,
    /// Circuit breaker (inverse time or instantaneous trip).
    CircuitBreaker,
    /// Fuse holder.
    FuseHolder,
    /// Disconnect switch, load switch, snap switch.
    Switch,
    /// Motor controller: contactor, starter, soft starter.
    MotorController,
    /// Overload relay.
    OverloadRelay,
    /// Terminal block (power).
    TerminalBlock,
    /// Power distribution block.
    DistributionBlock,
    /// Receptacle.
    Receptacle,
    /// Current meter or shunt.
    Meter,
    /// Variable frequency drive or other power electronics with its own
    /// marked rating (no default: unmarked counts as the lowest default).
    Drive,
    /// Any other power-circuit part.
    Other,
}

/// The default SCCR in kA for a component without a marked rating (the
/// table of UL 508A Supplement SB for unmarked components). Planning
/// values: check them against the edition in force.
pub fn default_sccr_ka(kind: Kind) -> f64 {
    match kind {
        Kind::BusBar | Kind::FuseHolder | Kind::TerminalBlock | Kind::DistributionBlock => 10.0,
        Kind::Receptacle | Kind::Meter => 10.0,
        Kind::CircuitBreaker
        | Kind::Switch
        | Kind::MotorController
        | Kind::OverloadRelay
        | Kind::Drive
        | Kind::Other => 5.0,
    }
}

/// What kind of protective device it is, for matching combination ratings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Class {
    /// A fuse of a UL class: `"CC"`, `"J"`, `"L"`, `"R"`, `"T"`, `"RK1"`…
    Fuse(String),
    /// A listed current-limiting circuit breaker.
    LimitingBreaker,
    /// A circuit breaker that does not limit (thermal-magnetic, generic).
    Breaker,
    /// A protective device by exact type designation.
    Type(String),
}

/// Which upstream protector a tested combination applies to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Combination {
    /// The protector class or exact type.
    pub upstream: Class,
    /// Largest rating of that protector, amperes.
    pub max_rating_a: f64,
    /// The SCCR the component gets in that combination, kA.
    pub sccr_ka: f64,
}

/// A power-circuit component.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Component {
    /// Tag or type designation.
    pub name: String,
    /// Its kind.
    pub kind: Kind,
    /// SCCR marked on the component, kA.
    pub sccr_ka: Option<f64>,
    /// Tested combinations that raise the rating.
    pub combinations: Vec<Combination>,
    /// What it withstands when a current-limiting protector is upstream.
    pub withstand: Option<Withstand>,
}

impl Component {
    /// A component with an optional marked rating.
    pub fn new(name: impl Into<String>, kind: Kind, sccr_ka: Option<f64>) -> Component {
        Component {
            name: name.into(),
            kind,
            sccr_ka,
            combinations: Vec::new(),
            withstand: None,
        }
    }

    fn own(&self) -> (f64, Source) {
        match self.sccr_ka {
            Some(k) if k.is_finite() && k > 0.0 => (k, Source::Marked),
            _ => (default_sccr_ka(self.kind), Source::Default),
        }
    }
}

/// A protective device in the tree: a component that also protects what
/// lies behind it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Protector {
    /// As a component: its own SCCR, combinations, withstand.
    pub component: Component,
    /// Its class.
    pub class: Class,
    /// Rating in amperes.
    pub rating_a: f64,
    /// Let-through table, for a current-limiting device.
    pub let_through: Option<LetThrough>,
}

/// A three-phase transformer in the power path. The panel sees its
/// primary; what lies behind it sees the secondary.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Transformer {
    /// Rated power in kVA.
    pub kva: f64,
    /// Primary line voltage, volts.
    pub primary_v: f64,
    /// Secondary line voltage, volts.
    pub secondary_v: f64,
    /// Impedance voltage in percent.
    pub z_percent: f64,
}

impl Transformer {
    /// Full-load secondary current, amperes.
    pub fn secondary_fla_a(&self) -> f64 {
        self.kva * 1000.0 / (3f64.sqrt() * self.secondary_v)
    }

    /// Secondary fault current with an infinite primary source, amperes
    /// (the figure UL 508A uses).
    pub fn secondary_fault_a(&self) -> f64 {
        self.secondary_fla_a() / (self.z_percent / 100.0)
    }

    /// The primary-side fault current, amperes, at which the secondary
    /// fault current is `secondary_a`, with the source impedance in series;
    /// `None` when `secondary_a` is at or above what even an infinite
    /// source gives (the transformer then never presents that much).
    pub fn line_current_for(&self, secondary_a: f64) -> Option<f64> {
        let z = self.z_percent / 100.0;
        let z_src = self.secondary_fla_a() / secondary_a - z;
        if !(z_src.is_finite() && z_src > 0.0) {
            return None;
        }
        Some(self.kva * 1000.0 / (3f64.sqrt() * self.primary_v * z_src))
    }

    fn valid(&self) -> bool {
        [self.kva, self.primary_v, self.secondary_v, self.z_percent]
            .iter()
            .all(|x| x.is_finite() && *x > 0.0)
    }
}

/// A part of the power circuit: an optional transformer, an optional
/// protector, components, and branches behind them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Branch {
    /// Name of the branch (`Main`, `M1 feeder`).
    pub name: String,
    /// A transformer at the head of the branch.
    pub transformer: Option<Transformer>,
    /// The protective device that protects the branch.
    pub protector: Option<Protector>,
    /// Components in the branch behind the protector.
    pub components: Vec<Component>,
    /// Sub-branches.
    pub branches: Vec<Branch>,
}

/// A panel's power circuit.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Panel {
    /// Panel name.
    pub name: String,
    /// The root branch: usually the main disconnect and bus.
    pub root: Branch,
    /// Fault current available at the panel's incoming terminals, kA.
    pub available_ka: Option<f64>,
}

/// How a component got its effective rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    /// The marked rating.
    Marked,
    /// The default for its kind (nothing marked).
    Default,
    /// A tested combination with an upstream protector.
    Combination,
    /// Raised by an upstream current-limiting protector's let-through.
    CurrentLimiting,
}

/// One row of the result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// Component name.
    pub name: String,
    /// Where it sits (`Main / M1 feeder`).
    pub path: String,
    /// Its kind.
    pub kind: Kind,
    /// Rating on its own (marked or default), kA, at its own location.
    pub own_ka: f64,
    /// Effective rating in terms of the fault current at the panel's
    /// incoming terminals, kA (infinity: it never limits the panel).
    pub effective_ka: f64,
    /// How the effective rating came about.
    pub source: Source,
}

/// What the result means for the panel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Verdict {
    /// No available fault current given.
    Unchecked,
    /// The panel's SCCR is at least the available current.
    Sufficient,
    /// The panel's SCCR is below the available current.
    Insufficient,
}

/// The SCCR of a panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    /// The panel's SCCR, kA (infinity for a panel with no power components).
    pub panel_sccr_ka: f64,
    /// Every component with its rating, weakest first.
    pub rows: Vec<Row>,
    /// Against the available current.
    pub verdict: Verdict,
    /// Notes: what limits the rating and what would raise it.
    pub notes: Vec<String>,
}

impl Report {
    /// The text for the panel's rating plate.
    pub fn marking(&self) -> String {
        if self.panel_sccr_ka.is_finite() {
            format!(
                "Short circuit current rating: {} kA rms symmetrical",
                trim(self.panel_sccr_ka)
            )
        } else {
            "Short circuit current rating: not limited by any power component".to_string()
        }
    }
}

struct Flat<'a> {
    component: &'a Component,
    path: String,
    /// Protectors from the head of the panel to this component, nearest
    /// last, each with the transformers between them and this component
    /// (nearest first).
    upstream: Vec<&'a Protector>,
    /// Transformers between the panel's line terminals and the component,
    /// nearest to the component first.
    transformers: Vec<Transformer>,
    protector_itself: bool,
}

fn flatten<'a>(
    branch: &'a Branch,
    path: &str,
    upstream: &[&'a Protector],
    transformers: &[Transformer],
    out: &mut Vec<Flat<'a>>,
) {
    let path = if path.is_empty() {
        branch.name.clone()
    } else {
        format!("{path} / {}", branch.name)
    };
    let mut tf: Vec<Transformer> = transformers.to_vec();
    if let Some(t) = branch.transformer.filter(Transformer::valid) {
        // The nearest-to-component transformer comes first.
        tf.insert(0, t);
    }
    let mut ups: Vec<&'a Protector> = upstream.to_vec();
    if let Some(p) = &branch.protector {
        // The protector itself sees the upstream ones, not itself. If the
        // branch has a transformer, the protector sits behind it.
        out.push(Flat {
            component: &p.component,
            path: path.clone(),
            upstream: ups.clone(),
            transformers: tf.clone(),
            protector_itself: true,
        });
        ups.push(p);
    }
    for c in &branch.components {
        out.push(Flat {
            component: c,
            path: path.clone(),
            upstream: ups.clone(),
            transformers: tf.clone(),
            protector_itself: false,
        });
    }
    for b in &branch.branches {
        flatten(b, &path, &ups, &tf, out);
    }
}

fn matches(class: &Class, p: &Protector) -> bool {
    match (class, &p.class) {
        (Class::Fuse(a), Class::Fuse(b)) => a.eq_ignore_ascii_case(b),
        (Class::LimitingBreaker, Class::LimitingBreaker) => true,
        (Class::Breaker, Class::Breaker) | (Class::Breaker, Class::LimitingBreaker) => true,
        (Class::Type(a), _) => a.trim().eq_ignore_ascii_case(p.component.name.trim()),
        _ => false,
    }
}

/// The highest fault current at which the limiter's let-through stays
/// within what the component withstands, A; scans upward from `from_a` up
/// to the limiter's own capacity `cap_a`.
fn let_through_limit(limiter: &Protector, w: Withstand, from_a: f64, cap_a: f64) -> Option<f64> {
    let table = limiter.let_through.as_ref()?;
    let fits = |i: f64| {
        table
            .at(i)
            .is_some_and(|(ip, e)| ip <= w.peak_a && e <= w.i2t)
    };
    if !(from_a.is_finite() && cap_a > from_a && fits(from_a.max(table.points()[0].0))) {
        return None;
    }
    let (mut lo, mut hi) = (from_a.max(table.points()[0].0), cap_a);
    if fits(hi) {
        return Some(hi);
    }
    for _ in 0..60 {
        let mid = (lo * hi).sqrt();
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// Effective rating of a component at its own location, kA, and how.
fn effective_local(
    flat: &Flat<'_>,
    upstream: &[&Protector],
    cache: &mut Vec<(String, f64)>,
) -> (f64, Source) {
    let (own, mut src) = flat.component.own();
    let mut best = own;
    // Try every upstream protector, nearest first: each is an independent
    // proof, the best one counts. A proof is capped by what that protector
    // itself can break.
    for (idx, limiter) in upstream.iter().enumerate().rev() {
        let cap = limiter_rating(limiter, &upstream[..idx], cache);
        for combo in &flat.component.combinations {
            if matches(&combo.upstream, limiter) && limiter.rating_a <= combo.max_rating_a {
                let v = combo.sccr_ka.min(cap);
                if v > best {
                    best = v;
                    src = Source::Combination;
                }
            }
        }
        if let Some(w) = flat.component.withstand
            && let Some(a) = let_through_limit(limiter, w, own * 1000.0, cap * 1000.0)
        {
            let v = a / 1000.0;
            if v > best {
                best = v;
                src = Source::CurrentLimiting;
            }
        }
    }
    (best, src)
}

/// A protector's own effective rating, with the same upgrades, cached by
/// name (names within a panel are expected to be unique; a duplicate name
/// reuses the first value, which is the safe direction only when both are
/// equal, so duplicates are listed in the notes).
fn limiter_rating(p: &Protector, above: &[&Protector], cache: &mut Vec<(String, f64)>) -> f64 {
    if let Some((_, v)) = cache.iter().find(|(n, _)| *n == p.component.name) {
        return *v;
    }
    let flat = Flat {
        component: &p.component,
        path: String::new(),
        upstream: above.to_vec(),
        transformers: Vec::new(),
        protector_itself: true,
    };
    let (v, _) = effective_local(&flat, above, cache);
    cache.push((p.component.name.clone(), v));
    v
}

/// Converts a rating at a component's location to the line-side current at
/// the panel's terminals, kA (infinity when nothing on the line side can
/// make the component see that much).
fn to_line(local_ka: f64, transformers: &[Transformer]) -> f64 {
    let mut a = local_ka * 1000.0;
    for t in transformers {
        match t.line_current_for(a) {
            Some(l) => a = l,
            None => return f64::INFINITY,
        }
    }
    a / 1000.0
}

/// Works out the SCCR of a panel.
pub fn analyse(panel: &Panel) -> Report {
    let mut flats = Vec::new();
    flatten(&panel.root, "", &[], &[], &mut flats);
    let mut cache: Vec<(String, f64)> = Vec::new();
    let mut rows: Vec<Row> = flats
        .iter()
        .map(|f| {
            let (own, _) = f.component.own();
            let (local, src) = effective_local(f, &f.upstream, &mut cache);
            Row {
                name: f.component.name.clone(),
                path: f.path.clone(),
                kind: f.component.kind,
                own_ka: own,
                effective_ka: to_line(local, &f.transformers),
                source: src,
            }
        })
        .collect();
    rows.sort_by(|a, b| a.effective_ka.total_cmp(&b.effective_ka));
    let panel_sccr_ka = rows.first().map_or(f64::INFINITY, |r| r.effective_ka);
    let verdict = match panel.available_ka {
        None => Verdict::Unchecked,
        Some(a) if panel_sccr_ka >= a => Verdict::Sufficient,
        Some(_) => Verdict::Insufficient,
    };
    let mut notes = Vec::new();
    if let Some(w) = rows.first().filter(|r| r.effective_ka.is_finite()) {
        let how = match w.source {
            Source::Default => {
                " (no rating marked, default for its kind used; a marked rating from the maker may be higher)"
            }
            Source::Marked => "",
            Source::Combination => " (as tested behind its upstream protector)",
            Source::CurrentLimiting => " (held up by the current-limiting protector ahead of it)",
        };
        notes.push(format!(
            "the rating is set by {} in {} at {} kA{}",
            w.name,
            w.path,
            trim(w.effective_ka),
            how
        ));
    }
    if verdict == Verdict::Insufficient
        && let Some(a) = panel.available_ka
    {
        let weak: Vec<&str> = rows
            .iter()
            .take_while(|r| r.effective_ka < a)
            .map(|r| r.name.as_str())
            .collect();
        notes.push(format!(
            "the available fault current is {} kA; to reach it replace or protect: {}",
            trim(a),
            weak.join(", ")
        ));
    }
    let mut names: Vec<&str> = flats
        .iter()
        .filter(|f| f.protector_itself)
        .map(|f| f.component.name.as_str())
        .collect();
    names.sort_unstable();
    if names.windows(2).any(|w| w[0] == w[1]) {
        notes.push(
            "two protectors share a name; give them different names so their ratings are not mixed"
                .to_string(),
        );
    }
    Report {
        panel_sccr_ka,
        rows,
        verdict,
        notes,
    }
}

fn trim(x: f64) -> String {
    let t = format!("{x:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &str, kind: Kind, k: Option<f64>) -> Component {
        Component::new(name, kind, k)
    }

    fn fuse_j(name: &str, rating: f64, icu_ka: f64) -> Protector {
        Protector {
            component: comp(name, Kind::FuseHolder, Some(icu_ka)),
            class: Class::Fuse("J".into()),
            rating_a: rating,
            let_through: None,
        }
    }

    fn panel(root: Branch, available: Option<f64>) -> Panel {
        Panel {
            name: "Panel".into(),
            root,
            available_ka: available,
        }
    }

    /// Worked example: the lowest component sets the panel. A disconnect
    /// marked 14 kA, an unmarked terminal block (10 kA default), a contactor
    /// with no marking (5 kA default) and an overload relay (5 kA default):
    /// the panel is 5 kA.
    #[test]
    fn lowest_component_sets_the_rating() {
        let root = Branch {
            name: "Main".into(),
            components: vec![
                comp("-Q1 disconnect", Kind::Switch, Some(14.0)),
                comp("-X1 terminal", Kind::TerminalBlock, None),
                comp("-K1 contactor", Kind::MotorController, None),
                comp("-F2 relay", Kind::OverloadRelay, None),
            ],
            ..Default::default()
        };
        let r = analyse(&panel(root, Some(10.0)));
        assert_eq!(r.panel_sccr_ka, 5.0);
        assert_eq!(r.verdict, Verdict::Insufficient);
        assert_eq!(r.rows[0].source, Source::Default);
        assert!(r.notes[0].contains("default"), "{:?}", r.notes);
        assert!(r.notes[1].contains("replace or protect"), "{:?}", r.notes);
        assert_eq!(
            r.marking(),
            "Short circuit current rating: 5 kA rms symmetrical"
        );
        // Marked values lift it.
        let root = Branch {
            name: "Main".into(),
            components: vec![
                comp("-Q1", Kind::Switch, Some(14.0)),
                comp("-K1", Kind::MotorController, Some(10.0)),
            ],
            ..Default::default()
        };
        let r = analyse(&panel(root, Some(10.0)));
        assert_eq!(r.panel_sccr_ka, 10.0);
        assert_eq!(r.verdict, Verdict::Sufficient);
        assert_eq!(r.rows[0].source, Source::Marked);
    }

    /// Worked example: a Class J fuse (200 kA) ahead of a contactor tested
    /// for 100 kA behind Class J up to 30 A raises the contactor, but not
    /// beyond the fuse's own rating; behind a 40 A fuse the combination
    /// does not apply.
    #[test]
    fn tested_combination_raises_a_component() {
        let mut contactor = comp("-K1", Kind::MotorController, None);
        contactor.combinations.push(Combination {
            upstream: Class::Fuse("J".into()),
            max_rating_a: 30.0,
            sccr_ka: 100.0,
        });
        let make = |rating| Branch {
            name: "M1".into(),
            protector: Some(fuse_j("-F1", rating, 200.0)),
            components: vec![contactor.clone()],
            ..Default::default()
        };
        let r = analyse(&panel(make(30.0), None));
        let k = r.rows.iter().find(|r| r.name == "-K1").unwrap();
        assert_eq!(k.effective_ka, 100.0);
        assert_eq!(k.source, Source::Combination);
        // The fuse holder itself is the weakest at 200 kA: panel 100 kA.
        assert_eq!(r.panel_sccr_ka, 100.0);
        let r = analyse(&panel(make(40.0), None));
        let k = r.rows.iter().find(|r| r.name == "-K1").unwrap();
        assert_eq!(k.effective_ka, 5.0);
        assert_eq!(k.source, Source::Default);
        // The proof is capped by the protector's own rating.
        let mut weak = make(30.0);
        weak.protector.as_mut().unwrap().component.sccr_ka = Some(50.0);
        let r = analyse(&panel(weak, None));
        let k = r.rows.iter().find(|r| r.name == "-K1").unwrap();
        assert_eq!(k.effective_ka, 50.0);
        assert_eq!(r.verdict, Verdict::Unchecked);
    }

    /// A protector that is itself tested only behind a main limiter gets
    /// the upgrade through the chain.
    #[test]
    fn protector_ratings_chain_through_the_tree() {
        let mut breaker = comp("-Q2", Kind::CircuitBreaker, None);
        breaker.combinations.push(Combination {
            upstream: Class::Fuse("J".into()),
            max_rating_a: 100.0,
            sccr_ka: 65.0,
        });
        let sub = Branch {
            name: "Feeder".into(),
            protector: Some(Protector {
                component: breaker,
                class: Class::Breaker,
                rating_a: 30.0,
                let_through: None,
            }),
            components: vec![comp("-X2", Kind::TerminalBlock, Some(65.0))],
            ..Default::default()
        };
        let root = Branch {
            name: "Main".into(),
            protector: Some(fuse_j("-F0", 100.0, 200.0)),
            branches: vec![sub],
            ..Default::default()
        };
        let r = analyse(&panel(root, Some(50.0)));
        assert_eq!(r.panel_sccr_ka, 65.0);
        assert_eq!(r.verdict, Verdict::Sufficient);
    }

    /// Current limiting: the fuse lets through at most 20 kA peak and
    /// 0.4 MA²s up to 100 kA; a component that withstands 25 kA peak and
    /// 0.5 MA²s is held up to the fuse's rating though it is marked 5 kA.
    /// One that withstands only 15 kA peak is held up to the current where
    /// the let-through peak reaches that figure.
    #[test]
    fn current_limiting_protector_raises_by_let_through() {
        let fuse = Protector {
            component: comp("-F1", Kind::FuseHolder, Some(200.0)),
            class: Class::Fuse("J".into()),
            rating_a: 30.0,
            let_through: Some(
                LetThrough::new(vec![
                    (10_000.0, 5_000.0, 5.0e4),
                    (50_000.0, 10_000.0, 2.0e5),
                    (100_000.0, 20_000.0, 4.0e5),
                    (200_000.0, 25_000.0, 6.0e5),
                ])
                .unwrap(),
            ),
        };
        let mut tough = comp("-K1", Kind::MotorController, Some(5.0));
        tough.withstand = Some(Withstand {
            peak_a: 25_000.0,
            i2t: 6.5e5,
        });
        let mut weaker = comp("-K2", Kind::MotorController, Some(5.0));
        weaker.withstand = Some(Withstand {
            peak_a: 15_000.0,
            i2t: 6.5e5,
        });
        let root = Branch {
            name: "M".into(),
            protector: Some(fuse),
            components: vec![tough, weaker],
            ..Default::default()
        };
        let r = analyse(&panel(root, None));
        let k1 = r.rows.iter().find(|r| r.name == "-K1").unwrap();
        assert_eq!(k1.source, Source::CurrentLimiting);
        assert!(
            (k1.effective_ka - 200.0).abs() < 1e-6,
            "{}",
            k1.effective_ka
        );
        let k2 = r.rows.iter().find(|r| r.name == "-K2").unwrap();
        // The peak doubles from 10 kA at 50 kA to 20 kA at 100 kA (slope 1
        // in log-log), so it reaches 15 kA at 75 kA.
        assert!((k2.effective_ka - 75.0).abs() < 0.01, "{}", k2.effective_ka);
        assert_eq!(k2.source, Source::CurrentLimiting);
        assert_eq!(r.panel_sccr_ka, k2.effective_ka);
    }

    /// Worked example: transformers.
    /// 10 kVA, 480 V to 240 V, 5 %: secondary full load 24.06 A, fault 481 A
    /// on an infinite source, far below any default: it never limits.
    /// 500 kVA, 480 V to 208 V, 5 %: full load 1387.9 A, fault 27.76 kA; a
    /// component marked 10 kA behind it is exceeded unless the source
    /// impedance helps: it limits the panel at the line current that gives
    /// 10 kA on the secondary: z_src = 1387.9/10000 - 0.05 = 0.0888 pu, so
    /// 500e3 / (sqrt(3) * 480 * 0.0888) = 6.77 kA.
    #[test]
    fn transformer_changes_what_components_see() {
        let t = Transformer {
            kva: 10.0,
            primary_v: 480.0,
            secondary_v: 240.0,
            z_percent: 5.0,
        };
        assert!((t.secondary_fla_a() - 24.056).abs() < 0.01);
        assert!((t.secondary_fault_a() - 481.1).abs() < 0.1);
        let root = Branch {
            name: "Main".into(),
            components: vec![comp("-Q1", Kind::Switch, Some(65.0))],
            branches: vec![Branch {
                name: "Aux".into(),
                transformer: Some(t),
                components: vec![comp("-K9", Kind::MotorController, None)],
                ..Default::default()
            }],
            ..Default::default()
        };
        let r = analyse(&panel(root, Some(30.0)));
        // 5 kA default is above 481 A: the contactor never limits.
        let k9 = r.rows.iter().find(|r| r.name == "-K9").unwrap();
        assert!(k9.effective_ka.is_infinite());
        assert_eq!(r.panel_sccr_ka, 65.0);

        let big = Transformer {
            kva: 500.0,
            primary_v: 480.0,
            secondary_v: 208.0,
            z_percent: 5.0,
        };
        assert!((big.secondary_fla_a() - 1387.8).abs() < 0.2);
        assert!((big.secondary_fault_a() / 1000.0 - 27.76).abs() < 0.01);
        let line = big.line_current_for(10_000.0).unwrap();
        assert!((line / 1000.0 - 6.77).abs() < 0.02, "{line}");
        assert_eq!(big.line_current_for(30_000.0), None);
        let root = Branch {
            name: "Main".into(),
            components: vec![comp("-Q1", Kind::Switch, Some(65.0))],
            branches: vec![Branch {
                name: "Distribution".into(),
                transformer: Some(big),
                components: vec![comp("-Q5", Kind::CircuitBreaker, Some(10.0))],
                ..Default::default()
            }],
            ..Default::default()
        };
        let r = analyse(&panel(root, Some(30.0)));
        assert!((r.panel_sccr_ka - 6.77).abs() < 0.02, "{}", r.panel_sccr_ka);
        assert_eq!(r.verdict, Verdict::Insufficient);
    }

    #[test]
    fn an_empty_panel_is_not_limited() {
        let r = analyse(&Panel::default());
        assert!(r.panel_sccr_ka.is_infinite());
        assert!(r.marking().contains("not limited"));
        assert_eq!(r.verdict, Verdict::Unchecked);
    }

    #[test]
    fn duplicate_protector_names_are_called_out() {
        let b = |n: &str| Branch {
            name: n.into(),
            protector: Some(fuse_j("-F1", 30.0, 200.0)),
            ..Default::default()
        };
        let root = Branch {
            name: "Main".into(),
            branches: vec![b("A"), b("B")],
            ..Default::default()
        };
        let r = analyse(&panel(root, None));
        assert!(r.notes.iter().any(|n| n.contains("share a name")));
    }

    #[test]
    fn default_table_values() {
        assert_eq!(default_sccr_ka(Kind::BusBar), 10.0);
        assert_eq!(default_sccr_ka(Kind::CircuitBreaker), 5.0);
        assert_eq!(default_sccr_ka(Kind::TerminalBlock), 10.0);
        assert_eq!(default_sccr_ka(Kind::OverloadRelay), 5.0);
    }
}
