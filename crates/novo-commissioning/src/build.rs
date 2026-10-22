//! Build mode: the guided assembly and wiring of one cabinet, step by step,
//! with a tick for every step. The answer to a shop-floor tablet app, with no
//! server: the plan is computed from the project, the ticks are kept in the
//! [`CommissioningStore`] and travel in the same `.kcfield` file as the
//! commissioning results.
//!
//! The plan, per cabinet with a mounting plate, in the order the workshop
//! does it (the order of `novo_layout::assembly`, with the plate work in
//! front):
//!
//! 1. **Prepare**: the plan is checked before anything is cut.
//! 2. **Drill**: the holes, one drill diameter at a time, then the cut-outs.
//! 3. **Rails** and **Ducts**: cut and fit.
//! 4. **Devices**: rail by rail, then the bare plate.
//! 5. **Wires**: power, then control, then earth; each after every device.
//! 6. **Labels**, then **Check**: what a person confirms by eye.
//!
//! Wires are cut and holes drilled by hand: a step says what to cut or drill
//! and where, never that a machine does it. What is a model and not a
//! measurement is said on the step ([`BuildStep::caution`]): a guessed part
//! size, an estimated wire length, the power/control/earth grouping.
//!
//! A tick remembers a fingerprint of the step text. When the design changes
//! so that a ticked step now reads differently (a device moved, a wire got
//! longer), the step is [`StepStatus::Changed`] and counts as open again, so
//! a finished job is never silently out of date.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{DeviceTag, Project, Structure, WireId};
use novo_layout::assembly::{Phase, WireJob, steps};
use novo_layout::drill::Surface;
use novo_reports::WireCutJob;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::check::Target;
use crate::punch::{PunchItem, PunchState, Severity};
use crate::store::CommissioningStore;

/// The part of the build a step belongs to, in build order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BuildPhase {
    Prepare,
    Drill,
    Rails,
    Ducts,
    Devices,
    Wires,
    Labels,
    Check,
}

impl BuildPhase {
    pub const ALL: [BuildPhase; 8] = [
        BuildPhase::Prepare,
        BuildPhase::Drill,
        BuildPhase::Rails,
        BuildPhase::Ducts,
        BuildPhase::Devices,
        BuildPhase::Wires,
        BuildPhase::Labels,
        BuildPhase::Check,
    ];

    pub fn name(self) -> &'static str {
        match self {
            BuildPhase::Prepare => "Prepare",
            BuildPhase::Drill => "Drill",
            BuildPhase::Rails => "Rails",
            BuildPhase::Ducts => "Ducts",
            BuildPhase::Devices => "Devices",
            BuildPhase::Wires => "Wires",
            BuildPhase::Labels => "Labels",
            BuildPhase::Check => "Check",
        }
    }
}

/// Which step, stable while the design changes: the cabinet, the phase and
/// the object (rail, duct, device, first wire of a connection, drill group)
/// the step is about. Step numbers are not used: they shift when a device
/// is added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct StepKey {
    pub cabinet: u64,
    pub phase: BuildPhase,
    pub subject: u64,
}

/// What a person decided about a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TickState {
    Done,
    /// Could not be done as drawn; a punch item with the note was raised.
    Problem,
}

/// One decision, with who and when (`at` is ISO 8601 text, like every
/// timestamp in this crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildTick {
    pub key: StepKey,
    /// The step's mark when ticked (`R1`, `-K1`, a wire number), so a file
    /// from another project is told apart on import.
    pub mark: String,
    /// Fingerprint of the step text when ticked.
    pub hash: String,
    pub state: TickState,
    #[serde(default)]
    pub note: String,
    pub by: String,
    pub at: String,
}

/// A person's "I take this step", so two people on one cabinet do not reach
/// for the same device. A released claim stays as a record with an empty
/// `by`, so the release travels through a merge as the newer record (the
/// same newest-wins rule as a tick).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildClaim {
    pub key: StepKey,
    /// The step's mark when claimed, so a file from another project is
    /// told apart on import.
    pub mark: String,
    /// Who holds the step; empty when it was released.
    pub by: String,
    pub at: String,
}

/// How a step stands now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    Open,
    Done,
    Problem,
    /// Ticked, but the step reads differently now: do it again or confirm.
    Changed,
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq)]
pub struct BuildStep {
    pub key: StepKey,
    /// 1-based position in the cabinet's plan.
    pub number: usize,
    pub phase: BuildPhase,
    /// `R1`, `-K1`, a wire number: what to look for on the plate.
    pub mark: String,
    /// The instruction, one sentence.
    pub title: String,
    /// Further lines: where, how big, which part, which ferrule.
    pub details: Vec<String>,
    /// What is a model or a guess here, in words; `None` when nothing is.
    pub caution: Option<String>,
    /// Numbers of the steps that must be done first.
    pub after: Vec<usize>,
    /// The device this step is about, for jumping to it in the other views.
    pub device: Option<u64>,
    /// The surface a device is fitted on when it is not the mounting plate
    /// (`door`, `left`, `roof`, `plate2`, ...), by the name the Cabinet's
    /// Surface picker uses.
    pub surface: Option<String>,
    pub wires: Vec<WireId>,
    fingerprint: String,
}

impl BuildStep {
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// The plan of one cabinet.
#[derive(Debug, Clone, PartialEq)]
pub struct CabinetPlan {
    /// The enclosure's structure id.
    pub cabinet: u64,
    pub tag: String,
    pub steps: Vec<BuildStep>,
}

/// Counts for a progress bar.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildProgress {
    pub total: usize,
    pub done: usize,
    pub problems: usize,
    pub changed: usize,
}

impl BuildProgress {
    pub fn open(&self) -> usize {
        self.total - self.done - self.problems
    }

    pub fn fraction(&self) -> f32 {
        if self.total == 0 {
            0.0
        } else {
            self.done as f32 / self.total as f32
        }
    }
}

/// What to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Next {
    /// Everything is done (or parked as a problem).
    Finished,
    /// This step has everything it needs done.
    Ready(usize),
    /// Open steps remain but each waits for one that is a problem or still
    /// open: `step` is the first of them, `waits_for` the first step it
    /// needs. Only reached when a problem is parked.
    Blocked { step: usize, waits_for: usize },
}

impl CabinetPlan {
    pub fn step(&self, number: usize) -> Option<&BuildStep> {
        number.checked_sub(1).and_then(|i| self.steps.get(i))
    }

    pub fn find(&self, key: StepKey) -> Option<&BuildStep> {
        self.steps.iter().find(|s| s.key == key)
    }

    pub fn status(&self, store: &CommissioningStore, step: &BuildStep) -> StepStatus {
        match store.build_tick(step.key) {
            None => StepStatus::Open,
            Some(t) if t.hash != step.fingerprint => StepStatus::Changed,
            Some(t) => match t.state {
                TickState::Done => StepStatus::Done,
                TickState::Problem => StepStatus::Problem,
            },
        }
    }

    pub fn progress(&self, store: &CommissioningStore) -> BuildProgress {
        self.progress_of(store, |_| true)
    }

    pub fn phase_progress(&self, store: &CommissioningStore, phase: BuildPhase) -> BuildProgress {
        self.progress_of(store, |s| s.phase == phase)
    }

    fn progress_of(
        &self,
        store: &CommissioningStore,
        keep: impl Fn(&BuildStep) -> bool,
    ) -> BuildProgress {
        let mut p = BuildProgress::default();
        for step in self.steps.iter().filter(|s| keep(s)) {
            p.total += 1;
            match self.status(store, step) {
                StepStatus::Done => p.done += 1,
                StepStatus::Problem => p.problems += 1,
                StepStatus::Changed => {
                    p.changed += 1;
                }
                StepStatus::Open => {}
            }
        }
        p
    }

    fn is_done(&self, store: &CommissioningStore, number: usize) -> bool {
        self.step(number)
            .is_some_and(|s| self.status(store, s) == StepStatus::Done)
    }

    /// The step to do next: the first not-done, not-parked step whose
    /// prerequisites are done.
    pub fn next(&self, store: &CommissioningStore) -> Next {
        let mut first_waiting: Option<(usize, usize)> = None;
        for step in &self.steps {
            if matches!(
                self.status(store, step),
                StepStatus::Done | StepStatus::Problem
            ) {
                continue;
            }
            match step.after.iter().find(|&&a| !self.is_done(store, a)) {
                None => return Next::Ready(step.number),
                Some(&a) => {
                    first_waiting.get_or_insert((step.number, a));
                }
            }
        }
        match first_waiting {
            Some((step, waits_for)) => Next::Blocked { step, waits_for },
            None => Next::Finished,
        }
    }

    /// [`CabinetPlan::next`] for `who`: when the next step is held by someone
    /// else and another ready step is free (or theirs), that one.
    pub fn next_for(&self, store: &CommissioningStore, who: &str) -> Next {
        let next = self.next(store);
        if let Next::Ready(n) = next
            && !self.ready_for(store, who).contains(&n)
            && let Some(&other) = self.ready_for(store, who).first()
        {
            return Next::Ready(other);
        }
        next
    }

    /// The ready steps nobody else holds: what `who` can take up now.
    pub fn ready_for(&self, store: &CommissioningStore, who: &str) -> Vec<usize> {
        self.ready(store)
            .into_iter()
            .filter(|&n| {
                self.step(n)
                    .and_then(|s| store.build_claim(s.key))
                    .is_none_or(|c| c.by == who)
            })
            .collect()
    }

    /// The steps that can be done now, in order: lets two people work on
    /// different devices at once.
    pub fn ready(&self, store: &CommissioningStore) -> Vec<usize> {
        self.steps
            .iter()
            .filter(|s| {
                matches!(
                    self.status(store, s),
                    StepStatus::Open | StepStatus::Changed
                )
            })
            .filter(|s| s.after.iter().all(|&a| self.is_done(store, a)))
            .map(|s| s.number)
            .collect()
    }
}

/// The plan of every cabinet with a mounting plate.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildPlan {
    pub cabinets: Vec<CabinetPlan>,
}

impl BuildPlan {
    /// Reads the project. Routes every wire, so callers keep the result
    /// until the project changes (`Project::revision`).
    pub fn of(project: &Project) -> BuildPlan {
        let jobs = novo_reports::wire_cut_jobs(project);
        let mut cabinets: Vec<CabinetPlan> = project
            .structures()
            .filter(|s| s.enclosure && s.plate.is_some())
            .filter_map(|s| cabinet(project, s, &jobs))
            .collect();
        cabinets.sort_by(|a, b| a.tag.cmp(&b.tag).then(a.cabinet.cmp(&b.cabinet)));
        BuildPlan { cabinets }
    }

    pub fn cabinet(&self, id: u64) -> Option<&CabinetPlan> {
        self.cabinets.iter().find(|c| c.cabinet == id)
    }

    pub fn find(&self, key: StepKey) -> Option<(&CabinetPlan, &BuildStep)> {
        let c = self.cabinet(key.cabinet)?;
        Some((c, c.find(key)?))
    }

    pub fn is_empty(&self) -> bool {
        self.cabinets.iter().all(|c| c.steps.is_empty())
    }

    pub fn progress(&self, store: &CommissioningStore) -> BuildProgress {
        self.cabinets
            .iter()
            .map(|c| c.progress(store))
            .fold(BuildProgress::default(), |a, b| BuildProgress {
                total: a.total + b.total,
                done: a.done + b.done,
                problems: a.problems + b.problems,
                changed: a.changed + b.changed,
            })
    }
}

fn hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest[..8].iter().map(|b| format!("{b:02x}")).collect()
}

/// A number that survives renaming of the surface: FNV-1a of its name.
fn surface_id(surface: &Surface) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in surface.name().bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    h >> 20
}

/// One decimal, for positions that are marked and drilled by hand.
fn mm1(v: f64) -> String {
    let s = format!("{v:.1}");
    s.strip_suffix(".0").map_or(s.clone(), str::to_owned)
}

fn mm(v: f64) -> String {
    novo_layout::drill::mm(v)
}

/// The usual practice, not a reading of the schematic: green-yellow is
/// protective earth, 2.5 mm² and up is power, the rest is control.
fn circuit_rank(job: &WireCutJob) -> u8 {
    let colour = job.colour.to_lowercase();
    // Whole words, so "grey pearl" is not earth.
    let words: Vec<&str> = colour
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    let earth = has("gnye") || has("pe") || (has("green") && has("yellow"));
    if earth {
        2
    } else if job.cross_section_mm2.is_some_and(|s| s >= 2.5) {
        0
    } else {
        1
    }
}

fn circuit_name(rank: u8) -> &'static str {
    match rank {
        0 => "power",
        2 => "earth",
        _ => "control",
    }
}

fn location_of(designation: &str) -> Option<Vec<String>> {
    let tag = designation.split(':').next()?;
    DeviceTag::parse(tag).ok().map(|t| t.location)
}

fn cabinet(project: &Project, enclosure: &Structure, jobs: &[WireCutJob]) -> Option<CabinetPlan> {
    let layout = novo_layout::model::load(project, enclosure.id)?;
    let cab = enclosure.id.raw();
    let tag = enclosure.tag.to_string();
    let name = |id: u64| {
        project
            .get::<novo_core::Device>(novo_core::DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_else(|| format!("device {id}"))
    };

    // The connections with an end in this cabinet, one circuit at a time.
    // The sort is stable, so inside a circuit the production order stays.
    let here = |designation: &str| {
        !enclosure.tag.location.is_empty()
            && location_of(designation).is_some_and(|l| l == enclosure.tag.location)
    };
    let mut mine: Vec<&WireCutJob> = jobs
        .iter()
        .filter(|j| here(&j.end_a.designation) || here(&j.end_b.designation))
        .collect();
    mine.sort_by_key(|j| circuit_rank(j));
    let wire_jobs: Vec<WireJob> = mine
        .iter()
        .map(|j| WireJob {
            name: j.number.clone(),
            text: String::new(),
        })
        .collect();
    let base = steps(&layout, &name, &wire_jobs);

    let drill = novo_layout::model::plate_documents(project, enclosure.id).map(|(_, t)| t);
    let report = layout.report();
    let unplaced: Vec<u64> = novo_layout::model::candidates(project, enclosure.id)
        .into_iter()
        .filter(|c| !layout.placements.iter().any(|p| p.id == c.id))
        .map(|c| c.id)
        .collect();

    let mut out: Vec<BuildStep> = Vec::new();
    let mut push = |phase: BuildPhase,
                    subject: u64,
                    mark: String,
                    title: String,
                    details: Vec<String>,
                    caution: Option<String>,
                    after: Vec<usize>,
                    device: Option<u64>,
                    wires: Vec<WireId>|
     -> usize {
        let number = out.len() + 1;
        let fingerprint = hash(&format!("{mark}|{title}|{}", details.join("|")));
        out.push(BuildStep {
            key: StepKey {
                cabinet: cab,
                phase,
                subject,
            },
            number,
            phase,
            mark,
            title,
            details,
            caution,
            after,
            device,
            surface: None,
            wires,
            fingerprint,
        });
        number
    };

    // 1. Prepare: the plan itself, before anything is cut.
    let tags = |ids: &[u64]| {
        let mut t: Vec<String> = ids.iter().map(|&i| name(i)).collect();
        t.sort();
        t.join(", ")
    };
    let mut warnings: Vec<String> = Vec::new();
    if !unplaced.is_empty() {
        warnings.push(format!(
            "Devices not placed on the plate: {}",
            tags(&unplaced)
        ));
    }
    if !report.collisions.is_empty() {
        warnings.push(format!(
            "{} overlapping device pairs",
            report.collisions.len()
        ));
    }
    if !report.too_close.is_empty() {
        warnings.push(format!(
            "{} pairs closer than their clearance",
            report.too_close.len()
        ));
    }
    if !report.outside.is_empty() {
        warnings.push(format!(
            "Outside the plate margin: {}",
            tags(&report.outside)
        ));
    }
    if !report.rail_overflows.is_empty() {
        warnings.push(format!(
            "{} devices overflow their rail",
            report.rail_overflows.len()
        ));
    }
    if !report.guessed.is_empty() {
        warnings.push(format!(
            "Part size not known, a guess was used: {}",
            tags(&report.guessed)
        ));
    }
    if let Some(t) = &drill
        && !t.problems.is_empty()
    {
        warnings.push(format!(
            "Unreadable drill patterns: {}",
            t.problems.join("; ")
        ));
    }
    let prepare = push(
        BuildPhase::Prepare,
        0,
        tag.clone(),
        format!(
            "Check the plate {} x {} mm and the plan before you start",
            mm(layout.plate.width),
            mm(layout.plate.height)
        ),
        if warnings.is_empty() {
            vec!["The layout has no open problems.".to_owned()]
        } else {
            warnings.clone()
        },
        (!warnings.is_empty()).then(|| {
            "The layout has problems. Fix them on the Cabinet canvas first, or the steps below may not fit.".to_owned()
        }),
        vec![],
        None,
        vec![],
    );

    // 2. Drill: one diameter at a time, smallest first, then cut-outs.
    let mut drill_steps: Vec<usize> = Vec::new();
    if let Some(table) = &drill {
        for (surface, dia, count) in table.summary() {
            let rows: Vec<String> = table
                .holes
                .iter()
                .filter(|h| h.surface == surface && h.dia == dia)
                .map(|h| format!("({}, {})", mm1(h.x), mm1(h.y)))
                .collect();
            let on = surface.name();
            let n = push(
                BuildPhase::Drill,
                (surface_id(&surface) << 20) | (dia * 100.0).round() as u64,
                format!("dia {} mm", mm(dia)),
                format!(
                    "Drill {count} hole{} dia {} mm in the {}",
                    if count == 1 { "" } else { "s" },
                    mm(dia),
                    on.to_lowercase()
                ),
                vec![
                    "Positions in mm from the top left corner, across then down:".to_owned(),
                    rows.join("  "),
                ],
                Some(
                    "Marked with the 1:1 template (Reports > Workshop pack) and drilled by hand: centre-punch each point first."
                        .to_owned(),
                ),
                vec![prepare],
                None,
                vec![],
            );
            drill_steps.push(n);
        }
        for c in &table.cutouts {
            let n = push(
                BuildPhase::Drill,
                (1 << 62) | (surface_id(&c.surface) << 20) | c.no as u64,
                format!("C{}", c.no),
                format!(
                    "Cut out {} in the {}, centre {} mm right and {} mm down from the top left",
                    c.shape.describe(),
                    c.surface.name().to_lowercase(),
                    mm1(c.x),
                    mm1(c.y)
                ),
                vec![format!("For {}", c.source)],
                Some("Marked from the template and cut by hand: file to the line.".to_owned()),
                vec![prepare],
                None,
                vec![],
            );
            drill_steps.push(n);
        }
    }
    let mut after_prepare = vec![prepare];
    after_prepare.extend(&drill_steps);

    // 3..: the layout engine's order. Its step numbers are mapped to ours.
    let mut map: BTreeMap<usize, usize> = BTreeMap::new();
    let mut wire_at = 0usize;
    let mut seen_wires: BTreeMap<u64, u64> = BTreeMap::new();
    let mut device_numbers: Vec<usize> = Vec::new();
    for s in &base {
        let mapped_after = |map: &BTreeMap<usize, usize>| -> Vec<usize> {
            let mut v: Vec<usize> = s.after.iter().filter_map(|a| map.get(a).copied()).collect();
            if matches!(s.phase, Phase::Rails | Phase::Ducts) {
                v = after_prepare.clone();
            }
            v
        };
        let n = match s.phase {
            Phase::Rails | Phase::Ducts => {
                let phase = if s.phase == Phase::Rails {
                    BuildPhase::Rails
                } else {
                    BuildPhase::Ducts
                };
                push(
                    phase,
                    s.subject.unwrap_or(0),
                    s.mark.clone(),
                    s.text.clone(),
                    vec![],
                    None,
                    mapped_after(&map),
                    None,
                    vec![],
                )
            }
            Phase::Devices => {
                let id = s.subject.unwrap_or(0);
                let mut details = Vec::new();
                if let Some(d) = project.get::<novo_core::Device>(novo_core::DeviceId::from(id)) {
                    details.push(part_text(project, d));
                }
                if let Some(surface) = layout.off_plate.get(&id) {
                    details.extend(surface_position(
                        project,
                        enclosure.id,
                        &layout,
                        id,
                        surface,
                    ));
                }
                let guessed = report.guessed.contains(&id);
                let title = match layout.off_plate.get(&id) {
                    Some(surface) => format!("Fit {} on the {surface}", s.mark),
                    None => s.text.clone(),
                };
                let n = push(
                    BuildPhase::Devices,
                    id,
                    s.mark.clone(),
                    title,
                    details,
                    guessed.then(|| {
                        "The size of this part is a guess: hold the device against the plate before you drill or clip."
                            .to_owned()
                    }),
                    {
                        let mut a = mapped_after(&map);
                        // A device on the bare plate is screwed on after the
                        // drilling, a clip-on device after its rail.
                        if a.is_empty() {
                            a = after_prepare.clone();
                        }
                        a
                    },
                    Some(id),
                    vec![],
                );
                device_numbers.push(n);
                n
            }
            Phase::Wires => {
                let job = mine[wire_at];
                wire_at += 1;
                let end = |label: &str, e: &novo_reports::WireEnd, here: bool| {
                    format!(
                        "{label} {}{}{}{}",
                        e.designation,
                        e.strip_mm
                            .map(|s| format!(", strip {} mm", mm(s)))
                            .unwrap_or_default(),
                        e.ferrule
                            .as_ref()
                            .map(|f| format!(", ferrule {f}"))
                            .unwrap_or_default(),
                        if here { "" } else { " (in another cabinet)" }
                    )
                };
                let mut details = vec![
                    end("From", &job.end_a, here(&job.end_a.designation)),
                    end("To", &job.end_b, here(&job.end_b.designation)),
                ];
                if !job.colour.is_empty() || job.cross_section_mm2.is_some() {
                    details.push(format!(
                        "{}{} mm²",
                        if job.colour.is_empty() {
                            String::new()
                        } else {
                            format!("{}, ", job.colour)
                        },
                        job.cross_section_mm2.map_or("?".to_owned(), mm)
                    ));
                }
                let estimate = novo_reports::is_estimate(&job.basis);
                let mut caution = Vec::new();
                if estimate {
                    caution.push(format!(
                        "The length is an estimate (basis: {}). Cut long and trim at the terminal.",
                        job.basis
                    ));
                }
                if job.cross_section_mm2.is_none() {
                    caution.push("No cross-section is set for this wire.".to_owned());
                }
                // A drawn wire can be part of two connections: the second
                // one gets its own key.
                let first = job.wires.first().map_or(0, |w| w.raw());
                let again = seen_wires.entry(first).or_insert(0u64);
                let key_wire = first | (*again << 40);
                *again += 1;
                push(
                    BuildPhase::Wires,
                    key_wire,
                    if job.number.is_empty() {
                        "(no number)".to_owned()
                    } else {
                        job.number.clone()
                    },
                    format!(
                        "Cut {} mm and run the {} wire {}",
                        mm(job.cut_mm),
                        circuit_name(circuit_rank(job)),
                        if job.number.is_empty() {
                            String::new()
                        } else {
                            job.number.clone()
                        }
                    )
                    .trim_end()
                    .to_owned(),
                    details,
                    (!caution.is_empty()).then(|| caution.join(" ")),
                    device_numbers.clone(),
                    None,
                    job.wires.clone(),
                )
            }
            Phase::Labels => push(
                BuildPhase::Labels,
                s.subject.unwrap_or(0),
                s.mark.clone(),
                s.text.clone(),
                vec![],
                None,
                mapped_after(&map),
                s.subject,
                vec![],
            ),
        };
        map.insert(s.number, n);
    }

    // The last step: what a person confirms by eye.
    let everything: Vec<usize> = (1..=map
        .values()
        .chain(&drill_steps)
        .copied()
        .max()
        .unwrap_or(prepare)
        .max(prepare))
        .collect();
    for (i, text) in [
        "Every terminal screw is tightened and every wire pulled once",
        "The wiring is compared with the schematic",
        "Labels are fixed and readable",
    ]
    .into_iter()
    .enumerate()
    {
        push(
            BuildPhase::Check,
            i as u64,
            tag.clone(),
            text.to_owned(),
            vec![],
            None,
            everything.clone(),
            None,
            vec![],
        );
    }
    for step in &mut out {
        step.surface = step
            .device
            .filter(|_| step.phase == BuildPhase::Devices)
            .and_then(|d| layout.off_plate.get(&d).cloned());
    }
    Some(CabinetPlan {
        cabinet: cab,
        tag,
        steps: out,
    })
}

/// Where a device off the mounting plate goes, in words a person with a
/// ruler can follow. The layout keeps its position as the top left corner in
/// the surface's own millimetres; the door is stored as seen from the front,
/// but a person fits it on the inside of the open door, so it is given from
/// the hinge edge, which is the same from either side.
fn surface_position(
    project: &Project,
    structure: novo_core::StructureId,
    layout: &novo_layout::state::CabinetLayout,
    id: u64,
    surface: &str,
) -> Vec<String> {
    let Some(p) = layout.placements.iter().find(|p| p.id == id) else {
        return Vec::new();
    };
    let assumed = layout.enclosure.is_some_and(|e| {
        novo_layout::surfaces::Surfaces::from_enclosure(
            &e,
            &layout.plate,
            &novo_layout::model::extra_surfaces(project, structure),
        )
        .0
        .list
        .iter()
        .any(|s| s.name == surface && s.assumed)
    });
    position_lines(
        surface,
        p.footprint(),
        layout.enclosure.and_then(|e| e.door).map(|d| d.hinge),
        layout.surface_sizes.get(surface).copied(),
        assumed,
    )
}

/// The words for a footprint `r` on `surface`; separate from the model so
/// each surface's wording can be tested on its own.
fn position_lines(
    surface: &str,
    r: novo_layout::Rect,
    hinge: Option<novo_layout::enclosure::Hinge>,
    size: Option<(f64, f64)>,
    assumed: bool,
) -> Vec<String> {
    use novo_layout::enclosure::Hinge;
    use novo_layout::surfaces::SurfaceKind;
    let label = surface_label(surface);
    let line = match SurfaceKind::from_name(surface) {
        Some(SurfaceKind::Door) => {
            let from_hinge = match (hinge, size) {
                (Some(Hinge::Right), Some((w, _))) => Some(w - r.x - r.w),
                (Some(Hinge::Left), _) => Some(r.x),
                _ => None,
            };
            match from_hinge {
                Some(x) => format!(
                    "{label}: {} mm from the hinge edge and {} mm down from the top edge, to the nearest corner of the device, on the inside of the door",
                    mm1(x),
                    mm1(r.y)
                ),
                None => format!(
                    "{label}: {} mm from the left edge seen from the front and {} mm down from the top edge",
                    mm1(r.x),
                    mm1(r.y)
                ),
            }
        }
        Some(SurfaceKind::LeftWall | SurfaceKind::RightWall) => format!(
            "{label}: {} mm from the back wall and {} mm down from the top edge, on the inside",
            mm1(r.x),
            mm1(r.y)
        ),
        Some(SurfaceKind::Roof | SurfaceKind::Floor) => format!(
            "{label}: {} mm from the left seen from the front and {} mm in from the back wall, on the inside",
            mm1(r.x),
            mm1(r.y)
        ),
        _ => format!(
            "{label}: {} mm across and {} mm down from the corner the Cabinet's Surface picker takes as 0, 0",
            mm1(r.x),
            mm1(r.y)
        ),
    };
    let mut lines = vec![
        line,
        format!(
            "The device is {} x {} mm; this position is the planned one, check it against the real device",
            mm1(r.w),
            mm1(r.h)
        ),
    ];
    if assumed {
        let size = size.map_or(String::new(), |(w, h)| {
            format!(" ({} x {} mm)", mm1(w), mm1(h))
        });
        lines.push(format!(
            "The {} is planned from the enclosure's inside size{size} with no wall thickness, not measured: measure the real one before you mark it",
            label.to_lowercase()
        ));
    }
    lines
}

fn surface_label(surface: &str) -> String {
    let mut c = surface.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

fn part_text(project: &Project, device: &novo_core::Device) -> String {
    let parts: Vec<String> = device
        .parts
        .iter()
        .filter_map(|id| project.get::<novo_core::Part>(*id))
        .map(|p| {
            let what = if p.type_number.is_empty() {
                &p.order_number
            } else {
                &p.type_number
            };
            format!("{} {}", p.manufacturer, what).trim().to_owned()
        })
        .collect();
    if parts.is_empty() {
        "No part assigned".to_owned()
    } else {
        parts.join(" + ")
    }
}

/// Ticks a step done. A claim on it is let go, so an undone step does not
/// come back with an old claim.
pub fn mark_done(store: &mut CommissioningStore, step: &BuildStep, by: &str, at: &str) {
    release(store, step, at);
    store.put_build(BuildTick {
        key: step.key,
        mark: step.mark.clone(),
        hash: step.fingerprint.clone(),
        state: TickState::Done,
        note: String::new(),
        by: by.to_owned(),
        at: at.to_owned(),
    });
}

/// `by` takes the step. Nothing stops another person from doing it: a claim
/// is a signal, and taking a step over replaces it.
pub fn claim(store: &mut CommissioningStore, step: &BuildStep, by: &str, at: &str) {
    store.put_claim(BuildClaim {
        key: step.key,
        mark: step.mark.clone(),
        by: by.to_owned(),
        at: at.to_owned(),
    });
}

/// Lets go of a step (the record stays, empty, so the release merges).
pub fn release(store: &mut CommissioningStore, step: &BuildStep, at: &str) {
    if store.build_claim_record(step.key).is_some() {
        store.put_claim(BuildClaim {
            key: step.key,
            mark: step.mark.clone(),
            by: String::new(),
            at: at.to_owned(),
        });
    }
}

/// Parks a step as a problem and raises a punch item with the note, so
/// engineering sees it in the same list as the commissioning findings.
/// Returns the punch item's id.
pub fn mark_problem(
    store: &mut CommissioningStore,
    cabinet_tag: &str,
    step: &BuildStep,
    note: &str,
    by: &str,
    at: &str,
) -> u64 {
    store.put_build(BuildTick {
        key: step.key,
        mark: step.mark.clone(),
        hash: step.fingerprint.clone(),
        state: TickState::Problem,
        note: note.to_owned(),
        by: by.to_owned(),
        at: at.to_owned(),
    });
    let title = format!("Build {cabinet_tag} step {}: {}", step.number, step.title);
    // A step parked again after Back reuses its open item.
    let existing = store
        .punch_items()
        .find(|p| p.title == title && p.state == PunchState::Open)
        .cloned();
    if let Some(item) = existing {
        let mut item = item;
        item.note = note.to_owned();
        let id = item.id;
        store.put_punch(item);
        return id;
    }
    let target = step
        .device
        .filter(|_| step.phase == BuildPhase::Devices)
        .map(|d| Target::Device(novo_core::DeviceId::from(d)))
        .or_else(|| step.wires.first().map(|&w| Target::Wire(w)));
    store.add_punch(PunchItem {
        id: 0,
        title,
        note: note.to_owned(),
        severity: Severity::Major,
        state: PunchState::Open,
        target,
        check: None,
        raised_by: by.to_owned(),
        raised_at: at.to_owned(),
        closed_by: None,
        closed_at: None,
        photos: Vec::new(),
        signature: None,
    })
}

/// Takes a tick back (a mis-tap). A punch item raised by a problem stays.
pub fn undo(store: &mut CommissioningStore, key: StepKey) {
    store.clear_build(key);
}

/// Keys of the steps in `plan`, for pruning ticks of steps that no longer
/// exist.
pub fn keys(plan: &BuildPlan) -> BTreeSet<StepKey> {
    plan.cabinets
        .iter()
        .flat_map(|c| c.steps.iter().map(|s| s.key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_layout::Rect;
    use novo_layout::enclosure::Hinge;

    // A 40 x 30 mm device with its top left corner at (80, 120).
    const R: Rect = Rect {
        x: 80.0,
        y: 120.0,
        w: 40.0,
        h: 30.0,
    };

    fn text(lines: Vec<String>) -> String {
        lines.join(" | ")
    }

    #[test]
    fn the_door_is_measured_from_the_hinge_edge_whichever_side_the_hinge_is() {
        let left = text(position_lines(
            "door",
            R,
            Some(Hinge::Left),
            Some((700.0, 900.0)),
            false,
        ));
        assert!(
            left.contains("80 mm from the hinge edge and 120 mm down"),
            "{left}"
        );
        // The hinge on the right: the door is 700 wide, the device ends at 120.
        let right = text(position_lines(
            "door",
            R,
            Some(Hinge::Right),
            Some((700.0, 900.0)),
            false,
        ));
        assert!(
            right.contains("580 mm from the hinge edge and 120 mm down"),
            "{right}"
        );
        // Without a hinge or a width the front view is said as it is.
        let none = text(position_lines("door", R, None, None, false));
        assert!(
            none.contains("80 mm from the left edge seen from the front"),
            "{none}"
        );
    }

    #[test]
    fn walls_roof_floor_and_extra_plates_each_say_their_own_measures() {
        for wall in ["left", "right"] {
            let t = text(position_lines(wall, R, None, None, false));
            assert!(
                t.contains("80 mm from the back wall and 120 mm down from the top edge"),
                "{t}"
            );
        }
        for deck in ["roof", "floor"] {
            let t = text(position_lines(deck, R, None, None, false));
            assert!(
                t.contains(
                    "80 mm from the left seen from the front and 120 mm in from the back wall"
                ),
                "{t}"
            );
        }
        let plate = text(position_lines("plate2", R, None, None, false));
        assert!(
            plate.contains("Plate2: 80 mm across and 120 mm down from the corner"),
            "{plate}"
        );
    }

    #[test]
    fn an_assumed_surface_says_it_is_planned_not_measured() {
        let t = text(position_lines(
            "door",
            R,
            Some(Hinge::Left),
            Some((700.0, 900.0)),
            true,
        ));
        assert!(t.contains("planned from the enclosure's inside size (700 x 900 mm) with no wall thickness, not measured"), "{t}");
        let known = text(position_lines(
            "door",
            R,
            Some(Hinge::Left),
            Some((700.0, 900.0)),
            false,
        ));
        assert!(!known.contains("not measured"), "{known}");
    }
}
