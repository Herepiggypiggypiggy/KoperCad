//! One-dimensional cutting stock: which pieces to cut from which bar, and
//! what to buy.
//!
//! The material comes in a few bar lengths (the catalogue) and the shop may
//! already hold bars and offcuts, which are free. The answer buys the least
//! length (then the fewest bars) that covers every piece, where each cut the
//! saw makes costs its kerf. An exact search (branch and bound over a
//! best-fit-decreasing start) finds the answer; when the search budget runs
//! out the best plan so far is kept and [`Plan::budget_hit`] says so. A bar
//! bought for a few short pieces is bought in the shortest catalogue length
//! that holds them. A piece longer than the longest bar is cut from whole
//! bars plus a remainder and marked as spliced.
//!
//! Pieces come in build order. Inside a bar the cuts are listed in that
//! order, and each cut says how long the bar piece was before it, so the
//! bench sheet can say "cut R2 from the offcut left by R1".
//!
//! Millimetres throughout.

/// The search gives up after this many nodes and keeps its best plan.
pub const SEARCH_LIMIT: u64 = 300_000;
const EPS: f64 = 1e-6;

/// One material: the bar lengths on sale and how the saw behaves.
#[derive(Debug, Clone, PartialEq)]
pub struct Material {
    /// Bar lengths that can be bought, ascending.
    pub catalogue: Vec<f64>,
    /// Length lost to every cut between two pieces.
    pub kerf: f64,
    /// A leftover at least this long is kept as an offcut, shorter is scrap.
    pub min_offcut: f64,
}

impl Material {
    /// A material; the catalogue is sorted, and lengths that are not
    /// positive numbers are dropped.
    pub fn new(catalogue: &[f64], kerf: f64, min_offcut: f64) -> Material {
        let mut c: Vec<f64> = catalogue
            .iter()
            .copied()
            .filter(|l| l.is_finite() && *l > 0.0)
            .collect();
        c.sort_by(f64::total_cmp);
        c.dedup_by(|a, b| (*a - *b).abs() < EPS);
        Material {
            catalogue: c,
            kerf: kerf.max(0.0),
            min_offcut: min_offcut.max(0.0),
        }
    }

    /// The longest bar on sale (0 when there is none).
    pub fn longest(&self) -> f64 {
        self.catalogue.last().copied().unwrap_or(0.0)
    }

    /// The shortest catalogue bar that holds a load of pieces plus one kerf
    /// each (the last cut of a bar does not need its kerf, hence `+ kerf`).
    fn shrink(&self, load: f64) -> f64 {
        self.catalogue
            .iter()
            .copied()
            .find(|c| load <= c + self.kerf + EPS)
            .unwrap_or_else(|| self.longest())
    }
}

/// A piece to cut. `id` is the caller's number for it; pieces are given in
/// build order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Demand {
    pub id: usize,
    pub length: f64,
}

/// Where a bar came from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Source {
    /// A bar or offcut already in the shop, of this length.
    Held(f64),
    /// A bar to buy, of this catalogue length.
    New(f64),
}

impl Source {
    pub fn length(self) -> f64 {
        match self {
            Source::Held(l) | Source::New(l) => l,
        }
    }
}

/// One cut of a bar.
#[derive(Debug, Clone, PartialEq)]
pub struct Cut {
    pub id: usize,
    pub length: f64,
    /// For a piece longer than the longest bar: which part this is and how
    /// many there are (`(1, 2)` is the first of two).
    pub part: Option<(usize, usize)>,
    /// Length of the bar piece before this cut is made.
    pub from_left: f64,
}

/// A bar and what is cut from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Bin {
    pub source: Source,
    /// The cuts in build order.
    pub cuts: Vec<Cut>,
    /// Length left after the last cut and the kerf of separating it.
    pub left: f64,
}

/// The answer.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub material: Material,
    /// The bars used, held ones first, then bars to buy.
    pub bins: Vec<Bin>,
    /// Held bars and offcuts the plan does not touch.
    pub unused_held: Vec<f64>,
    /// Bars to buy: `(length, count)`, ascending by length.
    pub bought: Vec<(f64, usize)>,
    /// Total length bought.
    pub bought_mm: f64,
    /// No plan can buy less than this (total piece length minus what is held).
    pub lower_bound_mm: f64,
    /// True when the search ran out of budget, so the plan may not be the
    /// cheapest possible.
    pub budget_hit: bool,
}

impl Plan {
    /// Bars to buy.
    pub fn bars_bought(&self) -> usize {
        self.bought.iter().map(|(_, n)| n).sum()
    }

    fn kept(&self, b: &Bin) -> bool {
        b.left >= self.material.min_offcut && b.left > 0.0
    }

    /// Leftovers long enough to keep, longest first.
    pub fn offcuts(&self) -> Vec<f64> {
        let mut v: Vec<f64> = self
            .bins
            .iter()
            .filter(|b| self.kept(b))
            .map(|b| b.left)
            .collect();
        v.sort_by(|a, b| b.total_cmp(a));
        v
    }

    /// Total length of the leftovers shorter than the minimum.
    pub fn scrap_mm(&self) -> f64 {
        self.bins
            .iter()
            .filter(|b| !self.kept(b))
            .map(|b| b.left)
            .sum()
    }

    /// True when the plan buys exactly the lower bound, so it is optimal.
    pub fn proven_optimal(&self) -> bool {
        !self.budget_hit && self.bought_mm <= self.lower_bound_mm + EPS
    }
}

struct Item {
    id: usize,
    len: f64,
    part: Option<(usize, usize)>,
}

#[derive(Clone)]
struct BinState {
    held: Option<f64>,
    load: f64,
}

struct Search<'a> {
    material: &'a Material,
    need: Vec<f64>,
    rest: Vec<f64>,
    bins: Vec<BinState>,
    assign: Vec<usize>,
    cost: f64,
    new_bars: usize,
    best_cost: f64,
    best_bars: usize,
    best_assign: Vec<usize>,
    best_bins: Vec<BinState>,
    lower: f64,
    nodes: u64,
    hit: bool,
    done: bool,
}

impl Search<'_> {
    fn room(&self, b: &BinState) -> f64 {
        match b.held {
            Some(cap) => cap + self.material.kerf,
            None => self.material.shrink(b.load) + self.material.kerf,
        }
    }

    fn improves(&self, cost: f64, bars: usize) -> bool {
        cost < self.best_cost - EPS
            || ((cost - self.best_cost).abs() <= EPS && bars < self.best_bars)
    }

    fn go(&mut self, k: usize) {
        if self.hit || self.done {
            return;
        }
        self.nodes += 1;
        if self.nodes > SEARCH_LIMIT {
            self.hit = true;
            return;
        }
        if k == self.need.len() {
            if self.improves(self.cost, self.new_bars) {
                self.best_cost = self.cost;
                self.best_bars = self.new_bars;
                self.best_assign = self.assign.clone();
                self.best_bins = self.bins.clone();
                if self.best_cost <= self.lower + EPS && self.best_bars <= self.bars_lower() {
                    self.done = true;
                }
            }
            return;
        }
        // Bound: what does not fit in the room already paid for must be bought.
        let free: f64 = self.bins.iter().map(|b| self.room(b) - b.load).sum();
        let extra = (self.rest[k] - free).max(0.0);
        if self.cost + extra > self.best_cost + EPS
            || (self.cost + extra >= self.best_cost - EPS && self.new_bars >= self.best_bars)
        {
            return;
        }
        let w = self.need[k];
        let max_room = self.material.longest() + self.material.kerf;
        let mut tried: Vec<(bool, f64, f64)> = Vec::new();
        for b in 0..self.bins.len() {
            let state = self.bins[b].clone();
            let room = self.room(&state);
            let key = (state.held.is_some(), room, state.load);
            if tried
                .iter()
                .any(|t| t.0 == key.0 && (t.1 - key.1).abs() < EPS && (t.2 - key.2).abs() < EPS)
            {
                continue;
            }
            let (fits, delta) = match state.held {
                Some(_) => (state.load + w <= room + EPS, 0.0),
                None => (
                    state.load + w <= max_room + EPS,
                    self.material.shrink(state.load + w) - self.material.shrink(state.load),
                ),
            };
            if !fits {
                continue;
            }
            tried.push(key);
            self.bins[b].load += w;
            self.assign[k] = b;
            self.cost += delta;
            self.go(k + 1);
            self.cost -= delta;
            self.bins[b].load -= w;
        }
        if w <= max_room + EPS {
            let add = self.material.shrink(w);
            self.bins.push(BinState {
                held: None,
                load: w,
            });
            self.assign[k] = self.bins.len() - 1;
            self.cost += add;
            self.new_bars += 1;
            self.go(k + 1);
            self.new_bars -= 1;
            self.cost -= add;
            self.bins.pop();
        }
    }

    fn bars_lower(&self) -> usize {
        let l = self.material.longest();
        if l <= 0.0 {
            0
        } else {
            ((self.lower / l) - EPS).ceil().max(0.0) as usize
        }
    }
}

/// Plans the cuts of `demands` (in build order) from the `held` bars and
/// offcuts first, then bars bought from the catalogue.
pub fn plan(material: &Material, demands: &[Demand], held: &[f64]) -> Plan {
    let mut plan = Plan {
        material: material.clone(),
        bins: Vec::new(),
        unused_held: Vec::new(),
        bought: Vec::new(),
        bought_mm: 0.0,
        lower_bound_mm: 0.0,
        budget_hit: false,
    };
    let longest = material.longest();
    let mut held: Vec<f64> = held
        .iter()
        .copied()
        .filter(|l| l.is_finite() && *l > 0.0)
        .collect();
    held.sort_by(|a, b| b.total_cmp(a));

    // Pieces longer than the longest bar: whole bars plus a remainder.
    let mut items: Vec<Item> = Vec::new();
    let mut spliced: Vec<Bin> = Vec::new();
    for d in demands {
        if !(d.length.is_finite() && d.length > 0.0) {
            continue;
        }
        if longest <= 0.0 {
            continue;
        }
        if d.length <= longest + EPS {
            items.push(Item {
                id: d.id,
                len: d.length,
                part: None,
            });
            continue;
        }
        let whole = (d.length / longest + EPS).floor() as usize;
        let rest = d.length - whole as f64 * longest;
        let parts = whole + usize::from(rest > EPS);
        for n in 0..whole {
            spliced.push(Bin {
                source: Source::New(longest),
                cuts: vec![Cut {
                    id: d.id,
                    length: longest,
                    part: Some((n + 1, parts)),
                    from_left: longest,
                }],
                left: 0.0,
            });
        }
        if rest > EPS {
            items.push(Item {
                id: d.id,
                len: rest,
                part: Some((parts, parts)),
            });
        }
    }

    // Held bars that cannot take any piece, and equal bars beyond the number
    // of pieces, cannot change the plan; keeping them out of the search keeps
    // a shelf of thousands of offcuts from drowning it.
    let shortest = items.iter().map(|i| i.len).fold(f64::INFINITY, f64::min);
    let mut skipped: Vec<f64> = Vec::new();
    let mut kept: Vec<f64> = Vec::new();
    let mut same = 0usize;
    for &h in &held {
        if h + EPS < shortest {
            skipped.push(h);
            continue;
        }
        same = match kept.last() {
            Some(&last) if (last - h).abs() <= EPS => same + 1,
            _ => 1,
        };
        if same > items.len() {
            skipped.push(h);
        } else {
            kept.push(h);
        }
    }
    let held = kept;

    let total: f64 = items.iter().map(|i| i.len).sum();
    let held_total: f64 = held.iter().sum();
    plan.lower_bound_mm =
        (total - held_total).max(0.0) + spliced.iter().map(|b| b.source.length()).sum::<f64>();

    // Longest piece first for the search; `order[k]` is the item index.
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by(|&a, &b| items[b].len.total_cmp(&items[a].len).then(a.cmp(&b)));

    let need: Vec<f64> = order
        .iter()
        .map(|&i| items[i].len + material.kerf)
        .collect();
    let mut rest = vec![0.0; need.len() + 1];
    for k in (0..need.len()).rev() {
        rest[k] = rest[k + 1] + items[order[k]].len;
    }

    let start_bins: Vec<BinState> = held
        .iter()
        .map(|&h| BinState {
            held: Some(h),
            load: 0.0,
        })
        .collect();
    let mut search = Search {
        material,
        need,
        rest,
        bins: start_bins.clone(),
        assign: vec![0; order.len()],
        cost: 0.0,
        new_bars: 0,
        best_cost: f64::INFINITY,
        best_bars: usize::MAX,
        best_assign: vec![0; order.len()],
        best_bins: start_bins.clone(),
        lower: plan.lower_bound_mm - spliced.iter().map(|b| b.source.length()).sum::<f64>(),
        nodes: 0,
        hit: false,
        done: false,
    };
    if !order.is_empty() && longest > 0.0 {
        seed(&mut search, start_bins);
        search.go(0);
    } else {
        search.best_cost = 0.0;
        search.best_bars = 0;
    }
    plan.budget_hit = search.hit;

    // Cuts per bin in build order.
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); search.best_bins.len()];
    for (k, &b) in search.best_assign.iter().enumerate() {
        if !order.is_empty() && longest > 0.0 {
            members[b].push(order[k]);
        }
    }
    let mut bins = Vec::new();
    for (b, state) in search.best_bins.iter().enumerate() {
        let mut m = members[b].clone();
        if m.is_empty() {
            if let Some(h) = state.held {
                plan.unused_held.push(h);
            }
            continue;
        }
        m.sort_by(|&x, &y| items[x].id.cmp(&items[y].id).then(x.cmp(&y)));
        let source = match state.held {
            Some(h) => Source::Held(h),
            None => Source::New(material.shrink(state.load)),
        };
        let mut remaining = source.length();
        let mut cuts = Vec::new();
        for (n, &i) in m.iter().enumerate() {
            cuts.push(Cut {
                id: items[i].id,
                length: items[i].len,
                part: items[i].part,
                from_left: remaining,
            });
            remaining -= items[i].len;
            if n + 1 < m.len() {
                remaining -= material.kerf;
            }
        }
        // Separating the leftover from the last piece takes one more cut.
        let left = if remaining > EPS {
            (remaining - material.kerf).max(0.0)
        } else {
            0.0
        };
        bins.push(Bin { source, cuts, left });
    }
    plan.unused_held.extend(skipped);
    plan.unused_held.sort_by(|a, b| b.total_cmp(a));
    bins.extend(spliced);
    bins.sort_by(|a, b| {
        let held = |x: &Bin| matches!(x.source, Source::New(_));
        held(a)
            .cmp(&held(b))
            .then_with(|| a.cuts[0].id.cmp(&b.cuts[0].id))
    });
    for b in &bins {
        if let Source::New(l) = b.source {
            match plan
                .bought
                .iter_mut()
                .find(|(len, _)| (*len - l).abs() < EPS)
            {
                Some((_, n)) => *n += 1,
                None => plan.bought.push((l, 1)),
            }
            plan.bought_mm += l;
        }
    }
    plan.bought.sort_by(|a, b| a.0.total_cmp(&b.0));
    plan.bins = bins;
    plan
}

/// Best-fit-decreasing start: held bars first, then bars to buy.
fn seed(s: &mut Search<'_>, start: Vec<BinState>) {
    let material = s.material;
    let max_room = material.longest() + material.kerf;
    let mut bins = start;
    let mut assign = vec![0usize; s.need.len()];
    let mut cost = 0.0;
    let mut new_bars = 0usize;
    for (k, &w) in s.need.clone().iter().enumerate() {
        let room_of = |b: &BinState| match b.held {
            Some(cap) => cap + material.kerf,
            None => max_room,
        };
        let best = (0..bins.len())
            .filter(|&b| bins[b].load + w <= room_of(&bins[b]) + EPS)
            .min_by(|&a, &b| {
                let key = |i: usize| (bins[i].held.is_none(), room_of(&bins[i]) - bins[i].load - w);
                let (ka, kb) = (key(a), key(b));
                ka.0.cmp(&kb.0).then(ka.1.total_cmp(&kb.1))
            });
        match best {
            Some(b) => {
                if bins[b].held.is_none() {
                    cost += material.shrink(bins[b].load + w) - material.shrink(bins[b].load);
                }
                bins[b].load += w;
                assign[k] = b;
            }
            None => {
                bins.push(BinState {
                    held: None,
                    load: w,
                });
                assign[k] = bins.len() - 1;
                cost += material.shrink(w);
                new_bars += 1;
            }
        }
    }
    s.best_cost = cost;
    s.best_bars = new_bars;
    s.best_assign = assign;
    s.best_bins = bins;
    if cost <= s.lower + EPS && new_bars <= s.bars_lower() {
        s.done = true;
    }
}

#[cfg(test)]
mod tests;
