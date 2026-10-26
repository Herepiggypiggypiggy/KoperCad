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
/// Clipped devices the rail card names in words; all of them are drawn.
pub const CLIPS_LISTED: usize = 6;

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
    /// What each end adds to the routed length (strip plus ferrule collar
    /// or lug barrel, the project's wire rules), mm; already in `cut_mm`.
    /// `None` when the cut list gave none.
    pub end_mm: Option<f64>,
    /// `ferrule` or `lug` when the allowance says which (the wire rules give
    /// the two different values), else empty.
    pub fitting: &'static str,
    /// Which of several wires carrying this number this is, `(1, 2)`: a
    /// number runs on through a terminal, so two rows can share it. `None`
    /// when the number is used once.
    pub part: Option<(usize, usize)>,
    /// The insulation to strip at the `from` end and at the `to` end, mm,
    /// as the cut list's own wire-cut jobs decide it (the part's or the
    /// wire's stated length first, else the typical one for the size).
    /// `None` when the size is not known.
    pub strip_from_mm: Option<f64>,
    pub strip_to_mm: Option<f64>,
    /// Where the strip length came from: `typical`, `part` or `stated`;
    /// empty when there is none.
    pub strip_basis: &'static str,
}

impl WireCut {
    /// `1 of 2` for a number used on several wires, empty otherwise.
    pub fn part_text(&self) -> String {
        self.part
            .map(|(i, n)| format!("{i} of {n}"))
            .unwrap_or_default()
    }

    /// `strip 10 mm typ.` (`part`, `stated` for the others), `strip 8 / 10 mm`
    /// from end / to end when the ends differ; empty without a strip length
    /// and for a lug, whose barrel length counts, not a strip length.
    pub fn strip_text(&self) -> String {
        if self.fitting == "lug" {
            return String::new();
        }
        let word = match self.strip_basis {
            "typical" => "typ.",
            other => other,
        };
        self.strip_with(word)
    }

    fn strip_with(&self, word: &str) -> String {
        let say = |v: Option<f64>| v.map_or("?".to_owned(), mm);
        let word = if word.is_empty() {
            String::new()
        } else {
            format!(" {word}")
        };
        match (self.strip_from_mm, self.strip_to_mm) {
            (None, None) => String::new(),
            (Some(a), Some(b)) if (a - b).abs() < 0.05 => format!("strip {} mm{word}", mm(a)),
            // The ends differ, or one is not known: both, with the source.
            (a, b) => format!("strip {}/{} mm{word}", say(a), say(b)),
        }
    }

    /// `10 mm ferrule`, `20 mm lug`, `10 mm` when the fitting is not known,
    /// empty without an allowance.
    pub fn end_text(&self) -> String {
        match self.end_mm {
            Some(e) if e > 0.0 => {
                if self.fitting.is_empty() {
                    format!("{} mm", mm(e))
                } else {
                    format!("{} mm {}", mm(e), self.fitting)
                }
            }
            _ => String::new(),
        }
    }
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

/// A device clipped on a rail strip: where it sits along the rail.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateClip {
    /// Distance of the device's start from the rail's start, mm.
    pub at: f64,
    /// Its extent along the rail, mm.
    pub len: f64,
    /// The device tag as shown on the drawing.
    pub tag: String,
}

impl TemplateClip {
    /// The short designation drawn on the strip: `K1` of `=A1+CAB1-K1`.
    pub fn short(&self) -> &str {
        self.tag.rsplit('-').next().unwrap_or(&self.tag)
    }
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
    /// Devices clipped on this rail strip, left to right.
    pub clips: Vec<TemplateClip>,
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
    /// A cut-out in a wall or the door (a grille or a fan); outside the
    /// build order, so its step numbers are 0.
    Opening,
    /// Wires with no end inside the plated cabinet (supply and field wires):
    /// not a build order step, so its step numbers are 0.
    Outside,
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

/// FNV-1a over a card's printed content. Stable across runs and platforms
/// (no random hasher), so it can be compared with the value of an earlier print.
fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

impl Card {
    /// Identifies what the card prints: it changes when the card's text,
    /// holes, template, wires or ticks change, and not otherwise.
    pub fn fingerprint(&self) -> u64 {
        fnv(&format!("{self:?}"))
    }
}

impl Pack {
    /// The pack's revision: six hex digits over the plate and every card in
    /// order. It is the layout state a print was made from, not a counter.
    pub fn revision(&self) -> String {
        let all = format!("{:?}|{:?}", self.plate, self.cards);
        format!("{:06x}", fnv(&all) & 0xff_ffff)
    }
}

/// What a print was made from: the date, each pack's revision and each
/// card's fingerprint. Kept by the app after an export and compared with the
/// current packs to say which cards changed since, and whether the printed
/// pack is out of date.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PrintRecord {
    /// `YYYY-MM-DD` of the print.
    pub date: String,
    /// Revision per cabinet tag.
    pub revisions: BTreeMap<String, String>,
    /// Fingerprint per (cabinet tag, card mark).
    pub cards: BTreeMap<(String, String), u64>,
}

/// The cards that differ from a print.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Changes {
    /// (cabinet, mark) of cards that are new or whose content differs.
    pub changed: Vec<(String, String)>,
    /// Cards of the print that the pack no longer has.
    pub removed: usize,
}

impl Changes {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.removed == 0
    }
}

impl PrintRecord {
    /// The record of printing `packs` on `date`.
    pub fn of(packs: &[Pack], date: &str) -> Self {
        let mut r = PrintRecord {
            date: date.to_owned(),
            ..Self::default()
        };
        for p in packs.iter().filter(|p| !p.cards.is_empty()) {
            r.revisions.insert(p.cabinet.clone(), p.revision());
            for c in &p.cards {
                r.cards
                    .insert((p.cabinet.clone(), c.mark.clone()), c.fingerprint());
            }
        }
        r
    }

    /// How the current `packs` differ from this print.
    pub fn changes(&self, packs: &[Pack]) -> Changes {
        let now = Self::of(packs, &self.date);
        let changed = now
            .cards
            .iter()
            .filter(|(k, v)| self.cards.get(*k) != Some(*v))
            .map(|(k, _)| k.clone())
            .collect();
        let removed = self
            .cards
            .keys()
            .filter(|k| !now.cards.contains_key(*k))
            .count();
        Changes { changed, removed }
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

/// A saw or punch size at the bench: up to the next 0.5 mm, so the cut-out
/// is never smaller than the part it must clear. A model value like 146.99
/// is float noise from the part data and prints as 147; a size already on a
/// half millimetre (within 0.0005 mm) stays.
pub fn half_mm(v: f64) -> f64 {
    ((v * 2.0 - 1e-3).ceil() / 2.0).max(0.0)
}

fn half_shape(s: CutoutShape) -> CutoutShape {
    match s {
        CutoutShape::Round { dia } => CutoutShape::Round { dia: half_mm(dia) },
        CutoutShape::Rect { w, h } => CutoutShape::Rect {
            w: half_mm(w),
            h: half_mm(h),
        },
    }
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

/// The rail card's drill lines: where each hole sits along the rail from its
/// start, and the pitch between neighbours with equal steps folded
/// (`164 x 2`), so one rule is walked along the rail once.
fn rail_pitch_lines(xs: &[f64], length: f64) -> Vec<String> {
    let mut xs = xs.to_vec();
    xs.sort_by(f64::total_cmp);
    if xs.is_empty() {
        return Vec::new();
    }
    let at: Vec<String> = xs.iter().map(|x| mm(*x)).collect();
    let mut lines = vec![format!(
        "Holes, mm from the rail start: {} (all on the centre line)",
        at.join(", ")
    )];
    let mut steps: Vec<(f64, usize)> = Vec::new();
    for w in xs.windows(2) {
        let d = w[1] - w[0];
        match steps.last_mut() {
            Some((p, n)) if (*p - d).abs() < 0.005 => *n += 1,
            _ => steps.push((d, 1)),
        }
    }
    if !steps.is_empty() {
        let parts: Vec<String> = steps
            .iter()
            .map(|(d, n)| {
                if *n > 1 {
                    format!("{} x {n}", mm(*d))
                } else {
                    mm(*d)
                }
            })
            .collect();
        lines.push(format!("Pitch between holes, mm: {}", parts.join(", ")));
    }
    let last = xs[xs.len() - 1];
    lines.push(format!(
        "Last hole to the rail end: {} mm",
        mm((length - last).max(0.0))
    ));
    lines
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
    rails.sort_by(|a, b| {
        a.y.total_cmp(&b.y)
            .then(a.x.total_cmp(&b.x))
            .then(a.id.cmp(&b.id))
    });
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
                let dims = rail_pitch_lines(
                    &tmpl_holes.iter().map(|h| h.x).collect::<Vec<_>>(),
                    rail.length,
                );
                // The devices clipped on this rail, along it from its start.
                let mut clips: Vec<TemplateClip> = layout
                    .placements
                    .iter()
                    .filter(|p| p.rail == Some(rail.id))
                    .map(|p| {
                        let fp = p.footprint();
                        let (at, len) = if rail.vertical {
                            (fp.y - rail.y, fp.h)
                        } else {
                            (fp.x - rail.x, fp.w)
                        };
                        TemplateClip {
                            at,
                            len,
                            tag: name(p.id),
                        }
                    })
                    .collect();
                clips.sort_by(|a, b| a.at.total_cmp(&b.at).then_with(|| a.tag.cmp(&b.tag)));
                let mut notes = vec![format!(
                    "Rail starts at x {}, y {} on the plate ({}).",
                    mm(rail.x),
                    mm(rail.y),
                    from_edges(rail.x, rail.y, plate)
                )];
                if !clips.is_empty() {
                    let shown: Vec<String> = clips
                        .iter()
                        .take(CLIPS_LISTED)
                        .map(|c| format!("{} at {}", c.tag, mm(c.at)))
                        .collect();
                    let more = clips.len().saturating_sub(CLIPS_LISTED);
                    notes.push(format!(
                        "Clipped on this rail, mm from its start: {}{}.",
                        shown.join(", "),
                        if more > 0 {
                            format!(" and {more} more (drawn on the strip)")
                        } else {
                            String::new()
                        }
                    ));
                }
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
                        clips,
                    }),
                    dims,
                    wires: Vec::new(),
                    wires_more: 0,
                    labels: Vec::new(),
                    notes,
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
                        caption: format!(
                            "{} duct, {} x {} mm",
                            step.mark,
                            mm(duct.length),
                            mm(duct.width)
                        ),
                        origin: if duct.vertical {
                            "Strip top = duct start (top end); the duct stands upright".to_owned()
                        } else {
                            "Strip left = duct start (left end)".to_owned()
                        },
                        rail: false,
                        holes: Vec::new(),
                        cutout: None,
                        clips: Vec::new(),
                    }),
                    dims: vec![format!("Top left corner: {}", from_edges(f.x, f.y, plate))],
                    wires: Vec::new(),
                    wires_more: 0,
                    labels: Vec::new(),
                    notes: vec![
                        "The duct is placed from the layout.".to_owned(),
                        duct_fixing_note(duct.length),
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
                        let shape = match (half_shape(c.shape), p.rotated) {
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
                                format!("Punch, nibbler or jigsaw for {} x {} mm", mm(w), mm(h))
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
                // The distance on the step is from the rail's start: say which
                // end that is here, the rail card may be out of reach.
                if let (Some(r), Some(i)) = (
                    p.rail.and_then(|id| rails.iter().find(|r| r.id == id)),
                    on_rail,
                ) {
                    notes.push(format!(
                        "R{i} starts at its {}, seen from the front: measure the distance from that end.",
                        if r.vertical {
                            "top end (the rail stands upright)"
                        } else {
                            "left end"
                        }
                    ));
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
                    clips: Vec::new(),
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
            tool: match half_shape(c.shape) {
                CutoutShape::Round { dia } => format!("Hole saw or step drill, dia {} mm", mm(dia)),
                CutoutShape::Rect { w, h } => {
                    format!("Punch, nibbler or jigsaw, {} x {} mm", mm(w), mm(h))
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

/// Margin of an opening's template frame around the cut-out outline, mm.
pub const OPENING_FRAME_MM: f64 = 10.0;

/// The wall of the enclosure an opening is cut into.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpeningWall {
    Door,
    Top,
    Bottom,
    Left,
    Right,
}

impl OpeningWall {
    pub fn name(self) -> &'static str {
        match self {
            OpeningWall::Door => "door",
            OpeningWall::Top => "top wall",
            OpeningWall::Bottom => "bottom wall",
            OpeningWall::Left => "left wall",
            OpeningWall::Right => "right wall",
        }
    }
}

/// A grille or fan cut-out as the Heat panel stores it. `along` is the
/// centre along the wall (x from the left edge for the door, top and bottom;
/// y from the top edge for a side wall), `down` the centre from the top edge
/// on the door only. `size` is the interior's width and height as seen
/// from the front, `depth` its depth.
#[derive(Debug, Clone, PartialEq)]
pub struct OpeningJob {
    pub name: String,
    /// `grille`, `fan` or `filter fan`.
    pub what: String,
    pub wall: OpeningWall,
    pub along: f64,
    pub down: Option<f64>,
    pub width: f64,
    pub height: f64,
    pub size: (f64, f64),
    pub depth: f64,
}

/// Wire cards for the cut-list wires that have no end inside this cabinet's
/// plate (supply feeds, field wires): outside the build order, after the
/// step cards, steps 0.
pub fn add_outside_wire_cards(pack: &mut Pack, wires: Vec<WireCut>) {
    for chunk in wires.chunks(WIRES_PER_CARD) {
        pack.cards.push(Card {
            kind: CardKind::Outside,
            steps: (0, 0),
            total: 0,
            phase: Phase::Wires,
            mark: "Wires outside the plate".to_owned(),
            text: "Supply and field wires: they have no end on this plate, so they are not steps of the build order. Cut them with the cabinet wires and run them last. Lengths are from the cut list; the ducts and the field are not measured, so cut long and trim (~).".to_owned(),
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

/// Moves the cut-out cards to the front of the pack (stable), so a bench
/// user working card by card cuts the walls first. They stay outside the
/// build order (steps 0).
fn cut_outs_first(pack: &mut Pack) {
    pack.cards.sort_by_key(|c| c.kind != CardKind::Opening);
}

/// Adds one card per opening in front of the cards of the build order, plus
/// the tool lines they need. The cards sit outside the build order (steps
/// 0): a wall is cut before the parts go in, so the text says so and they
/// are printed first.
pub fn add_opening_cards(pack: &mut Pack, openings: &[OpeningJob]) {
    for (i, o) in openings.iter().enumerate() {
        let mark = format!("{} {} ({})", capitalised(&o.what), i + 1, o.wall.name());
        let (w, h) = (half_mm(o.width), half_mm(o.height));
        let mut dims = Vec::new();
        let mut notes = Vec::new();
        let face = match o.wall {
            OpeningWall::Door => {
                let v = o.down.unwrap_or(o.size.1 / 2.0);
                dims.push(format!("Centre: {}", from_edges(o.along, v, o.size)));
                dims.push(format!(
                    "Left edge {} mm and right edge {} mm from the left; top edge {} mm and bottom edge {} mm from the top",
                    mm(o.along - w / 2.0),
                    mm(o.along + w / 2.0),
                    mm(v - h / 2.0),
                    mm(v + h / 2.0)
                ));
                notes.push(format!(
                    "Seen from outside with the door closed. Measured on the door from the corner of the interior opening ({} x {} mm), not from the outer door edge: add the door's flange.",
                    mm(o.size.0),
                    mm(o.size.1)
                ));
                "the door"
            }
            OpeningWall::Top | OpeningWall::Bottom => {
                dims.push(format!(
                    "Centre: {} mm from the left edge of the interior, seen from the front ({} mm wide)",
                    mm(o.along),
                    mm(o.size.0)
                ));
                dims.push(format!(
                    "Left edge {} mm, right edge {} mm from the left",
                    mm(o.along - w / 2.0),
                    mm(o.along + w / 2.0)
                ));
                dims.push(format!(
                    "Across the wall the cut-out is {} mm deep (wall {} mm deep)",
                    mm(h),
                    mm(o.depth)
                ));
                notes.push("The cabinet model gives no position across the wall's depth: centre it by eye or put it where the wall is free.".to_owned());
                if o.wall == OpeningWall::Top {
                    "the roof"
                } else {
                    "the floor"
                }
            }
            OpeningWall::Left | OpeningWall::Right => {
                dims.push(format!(
                    "Centre: {} mm from the top edge of the interior ({} mm high)",
                    mm(o.along),
                    mm(o.size.1)
                ));
                dims.push(format!(
                    "Top edge {} mm, bottom edge {} mm from the top",
                    mm(o.along - w / 2.0),
                    mm(o.along + w / 2.0)
                ));
                dims.push(format!(
                    "Across the wall the cut-out is {} mm deep (wall {} mm deep)",
                    mm(h),
                    mm(o.depth)
                ));
                notes.push("The cabinet model gives no position across the wall's depth: centre it by eye or put it where the wall is free.".to_owned());
                if o.wall == OpeningWall::Left {
                    "the left wall"
                } else {
                    "the right wall"
                }
            }
        };
        if o.what.contains("fan") {
            notes.push("The Heat panel stores a rectangle: a round fan cut-out and the fan's screw holes come from the fan's datasheet.".to_owned());
        }
        let side = matches!(o.wall, OpeningWall::Left | OpeningWall::Right);
        // On a side wall the model's width runs along y: draw it that way.
        let (tw, th) = if side { (h, w) } else { (w, h) };
        let (cw, ch) = (tw + 2.0 * OPENING_FRAME_MM, th + 2.0 * OPENING_FRAME_MM);
        pack.cards.push(Card {
            kind: CardKind::Opening,
            steps: (0, 0),
            total: 0,
            phase: Phase::Devices,
            mark,
            text: format!(
                "Cut-out for a {} in {}: {} x {} mm. It is not a step of the build order: cut it before the plate goes in, with the door or wall off the cabinet.",
                o.what,
                face,
                mm(w),
                mm(h)
            ),
            tools: vec![
                format!("Punch, nibbler or jigsaw for {} x {} mm", mm(w), mm(h)),
                "Drill for the corner starter holes, file for the edges".to_owned(),
                "Tape measure, pencil, centre punch".to_owned(),
            ],
            template: Some(Template {
                width: cw,
                height: ch,
                caption: format!("{} cut-out {} x {} mm", o.what, mm(w), mm(h)),
                origin: frame_note(cw, ch),
                rail: false,
                holes: Vec::new(),
                cutout: Some(TemplateCutout {
                    shape: CutoutShape::Rect { w: tw, h: th },
                    x: cw / 2.0,
                    y: ch / 2.0,
                }),
                clips: Vec::new(),
            }),
            dims,
            wires: Vec::new(),
            wires_more: 0,
            labels: Vec::new(),
            notes,
            ticks: ticks(&["Marked", "Cut", "Edges filed", "Fitted"]),
        });
    }
    cut_outs_first(pack);
    // One tool line per size and wall, counted, as the holes are listed.
    let mut groups: Vec<(String, String, usize)> = Vec::new();
    for o in openings {
        let tool = format!(
            "Punch, nibbler or jigsaw for {} x {} mm",
            mm(half_mm(o.width)),
            mm(half_mm(o.height))
        );
        let for_what = format!("{} cut-out in the {}", o.what, o.wall.name());
        match groups.iter_mut().find(|g| g.0 == tool && g.1 == for_what) {
            Some(g) => g.2 += 1,
            None => groups.push((tool, for_what, 1)),
        }
    }
    let at = pack
        .tools
        .iter()
        .position(|t| t.for_what == "every step")
        .unwrap_or(pack.tools.len());
    for (k, (tool, for_what, n)) in groups.into_iter().enumerate() {
        pack.tools.insert(
            at + k,
            ToolLine {
                tool,
                for_what: if n > 1 {
                    format!("{n} x {for_what}")
                } else {
                    for_what
                },
            },
        );
    }
}

/// How a surface's own coordinates run, in the model's frames (`surfaces.rs`):
/// where x and y are measured from and which way the surface is seen. A wall
/// or the roof does not start at "the left edge" the way a door does.
struct Frame {
    x_from: &'static str,
    y_from: &'static str,
    /// Plate and door are told by the nearest edge; the others by their origin.
    nearest_edges: bool,
    view: &'static str,
}

fn frame_of(name: &str) -> Frame {
    use crate::surfaces::SurfaceKind::*;
    match crate::surfaces::SurfaceKind::from_name(name) {
        Some(Plate) => Frame {
            x_from: "the left edge",
            y_from: "the top edge",
            nearest_edges: true,
            view: "Seen from the front of the cabinet with the door open",
        },
        Some(Door) => Frame {
            x_from: "the left edge",
            y_from: "the top edge",
            nearest_edges: true,
            view: "Seen from outside with the door closed (lying on the bench: outside face up, hinge side as on the cabinet)",
        },
        Some(LeftWall) => Frame {
            x_from: "the back wall",
            y_from: "the top edge",
            nearest_edges: false,
            view: "Seen from outside the cabinet: the back wall is on the left (from inside it is on the right)",
        },
        Some(RightWall) => Frame {
            x_from: "the back wall",
            y_from: "the top edge",
            nearest_edges: false,
            view: "Seen from inside the cabinet: the back wall is on the left (from outside it is on the right)",
        },
        Some(Roof) => Frame {
            x_from: "the left edge, seen from the front",
            y_from: "the back wall",
            nearest_edges: false,
            view: "Seen from above, outside: the back wall is at the top",
        },
        Some(Floor) => Frame {
            x_from: "the left edge, seen from the front",
            y_from: "the back wall",
            nearest_edges: false,
            view: "Seen from above, inside the cabinet: the back wall is at the top",
        },
        _ => Frame {
            x_from: "its left edge",
            y_from: "its top edge",
            nearest_edges: false,
            view: "Seen as the model draws it (extra plates and profiles have no fixed viewing side: check against the drawing)",
        },
    }
}

/// `x mm from <frame x>, y mm from <frame y>` for a point of a surface.
fn point_words(name: &str, x: f64, y: f64, size: Option<(f64, f64)>) -> String {
    let f = frame_of(name);
    match size {
        Some(size) if f.nearest_edges => from_edges(x, y, size),
        _ => format!(
            "{} mm from {}, {} mm from {}",
            mm(x),
            f.x_from,
            mm(y),
            f.y_from
        ),
    }
}

/// Adds a cut-out card for every wiring opening of the layout (grommet
/// holes, slots, cable entries on the plate, the door or a wall), the ones
/// that no device carries. Like [`add_opening_cards`] they sit in front of the
/// step cards, outside the build order; their tool lines are already in the
/// pack's list, which counts every cut-out of the drill table. An opening on
/// a surface whose size is unknown is named in the pack's problems.
pub fn add_wiring_opening_cards(pack: &mut Pack, layout: &CabinetLayout) {
    use crate::openings::OpeningShape;
    for o in &layout.openings {
        // The door is a surface of the drill table even where the enclosure
        // has no door data: its size is then the interior's, or unknown.
        let size = if o.surface == crate::surfaces::MAIN_PLATE {
            Some((layout.plate.width, layout.plate.height))
        } else {
            layout.surface_sizes.get(&o.surface).copied().or_else(|| {
                (o.surface == "door")
                    .then(|| layout.enclosure.map(|e| (e.width, e.height)))
                    .flatten()
            })
        };
        if size.is_none() && o.surface != "door" {
            pack.problems.push(format!(
                "Opening '{}' is on '{}', which this enclosure does not have: no card for it.",
                o.name, o.surface
            ));
            continue;
        }
        let shape = match o.shape {
            OpeningShape::Round { dia } => CutoutShape::Round { dia: half_mm(dia) },
            OpeningShape::Rect { w, h } => CutoutShape::Rect {
                w: half_mm(w),
                h: half_mm(h),
            },
        };
        let (w, h) = match shape {
            CutoutShape::Round { dia } => (dia, dia),
            CutoutShape::Rect { w, h } => (w, h),
        };
        let surface = if o.surface == crate::surfaces::MAIN_PLATE {
            "mounting plate".to_owned()
        } else {
            crate::surfaces::SurfaceKind::from_name(&o.surface)
                .map_or_else(|| o.surface.clone(), |k| k.label().to_owned())
        };
        let frame = frame_of(&o.surface);
        let mut dims = vec![format!(
            "Centre: {}",
            point_words(&o.surface, o.x, o.y, size)
        )];
        let edges = |w: f64, h: f64| {
            format!(
                "Outline from {} to {} mm from {}, and from {} to {} mm from {}",
                mm(o.x - w / 2.0),
                mm(o.x + w / 2.0),
                frame.x_from,
                mm(o.y - h / 2.0),
                mm(o.y + h / 2.0),
                frame.y_from
            )
        };
        let tool = match shape {
            CutoutShape::Round { dia } => {
                dims.push(edges(dia, dia));
                format!("Hole saw or step drill, dia {} mm", mm(dia))
            }
            CutoutShape::Rect { w, h } => {
                dims.push(edges(w, h));
                format!("Punch, nibbler or jigsaw for {} x {} mm", mm(w), mm(h))
            }
        };
        let mut notes = vec![
            format!("{}.", frame.view),
            match size {
                Some(size) => format!(
                    "Measured on the {surface} ({} x {} mm).",
                    mm(size.0),
                    mm(size.1)
                ),
                None => format!("Measured on the {surface}; its size is not in the model."),
            },
        ];
        if let Some(e) = &o.exit {
            let far = layout.surface_sizes.get(&e.surface).copied();
            notes.push(format!(
                "Passes through to the {} at {} ({}): cut both ends.",
                crate::surfaces::SurfaceKind::from_name(&e.surface)
                    .map_or_else(|| e.surface.clone(), |k| k.label().to_owned()),
                point_words(&e.surface, e.x, e.y, far),
                frame_of(&e.surface).view.to_lowercase()
            ));
        }
        notes.push(format!("Keep devices {} mm clear of the edge.", mm(o.keep)));
        let (cw, ch) = (w + 2.0 * OPENING_FRAME_MM, h + 2.0 * OPENING_FRAME_MM);
        pack.cards.push(Card {
            kind: CardKind::Opening,
            steps: (0, 0),
            total: 0,
            phase: Phase::Devices,
            mark: format!("Opening {} ({surface})", o.name),
            text: format!(
                "Wiring opening '{}' in the {surface}: {} mm. It is not a step of the build order: cut it with the {surface} off the cabinet, before parts go on.",
                o.name,
                shape.describe()
            ),
            tools: vec![
                tool,
                "Drill for starter holes, file and deburring tool for the edges".to_owned(),
                "Tape measure, pencil, centre punch".to_owned(),
            ],
            template: Some(Template {
                width: cw,
                height: ch,
                caption: format!("opening cut-out {} mm", shape.describe()),
                origin: frame_note(cw, ch),
                rail: false,
                holes: Vec::new(),
                clips: Vec::new(),
                cutout: Some(TemplateCutout {
                    shape,
                    x: cw / 2.0,
                    y: ch / 2.0,
                }),
            }),
            dims,
            wires: Vec::new(),
            wires_more: 0,
            labels: Vec::new(),
            notes,
            ticks: ticks(&["Marked", "Cut", "Edges filed", "Edge protected"]),
        });
    }
    cut_outs_first(pack);
}

/// What the numbers on a cut-out template's axes mean: the distance of the
/// cross hair (the cut-out's centre) from the frame's top left corner.
fn frame_note(cw: f64, ch: f64) -> String {
    format!(
        "Frame {} x {} mm, {} mm larger than the cut-out on every side. The cross hair is the centre, {} mm from the left edge and {} mm from the top edge of the frame (the numbers on the template's axes)",
        mm(cw),
        mm(ch),
        mm(OPENING_FRAME_MM),
        mm(cw / 2.0),
        mm(ch / 2.0)
    )
}

fn capitalised(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

/// Planning rule for fixing a wiring duct to the plate when the layout does
/// not say how: a fixing 25 mm from each end and none further apart than
/// 200 mm. An assumption (common practice), not a datasheet value.
pub const DUCT_FIXING_END_MM: f64 = 25.0;
pub const DUCT_FIXING_PITCH_MM: f64 = 200.0;

/// The fixing positions along a duct of `length` mm, from its start, mm.
pub fn duct_fixings(length: f64) -> Vec<f64> {
    if length <= 2.0 * DUCT_FIXING_END_MM {
        return vec![length / 2.0];
    }
    let span = length - 2.0 * DUCT_FIXING_END_MM;
    let gaps = (span / DUCT_FIXING_PITCH_MM).ceil().max(1.0) as usize;
    (0..=gaps)
        .map(|i| DUCT_FIXING_END_MM + span * i as f64 / gaps as f64)
        .collect()
}

pub fn duct_fixing_note(length: f64) -> String {
    let at = duct_fixings(length);
    let list = at.iter().map(|x| mm(*x)).collect::<Vec<_>>().join(", ");
    format!(
        "Fixing (a planning rule, not a datasheet value): {} rivets or screws on the centre line of the duct base at {list} mm from the start; the duct maker's instructions win. These holes are not in the drilling list.",
        at.len()
    )
}
