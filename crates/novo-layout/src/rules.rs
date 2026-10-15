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

use crate::plan::{Candidate, Plan, Proposed, ROW_GAP_MM};
use crate::state::CabinetLayout;
use crate::{DEFAULT_CLEARANCE_MM, Plate, Rail, Size};

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
        }
    }
}

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
            hot: None,
            rules: vec![
                KindRule::new(&["Q", "F", "G"], Zone::Top, RailUse::Shared),
                KindRule::new(&["A", "K"], Zone::Middle, RailUse::Own),
                KindRule::new(&["T", "U"], Zone::Bottom, RailUse::Own),
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

    /// Reads a rule set from text. Blank lines and `#` comments are ignored.
    /// A line that is not understood is an error naming its number.
    pub fn parse(text: &str) -> Result<RuleSet, ParseError> {
        let mut set = RuleSet {
            gap: DEFAULT_CLEARANCE_MM,
            row_gap: ROW_GAP_MM,
            hot: None,
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
                "gap" | "row_gap" => {
                    let [value] = rest.as_slice() else {
                        return Err(err(format!("`{keyword}` takes one number of millimetres")));
                    };
                    let v = mm(value).ok_or_else(|| err(format!("`{value}` is not a distance")))?;
                    if keyword == "gap" {
                        set.gap = v;
                    } else {
                        set.row_gap = v;
                    }
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

    /// The rule set as text that [`RuleSet::parse`] reads back.
    pub fn to_text(&self) -> String {
        let mut out = String::from("# KoperCAD place rules: where Place all puts each kind\n");
        out.push_str(&format!("gap {}\nrow_gap {}\n", self.gap, self.row_gap));
        if let Some(h) = self.hot {
            out.push_str(&format!("hot {} {}\n", h.min_loss_w, h.zone.as_str()));
        }
        for r in &self.rules {
            out.push_str(&format!(
                "rule {} zone={} rail={}",
                r.codes.join(" "),
                r.zone.as_str(),
                r.rail.as_str()
            ));
            for (key, v) in [("gap", r.gap), ("above", r.above), ("below", r.below)] {
                if let Some(v) = v {
                    out.push_str(&format!(" {key}={v}"));
                }
            }
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
}

struct Group {
    zone: Zone,
    order: (usize, usize),
    name: String,
    members: Vec<usize>,
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
    let usable = plate.usable();
    let mut plan = Plan::default();

    // Which rule, and so which zone and group, each device falls under.
    let mut groups: Vec<Group> = Vec::new();
    let mut resolved: Vec<(usize, KindRule)> = Vec::with_capacity(specs.len());
    for (i, spec) in specs.iter().enumerate() {
        let (rule_index, code_index, rule) = rules.rule_for(&spec.kind);
        let zone = match rules.hot {
            Some(h) if spec.loss_w.is_some_and(|w| w >= h.min_loss_w) => h.zone,
            _ => rule.zone,
        };
        let name = match rule.rail {
            RailUse::Own => spec.kind.to_ascii_uppercase(),
            RailUse::Shared => String::new(),
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
                members: vec![i],
            }),
        }
        resolved.push((rule_index, rule));
    }
    groups.sort_by(|a, b| (a.zone, a.order, &a.name).cmp(&(b.zone, b.order, &b.name)));

    // Rows of each group, left to right inside the usable width.
    let need = |i: usize, side: fn(&Gaps) -> Option<f64>, rule_side: Option<f64>| -> f64 {
        side(&specs[i].gaps)
            .or(rule_side)
            .unwrap_or(rules.gap)
            .max(0.0)
    };
    let mut rows: Vec<(Zone, Row)> = Vec::new();
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
                continue;
            }
            let rule = &resolved[i].1;
            let left = need(i, |g| g.left, rule.gap);
            let mut gap_before = 0.0;
            if let Some(row) = &current {
                let (prev, _) = row.items.last().copied().unwrap_or((i, 0.0));
                let prev_rule = &resolved[prev].1;
                gap_before = need(prev, |g| g.right, prev_rule.gap).max(left);
                if x + gap_before + w > usable.w {
                    rows.push((group.zone, current.take().expect("row is open")));
                }
            }
            let row = current.get_or_insert_with(|| {
                x = 0.0;
                gap_before = 0.0;
                Row {
                    items: Vec::new(),
                    height: 0.0,
                    above: 0.0,
                    below: 0.0,
                }
            });
            let at = x + gap_before;
            row.items.push((i, at));
            x = at + w;
            row.height = row.height.max(h);
            row.above = row
                .above
                .max(specs[i].gaps.above.or(rule.above).unwrap_or(0.0));
            row.below = row
                .below
                .max(specs[i].gaps.below.or(rule.below).unwrap_or(0.0));
        }
        if let Some(row) = current {
            rows.push((group.zone, row));
        }
    }

    // Vertical placement by zone.
    let between = |upper: &Row, lower: &Row| rules.row_gap.max(upper.below).max(lower.above);
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
    let mut placed: Vec<(f64, &Row)> = Vec::new();
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
        placed.push((y, row));
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
        placed.push((y, row));
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
        placed.push((y, row));
        cursor = y + row.height;
        last = Some(row);
    }

    for row in skipped {
        plan.unplaced
            .extend(row.items.iter().map(|(i, _)| specs[*i].id));
    }
    placed.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (n, (y, row)) in placed.iter().enumerate() {
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
        }
    }
    plan
}

/// What Place all proposes for the devices of `specs` that are not on the
/// plate yet, by the rules. Like [`CabinetLayout::preview`], the plan sits
/// below everything already placed, its rails continue the numbering, and it
/// is only a proposal: apply it with `model::apply_plan`.
pub fn preview(layout: &CabinetLayout, specs: &[Spec], rules: &RuleSet) -> Plan {
    let unplaced: Vec<Spec> = specs
        .iter()
        .filter(|s| !layout.placements.iter().any(|p| p.id == s.id))
        .cloned()
        .collect();
    let top = layout
        .placements
        .iter()
        .map(|p| p.footprint().bottom() + ROW_GAP_MM)
        .fold(0.0, f64::max);
    let first_rail = layout.rails.iter().map(|r| r.id + 1).max().unwrap_or(1);
    place_all_rules(&layout.plate, &unplaced, rules, first_rail, top)
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
}
