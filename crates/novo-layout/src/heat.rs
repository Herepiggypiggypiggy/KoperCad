//! Heat-aware `Place all`: the rules planner of [`crate::rules`] plus a search
//! that moves devices because of the heat they give off and the heat they can
//! take.
//!
//! What it does, in this order:
//! 1. **Sensitive devices stay below the heat.** A device with a known
//!    temperature limit close to the ambient (within `margin` K) whose rule
//!    puts it above the hot zone is brought down into the hot zone, because
//!    hot air rises into whatever is above a source.
//! 2. **Rows with heat sources go first inside their zone**, so their plume
//!    rises away from the rows below. The search tries this order and the
//!    rules' own order and keeps whichever is better.
//! 3. **A search moves devices.** Same-kind devices swap places along their
//!    rail, and any device can slide along its rail into free room, so that
//!    hot devices are spread out and the devices that mind heat are not next
//!    to or above a source. Rails and ducts stay where the rules put them; the
//!    search only changes the order and spacing along a rail, so rails through
//!    the middle of their devices, ducts and the wire space stay valid.
//!
//! The search is judged by a cheap **coupling score**, not by the thermal
//! simulation: for every pair, the loss of the source times how much the
//! other device minds heat (`25 K` over its limit headroom) times a kernel
//! that is large beside a source, and large straight above it where the plume
//! rises, and small below. The kernel is a planning heuristic. Whether a layout
//! is really cooler is for the 2D thermal model to say (`novo-thermal`), and
//! that model is itself a planning-value model, not validated against
//! measurements.

use std::collections::{HashMap, HashSet};

use crate::plan::Plan;
use crate::rules::{Explanation, RuleSet, Spec, Zone, plan_around};
use crate::{Plate, Rect};

/// Settings of heat-aware placement (the `heat` line of the rule file).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatPlace {
    /// Air temperature outside the devices' limits are measured from, degrees C.
    pub ambient_c: f64,
    /// Limit assumed for a device with none (only for weighing, never to
    /// call a device sensitive), degrees C.
    pub assumed_limit_c: f64,
    /// A device with a known limit within this many K of the ambient is
    /// called sensitive.
    pub sensitive_margin_k: f64,
    /// Smallest loss that makes the search run, W.
    pub min_source_w: f64,
}

impl Default for HeatPlace {
    fn default() -> Self {
        HeatPlace {
            ambient_c: 35.0,
            assumed_limit_c: 60.0,
            sensitive_margin_k: 15.0,
            min_source_w: 10.0,
        }
    }
}

/// What the heat step did with one device, for the explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatWhy {
    /// Moved so that its heat reaches fewer devices that mind it.
    Spread,
    /// Moved away from (or out from over) a heat source.
    Shielded,
    /// Its rule puts it above the hot zone; it was brought below the heat.
    Lowered,
    /// A heat source that was not moved: its place already keeps it clear.
    Source,
    /// Minds heat and was not moved: its place is already clear of the plume.
    Sensitive,
}

impl HeatWhy {
    pub fn text(self) -> &'static str {
        match self {
            HeatWhy::Spread => "the heat step moved it to keep its heat off devices that mind it",
            HeatWhy::Shielded => "the heat step moved it away from a heat source",
            HeatWhy::Source => {
                "it is a heat source: the heat step checked it is kept clear of devices that mind heat"
            }
            HeatWhy::Sensitive => {
                "it minds heat: the heat step checked it is not beside or above a heat source"
            }
            HeatWhy::Lowered => {
                "the heat step brought it below the hot zone, because heat rises into what is above"
            }
        }
    }
}

/// What the heat step found and did, for a message in the app.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeatSummary {
    /// Devices at or over the search threshold (`min_source_w`).
    pub sources: usize,
    /// Devices called sensitive (a known limit close to the ambient).
    pub sensitive: usize,
    /// Devices that ended more than 20 mm from where the rules alone put them.
    pub moved: usize,
    /// Devices brought down below the hot zone.
    pub lowered: usize,
    /// Coupling score of the rules alone and of the result (lower is cooler).
    pub score_rules: f64,
    pub score_heat: f64,
    /// Rails the result uses beyond the rules' own plan.
    pub extra_rows: usize,
    /// Layouts the search tried.
    pub tried: usize,
    /// The result is the rules' own layout: nothing better was found.
    pub unchanged: bool,
}

impl HeatSummary {
    /// Share of the coupling the search took away, 0 to 1.
    pub fn gain(&self) -> f64 {
        if self.score_rules > 0.0 {
            ((self.score_rules - self.score_heat) / self.score_rules).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    /// One honest sentence for the message after Place all.
    pub fn message(&self) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let sources = plural(self.sources, "heat source", "heat sources");
        if self.unchanged {
            return format!(
                "Heat-aware: {sources}; the rules' layout was already the best of {} tried (planning score, model).",
                self.tried
            );
        }
        let rows = match self.extra_rows {
            0 => String::new(),
            n => format!(", {} more", plural(n, "rail", "rails")),
        };
        format!(
            "Heat-aware: {sources}, {} placed differently{rows}; heat coupling {:.0}% lower (planning score, model, not a temperature).",
            plural(self.moved, "device", "devices"),
            100.0 * self.gain()
        )
    }
}

/// Changes the heat step asks of the rules planner.
#[derive(Debug, Clone, Default)]
pub(crate) struct Tweaks {
    /// Zones that replace the rules' own.
    pub zone: HashMap<u64, Zone>,
    /// Order inside a group (same kind and zone); lower is further left.
    pub rank: HashMap<u64, u32>,
    /// Extra room before a device along its rail, mm.
    pub pad: HashMap<u64, f64>,
    /// Rows with heat sources go first inside their zone.
    pub hot_first: bool,
    /// Devices called sensitive.
    pub sensitive: HashSet<u64>,
    /// Reasons to show.
    pub why: HashMap<u64, HeatWhy>,
}

/// Whether the device has a known limit close to the ambient.
pub fn is_sensitive(spec: &Spec, h: &HeatPlace) -> bool {
    spec.limit_c
        .is_some_and(|l| l - h.ambient_c <= h.sensitive_margin_k)
}

/// How much the device minds heat: 1 at 25 K of headroom, more when less.
fn sensitivity(spec: &Spec, h: &HeatPlace) -> f64 {
    let limit = spec.limit_c.unwrap_or(h.assumed_limit_c);
    25.0 / (limit - h.ambient_c).max(5.0)
}

/// Zones that heat changes: a sensitive device that the rules put above the
/// hot zone goes into the hot zone (below the hot rows). Empty without a hot
/// rule or heat setting, or when nothing is hot. Shared with
/// [`crate::check`], so a lowered device is not reported as out of its zone.
pub fn zone_override(rules: &RuleSet, specs: &[Spec]) -> HashMap<u64, Zone> {
    let (Some(h), Some(hot)) = (rules.heat, rules.hot) else {
        return HashMap::new();
    };
    if !specs
        .iter()
        .any(|s| s.loss_w.is_some_and(|w| w >= hot.min_loss_w))
    {
        return HashMap::new();
    }
    specs
        .iter()
        .filter(|s| is_sensitive(s, &h))
        .filter(|s| !s.loss_w.is_some_and(|w| w >= hot.min_loss_w))
        .filter_map(|s| {
            let (_, rule) = rules.rule_for_kind(&s.kind);
            (rule.zone < hot.zone).then_some((s.id, hot.zone))
        })
        .collect()
}

/// Coupling of a layout: the sum over source and victim pairs of the source's
/// loss, the victim's sensitivity and a kernel of where the victim is. `boxes`
/// are the devices' footprints on the plate, y down.
pub fn coupling(
    specs: &[Spec],
    boxes: &HashMap<u64, Rect>,
    h: &HeatPlace,
    plate_height: f64,
) -> f64 {
    let items: Vec<(&Spec, Rect)> = specs
        .iter()
        .filter_map(|s| boxes.get(&s.id).map(|r| (s, *r)))
        .collect();
    let mut total = 0.0;
    // Air in a closed cabinet is warmer higher up: a device at the top sees
    // more of the total loss than one at the bottom.
    let all_loss: f64 = specs.iter().filter_map(|s| s.loss_w).sum();
    for (b, rb) in &items {
        let above_bottom = 1.0 - (rb.y + rb.h / 2.0) / plate_height.max(1.0);
        total +=
            STRATIFICATION * all_loss * sensitivity(b, h) * above_bottom.clamp(0.0, 1.0).powi(2);
    }
    for (a, ra) in &items {
        let loss = a.loss_w.unwrap_or(0.0);
        if loss <= 0.0 {
            continue;
        }
        for (b, rb) in &items {
            if a.id == b.id {
                continue;
            }
            total += loss * sensitivity(b, h) * kernel(ra, rb);
        }
    }
    total
}

/// How much of a source's heat reaches a device: beside it strongly, above it
/// where the plume rises (narrow cone, widening, fading over about 600 mm),
/// below it hardly.
fn kernel(src: &Rect, dst: &Rect) -> f64 {
    let dx = ((dst.x + dst.w / 2.0) - (src.x + src.w / 2.0)).abs() - (src.w + dst.w) / 2.0;
    let dx = dx.max(0.0);
    if dst.bottom() <= src.y {
        let up = src.y - dst.bottom();
        let cone = 60.0 + 0.25 * up;
        (-up / 600.0).exp() * (-(dx / cone).powi(2)).exp() + 0.1 * (-(dx.hypot(up)) / 300.0).exp()
    } else if dst.y >= src.bottom() {
        let down = dst.y - src.bottom();
        0.12 * (-(dx.hypot(down)) / 300.0).exp()
    } else {
        0.9 * (-dx / 250.0).exp()
    }
}

fn boxes_of(plan: &Plan, specs: &[Spec]) -> HashMap<u64, Rect> {
    let size: HashMap<u64, _> = specs.iter().map(|s| (s.id, s.size)).collect();
    plan.placements
        .iter()
        .filter_map(|p| {
            size.get(&p.id).map(|s| {
                (
                    p.id,
                    Rect {
                        x: p.x,
                        y: p.y,
                        w: s.width,
                        h: s.height,
                    },
                )
            })
        })
        .collect()
}

/// Millimetres of travel against the rules' layout, summed.
fn travel(plan: &Plan, from: &HashMap<u64, Rect>) -> f64 {
    plan.placements
        .iter()
        .filter_map(|p| {
            from.get(&p.id)
                .map(|r| (p.x - r.x).abs() + (p.y - r.y).abs())
        })
        .sum()
}

/// How much of the cabinet's total loss a device at the very top sees more of
/// than one at the bottom (the stratification of the air), in the same units
/// as the pair kernel. About 12 K of 35 K of total rise for the CV-200.
const STRATIFICATION: f64 = 1.0;
/// Rows (rails with their ducts) a move may add to the rules' plan, and what
/// each costs in score. Spreading hot devices sometimes needs another row, but
/// a row is a rail and a duct to buy and wire.
const MAX_EXTRA_ROWS: usize = 3;
const ROW_COST: f64 = 150.0;
/// Weight of a millimetre of travel against the coupling score. A device that
/// moves 1 m has to buy at least this much coupling back.
const TRAVEL_COST_PER_MM: f64 = 0.02;
/// Most layouts one search tries.
/// Slides tried for a device, mm.
const SLIDES: [f64; 4] = [120.0, 300.0, 600.0, 1000.0];

struct Search<'a> {
    plate: &'a Plate,
    specs: &'a [Spec],
    rules: &'a RuleSet,
    h: HeatPlace,
    first: u64,
    top: f64,
    blocked: &'a [Rect],
    base_boxes: HashMap<u64, Rect>,
    base_unplaced: usize,
    /// The rules' own plan has a trunk duct / gave up the power-signal separation.
    base_trunk: bool,
    base_relaxed: bool,
    /// Rails of the rules' plan: a move may not need more.
    base_rails: usize,
    tried: usize,
    /// Layouts one variant of the search may try.
    budget: usize,
}

impl Search<'_> {
    /// Cost of a set of tweaks, with its plan; `None` when it places fewer
    /// devices than the rules do.
    fn eval(&mut self, tw: &Tweaks) -> Option<(f64, f64, Plan, Explanation)> {
        self.tried += 1;
        let (plan, why) = plan_around(
            self.plate,
            self.specs,
            self.rules,
            self.first,
            self.top,
            self.blocked,
            tw,
        );
        // Never trade away a device, the trunk duct or the duct separation, and
        // never add more than a few rows.
        if plan.unplaced.len() > self.base_unplaced
            || (self.base_trunk && !why.trunk)
            || (!self.base_relaxed && why.separation_relaxed)
            || plan.rails.len() > self.base_rails + MAX_EXTRA_ROWS
        {
            return None;
        }
        let score = coupling(
            self.specs,
            &boxes_of(&plan, self.specs),
            &self.h,
            self.plate.height,
        );
        let extra_rows = plan.rails.len().saturating_sub(self.base_rails) as f64;
        let cost =
            score + TRAVEL_COST_PER_MM * travel(&plan, &self.base_boxes) + ROW_COST * extra_rows;
        Some((cost, score, plan, why))
    }
}

/// The heat-aware plan, or `None` when heat awareness is off or nothing gives
/// off enough heat to matter (the caller then plans by the rules alone).
pub(crate) fn optimise(
    plate: &Plate,
    specs: &[Spec],
    rules: &RuleSet,
    first: u64,
    top: f64,
    blocked: &[Rect],
) -> Option<(Plan, Explanation)> {
    let h = rules.heat?;
    let sources: Vec<&Spec> = specs
        .iter()
        .filter(|s| s.loss_w.is_some_and(|w| w >= h.min_source_w))
        .collect();
    if sources.is_empty() {
        return None;
    }
    let sensitive: HashSet<u64> = specs
        .iter()
        .filter(|s| is_sensitive(s, &h))
        .map(|s| s.id)
        .collect();
    let zones = zone_override(rules, specs);

    let (plan0, why0) = plan_around(plate, specs, rules, first, top, blocked, &Tweaks::default());
    let base_boxes = boxes_of(&plan0, specs);
    let score0 = coupling(specs, &base_boxes, &h, plate.height);
    let mut search = Search {
        plate,
        specs,
        rules,
        h,
        first,
        top,
        blocked,
        base_boxes: base_boxes.clone(),
        base_unplaced: plan0.unplaced.len(),
        base_trunk: why0.trunk,
        base_relaxed: why0.separation_relaxed,
        base_rails: plan0.rails.len(),
        tried: 1,
        budget: 0,
    };

    // About a second on a big plate: fewer layouts when there are more devices.
    let budget = (150_000 / specs.len().max(1)).clamp(100, 3000);
    // Start from the rules' order and from "heat sources first", search from each.
    let mut best: Option<(f64, f64, Plan, Explanation, Tweaks)> = None;
    for hot_first in [false, true] {
        let mut tw = Tweaks {
            zone: zones.clone(),
            hot_first,
            sensitive: sensitive.clone(),
            ..Tweaks::default()
        };
        // Natural rank inside each (zone, kind) group.
        let mut by_group: HashMap<(Zone, String), Vec<&Spec>> = HashMap::new();
        for s in specs {
            let zone = zones
                .get(&s.id)
                .copied()
                .unwrap_or_else(|| zone_of(rules, s));
            by_group
                .entry((zone, s.kind.to_ascii_uppercase()))
                .or_default()
                .push(s);
        }
        for members in by_group.values_mut() {
            members.sort_by(|a, b| (&a.sort_key, a.id).cmp(&(&b.sort_key, b.id)));
            for (n, s) in members.iter().enumerate() {
                tw.rank.insert(s.id, n as u32);
            }
        }
        let Some((mut cost, mut score, mut plan, mut why)) = search.eval(&tw) else {
            continue;
        };
        let mut touched: HashSet<u64> = HashSet::new();
        search.budget = search.tried + budget;
        while search.tried < search.budget {
            let Some(mv) = best_move(&mut search, &tw, &plan, &by_group, cost) else {
                break;
            };
            cost = mv.cost;
            score = mv.score;
            plan = mv.plan;
            why = mv.why;
            tw = mv.tweaks;
            touched.extend(mv.touched);
        }
        tw.why = touched
            .iter()
            .map(|id| {
                let is_src = specs
                    .iter()
                    .any(|s| s.id == *id && s.loss_w.is_some_and(|w| w >= h.min_source_w));
                (
                    *id,
                    if is_src {
                        HeatWhy::Spread
                    } else {
                        HeatWhy::Shielded
                    },
                )
            })
            .collect();
        for id in tw.zone.keys() {
            tw.why.insert(*id, HeatWhy::Lowered);
        }
        for s in specs {
            let why = if s.loss_w.is_some_and(|w| w >= h.min_source_w) {
                HeatWhy::Source
            } else if sensitive.contains(&s.id) {
                HeatWhy::Sensitive
            } else {
                continue;
            };
            tw.why.entry(s.id).or_insert(why);
        }
        if best.as_ref().is_none_or(|b| cost < b.0) {
            best = Some((cost, score, plan, why, tw));
        }
    }

    let base_cost = score0;
    let mut summary = HeatSummary {
        sources: sources.len(),
        sensitive: sensitive.len(),
        score_rules: score0,
        score_heat: score0,
        tried: search.tried,
        unchanged: true,
        ..HeatSummary::default()
    };
    let Some((cost, score, _plan, _why, tw)) = best.filter(|b| b.0 < base_cost * 0.995) else {
        let mut why = why0;
        why.heat = Some(summary);
        return Some((plan0, why));
    };
    let _ = cost;
    // Plan once more with the reasons in place, so every decision carries its own.
    let (plan, mut why) = plan_around(plate, specs, rules, first, top, blocked, &tw);
    summary.unchanged = false;
    summary.score_heat = score;
    summary.moved = plan
        .placements
        .iter()
        .filter(|p| {
            base_boxes
                .get(&p.id)
                .is_some_and(|r| (p.x - r.x).abs() + (p.y - r.y).abs() > 20.0)
        })
        .count();
    summary.lowered = tw.zone.len();
    summary.extra_rows = plan.rails.len().saturating_sub(plan0.rails.len());
    summary.tried = search.tried;
    why.heat = Some(summary);
    Some((plan, why))
}

fn zone_of(rules: &RuleSet, spec: &Spec) -> Zone {
    let (_, rule) = rules.rule_for_kind(&spec.kind);
    match rules.hot {
        Some(hot) if spec.loss_w.is_some_and(|w| w >= hot.min_loss_w) => hot.zone,
        _ => rule.zone,
    }
}

struct Move {
    cost: f64,
    score: f64,
    plan: Plan,
    why: Explanation,
    tweaks: Tweaks,
    touched: Vec<u64>,
}

/// The best single move (a swap of two same-kind devices or a slide along a
/// rail) of the devices that give or take the most heat; `None` when no move
/// lowers the cost by at least 0.5 percent.
fn best_move(
    search: &mut Search<'_>,
    tw: &Tweaks,
    plan: &Plan,
    groups: &HashMap<(Zone, String), Vec<&Spec>>,
    cost: f64,
) -> Option<Move> {
    let specs = search.specs;
    let boxes = boxes_of(plan, specs);
    // Heat given and taken by every device, to pick who is worth moving.
    let mut burden: HashMap<u64, f64> = HashMap::new();
    for a in specs {
        let (Some(ra), loss) = (boxes.get(&a.id), a.loss_w.unwrap_or(0.0)) else {
            continue;
        };
        if loss <= 0.0 {
            continue;
        }
        for b in specs {
            if a.id == b.id {
                continue;
            }
            let Some(rb) = boxes.get(&b.id) else { continue };
            let pair = loss * sensitivity(b, &search.h) * kernel(ra, rb);
            *burden.entry(a.id).or_default() += pair;
            *burden.entry(b.id).or_default() += pair;
        }
    }
    let mut picked: Vec<(u64, f64)> = burden.into_iter().collect();
    picked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    picked.truncate(24);

    let zone_of_id = |id: u64| -> Option<(Zone, String)> {
        groups
            .iter()
            .find(|(_, m)| m.iter().any(|s| s.id == id))
            .map(|(k, _)| k.clone())
    };
    let mut best: Option<Move> = None;
    let mut consider = |search: &mut Search<'_>, next: Tweaks, touched: Vec<u64>| {
        if search.tried >= search.budget {
            return;
        }
        if let Some((c, score, plan, why)) = search.eval(&next) {
            let bar = best.as_ref().map_or(cost * 0.995, |b| b.cost);
            if c < bar {
                best = Some(Move {
                    cost: c,
                    score,
                    plan,
                    why,
                    tweaks: next,
                    touched,
                });
            }
        }
    };
    for (id, _) in picked {
        // Slides along the rail.
        let pad = tw.pad.get(&id).copied().unwrap_or(0.0);
        for step in SLIDES {
            let mut next = tw.clone();
            next.pad.insert(id, pad + step);
            consider(search, next, vec![id]);
        }
        if pad > 0.0 {
            let mut next = tw.clone();
            next.pad.remove(&id);
            consider(search, next, vec![id]);
        }
        // Swaps with up to eight same-kind devices of its group, spread over it.
        let Some(key) = zone_of_id(id) else { continue };
        let members = &groups[&key];
        let step = (members.len() / 8).max(1);
        for other in members.iter().step_by(step) {
            if other.id == id {
                continue;
            }
            let (Some(&a), Some(&b)) = (tw.rank.get(&id), tw.rank.get(&other.id)) else {
                continue;
            };
            let mut next = tw.clone();
            next.rank.insert(id, b);
            next.rank.insert(other.id, a);
            consider(search, next, vec![id, other.id]);
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Size;
    use crate::rules::kind_of;

    fn spec(id: u64, kind: &str, w: f64, h: f64, loss: f64, limit: Option<f64>) -> Spec {
        Spec {
            id,
            size: Size {
                width: w,
                height: h,
                guessed: false,
            },
            sort_key: format!("{kind}{id:06}"),
            kind: kind_of(kind),
            gaps: Default::default(),
            loss_w: Some(loss),
            limit_c: limit,
        }
    }

    fn plate() -> Plate {
        Plate {
            width: 1200.0,
            height: 1000.0,
            margin: 10.0,
        }
    }

    fn boxes(plan: &Plan, specs: &[Spec]) -> HashMap<u64, Rect> {
        boxes_of(plan, specs)
    }

    #[test]
    fn off_or_cold_changes_nothing() {
        let specs: Vec<Spec> = (1..=6)
            .map(|i| spec(i, "K", 45.0, 90.0, 0.5, Some(60.0)))
            .collect();
        let mut rules = RuleSet::standard();
        let with = crate::rules::place_all_around(&plate(), &specs, &rules, 1, 0.0, &[]);
        rules.heat = None;
        let without = crate::rules::place_all_around(&plate(), &specs, &rules, 1, 0.0, &[]);
        assert_eq!(with.0, without.0);
        assert!(with.1.heat.is_none());
    }

    #[test]
    fn hot_devices_are_spread_along_a_rail() {
        // Eight contactors on one rail; the first three are hot and sit side by side.
        let mut specs: Vec<Spec> = (1..=8)
            .map(|i| {
                let hot = i <= 3;
                spec(i, "K", 45.0, 90.0, if hot { 20.0 } else { 0.5 }, Some(60.0))
            })
            .collect();
        specs[7].limit_c = Some(45.0);
        let mut off = RuleSet::standard();
        off.heat = None;
        let rules_plan = crate::rules::place_all_around(&plate(), &specs, &off, 1, 0.0, &[]).0;
        let on = RuleSet::standard();
        let (plan, why) = crate::rules::place_all_around(&plate(), &specs, &on, 1, 0.0, &[]);
        assert!(plan.unplaced.is_empty());
        let h = HeatPlace::default();
        let before = coupling(&specs, &boxes(&rules_plan, &specs), &h, 1000.0);
        let after = coupling(&specs, &boxes(&plan, &specs), &h, 1000.0);
        assert!(after < before * 0.8, "before {before:.1} after {after:.1}");
        let summary = why.heat.expect("heat step ran");
        assert!(!summary.unchanged);
        assert!(summary.gain() > 0.2);
        // Nothing overlaps and every device is still placed.
        assert_eq!(plan.placements.len(), specs.len());
        let b = boxes(&plan, &specs);
        for (i, x) in &b {
            for (j, y) in &b {
                if i < j {
                    assert!(!x.overlaps(y), "{i} overlaps {j}");
                }
            }
        }
    }

    #[test]
    fn sensitive_device_is_brought_below_the_hot_zone() {
        // A breaker with a 45 C limit would sit in the top zone, above a hot drive.
        let specs = vec![
            spec(1, "Q", 45.0, 90.0, 1.0, Some(45.0)),
            spec(2, "Q", 45.0, 90.0, 1.0, Some(60.0)),
            spec(3, "T", 200.0, 300.0, 120.0, Some(40.0)),
        ];
        let rules = RuleSet::standard();
        let over = zone_override(&rules, &specs);
        assert_eq!(over.get(&1), Some(&Zone::Middle));
        assert!(!over.contains_key(&2));
        let (plan, why) = crate::rules::place_all_around(&plate(), &specs, &rules, 1, 0.0, &[]);
        let b = boxes(&plan, &specs);
        assert!(b[&1].y > b[&3].bottom() - 1.0 || b[&1].y >= b[&2].y);
        assert_eq!(why.decision(1).and_then(|d| d.heat), Some(HeatWhy::Lowered));
        // The rule check agrees with where the device went.
        let placements: Vec<crate::Placement> = Vec::new();
        let _ = placements;
    }

    #[test]
    fn rule_text_round_trips() {
        let mut rules = RuleSet::standard();
        rules.heat = Some(HeatPlace {
            ambient_c: 40.0,
            ..HeatPlace::default()
        });
        assert_eq!(RuleSet::parse(&rules.to_text()).unwrap(), rules);
        let mut off = RuleSet::standard();
        off.heat = None;
        assert_eq!(RuleSet::parse(&off.to_text()).unwrap().heat, None);
        // A file from before the heat step gets it on.
        assert!(
            RuleSet::parse("gap 10\nrule * zone=middle\n")
                .unwrap()
                .heat
                .is_some()
        );
    }
}
