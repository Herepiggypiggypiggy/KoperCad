//! The data a view draws: everything geometric and everything to say about
//! it, with no physics left to do. The visualisation (Cabinet, 3D, the
//! schematic mirror, the EMC card) reads this and nothing else of the engine.
//!
//! Wording rule of the lane: results are findings, indices and estimates,
//! never "pass", "compliant" or "certified". [`DESIGN_AID`] goes on every
//! card and report.

use crate::class::EmcClass;
use crate::input::{Finish, Input, Origin, P, Role};
use crate::report::{Assumption, Band, Finding, Level, Metric, Report, Ribbon, Severity, Subject};

/// Said on every card and report.
pub const DESIGN_AID: &str = "Design aid, not a test result.";

/// A wire run to colour by class.
#[derive(Debug, Clone, PartialEq)]
pub struct PictureRun {
    pub id: u64,
    pub name: String,
    pub class: EmcClass,
    pub class_reason: String,
    /// The class was set by the user.
    pub overridden: bool,
    pub guessed: bool,
    pub plate: Option<u64>,
    pub path: Vec<P>,
    pub shielded: bool,
    /// Both shield ends all round.
    pub shield_full: bool,
    pub score: u32,
    pub band: Band,
}

/// Two runs side by side: a ribbon along each stretch.
#[derive(Debug, Clone, PartialEq)]
pub struct PicturePair {
    pub rule: &'static str,
    pub level: Level,
    pub severity: Severity,
    pub a: u64,
    pub b: u64,
    pub ribbons: Vec<Ribbon>,
    /// Points the finding gives its victim, the ribbon's colour value.
    pub points: u32,
    pub message: String,
}

/// The sheet between go and return.
#[derive(Debug, Clone, PartialEq)]
pub struct PictureLoop {
    pub runs: (u64, u64),
    pub plate: Option<u64>,
    pub polygon: Vec<P>,
    pub area_cm2: f64,
    pub message: String,
}

/// A device coloured by what it does for EMC.
#[derive(Debug, Clone, PartialEq)]
pub struct Zone {
    pub device: u64,
    pub tag: String,
    pub role: Role,
    pub plate: u64,
    /// x, y, width, height on the plate, mm.
    pub rect: (f64, f64, f64, f64),
    /// How far it stands off the plate, mm, when its depth is stated.
    pub height_mm: Option<f64>,
    pub score: u32,
}

/// How well a case makes metal contact with its plate, from what the project
/// states. A rule on stated facts, not a measured contact resistance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BondQuality {
    /// Bare or galvanised plate and no statement against bonding.
    Good,
    /// Painted plate: no contact at high frequency until scraped bare.
    Weak,
    /// Stated as not bonded.
    Bad,
}

/// One case-to-plate link: the mounting area of a drive or filter.
#[derive(Debug, Clone, PartialEq)]
pub struct Bond {
    pub device: u64,
    pub tag: String,
    pub plate: u64,
    /// The contact area as a closed outline on the plate (first point
    /// repeated at the end).
    pub outline: Vec<P>,
    pub quality: BondQuality,
    /// Why, in words, naming the stated facts the rule used.
    pub reason: String,
}

/// A place to mark, with what the card shows for it.
#[derive(Debug, Clone, PartialEq)]
pub struct Hotspot {
    pub plate: u64,
    pub at: P,
    pub rule: &'static str,
    pub level: Level,
    pub severity: Severity,
    pub message: String,
    /// Numbers, each with the level of its method.
    pub values: Vec<Metric>,
    /// The band an estimate holds in, Hz.
    pub band_hz: Option<(f64, f64)>,
    /// Where the rule is written down; unverified.
    pub source: &'static str,
    pub subjects: Vec<Subject>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub runs: Vec<PictureRun>,
    pub pairs: Vec<PicturePair>,
    pub loops: Vec<PictureLoop>,
    pub zones: Vec<Zone>,
    /// Case-to-plate links of drives and filters whose bonding the project
    /// states (plate finish or `emc.bonded`). Rule-based (the same facts as
    /// EMC-11), not a measurement; plate to enclosure and PE bar links are
    /// not modelled.
    pub bonds: Vec<Bond>,
    pub hotspots: Vec<Hotspot>,
    /// Every setting the numbers rest on, marked default or set in the project.
    pub assumptions: Vec<Assumption>,
    pub not_checked: &'static [&'static str],
    pub note: &'static str,
}

fn pair_of(f: &Finding) -> Option<(u64, u64)> {
    let mut runs = f.subjects.iter().filter_map(|s| match s {
        Subject::Run(id) => Some(*id),
        _ => None,
    });
    Some((runs.next()?, runs.next()?))
}

/// Arranges a report for drawing.
pub fn picture(input: &Input, report: &Report) -> Picture {
    let runs = input
        .runs
        .iter()
        .map(|r| {
            let score = report.score_of(Subject::Run(r.id));
            PictureRun {
                id: r.id,
                name: r.name.clone(),
                class: r.class,
                class_reason: r.class_reason.clone(),
                overridden: r.overridden(),
                guessed: r.origin == Origin::Guessed,
                plate: r.plate,
                path: r.path.clone(),
                shielded: r.shield.is_some(),
                shield_full: r.shield.is_some_and(|s| s.is_full()),
                score: score.map_or(0, |s| s.points),
                band: score.map_or(Band::None, |s| s.band),
            }
        })
        .collect();
    let mut pairs = Vec::new();
    let mut loops = Vec::new();
    let mut hotspots = Vec::new();
    for f in &report.findings {
        if !f.ribbons.is_empty()
            && let Some((a, b)) = pair_of(f)
        {
            pairs.push(PicturePair {
                rule: f.rule,
                level: f.level,
                severity: f.severity,
                a,
                b,
                ribbons: f.ribbons.clone(),
                points: f.points.first().map_or(0, |p| p.1),
                message: f.message.clone(),
            });
        }
        if !f.loop_polygon.is_empty()
            && let Some((a, b)) = pair_of(f)
        {
            loops.push(PictureLoop {
                runs: (a, b),
                plate: f.at.map(|x| x.0),
                polygon: f.loop_polygon.clone(),
                area_cm2: polygon_area_cm2(&f.loop_polygon),
                message: f.message.clone(),
            });
        }
        if let Some((plate, at)) = f.at {
            hotspots.push(Hotspot {
                plate,
                at,
                rule: f.rule,
                level: f.level,
                severity: f.severity,
                message: f.message.clone(),
                values: f.metrics.clone(),
                band_hz: f.band_hz,
                source: f.source(),
                subjects: f.subjects.clone(),
            });
        }
    }
    let zones = input
        .devices
        .iter()
        .filter_map(|d| {
            Some(Zone {
                device: d.id,
                tag: d.tag.clone(),
                role: d.role,
                plate: d.plate?,
                rect: d.rect?,
                height_mm: d.height_mm,
                score: report
                    .score_of(Subject::Device(d.id))
                    .map_or(0, |s| s.points),
            })
        })
        .collect();
    Picture {
        runs,
        pairs,
        loops,
        zones,
        bonds: bonds(input),
        hotspots,
        assumptions: report.assumptions.clone(),
        not_checked: crate::report::NOT_CHECKED,
        note: DESIGN_AID,
    }
}

/// The case-to-plate links the project states something about. A drive or
/// filter with neither a plate finish nor `emc.bonded` stated gets no bond
/// rather than a guessed one.
pub fn bonds(input: &Input) -> Vec<Bond> {
    input
        .devices
        .iter()
        .filter(|d| matches!(d.role, Role::Drive | Role::Filter))
        .filter_map(|d| {
            let plate_id = d.plate?;
            let (x, y, w, h) = d.rect?;
            let plate = input.plates.iter().find(|p| p.id == plate_id)?;
            let (quality, reason) = match (d.bonded, plate.finish) {
                (Some(false), _) => (
                    BondQuality::Bad,
                    "stated as not bonded to the plate".to_owned(),
                ),
                (_, Some(Finish::Painted)) => (
                    BondQuality::Weak,
                    format!("plate {} is painted where the case sits", plate.tag),
                ),
                (Some(true), _) => (
                    BondQuality::Good,
                    "stated as bonded to the plate".to_owned(),
                ),
                (None, Some(Finish::Conductive)) => (
                    BondQuality::Good,
                    format!("plate {} is bare or galvanised", plate.tag),
                ),
                (None, None) => return None,
            };
            Some(Bond {
                device: d.id,
                tag: d.tag.clone(),
                plate: plate_id,
                outline: vec![(x, y), (x + w, y), (x + w, y + h), (x, y + h), (x, y)],
                quality,
                reason,
            })
        })
        .collect()
}

/// Area of a closed polygon in cm2 (shoelace).
pub fn polygon_area_cm2(poly: &[P]) -> f64 {
    let mut sum = 0.0;
    for i in 0..poly.len() {
        let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
        sum += p.0 * q.1 - q.0 * p.1;
    }
    sum.abs() / 2.0 / 100.0
}
