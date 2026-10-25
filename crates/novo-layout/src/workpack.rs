//! Workshop pack: the build order of a cabinet as work cards for people who
//! cut wires and drill holes by hand.
//!
//! One card per rail, duct and device step of `assembly::steps`, one card per
//! chunk of wires of a cut group, one card per chunk of labels. A card names
//! the tools, lists the holes as distances from the nearest plate edges, and
//! carries the geometry of a hand template (the part's footprint or the rail
//! strip with the holes and cut-out marked) that the PDF draws at true size
//! where it fits a half page, and at a stated smaller scale where it does not.
//!
//! Nothing here knows PDF or the project model. The caller hands in the
//! layout, the device features, the wire cut groups and the device names; the
//! pack is a function of those, so the same cabinet always gives the same
//! cards. Positions are the layout's, not measured on a plate.

use std::collections::BTreeMap;

use crate::assembly::{Phase, WireJob, steps};
use crate::drill::{CutoutShape, DeviceFeatures, DrillTable, Surface, drill_table, mm, rail_holes};
use crate::state::CabinetLayout;
use crate::{Placement, RAIL_BAND_MM};

/// Wire rows on one card; more wires continue on the next card.
pub const WIRES_PER_CARD: usize = 9;
/// Label rows on one card.
pub const LABELS_PER_CARD: usize = 12;
/// Wires listed on a device card before "and n more".
pub const WIRES_ON_DEVICE_CARD: usize = 6;

/// One wire to cut, as the cut list gives it.
#[derive(Debug, Clone, PartialEq)]
pub struct WireCut {
    pub number: String,
    pub from: String,
    pub to: String,
    /// Size and colour, `1.5 mm2 black`.
    pub kind: String,
    /// Length to cut in mm, end allowances included.
    pub cut_mm: f64,
    /// The length is an estimate, not routed through the ducts.
    pub estimated: bool,
}

/// The wires of one group of the build order (a circuit), in the order they
/// are to be run.
#[derive(Debug, Clone, PartialEq)]
pub struct WireGroup {
    /// `Power`, `Control`.
    pub title: String,
    pub wires: Vec<WireCut>,
}

/// A hole of a hand template, in the template's own millimetres (y down).
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateHole {
    pub x: f64,
    pub y: f64,
    pub dia: f64,
    /// Number on the tiled drilling template, 0 if none.
    pub no: usize,
}

/// The cut-out of a hand template, centre in template millimetres.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateCutout {
    pub shape: CutoutShape,
    pub x: f64,
    pub y: f64,
}

/// What a hand template shows: a frame of `width` x `height` mm with holes.
#[derive(Debug, Clone, PartialEq)]
pub struct Template {
    pub width: f64,
    pub height: f64,
    /// Caption: `Rail R1, 400 mm` or `-K1 footprint`.
    pub caption: String,
    /// Where the template's top left goes, in words.
    pub origin: String,
    /// The frame is a DIN rail (drawn with its centre line).
    pub rail: bool,
    pub holes: Vec<TemplateHole>,
    pub cutout: Option<TemplateCutout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardKind {
    Rail,
    Duct,
    /// A device clipped on a rail.
    Clip,
    /// A device screwed to a surface.
    Screw,
    Wires,
    Labels,
}

/// One half page of the pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    pub kind: CardKind,
    /// First and last build order step the card covers.
    pub steps: (usize, usize),
    /// Steps in the whole build order.
    pub total: usize,
    pub phase: Phase,
    pub mark: String,
    pub text: String,
    pub tools: Vec<String>,
    pub template: Option<Template>,
    /// Hole and position lines, in print order.
    pub dims: Vec<String>,
    pub wires: Vec<WireCut>,
    /// Wires beyond the ones listed.
    pub wires_more: usize,
    /// Label texts of a label card.
    pub labels: Vec<String>,
    pub notes: Vec<String>,
    /// Boxes to tick at the foot of the card.
    pub ticks: Vec<String>,
}

impl Card {
    /// `12` or `35-43`.
    pub fn steps_text(&self) -> String {
        if self.steps.0 == self.steps.1 {
            self.steps.0.to_string()
        } else {
            format!("{}-{}", self.steps.0, self.steps.1)
        }
    }
}

/// One line of the pack's tool list.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolLine {
    pub tool: String,
    pub for_what: String,
}

/// One row of the tick-off overview: a step of the build order.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRow {
    pub step: usize,
    pub phase: Phase,
    pub mark: String,
    pub text: String,
}

/// The workshop pack of one cabinet.
#[derive(Debug, Clone, PartialEq)]
pub struct Pack {
    pub cabinet: String,
    pub plate: (f64, f64),
    pub overview: Vec<OverviewRow>,
    pub tools: Vec<ToolLine>,
    pub cards: Vec<Card>,
    /// Pattern text that could not be read, as the drill table reports it.
    pub problems: Vec<String>,
}

/// The distance of a point from the nearest edges of a `width` x `height`
/// surface, as a bench sentence: `12.5 mm from the left edge, 40 mm from the
/// top edge`. The nearest edge is the one a tape measure reaches first.
pub fn from_edges(x: f64, y: f64, (width, height): (f64, f64)) -> String {
    let (h_name, h) = if x <= width - x {
        ("left", x)
    } else {
        ("right", width - x)
    };
    let (v_name, v) = if y <= height - y {
        ("top", y)
    } else {
        ("bottom", height - y)
    };
    format!(
        "{} mm from the {h_name} edge, {} mm from the {v_name} edge",
        mm(h),
        mm(v)
    )
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Turns an offset inside the unrotated footprint into one inside the placed
/// footprint, relative to its top left (rotation is 90 degrees clockwise on
/// the y-down plate, as `drill_table` does).
fn local(dx: f64, dy: f64, p: &Placement) -> (f64, f64) {
    if p.rotated {
        (p.size.height - dy, dx)
    } else {
        (dx, dy)
    }
}

fn surface_size(layout: &CabinetLayout, surface: &Surface) -> (f64, f64) {
    match surface {
        Surface::Plate => (layout.plate.width, layout.plate.height),
        Surface::Door => layout
            .surface_sizes
            .get("door")
            .copied()
            .unwrap_or((layout.plate.width, layout.plate.height)),
        Surface::Named(n) => layout
            .surface_sizes
            .get(n)
            .copied()
            .unwrap_or((layout.plate.width, layout.plate.height)),
    }
}

/// The number the tiled drilling template gives the hole at `(x, y)` of
/// `source` on `surface`, 0 when the table has no such hole.
fn hole_no(table: &DrillTable, surface: &Surface, source: &str, x: f64, y: f64) -> usize {
    table
        .holes
        .iter()
        .find(|h| {
            h.surface == *surface
                && h.source == source
                && (h.x - round2(x)).abs() < 0.011
                && (h.y - round2(y)).abs() < 0.011
        })
        .map_or(0, |h| h.no)
}

fn hole_line(no: usize, dia: f64, x: f64, y: f64, size: (f64, f64)) -> String {
    let tag = if no > 0 {
        format!("No. {no}  ")
    } else {
        String::new()
    };
    format!("{tag}dia {}: {}", mm(dia), from_edges(x, y, size))
}

fn ticks(labels: &[&str]) -> Vec<String> {
    labels.iter().map(|s| (*s).to_owned()).collect()
}

fn wire_text(w: &WireCut) -> String {
    format!(
        "Run {} to {}, cut {}{} mm",
        w.from,
        w.to,
        if w.estimated { "about " } else { "" },
        mm(w.cut_mm)
    )
}

/// Builds the pack. `name` gives a device's tag. `groups` are the wire cut
/// groups in the order the wires are to be run.
pub fn build(
    cabinet: &str,
    layout: &CabinetLayout,
    features: &[DeviceFeatures],
    name: &dyn Fn(u64) -> String,
    groups: &[WireGroup],
) -> Pack {
    let table = drill_table(layout, features);
    let jobs: Vec<WireJob> = groups
        .iter()
        .flat_map(|g| &g.wires)
        .map(|w| WireJob {
            name: w.number.clone(),
            text: wire_text(w),
        })
        .collect();
    let list = steps(layout, name, &jobs);
    let total = list.len();
    let plate = (layout.plate.width, layout.plate.height);

    let overview = list
        .iter()
        .map(|s| OverviewRow {
            step: s.number,
            phase: s.phase,
            mark: s.mark.clone(),
            text: s.text.clone(),
        })
        .collect();

    // Wires by the device they end at, for the device cards.
    let mut device_wires: BTreeMap<String, Vec<WireCut>> = BTreeMap::new();
    for w in groups.iter().flat_map(|g| &g.wires) {
        for end in [&w.from, &w.to] {
            let tag = end.split(':').next().unwrap_or_default();
            if !tag.is_empty() {
                let list = device_wires.entry(tag.to_owned()).or_default();
                if !list.contains(w) {
                    list.push(w.clone());
                }
            }
        }
    }

    let features_of: BTreeMap<u64, &DeviceFeatures> = features.iter().map(|f| (f.id, f)).collect();
    let placement_of: BTreeMap<u64, &Placement> =
        layout.placements.iter().map(|p| (p.id, p)).collect();

    let mut rails: Vec<&crate::Rail> = layout.rails.iter().collect();
    rails.sort_by(|a, b| (a.y, a.x, a.id).partial_cmp(&(b.y, b.x, b.id)).unwrap());
    let rail_index: BTreeMap<u64, usize> = rails
        .iter()
        .enumerate()
        .map(|(i, r)| (r.id, i + 1))
        .collect();

    let mut cards: Vec<Card> = Vec::new();
    let mut wire_steps: Vec<usize> = Vec::new();
    let mut label_steps: Vec<(usize, String)> = Vec::new();

    for step in &list {
        match step.phase {
            Phase::Rails => {
                let Some(rail) = rails.iter().find(|r| Some(r.id) == step.subject) else {
                    continue;
                };
                let i = rail_index[&rail.id];
                let holes = rail_holes(rail);
                let source = format!("Rail {i}");
                let tmpl_holes: Vec<TemplateHole> = holes
                    .iter()
                    .map(|&(x, y)| TemplateHole {
                        x: if rail.vertical {
                            y - rail.y
                        } else {
                            x - rail.x
                        },
                        y: RAIL_BAND_MM / 2.0,
                        dia: crate::drill::RAIL_HOLE_DIA_MM,
                        no: hole_no(&table, &Surface::Plate, &source, x, y),
                    })
                    .collect();
                let dims: Vec<String> = holes
                    .iter()
                    .zip(&tmpl_holes)
                    .map(|(&(x, y), h)| hole_line(h.no, h.dia, x, y, plate))
                    .collect();
                cards.push(Card {
                    kind: CardKind::Rail,
                    steps: (step.number, step.number),
                    total,
                    phase: step.phase,
                    mark: step.mark.clone(),
                    text: step.text.clone(),
                    tools: vec![
                        format!("Rail cutter or hacksaw: cut to {} mm", mm(rail.length)),
                        format!(
                            "Centre punch, drill dia {} mm: {} holes",
                            mm(crate::drill::RAIL_HOLE_DIA_MM),
                            holes.len()
                        ),
                        "Tape measure, pencil".to_owned(),
                    ],
                    template: Some(Template {
                        width: rail.length,
                        height: RAIL_BAND_MM,
                        caption: format!("{} rail strip, {} mm", step.mark, mm(rail.length)),
                        origin: if rail.vertical {
                            "Strip top = rail start (top end); the rail stands upright".to_owned()
                        } else {
                            "Strip left = rail start (left end)".to_owned()
                        },
                        rail: true,
                        holes: tmpl_holes,
                        cutout: None,
                    }),
                    dims,
                    wires: Vec::new(),
                    wires_more: 0,
                    labels: Vec::new(),
                    notes: vec![format!(
                        "Rail starts at x {}, y {} on the plate ({}).",
                        mm(rail.x),
                        mm(rail.y),
                        from_edges(rail.x, rail.y, plate)
                    )],
                    ticks: ticks(&["Cut", "Holes drilled", "Rail fitted"]),
                });
            }
            Phase::Ducts => {
                let Some(duct) = layout.ducts.iter().find(|d| Some(d.id) == step.subject) else {
                    continue;
                };
                let f = duct.footprint();
                cards.push(Card {
                    kind: CardKind::Duct,
                    steps: (step.number, step.number),
                    total,
                    phase: step.phase,
                    mark: step.mark.clone(),
                    text: step.text.clone(),
                    tools: vec![
                        format!(
                            "Duct cutter or hacksaw: cut to {} mm ({} mm wide)",
                            mm(duct.length),
                            mm(duct.width)
                        ),
                        "Tape measure, pencil".to_owned(),
                    ],
                    template: Some(Template {
                        width: duct.length,
                        height: duct.width,
                        caption: format!("{} duct, {} x {} mm", step.mark, mm(duct.length), mm(duct.width)),
                        origin: if duct.vertical {
                            "Strip top = duct start (top end); the duct stands upright".to_owned()
                        } else {
                            "Strip left = duct start (left end)".to_owned()
                        },
                        rail: false,
                        holes: Vec::new(),
                        cutout: None,
                    }),
                    dims: vec![format!(
                        "Top left corner: {}",
                        from_edges(f.x, f.y, plate)
                    )],
                    wires: Vec::new(),
                    wires_more: 0,
                    labels: Vec::new(),
                    notes: vec![
                        "The duct is placed from the layout; the fixing points are the duct maker's."
                            .to_owned(),
                    ],
                    ticks: ticks(&["Cut", "Duct fitted"]),
                });
            }
            Phase::Devices => {
                let Some(p) = step.subject.and_then(|id| placement_of.get(&id).copied()) else {
                    continue;
                };
                let tag = name(p.id);
                let feat = features_of.get(&p.id).copied();
                let surface = feat.map_or(Surface::Plate, |f| f.surface.clone());
                let size = surface_size(layout, &surface);
                let fp = p.footprint();
                let mut tools: Vec<String> = Vec::new();
                let mut dims: Vec<String> = Vec::new();
                let mut notes: Vec<String> = Vec::new();
                let mut tmpl_holes = Vec::new();
                let mut cutout = None;
                if let Some(f) = feat {
                    for h in &f.holes {
                        let (lx, ly) = local(h.dx, h.dy, p);
                        let (x, y) = (fp.x + lx, fp.y + ly);
                        let no = hole_no(&table, &f.surface, &f.label, x, y);
                        dims.push(hole_line(no, h.dia, x, y, size));
                        tmpl_holes.push(TemplateHole {
                            x: lx,
                            y: ly,
                            dia: h.dia,
                            no,
                        });
                    }
                    if let Some(c) = &f.cutout {
                        let (cx, cy) = c
                            .centre
                            .unwrap_or((p.size.width / 2.0, p.size.height / 2.0));
                        let (lx, ly) = local(cx, cy, p);
                        let shape = match (c.shape, p.rotated) {
                            (CutoutShape::Rect { w, h }, true) => CutoutShape::Rect { w: h, h: w },
                            (s, _) => s,
                        };
                        dims.push(format!(
                            "Cut-out {}: centre {}",
                            shape.describe(),
                            from_edges(fp.x + lx, fp.y + ly, size)
                        ));
                        tools.push(match shape {
                            CutoutShape::Round { dia } => {
                                format!("Hole saw or step drill, dia {} mm", mm(dia))
                            }
                            CutoutShape::Rect { w, h } => {
                                format!("Punch or jigsaw for {} x {} mm", mm(w), mm(h))
                            }
                        });
                        cutout = Some(TemplateCutout {
                            shape,
                            x: lx,
                            y: ly,
                        });
                    }
                    notes.extend(f.problems.iter().cloned());
                }
                let mut dias: Vec<f64> = tmpl_holes.iter().map(|h| h.dia).collect();
                dias.sort_by(f64::total_cmp);
                dias.dedup();
                for d in dias {
                    let n = tmpl_holes.iter().filter(|h| h.dia == d).count();
                    tools.push(format!("Drill dia {} mm: {n} holes", mm(d)));
                }
                if !tools.is_empty() {
                    tools.push("Centre punch".to_owned());
                }
                tools.push("Screwdriver".to_owned());
                let on_rail = p.rail.and_then(|r| rail_index.get(&r));
                let kind = if on_rail.is_some() {
                    CardKind::Clip
                } else {
                    CardKind::Screw
                };
                if surface != Surface::Plate {
                    notes.push(format!(
                        "On the {}: measure from that surface's corner, not the mounting plate's.",
                        surface.name().to_lowercase()
                    ));
                } else if on_rail.is_none() {
                    dims.insert(
                        0,
                        format!(
                            "Top left corner of the device: {}",
                            from_edges(fp.x, fp.y, size)
                        ),
                    );
                }
                if tmpl_holes.is_empty() && cutout.is_none() && on_rail.is_some() {
                    notes.push("Nothing to drill: the rail holds it.".to_owned());
                }
                let mut wires = device_wires.get(&tag).cloned().unwrap_or_default();
                let wires_more = wires.len().saturating_sub(WIRES_ON_DEVICE_CARD);
                wires.truncate(WIRES_ON_DEVICE_CARD);
                let template = (!tmpl_holes.is_empty() || cutout.is_some()).then(|| Template {
                    width: fp.w,
                    height: fp.h,
                    caption: format!("{tag} footprint, {} x {} mm", mm(fp.w), mm(fp.h)),
                    origin: format!(
                        "Template top left = device top left{}",
                        if p.rotated {
                            " (device turned 90 degrees)"
                        } else {
                            ""
                        }
                    ),
                    rail: false,
                    holes: tmpl_holes,
                    cutout,
                });
                let mut tick = vec!["Fitted"];
                if template.is_some() {
                    tick.insert(0, "Drilled");
                }
                tick.push("Checked");
                cards.push(Card {
                    kind,
                    steps: (step.number, step.number),
                    total,
                    phase: step.phase,
                    mark: step.mark.clone(),
                    text: step.text.clone(),
                    tools,
                    template,
                    dims,
                    wires,
                    wires_more,
                    labels: Vec::new(),
                    notes,
                    ticks: ticks(&tick),
                });
            }
            Phase::Wires => wire_steps.push(step.number),
            Phase::Labels => label_steps.push((step.number, step.mark.clone())),
        }
    }

    // Wire cards: per group, chunks of WIRES_PER_CARD, steps numbered in the
    // order `steps` listed the wires.
    let mut next = 0;
    for g in groups {
        for chunk in g.wires.chunks(WIRES_PER_CARD) {
            let first = wire_steps.get(next).copied().unwrap_or(total);
            let last = wire_steps
                .get(next + chunk.len() - 1)
                .copied()
                .unwrap_or(first);
            next += chunk.len();
            cards.push(Card {
                kind: CardKind::Wires,
                steps: (first, last),
                total,
                phase: Phase::Wires,
                mark: g.title.clone(),
                text: format!(
                    "Wire group {}: cut and run the wires below. Cut lengths include the end allowances of the project's wire rules; ~ marks an estimated length, cut it long and trim.",
                    g.title
                ),
                tools: vec![
                    "Wire cutter and stripper".to_owned(),
                    "Ferrule or lug crimp tool".to_owned(),
                    "Tape measure".to_owned(),
                ],
                template: None,
                dims: Vec::new(),
                wires: chunk.to_vec(),
                wires_more: 0,
                labels: Vec::new(),
                notes: Vec::new(),
                ticks: ticks(&["Cut", "Run"]),
            });
        }
    }

    for chunk in label_steps.chunks(LABELS_PER_CARD) {
        cards.push(Card {
            kind: CardKind::Labels,
            steps: (chunk[0].0, chunk[chunk.len() - 1].0),
            total,
            phase: Phase::Labels,
            mark: "Labels".to_owned(),
            text: "Fit the label of each device, in mounting order.".to_owned(),
            tools: vec!["Labels from the labels sheet".to_owned()],
            template: None,
            dims: Vec::new(),
            wires: Vec::new(),
            wires_more: 0,
            labels: chunk.iter().map(|(_, m)| m.clone()).collect(),
            notes: Vec::new(),
            ticks: ticks(&["Fitted"]),
        });
    }

    let tools = tool_list(&cards, &table, groups);
    Pack {
        cabinet: cabinet.to_owned(),
        plate,
        overview,
        tools,
        cards,
        problems: table.problems.clone(),
    }
}

/// The tools the whole pack needs, one line per tool and size.
fn tool_list(cards: &[Card], table: &DrillTable, groups: &[WireGroup]) -> Vec<ToolLine> {
    let mut out = Vec::new();
    let summary = table.summary();
    for (surface, dia, n) in &summary {
        out.push(ToolLine {
            tool: format!("Drill dia {} mm", mm(*dia)),
            for_what: format!("{n} holes in the {}", surface.name().to_lowercase()),
        });
    }
    if !summary.is_empty() {
        out.push(ToolLine {
            tool: "Centre punch, hammer".to_owned(),
            for_what: "marking every hole through the template".to_owned(),
        });
    }
    for c in &table.cutouts {
        out.push(ToolLine {
            tool: match c.shape {
                CutoutShape::Round { dia } => format!("Hole saw or step drill, dia {} mm", mm(dia)),
                CutoutShape::Rect { w, h } => {
                    format!("Punch or jigsaw, {} x {} mm", mm(w), mm(h))
                }
            },
            for_what: format!(
                "cut-out for {} on the {}",
                c.source,
                c.surface.name().to_lowercase()
            ),
        });
    }
    let mut cuts: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in cards {
        if matches!(c.kind, CardKind::Rail | CardKind::Duct)
            && let Some(t) = &c.template
        {
            let what = if c.kind == CardKind::Rail {
                "DIN rail"
            } else {
                "wiring duct"
            };
            cuts.entry(format!("{what} {} mm", mm(t.width)))
                .or_default()
                .push(c.mark.clone());
        }
    }
    for (cut, marks) in cuts {
        out.push(ToolLine {
            tool: "Rail cutter or hacksaw".to_owned(),
            for_what: format!("{cut}: {}", marks.join(", ")),
        });
    }
    out.push(ToolLine {
        tool: "Tape measure, pencil, screwdriver".to_owned(),
        for_what: "every step".to_owned(),
    });
    let wires: usize = groups.iter().map(|g| g.wires.len()).sum();
    if wires > 0 {
        out.push(ToolLine {
            tool: "Wire cutter, stripper and crimp tool".to_owned(),
            for_what: format!(
                "{wires} wire{} in {} group{}",
                if wires == 1 { "" } else { "s" },
                groups.len(),
                if groups.len() == 1 { "" } else { "s" }
            ),
        });
    }
    out
}
