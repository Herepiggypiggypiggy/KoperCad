//! Test as you build: the point-to-point continuity and insulation test
//! sheet, in wiring order.
//!
//! Every drawn wire is a test point. [`TestSheet::of`] lists them in the
//! order the wires are put in ([`wiring_order`]: the Build mode wire steps,
//! so a better wiring order there reaches this sheet too), each with its
//! expected values from [`Expected`]. A measured value is entered with
//! [`record_reading`]; a miss goes through the Commissioning store, so it
//! sets the wire's Continuity check to Failed and opens a punch item (an
//! insulation miss opens a Blocking one). [`untested`] is the dead wire
//! cross-check: a wire of the drawing nobody has measured.
//!
//! Honest labelling: the expected values are planning values worked out from
//! the drawn length and cross-section (copper at 20 degrees C with a margin
//! and a contact allowance) and the settings below. This is a design aid for
//! the bench, not a compliance test record, and it quotes no standard.

use std::collections::HashSet;

use novo_core::{Project, WireId};
use novo_reports::{Cell, Column, ReportOutput, Section};
use serde::{Deserialize, Serialize};

use crate::build::{BuildPhase, BuildPlan};
use crate::check::{CheckKind, CheckRecord, CheckState, Target};
use crate::punch::{PunchItem, PunchState, Severity};
use crate::store::CommissioningStore;
use crate::walk::{WalkScope, WireKind, WirePlan, walk_order};

/// Resistivity of copper at 20 degrees C, ohm mm2 per metre.
pub const COPPER_OHM_MM2_PER_M: f64 = 0.0178;
/// Said on the sheet.
pub const DISCLAIMER: &str = "Design aid: expected values are planning values from the drawing, not a compliance test record.";

/// What the sheet expects. Planning values the user can change.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TestSettings {
    /// Also ask for an insulation reading on power and control wires.
    pub insulation: bool,
    /// Test voltage in volts d.c., a planning value.
    pub insulation_volts: u32,
    /// The least reading that passes, in megohm.
    pub min_insulation_mohm: f64,
    /// Both ends together: ferrule, terminal and probe contact, in ohm.
    pub contact_ohm: f64,
    /// Factor on the calculated wire resistance (temperature, tolerance).
    pub margin: f64,
    /// The limit used when length or cross-section is not known, in ohm.
    pub fallback_ohm: f64,
}

impl TestSettings {
    /// The settings with every field that is not a usable number (NaN,
    /// infinite, negative, a zero voltage or margin) replaced by its default,
    /// so a bad value can never be kept and make the project file unreadable.
    pub fn sanitised(self) -> TestSettings {
        let d = TestSettings::default();
        let ok = |v: f64, min: f64| v.is_finite() && v >= min;
        TestSettings {
            insulation: self.insulation,
            insulation_volts: if self.insulation_volts == 0 {
                d.insulation_volts
            } else {
                self.insulation_volts
            },
            min_insulation_mohm: if ok(self.min_insulation_mohm, 0.0) {
                self.min_insulation_mohm
            } else {
                d.min_insulation_mohm
            },
            contact_ohm: if ok(self.contact_ohm, 0.0) {
                self.contact_ohm
            } else {
                d.contact_ohm
            },
            margin: if ok(self.margin, 1e-9) {
                self.margin
            } else {
                d.margin
            },
            fallback_ohm: if ok(self.fallback_ohm, 0.0) {
                self.fallback_ohm
            } else {
                d.fallback_ohm
            },
        }
    }
}

impl Default for TestSettings {
    fn default() -> Self {
        TestSettings {
            insulation: true,
            insulation_volts: 500,
            min_insulation_mohm: 1.0,
            contact_ohm: 0.05,
            margin: 1.25,
            fallback_ohm: 0.5,
        }
    }
}

/// Which circuit a wire belongs to, by the usual practice (the same
/// grouping Build mode uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Circuit {
    Power,
    Control,
    Earth,
}

impl Circuit {
    pub fn label(self) -> &'static str {
        match self {
            Circuit::Power => "power",
            Circuit::Control => "control",
            Circuit::Earth => "earth",
        }
    }
}

/// How the continuity limit was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Basis {
    /// From the drawn length and cross-section.
    Calculated,
    /// The fallback limit: length or cross-section is missing.
    Fallback,
}

/// The values a good wire should give.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Expected {
    /// The most resistance end to end that passes, in ohm.
    pub max_ohm: f64,
    pub basis: Basis,
    /// Test voltage and least megohm; `None` for earth wires and when off.
    pub insulation: Option<(u32, f64)>,
}

impl Expected {
    pub fn of(
        circuit: Circuit,
        length_mm: Option<f64>,
        section_mm2: Option<f64>,
        s: &TestSettings,
    ) -> Expected {
        let (max_ohm, basis) = match (length_mm, section_mm2) {
            (Some(l), Some(a)) if l > 0.0 && a > 0.0 => (
                COPPER_OHM_MM2_PER_M * (l / 1000.0) / a * s.margin + s.contact_ohm,
                Basis::Calculated,
            ),
            _ => (s.fallback_ohm, Basis::Fallback),
        };
        Expected {
            max_ohm,
            basis,
            insulation: (s.insulation && circuit != Circuit::Earth)
                .then_some((s.insulation_volts, s.min_insulation_mohm)),
        }
    }

    /// `<= 0.31 ohm` (`<= 0.50 ohm (assumed)` on the fallback).
    pub fn continuity_text(&self) -> String {
        let assumed = if self.basis == Basis::Fallback {
            " (assumed)"
        } else {
            ""
        };
        format!("<= {:.3} ohm{assumed}", self.max_ohm)
    }

    /// `500 V >= 1 Mohm`, empty when no insulation test is asked.
    pub fn insulation_text(&self) -> String {
        self.insulation
            .map(|(v, m)| format!("{v} V >= {m} Mohm"))
            .unwrap_or_default()
    }
}

/// One wire as a test point.
#[derive(Debug, Clone, PartialEq)]
pub struct TestPoint {
    /// 1-based place in the wiring order.
    pub seq: usize,
    pub wire: WireId,
    /// The cabinet whose Build mode wire step this is, `None` for a wire
    /// outside any cabinet plan.
    pub cabinet: Option<String>,
    pub number: String,
    pub kind: WireKind,
    pub from: String,
    pub to: String,
    pub colour: String,
    pub section_mm2: Option<f64>,
    pub length_mm: Option<f64>,
    pub circuit: Circuit,
    pub expected: Expected,
    /// "Same number also on ..." when the number is drawn more than once.
    pub hint: String,
}

/// The wires in the order they are put in: per cabinet the Build mode wire
/// steps in order, then every other drawn wire in the drawing's order.
/// This is the one seam to the wiring order; a better order in Build mode
/// changes the sheet without a change here.
pub fn wiring_order(project: &Project, build: &BuildPlan) -> Vec<(Option<String>, WireId)> {
    let mut seen: HashSet<WireId> = HashSet::new();
    let mut out = Vec::new();
    for cab in &build.cabinets {
        for step in cab.steps.iter().filter(|s| s.phase == BuildPhase::Wires) {
            for &w in &step.wires {
                if project.wire(w).is_some() && seen.insert(w) {
                    out.push((Some(cab.tag.clone()), w));
                }
            }
        }
    }
    for info in walk_order(project, WalkScope::Wires) {
        if let Target::Wire(w) = info.target
            && seen.insert(w)
        {
            out.push((None, w));
        }
    }
    out
}

fn circuit_of(colour: &str, section: Option<f64>) -> Circuit {
    let lower = colour.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    if has("gnye") || has("pe") || (has("green") && has("yellow")) {
        Circuit::Earth
    } else if section.is_some_and(|s| s >= 2.5) {
        Circuit::Power
    } else {
        Circuit::Control
    }
}

/// The sheet: every wire as a test point in wiring order.
#[derive(Debug, Clone, PartialEq)]
pub struct TestSheet {
    pub points: Vec<TestPoint>,
    pub settings: TestSettings,
}

impl TestSheet {
    pub fn of(
        project: &Project,
        build: &BuildPlan,
        wires: &WirePlan,
        settings: TestSettings,
    ) -> TestSheet {
        let points = wiring_order(project, build)
            .into_iter()
            .filter_map(|(cabinet, w)| wires.step(w).map(|s| (cabinet, s)))
            .enumerate()
            .map(|(i, (cabinet, s))| {
                let circuit = circuit_of(&s.colour, s.cross_section_mm2);
                TestPoint {
                    seq: i + 1,
                    wire: s.wire,
                    cabinet,
                    number: s.number.clone(),
                    kind: s.kind.clone(),
                    from: s.from.clone(),
                    to: s.to.clone(),
                    colour: s.colour.clone(),
                    section_mm2: s.cross_section_mm2,
                    length_mm: s.length_mm,
                    circuit,
                    expected: Expected::of(circuit, s.length_mm, s.cross_section_mm2, &settings),
                    hint: s.same_number_hint(),
                }
            })
            .collect();
        TestSheet { points, settings }
    }

    pub fn point(&self, wire: WireId) -> Option<&TestPoint> {
        self.points.iter().find(|p| p.wire == wire)
    }
}

/// What was typed for the continuity reading.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ContinuityInput {
    Ohm(f64),
    /// The meter showed no connection.
    Open,
}

/// A reading entered at the bench. Either part may be left out.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ReadingInput {
    pub continuity: Option<ContinuityInput>,
    pub insulation_mohm: Option<f64>,
}

/// How a reading compares with what was expected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Miss,
}

/// The verdicts of one reading; `None` for a part not measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdicts {
    pub continuity: Option<Verdict>,
    pub insulation: Option<Verdict>,
}

pub fn judge(expected: &Expected, input: &ReadingInput) -> Verdicts {
    let continuity = input.continuity.map(|c| match c {
        ContinuityInput::Ohm(v) if v >= 0.0 && v <= expected.max_ohm => Verdict::Pass,
        _ => Verdict::Miss,
    });
    let insulation = match (input.insulation_mohm, expected.insulation) {
        (Some(v), Some((_, min))) => Some(if v >= min {
            Verdict::Pass
        } else {
            Verdict::Miss
        }),
        _ => None,
    };
    Verdicts {
        continuity,
        insulation,
    }
}

/// The text of a continuity reading as it is kept on the check:
/// `0.12 ohm (max 0.31)` or `open (max 0.31 ohm)`.
fn continuity_value(input: ContinuityInput, expected: &Expected) -> String {
    match input {
        ContinuityInput::Ohm(v) => {
            // Three decimals, more when the reading and the limit would
            // otherwise print the same and a miss would read as equal.
            let max = expected.max_ohm;
            let places = (3..=6)
                .find(|&d| format!("{v:.d$}") != format!("{max:.d$}"))
                .unwrap_or(3);
            format!("{v:.places$} ohm (max {max:.places$})")
        }
        ContinuityInput::Open => format!("open (max {:.3} ohm)", expected.max_ohm),
    }
}

/// What [`record_reading`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    pub verdicts: Verdicts,
    /// The punch items this reading opened.
    pub punch: [Option<u64>; 2],
}

/// Enters a reading for `point`. A continuity value decides the wire's
/// Continuity check (Passed, or Failed with a punch item); an insulation
/// miss opens its own Blocking punch item. A part left out changes nothing.
pub fn record_reading(
    store: &mut CommissioningStore,
    point: &TestPoint,
    input: ReadingInput,
    by: &str,
    at: &str,
) -> Recorded {
    // A reading that is not a finite number is not a reading.
    let input = ReadingInput {
        continuity: input.continuity.filter(|c| match c {
            ContinuityInput::Ohm(v) => v.is_finite(),
            ContinuityInput::Open => true,
        }),
        insulation_mohm: input.insulation_mohm.filter(|v| v.is_finite()),
    };
    let verdicts = judge(&point.expected, &input);
    let target = Target::Wire(point.wire);
    let mut punch = [None, None];
    if let (Some(c), Some(v)) = (input.continuity, verdicts.continuity) {
        let value = continuity_value(c, &point.expected);
        let state = if v == Verdict::Pass {
            CheckState::Passed
        } else {
            CheckState::Failed
        };
        let note = if v == Verdict::Miss {
            format!(
                "Test sheet #{}: {}->{} read {value}",
                point.seq, point.from, point.to
            )
        } else {
            String::new()
        };
        punch[0] = store
            .set_check(CheckRecord {
                target,
                check: CheckKind::Continuity,
                state,
                by: by.to_owned(),
                at: at.to_owned(),
                value,
                note,
            })
            .ok()
            .flatten();
    }
    if let (Some(m), Some(v)) = (input.insulation_mohm, verdicts.insulation) {
        store.put_insulation(point.wire, m, v == Verdict::Pass, by, at);
        if v == Verdict::Miss && store.open_insulation_punch(point.wire).is_none() {
            let (volts, min) = point.expected.insulation.unwrap_or((0, 0.0));
            punch[1] = Some(store.add_punch(PunchItem {
                id: 0,
                title: "Insulation low".to_owned(),
                note: format!(
                    "Test sheet #{}: {}->{} read {m:.2} Mohm at {volts} V (min {min:.2})",
                    point.seq, point.from, point.to
                ),
                severity: Severity::Blocking,
                state: PunchState::Open,
                target: Some(target),
                check: None,
                raised_by: by.to_owned(),
                raised_at: at.to_owned(),
                closed_by: None,
                closed_at: None,
                photos: Vec::new(),
                signature: None,
            }));
        }
    }
    Recorded { verdicts, punch }
}

/// The state of one point, for the sheet and the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointState {
    /// Nothing measured: the dead wire cross-check lists these.
    NotTested,
    /// Something passed but not everything asked: continuity not read yet,
    /// or insulation asked and not read. Not a dead wire.
    Partial,
    /// Continuity passed and so did the insulation reading, or none asked.
    Complete,
    /// A reading missed.
    Miss,
}

pub fn point_state(store: &CommissioningStore, point: &TestPoint) -> PointState {
    let c = store.state(Target::Wire(point.wire), CheckKind::Continuity);
    let ins = store.insulation(point.wire);
    if c == CheckState::Failed || ins.is_some_and(|i| !i.ok) {
        return PointState::Miss;
    }
    match (c, point.expected.insulation.is_some(), ins) {
        (CheckState::Passed, false, _) | (CheckState::Passed, true, Some(_)) => {
            PointState::Complete
        }
        (CheckState::NotApplicable, ..) => PointState::Complete,
        // Insulation read, continuity not: half measured.
        (CheckState::Pending, _, Some(_)) => PointState::Partial,
        (CheckState::Passed, true, None) => PointState::Partial,
        _ => PointState::NotTested,
    }
}

/// Wires that no reading has touched, in wiring order: the dead wire
/// cross-check. A wire in the drawing that the bench never measured.
pub fn untested<'a>(sheet: &'a TestSheet, store: &CommissioningStore) -> Vec<&'a TestPoint> {
    sheet
        .points
        .iter()
        .filter(|p| point_state(store, p) == PointState::NotTested)
        .collect()
}

/// Counts for a progress line.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TestProgress {
    pub total: usize,
    pub not_tested: usize,
    pub part: usize,
    pub complete: usize,
    pub miss: usize,
}

pub fn test_progress(sheet: &TestSheet, store: &CommissioningStore) -> TestProgress {
    let mut p = TestProgress {
        total: sheet.points.len(),
        ..Default::default()
    };
    for pt in &sheet.points {
        match point_state(store, pt) {
            PointState::NotTested => p.not_tested += 1,
            PointState::Partial => p.part += 1,
            PointState::Complete => p.complete += 1,
            PointState::Miss => p.miss += 1,
        }
    }
    p
}

/// What the Measured ohm cell says: the reading, `ticked` for a Continuity
/// check passed on the Checks tab with no value typed, blank box if none.
fn continuity_cell(check: Option<&CheckRecord>) -> String {
    match check {
        None => "[ ]".to_owned(),
        Some(c) if c.value.is_empty() && c.state == CheckState::Passed => "ticked".to_owned(),
        Some(c) if c.value.is_empty() => c.state.label().to_owned(),
        Some(c) => c.value.clone(),
    }
}

fn result_text(state: PointState, check: Option<&CheckRecord>) -> &'static str {
    match state {
        PointState::NotTested => "NOT TESTED",
        PointState::Partial if check.is_none_or(|c| c.state == CheckState::Pending) => {
            "PART: continuity not read"
        }
        PointState::Partial => "PART: insulation not read",
        PointState::Complete => "OK",
        PointState::Miss => "MISS",
    }
}

/// The printed sheet: one table per cabinet in wiring order, expected values
/// beside empty boxes (or the entered readings), then a dead wire section
/// listing what was never measured. Same state, same sheet.
pub fn test_sheet_report(
    sheet: &TestSheet,
    store: &CommissioningStore,
    revision: u64,
) -> ReportOutput {
    let columns = || {
        vec![
            Column::new("seq", "No.  ", 7.0).right(),
            Column::new("wire", "Wire                ", 28.0),
            Column::new("from", "From            ", 30.0),
            Column::new("to", "To              ", 30.0),
            Column::new("wire_type", "Type / mm2  ", 16.0),
            Column::new("cont", "Continuity expected   ", 24.0),
            Column::new("cont_m", "Measured ohm  ", 16.0),
            Column::new("ins", "Insulation expected   ", 24.0),
            Column::new("ins_m", "Measured Mohm  ", 16.0),
            Column::new("res", "Result      ", 22.0),
            Column::new("by", "By / when        ", 26.0),
        ]
    };
    let mut groups: Vec<(Option<String>, Vec<&TestPoint>)> = Vec::new();
    for p in &sheet.points {
        match groups.last_mut() {
            Some((c, v)) if *c == p.cabinet => v.push(p),
            _ => groups.push((p.cabinet.clone(), vec![p])),
        }
    }
    let progress = test_progress(sheet, store);
    let mut sections = Vec::new();
    for (i, (cab, pts)) in groups.iter().enumerate() {
        let name = cab
            .clone()
            .unwrap_or_else(|| "Wires outside a cabinet plan".into());
        let title = if i == 0 {
            format!(
                "Test sheet {name}: {} of {} wires tested, {} missed. {DISCLAIMER}",
                progress.total - progress.not_tested,
                progress.total,
                progress.miss
            )
        } else {
            format!("Test sheet {name}")
        };
        let mut section = Section::new(&format!("test:{i}"), &title, columns());
        section.flow = i > 0;
        for p in pts {
            let check = store.record(Target::Wire(p.wire), CheckKind::Continuity);
            let ins = store.insulation(p.wire);
            let state = point_state(store, p);
            let label = if p.number.is_empty() {
                p.kind.short()
            } else {
                p.number.clone()
            };
            let who = check
                .map(|c| (c.by.clone(), c.at.clone()))
                .or_else(|| ins.map(|i| (i.by.clone(), i.at.clone())))
                .map(|(b, a)| format!("{b} {}", a.get(5..16).unwrap_or(&a).replace('T', " ")))
                .unwrap_or_default();
            section.push(
                vec![
                    Cell::Int(p.seq as i64),
                    Cell::text(label),
                    Cell::text(p.from.clone()),
                    Cell::text(p.to.clone()),
                    Cell::text(format!(
                        "{} {}",
                        p.circuit.label(),
                        p.section_mm2
                            .map(|s| format!("{s} mm2"))
                            .unwrap_or_default()
                    )),
                    Cell::text(p.expected.continuity_text()),
                    Cell::text(continuity_cell(check)),
                    Cell::text(p.expected.insulation_text()),
                    Cell::text(match (ins, p.expected.insulation) {
                        (Some(i), _) => format!("{:.2}", i.mohm),
                        (None, Some(_)) => "[ ]".to_owned(),
                        (None, None) => "-".to_owned(),
                    }),
                    Cell::text(result_text(state, check).to_owned()),
                    Cell::text(who),
                ],
                Vec::new(),
            );
        }
        sections.push(section);
    }
    let dead = untested(sheet, store);
    if !sheet.points.is_empty() {
        let mut section = Section::new(
            "test:dead",
            &format!(
                "Dead wire cross-check: {} wire(s) of the drawing not measured yet",
                dead.len()
            ),
            vec![
                Column::new("seq", "No.  ", 7.0).right(),
                Column::new("wire", "Wire                ", 28.0),
                Column::new("ends", "From and to                    ", 70.0),
                Column::new("res", "Result      ", 22.0),
            ],
        );
        for p in &dead {
            section.push(
                vec![
                    Cell::Int(p.seq as i64),
                    Cell::text(if p.number.is_empty() {
                        p.kind.short()
                    } else {
                        p.number.clone()
                    }),
                    Cell::text(format!("{} -> {}", p.from, p.to)),
                    Cell::text("NOT TESTED".to_owned()),
                ],
                Vec::new(),
            );
        }
        sections.push(section);
    }
    ReportOutput {
        report: "test-sheet".into(),
        title: "Continuity and insulation test sheet".into(),
        revision,
        sections,
    }
}
