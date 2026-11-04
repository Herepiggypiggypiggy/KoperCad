use serde::{Deserialize, Serialize};

use crate::{Anchor, FlagId, Severity};

/// Which engine raised a flag.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Source {
    /// The Issues engine; the text is the check kind key (`check.voltage_drop`).
    Issues(String),
    Layout,
    Routing,
    Heat,
    Sim,
    Emc,
    Safety,
    Calc,
    Parts,
    Job,
    Shell,
}

impl Source {
    /// Stable text for ids.
    pub fn key(&self) -> String {
        match self {
            Source::Issues(k) => format!("issues:{k}"),
            Source::Layout => "layout".into(),
            Source::Routing => "routing".into(),
            Source::Heat => "heat".into(),
            Source::Sim => "sim".into(),
            Source::Emc => "emc".into(),
            Source::Safety => "safety".into(),
            Source::Calc => "calc".into(),
            Source::Parts => "parts".into(),
            Source::Job => "job".into(),
            Source::Shell => "shell".into(),
        }
    }

    /// Short badge for the popover.
    pub fn badge(&self) -> &'static str {
        match self {
            Source::Issues(_) => "Check",
            Source::Layout => "Cabinet",
            Source::Routing => "Routing",
            Source::Heat => "Heat",
            Source::Sim => "Run",
            Source::Emc => "EMC",
            Source::Safety => "Safety",
            Source::Calc => "Calc",
            Source::Parts => "Parts",
            Source::Job => "Job",
            Source::Shell => "App",
        }
    }
}

/// One to three short lines with the numbers, in the model's units.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detail(pub Vec<String>);

impl Detail {
    pub const MAX_LINES: usize = 3;

    /// Keeps at most [`Detail::MAX_LINES`] non-empty lines.
    pub fn new(lines: impl IntoIterator<Item = String>) -> Self {
        Detail(
            lines
                .into_iter()
                .filter(|l| !l.trim().is_empty())
                .take(Self::MAX_LINES)
                .collect(),
        )
    }
}

/// A button in the popover. `Waive` and `Explain` are always offered by the UI
/// when the flag has a waiver key or long text; the engine lists the rest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Locate,
    /// A command id the app runs as one undo step.
    Fix(String),
    /// A panel key to open.
    Open(String),
    Retry,
    Waive,
    Explain,
    Copy,
}

/// How long a flag lives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Life {
    /// Recomputed with the model.
    Live,
    /// Stays until fixed or dismissed (a failed export).
    Until,
    /// Belongs to one run; goes when the run is replaced.
    Run(String),
}

/// What an adapter hands over. The store turns it into a [`Flag`] with a stable id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagDraft {
    pub severity: Severity,
    pub unchecked: bool,
    pub source: Source,
    /// The rule key inside the source (`door.cannot_close`).
    pub rule: String,
    pub anchors: Vec<Anchor>,
    pub title: String,
    pub detail: Detail,
    pub actions: Vec<Action>,
    pub waiver_key: Option<String>,
    pub life: Life,
}

/// The title limit of the design (section 3).
pub const TITLE_MAX: usize = 28;

/// A problem, shown as a triangle on each of its anchors.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flag {
    pub id: FlagId,
    pub severity: Severity,
    /// Could not be judged (missing data): drawn dashed.
    pub unchecked: bool,
    pub source: Source,
    pub anchors: Vec<Anchor>,
    pub title: String,
    pub detail: Detail,
    pub actions: Vec<Action>,
    pub waiver_key: Option<String>,
    pub life: Life,
}

impl Flag {
    /// Builds a flag; the id hashes source, rule and the sorted anchor keys, so
    /// the order the engine lists anchors in does not change it.
    pub fn from_draft(d: FlagDraft) -> Flag {
        let mut keys: Vec<String> = d.anchors.iter().map(Anchor::key).collect();
        keys.sort();
        let source = d.source.key();
        let parts = std::iter::once(source.as_str())
            .chain(std::iter::once(d.rule.as_str()))
            .chain(keys.iter().map(String::as_str));
        let id = FlagId::of(parts);
        Flag {
            id,
            severity: d.severity,
            unchecked: d.unchecked,
            source: d.source,
            anchors: d.anchors,
            title: truncate(&d.title, TITLE_MAX),
            detail: d.detail,
            actions: d.actions,
            waiver_key: d.waiver_key,
            life: d.life,
        }
    }

    /// "Error: Door cannot close" for screen readers and the grey-scale check.
    pub fn accessible_name(&self) -> String {
        let level = match (self.severity, self.unchecked) {
            (Severity::Red, _) => "Error",
            (Severity::Yellow, false) => "Warning",
            (Severity::Yellow, true) => "Not checked",
        };
        format!("{level}: {}", self.title)
    }
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push('…');
    out
}
