//! Heat-aware placement: moves the devices that make or suffer the heat to
//! other places on their rails, and keeps a move only when the model shows
//! that it wins.
//!
//! The planner of `novo-layout` places by rules and knows nothing about
//! heat; [`crate::advice`] tries one device at a time at free spots of the
//! plate without regard to rails. This search works on the rails: a device
//! only goes to a free stretch of a rail (or stays), and every move is
//! tried in the model first.
//!
//! **Search.** The movers are the devices that make heat (at least
//! [`Settings::hot_min_w`], and at least 5 % of the total) and the devices
//! whose margin to their limit is thin. For each mover, in order of loss and
//! then of thin margin, the free stretches of every rail that take it are
//! listed. A cheap plume estimate (hot air rises: a device above a hot one
//! is exposed) ranks them; the best [`Settings::top_k`] are run through the
//! 2D model on its coarse advice grid, in parallel. The best one is kept when it lowers the layout's cost (see [`Summary::cost`]) by at least
//! [`Settings::min_gain_k`] after the price of
//! the longer wires ([`Settings::wire_price_k_per_m`]); no device travels
//! farther than [`Settings::max_move_mm`]. Swaps with one
//! partner are tried the same way. After [`Settings::max_moves`] moves, or
//! when a pass finds nothing, the search ends.
//!
//! **What it is not.** A re-placement does not make heat vanish: a loss
//! that the walls cannot carry away stays a problem, and the result says so
//! by listing the cooling that is still needed. It also knows nothing about
//! wiring or the planner's zone rules: a move is a proposal that the person
//! looks at (and the rule check re-runs after accepting).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use novo_layout::Rect;

use crate::advice::{Score, score};
use crate::model::*;
use crate::result::{Advice, Recommendation};
use crate::setup::Error;
use crate::thermal::State;

/// A rail with the room a device on it has.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    /// The caller's id for the rail (echoed in the moves).
    pub id: u64,
    /// Where devices may stand along the rail, mm, interior frame.
    pub x_min: f64,
    pub x_max: f64,
    /// Top of the devices on this rail, mm.
    pub y_top: f64,
    /// Room under `y_top` before the next duct or rail row, mm.
    pub height_mm: f64,
}

/// Knobs of the search; the defaults suit a cabinet of up to a few hundred
/// devices.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Free space kept between neighbours on a rail, mm.
    pub gap_mm: f64,
    /// A device with at least this loss counts as a heat source, W.
    pub hot_min_w: f64,
    /// A device with a margin under this is a victim that may move, K.
    pub thin_margin_k: f64,
    /// Candidates run through the model per mover.
    pub top_k: usize,
    /// Least a move must lower the cost by to be kept, kelvin-devices.
    pub min_gain_k: f64,
    /// Price of wire length: cost a metre of displacement must pay for.
    pub wire_price_k_per_m: f64,
    /// Farthest a single device may travel, mm. A power supply or a main
    /// breaker at the far corner would cool well and cost a long wire.
    pub max_move_mm: f64,
    pub max_moves: usize,
    /// Movers tried per pass.
    pub max_movers: usize,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            gap_mm: 10.0,
            hot_min_w: 10.0,
            thin_margin_k: 8.0,
            top_k: 8,
            min_gain_k: 0.3,
            wire_price_k_per_m: 0.5,
            max_move_mm: 900.0,
            max_moves: 6,
            max_movers: 6,
        }
    }
}

/// One device moved.
#[derive(Debug, Clone, PartialEq)]
pub struct Move {
    /// The scenario device id.
    pub id: u64,
    pub from_row: u64,
    pub to_row: u64,
    pub from: Rect,
    pub to: Rect,
    /// Cost of the layout before and after this move.
    pub cost_before: f64,
    pub cost_after: f64,
    /// A swap partner: the device that took the vacated place.
    pub swapped_with: Option<u64>,
}

/// What the model says about one layout.
#[derive(Debug, Clone, PartialEq)]
pub struct Summary {
    /// Smallest margin to a limit, K (negative = over); `None` without limits.
    pub worst_margin_k: Option<f64>,
    /// The search's cost of the layout, kelvin-devices (lower is better).
    pub cost: f64,
    /// The 2D run for this layout met its tolerance; when false it wobbled
    /// (about 1 K) and small differences mean nothing.
    pub settled: bool,
    pub max_air_c: f64,
    pub devices_over: usize,
    /// Margin per device in scenario order.
    pub margins: Vec<Option<f64>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Suggestion {
    pub moves: Vec<Move>,
    pub before: Summary,
    pub after: Summary,
    /// The scenario with the moves applied.
    pub scenario: Scenario,
    /// Layouts run through the model.
    pub evaluated: usize,
    /// Total displacement of the moved devices, mm.
    pub moved_mm: f64,
    /// Sentences for the person: what moved and what is still not solved.
    pub notes: Vec<String>,
    /// Cooling tried on the moved layout (fan pair, vents, a cooler
    /// device), best first; only when a device is still over its limit.
    pub cooling: Vec<Recommendation>,
}

/// Margin below which a device counts as thin, K.
const THIN_K: f64 = 8.0;
/// Smoothing of the penalty steps, K.
const SMOOTH_K: f64 = 1.0;

fn softplus(x: f64) -> f64 {
    SMOOTH_K * (x / SMOOTH_K).exp().ln_1p()
}

/// What a layout costs, in kelvin-devices: every device's shortfall under
/// its limit, a fifth of every device's shortfall under a margin of
/// [`THIN_K`], and a small price for hot air. Lower is better. A sum, not a
/// worst case: a device that placement cannot save must not hide that the
/// others can be.
fn cost(margins: &[Option<f64>], max_air: f64, ambient: f64) -> f64 {
    let devices: f64 = margins
        .iter()
        .flatten()
        .map(|m| softplus(-m) + 0.2 * softplus(THIN_K - m))
        .sum();
    devices + 0.05 * (max_air - ambient).max(0.0)
}

fn summary(s: &Score, ambient: f64, settled: bool) -> Summary {
    Summary {
        worst_margin_k: s.worst_margin,
        cost: cost(&s.margins, s.max_air, ambient),
        settled,
        max_air_c: s.max_air,
        devices_over: s.margins.iter().flatten().filter(|m| **m < 0.0).count(),
        margins: s.margins.clone(),
    }
}

/// The scenario plus where each device stands: `row[i]` is the index into
/// `rows` of scenario device `i`, `None` for a device that is not on a rail
/// (it stays where it is).
struct Layout<'a> {
    scn: Scenario,
    rows: &'a [Row],
    row: Vec<Option<usize>>,
}

impl Layout<'_> {
    /// Free stretches of row `r` as `(from, to)` x ranges when device
    /// `skip` (and `skip2`) are not there.
    fn free_stretches(&self, r: usize, skip: &[usize], gap: f64) -> Vec<(f64, f64)> {
        let row = &self.rows[r];
        let mut taken: Vec<(f64, f64)> = (0..self.scn.devices.len())
            .filter(|i| self.row[*i] == Some(r) && !skip.contains(i))
            .map(|i| {
                let f = self.scn.devices[i].footprint;
                (f.x - gap, f.right() + gap)
            })
            .collect();
        taken.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut out = Vec::new();
        let mut at = row.x_min;
        for (a, b) in taken {
            if a > at {
                out.push((at, a.min(row.x_max)));
            }
            at = at.max(b);
        }
        if at < row.x_max {
            out.push((at, row.x_max));
        }
        out
    }
}

/// Where a hot device's plume goes: heat per unit exposure at `target` from
/// a source of `loss_w` standing at `src`. Air rises, so only things above
/// the source (and, weakly, beside it) are exposed.
fn exposure(src: &Rect, loss_w: f64, target: &Rect) -> f64 {
    let dx = (target.x + target.w / 2.0 - src.x - src.w / 2.0).abs();
    let rise = src.y - target.bottom(); // mm the target stands above the source's top
    if rise >= 0.0 {
        let spread = src.w / 2.0 + 0.3 * rise + 40.0;
        loss_w * (-(rise / 700.0)).exp() * (-(dx / spread).powi(2)).exp()
    } else {
        // Beside or below: heat spreads through the plate and the air nearby.
        let d = src.gap(target);
        0.15 * loss_w * (-(d / 150.0)).exp()
    }
}

/// Searches a cooler placement; see the module text.
pub fn suggest(
    scn: &Scenario,
    rows: &[Row],
    row_of: &[Option<usize>],
    settings: &Settings,
) -> Result<Suggestion, Error> {
    suggest_with(scn, rows, row_of, settings, &Control::default())
}

/// What a caller can do while a search runs.
#[derive(Debug, Default)]
pub struct Control {
    /// Set to true to end the search with the best layout found so far.
    pub cancel: AtomicBool,
    /// Layouts run through the model so far.
    pub evaluated: AtomicUsize,
}

/// [`suggest`] that can be stopped and watched.
pub fn suggest_with(
    scn: &Scenario,
    rows: &[Row],
    row_of: &[Option<usize>],
    settings: &Settings,
    control: &Control,
) -> Result<Suggestion, Error> {
    assert_eq!(row_of.len(), scn.devices.len(), "one row entry per device");
    let (base_score, base_state) = score(scn, None)?;
    let before = summary(&base_score, scn.ambient_c, base_state.converged);
    let total: f64 = scn.devices.iter().map(|d| d.loss_w).sum();
    let hot_min = settings.hot_min_w.max(0.05 * total);

    let mut cur = Layout {
        scn: scn.clone(),
        rows,
        row: row_of.to_vec(),
    };
    let mut cur_score = (
        summary(&base_score, scn.ambient_c, base_state.converged),
        base_score,
    );
    let mut state = base_state;
    let mut moves: Vec<Move> = Vec::new();
    let mut evaluated = 1usize;
    let mut moved: Vec<bool> = vec![false; scn.devices.len()];

    for _pass in 0..3 {
        if moves.len() >= settings.max_moves {
            break;
        }
        let mut movers: Vec<usize> = (0..cur.scn.devices.len())
            .filter(|i| cur.row[*i].is_some() && !moved[*i])
            .filter(|i| {
                let d = &cur.scn.devices[*i];
                let thin = cur_score.0.margins[*i].is_some_and(|m| m < settings.thin_margin_k);
                d.loss_w >= hot_min || thin
            })
            .collect();
        movers.sort_by(|a, b| {
            let (da, db) = (&cur.scn.devices[*a], &cur.scn.devices[*b]);
            db.loss_w.total_cmp(&da.loss_w).then(
                cur_score.0.margins[*a]
                    .unwrap_or(f64::MAX)
                    .total_cmp(&cur_score.0.margins[*b].unwrap_or(f64::MAX)),
            )
        });
        movers.truncate(settings.max_movers);
        let mut progress = false;
        for m in movers {
            if moves.len() >= settings.max_moves || control.cancel.load(Ordering::Relaxed) {
                break;
            }
            let cands = candidates(&cur, m, &cur_score.0, settings);
            if cands.is_empty() {
                continue;
            }
            let scored: Vec<Result<(Score, State), Error>> =
                crate::par_map(&cands, |c| score(&c.scn, Some(&state)));
            evaluated += scored.len();
            control.evaluated.store(evaluated, Ordering::Relaxed);
            let mut best: Option<(f64, usize, Score, State)> = None; // lowest cost
            for (k, s) in scored.into_iter().enumerate() {
                let (s, st) = s?;
                let c = cost(&s.margins, s.max_air, scn.ambient_c)
                    + settings.wire_price_k_per_m * cands[k].travel_mm / 1000.0;
                if best.as_ref().is_none_or(|b| c < b.0) {
                    best = Some((c, k, s, st));
                }
            }
            let Some((priced, k, s, st)) = best else {
                continue;
            };
            if cur_score.0.cost - priced < settings.min_gain_k {
                continue;
            }
            let c = &cands[k];
            let after = summary(&s, scn.ambient_c, st.converged);
            moves.push(Move {
                id: cur.scn.devices[m].id,
                from_row: rows[cur.row[m].expect("mover is on a row")].id,
                to_row: rows[c.row].id,
                from: cur.scn.devices[m].footprint,
                to: c.scn.devices[m].footprint,
                cost_before: cur_score.0.cost,
                cost_after: after.cost,
                swapped_with: c.partner.map(|p| cur.scn.devices[p].id),
            });
            if let (Some(p), Some(pr)) = (c.partner, c.partner_row) {
                moves.push(Move {
                    id: cur.scn.devices[p].id,
                    from_row: rows[cur.row[p].expect("partner is on a row")].id,
                    to_row: rows[pr].id,
                    from: cur.scn.devices[p].footprint,
                    to: c.scn.devices[p].footprint,
                    cost_before: cur_score.0.cost,
                    cost_after: after.cost,
                    swapped_with: Some(cur.scn.devices[m].id),
                });
            }
            moved[m] = true;
            if let Some(p) = c.partner {
                moved[p] = true;
            }
            cur.row[m] = Some(c.row);
            if let Some(p) = c.partner {
                cur.row[p] = c.partner_row;
            }
            cur.scn = c.scn.clone();
            cur_score = (after, s);
            state = st;
            progress = true;
        }
        if !progress || control.cancel.load(Ordering::Relaxed) {
            break;
        }
    }

    let after = cur_score.0;
    let moved_mm = moves
        .iter()
        .map(|m| (m.to.x - m.from.x).hypot(m.to.y - m.from.y))
        .sum();
    let mut notes = Vec::new();
    if moves.is_empty() {
        notes.push(
            "No move on the rails wins enough in the model. The heat has to be taken out some other way (fan, vents, a larger enclosure, a cooler place for the machine)."
                .to_owned(),
        );
    }
    if !moves.is_empty() && !(after.settled && before.settled) {
        notes.push(
            "The 2D model had not fully settled for one of the layouts; it wobbles by about 1 K there, so read differences under 1 K as no difference."
                .to_owned(),
        );
    }
    if after.devices_over > 0 {
        notes.push(format!(
            "{} device(s) are still over their limit after the moves: placement alone does not carry the heat away.",
            after.devices_over
        ));
    }
    let mut cooling = Vec::new();
    if after.devices_over > 0 && !control.cancel.load(Ordering::Relaxed) {
        let tried = crate::simulate_steady_with_advice(&cur.scn)?;
        cooling = tried
            .recommendations
            .into_iter()
            .filter(|r| !matches!(r.advice, Advice::MoveDevice { .. }))
            .collect();
    }
    Ok(Suggestion {
        cooling,
        moves,
        before,
        after,
        scenario: cur.scn,
        evaluated,
        moved_mm,
        notes,
    })
}

struct Candidate {
    scn: Scenario,
    row: usize,
    partner: Option<usize>,
    partner_row: Option<usize>,
    /// How far the moved devices travel, mm.
    travel_mm: f64,
    /// Plume estimate, lower is better (a ranking only; the model decides).
    rank: f64,
}

/// Relocations and swaps of device `m`, the best `top_k` by plume estimate.
fn candidates(cur: &Layout<'_>, m: usize, score: &Summary, settings: &Settings) -> Vec<Candidate> {
    let scn = &cur.scn;
    let dev = &scn.devices[m];
    let (w, h) = (dev.footprint.w, dev.footprint.h);
    let sens = |i: usize| -> f64 {
        // Thin or unknown limits make a device sensitive to warm air.
        match score.margins[i] {
            Some(mg) => (-(mg) / 10.0).exp().min(20.0),
            None => 0.05,
        }
    };
    let hot: Vec<usize> = (0..scn.devices.len())
        .filter(|i| scn.devices[*i].loss_w >= 5.0)
        .collect();
    let is_source = dev.loss_w >= 5.0;
    let mut out: Vec<Candidate> = Vec::new();

    for r in 0..cur.rows.len() {
        let row = &cur.rows[r];
        if row.height_mm + 1e-9 < h {
            continue;
        }
        // Relocation into free stretches.
        for (a, b) in cur.free_stretches(r, &[m], settings.gap_mm) {
            if b - a + 1e-9 < w {
                continue;
            }
            let mut xs = vec![a, b - w, (a + b - w) / 2.0];
            // Samples along long stretches.
            let mut x = a + 150.0;
            while x + w < b {
                xs.push(x);
                x += 150.0;
            }
            for x in xs {
                let rect = Rect {
                    x: x.max(a),
                    y: row.y_top,
                    w,
                    h,
                };
                if rect.right() > b + 1e-9 {
                    continue;
                }
                if (rect.x - dev.footprint.x).abs() < 1.0 && (rect.y - dev.footprint.y).abs() < 1.0
                {
                    continue;
                }
                let travel = (rect.x - dev.footprint.x).hypot(rect.y - dev.footprint.y);
                if travel > settings.max_move_mm {
                    continue;
                }
                // Plume estimate: what this device exposes (as a source) or
                // what is exposed to the sources (as a victim).
                let mut rank = 0.0;
                if is_source {
                    for j in 0..scn.devices.len() {
                        if j != m {
                            rank +=
                                sens(j) * exposure(&rect, dev.loss_w, &scn.devices[j].footprint);
                        }
                    }
                }
                let me = sens(m);
                for &s in &hot {
                    if s != m {
                        rank +=
                            me * exposure(&scn.devices[s].footprint, scn.devices[s].loss_w, &rect);
                    }
                }
                let mut moved = scn.clone();
                moved.devices[m].footprint = rect;
                out.push(Candidate {
                    scn: moved,
                    row: r,
                    partner: None,
                    partner_row: None,
                    travel_mm: travel,
                    rank,
                });
            }
        }
        // Swap with a device standing on this row, if both then fit.
        for p in 0..scn.devices.len() {
            if p == m || cur.row[p] != Some(r) {
                continue;
            }
            let (Some(rm), Some(rp)) = (cur.row[m], cur.row[p]) else {
                continue;
            };
            if rm == rp {
                continue;
            }
            let (fm, fp) = (dev.footprint, scn.devices[p].footprint);
            if fp.h > cur.rows[rm].height_mm + 1e-9 || h > cur.rows[rp].height_mm + 1e-9 {
                continue;
            }
            // Each takes the other's place, centred on the old centre.
            let nm = Rect {
                x: fp.x + fp.w / 2.0 - w / 2.0,
                y: cur.rows[rp].y_top,
                w,
                h,
            };
            let np = Rect {
                x: fm.x + fm.w / 2.0 - fp.w / 2.0,
                y: cur.rows[rm].y_top,
                w: fp.w,
                h: fp.h,
            };
            let fits = |rect: &Rect, row: usize, skip: &[usize]| {
                rect.x >= cur.rows[row].x_min - 1e-9
                    && rect.right() <= cur.rows[row].x_max + 1e-9
                    && cur
                        .free_stretches(row, skip, settings.gap_mm)
                        .iter()
                        .any(|(a, b)| rect.x >= a - 1e-9 && rect.right() <= b + 1e-9)
            };
            if !(fits(&nm, rp, &[m, p]) && fits(&np, rm, &[m, p])) {
                continue;
            }
            let travel = (nm.x - fm.x).hypot(nm.y - fm.y) + (np.x - fp.x).hypot(np.y - fp.y);
            if (nm.x - fm.x).hypot(nm.y - fm.y) > settings.max_move_mm
                || (np.x - fp.x).hypot(np.y - fp.y) > settings.max_move_mm
            {
                continue;
            }
            let mut rank = 0.0;
            for j in 0..scn.devices.len() {
                if j != m && j != p {
                    if is_source {
                        rank += sens(j) * exposure(&nm, dev.loss_w, &scn.devices[j].footprint);
                    }
                    if scn.devices[p].loss_w >= 5.0 {
                        rank += sens(j)
                            * exposure(&np, scn.devices[p].loss_w, &scn.devices[j].footprint);
                    }
                }
            }
            let mut moved = scn.clone();
            moved.devices[m].footprint = nm;
            moved.devices[p].footprint = np;
            out.push(Candidate {
                scn: moved,
                row: rp,
                partner: Some(p),
                partner_row: Some(rm),
                travel_mm: travel,
                rank,
            });
        }
    }
    out.sort_by(|a, b| a.rank.total_cmp(&b.rank));
    // Keep the best by plume estimate; the model has the last word.
    out.truncate(settings.top_k);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::sizing::{Installation, Material};

    fn dev(id: u64, x: f64, y: f64, w: f64, h: f64, loss: f64, limit: Option<f64>) -> Device {
        Device {
            id,
            footprint: Rect { x, y, w, h },
            height_mm: Some(80.0),
            loss_w: loss,
            rated_max_ambient_c: limit,
            rated_max_surface_c: None,
            heat_capacity_j_per_k: None,
        }
    }

    fn scenario(devices: Vec<Device>) -> Scenario {
        Scenario {
            enclosure: Enclosure {
                width_mm: 904.0,
                height_mm: 1204.0,
                depth_mm: 304.0,
                wall_mm: 2.0,
                material: Material::Steel,
                installation: Installation::Wall,
                plate: Rect {
                    x: 50.0,
                    y: 50.0,
                    w: 800.0,
                    h: 1100.0,
                },
            },
            ambient_c: 35.0,
            devices,
            obstacles: Vec::new(),
            vents: Vec::new(),
            fans: Vec::new(),
            params: Params::default(),
        }
    }

    fn rows() -> Vec<Row> {
        // Three rails, 300 mm of room each.
        (0..3)
            .map(|i| Row {
                id: 100 + i,
                x_min: 60.0,
                x_max: 840.0,
                y_top: 60.0 + 350.0 * i as f64,
                height_mm: 300.0,
            })
            .collect()
    }

    #[test]
    fn cost_counts_every_shortfall_and_ignores_comfortable_margins() {
        let one = cost(&[Some(-10.0)], 35.0, 35.0);
        let two = cost(&[Some(-10.0), Some(-4.0)], 35.0, 35.0);
        assert!(two > one + 3.5, "{one} {two}");
        // A device far under its limit adds next to nothing.
        let cool = cost(&[Some(-10.0), Some(40.0)], 35.0, 35.0);
        assert!((cool - one).abs() < 0.01, "{one} {cool}");
        // Hot air costs a little.
        assert!(cost(&[], 55.0, 35.0) > cost(&[], 40.0, 35.0));
    }

    #[test]
    fn plume_hits_what_stands_above_not_below() {
        let src = Rect {
            x: 100.0,
            y: 500.0,
            w: 100.0,
            h: 200.0,
        };
        let above = Rect {
            x: 110.0,
            y: 300.0,
            w: 50.0,
            h: 90.0,
        };
        let below = Rect {
            x: 110.0,
            y: 750.0,
            w: 50.0,
            h: 90.0,
        };
        let aside = Rect {
            x: 600.0,
            y: 300.0,
            w: 50.0,
            h: 90.0,
        };
        let a = exposure(&src, 100.0, &above);
        assert!(a > 5.0 * exposure(&src, 100.0, &below));
        assert!(a > 20.0 * exposure(&src, 100.0, &aside));
    }

    #[test]
    fn a_hot_device_under_a_sensitive_one_moves_aside_and_the_margin_grows() {
        // A 120 W heater-like drive on the lower row, a 60 C-limit device
        // right above it in the middle row; the right side of the lower row
        // is free.
        let scn = scenario(vec![
            dev(
                1,
                100.0,
                410.0 + 700.0 - 410.0,
                120.0,
                200.0,
                120.0,
                Some(40.0),
            ),
            dev(2, 110.0, 410.0, 50.0, 90.0, 4.0, Some(45.0)),
        ]);
        // device 1 sits on row 2 (y 760), device 2 on row 1 (y 410).
        let mut scn = scn;
        scn.devices[0].footprint.y = 760.0;
        let rows = rows();
        let row_of = vec![Some(2), Some(1)];
        let s = suggest(&scn, &rows, &row_of, &Settings::default()).unwrap();
        assert!(s.evaluated > 3);
        assert!(!s.moves.is_empty(), "{:?}", s.notes);
        assert!(
            s.after.cost < s.before.cost - 0.3,
            "{:?} -> {:?}",
            s.before.cost,
            s.after.cost
        );
        // Everything stays on a rail, inside the row, without overlap.
        for m in &s.moves {
            let r = rows.iter().find(|r| r.id == m.to_row).unwrap();
            assert!(m.to.x >= r.x_min - 1e-6 && m.to.right() <= r.x_max + 1e-6);
            assert!((m.to.y - r.y_top).abs() < 1e-6);
        }
        let d = &s.scenario.devices;
        assert!(!d[0].footprint.overlaps(&d[1].footprint));
    }

    #[test]
    fn no_device_travels_farther_than_the_limit() {
        let mut scn = scenario(vec![
            dev(1, 100.0, 760.0, 120.0, 200.0, 120.0, Some(40.0)),
            dev(2, 110.0, 410.0, 50.0, 90.0, 4.0, Some(45.0)),
        ]);
        scn.devices[0].footprint.y = 760.0;
        let near = Settings {
            max_move_mm: 150.0,
            ..Settings::default()
        };
        let s = suggest(&scn, &rows(), &[Some(2), Some(1)], &near).unwrap();
        for m in &s.moves {
            let d = (m.to.x - m.from.x).hypot(m.to.y - m.from.y);
            assert!(d <= 150.0 + 1e-6, "{d}");
        }
    }

    #[test]
    fn nothing_hot_nothing_moves() {
        let scn = scenario(vec![
            dev(1, 100.0, 60.0, 50.0, 90.0, 1.0, Some(60.0)),
            dev(2, 200.0, 60.0, 50.0, 90.0, 1.0, Some(60.0)),
        ]);
        let s = suggest(&scn, &rows(), &[Some(0), Some(0)], &Settings::default()).unwrap();
        assert!(s.moves.is_empty());
        assert_eq!(s.before.cost, s.after.cost);
        assert!(!s.notes.is_empty());
    }

    #[test]
    fn a_device_too_tall_for_a_row_is_not_offered_that_row() {
        let mut scn = scenario(vec![dev(1, 100.0, 60.0, 100.0, 280.0, 150.0, Some(40.0))]);
        scn.devices[0].footprint.y = 60.0;
        let mut r = rows();
        r[1].height_mm = 100.0;
        r[2].height_mm = 100.0;
        let s = suggest(&scn, &r, &[Some(0)], &Settings::default()).unwrap();
        for m in &s.moves {
            assert_eq!(m.to_row, r[0].id);
        }
    }
}
