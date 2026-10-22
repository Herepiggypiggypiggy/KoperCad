//! Does a layout obey the place rules? [`place_all_rules`](crate::rules::place_all_rules)
//! builds a layout that does; this module judges one the user has edited by
//! hand, and every violation names the rule it breaks (by index into the
//! rule set, so the UI can jump to the rule) and by how much.
//!
//! Checked:
//! - **zone**: devices are in the order of their zones, top over middle over
//!   bottom (judged by the centre of each device, and reported for the
//!   device that is out of order with the most others);
//! - **hot**: a device at or over the hot limit takes the hot rule's zone
//!   (away from the top), so it is judged by the same zone order;
//! - **clearance**: the room a part (or its rule, or the hot gap) wants on
//!   each side of it is free of other devices;
//! - **separation**: power and signal ducts that run side by side are at
//!   least `separate` mm apart.
//!
//! Everything is millimetres on the plate, like the rest of the crate.

use std::collections::HashMap;

use crate::plan::PlannedDuct;
use crate::rules::{Class, GapSource, RuleSet, Spec, Zone};
use crate::{Placement, Rect};

/// Distances closer than this are equal (millimetres).
const EPS: f64 = 1e-6;

/// Which side of a device a clearance is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Above,
    Below,
    Left,
    Right,
}

impl Side {
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Above => "above",
            Side::Below => "below",
            Side::Left => "to the left",
            Side::Right => "to the right",
        }
    }
}

/// What is wrong, with the numbers.
#[derive(Debug, Clone, PartialEq)]
pub enum Problem {
    /// The device is out of the order of the zones: it should be `wants`
    /// relative to `against`, but is on the wrong side of it.
    WrongZone {
        device: u64,
        wants: Zone,
        against: u64,
        /// The zone comes from the hot rule (this device loses too much heat).
        hot: bool,
    },
    /// Less free room than wanted on one side of a device.
    Clearance {
        device: u64,
        side: Side,
        need: f64,
        actual: f64,
        /// The neighbour that is too close.
        neighbour: u64,
        source: GapSource,
    },
    /// A power duct and a signal duct side by side, closer than allowed.
    Separation {
        /// Indices into the ducts handed to [`check`].
        power: usize,
        signal: usize,
        need: f64,
        actual: f64,
    },
}

/// A broken rule.
#[derive(Debug, Clone, PartialEq)]
pub struct Violation {
    pub problem: Problem,
    /// Index of the rule in [`RuleSet::rules`] when a kind rule is the cause.
    pub rule: Option<usize>,
}

impl Violation {
    /// The devices involved, the offender first.
    pub fn devices(&self) -> Vec<u64> {
        match &self.problem {
            Problem::WrongZone {
                device, against, ..
            } => vec![*device, *against],
            Problem::Clearance {
                device, neighbour, ..
            } => vec![*device, *neighbour],
            Problem::Separation { .. } => Vec::new(),
        }
    }

    /// A short name of the rule, for a column or a filter.
    pub fn rule_name(&self) -> &'static str {
        match self.problem {
            Problem::WrongZone { hot: true, .. } => "hot",
            Problem::WrongZone { .. } => "zone",
            Problem::Clearance { .. } => "clearance",
            Problem::Separation { .. } => "separation",
        }
    }

    /// One sentence for the Issues list; `label` turns a device id into its tag.
    pub fn describe(&self, rules: &RuleSet, label: &dyn Fn(u64) -> String) -> String {
        let rule = |r: Option<usize>| match r {
            Some(i) => format!(" (rule `{}`)", rules.rule_text(i)),
            None => String::new(),
        };
        match &self.problem {
            Problem::WrongZone {
                device,
                wants,
                against,
                hot,
            } => format!(
                "{} belongs in the {} zone{}, which is {} {}{}.",
                label(*device),
                wants.as_str(),
                if *hot { " (it is hot)" } else { "" },
                if *wants == Zone::Bottom {
                    "below"
                } else {
                    "above"
                },
                label(*against),
                rule(self.rule)
            ),
            Problem::Clearance {
                device,
                side,
                need,
                actual,
                neighbour,
                source,
            } => {
                let by = match source {
                    GapSource::Part => "its part asks for",
                    GapSource::Kind => "the rule asks for",
                    GapSource::Hot => "a hot device needs",
                    _ => "the gap rule asks for",
                };
                format!(
                    "{} needs {need:.0} mm free {} ({by}); {} is {actual:.0} mm away{}.",
                    label(*device),
                    side.as_str(),
                    label(*neighbour),
                    rule(self.rule)
                )
            }
            Problem::Separation { need, actual, .. } => format!(
                "A power duct and a signal duct run {actual:.0} mm apart; the rules keep them {need:.0} mm apart."
            ),
        }
    }
}

/// The zone a device belongs to under the rules, and the rule that says so.
fn zone_of(
    rules: &RuleSet,
    spec: &Spec,
    lowered: &HashMap<u64, Zone>,
) -> (Zone, Option<usize>, bool) {
    let (index, rule) = rules.rule_for_kind(&spec.kind);
    if let Some(zone) = lowered.get(&spec.id) {
        return (*zone, index, false);
    }
    if let Some(h) = rules.hot
        && spec.loss_w.is_some_and(|w| w >= h.min_loss_w)
    {
        return (h.zone, index, true);
    }
    (rule.zone, index, false)
}

/// Checks a layout against the rules. `specs` carry kind, part clearances
/// and loss; a placement without a spec is only checked for nothing, and a
/// spec without a placement is ignored.
pub fn check(
    placements: &[Placement],
    ducts: &[PlannedDuct],
    specs: &[Spec],
    rules: &RuleSet,
) -> Vec<Violation> {
    let spec_of: HashMap<u64, &Spec> = specs.iter().map(|s| (s.id, s)).collect();
    let mut out = Vec::new();
    // Sensitive devices the heat step brought below the hot zone.
    let lowered = crate::heat::zone_override(rules, specs);

    let mut ordered: Vec<&Placement> = placements
        .iter()
        .filter(|p| spec_of.contains_key(&p.id))
        .collect();
    ordered.sort_by_key(|p| p.id);

    // Zone order: count, for every device, the others it is out of order
    // with, and report the worst offender until no inversion is left.
    struct Z {
        id: u64,
        zone: Zone,
        rule: Option<usize>,
        hot: bool,
        cy: f64,
    }
    let mut zs: Vec<Z> = ordered
        .iter()
        .map(|p| {
            let (zone, rule, hot) = zone_of(rules, spec_of[&p.id], &lowered);
            let f = p.footprint();
            Z {
                id: p.id,
                zone,
                rule,
                hot,
                cy: f.y + f.h / 2.0,
            }
        })
        .collect();
    let inverted = |a: &Z, b: &Z| a.zone < b.zone && a.cy > b.cy + EPS;
    loop {
        let counts: Vec<usize> = zs
            .iter()
            .map(|a| {
                zs.iter()
                    .filter(|b| inverted(a, b) || inverted(b, a))
                    .count()
            })
            .collect();
        let Some((worst, &n)) = counts
            .iter()
            .enumerate()
            .max_by_key(|(i, n)| (**n, usize::MAX - i))
        else {
            break;
        };
        if n == 0 {
            break;
        }
        let a = &zs[worst];
        let against = zs
            .iter()
            .filter(|b| inverted(a, b) || inverted(b, a))
            .min_by(|x, y| (x.cy - a.cy).abs().total_cmp(&(y.cy - a.cy).abs()))
            .map(|b| b.id)
            .unwrap_or(a.id);
        out.push(Violation {
            problem: Problem::WrongZone {
                device: a.id,
                wants: a.zone,
                against,
                hot: a.hot,
            },
            rule: if a.hot { None } else { a.rule },
        });
        zs.remove(worst);
    }

    for p in &ordered {
        let spec = spec_of[&p.id];
        let foot = p.footprint();
        let (_, rule_index, _) = zone_of(rules, spec, &lowered);
        out.extend(clearances(p, spec, &foot, &ordered, rules, rule_index));
    }
    out.extend(separations(ducts, rules));
    out
}

/// Needs on each side: the part's figure, else the rule's, else for the
/// sides along a rail the rule set's gap; a hot device needs at least the
/// hot gap on every side.
fn needs(spec: &Spec, rules: &RuleSet, rule_index: Option<usize>) -> [(Side, f64, GapSource); 4] {
    let rule = rule_index.map(|i| &rules.rules[i]);
    let hot = rules
        .hot
        .is_some_and(|h| spec.loss_w.is_some_and(|w| w >= h.min_loss_w));
    let pick = |part: Option<f64>, rule_v: Option<f64>, default: Option<f64>| {
        let base = if let Some(v) = part {
            (v, GapSource::Part)
        } else if let Some(v) = rule_v {
            (v, GapSource::Kind)
        } else {
            (default.unwrap_or(0.0), GapSource::RuleSet)
        };
        if hot && rules.hot_gap > base.0 {
            (rules.hot_gap, GapSource::Hot)
        } else {
            base
        }
    };
    let side =
        |part: Option<f64>, rule_v: Option<f64>, default: Option<f64>| pick(part, rule_v, default);
    let a = side(spec.gaps.above, rule.and_then(|r| r.above), None);
    let b = side(spec.gaps.below, rule.and_then(|r| r.below), None);
    let l = side(spec.gaps.left, rule.and_then(|r| r.gap), Some(rules.gap));
    let r = side(spec.gaps.right, rule.and_then(|r| r.gap), Some(rules.gap));
    [
        (Side::Above, a.0, a.1),
        (Side::Below, b.0, b.1),
        (Side::Left, l.0, l.1),
        (Side::Right, r.0, r.1),
    ]
}

fn clearances(
    p: &Placement,
    spec: &Spec,
    foot: &Rect,
    all: &[&Placement],
    rules: &RuleSet,
    rule_index: Option<usize>,
) -> Vec<Violation> {
    let mut out = Vec::new();
    for (side, need, source) in needs(spec, rules, rule_index) {
        if need <= EPS {
            continue;
        }
        // The nearest neighbour on that side whose extent overlaps ours.
        let mut worst: Option<(f64, u64)> = None;
        for q in all {
            if q.id == p.id {
                continue;
            }
            let o = q.footprint();
            let (overlaps_along, dist) = match side {
                Side::Above => (
                    o.x < foot.right() && foot.x < o.right(),
                    foot.y - o.bottom(),
                ),
                Side::Below => (
                    o.x < foot.right() && foot.x < o.right(),
                    o.y - foot.bottom(),
                ),
                Side::Left => (
                    o.y < foot.bottom() && foot.y < o.bottom(),
                    foot.x - o.right(),
                ),
                Side::Right => (
                    o.y < foot.bottom() && foot.y < o.bottom(),
                    o.x - foot.right(),
                ),
            };
            // `dist` < 0 means the neighbour is on the other side (or overlaps,
            // which the collision check reports).
            if !overlaps_along || dist < -EPS {
                continue;
            }
            if dist < need - EPS && worst.is_none_or(|(d, _)| dist < d) {
                worst = Some((dist, q.id));
            }
        }
        if let Some((actual, neighbour)) = worst {
            out.push(Violation {
                problem: Problem::Clearance {
                    device: p.id,
                    side,
                    need,
                    actual,
                    neighbour,
                    source,
                },
                rule: matches!(source, GapSource::Kind)
                    .then_some(rule_index)
                    .flatten(),
            });
        }
    }
    out
}

/// The rectangle a duct covers on the plate (centre line, `width` across).
pub fn duct_rect(d: &PlannedDuct) -> Rect {
    if d.vertical {
        Rect {
            x: d.x - d.width / 2.0,
            y: d.y,
            w: d.width,
            h: d.length,
        }
    } else {
        Rect {
            x: d.x,
            y: d.y - d.width / 2.0,
            w: d.length,
            h: d.width,
        }
    }
}

fn separations(ducts: &[PlannedDuct], rules: &RuleSet) -> Vec<Violation> {
    let mut out = Vec::new();
    if rules.separation <= 0.0 {
        return out;
    }
    for (i, a) in ducts.iter().enumerate() {
        for (j, b) in ducts.iter().enumerate().skip(i + 1) {
            if !a.class.conflicts(b.class) || a.vertical != b.vertical {
                continue;
            }
            let (ra, rb) = (duct_rect(a), duct_rect(b));
            // Side by side: they overlap along the run, and their edges are
            // closer across than the separation.
            let (along, across) = if a.vertical {
                (
                    ra.y < rb.bottom() && rb.y < ra.bottom(),
                    (rb.x - ra.right()).max(ra.x - rb.right()),
                )
            } else {
                (
                    ra.x < rb.right() && rb.x < ra.right(),
                    (rb.y - ra.bottom()).max(ra.y - rb.bottom()),
                )
            };
            if along && across < rules.separation - EPS {
                let (power, signal) = if a.class == Class::Power {
                    (i, j)
                } else {
                    (j, i)
                };
                out.push(Violation {
                    problem: Problem::Separation {
                        power,
                        signal,
                        need: rules.separation,
                        actual: across.max(0.0),
                    },
                    rule: None,
                });
            }
        }
    }
    out
}
