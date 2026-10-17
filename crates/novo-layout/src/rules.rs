//! Rules-based `place all` (Pro Panel gap analysis, R7): where each kind of
//! device goes on the plate, set by a rule set the user can edit.
//!
//! The first-fit planner in [`crate::plan`] fills rails in letter-code order
//! and knows nothing about heat, drives or terminals. This planner reads a
//! [`RuleSet`]: per device kind (the letter code of its tag) a **zone** of
//! the plate (top, middle or bottom), whether the kind gets a **rail of its
//! own** or shares rails with the other devices in its zone, and the **gaps**
//! it wants. Parts can ask for more through their own clearances ([`Gaps`]),
//! and a device with a high power loss can be sent to another zone (`hot`).
//! The result is the same [`Plan`] as `place_all`, so previews, `apply_plan`
//! and the one-undo-step rule are unchanged.
//!
//! Spacing, all in millimetres:
//! - Along a rail, two neighbours are as far apart as the larger of the left
//!   one's right need and the right one's left need. A side's need is the
//!   part's own figure, else the kind's `gap`, else the rule set's `gap`.
//! - Between two rails, the space is the larger of the rule set's `row_gap`
//!   (room for a duct) and the biggest below-need of the upper row and
//!   above-need of the lower row. Needs here default to 0, so `row_gap`
//!   rules unless a part asks for more.
//!
//! Zones: top rows flow from the top of the plate (or from below the devices
//! already placed), bottom rows sit against the bottom edge, and middle rows
//! take what is between. When the plate is too small the bottom rows are
//! kept before the middle rows, so the terminals stay by the cable entry;
//! whatever does not fit comes back in [`Plan::unplaced`], never silently
//! dropped.
//!
//! The rule set is plain text, like the keymap file ([`RuleSet::parse`] and
//! [`RuleSet::to_text`]); where the file lives is the app's business.
//!
//! ```text
//! # kopercad place rules
//! gap 10
//! row_gap 40
//! hot 40 bottom
//! rule Q F G zone=top rail=shared
//! rule A K   zone=middle
//! rule X     zone=bottom gap=5
//! rule *     zone=middle rail=shared
//! ```

use std::fmt;

use crate::plan::{Candidate, Plan, PlannedDuct, Proposed, ROW_GAP_MM};
use crate::{DEFAULT_CLEARANCE_MM, Plate, Rail, Rect, Size};

/// Where on the plate a kind of device goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Zone {
    Top,
    Middle,
    Bottom,
}

impl Zone {
    pub fn as_str(self) -> &'static str {
        match self {
            Zone::Top => "top",
            Zone::Middle => "middle",
            Zone::Bottom => "bottom",
        }
    }

    pub fn parse(text: &str) -> Option<Zone> {
        match text.trim().to_ascii_lowercase().as_str() {
            "top" | "high" => Some(Zone::Top),
            "middle" | "mid" => Some(Zone::Middle),
            "bottom" | "low" => Some(Zone::Bottom),
            _ => None,
        }
    }
}

/// Whether a kind gets rails of its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RailUse {
    /// Its devices never share a rail with another kind.
    Own,
    /// Its devices share rails with the other shared kinds of the zone, in
    /// the order the rules list them.
    Shared,
}

impl RailUse {
    pub fn as_str(self) -> &'static str {
        match self {
            RailUse::Own => "own",
            RailUse::Shared => "shared",
        }
    }
}

/// What a kind of device carries, which decides which wiring duct serves its
/// row. Power and signal ducts are kept apart (see [`RuleSet::separation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, PartialOrd, Ord)]
pub enum Class {
    /// Mains, motor and other load current.
    Power,
    /// Control, sensor and bus wiring.
    Signal,
    /// Carries both (terminals) or is not said; fits next to either.
    #[default]
    Mixed,
}

impl Class {
    pub fn as_str(self) -> &'static str {
        match self {
            Class::Power => "power",
            Class::Signal => "signal",
            Class::Mixed => "mixed",
        }
    }

    pub fn parse(text: &str) -> Option<Class> {
        match text.trim().to_ascii_lowercase().as_str() {
            "power" => Some(Class::Power),
            "signal" | "control" => Some(Class::Signal),
            "mixed" | "any" => Some(Class::Mixed),
            _ => None,
        }
    }

    /// True when the two must not share a duct or lie close together.
    pub fn conflicts(self, other: Class) -> bool {
        matches!(
            (self, other),
            (Class::Power, Class::Signal) | (Class::Signal, Class::Power)
        )
    }
}

/// What the rule set says about some kinds of device.
#[derive(Debug, Clone, PartialEq)]
pub struct KindRule {
    /// Letter codes this rule is for (`Q`, `K`, `QA`); `*` is the fallback.
    /// A device matches the longest code its own letter code starts with.
    pub codes: Vec<String>,
    pub zone: Zone,
    pub rail: RailUse,
    /// Gap to the neighbours along the rail, if not the rule set's default.
    pub gap: Option<f64>,
    /// Room wanted above and below the row, if more than `row_gap`.
    pub above: Option<f64>,
    pub below: Option<f64>,
    /// What the kind carries; decides the duct next to its row.
    pub class: Class,
}

impl KindRule {
    fn new(codes: &[&str], zone: Zone, rail: RailUse) -> KindRule {
        KindRule {
            codes: codes.iter().map(|c| (*c).to_owned()).collect(),
            zone,
            rail,
            gap: None,
            above: None,
            below: None,
            class: Class::Mixed,
        }
    }

    fn with(mut self, class: Class, gap: Option<f64>, around: Option<f64>) -> KindRule {
        self.class = class;
        self.gap = gap;
        self.above = around;
        self.below = around;
        self
    }
}

/// Room kept between a duct and the rail rows beside it, for wires to fan out.
pub const DUCT_CLEAR_MM: f64 = 10.0;
/// Standard duct cross-section (width x height) in millimetres.
pub const DUCT_WIDTH_MM: f64 = 25.0;
pub const DUCT_HEIGHT_MM: f64 = 60.0;
/// Standard distance kept between a power duct and a signal duct.
pub const SEPARATION_MM: f64 = 40.0;
/// Standard extra gap on each side of a hot device.
pub const HOT_GAP_MM: f64 = 20.0;

/// Devices at or above `min_loss_w` go to `zone`, whatever their kind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HotRule {
    pub min_loss_w: f64,
    pub zone: Zone,
}

/// The editable rule set.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleSet {
    /// Default gap between neighbours along a rail.
    pub gap: f64,
    /// Smallest space between two rails (room for a wiring duct).
    pub row_gap: f64,
    pub hot: Option<HotRule>,
    /// Extra gap on each side of a hot device, so it does not heat its
    /// neighbours along the rail.
    pub hot_gap: f64,
    /// Distance kept between a power duct and a signal duct; 0 turns the
    /// separation off.
    pub separation: f64,
    /// Cross-section of the ducts the planner draws.
    pub duct_width: f64,
    pub duct_height: f64,
    pub rules: Vec<KindRule>,
}

impl Default for RuleSet {
    /// The standard rule set, a common panel layout: protection and supplies
    /// at the top, controllers and relays in the middle, heavy and hot parts
    /// and the terminals at the bottom. A starting point, edited per user.
    fn default() -> Self {
        RuleSet::standard()
    }
}

impl RuleSet {
    pub fn standard() -> RuleSet {
        RuleSet {
            gap: DEFAULT_CLEARANCE_MM,
            row_gap: ROW_GAP_MM,
            hot: Some(HotRule {
                min_loss_w: 25.0,
                zone: Zone::Middle,
            }),
            hot_gap: HOT_GAP_MM,
            separation: SEPARATION_MM,
            duct_width: DUCT_WIDTH_MM,
            duct_height: DUCT_HEIGHT_MM,
            rules: vec![
                KindRule::new(&["Q", "F", "G"], Zone::Top, RailUse::Shared).with(
                    Class::Power,
                    None,
                    None,
                ),
                KindRule::new(&["A", "K"], Zone::Middle, RailUse::Own).with(
                    Class::Signal,
                    None,
                    None,
                ),
                // Drives and transformers: room to breathe on every side.
                KindRule::new(&["T"], Zone::Bottom, RailUse::Own).with(
                    Class::Power,
                    Some(20.0),
                    Some(80.0),
                ),
                KindRule::new(&["U"], Zone::Bottom, RailUse::Own).with(Class::Signal, None, None),
                KindRule::new(&["X"], Zone::Bottom, RailUse::Own),
                KindRule::new(&["*"], Zone::Middle, RailUse::Shared),
            ],
        }
    }

    /// The rule for a device kind: the longest listed code that the kind's
    /// letter code starts with, else the `*` rule, else a middle-zone shared
    /// default. Returns the rule with its index and the code's position in it.
    fn rule_for(&self, kind: &str) -> (usize, usize, KindRule) {
        let kind = kind.to_ascii_uppercase();
        let mut best: Option<(usize, usize, usize)> = None;
        let mut fallback = None;
        for (i, rule) in self.rules.iter().enumerate() {
            for (j, code) in rule.codes.iter().enumerate() {
                if code == "*" {
                    fallback.get_or_insert((i, j));
                } else if !code.is_empty()
                    && kind.starts_with(code.as_str())
                    && best.is_none_or(|(_, _, len)| code.len() > len)
                {
                    best = Some((i, j, code.len()));
                }
            }
        }
        if let Some((i, j, _)) = best {
            return (i, j, self.rules[i].clone());
        }
        if let Some((i, j)) = fallback {
            return (i, j, self.rules[i].clone());
        }
        (
            usize::MAX,
            0,
            KindRule::new(&["*"], Zone::Middle, RailUse::Shared),
        )
    }

    /// The rule for a device kind and its index in [`RuleSet::rules`];
    /// `None` when no rule matched and the built-in default applies.
    pub fn rule_for_kind(&self, kind: &str) -> (Option<usize>, KindRule) {
        let (index, _, rule) = self.rule_for(kind);
        ((index != usize::MAX).then_some(index), rule)
    }

    /// Reads a rule set from text. Blank lines and `#` comments are ignored.
    /// A line that is not understood is an error naming its number.
    pub fn parse(text: &str) -> Result<RuleSet, ParseError> {
        let mut set = RuleSet {
            gap: DEFAULT_CLEARANCE_MM,
            row_gap: ROW_GAP_MM,
            hot: None,
            hot_gap: 0.0,
            separation: 0.0,
            duct_width: DUCT_WIDTH_MM,
            duct_height: DUCT_HEIGHT_MM,
            rules: Vec::new(),
        };
        for (n, raw) in text.lines().enumerate() {
            let line_no = n + 1;
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let err = |message: String| ParseError {
                line: line_no,
                message,
            };
            let mut words = line.split_whitespace();
            let keyword = words.next().unwrap_or("").to_ascii_lowercase();
            let rest: Vec<&str> = words.collect();
            match keyword.as_str() {
                "gap" | "row_gap" | "hot_gap" | "separate" => {
                    let [value] = rest.as_slice() else {
                        return Err(err(format!("`{keyword}` takes one number of millimetres")));
                    };
                    let v = mm(value).ok_or_else(|| err(format!("`{value}` is not a distance")))?;
                    match keyword.as_str() {
                        "gap" => set.gap = v,
                        "row_gap" => set.row_gap = v,
                        "hot_gap" => set.hot_gap = v,
                        _ => set.separation = v,
                    }
                }
                "duct" => {
                    let [w, h] = rest.as_slice() else {
                        return Err(err("`duct` takes width and height: `duct 25 40`".into()));
                    };
                    let width = mm(w).ok_or_else(|| err(format!("`{w}` is not a distance")))?;
                    let height = mm(h).ok_or_else(|| err(format!("`{h}` is not a distance")))?;
                    if width <= 0.0 || height <= 0.0 {
                        return Err(err("a duct needs a width and height above 0".into()));
                    }
                    set.duct_width = width;
                    set.duct_height = height;
                }
                "hot" => {
                    let [watts, zone] = rest.as_slice() else {
                        return Err(err("`hot` takes watts and a zone: `hot 40 bottom`".into()));
                    };
                    let min_loss_w =
                        mm(watts).ok_or_else(|| err(format!("`{watts}` is not a power")))?;
                    let zone = Zone::parse(zone).ok_or_else(|| {
                        err(format!("`{zone}` is not a zone (top, middle, bottom)"))
                    })?;
                    set.hot = Some(HotRule { min_loss_w, zone });
                }
                "rule" => set.rules.push(parse_rule(&rest).map_err(err)?),
                other => return Err(err(format!("unknown keyword `{other}`"))),
            }
        }
        Ok(set)
    }

    /// One rule as it is written in the file, without the leading `rule`.
    pub fn rule_text(&self, index: usize) -> String {
        let Some(r) = self.rules.get(index) else {
            return String::new();
        };
        let mut out = format!(
            "{} zone={} rail={}",
            r.codes.join(" "),
            r.zone.as_str(),
            r.rail.as_str()
        );
        if r.class != Class::Mixed {
            out.push_str(&format!(" class={}", r.class.as_str()));
        }
        for (key, v) in [("gap", r.gap), ("above", r.above), ("below", r.below)] {
            if let Some(v) = v {
                out.push_str(&format!(" {key}={v}"));
            }
        }
        out
    }

    /// The rule set as text that [`RuleSet::parse`] reads back.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# KoperCAD place rules: where Place all puts each kind\n");
        out.push_str(&format!("gap {}\nrow_gap {}\n", self.gap, self.row_gap));
        out.push_str(&format!(
            "separate {}\nduct {} {}\n",
            self.separation, self.duct_width, self.duct_height
        ));
        if let Some(h) = self.hot {
            out.push_str(&format!("hot {} {}\n", h.min_loss_w, h.zone.as_str()));
        }
        out.push_str(&format!("hot_gap {}\n", self.hot_gap));
        for i in 0..self.rules.len() {
            out.push_str("rule ");
            out.push_str(&self.rule_text(i));
            out.push('\n');
        }
        out
    }
}

fn mm(text: &str) -> Option<f64> {
    text.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn parse_rule(words: &[&str]) -> Result<KindRule, String> {
    let mut rule = KindRule::new(&[], Zone::Middle, RailUse::Own);
    for word in words {
        match word.split_once('=') {
            None => rule.codes.push(word.to_ascii_uppercase()),
            Some((key, value)) => match key.to_ascii_lowercase().as_str() {
                "zone" => {
                    rule.zone = Zone::parse(value)
                        .ok_or_else(|| format!("`{value}` is not a zone (top, middle, bottom)"))?;
                }
                "rail" => {
                    rule.rail = match value.to_ascii_lowercase().as_str() {
                        "own" => RailUse::Own,
                        "shared" => RailUse::Shared,
                        _ => return Err(format!("`{value}` is not a rail use (own, shared)")),
                    };
                }
                "class" => {
                    rule.class = Class::parse(value).ok_or_else(|| {
                        format!("`{value}` is not a class (power, signal, mixed)")
                    })?;
                }
                "gap" | "above" | "below" => {
                    let v = mm(value).ok_or_else(|| format!("`{value}` is not a distance"))?;
                    match key.to_ascii_lowercase().as_str() {
                        "gap" => rule.gap = Some(v),
                        "above" => rule.above = Some(v),
                        _ => rule.below = Some(v),
                    }
                }
                other => return Err(format!("unknown setting `{other}`")),
            },
        }
    }
    if rule.codes.is_empty() {
        return Err("a rule needs at least one letter code, or `*`".into());
    }
    Ok(rule)
}

/// A line of a rule file that could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    /// 1-based line number.
    pub line: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for ParseError {}

/// Free space a part asks for on each side of an upright device (the keys
/// `clearance_*_mm` of the part data); `None` is no request.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Gaps {
    pub above: Option<f64>,
    pub below: Option<f64>,
    pub left: Option<f64>,
    pub right: Option<f64>,
}

/// A device to place with what the rules need to know about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Spec {
    pub id: u64,
    pub size: Size,
    /// Order within a rail: letter code, then device number.
    pub sort_key: String,
    /// The letter code of the device's tag (`K`, `QA`); may be empty.
    pub kind: String,
    pub gaps: Gaps,
    /// Effective power loss in W, when known.
    pub loss_w: Option<f64>,
}

impl Spec {
    /// A spec with no part data: the kind is the letters at the start of the
    /// candidate's sort key.
    pub fn from_candidate(c: &Candidate) -> Spec {
        Spec {
            id: c.id,
            size: c.size,
            sort_key: c.sort_key.clone(),
            kind: kind_of(&c.sort_key),
            gaps: Gaps::default(),
            loss_w: None,
        }
    }
}

/// The letter code at the start of a sort key (`K000012` gives `K`).
pub fn kind_of(sort_key: &str) -> String {
    sort_key
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect::<String>()
}

/// One row of devices on its own rail.
struct Row {
    items: Vec<(usize, f64)>,
    height: f64,
    above: f64,
    below: f64,
    /// Which of the part clearances set `above` and `below`.
    above_from: Option<u64>,
    below_from: Option<u64>,
    class: Class,
}

struct Group {
    zone: Zone,
    order: (usize, usize),
    name: String,
    class: Class,
    members: Vec<usize>,
}

/// Where a gap along a rail comes from, for the explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GapSource {
    /// First device of its row: nothing to its left.
    Start,
    /// The rule set's default `gap`.
    RuleSet,
    /// The `gap` of the kind's rule.
    Kind,
    /// A clearance the part asks for.
    Part,
    /// The extra room around a hot device (`hot_gap`).
    Hot,
}

/// Why the room between two rails is what it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowGapCause {
    /// The rule set's `row_gap`.
    RowGap,
    /// A part (the device id) asks for more room.
    Part(u64),
    /// A power row beside a signal row: two ducts, kept `separate` apart.
    Separation,
}

/// Why one device is where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub id: u64,
    /// Index in [`RuleSet::rules`] of the rule that applied; `None` when no
    /// rule matched and the built-in middle-zone default was used.
    pub rule: Option<usize>,
    pub zone: Zone,
    /// The zone came from the `hot` rule, not from the kind's rule.
    pub hot: bool,
    pub class: Class,
    /// The rail (plan id) the device is on; `None` when it was not placed.
    pub rail: Option<u64>,
    /// Gap to the left neighbour along the rail, and what set it.
    pub gap_before: f64,
    pub gap_source: GapSource,
}

/// Why a device was left off the plate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnplacedWhy {
    /// Wider or higher than the usable plate.
    TooLarge,
    /// Its row does not fit on the plate any more.
    NoRoom,
}

impl UnplacedWhy {
    /// The reason, for a message after a device's name.
    pub fn text(self) -> &'static str {
        match self {
            UnplacedWhy::TooLarge => "larger than the usable plate",
            UnplacedWhy::NoRoom => "no room left on the plate",
        }
    }
}

/// Why a rail is where it is, relative to the rail above it.
#[derive(Debug, Clone, PartialEq)]
pub struct RowNote {
    pub rail: u64,
    pub zone: Zone,
    pub class: Class,
    /// Space between this row and the one above, `None` for the first row.
    pub space_above: Option<f64>,
    pub cause: Option<RowGapCause>,
}

/// Everything the planner decided and why: every placement names the rule
/// that put it there.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Explanation {
    pub decisions: Vec<Decision>,
    pub rows: Vec<RowNote>,
    pub unplaced: Vec<(u64, UnplacedWhy)>,
    /// The plate was too small to keep power and signal rows apart, so the
    /// separation was given up to fit every device.
    pub separation_relaxed: bool,
    /// The plan has a trunk duct joining the ducts between the rows.
    pub trunk: bool,
    /// A trunk was left out because the narrower plate would not hold every
    /// device; the rows' ducts are then not joined.
    pub trunk_dropped: bool,
}

impl Explanation {
    pub fn decision(&self, id: u64) -> Option<&Decision> {
        self.decisions.iter().find(|d| d.id == id)
    }

    /// One sentence for a device, for a tooltip or the inspector.
    pub fn describe(&self, id: u64, rules: &RuleSet) -> String {
        if let Some((_, why)) = self.unplaced.iter().find(|(u, _)| *u == id) {
            return match why {
                UnplacedWhy::TooLarge => "Not placed: it is larger than the usable plate.".into(),
                UnplacedWhy::NoRoom => "Not placed: no room left on the plate.".into(),
            };
        }
        let Some(d) = self.decision(id) else {
            return "Not part of this plan.".into();
        };
        let mut out = match d.rule {
            Some(i) => format!("Rule `{}`", rules.rule_text(i)),
            None => "No rule matched, so the middle zone".to_owned(),
        };
        out.push_str(&format!(" puts it in the {} zone", d.zone.as_str()));
        if d.hot {
            out.push_str(" (it is hot: the hot rule moved it)");
        }
        if d.class != Class::Mixed {
            out.push_str(&format!(", on the {} side", d.class.as_str()));
        }
        match d.gap_source {
            GapSource::Start => {}
            GapSource::RuleSet => {
                out.push_str(&format!("; {} mm from its neighbour (gap)", d.gap_before))
            }
            GapSource::Kind => out.push_str(&format!(
                "; {} mm from its neighbour (the rule's gap)",
                d.gap_before
            )),
            GapSource::Part => out.push_str(&format!(
                "; {} mm from its neighbour (its part asks for it)",
                d.gap_before
            )),
            GapSource::Hot => out.push_str(&format!(
                "; {} mm from its neighbour (hot device)",
                d.gap_before
            )),
        }
        out.push('.');
        out
    }
}

/// Fills rails by the rules; see the module documentation. Rail ids count up
/// from `first_rail_id` in plate order, top to bottom. The first rail starts
/// at `top` or lower, so a plan can sit below devices already placed.
pub fn place_all_rules(
    plate: &Plate,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
) -> Plan {
    place_all_explained(plate, specs, rules, first_rail_id, top).0
}

/// [`place_all_rules`] with the reason for every decision. When the plate is
/// too small to keep power and signal rows apart, the separation is given up
/// before a device is left off, and [`Explanation::separation_relaxed`] says so.
pub fn place_all_explained(
    plate: &Plate,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
) -> (Plan, Explanation) {
    place_all_around(plate, specs, rules, first_rail_id, top, &[])
}

/// [`place_all_explained`] on a plate that already has ducts: nothing is
/// planned over a `blocked` area (a duct's footprint), nor within
/// [`DUCT_CLEAR_MM`] of it. The usable plate is cut around the blocked areas
/// into free regions, biggest first; each region is filled by the rules and
/// what it could not take goes to the next. The zone order therefore holds
/// inside a region, not across a duct. With nothing blocked the result is
/// that of [`place_all_explained`].
pub fn place_all_around(
    plate: &Plate,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
    blocked: &[Rect],
) -> (Plan, Explanation) {
    let regions = free_regions(plate.usable(), blocked);
    let mut plan = Plan::default();
    let mut why = Explanation::default();
    let mut left: Vec<Spec> = specs.to_vec();
    for (n, region) in regions.iter().enumerate() {
        if left.is_empty() && n > 0 {
            break;
        }
        let first = first_rail_id + plan.rails.len() as u64;
        let (p, w) = plan_region(*region, &left, rules, first, top);
        let unplaced = p.unplaced.clone();
        plan.rails.extend(p.rails);
        plan.ducts.extend(p.ducts);
        plan.placements.extend(p.placements);
        why.decisions.extend(w.decisions);
        why.rows.extend(w.rows);
        why.separation_relaxed |= w.separation_relaxed;
        why.trunk |= w.trunk;
        why.trunk_dropped |= w.trunk_dropped;
        // Only what no region takes is unplaced (and why, from the last try).
        left.retain(|s| unplaced.contains(&s.id));
        why.unplaced = w.unplaced;
    }
    plan.unplaced = left.iter().map(|s| s.id).collect();
    if regions.is_empty() {
        why.unplaced = left.iter().map(|s| (s.id, UnplacedWhy::NoRoom)).collect();
    }
    why.unplaced.retain(|(id, _)| plan.unplaced.contains(id));
    (plan, why)
}

/// The usable area cut around `blocked` (each grown by [`DUCT_CLEAR_MM`]),
/// biggest region first, ties top-left first. Without blockers it is the
/// area itself. Slivers under 20 mm either way are dropped.
fn free_regions(area: Rect, blocked: &[Rect]) -> Vec<Rect> {
    const MIN_MM: f64 = 20.0;
    let mut regions = vec![area];
    for b in blocked {
        let b = Rect {
            x: b.x - DUCT_CLEAR_MM,
            y: b.y - DUCT_CLEAR_MM,
            w: b.w + 2.0 * DUCT_CLEAR_MM,
            h: b.h + 2.0 * DUCT_CLEAR_MM,
        };
        let mut next = Vec::new();
        for r in regions {
            if !r.overlaps(&b) {
                next.push(r);
                continue;
            }
            // Left and right take the full height; above and below only the
            // blocker's columns.
            let (cx0, cx1) = (b.x.max(r.x), b.right().min(r.right()));
            let parts = [
                Rect {
                    x: r.x,
                    y: r.y,
                    w: b.x - r.x,
                    h: r.h,
                },
                Rect {
                    x: b.right(),
                    y: r.y,
                    w: r.right() - b.right(),
                    h: r.h,
                },
                Rect {
                    x: cx0,
                    y: r.y,
                    w: cx1 - cx0,
                    h: b.y - r.y,
                },
                Rect {
                    x: cx0,
                    y: b.bottom(),
                    w: cx1 - cx0,
                    h: r.bottom() - b.bottom(),
                },
            ];
            next.extend(parts.into_iter().filter(|p| p.w >= MIN_MM && p.h >= MIN_MM));
        }
        regions = next;
    }
    regions.sort_by(|a, b| {
        b.area()
            .total_cmp(&a.area())
            .then(a.y.total_cmp(&b.y))
            .then(a.x.total_cmp(&b.x))
    });
    regions
}

/// One region: the rules' plan, with the separation given up before a device
/// is left off (see [`place_all_explained`]).
fn plan_region(
    area: Rect,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
) -> (Plan, Explanation) {
    let (plan, why) = plan_with_trunk(area, specs, rules, first_rail_id, top);
    if plan.unplaced.is_empty() || rules.separation <= 0.0 {
        return (plan, why);
    }
    let relaxed = RuleSet {
        separation: 0.0,
        ..rules.clone()
    };
    let (plan2, mut why2) = plan_with_trunk(area, specs, &relaxed, first_rail_id, top);
    if plan2.unplaced.len() < plan.unplaced.len() {
        why2.separation_relaxed = true;
        (plan2, why2)
    } else {
        (plan, why)
    }
}

/// Plans the rows, and when there are two or more adds a trunk duct that
/// joins their ducts. Without it the horizontal ducts between rows never
/// meet, and no wire can leave its own row's duct for another's. The trunk
/// is a vertical duct at the left edge of the usable plate, so a strip of
/// the plate is kept free for it and the rows are packed again narrower. A
/// router that bridged the runs instead would route wires across a gap no
/// duct covers, which the cabinet cannot build; the trunk is a part on the
/// cut list. Its cross-section is the rule set's duct size: the planner
/// knows devices, not wires, so the fill is checked by Route all.
///
/// The trunk is class `mixed` (a divided duct): ducts of both classes meet
/// in it, as they do at the terminals, and it does not trip the separation
/// check. If the narrower plate loses a device the plan without a trunk is
/// kept and [`Explanation::trunk_dropped`] says so.
fn plan_with_trunk(
    area: Rect,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
) -> (Plan, Explanation) {
    let (plan, why) = plan_once(area, specs, rules, first_rail_id, top, false);
    if plan.rails.len() < 2 {
        return (plan, why);
    }
    let (with, mut why_with) = plan_once(area, specs, rules, first_rail_id, top, true);
    if with.unplaced.len() <= plan.unplaced.len() {
        (with, why_with)
    } else {
        let mut why = why;
        why_with.trunk_dropped = true;
        why.trunk_dropped = true;
        (plan, why)
    }
}

fn plan_once(
    area: Rect,
    specs: &[Spec],
    rules: &RuleSet,
    first_rail_id: u64,
    top: f64,
    trunk: bool,
) -> (Plan, Explanation) {
    let full = area;
    let mut usable = full;
    // The strip kept free along the left edge for the trunk duct.
    let strip = rules.duct_width + 2.0 * DUCT_CLEAR_MM;
    if trunk {
        usable.x += strip;
        usable.w -= strip;
    }
    let mut plan = Plan::default();
    let mut why = Explanation::default();

    // Which rule, and so which zone and group, each device falls under.
    let mut groups: Vec<Group> = Vec::new();
    let mut resolved: Vec<(usize, KindRule)> = Vec::with_capacity(specs.len());
    let mut hot_flags: Vec<bool> = Vec::with_capacity(specs.len());
    let mut zones: Vec<Zone> = Vec::with_capacity(specs.len());
    let mut rule_ids: Vec<Option<usize>> = Vec::with_capacity(specs.len());
    for (i, spec) in specs.iter().enumerate() {
        let (rule_index, code_index, rule) = rules.rule_for(&spec.kind);
        let hot = matches!(
            rules.hot,
            Some(h) if spec.loss_w.is_some_and(|w| w >= h.min_loss_w)
        );
        let zone = match rules.hot {
            Some(h) if hot => h.zone,
            _ => rule.zone,
        };
        // A shared rail never mixes power and signal wiring.
        let name = match rule.rail {
            RailUse::Own => spec.kind.to_ascii_uppercase(),
            RailUse::Shared => format!("~{}", rule.class.as_str()),
        };
        let order = (rule_index, code_index);
        match groups.iter_mut().find(|g| g.zone == zone && g.name == name) {
            Some(g) => {
                g.order = g.order.min(order);
                g.members.push(i);
            }
            None => groups.push(Group {
                zone,
                order,
                name,
                class: rule.class,
                members: vec![i],
            }),
        }
        hot_flags.push(hot);
        zones.push(zone);
        rule_ids.push((rule_index != usize::MAX).then_some(rule_index));
        resolved.push((rule_index, rule));
    }
    groups.sort_by(|a, b| (a.zone, a.order, &a.name).cmp(&(b.zone, b.order, &b.name)));

    // Rows of each group, left to right inside the usable width.
    let need =
        |i: usize, side: fn(&Gaps) -> Option<f64>, rule_side: Option<f64>| -> (f64, GapSource) {
            let base = if let Some(v) = side(&specs[i].gaps) {
                (v, GapSource::Part)
            } else if let Some(v) = rule_side {
                (v, GapSource::Kind)
            } else {
                (rules.gap, GapSource::RuleSet)
            };
            if hot_flags[i] && rules.hot_gap > base.0 {
                (rules.hot_gap, GapSource::Hot)
            } else {
                (base.0.max(0.0), base.1)
            }
        };
    let mut rows: Vec<(Zone, Row)> = Vec::new();
    // Gap before each device and what set it (device index to gap).
    let mut gap_before_of: Vec<(f64, GapSource)> = vec![(0.0, GapSource::Start); specs.len()];
    for group in &mut groups {
        group.members.sort_by(|&a, &b| {
            let key = |i: usize| (resolved[i].0, &specs[i].sort_key, specs[i].id);
            let (ra, rb) = (key(a), key(b));
            // Within a shared group the rules' order comes first, so Q sits
            // left of F when the rule lists Q first.
            let pos = |i: usize| {
                resolved[i]
                    .1
                    .codes
                    .iter()
                    .position(|c| {
                        c == "*" || specs[i].kind.to_ascii_uppercase().starts_with(c.as_str())
                    })
                    .unwrap_or(0)
            };
            (ra.0, pos(a), ra.1, ra.2).cmp(&(rb.0, pos(b), rb.1, rb.2))
        });
        let mut current: Option<Row> = None;
        let mut x = 0.0;
        for &i in &group.members {
            let (w, h) = (specs[i].size.width, specs[i].size.height);
            if w > usable.w || h > usable.h {
                plan.unplaced.push(specs[i].id);
                why.unplaced.push((specs[i].id, UnplacedWhy::TooLarge));
                continue;
            }
            let rule = &resolved[i].1;
            let (left, left_src) = need(i, |g| g.left, rule.gap);
            let mut gap_before = (0.0, GapSource::Start);
            if let Some(row) = &current {
                let (prev, _) = row.items.last().copied().unwrap_or((i, 0.0));
                let prev_rule = &resolved[prev].1;
                let (right, right_src) = need(prev, |g| g.right, prev_rule.gap);
                gap_before = if right >= left {
                    (right, right_src)
                } else {
                    (left, left_src)
                };
                if x + gap_before.0 + w > usable.w {
                    rows.push((group.zone, current.take().expect("row is open")));
                }
            }
            let row = current.get_or_insert_with(|| {
                x = 0.0;
                gap_before = (0.0, GapSource::Start);
                Row {
                    items: Vec::new(),
                    height: 0.0,
                    above: 0.0,
                    below: 0.0,
                    above_from: None,
                    below_from: None,
                    class: group.class,
                }
            });
            let at = x + gap_before.0;
            gap_before_of[i] = gap_before;
            row.items.push((i, at));
            x = at + w;
            row.height = row.height.max(h);
            let above = specs[i].gaps.above.or(rule.above).unwrap_or(0.0);
            if above > row.above {
                row.above = above;
                row.above_from = specs[i].gaps.above.map(|_| specs[i].id);
            }
            let below = specs[i].gaps.below.or(rule.below).unwrap_or(0.0);
            if below > row.below {
                row.below = below;
                row.below_from = specs[i].gaps.below.map(|_| specs[i].id);
            }
        }
        if let Some(row) = current {
            rows.push((group.zone, row));
        }
    }

    // Vertical placement by zone. Between a power row and a signal row the
    // space holds two ducts and the separation between them.
    let separated = rules.separation > 0.0;
    let between_with = |upper: &Row, lower: &Row| -> (f64, RowGapCause) {
        let mut best = (rules.row_gap, RowGapCause::RowGap);
        if upper.below > best.0 {
            best = (
                upper.below,
                upper
                    .below_from
                    .map_or(RowGapCause::RowGap, RowGapCause::Part),
            );
        }
        if lower.above > best.0 {
            best = (
                lower.above,
                lower
                    .above_from
                    .map_or(RowGapCause::RowGap, RowGapCause::Part),
            );
        }
        if separated && upper.class.conflicts(lower.class) {
            let both = 2.0 * rules.duct_width + rules.separation + 2.0 * DUCT_CLEAR_MM;
            if both > best.0 {
                best = (both, RowGapCause::Separation);
            }
        }
        best
    };
    let between = |upper: &Row, lower: &Row| between_with(upper, lower).0;
    let in_zone = |zone: Zone| -> Vec<&Row> {
        rows.iter()
            .filter(|(z, _)| *z == zone)
            .map(|(_, r)| r)
            .collect()
    };
    let (tops, middles, bottoms) = (
        in_zone(Zone::Top),
        in_zone(Zone::Middle),
        in_zone(Zone::Bottom),
    );
    let limit = usable.y + usable.h;
    let mut placed: Vec<(f64, &Row, Zone)> = Vec::new();
    let mut skipped: Vec<&Row> = Vec::new();

    // Top rows flow down from the start.
    let mut cursor = usable.y.max(top);
    let mut last: Option<&Row> = None;
    for row in &tops {
        let y = last.map_or(cursor, |l| cursor + between(l, row));
        if y + row.height > limit {
            skipped.push(row);
            continue;
        }
        placed.push((y, row, Zone::Top));
        cursor = y + row.height;
        last = Some(row);
    }
    let top_end = cursor;
    let top_last = last;

    // Bottom rows: as many as fit under the top rows, kept against the bottom edge.
    let mut kept: Vec<&Row> = Vec::new();
    let mut height = 0.0;
    for row in &bottoms {
        let add = kept
            .last()
            .map_or(row.height, |l| between(l, row) + row.height);
        // The gap under the top rows belongs to the whole block, so it counts
        // for every row, measured to the block's first row.
        let lead = top_last.map_or(0.0, |l| between(l, kept.first().copied().unwrap_or(row)));
        if top_end + lead + height + add > limit {
            skipped.push(row);
        } else {
            height += add;
            kept.push(row);
        }
    }
    let mut bottom_start = limit - height;
    if let (Some(first), Some(l)) = (kept.first(), top_last) {
        bottom_start = bottom_start.max(top_end + between(l, first));
    }
    let mut y = bottom_start;
    for (n, row) in kept.iter().enumerate() {
        if n > 0 {
            y += between(kept[n - 1], row);
        }
        placed.push((y, row, Zone::Bottom));
        y += row.height;
    }

    // Middle rows fill what is between.
    let ceiling = kept.first().map_or(limit, |_| bottom_start);
    let ceiling_gap = |row: &Row| kept.first().map_or(0.0, |f| between(row, f));
    let mut cursor = top_end;
    let mut last = top_last;
    for row in &middles {
        let y = last.map_or(cursor, |l| cursor + between(l, row));
        if y + row.height + ceiling_gap(row) > ceiling {
            skipped.push(row);
            continue;
        }
        placed.push((y, row, Zone::Middle));
        cursor = y + row.height;
        last = Some(row);
    }

    for row in skipped {
        for (i, _) in &row.items {
            plan.unplaced.push(specs[*i].id);
            why.unplaced.push((specs[*i].id, UnplacedWhy::NoRoom));
        }
    }
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut previous: Option<(f64, &Row)> = None;
    for (n, (y, row, zone)) in placed.iter().enumerate() {
        let rail = first_rail_id + n as u64;
        plan.rails.push(Rail {
            id: rail,
            x: usable.x,
            y: *y,
            length: usable.w,
            vertical: false,
        });
        for (i, at) in &row.items {
            plan.placements.push(Proposed {
                id: specs[*i].id,
                x: usable.x + at,
                y: *y,
                rail,
            });
            why.decisions.push(Decision {
                id: specs[*i].id,
                rule: rule_ids[*i],
                zone: zones[*i],
                hot: hot_flags[*i],
                class: row.class,
                rail: Some(rail),
                gap_before: gap_before_of[*i].0,
                gap_source: gap_before_of[*i].1,
            });
        }
        why.rows.push(RowNote {
            rail,
            zone: *zone,
            class: row.class,
            space_above: previous.map(|(py, p)| y - (py + p.height)),
            cause: previous.map(|(_, p)| between_with(p, row).1),
        });
        if let Some((py, p)) = previous {
            // Ducts start on the trunk's centre line so they meet it.
            let duct_x = if trunk {
                full.x + strip / 2.0
            } else {
                usable.x
            };
            plan_ducts(
                &mut plan.ducts,
                rules,
                duct_x,
                usable.x + usable.w - duct_x,
                (py + p.height, p.class),
                (*y, row.class),
            );
        }
        previous = Some((*y, row));
    }
    if trunk && let (Some(first), Some(last)) = (placed.first(), placed.last()) {
        let start = first.0;
        plan.ducts.push(PlannedDuct {
            x: full.x + strip / 2.0,
            y: start,
            length: last.0 + last.1.height - start,
            vertical: true,
            width: rules.duct_width,
            height: rules.duct_height,
            class: Class::Mixed,
        });
        why.trunk = true;
    }
    (plan, why)
}

/// The duct class that serves a row next to a neighbour: a mixed row takes
/// the neighbour's class (signal when both are mixed).
fn duct_class(row: Class, other: Class) -> Class {
    match (row, other) {
        (Class::Mixed, Class::Mixed) => Class::Signal,
        (Class::Mixed, c) => c,
        (c, _) => c,
    }
}

/// Wiring ducts for the space between two rows (`upper_end` is the bottom
/// edge of the row above, `lower_start` the top edge of the row below).
fn plan_ducts(
    out: &mut Vec<PlannedDuct>,
    rules: &RuleSet,
    x: f64,
    length: f64,
    upper: (f64, Class),
    lower: (f64, Class),
) {
    let space = lower.0 - upper.0;
    let w = rules.duct_width;
    let duct = |centre: f64, class: Class| PlannedDuct {
        x,
        y: centre,
        length,
        vertical: false,
        width: w,
        height: rules.duct_height,
        class,
    };
    if rules.separation > 0.0 && upper.1.conflicts(lower.1) {
        // Two ducts, each hugging the row it serves, kept apart.
        out.push(duct(upper.0 + DUCT_CLEAR_MM + w / 2.0, upper.1));
        out.push(duct(lower.0 - DUCT_CLEAR_MM - w / 2.0, lower.1));
    } else if space >= w {
        let class = match (upper.1, lower.1) {
            (a, b) if a == b => duct_class(a, b),
            (Class::Mixed, b) => b,
            (a, _) => a,
        };
        out.push(duct(upper.0 + space / 2.0, class));
    }
}

pub mod model {
    //! Reads what the rules need from a project: the devices at an
    //! enclosure's location with their kind, their parts' clearances and
    //! their power loss.

    use novo_core::{Device, DeviceId, Part, Project, StructureId, Value};

    use super::{Gaps, Spec, kind_of};
    use crate::model::candidates;

    // Part property keys, the same as `novo_parts::keys` (the layout crate
    // does not depend on novo-parts).
    const ABOVE: &str = "clearance_above_mm";
    const BELOW: &str = "clearance_below_mm";
    const LEFT: &str = "clearance_left_mm";
    const RIGHT: &str = "clearance_right_mm";
    const LOSS: &str = "power_loss_w";
    const LOAD_FACTOR: &str = "load_factor";

    fn number(value: &Value) -> Option<f64> {
        match value {
            Value::Float(v) => Some(*v),
            Value::Int(v) => Some(*v as f64),
            _ => None,
        }
        .filter(|v| v.is_finite() && *v >= 0.0)
    }

    /// A property of the device, else of the first of its parts that has it.
    fn property(project: &Project, device: &Device, key: &str) -> Option<f64> {
        device.properties.get(key).and_then(number).or_else(|| {
            device.parts.iter().find_map(|part| {
                project
                    .get::<Part>(*part)?
                    .properties
                    .get(key)
                    .and_then(number)
            })
        })
    }

    /// Specs for the devices that belong in an enclosure (the same devices
    /// and order as [`candidates`]). Missing data stays `None`, never 0.
    pub fn specs(project: &Project, id: StructureId) -> Vec<Spec> {
        candidates(project, id)
            .into_iter()
            .map(|c| {
                let mut spec = Spec::from_candidate(&c);
                if let Some(device) = project.get::<Device>(DeviceId::from(c.id)) {
                    spec.kind = match device.tag.letter_code() {
                        "" => kind_of(&c.sort_key),
                        code => code.to_owned(),
                    };
                    spec.gaps = Gaps {
                        above: property(project, device, ABOVE),
                        below: property(project, device, BELOW),
                        left: property(project, device, LEFT),
                        right: property(project, device, RIGHT),
                    };
                    spec.loss_w = property(project, device, LOSS).map(|w| {
                        w * property(project, device, LOAD_FACTOR)
                            .unwrap_or(1.0)
                            .min(1.0)
                    });
                }
                spec
            })
            .collect()
    }

    /// The surface rules of an enclosure (`surface_rules` property) and the
    /// sentences for lines that could not be read.
    pub fn surface_rules(
        project: &Project,
        id: StructureId,
    ) -> (crate::surface_rules::SurfaceRules, Vec<String>) {
        match project
            .get::<novo_core::Structure>(id)
            .and_then(|s| s.properties.get(crate::surface_rules::SURFACE_RULES_KEY))
        {
            Some(Value::Text(t)) => crate::surface_rules::SurfaceRules::parse(t),
            Some(_) => (
                Default::default(),
                vec![format!(
                    "The '{}' property should be text such as 'door: H S P'.",
                    crate::surface_rules::SURFACE_RULES_KEY
                )],
            ),
            None => Default::default(),
        }
    }

    /// The rule violations of an enclosure's placed devices and ducts;
    /// empty when it has no plate.
    pub fn violations(
        project: &Project,
        id: StructureId,
        rules: &super::RuleSet,
    ) -> Vec<crate::check::Violation> {
        let Some(layout) = crate::model::load(project, id) else {
            return Vec::new();
        };
        crate::check::check(
            &layout.placements,
            &crate::model::ducts(project, id),
            &specs(project, id),
            rules,
        )
    }
}
