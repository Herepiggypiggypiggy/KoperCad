//! Numbering schemes: how a wire number is built for a group of wires.
//!
//! A scheme is stored with the project (keys `wirenum.*` in the project
//! properties), so every command, report and script numbers the same way.

use novo_core::{Project, Result, Tx, Value};

/// How numbers are composed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SchemeKind {
    /// One running count for the project: `101`, `102`, ...
    #[default]
    Sequential,
    /// A count per page: `3.1`, `3.2` (page name, separator, count).
    ByPage,
    /// A count per page and grid column: `3.4.1`.
    ByColumn,
    /// By the potential the net carries: `L1.1`, `L2.1`; `PE` and `N` stay
    /// bare when [`Scheme::bare_potentials`] is on. Nets without a
    /// potential fall back to [`SchemeKind::ByPage`].
    ByPotential,
    /// By cable and core: `W1.3`. Wires without a cable core fall back to
    /// [`SchemeKind::ByPage`].
    ByCable,
    /// By function aspect of the page structure: `A2.1`. Pages without one
    /// fall back to [`SchemeKind::ByPage`].
    ByFunction,
}

impl SchemeKind {
    pub const ALL: [SchemeKind; 6] = [
        SchemeKind::Sequential,
        SchemeKind::ByPage,
        SchemeKind::ByColumn,
        SchemeKind::ByPotential,
        SchemeKind::ByCable,
        SchemeKind::ByFunction,
    ];

    /// Short name for menus.
    pub fn label(self) -> &'static str {
        match self {
            SchemeKind::Sequential => "Sequential",
            SchemeKind::ByPage => "By page",
            SchemeKind::ByColumn => "By page and column",
            SchemeKind::ByPotential => "By potential",
            SchemeKind::ByCable => "By cable and core",
            SchemeKind::ByFunction => "By function",
        }
    }

    fn key(self) -> &'static str {
        match self {
            SchemeKind::Sequential => "sequential",
            SchemeKind::ByPage => "page",
            SchemeKind::ByColumn => "column",
            SchemeKind::ByPotential => "potential",
            SchemeKind::ByCable => "cable",
            SchemeKind::ByFunction => "function",
        }
    }

    fn from_key(key: &str) -> Option<SchemeKind> {
        SchemeKind::ALL.into_iter().find(|k| k.key() == key)
    }
}

/// A numbering scheme with its options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scheme {
    pub kind: SchemeKind,
    /// Text before every number (`W` gives `W101`).
    pub prefix: String,
    /// Between the parts of a number (`3.1`).
    pub separator: String,
    /// First count of every series.
    pub start: u32,
    /// Distance between counts (10 leaves room to insert wires later).
    pub step: u32,
    /// Minimum digits of the count, zero padded (`3` gives `003`).
    pub digits: u8,
    /// A new wire may take a number that a deleted wire left free. Off by
    /// default: a number that was printed on a cable stays retired.
    pub reuse_gaps: bool,
    /// PE, N and PEN wires carry the bare potential name on every net
    /// (`PE`, not `PE.1`), and the repeats are not conflicts.
    pub bare_potentials: bool,
}

impl Default for Scheme {
    fn default() -> Self {
        Scheme {
            kind: SchemeKind::default(),
            prefix: String::new(),
            separator: ".".into(),
            start: 1,
            step: 1,
            digits: 1,
            reuse_gaps: false,
            bare_potentials: true,
        }
    }
}

const KIND: &str = "wirenum.scheme";
const PREFIX: &str = "wirenum.prefix";
const SEPARATOR: &str = "wirenum.separator";
const START: &str = "wirenum.start";
const STEP: &str = "wirenum.step";
const DIGITS: &str = "wirenum.digits";
const GAPS: &str = "wirenum.reuse_gaps";
const BARE: &str = "wirenum.bare_potentials";

impl Scheme {
    pub fn new(kind: SchemeKind) -> Self {
        Scheme {
            kind,
            ..Scheme::default()
        }
    }

    /// The project's scheme; the default for a project that never set one.
    pub fn load(project: &Project) -> Scheme {
        let props = &project.info().properties;
        let mut scheme = Scheme::default();
        if let Some(Value::Text(k)) = props.get(KIND)
            && let Some(kind) = SchemeKind::from_key(k)
        {
            scheme.kind = kind;
        }
        if let Some(Value::Text(t)) = props.get(PREFIX) {
            scheme.prefix = t.clone();
        }
        if let Some(Value::Text(t)) = props.get(SEPARATOR) {
            scheme.separator = t.clone();
        }
        let int = |key: &str| match props.get(key) {
            Some(Value::Int(i)) => u32::try_from(*i).ok(),
            _ => None,
        };
        if let Some(v) = int(START) {
            scheme.start = v;
        }
        if let Some(v) = int(STEP).filter(|v| *v > 0) {
            scheme.step = v;
        }
        if let Some(v) = int(DIGITS) {
            scheme.digits = v.min(12) as u8;
        }
        if let Some(Value::Bool(b)) = props.get(GAPS) {
            scheme.reuse_gaps = *b;
        }
        if let Some(Value::Bool(b)) = props.get(BARE) {
            scheme.bare_potentials = *b;
        }
        scheme
    }

    /// Saves the scheme in the project as one undo step's worth of edits.
    pub fn store(&self, tx: &mut Tx<'_>) -> Result<()> {
        let scheme = self.clone();
        tx.set_info(move |info| {
            let p = &mut info.properties;
            p.insert(KIND.into(), Value::Text(scheme.kind.key().into()));
            p.insert(PREFIX.into(), Value::Text(scheme.prefix));
            p.insert(SEPARATOR.into(), Value::Text(scheme.separator));
            p.insert(START.into(), Value::Int(i64::from(scheme.start)));
            p.insert(STEP.into(), Value::Int(i64::from(scheme.step.max(1))));
            p.insert(DIGITS.into(), Value::Int(i64::from(scheme.digits)));
            p.insert(GAPS.into(), Value::Bool(scheme.reuse_gaps));
            p.insert(BARE.into(), Value::Bool(scheme.bare_potentials));
        })
    }

    /// Three sample numbers built from these settings (prefix, separator, start, step, digits),
    /// so the line next to the scheme picker always agrees with the fields.
    pub fn sample(&self) -> String {
        let (pre, sep) = (&self.prefix, &self.separator);
        let c = |i: u32| self.count_text(self.start.saturating_add(self.step.saturating_mul(i)));
        let parts = match self.kind {
            SchemeKind::Sequential => [
                format!("{pre}{}", c(0)),
                format!("{pre}{}", c(1)),
                format!("{pre}{}", c(2)),
            ],
            SchemeKind::ByPage => [
                format!("{pre}3{sep}{}", c(0)),
                format!("{pre}3{sep}{}", c(1)),
                format!("{pre}4{sep}{}", c(0)),
            ],
            SchemeKind::ByColumn => [
                format!("{pre}3{sep}4{sep}{}", c(0)),
                format!("{pre}3{sep}4{sep}{}", c(1)),
                format!("{pre}3{sep}5{sep}{}", c(0)),
            ],
            SchemeKind::ByPotential => [
                format!("{pre}L1{sep}{}", c(0)),
                format!("{pre}L2{sep}{}", c(0)),
                if self.bare_potentials {
                    "PE".to_owned()
                } else {
                    format!("{pre}PE{sep}{}", c(0))
                },
            ],
            SchemeKind::ByCable => [
                format!("{pre}W1{sep}1"),
                format!("{pre}W1{sep}2"),
                format!("{pre}W2{sep}1"),
            ],
            SchemeKind::ByFunction => [
                format!("{pre}A2{sep}{}", c(0)),
                format!("{pre}A2{sep}{}", c(1)),
                format!("{pre}B1{sep}{}", c(0)),
            ],
        };
        parts.join(", ")
    }

    /// The count as text, zero padded.
    pub(crate) fn count_text(&self, n: u32) -> String {
        format!("{n:0width$}", width = usize::from(self.digits.max(1)))
    }
}
