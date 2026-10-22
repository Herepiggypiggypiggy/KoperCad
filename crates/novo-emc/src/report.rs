//! What the engine returns.

use crate::class::EmcClass;
use crate::input::P;

/// How much a result can be trusted (`docs/EMC.md` section 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// 0: a rule on what is drawn.
    Fact,
    /// 1: a number from geometry only.
    Index,
    /// 2: a closed-form estimate with stated assumptions.
    Estimate,
    /// 3: a quasi-static field model (later). Level 4, "not computed", is
    /// never a result: it is listed in [`NOT_CHECKED`].
    Field,
}

impl Level {
    /// The word shown next to a result.
    pub fn label(self) -> &'static str {
        match self {
            Level::Fact => "Rule",
            Level::Index => "Index",
            Level::Estimate => "Estimate",
            Level::Field => "Field",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// What a finding or a score is about. Ids are raw model ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Subject {
    Run(u64),
    Device(u64),
    Cable(u64),
    Plate(u64),
}

/// How an engineer reads a score.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Band {
    None,
    Low,
    Medium,
    High,
}

impl Band {
    pub fn of(points: u32) -> Band {
        match points {
            0 => Band::None,
            1..=29 => Band::Low,
            30..=59 => Band::Medium,
            _ => Band::High,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Band::None => "No findings",
            Band::Low => "Low",
            Band::Medium => "Medium",
            Band::High => "High",
        }
    }
}

/// A suggestion. Some can be applied through a command
/// ([`crate::project::apply`]); the others describe what to do by hand.
#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    /// Declare what class a wire is (`emc.class`), when it was guessed.
    DeclareClass { wire: u64, class: EmcClass },
    /// Give the cable a shield core.
    AddShield { cable: u64 },
    /// Make both shield ends all round (a design requirement on the cable).
    Terminate360 { cable: u64 },
    /// Add an EMC filter device for a drive.
    AddFilter { drive: u64 },
    /// Run a wire in another duct. By hand: routing does not take it yet.
    MoveRun {
        wire: u64,
        from_duct: u64,
        to_duct: u64,
        points_after: u32,
    },
    /// Keep two runs this far apart (mm). By hand.
    SeparateRuns { a: u64, b: u64, to_mm: f64 },
    /// Move a device this far (mm) from another. By hand.
    MoveDevice { device: u64, from: u64, to_mm: f64 },
    /// Bond the mounting area to bare metal. By hand.
    BarePlate { plate: u64 },
    /// Move a shield out of a pigtail. By hand.
    ShortenPigtail { cable: u64, to_mm: f64 },
}

impl Fix {
    /// One short sentence saying what the fix is, without names.
    pub fn hint(&self) -> String {
        match self {
            Fix::DeclareClass { class, .. } => {
                format!("Mark the wire as {} if it is.", class.label())
            }
            Fix::AddShield { .. } => "Use a shielded cable.".to_owned(),
            Fix::Terminate360 { .. } => {
                "Bond both shield ends all round at the gland or a shield bar.".to_owned()
            }
            Fix::AddFilter { .. } => "Add a mains filter next to the drive.".to_owned(),
            Fix::MoveRun { points_after, .. } => {
                format!("Run the wire in another duct (score about {points_after} after).")
            }
            Fix::SeparateRuns { to_mm, .. } if *to_mm > 0.0 => format!(
                "Keep the two runs {} mm apart, or in separate ducts.",
                to_mm.round() as i64
            ),
            Fix::SeparateRuns { .. } => {
                "Run go and return together, or in one twisted cable.".to_owned()
            }
            Fix::MoveDevice { to_mm, .. } => {
                format!("Move the device to keep {} mm clear.", to_mm.round() as i64)
            }
            Fix::BarePlate { .. } => {
                "Scrape the mounting area bare or use a galvanised plate.".to_owned()
            }
            Fix::ShortenPigtail { to_mm, .. } => {
                format!("Shorten the pigtail to under {} mm.", to_mm.round() as i64)
            }
        }
    }

    /// Whether the program can do it as one undoable command.
    pub fn applicable(&self) -> bool {
        matches!(
            self,
            Fix::DeclareClass { .. }
                | Fix::AddShield { .. }
                | Fix::Terminate360 { .. }
                | Fix::AddFilter { .. }
        )
    }
}

/// One number a finding shows, with its unit already in the text.
#[derive(Debug, Clone, PartialEq)]
pub struct Metric {
    pub label: &'static str,
    pub value: String,
    /// How the number was made. An estimate also has a band in the finding.
    pub level: Level,
}

/// The frequency band in which an estimate or model may be quoted, Hz.
/// Outside it the engine gives no number.
pub const BAND_HZ: (f64, f64) = (0.0, 30.0e6);

/// A setting a result rests on, shown next to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Assumption {
    pub name: &'static str,
    pub value: String,
    /// Set in the project (true) or the program's default (false).
    pub user_set: bool,
    pub property: &'static str,
}

/// A stretch where two runs lie side by side, for drawing a ribbon.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ribbon {
    pub a: (P, P),
    pub b: (P, P),
    pub length_mm: f64,
    pub distance_mm: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// `EMC-01` and so on.
    pub rule: &'static str,
    pub level: Level,
    pub severity: Severity,
    /// What it is about, most important first.
    pub subjects: Vec<Subject>,
    /// How many points each subject gets for it.
    pub points: Vec<(Subject, u32)>,
    /// The reason, in a sentence or two.
    pub message: String,
    pub metrics: Vec<Metric>,
    pub fixes: Vec<Fix>,
    /// Plate id and position on it, for marking.
    pub at: Option<(u64, P)>,
    /// The stretches of a segregation finding, for ribbons.
    pub ribbons: Vec<Ribbon>,
    /// The polygon of a loop finding, plate coordinates in mm.
    pub loop_polygon: Vec<P>,
    /// The band in which the finding's estimates hold, when it has any.
    pub band_hz: Option<(f64, f64)>,
}

impl Finding {
    /// Where the rule is written down and that it is unverified.
    pub fn source(&self) -> &'static str {
        crate::rules::rule(self.rule).source
    }
}

/// One entry of a subject's score: where the points came from.
#[derive(Debug, Clone, PartialEq)]
pub struct Reason {
    pub rule: &'static str,
    pub points: u32,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Score {
    pub subject: Subject,
    /// 0 to 100: the sum of the reasons' points, capped.
    pub points: u32,
    pub band: Band,
    pub reasons: Vec<Reason>,
}

/// How a rule came out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The rule ran on everything it applies to and found nothing. Not a
    /// statement that the design is fine.
    NoFinding,
    Finding,
    /// Nothing in the project falls under the rule.
    NotApplicable,
    /// The data to judge it is missing.
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuleStatus {
    pub rule: &'static str,
    pub status: Status,
    pub findings: usize,
    /// Why not evaluated, or what was looked at.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Report {
    pub findings: Vec<Finding>,
    pub runs: Vec<Score>,
    pub devices: Vec<Score>,
    pub plates: Vec<Score>,
    /// Per net, the worst run on it: net id and the run's score.
    pub nets: Vec<(u32, Score)>,
    pub rules: Vec<RuleStatus>,
    /// Every setting the numbers rest on, marked default or set in the project.
    pub assumptions: Vec<Assumption>,
}

impl Report {
    pub fn score_of(&self, subject: Subject) -> Option<&Score> {
        let list = match subject {
            Subject::Run(_) => &self.runs,
            Subject::Device(_) => &self.devices,
            Subject::Plate(_) => &self.plates,
            Subject::Cable(_) => return None,
        };
        list.iter().find(|s| s.subject == subject)
    }

    pub fn of_rule(&self, rule: &str) -> impl Iterator<Item = &Finding> {
        self.findings.iter().filter(move |f| f.rule == rule)
    }
}

/// What a design aid like this cannot judge. Shown with every report.
pub const NOT_CHECKED: &[&str] = &[
    "Radiated and conducted emission margins (needs a measurement or a full-wave solver).",
    "Real filter insertion loss and drive-maker limits on motor cable length.",
    "Enclosure shielding: seams, slots, gaskets and cavity resonances.",
    "Surge, burst and ESD paths into the cabinet (EMC-12, EMC-16 are not built yet).",
    "Return current through the plate instead of its own conductor (EMC-10).",
    "Anything above about 30 MHz: cable lengths approach a wavelength fraction there.",
    "Compliance with any standard. Compliance is shown by measurement on the finished machine.",
];
