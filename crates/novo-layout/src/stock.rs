//! The cut list for rails and ducts: how many bars of stock to buy, which
//! pieces to cut from which bar, and what is left over.
//!
//! The pieces come from a layout: each DIN rail needs at least the length its
//! devices use (plus an end allowance, rounded up to a whole 5 mm) and may be
//! as long as the plate allows; each duct has one length. [`optimise`] finds
//! the fewest bars with an exact search (a branch and bound over a
//! best-fit-decreasing start), then gives each bar's slack to the rails on
//! it, up to their full length, so a rail is only shorter than the plate when
//! that saves a bar. A piece longer than a bar is cut from whole bars plus
//! one remainder and marked as spliced.
//!
//! Millimetres. The saw takes `kerf` from every cut between two pieces.

use crate::Placement;
use crate::Rail;

/// End allowance kept on a DIN rail beyond its last device.
pub const RAIL_END_MM: f64 = 10.0;
/// Rails are cut in steps of this length.
pub const CUT_STEP_MM: f64 = 5.0;
/// Standard stock bar length for DIN rail and ducts.
pub const STOCK_LENGTH_MM: f64 = 2000.0;
/// Saw kerf per cut.
pub const KERF_MM: f64 = 2.0;
/// An offcut shorter than this is scrap, longer is kept for reuse.
pub const MIN_OFFCUT_MM: f64 = 100.0;
/// The exact search gives up after this many nodes and keeps its best answer.
const SEARCH_LIMIT: u64 = 400_000;

/// The bars the pieces are cut from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stock {
    pub length: f64,
    pub kerf: f64,
    pub min_offcut: f64,
}

impl Default for Stock {
    fn default() -> Self {
        Stock {
            length: STOCK_LENGTH_MM,
            kerf: KERF_MM,
            min_offcut: MIN_OFFCUT_MM,
        }
    }
}

/// One piece to cut: at least `min` long, at most `max` (equal for a duct).
#[derive(Debug, Clone, PartialEq)]
pub struct Piece {
    /// The caller's id (the rail or duct id).
    pub id: u64,
    pub min: f64,
    pub max: f64,
}

/// A piece on a bar, with its final length.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub id: u64,
    pub length: f64,
    /// For a piece longer than a bar: which part this is and how many parts
    /// there are (`(1, 2)` is the first of two). `None` for a whole piece.
    pub part: Option<(usize, usize)>,
}

/// One bar of stock and what is cut from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub cuts: Vec<Cut>,
    /// Length left after the cuts and the kerf.
    pub left: f64,
}

impl Bar {
    /// True when the leftover is long enough to keep.
    pub fn offcut_kept(&self, stock: &Stock) -> bool {
        self.left >= stock.min_offcut
    }

    /// One line for the workshop: `Rail 3 1000, Rail 4 940; 58 left (scrap)`.
    pub fn line(&self, stock: &Stock, label: &dyn Fn(u64) -> String) -> String {
        let cuts: Vec<String> = self
            .cuts
            .iter()
            .map(|c| match c.part {
                Some((n, of)) => format!("{} {:.0} (part {n} of {of})", label(c.id), c.length),
                None => format!("{} {:.0}", label(c.id), c.length),
            })
            .collect();
        let end = if self.left <= 0.0 {
            String::new()
        } else if self.offcut_kept(stock) {
            format!("; {:.0} left, keep", self.left)
        } else {
            format!("; {:.0} left (scrap)", self.left)
        };
        format!("{}{end}", cuts.join(", "))
    }
}

/// The answer for one kind of stock.
#[derive(Debug, Clone, PartialEq)]
pub struct CutPlan {
    pub stock: Stock,
    pub bars: Vec<Bar>,
    /// The final length chosen for every piece id (spliced pieces: the sum).
    pub lengths: Vec<(u64, f64)>,
    /// The fewest bars any plan could use (total length over bar length).
    pub lower_bound: usize,
    /// True when the search ran out of budget and the bar count may not be
    /// the smallest possible.
    pub budget_hit: bool,
}

impl CutPlan {
    pub fn bars_needed(&self) -> usize {
        self.bars.len()
    }

    /// Length of offcuts shorter than the minimum, thrown away.
    pub fn scrap(&self) -> f64 {
        self.bars
            .iter()
            .filter(|b| !b.offcut_kept(&self.stock))
            .map(|b| b.left)
            .sum()
    }

    /// Length of offcuts long enough to keep.
    pub fn reusable(&self) -> f64 {
        self.bars
            .iter()
            .filter(|b| b.offcut_kept(&self.stock))
            .map(|b| b.left)
            .sum()
    }
}

/// Rounds up to a whole cut step.
pub fn round_up(length: f64) -> f64 {
    (length / CUT_STEP_MM - 1e-9).ceil() * CUT_STEP_MM
}

/// Pieces for the DIN rails of a layout: at least what the devices on the
/// rail use plus the end allowance (rounded up), at most the rail's length
/// in the layout. A rail without devices is a duct-like fixed piece.
pub fn rail_pieces(rails: &[Rail], placements: &[Placement]) -> Vec<Piece> {
    rails
        .iter()
        .map(|r| {
            let on: Vec<&Placement> = placements.iter().filter(|p| p.rail == Some(r.id)).collect();
            let used = if on.is_empty() {
                r.length
            } else if r.vertical {
                let top = on
                    .iter()
                    .map(|p| p.footprint().y)
                    .fold(f64::INFINITY, f64::min);
                let end = on
                    .iter()
                    .map(|p| p.footprint().bottom())
                    .fold(0.0, f64::max);
                (end - top).max(0.0)
            } else {
                let left = on
                    .iter()
                    .map(|p| p.footprint().x)
                    .fold(f64::INFINITY, f64::min);
                let end = on.iter().map(|p| p.footprint().right()).fold(0.0, f64::max);
                (end - left).max(0.0)
            };
            let min = round_up(used + 2.0 * RAIL_END_MM).min(r.length);
            Piece {
                id: r.id,
                min,
                max: r.length,
            }
        })
        .collect()
}

/// Packs the pieces into the fewest bars of `stock`.
pub fn optimise(pieces: &[Piece], stock: Stock) -> CutPlan {
    let l = stock.length;
    // Split pieces longer than a bar into whole bars and a remainder.
    struct Item {
        id: u64,
        min: f64,
        max: f64,
        part: Option<(usize, usize)>,
    }
    let mut items: Vec<Item> = Vec::new();
    let mut bars: Vec<Bar> = Vec::new();
    for p in pieces {
        let (min, max) = (p.min.max(0.0), p.max.max(p.min.max(0.0)));
        if max <= l + 1e-9 {
            items.push(Item {
                id: p.id,
                min,
                max,
                part: None,
            });
            continue;
        }
        let whole = (max / l).floor() as usize;
        let rest = max - whole as f64 * l;
        let parts = whole + usize::from(rest > 1e-9);
        for n in 0..whole {
            bars.push(Bar {
                cuts: vec![Cut {
                    id: p.id,
                    length: l,
                    part: Some((n + 1, parts)),
                }],
                left: 0.0,
            });
        }
        if rest > 1e-9 {
            items.push(Item {
                id: p.id,
                min: rest,
                max: rest,
                part: Some((parts, parts)),
            });
        }
    }

    let sizes: Vec<f64> = items.iter().map(|i| i.min).collect();
    let (assign, bar_count, budget_hit) = pack(&sizes, l, stock.kerf);

    // Cuts per bar, longest first, then the slack goes to the pieces that may grow.
    let mut packed: Vec<Vec<usize>> = vec![Vec::new(); bar_count];
    for (item, bar) in assign.iter().enumerate() {
        packed[*bar].push(item);
    }
    let mut lengths: Vec<f64> = sizes.clone();
    for members in &packed {
        let used: f64 = members.iter().map(|&i| lengths[i]).sum::<f64>()
            + stock.kerf * members.len().saturating_sub(1) as f64;
        let mut slack = (l - used).max(0.0);
        // Grow in order of how much room each piece can take, rails first.
        let mut order = members.clone();
        order.sort_by(|&a, &b| {
            let room = |i: usize| items[i].max - lengths[i];
            room(b).total_cmp(&room(a))
        });
        for i in order {
            let grow = (items[i].max - lengths[i]).min(slack).max(0.0);
            // Whole cut steps only, unless it reaches the full length.
            let grow = if (items[i].max - lengths[i] - grow).abs() < 1e-9 {
                grow
            } else {
                (grow / CUT_STEP_MM).floor() * CUT_STEP_MM
            };
            lengths[i] += grow;
            slack -= grow;
        }
    }
    let left_after = |used: f64| {
        // What remains after the pieces; taking an offcut off the bar costs one more kerf.
        let r = (l - used).max(0.0);
        if r > stock.kerf { r - stock.kerf } else { r }
    };
    for members in packed {
        let mut cuts: Vec<Cut> = members
            .iter()
            .map(|&i| Cut {
                id: items[i].id,
                length: lengths[i],
                part: items[i].part,
            })
            .collect();
        cuts.sort_by(|a, b| b.length.total_cmp(&a.length).then(a.id.cmp(&b.id)));
        let used: f64 = cuts.iter().map(|c| c.length).sum::<f64>()
            + stock.kerf * cuts.len().saturating_sub(1) as f64;
        bars.push(Bar {
            cuts,
            left: left_after(used),
        });
    }
    bars.sort_by(|a, b| {
        let key = |bar: &Bar| bar.cuts.first().map_or(u64::MAX, |c| c.id);
        key(a).cmp(&key(b))
    });

    let mut totals: Vec<(u64, f64)> = Vec::new();
    for bar in &bars {
        for c in &bar.cuts {
            match totals.iter_mut().find(|(id, _)| *id == c.id) {
                Some((_, sum)) => *sum += c.length,
                None => totals.push((c.id, c.length)),
            }
        }
    }
    totals.sort_by_key(|t| t.0);
    let whole_bars = bars
        .iter()
        .filter(|b| b.cuts.iter().all(|c| c.length >= l))
        .count();
    let rest: f64 = sizes.iter().map(|s| s + stock.kerf).sum();
    let lower_bound = whole_bars
        + if l > 0.0 {
            ((rest / (l + stock.kerf)) - 1e-9).ceil().max(0.0) as usize
        } else {
            0
        };
    CutPlan {
        stock,
        bars,
        lengths: totals,
        lower_bound,
        budget_hit,
    }
}

/// Bin packing with a kerf: assignment of each size to a bar, the number of
/// bars, and whether the search budget ran out.
fn pack(sizes: &[f64], cap: f64, kerf: f64) -> (Vec<usize>, usize, bool) {
    let n = sizes.len();
    if n == 0 {
        return (Vec::new(), 0, false);
    }
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| sizes[b].total_cmp(&sizes[a]).then(a.cmp(&b)));
    // A piece needs its length plus one kerf, except the last on a bar,
    // which we count by allowing the bar one extra kerf of room.
    let room = cap + kerf;
    let need = |i: usize| sizes[i] + kerf;

    // Best-fit decreasing start.
    let mut best_assign = vec![0usize; n];
    let mut loads: Vec<f64> = Vec::new();
    for &i in &order {
        let fit = loads
            .iter()
            .enumerate()
            .filter(|(_, load)| **load + need(i) <= room + 1e-9)
            .max_by(|a, b| a.1.total_cmp(b.1))
            .map(|(b, _)| b);
        let b = fit.unwrap_or_else(|| {
            loads.push(0.0);
            loads.len() - 1
        });
        loads[b] += need(i);
        best_assign[i] = b;
    }
    let mut best = loads.len();
    let total: f64 = order.iter().map(|&i| need(i)).sum();
    let bound = ((total / room) - 1e-9).ceil() as usize;
    if best <= bound.max(1) {
        return (best_assign, best, false);
    }

    // Exact search: place pieces longest first, into an existing bar or a new one.
    struct Search<'a> {
        order: &'a [usize],
        need: &'a dyn Fn(usize) -> f64,
        room: f64,
        loads: Vec<f64>,
        assign: Vec<usize>,
        best: usize,
        best_assign: Vec<usize>,
        bound: usize,
        nodes: u64,
        hit: bool,
    }
    impl Search<'_> {
        fn go(&mut self, k: usize) {
            if self.hit || self.best <= self.bound {
                return;
            }
            self.nodes += 1;
            if self.nodes > SEARCH_LIMIT {
                self.hit = true;
                return;
            }
            if k == self.order.len() {
                if self.loads.len() < self.best {
                    self.best = self.loads.len();
                    self.best_assign = self.assign.clone();
                }
                return;
            }
            let i = self.order[k];
            let w = (self.need)(i);
            let mut tried: Vec<f64> = Vec::new();
            for b in 0..self.loads.len() {
                let load = self.loads[b];
                if load + w > self.room + 1e-9 || tried.iter().any(|t| (t - load).abs() < 1e-9) {
                    continue;
                }
                tried.push(load);
                self.loads[b] += w;
                self.assign[i] = b;
                self.go(k + 1);
                self.loads[b] -= w;
            }
            if self.loads.len() + 1 < self.best {
                self.loads.push(w);
                self.assign[i] = self.loads.len() - 1;
                self.go(k + 1);
                self.loads.pop();
            }
        }
    }
    let mut search = Search {
        order: &order,
        need: &need,
        room,
        loads: Vec::new(),
        assign: vec![0; n],
        best,
        best_assign: best_assign.clone(),
        bound: bound.max(1),
        nodes: 0,
        hit: false,
    };
    search.go(0);
    best = search.best;
    (search.best_assign, best, search.hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn piece(id: u64, len: f64) -> Piece {
        Piece {
            id,
            min: len,
            max: len,
        }
    }

    fn stock(len: f64) -> Stock {
        Stock {
            length: len,
            kerf: 0.0,
            min_offcut: 100.0,
        }
    }

    #[test]
    fn perfect_pairs_fill_the_bars() {
        let pieces: Vec<Piece> = [6.0, 4.0, 5.0, 5.0, 7.0, 3.0]
            .iter()
            .enumerate()
            .map(|(i, l)| piece(i as u64, l * 100.0))
            .collect();
        let plan = optimise(&pieces, stock(1000.0));
        assert_eq!(plan.bars_needed(), 3);
        assert!(plan.bars.iter().all(|b| b.left == 0.0));
    }

    #[test]
    fn the_exact_search_beats_best_fit_decreasing() {
        // Best-fit decreasing needs four bars of 1000 for these; three is possible
        // (470+300+210+..., 420+300+280, 390+290+240 plus the rest).
        let lens = [
            290.0, 390.0, 280.0, 240.0, 420.0, 210.0, 470.0, 300.0, 300.0,
        ];
        let pieces: Vec<Piece> = lens
            .iter()
            .enumerate()
            .map(|(i, l)| piece(i as u64, *l))
            .collect();
        let plan = optimise(&pieces, stock(1000.0));
        assert_eq!(plan.bars_needed(), 3);
        assert!(!plan.budget_hit);
        for bar in &plan.bars {
            assert!(bar.cuts.iter().map(|c| c.length).sum::<f64>() <= 1000.0);
        }
    }

    #[test]
    fn slack_goes_to_rails_up_to_their_full_length() {
        let pieces = vec![
            Piece {
                id: 1,
                min: 300.0,
                max: 800.0,
            },
            Piece {
                id: 2,
                min: 300.0,
                max: 800.0,
            },
        ];
        let plan = optimise(&pieces, stock(2000.0));
        assert_eq!(plan.bars_needed(), 1);
        let lengths: Vec<f64> = plan.lengths.iter().map(|l| l.1).collect();
        assert_eq!(lengths, vec![800.0, 800.0]);
        assert_eq!(plan.bars[0].left, 400.0);
    }

    #[test]
    fn rails_stay_short_when_full_length_would_cost_a_bar() {
        // Three rails of 500..800: full length needs 2400 (two bars); at
        // the minimum they need 1500 (one bar), so they stay short.
        let pieces: Vec<Piece> = (1..=3)
            .map(|id| Piece {
                id,
                min: 500.0,
                max: 800.0,
            })
            .collect();
        let plan = optimise(&pieces, stock(2000.0));
        assert_eq!(plan.bars_needed(), 1);
        let sum: f64 = plan.lengths.iter().map(|l| l.1).sum();
        assert!(sum <= 2000.0);
        assert!(plan.lengths.iter().all(|l| l.1 >= 500.0 && l.1 <= 800.0));
    }

    #[test]
    fn the_kerf_is_paid_between_pieces() {
        let s = Stock {
            length: 1000.0,
            kerf: 3.0,
            min_offcut: 100.0,
        };
        // 500 + 3 + 500 = 1003 does not fit; 497 does.
        let two = optimise(&[piece(1, 500.0), piece(2, 500.0)], s);
        assert_eq!(two.bars_needed(), 2);
        let two = optimise(&[piece(1, 497.0), piece(2, 500.0)], s);
        assert_eq!(two.bars_needed(), 1);
        assert_eq!(two.bars[0].left, 0.0);
    }

    #[test]
    fn a_piece_longer_than_a_bar_is_spliced() {
        let plan = optimise(&[piece(9, 2500.0)], Stock::default());
        assert_eq!(plan.bars_needed(), 2);
        assert_eq!(plan.lengths, vec![(9, 2500.0)]);
        let parts: Vec<_> = plan
            .bars
            .iter()
            .flat_map(|b| b.cuts.iter().map(|c| c.part))
            .collect();
        assert!(parts.contains(&Some((1, 2))) && parts.contains(&Some((2, 2))));
    }

    #[test]
    fn offcuts_are_kept_or_scrapped_by_length() {
        let plan = optimise(&[piece(1, 1950.0), piece(2, 1500.0)], Stock::default());
        assert_eq!(plan.bars_needed(), 2);
        assert!((plan.scrap() - 48.0).abs() < 1e-9);
        assert!((plan.reusable() - 498.0).abs() < 1e-9);
    }

    #[test]
    fn rail_pieces_round_up_and_never_exceed_the_rail() {
        let rail = Rail {
            id: 1,
            x: 10.0,
            y: 10.0,
            length: 400.0,
            vertical: false,
        };
        let p = |id: u64, x: f64, w: f64| Placement {
            id,
            x,
            y: 10.0,
            rotated: false,
            size: crate::Size::from_part(Some(w), Some(90.0)),
            clearance: None,
            rail: Some(1),
        };
        let pieces = rail_pieces(
            std::slice::from_ref(&rail),
            &[p(1, 10.0, 45.0), p(2, 65.0, 45.0)],
        );
        // 100 mm used + 20 mm ends = 120.
        assert_eq!((pieces[0].min, pieces[0].max), (120.0, 400.0));
        let big = rail_pieces(&[rail], &[p(1, 10.0, 395.0)]);
        assert_eq!(big[0].min, 400.0);
    }
}
