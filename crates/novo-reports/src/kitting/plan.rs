//! The kitting plan: what to take from stock, in which build phase, and what
//! to buy, worked out from the cabinets, the wire jobs and the bill of
//! materials.
//!
//! - Rails and ducts of every cabinet with a mounting plate are cut from bars
//!   of the lengths on sale, held offcuts first ([`cutting`]).
//! - Hookup wire is cut from reels, one group per cross-section and colour;
//!   cable cores come from their cable, not from a reel.
//! - Ferrules, lugs, fasteners and labels are counted, with spares.
//! - The shopping list is the bill of materials plus the stock lines, minus
//!   what is on hand, so one list goes to the supplier.
//!
//! Counts of parts, wires and holes are read from the drawing and the layout;
//! reel lengths, pack sizes, kerf and allowances are planning values the
//! person sets, and every row says which it is.

use std::collections::BTreeMap;

use novo_core::{CabinetPlacement, Device, Part, Project, Rail, RailKind, Structure, natural_cmp};
use novo_layout::model::plate_documents;

use super::cutting::{self, Demand, Material, Source};
use super::settings::{KitSettings, OnHand};
use crate::bench::{FerruleStandard, bench_jobs, ferrule_counts, is_estimate, with_spares};
use crate::wirecut::{WireCutJob, cable_runs};
use crate::{BillOfMaterials, Cell, LabelSettings, Labels, Report, label_items, layout_labels};

/// A leftover of a reel at least this long is kept as a part reel, mm.
pub const KEEP_REEL_MM: f64 = 1000.0;

/// The build phase a pick belongs to, as the assembly checklist numbers them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phase {
    Prepare,
    Mount,
    Wire,
    Label,
}

impl Phase {
    pub const ALL: [Phase; 4] = [Phase::Prepare, Phase::Mount, Phase::Wire, Phase::Label];

    pub fn name(self) -> &'static str {
        match self {
            Phase::Prepare => "Prepare",
            Phase::Mount => "Mount",
            Phase::Wire => "Wire",
            Phase::Label => "Label",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Phase::Prepare => "pick-prepare",
            Phase::Mount => "pick-mount",
            Phase::Wire => "pick-wire",
            Phase::Label => "pick-label",
        }
    }

    /// The number in the assembly checklist (Drill is 2, not a pick phase).
    pub fn step(self) -> usize {
        match self {
            Phase::Prepare => 1,
            Phase::Mount => 3,
            Phase::Wire => 4,
            Phase::Label => 5,
        }
    }
}

/// Where a number comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// Counted or measured from the drawing and the layout.
    Model,
    /// Follows from a value the person set (reel length, pack size, kerf).
    Planning,
    /// Depends on lengths that are estimates (wires not routed through ducts).
    Estimate,
}

impl Basis {
    pub fn text(self) -> &'static str {
        match self {
            Basis::Model => "from the drawing",
            Basis::Planning => "planning value",
            Basis::Estimate => "estimate",
        }
    }
}

/// One line to pick.
#[derive(Debug, Clone, PartialEq)]
pub struct PickLine {
    pub phase: Phase,
    pub item: String,
    pub qty: f64,
    pub unit: &'static str,
    /// Where it is taken from: the shelf, the offcut bin, a reel.
    pub from: String,
    /// What it is for.
    pub for_: String,
    pub basis: Basis,
}

/// One cut of a bar, with the name of the piece.
#[derive(Debug, Clone, PartialEq)]
pub struct SawCut {
    pub label: String,
    pub length: f64,
    pub part: Option<(usize, usize)>,
    pub from_left: f64,
}

/// One bar of the saw plan.
#[derive(Debug, Clone, PartialEq)]
pub struct SawBar {
    pub number: usize,
    pub source: Source,
    pub cuts: Vec<SawCut>,
    pub left: f64,
    pub keep: bool,
}

/// The cutting of one material (all DIN rail, or one duct size).
#[derive(Debug, Clone, PartialEq)]
pub struct SawPlan {
    pub key: String,
    pub name: String,
    pub plan: cutting::Plan,
    pub bars: Vec<SawBar>,
    pub pieces: usize,
}

/// One reel of a wire group.
#[derive(Debug, Clone, PartialEq)]
pub struct ReelRow {
    /// A new reel of this length, or a part reel on hand.
    pub source: Source,
    pub wires: usize,
    /// Metres cut from the reel (without the allowance).
    pub used_m: f64,
    /// Metres left on the reel.
    pub left_m: f64,
}

/// The reels of one cross-section and colour.
#[derive(Debug, Clone, PartialEq)]
pub struct ReelGroup {
    pub label: String,
    pub mm2: Option<f64>,
    pub wires: usize,
    pub metres: f64,
    pub reels: Vec<ReelRow>,
    pub estimated: usize,
    pub budget_hit: bool,
    pub new_reels: usize,
    pub reel_m: f64,
}

/// Where a shopping line comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shop {
    Bom,
    Stock,
}

impl Shop {
    pub fn text(self) -> &'static str {
        match self {
            Shop::Bom => "BOM",
            Shop::Stock => "Stock",
        }
    }
}

/// One line of the merged shopping list.
#[derive(Debug, Clone, PartialEq)]
pub struct ShopLine {
    pub shop: Shop,
    pub item: String,
    pub order_number: String,
    pub need: f64,
    pub unit: &'static str,
    pub have: f64,
    pub buy: f64,
    pub note: String,
}

/// Everything the kitting reports print.
#[derive(Debug, Clone, PartialEq)]
pub struct KitPlan {
    pub settings: KitSettings,
    pub saw: Vec<SawPlan>,
    pub reels: Vec<ReelGroup>,
    pub picks: Vec<PickLine>,
    pub shopping: Vec<ShopLine>,
    /// What stops the plan from being complete.
    pub problems: Vec<String>,
}

impl KitPlan {
    /// True when there is nothing to kit.
    pub fn is_empty(&self) -> bool {
        self.picks.is_empty() && self.shopping.is_empty()
    }

    /// Bars to buy, over all materials.
    pub fn bars_to_buy(&self) -> usize {
        self.saw.iter().map(|s| s.plan.bars_bought()).sum()
    }

    /// Reels to buy, over all groups.
    pub fn reels_to_buy(&self) -> usize {
        self.reels.iter().map(|g| g.new_reels).sum()
    }

    pub fn picks_in(&self, phase: Phase) -> impl Iterator<Item = &PickLine> {
        self.picks.iter().filter(move |p| p.phase == phase)
    }
}

/// A number as printed in the lists: at most two decimals, none when whole.
pub fn num(v: f64) -> String {
    Cell::Number((v * 100.0).round() / 100.0).to_string()
}

fn position_cmp(a: &Rail, b: &Rail) -> std::cmp::Ordering {
    a.y.total_cmp(&b.y)
        .then(a.x.total_cmp(&b.x))
        .then(a.id.raw().cmp(&b.id.raw()))
}

/// A rail or duct to cut, named like the assembly checklist names it.
struct RailPiece {
    label: String,
    length: f64,
    material: String,
    name: String,
    duct: bool,
}

fn rail_pieces(project: &Project, enclosure: &Structure) -> Vec<RailPiece> {
    let tag = enclosure.tag.to_string();
    let mut rails: Vec<&Rail> = project
        .all::<Rail>()
        .filter(|r| r.structure == enclosure.id)
        .collect();
    rails.sort_by(|a, b| position_cmp(a, b));
    let (mut din, mut duct) = (0, 0);
    let mut out = Vec::new();
    for rail in rails {
        if !(rail.length.is_finite() && rail.length > 0.0) {
            continue;
        }
        let (mark, material, name, is_duct) = match rail.kind {
            RailKind::DinRail => {
                din += 1;
                (
                    format!("R{din}"),
                    "din".into(),
                    "DIN rail".to_owned(),
                    false,
                )
            }
            RailKind::Duct => {
                duct += 1;
                let sized = rail.width > 0.0 && rail.height > 0.0;
                let name = if sized {
                    format!("Wiring duct {} x {}", num(rail.width), num(rail.height))
                } else {
                    "Wiring duct".to_owned()
                };
                let key = if sized {
                    format!("duct-{}x{}", num(rail.width), num(rail.height))
                } else {
                    "duct".to_owned()
                };
                (format!("D{duct}"), key, name, true)
            }
        };
        out.push(RailPiece {
            label: format!("{tag} {mark}"),
            length: rail.length,
            material,
            name,
            duct: is_duct,
        });
    }
    out
}

/// `{n} x {length} mm` summary of the bars to buy.
fn bar_pick(name: &str, length: f64, n: usize, bars: &[&SawBar]) -> PickLine {
    let mut marks: Vec<String> = bars
        .iter()
        .flat_map(|b| b.cuts.iter().map(|c| c.label.clone()))
        .collect();
    marks.sort_by(|a, b| natural_cmp(a, b));
    PickLine {
        phase: Phase::Prepare,
        item: format!("{name} bar, {} mm", num(length)),
        qty: n as f64,
        unit: "bars",
        from: "New bar, to buy".into(),
        for_: short_list(&marks),
        basis: Basis::Model,
    }
}

/// A list of names, cut to a length a table cell holds.
fn short_list(names: &[String]) -> String {
    const SHOWN: usize = 6;
    if names.len() <= SHOWN {
        names.join(", ")
    } else {
        format!(
            "{}, and {} more",
            names[..SHOWN].join(", "),
            names.len() - SHOWN
        )
    }
}

fn saw_plans(
    project: &Project,
    enclosures: &[&Structure],
    settings: &KitSettings,
    held: &OnHand,
) -> Vec<SawPlan> {
    // Demands per material, in build order: cabinet by cabinet.
    let mut order: Vec<String> = Vec::new();
    let mut by_material: BTreeMap<String, (String, bool, Vec<RailPiece>)> = BTreeMap::new();
    for e in enclosures {
        for p in rail_pieces(project, e) {
            if !order.contains(&p.material) {
                order.push(p.material.clone());
            }
            by_material
                .entry(p.material.clone())
                .or_insert_with(|| (p.name.clone(), p.duct, Vec::new()))
                .2
                .push(p);
        }
    }
    order.sort_by(|a, b| (a != "din").cmp(&(b != "din")).then(natural_cmp(a, b)));
    let mut out = Vec::new();
    for key in order {
        let (name, is_duct, pieces) = by_material.remove(&key).expect("collected above");
        let catalogue = if is_duct {
            &settings.duct_stock_mm
        } else {
            &settings.rail_stock_mm
        };
        let material = Material::new(catalogue, settings.kerf_mm, settings.min_offcut_mm);
        let demands: Vec<Demand> = pieces
            .iter()
            .enumerate()
            .map(|(id, p)| Demand {
                id,
                length: p.length,
            })
            .collect();
        let held_here: Vec<f64> = if is_duct {
            let want: Option<(f64, f64)> = key
                .strip_prefix("duct-")
                .and_then(|s| s.split_once('x'))
                .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)));
            held.ducts
                .iter()
                .filter(|(section, _)| Some(*section) == want)
                .map(|(_, l)| *l)
                .collect()
        } else {
            held.rails.clone()
        };
        let plan = cutting::plan(&material, &demands, &held_here);
        let bars = plan
            .bins
            .iter()
            .enumerate()
            .map(|(n, b)| SawBar {
                number: n + 1,
                source: b.source,
                cuts: b
                    .cuts
                    .iter()
                    .map(|c| SawCut {
                        label: pieces[c.id].label.clone(),
                        length: c.length,
                        part: c.part,
                        from_left: c.from_left,
                    })
                    .collect(),
                left: b.left,
                keep: b.left >= material.min_offcut && b.left > 0.0,
            })
            .collect();
        out.push(SawPlan {
            key,
            name,
            plan,
            bars,
            pieces: pieces.len(),
        });
    }
    out
}

fn colour_label(colour: &str) -> String {
    if colour.trim().is_empty() {
        "colour not set".into()
    } else {
        colour.trim().to_owned()
    }
}

fn group_label(mm2: Option<f64>, colour: &str) -> String {
    match mm2 {
        Some(s) => format!("{} mm² {}", num(s), colour_label(colour)),
        None => format!("size not set, {}", colour_label(colour)),
    }
}

fn reel_groups(
    jobs: &[&WireCutJob],
    settings: &KitSettings,
    held: &OnHand,
    problems: &mut Vec<String>,
) -> Vec<ReelGroup> {
    let mut groups: BTreeMap<(Option<u64>, String), Vec<&WireCutJob>> = BTreeMap::new();
    for j in jobs {
        groups
            .entry((
                j.cross_section_mm2.map(|s| (s * 1000.0).round() as u64),
                j.colour.trim().to_lowercase(),
            ))
            .or_default()
            .push(j);
    }
    let mut keys: Vec<_> = groups.keys().cloned().collect();
    keys.sort_by(|a, b| {
        a.0.is_none()
            .cmp(&b.0.is_none())
            .then(a.0.cmp(&b.0))
            .then_with(|| natural_cmp(&a.1, &b.1))
    });
    let allowance = 1.0 + settings.wire_allowance_percent / 100.0;
    let mut out = Vec::new();
    for key in keys {
        let list = &groups[&key];
        let mm2 = list[0].cross_section_mm2;
        let colour = list[0].colour.clone();
        let reel_m = settings.reel_m_for(mm2);
        let reel_mm = reel_m * 1000.0;
        let label = group_label(mm2, &colour);
        // The allowance is held back by cutting "longer" wires from the reel.
        let mut demands = Vec::new();
        let mut usable: Vec<usize> = Vec::new();
        for (id, j) in list.iter().enumerate() {
            if j.cut_mm * allowance > reel_mm + 1e-6 {
                problems.push(format!(
                    "Wire {} ({label}) is {} m, longer than a {} m reel: use a longer reel or join it",
                    j.number,
                    num(j.cut_mm / 1000.0),
                    num(reel_m)
                ));
                continue;
            }
            demands.push(Demand {
                id,
                length: j.cut_mm * allowance,
            });
            usable.push(id);
        }
        let held_here: Vec<f64> = held
            .wires
            .iter()
            .filter(|(s, c, _)| {
                mm2.is_some_and(|m| (m - s).abs() < 1e-9) && *c == colour.trim().to_lowercase()
            })
            .map(|(_, _, mm)| *mm)
            .collect();
        let material = Material::new(&[reel_mm], 0.0, KEEP_REEL_MM * allowance);
        let plan = cutting::plan(&material, &demands, &held_here);
        let mut reels: Vec<ReelRow> = plan
            .bins
            .iter()
            .map(|b| {
                let used: f64 = b.cuts.iter().map(|c| list[c.id].cut_mm).sum();
                ReelRow {
                    source: b.source,
                    wires: b.cuts.len(),
                    used_m: used / 1000.0,
                    left_m: (b.source.length() - used) / 1000.0,
                }
            })
            .collect();
        reels.sort_by(|a, b| {
            matches!(a.source, Source::New(_)).cmp(&matches!(b.source, Source::New(_)))
        });
        out.push(ReelGroup {
            label,
            mm2,
            wires: usable.len(),
            metres: usable.iter().map(|&i| list[i].cut_mm).sum::<f64>() / 1000.0,
            new_reels: plan.bars_bought(),
            reels,
            estimated: list.iter().filter(|j| is_estimate(&j.basis)).count(),
            budget_hit: plan.budget_hit,
            reel_m,
        });
    }
    out
}

/// Plans the kitting of `project` under `settings`.
pub fn kit_plan(project: &Project, settings: &KitSettings) -> KitPlan {
    let mut problems = Vec::new();
    let settings = if settings.is_valid() {
        settings.clone()
    } else {
        problems.push(
            "The kitting settings are not valid (a length, a pack or the allowance is out of range): defaults used"
                .to_owned(),
        );
        KitSettings::default()
    };
    let (held, held_problems) = OnHand::parse(&settings.on_hand);
    problems.extend(held_problems);

    let mut enclosures: Vec<&Structure> = project
        .structures()
        .filter(|s| s.enclosure && s.plate.is_some())
        .collect();
    enclosures.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));

    let mut picks: Vec<PickLine> = Vec::new();
    let mut shopping: Vec<ShopLine> = Vec::new();

    // ------------------------------------------------------------ Prepare
    let saw = saw_plans(project, &enclosures, &settings, &held);
    for s in &saw {
        let mut new_by_len: Vec<(f64, usize, Vec<&SawBar>)> = Vec::new();
        for bar in &s.bars {
            if let Source::New(l) = bar.source {
                match new_by_len
                    .iter_mut()
                    .find(|(len, _, _)| (*len - l).abs() < 1e-6)
                {
                    Some(e) => {
                        e.1 += 1;
                        e.2.push(bar);
                    }
                    None => new_by_len.push((l, 1, vec![bar])),
                }
            }
        }
        new_by_len.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (len, n, bars) in new_by_len {
            picks.push(bar_pick(&s.name, len, n, &bars));
            shopping.push(ShopLine {
                shop: Shop::Stock,
                item: format!("{} {} mm bar", s.name, num(len)),
                order_number: String::new(),
                need: n as f64,
                unit: "bars",
                have: 0.0,
                buy: n as f64,
                note: if s.plan.budget_hit {
                    "search stopped early, a bar fewer may be possible".into()
                } else {
                    String::new()
                },
            });
        }
        for bar in s
            .bars
            .iter()
            .filter(|b| matches!(b.source, Source::Held(_)))
        {
            let marks: Vec<String> = bar.cuts.iter().map(|c| c.label.clone()).collect();
            picks.push(PickLine {
                phase: Phase::Prepare,
                item: format!("{} offcut, {} mm", s.name, num(bar.source.length())),
                qty: 1.0,
                unit: "pcs",
                from: "Offcut bin".into(),
                for_: short_list(&marks),
                basis: Basis::Model,
            });
        }
    }

    // -------------------------------------------------------------- Mount
    for e in &enclosures {
        let tag = e.tag.to_string();
        let mut parts: BTreeMap<(String, String), (Vec<String>, usize, String)> = BTreeMap::new();
        let mut placed: Vec<(&CabinetPlacement, &Device)> = project
            .all::<CabinetPlacement>()
            .filter(|p| p.structure == e.id)
            .filter_map(|p| Some((p, project.get::<Device>(p.device)?)))
            .collect();
        placed.sort_by(|(_, a), (_, b)| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
        for (_, device) in &placed {
            for part in device
                .parts
                .iter()
                .filter_map(|id| project.get::<Part>(*id))
            {
                let entry = parts
                    .entry((part.manufacturer.clone(), part.order_number.clone()))
                    .or_insert_with(|| {
                        let what = if part.type_number.is_empty() {
                            part.order_number.clone()
                        } else {
                            part.type_number.clone()
                        };
                        (
                            Vec::new(),
                            0,
                            format!("{} {what}", part.manufacturer).trim().to_owned(),
                        )
                    });
                entry.0.push(device.tag.to_string());
                entry.1 += 1;
            }
        }
        for (_, (tags, n, name)) in parts {
            picks.push(PickLine {
                phase: Phase::Mount,
                item: name,
                qty: n as f64,
                unit: "pcs",
                from: format!("Parts, {tag}"),
                for_: short_list(&tags),
                basis: Basis::Model,
            });
        }
    }
    // Fasteners: one per drilled hole, with spares.
    let standard = FerruleStandard::of(project);
    let mut holes: BTreeMap<String, usize> = BTreeMap::new();
    for e in &enclosures {
        if let Some((_, table)) = plate_documents(project, e.id) {
            for (_, dia, n) in table.summary() {
                *holes.entry(novo_layout::drill::mm(dia)).or_default() += n;
            }
        }
    }
    for (dia, n) in &holes {
        let qty = with_spares(*n, standard.spare_percent);
        let item = format!("Screw and washer for dia {dia} mm holes");
        picks.push(PickLine {
            phase: Phase::Mount,
            item: item.clone(),
            qty: qty as f64,
            unit: "pcs",
            from: "Fasteners".into(),
            for_: format!(
                "{n} holes + {} % spares; size and length to choose",
                num(standard.spare_percent)
            ),
            basis: Basis::Planning,
        });
        let have = f64::from(held.item(&[&item]));
        shopping.push(pack_line(
            &settings,
            &item,
            qty as f64,
            have,
            settings.pack_screw,
            "screws",
        ));
    }

    // --------------------------------------------------------------- Wire
    let jobs = bench_jobs(project);
    let hookup: Vec<&WireCutJob> = jobs.iter().filter(|j| j.cable.is_none()).collect();
    let reels = reel_groups(&hookup, &settings, &held, &mut problems);
    for g in &reels {
        let basis = if g.estimated > 0 {
            Basis::Estimate
        } else {
            Basis::Planning
        };
        let note = format!(
            "{}, {} m",
            crate::count_text(g.wires, "wires"),
            num(g.metres)
        );
        for r in &g.reels {
            if let Source::Held(l) = r.source {
                picks.push(PickLine {
                    phase: Phase::Wire,
                    item: format!("Part reel {}, {} m", g.label, num(l / 1000.0)),
                    qty: 1.0,
                    unit: "reel",
                    from: "Wire shelf".into(),
                    for_: format!(
                        "{}, {} m",
                        crate::count_text(r.wires, "wires"),
                        num(r.used_m)
                    ),
                    basis,
                });
            }
        }
        if g.new_reels > 0 {
            picks.push(PickLine {
                phase: Phase::Wire,
                item: format!("Wire {}, reel of {} m", g.label, num(g.reel_m)),
                qty: g.new_reels as f64,
                unit: "reels",
                from: "New reel, to buy".into(),
                for_: note.clone(),
                basis,
            });
            shopping.push(ShopLine {
                shop: Shop::Stock,
                item: format!("Wire {}, reel of {} m", g.label, num(g.reel_m)),
                order_number: String::new(),
                need: g.new_reels as f64,
                unit: "reels",
                have: 0.0,
                buy: g.new_reels as f64,
                note: format!(
                    "{note}; {} % held back",
                    num(settings.wire_allowance_percent)
                ),
            });
        }
    }
    for run in cable_runs(project, &jobs) {
        picks.push(PickLine {
            phase: Phase::Wire,
            item: format!("Cable {}", run.tag),
            qty: run.metres(),
            unit: "m",
            from: "Cable drum".into(),
            for_: format!("{} cores, length {}", run.cores, run.basis()),
            basis: if run.stated {
                Basis::Model
            } else {
                Basis::Estimate
            },
        });
    }
    let unsized_wires = hookup
        .iter()
        .filter(|j| j.cross_section_mm2.is_none())
        .count();
    if unsized_wires > 0 {
        problems.push(format!(
            "{unsized_wires} wires have no cross-section: they are listed as one group and the ferrules for them are not counted"
        ));
    }
    let estimated = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
    if estimated > 0 {
        problems.push(format!(
            "{estimated} of {} wire lengths are estimates (not routed through ducts): the reels are a guide until they are routed",
            jobs.len()
        ));
    }
    for f in ferrule_counts(&jobs, &standard) {
        let qty = with_spares(f.pieces, standard.spare_percent);
        let item = format!(
            "Ferrule {}{}",
            f.ferrule,
            if f.colour.is_empty() {
                String::new()
            } else {
                format!(" ({})", f.colour)
            }
        );
        picks.push(PickLine {
            phase: Phase::Wire,
            item: item.clone(),
            qty: qty as f64,
            unit: "pcs",
            from: "Ferrule box".into(),
            for_: format!(
                "{} to crimp + {} % spares",
                f.pieces,
                num(standard.spare_percent)
            ),
            basis: Basis::Planning,
        });
        let have = f64::from(held.item(&[&f.ferrule, &item]));
        shopping.push(pack_line(
            &settings,
            &item,
            qty as f64,
            have,
            settings.pack_ferrule,
            "ferrules",
        ));
    }
    let mut lugs: BTreeMap<String, usize> = BTreeMap::new();
    for j in &jobs {
        for end in [&j.end_a, &j.end_b] {
            if end.ferrule.is_none() && end.processing_basis == crate::StripBasis::Stated {
                *lugs.entry(end.processing.clone()).or_default() += 1;
            }
        }
    }
    for (what, n) in lugs {
        let qty = with_spares(n, standard.spare_percent);
        picks.push(PickLine {
            phase: Phase::Wire,
            item: what.clone(),
            qty: qty as f64,
            unit: "pcs",
            from: "Lug drawer".into(),
            for_: format!("{n} wire ends + {} % spares", num(standard.spare_percent)),
            basis: Basis::Planning,
        });
        let have = f64::from(held.item(&[&what]));
        shopping.push(pack_line(
            &settings,
            &what,
            qty as f64,
            have,
            settings.pack_lug,
            "lugs",
        ));
    }

    // -------------------------------------------------------------- Label
    let label_settings = LabelSettings::of(project);
    let labels = Labels.generate(project);
    let items = label_items(&labels, &label_settings.kinds);
    if !items.is_empty() && label_settings.is_valid() {
        let run = layout_labels(&items, &label_settings);
        let stock = label_settings.stock.name();
        let sheets = run.pages.len();
        let (unit, qty) = if run.per_page > 1 {
            ("sheets", sheets as f64)
        } else {
            ("labels", run.labels as f64)
        };
        let item = format!("Label stock {stock}");
        picks.push(PickLine {
            phase: Phase::Label,
            item: item.clone(),
            qty,
            unit,
            from: "Label drawer".into(),
            for_: format!("{} labels x {} copies", items.len(), label_settings.copies),
            basis: Basis::Model,
        });
        let have = f64::from(held.item(&[&item]));
        shopping.push(ShopLine {
            shop: Shop::Stock,
            item,
            order_number: String::new(),
            need: qty,
            unit,
            have,
            buy: (qty - have).max(0.0),
            note: format!("{} labels printed", run.labels),
        });
    }

    // ---------------------------------------------- Shopping: BOM + stock
    let mut merged: Vec<ShopLine> = Vec::new();
    let bom = BillOfMaterials.generate(project);
    let mut without = 0;
    if let Some(parts) = bom.section("parts") {
        let col = |row: &crate::Row, key: &str| {
            parts
                .column(key)
                .and_then(|i| row.cells.get(i))
                .cloned()
                .unwrap_or(Cell::Empty)
        };
        for row in &parts.rows {
            let qty = match col(row, "quantity") {
                Cell::Int(q) => q as f64,
                Cell::Number(q) => q,
                _ => 1.0,
            };
            let length = match col(row, "length_m") {
                Cell::Number(m) => Some(m),
                Cell::Int(m) => Some(m as f64),
                _ => None,
            };
            let manufacturer = col(row, "manufacturer").to_string();
            let order = col(row, "order_number").to_string();
            let kind = col(row, "type_number").to_string();
            let what = if kind.is_empty() { order.clone() } else { kind };
            let (need, unit) = match length {
                Some(m) => (m, "m"),
                None => (qty, "pcs"),
            };
            let have = f64::from(held.item(&[
                &order,
                &format!("{manufacturer} {order}"),
                &format!("{manufacturer} {what}"),
            ]));
            merged.push(ShopLine {
                shop: Shop::Bom,
                item: format!("{manufacturer} {what}").trim().to_owned(),
                order_number: order,
                need,
                unit,
                have,
                buy: (need - have).max(0.0),
                note: col(row, "description").to_string(),
            });
        }
    }
    if let Some(missing) = bom.section("without_parts") {
        without = missing.rows.len();
    }
    if without > 0 {
        problems.push(format!(
            "{without} devices or cables have no part: they cannot be on the shopping list until a part is assigned"
        ));
    }
    for mut line in shopping {
        if let Some(order) = settings.article_of(&line.item) {
            line.order_number = order.clone();
            if let Some(existing) = merged.iter_mut().find(|m| {
                m.shop == Shop::Bom
                    && m.order_number.eq_ignore_ascii_case(&order)
                    && (m.unit == "m") == (line.unit == "m")
            }) {
                existing.need += line.need;
                existing.buy = (existing.need - existing.have).max(0.0);
                existing.note = format!("{}; also {} for the build", existing.note, line.item)
                    .trim_start_matches("; ")
                    .to_owned();
                continue;
            }
        }
        merged.push(line);
    }
    merged.sort_by(|a, b| {
        (a.shop == Shop::Stock)
            .cmp(&(b.shop == Shop::Stock))
            .then_with(|| natural_cmp(&a.item, &b.item))
    });

    KitPlan {
        settings,
        saw,
        reels,
        picks,
        shopping: merged,
        problems,
    }
}

/// A counted item sold in packs: the buy quantity rounds up to whole packs.
fn pack_line(
    settings: &KitSettings,
    item: &str,
    need: f64,
    have: f64,
    pack: u32,
    _what: &str,
) -> ShopLine {
    let missing = (need - have).max(0.0);
    let pack_f = f64::from(pack);
    let buy = (missing / pack_f - 1e-9).ceil().max(0.0) * pack_f;
    ShopLine {
        shop: Shop::Stock,
        item: item.to_owned(),
        order_number: settings.article_of(item).unwrap_or_default(),
        need,
        unit: "pcs",
        have,
        buy,
        note: if pack > 1 {
            format!("packs of {pack}, incl. spares")
        } else {
            "incl. spares".into()
        },
    }
}
