//! Bench output for a workshop that cuts wires and crimps by hand: the
//! ferrule standard (cross-section, colour, contact length), the ferrule
//! table to prepare, and the bench cut list with tick boxes.
//!
//! Everything is generated from [`wire_cut_jobs`](crate::wire_cut_jobs), so
//! the lengths, strip lengths and ferrules agree with the wire production
//! report and the machine CSV. Lengths are exact only for wires routed
//! through ducts between devices with real pin positions; every other basis
//! is marked `~` on the sheet.

use std::collections::BTreeMap;

use novo_core::{PinRef, Project, Value};
use serde::{Deserialize, Serialize};

use crate::report::{Dependencies, Report};
use crate::table::{Cell, Column, ReportOutput, Section};
use crate::wirecut::{WireCutJob, typical_strip_mm, wire_cut_jobs_with};

/// Project property that holds a changed ferrule standard (JSON text).
pub const STANDARD_KEY: &str = "bench.ferrule_standard";

/// The box printed in front of a row to tick when done. The PDF writer
/// draws it as a square (the base fonts have no such glyph).
pub const TICK_BOX: &str = "\u{2610}";

/// One size of the ferrule standard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FerruleSize {
    /// Conductor cross-section the ferrule takes, mm².
    pub mm2: f64,
    /// Sleeve colour as printed on the sheet (`white`, `grey`, ...).
    pub colour: String,
    /// Contact (crimp) length in mm, which is also the strip length.
    pub length_mm: f64,
}

/// The ferrule standard a workshop works to: colour and contact length per
/// cross-section, how many spares to prepare and up to which size a twin
/// ferrule is used where two wires share a clamp.
///
/// The default is the DIN 46228-4 colour code as ferrules are commonly sold
/// in Europe. Makers differ, above 16 mm² in particular, so it is a
/// starting point to edit, and it is printed on the ferrule sheet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FerruleStandard {
    pub sizes: Vec<FerruleSize>,
    /// Extra ferrules to prepare, percent of the count (crimping mistakes).
    pub spare_percent: f64,
    /// Largest cross-section (mm²) for which twin ferrules are made; two
    /// wires of one size on one clamp use a twin up to there. Zero turns
    /// twins off.
    pub twin_to_mm2: f64,
}

impl Default for FerruleStandard {
    fn default() -> Self {
        let size = |mm2: f64, colour: &str| FerruleSize {
            mm2,
            colour: colour.to_owned(),
            length_mm: typical_strip_mm(mm2),
        };
        FerruleStandard {
            sizes: vec![
                size(0.5, "white"),
                size(0.75, "grey"),
                size(1.0, "red"),
                size(1.5, "black"),
                size(2.5, "blue"),
                size(4.0, "grey"),
                size(6.0, "yellow"),
                size(10.0, "red"),
                size(16.0, "blue"),
                size(25.0, "yellow"),
                size(35.0, "red"),
            ],
            spare_percent: 10.0,
            twin_to_mm2: 6.0,
        }
    }
}

impl FerruleStandard {
    /// The standard of `project`: the stored one when it reads, else the
    /// default.
    pub fn of(project: &Project) -> FerruleStandard {
        match project.info().properties.get(STANDARD_KEY) {
            Some(Value::Text(text)) => serde_json::from_str::<FerruleStandard>(text)
                .ok()
                .filter(FerruleStandard::is_valid)
                .unwrap_or_default(),
            _ => FerruleStandard::default(),
        }
    }

    /// Stores `standard` in the project; the default removes the property,
    /// so a project that never changed it stays clean.
    pub fn store(tx: &mut novo_core::Tx<'_>, standard: &FerruleStandard) -> novo_core::Result<()> {
        let text = (*standard != FerruleStandard::default())
            .then(|| serde_json::to_string(standard).ok())
            .flatten();
        tx.set_info(|info| match text {
            Some(text) => {
                info.properties
                    .insert(STANDARD_KEY.to_owned(), Value::Text(text));
            }
            None => {
                info.properties.remove(STANDARD_KEY);
            }
        })
    }

    /// Sizes positive and finite, no size twice, spare and twin in range.
    pub fn is_valid(&self) -> bool {
        let ok = |v: f64| v.is_finite() && v > 0.0;
        self.sizes.iter().all(|s| ok(s.mm2) && ok(s.length_mm))
            && self.spare_percent.is_finite()
            && (0.0..=100.0).contains(&self.spare_percent)
            && self.twin_to_mm2.is_finite()
            && self.twin_to_mm2 >= 0.0
            && self
                .sizes
                .iter()
                .enumerate()
                .all(|(i, a)| !self.sizes[..i].iter().any(|b| same(a.mm2, b.mm2)))
    }

    /// The size a conductor of `mm2` takes: the listed size when it is
    /// listed, else the next larger one (a 1.2 mm² conductor takes a 1.5
    /// ferrule), else the largest.
    pub fn lookup(&self, mm2: f64) -> Option<&FerruleSize> {
        if let Some(exact) = self.sizes.iter().find(|s| same(s.mm2, mm2)) {
            return Some(exact);
        }
        self.sizes
            .iter()
            .filter(|s| s.mm2 > mm2)
            .min_by(|a, b| a.mm2.total_cmp(&b.mm2))
            .or_else(|| self.sizes.iter().max_by(|a, b| a.mm2.total_cmp(&b.mm2)))
    }

    /// The contact length for `mm2`, if the standard lists a size for it.
    pub fn length_mm(&self, mm2: f64) -> Option<f64> {
        self.lookup(mm2).map(|s| s.length_mm)
    }

    /// Sizes in ascending order (the editor keeps them sorted).
    pub fn sort(&mut self) {
        self.sizes.sort_by(|a, b| a.mm2.total_cmp(&b.mm2));
    }
}

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

pub(crate) fn number_text(v: f64) -> String {
    let t = format!("{v:.2}");
    t.trim_end_matches('0').trim_end_matches('.').to_owned()
}

/// Wire jobs under `project`'s ferrule standard.
pub fn bench_jobs(project: &Project) -> Vec<WireCutJob> {
    wire_cut_jobs_with(project, &FerruleStandard::of(project))
}

/// Whether a length basis is an estimate to check on the plate, not a
/// length found by routing (`routed`, `routed, no depth`) or from a length the
/// designer states (the cable's, or the wire outside the entry).
pub fn is_estimate(basis: &str) -> bool {
    !(basis == "routed"
        || basis == "routed, no depth"
        || basis == "cable length"
        || basis == "to entry + stated outside")
}

// ---------------------------------------------------------------- ferrules

/// What one row of the ferrule table is.
#[derive(Debug, Clone, PartialEq)]
pub struct FerruleCount {
    /// Designation to pick from the box: `A1.5-10`, or `2x1.5-10` for a twin.
    pub ferrule: String,
    pub twin: bool,
    pub mm2: Option<f64>,
    pub colour: String,
    pub length_mm: Option<f64>,
    /// Wire ends this ferrule covers (two per twin).
    pub ends: usize,
    /// Ferrules to crimp.
    pub pieces: usize,
}

/// The ferrules the jobs need, one row per designation: singles, and twins
/// where two wire ends of one size share a pin (up to
/// [`FerruleStandard::twin_to_mm2`]). Sorted by cross-section, singles before
/// twins. Wire ends without a ferrule (unknown cross-section, or `none` on
/// the wire) are not counted; see [`ends_without_ferrule`].
pub fn ferrule_counts(jobs: &[WireCutJob], standard: &FerruleStandard) -> Vec<FerruleCount> {
    // Ends per pin and designation: two on one pin can share a twin.
    let mut per_pin: BTreeMap<(PinRef, String), (usize, Option<f64>)> = BTreeMap::new();
    for j in jobs {
        for end in [&j.end_a, &j.end_b] {
            if let Some(f) = &end.ferrule {
                per_pin
                    .entry((end.pin, f.clone()))
                    .or_insert((0, j.cross_section_mm2))
                    .0 += 1;
            }
        }
    }
    let mut rows: BTreeMap<(bool, String), FerruleCount> = BTreeMap::new();
    let mut add = |ferrule: String, twin: bool, mm2: Option<f64>, ends: usize, pieces: usize| {
        let colour = mm2
            .and_then(|s| standard.lookup(s))
            .map(|s| s.colour.clone())
            .unwrap_or_default();
        let length_mm = ferrule
            .rsplit('-')
            .next()
            .and_then(|l| l.parse::<f64>().ok());
        let e = rows
            .entry((twin, ferrule.clone()))
            .or_insert_with(|| FerruleCount {
                ferrule,
                twin,
                mm2,
                colour,
                length_mm,
                ends: 0,
                pieces: 0,
            });
        e.ends += ends;
        e.pieces += pieces;
    };
    for ((_, ferrule), (ends, mm2)) in per_pin {
        let twin_ok = mm2.is_some_and(|s| s <= standard.twin_to_mm2) && ferrule.starts_with('A');
        let twins = if twin_ok { ends / 2 } else { 0 };
        let singles = ends - 2 * twins;
        if twins > 0 {
            add(format!("2x{}", &ferrule[1..]), true, mm2, 2 * twins, twins);
        }
        if singles > 0 {
            add(ferrule, false, mm2, singles, singles);
        }
    }
    let mut out: Vec<FerruleCount> = rows.into_values().collect();
    out.sort_by(|a, b| {
        let key = |c: &FerruleCount| c.mm2.unwrap_or(f64::INFINITY);
        key(a)
            .total_cmp(&key(b))
            .then(a.twin.cmp(&b.twin))
            .then(a.ferrule.cmp(&b.ferrule))
    });
    out
}

/// Wire ends that get no ferrule: `(unknown cross-section, switched off)`.
pub fn ends_without_ferrule(jobs: &[WireCutJob]) -> (usize, usize) {
    let (mut unknown, mut off) = (0, 0);
    for j in jobs {
        for end in [&j.end_a, &j.end_b] {
            if end.ferrule.is_none() {
                if j.cross_section_mm2.is_none() {
                    unknown += 1;
                } else {
                    off += 1;
                }
            }
        }
    }
    (unknown, off)
}

/// Ferrules to prepare for `pieces` with `spare_percent` spares, rounded up.
pub fn with_spares(pieces: usize, spare_percent: f64) -> usize {
    (pieces as f64 * (1.0 + spare_percent / 100.0) - 1e-9).ceil() as usize
}

/// The ferrule table (`ferrules`): what to crimp, per size and colour, and
/// the standard it was worked out with.
#[derive(Debug, Clone, Copy, Default)]
pub struct FerruleTable;

impl FerruleTable {
    pub const ID: &'static str = "ferrules";

    /// The table for `jobs` under `standard`.
    pub fn build(
        project: &Project,
        jobs: &[WireCutJob],
        standard: &FerruleStandard,
    ) -> ReportOutput {
        let counts = ferrule_counts(jobs, standard);
        let mut sections = Vec::new();
        if !counts.is_empty() {
            let mut table = Section::new(
                "ferrules",
                "Ferrules to crimp",
                vec![
                    Column::new("done", "", 5.0),
                    Column::new("ferrule", "Ferrule", 14.0),
                    Column::new("mm2", "mm²", 8.0).right(),
                    Column::new("colour", "Colour", 10.0),
                    Column::new("length", "Contact mm", 10.0).right(),
                    Column::new("type", "Type", 8.0),
                    Column::new("ends", "Wire ends", 10.0).right(),
                    Column::new("pieces", "Crimp", 8.0).right(),
                    Column::new("prepare", "Prepare", 10.0).right(),
                ],
            );
            for c in &counts {
                table.push(
                    vec![
                        Cell::text(TICK_BOX),
                        Cell::text(c.ferrule.clone()),
                        c.mm2.into(),
                        Cell::text(c.colour.clone()),
                        c.length_mm.into(),
                        Cell::text(if c.twin { "twin" } else { "single" }),
                        Cell::Int(c.ends as i64),
                        Cell::Int(c.pieces as i64),
                        Cell::Int(with_spares(c.pieces, standard.spare_percent) as i64),
                    ],
                    Vec::new(),
                );
            }
            let total: usize = counts.iter().map(|c| c.pieces).sum();
            let ends: usize = counts.iter().map(|c| c.ends).sum();
            let prepare: usize = counts
                .iter()
                .map(|c| with_spares(c.pieces, standard.spare_percent))
                .sum();
            table.push(
                vec![
                    Cell::Empty,
                    Cell::text("Total"),
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Empty,
                    Cell::Int(ends as i64),
                    Cell::Int(total as i64),
                    Cell::Int(prepare as i64),
                ],
                Vec::new(),
            );
            sections.push(table);
        }

        let (unknown, off) = ends_without_ferrule(jobs);
        if unknown + off > 0 {
            let mut skipped = Section::new(
                "skipped",
                "Wire ends without a ferrule",
                vec![
                    Column::new("why", "Why", 40.0),
                    Column::new("ends", "Wire ends", 10.0).right(),
                ],
            );
            if unknown > 0 {
                skipped.push(
                    vec![
                        Cell::text("Cross-section not set: give the wire a size"),
                        Cell::Int(unknown as i64),
                    ],
                    Vec::new(),
                );
            }
            if off > 0 {
                skipped.push(
                    vec![
                        Cell::text("Ferrule switched off on the wire (none)"),
                        Cell::Int(off as i64),
                    ],
                    Vec::new(),
                );
            }
            // A few lines: they share the sheet of the table above.
            sections.push(skipped.flowing());
        }

        let mut std_section = Section::new(
            "standard",
            "Ferrule standard used",
            vec![
                Column::new("mm2", "mm²", 10.0).right(),
                Column::new("colour", "Colour", 14.0),
                Column::new("length", "Contact mm", 12.0).right(),
                Column::new("note", "", 60.0),
            ],
        );
        for s in &standard.sizes {
            std_section.push(
                vec![
                    Cell::Number(s.mm2),
                    Cell::text(s.colour.clone()),
                    Cell::Number(s.length_mm),
                    Cell::Empty,
                ],
                Vec::new(),
            );
        }
        for note in [
            format!(
                "Spares: {}%. Twin ferrules up to {} mm².",
                number_text(standard.spare_percent),
                number_text(standard.twin_to_mm2)
            ),
            "Default colours follow DIN 46228-4 as commonly sold; makers differ, above 16 mm² in particular. Edit under Reports > Bench output.".to_owned(),
        ] {
            std_section.push(
                vec![Cell::Empty, Cell::Empty, Cell::Empty, Cell::text(note)],
                Vec::new(),
            );
        }
        sections.push(std_section.flowing());

        ReportOutput {
            report: Self::ID.into(),
            title: "Ferrules".into(),
            revision: project.revision(),
            sections,
        }
    }
}

impl Report for FerruleTable {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Ferrules".into()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn dependencies(&self) -> Dependencies {
        crate::WireCut.dependencies()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        let standard = FerruleStandard::of(project);
        let jobs = wire_cut_jobs_with(project, &standard);
        Self::build(project, &jobs, &standard)
    }
}

// ----------------------------------------------------------------- cut list

/// The bench cut list (`benchcut`): wires to cut, one table per
/// cross-section and colour so one kind of wire is pulled and cut at a time,
/// longest first, with a tick box per wire, the strip length and ferrule at
/// each end, and the end allowance shown apart from the route.
///
/// A summary opens the report (wires and metres to pull per group) and says
/// how many lengths are estimates (in its heading, and per group in a column);
/// those carry a `~` in the table. No page is spent on a note alone.
#[derive(Debug, Clone, Copy, Default)]
pub struct BenchCut;

impl BenchCut {
    pub const ID: &'static str = "benchcut";

    /// The section key of the group `mm2`, `colour`.
    pub fn group_key(mm2: Option<f64>, colour: &str) -> String {
        format!("g:{}:{}", mm2.map_or("?".to_owned(), number_text), colour)
    }

    /// The report for `jobs` (in production order).
    pub fn build(project: &Project, jobs: &[WireCutJob]) -> ReportOutput {
        let title = "Bench cut list".to_owned();
        if jobs.is_empty() {
            return ReportOutput {
                report: Self::ID.into(),
                title,
                revision: project.revision(),
                sections: Vec::new(),
            };
        }
        // Groups in job order: the jobs are sorted by size and colour.
        let mut groups: Vec<(Option<f64>, String, Vec<&WireCutJob>)> = Vec::new();
        for j in jobs {
            match groups.last_mut() {
                Some((s, c, list)) if *s == j.cross_section_mm2 && *c == j.colour => list.push(j),
                _ => groups.push((j.cross_section_mm2, j.colour.clone(), vec![j])),
            }
        }

        // The estimates note rides in the summary's title and a column, not on
        // a page of its own (every section starts a new page when printed).
        let estimates = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
        let summary_title = if estimates > 0 {
            format!(
                "What to pull · {estimates} of {} lengths are estimates (~): measure on the plate",
                jobs.len()
            )
        } else {
            "What to pull".to_owned()
        };
        let mut summary = Section::new(
            "summary",
            &summary_title,
            vec![
                Column::new("done", "", 5.0),
                Column::new("mm2", "mm²", 8.0).right(),
                Column::new("colour", "Colour", 12.0),
                Column::new("wires", "Wires", 8.0).right(),
                Column::new("metres", "Metres", 10.0).right(),
                Column::new("longest", "Longest reel mm", 12.0).right(),
                Column::new("ferrule", "Ferrule", 14.0),
                Column::new("estimates", "Estimated", 10.0).right(),
            ],
        );
        // Cores of a cable come with the cable (bought by the length on its
        // BOM line); they are stripped and ferruled here but not pulled from
        // a reel, so their metres are not metres to pull.
        let on_cable = |j: &WireCutJob| j.cable.is_some();
        let reel_mm = |list: &[&WireCutJob]| -> f64 {
            list.iter().filter(|j| !on_cable(j)).map(|j| j.cut_mm).sum()
        };
        for (mm2, colour, list) in &groups {
            let mm: f64 = reel_mm(list);
            let longest = list
                .iter()
                .filter(|j| !on_cable(j))
                .map(|j| j.cut_mm)
                .fold(0.0, f64::max);
            let ferrule = list
                .iter()
                .find_map(|j| j.end_a.ferrule.clone())
                .unwrap_or_default();
            summary.push(
                vec![
                    Cell::text(TICK_BOX),
                    (*mm2).into(),
                    Cell::text(colour.clone()),
                    Cell::Int(list.len() as i64),
                    Cell::Number((mm / 1000.0 * 10.0).ceil() / 10.0),
                    Cell::Number(longest),
                    Cell::text(ferrule),
                    Cell::Int(list.iter().filter(|j| is_estimate(&j.basis)).count() as i64),
                ],
                Vec::new(),
            );
        }
        let no_size = jobs
            .iter()
            .filter(|j| j.cross_section_mm2.is_none())
            .count();
        let no_colour = jobs.iter().filter(|j| j.colour.is_empty()).count();
        let no_number = jobs.iter().filter(|j| j.number.trim().is_empty()).count();
        let mut notes: Vec<String> = Vec::new();
        if no_number > 0 {
            notes.push(format!(
                "{no_number} of {} wires have no wire number (shown as \"-\"), so the wires cannot be told apart and the labels are blank. Number them under Edit > Wire numbers (Ctrl+K, number wires), then print again.",
                jobs.len()
            ));
        }
        if no_size > 0 {
            notes.push(format!(
                "{no_size} of {} wires have no cross-section set (listed under \"size not set\"): they get no strip length and no ferrule. Set the size on the wire, or fill it in for all of them under Reports > Bench output.",
                jobs.len()
            ));
        }
        if no_colour > 0 {
            notes.push(format!(
                "{no_colour} of {} wires have no insulation colour set, so their Colour is blank here. Ferrule sleeve colours come from the ferrule standard and are on the ferrule table.",
                jobs.len()
            ));
        }
        let cores: Vec<&WireCutJob> = jobs.iter().filter(|j| on_cable(j)).collect();
        if !cores.is_empty() {
            let (n, verb) = if cores.len() == 1 {
                ("1 of", "is a core of a cable")
            } else {
                (&*format!("{} of", cores.len()), "are cores of cables")
            };
            notes.push(format!(
                "{n} {} wires {verb}. A cable is bought by its own length (the BOM lists it when the cable has a stated length), not cut from a reel, so cores are left out of Metres and Longest reel mm; strip and ferrule each core end as listed.",
                jobs.len()
            ));
        }
        let mut sections = vec![summary];
        let mut cable_section: Option<Section> = None;
        let runs = crate::cable_runs(project, jobs);
        if !runs.is_empty() {
            let mut cables = Section::new(
                "cables",
                "Cables",
                vec![
                    Column::new("done", "", 5.0),
                    Column::new("cable", "Cable", 14.0),
                    Column::new("cores", "Cores used", 10.0).right(),
                    Column::new("metres", "Metres to buy", 12.0).right(),
                    Column::new("basis", "Length basis", 12.0),
                ],
            );
            for r in runs {
                cables.push(
                    vec![
                        Cell::text(TICK_BOX),
                        Cell::text(r.tag.clone()),
                        Cell::Int(r.cores as i64),
                        Cell::Number(r.metres()),
                        Cell::text(r.basis()),
                    ],
                    vec![r.cable.into()],
                );
            }
            cable_section = Some(cables);
        }
        if !notes.is_empty() {
            let mut note = Section::new(
                "check",
                "Check before cutting",
                vec![Column::new("note", "", 60.0)],
            );
            for text in notes {
                note.push(vec![Cell::text(text)], Vec::new());
            }
            sections.push(note);
        }

        let mut first_group = true;
        for (mm2, colour, list) in &groups {
            let total: f64 = reel_mm(list);
            let cores = list.iter().filter(|j| on_cable(j)).count();
            let heading = format!(
                "{} · {} wires · {:.1} m{}",
                group_name(*mm2, colour),
                list.len(),
                total / 1000.0,
                if cores > 0 {
                    format!(" (+{cores} cable cores, not counted in metres)")
                } else {
                    String::new()
                }
            );
            let mut section = Section::new(
                &Self::group_key(*mm2, colour),
                &heading,
                vec![
                    Column::new("done", "", 4.0),
                    Column::new("number", "Wire no.", 12.0),
                    Column::new("cut", "Cut mm", 9.0).right(),
                    Column::new("route", "Route", 8.0).right(),
                    Column::new("ends", "+Ends", 6.0).right(),
                    Column::new("strip_a", "Strip A", 7.0).right(),
                    Column::new("ferrule_a", "Ferr. A", 10.0),
                    Column::new("strip_b", "Strip B", 7.0).right(),
                    Column::new("ferrule_b", "Ferr. B", 10.0),
                    Column::new("from", "From", 22.0),
                    Column::new("to", "To", 22.0),
                ],
            );
            for j in list {
                let est = is_estimate(&j.basis);
                let mark = |v: f64| {
                    if est {
                        Cell::text(format!("~{}", number_text(v)))
                    } else {
                        Cell::Number(v)
                    }
                };
                section.push(
                    vec![
                        Cell::text(TICK_BOX),
                        Cell::text(j.number.clone()),
                        mark(j.cut_mm),
                        mark(j.length_mm),
                        Cell::Number(j.cut_mm - j.length_mm),
                        j.end_a.strip_mm.into(),
                        j.end_a.ferrule.clone().into(),
                        j.end_b.strip_mm.into(),
                        j.end_b.ferrule.clone().into(),
                        Cell::text(j.end_a.designation.clone()),
                        Cell::text(j.end_b.designation.clone()),
                    ],
                    j.wires.iter().map(|w| novo_core::AnyId::from(*w)).collect(),
                );
            }
            // The first wire group follows the summary on its sheet.
            sections.push(if first_group {
                section.flowing()
            } else {
                section
            });
            first_group = false;
        }

        sections.extend(cable_section);
        ReportOutput {
            report: Self::ID.into(),
            title,
            revision: project.revision(),
            sections,
        }
    }
}

fn group_name(mm2: Option<f64>, colour: &str) -> String {
    let size = mm2.map_or("size not set".to_owned(), |s| {
        format!("{} mm²", number_text(s))
    });
    if colour.is_empty() {
        size
    } else {
        format!("{size} {colour}")
    }
}

impl Report for BenchCut {
    fn id(&self) -> &str {
        Self::ID
    }

    fn title(&self) -> String {
        "Bench cut list".into()
    }

    fn proposed_by_default(&self) -> bool {
        false
    }

    fn dependencies(&self) -> Dependencies {
        crate::WireCut.dependencies()
    }

    fn generate(&self, project: &Project) -> ReportOutput {
        Self::build(project, &bench_jobs(project))
    }
}
