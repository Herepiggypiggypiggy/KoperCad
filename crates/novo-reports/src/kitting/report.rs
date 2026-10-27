//! The kitting report: what to pick for each build phase, how to cut the
//! bars, the reels, and the shopping list.

use novo_core::EntityKind;

use super::cutting::Source;
use super::plan::{KitPlan, Phase, kit_plan, num};
use super::settings::KitSettings;
use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};
use novo_core::Project;

/// The box printed in the Done column.
pub const KITTING_BOX: &str = "[ ]";

/// The kitting report: pick list per phase, saw plan, reels, offcuts and
/// the shopping list. Not proposed as a generated page: it is a document for
/// the workshop and the purchaser.
#[derive(Debug, Clone, Copy, Default)]
pub struct Kitting;

impl Kitting {
    pub const ID: &'static str = "kitting";

    /// The report for a plan worked out elsewhere (the window's draft
    /// settings, for instance).
    pub fn build(plan: &KitPlan, revision: u64) -> ReportOutput {
        let mut sections = Vec::new();
        for phase in Phase::ALL {
            let mut s = Section::new(
                phase.key(),
                &format!("Pick list {}  {}", phase.step(), phase.name()),
                vec![
                    Column::new("done", "Picked", 8.0),
                    Column::new("item", "Item", 42.0),
                    Column::new("qty", "Qty", 8.0).right(),
                    Column::new("unit", "Unit", 8.0),
                    Column::new("from", "Take from", 20.0),
                    Column::new("for", "For", 40.0),
                    Column::new("basis", "Basis", 16.0),
                ],
            );
            for p in plan.picks_in(phase) {
                s.push(
                    vec![
                        Cell::text(KITTING_BOX),
                        Cell::text(p.item.clone()),
                        Cell::Number((p.qty * 100.0).round() / 100.0),
                        Cell::text(p.unit),
                        Cell::text(p.from.clone()),
                        Cell::text(p.for_.clone()),
                        Cell::text(p.basis.text()),
                    ],
                    Vec::new(),
                );
            }
            if !s.rows.is_empty() {
                sections.push(s);
            }
        }
        for saw in &plan.saw {
            let mut s = Section::new(
                &format!("saw-{}", saw.key),
                &format!("Saw plan  {}", saw.name),
                vec![
                    Column::new("bar", "Bar", 6.0).right(),
                    Column::new("from", "Cut from", 26.0),
                    Column::new("piece", "Piece", 22.0),
                    Column::new("length_mm", "Cut mm", 10.0).right(),
                    Column::new("before_mm", "Bar piece mm", 12.0).right(),
                    Column::new("left_mm", "Left mm", 10.0).right(),
                    Column::new("keep", "Leftover", 10.0),
                    Column::new("done", "Done", 7.0),
                ],
            );
            for bar in &saw.bars {
                let from = match bar.source {
                    Source::New(l) => format!("New bar {} mm", num(l)),
                    Source::Held(l) => format!("Offcut {} mm", num(l)),
                };
                for (i, cut) in bar.cuts.iter().enumerate() {
                    let last = i + 1 == bar.cuts.len();
                    let piece = match cut.part {
                        Some((n, of)) => format!("{} (part {n} of {of})", cut.label),
                        None => cut.label.clone(),
                    };
                    s.push(
                        vec![
                            Cell::Int(bar.number as i64),
                            Cell::text(if i == 0 {
                                from.clone()
                            } else {
                                "Offcut of the cut above".into()
                            }),
                            Cell::text(piece),
                            Cell::Number(cut.length),
                            Cell::Number((cut.from_left * 10.0).round() / 10.0),
                            if last {
                                Cell::Number((bar.left * 10.0).round() / 10.0)
                            } else {
                                Cell::Empty
                            },
                            Cell::text(if !last || bar.left <= 0.0 {
                                ""
                            } else if bar.keep {
                                "keep"
                            } else {
                                "scrap"
                            }),
                            Cell::text(KITTING_BOX),
                        ],
                        Vec::new(),
                    );
                }
            }
            if !saw.plan.unused_held.is_empty() {
                let mut all = saw.plan.unused_held.clone();
                all.sort_by(|a, b| b.total_cmp(a));
                s.push(
                    vec![
                        Cell::Empty,
                        Cell::text("Not needed"),
                        Cell::text(format!(
                            "{} on hand stay in the bin: {}",
                            all.len(),
                            all.iter()
                                .map(|l| format!("{} mm", num(*l)))
                                .collect::<Vec<_>>()
                                .join(", ")
                        )),
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                        Cell::Empty,
                    ],
                    Vec::new(),
                );
            }
            sections.push(s);
        }
        if !plan.reels.is_empty() {
            let mut s = Section::new(
                "reels",
                "Wire reels",
                vec![
                    Column::new("group", "Wire", 24.0),
                    Column::new("reel", "Reel", 20.0),
                    Column::new("wires", "Wires", 8.0).right(),
                    Column::new("used_m", "Cut m", 9.0).right(),
                    Column::new("left_m", "Left m", 9.0).right(),
                    Column::new("basis", "Lengths", 14.0),
                    Column::new("done", "Done", 7.0),
                ],
            );
            for g in &plan.reels {
                for r in &g.reels {
                    let reel = match r.source {
                        Source::New(l) => format!("New reel {} m", num(l / 1000.0)),
                        Source::Held(l) => format!("Part reel {} m", num(l / 1000.0)),
                    };
                    s.push(
                        vec![
                            Cell::text(g.label.clone()),
                            Cell::text(reel),
                            Cell::Int(r.wires as i64),
                            Cell::Number((r.used_m * 100.0).round() / 100.0),
                            Cell::Number((r.left_m * 100.0).round() / 100.0),
                            Cell::text(if g.estimated > 0 {
                                "some estimated"
                            } else {
                                "routed"
                            }),
                            Cell::text(KITTING_BOX),
                        ],
                        Vec::new(),
                    );
                }
            }
            sections.push(s);
        }
        let mut offcuts = Section::new(
            "offcuts",
            "Offcuts to keep",
            vec![
                Column::new("material", "Material", 30.0),
                Column::new("length_mm", "Length mm", 12.0).right(),
                Column::new("from", "Left by", 40.0),
                Column::new("done", "In the bin", 10.0),
            ],
        );
        for saw in &plan.saw {
            for bar in saw.bars.iter().filter(|b| b.keep) {
                let last = bar.cuts.last().map(|c| c.label.clone()).unwrap_or_default();
                offcuts.push(
                    vec![
                        Cell::text(saw.name.clone()),
                        Cell::Number((bar.left * 10.0).round() / 10.0),
                        Cell::text(format!("bar {}, after the cut of {last}", bar.number)),
                        Cell::text(KITTING_BOX),
                    ],
                    Vec::new(),
                );
            }
        }
        for g in &plan.reels {
            for r in g
                .reels
                .iter()
                .filter(|r| r.left_m * 1000.0 >= super::plan::KEEP_REEL_MM)
            {
                offcuts.push(
                    vec![
                        Cell::text(format!("Wire {}", g.label)),
                        Cell::Number((r.left_m * 1000.0).round()),
                        Cell::text("part reel, back on the shelf"),
                        Cell::text(KITTING_BOX),
                    ],
                    Vec::new(),
                );
            }
        }
        if !offcuts.rows.is_empty() {
            sections.push(offcuts);
        }
        let mut shopping = Section::new(
            "shopping",
            "Shopping list (bill of materials and stock)",
            vec![
                Column::new("source", "From", 8.0),
                Column::new("item", "Item", 42.0),
                Column::new("order_number", "Order number", 22.0),
                Column::new("need", "Need", 9.0).right(),
                Column::new("unit", "Unit", 7.0),
                Column::new("have", "On hand", 9.0).right(),
                Column::new("buy", "Buy", 9.0).right(),
                Column::new("note", "Note", 36.0),
            ],
        );
        for l in &plan.shopping {
            shopping.push(
                vec![
                    Cell::text(l.shop.text()),
                    Cell::text(l.item.clone()),
                    Cell::text(l.order_number.clone()),
                    Cell::Number((l.need * 100.0).round() / 100.0),
                    Cell::text(l.unit),
                    Cell::Number((l.have * 100.0).round() / 100.0),
                    Cell::Number((l.buy * 100.0).round() / 100.0),
                    Cell::text(l.note.clone()),
                ],
                Vec::new(),
            );
        }
        if !shopping.rows.is_empty() {
            sections.push(shopping);
        }
        let mut notes = Section::new(
            "notes",
            "What is exact and what is a planning value",
            vec![Column::new("note", "Note", 120.0)],
        );
        let s = &plan.settings;
        let mut lines = vec![
            "From the drawing: the rails, ducts, devices, parts, wires, ferrules, drill holes and labels counted here.".to_owned(),
            format!(
                "Planning values you set: bars of {} mm (rail) and {} mm (duct), saw cut {} mm, offcuts of {} mm or more kept, wire reels of {} m with {} % held back, spares as in the ferrule standard.",
                s.rail_stock_mm.iter().map(|l| num(*l)).collect::<Vec<_>>().join("/"),
                s.duct_stock_mm.iter().map(|l| num(*l)).collect::<Vec<_>>().join("/"),
                num(s.kerf_mm),
                num(s.min_offcut_mm),
                num(s.reel_m),
                num(s.wire_allowance_percent),
            ),
            "Bars are bought in the least length that covers every piece with the saw cuts; the plan is the cheapest found by an exact search, and a note says when the search stopped early.".to_owned(),
            "Screws are counted one per drilled hole; their size and length are not chosen here.".to_owned(),
        ];
        lines.extend(plan.problems.iter().cloned());
        for l in lines {
            notes.push(vec![Cell::text(l)], Vec::new());
        }
        sections.push(notes);
        ReportOutput {
            report: Self::ID.into(),
            title: "Kitting and shopping list".into(),
            revision,
            sections,
        }
    }
}

impl Report for Kitting {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Kitting and shopping list".into()
    }

    fn dependencies(&self) -> Dependencies {
        use EntityKind::*;
        Dependencies::on(&[
            Page,
            Device,
            Symbol,
            Junction,
            Wire,
            Potential,
            Terminal,
            Cable,
            Structure,
            Rail,
            CabinetPlacement,
            Part,
        ])
        .with_info()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let plan = kit_plan(project, &KitSettings::of(project));
        Kitting::build(&plan, project.revision())
    }
}
