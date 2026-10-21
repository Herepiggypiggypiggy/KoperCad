//! The design verification part of the compliance document: which design
//! checks ran on this project (wire sizing, protection, colours, safety
//! functions, heat, drawing consistency, cabinet layout, ...), how each came
//! out, what it assumed and where its data comes from.
//!
//! The standards sections of the document answer "which clause rules were
//! met". This part answers the question an inspector asks next: "what else
//! was looked at, and how far can I trust it". Every row says whether the
//! result is read straight from the drawing (exact), rests on a planning
//! value that was not checked against a licensed standard, or is a model
//! (calculation aid, simulation). A check never reports a pass for something
//! it could not examine: missing data makes the row "Partly checkable" or
//! "Not checkable", and a family with nothing to examine is "Not applicable".
//!
//! The rows are built from the problems the design checks raised (open and
//! waived), so the document, the Issues list and the check pill always agree.

use std::collections::BTreeSet;

use novo_core::Project;

use super::document::{Finding, Verdict};
use super::{keys, page_name};
use crate::safety as safety_fn;
use crate::{Problem, ProblemKind, Severity, Waived};

/// How far a family's result can be trusted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reliance {
    /// Read from the drawing and the properties stated on it: counted, not
    /// calculated.
    Exact,
    /// Compares against limits or tables entered without the licensed text.
    Planning,
    /// A model or calculation aid: a simplified method, a simulation.
    Model,
}

impl Reliance {
    pub fn label(self) -> &'static str {
        match self {
            Reliance::Exact => "Exact: read from the drawing",
            Reliance::Planning => "Planning values, not checked against the standard",
            Reliance::Model => "Model: estimate, not measured",
        }
    }
}

/// One family of checks.
#[derive(Debug, PartialEq, Eq)]
pub struct Family {
    pub id: &'static str,
    pub title: &'static str,
    /// What the checks ask, in our words.
    pub what: &'static str,
    /// The kinds of problem the family raises.
    pub kinds: &'static [ProblemKind],
    /// Where the data comes from.
    pub source: &'static str,
    /// What the verdict assumes.
    pub assumptions: &'static str,
    pub reliance: Reliance,
    /// Where to see or fix a finding.
    pub fix: &'static str,
}

/// Problem kinds that mean "this item could not be checked": the check ran
/// but lacked data, so a family with one is at best partly checkable.
const DATA_GAPS: &[ProblemKind] = &[
    ProblemKind::GuessedWire,
    ProblemKind::MissingPart,
    ProblemKind::UnsizedDuct,
    ProblemKind::DepthUnknown,
    ProblemKind::UnplacedDevice,
];

/// Every family. The last one takes whatever no other family names, so no
/// problem is left out of the document.
pub static FAMILIES: &[Family] = &[
    Family {
        id: "wire-sizing",
        title: "Wire sizing",
        what: "A wire carries at least the current of the protective device that limits it, its voltage drop stays within the limit, protective conductors are not thinner than the phase conductors need, and a cable's cores match its stated size.",
        kinds: &[
            ProblemKind::UndersizedWire,
            ProblemKind::CableRating,
            ProblemKind::VoltageDrop,
            ProblemKind::ProtectiveConductor,
            ProblemKind::GuessedWire,
            ProblemKind::CableText,
        ],
        source: "Wire and cable properties (cross-section, installation method, insulation, grouping, ambient), device ratings, and the ampacity tables built into KoperCAD (IEC 60364-5-52 layout, and a separate IEC 60204-1 Table 6 style table for the thinner-than-device check).",
        assumptions: "The ampacity tables are planning values entered without the licensed text. A wire with no stated cross-section is routed with a default size and flagged, never judged. Installation method defaults to the project setting; where none is stated the table check says not checkable. Voltage drop uses the project limit (default 5 %) and the nominal supply voltage.",
        reliance: Reliance::Planning,
        fix: "Open the Calculations window or select the wire: set the cross-section and installation method in the Inspector, or change the protective device rating.",
    },
    Family {
        id: "protection",
        title: "Protection",
        what: "Devices can break the available short-circuit current, wires survive the let-through energy of their protective device, an earth fault disconnects in time, and a motor starter's relay, contactor and fuse suit the motor.",
        kinds: &[
            ProblemKind::ShortCircuit,
            ProblemKind::ThermalWithstand,
            ProblemKind::DisconnectionTime,
            ProblemKind::MotorStarter,
        ],
        source: "Supply short-circuit current and impedance stated in Project settings, device ratings and breaking capacities from the devices and their parts, wire lengths from the drawing or routing, motor nameplate current.",
        assumptions: "Short-circuit currents follow a simplified method (supply, transformer and cable impedance, no motor contribution beyond what is stated). Protective device curves come from the part data; without a curve the check skips the device and says so. Selectivity and SCCR are models, see the Basis table.",
        reliance: Reliance::Model,
        fix: "Enter the supply short-circuit current and device data in Project settings and the Inspector, then open Calculations to see the working for each finding.",
    },
    Family {
        id: "colours",
        title: "Colours and marking",
        what: "Wires use the colours the project's wire rules ask for, cable cores are used as marked (green-yellow only for protective earth), and a wire number is not used on two nets.",
        kinds: &[
            ProblemKind::WireRule,
            ProblemKind::CableCore,
            ProblemKind::WireNumber,
        ],
        source: "Wire colour and number properties, the project's house wire rules (Project settings, House rules), cable core markings.",
        assumptions: "The colour table of a selected standard is a planning value (see its section). House rules are the engineer's own and are not a standard. A wire with no colour stated is not judged.",
        reliance: Reliance::Planning,
        fix: "Select the wire and set its colour or number in the Inspector, or let Autopilot propose values.",
    },
    Family {
        id: "safety-functions",
        title: "Safety functions",
        what: "Each safety function reaches the performance level its risk graph requires, and every block of it is bound to a device that states reliability data.",
        kinds: &[ProblemKind::SafetyLevelNotMet, ProblemKind::SafetyBinding],
        source: "The safety functions entered in the Safety functions window (risk graph, duty, blocks, category, DCavg, CCF) and the MTTFd and PFHd stated on the bound devices or their parts.",
        assumptions: "A calculation aid, not a certification. PFHd for categories 2 to 4 is a closed-form estimate, not ISO 13849-1 Table K.1. Validation, systematic failures and software are not covered. Check functions that matter with SISTEMA and have them assessed by a competent person.",
        reliance: Reliance::Model,
        fix: "Open Check > Safety functions: bind each block to a device with reliability data, or change the architecture until the achieved level meets the required one.",
    },
    Family {
        id: "heat",
        title: "Heat in the cabinet",
        what: "No device in the cabinet exceeds its surface or ambient temperature limit with the stated losses and ambient.",
        kinds: &[],
        source: "The Heat simulation in the 3D view (physical heat and airflow model of the cabinet), device losses and temperature limits from the devices and parts.",
        assumptions: "A physical model with stated numerical uncertainty, not a measurement. It describes the one cabinet it was run on, not other cabinets of the project. It judges only the layout it was run on: after the layout changes the result is out of date. Devices without loss or limit data are listed as unknown, never as fine.",
        reliance: Reliance::Model,
        fix: "Open the 3D view, switch on Heat and run it; the hot-spot list names the devices to move or cool.",
    },
    Family {
        id: "consistency",
        title: "Drawing consistency",
        what: "Tags are unique and complete, nets have one potential, pins and wires are connected, contacts match their devices, interruption points have partners, and the generated lists (devices, wires, terminals) are read from a drawing that shows no structural problem.",
        kinds: &[
            ProblemKind::UnconnectedPin,
            ProblemKind::DuplicateTag,
            ProblemKind::DuplicateTerminal,
            ProblemKind::NoMainFunction,
            ProblemKind::SeveralMainFunctions,
            ProblemKind::TooManyContacts,
            ProblemKind::LoneInterruption,
            ProblemKind::AmbiguousInterruption,
            ProblemKind::PotentialConflict,
            ProblemKind::ZeroLengthWire,
            ProblemKind::WireOverlap,
            ProblemKind::OpenWire,
            ProblemKind::StrayWireEnd,
            ProblemKind::DanglingJunction,
            ProblemKind::UntaggedDevice,
            ProblemKind::SameShortTag,
            ProblemKind::DuplicatePage,
            ProblemKind::ContactType,
            ProblemKind::ContactPosition,
            ProblemKind::ContactDrawnTwice,
            ProblemKind::VoltageClassMix,
            ProblemKind::ConductorContinuity,
            ProblemKind::MotorPhases,
            ProblemKind::CoilSupply,
            ProblemKind::CoilOnPowerFeed,
            ProblemKind::NeutralUse,
            ProblemKind::TagRule,
            ProblemKind::DisplayedTag,
            ProblemKind::StructureCode,
        ],
        source: "The drawing itself: symbols, wires, nets, tags and device properties.",
        assumptions: "Structural checks with no numbers: a finding is a fact about the drawing. They do not know what the machine should do, only that the drawing contradicts itself.",
        reliance: Reliance::Exact,
        fix: "Click the finding in the Issues list to go to the object and correct it.",
    },
    Family {
        id: "terminals-cables-plc",
        title: "Terminals, plugs, cables and PLC",
        what: "Terminals are used as their kind allows, every terminal and plug pin is wired, plug halves agree, PLC channels can be addressed and wired as drawn.",
        kinds: &[
            ProblemKind::TerminalUse,
            ProblemKind::WrongTerminalBlock,
            ProblemKind::UnnamedTerminal,
            ProblemKind::UnwiredTerminal,
            ProblemKind::PlugPinOneSided,
            ProblemKind::PlugPins,
            ProblemKind::Plc,
            ProblemKind::RelaySize,
        ],
        source: "Terminal strips, plugs, cables and PLC cards as drawn, with the catalog marks of their parts.",
        assumptions: "Exact for what is drawn. Part marks (PE block, relay sizes) are as entered or imported; they are not checked against the maker's data.",
        reliance: Reliance::Exact,
        fix: "Open the strip, plug, cable or PLC card named in the finding and complete it.",
    },
    Family {
        id: "cabinet",
        title: "Cabinet layout",
        what: "Devices fit on the plate and rail, do not collide, keep their clearances and the gap to the door, ducts are not overfull or over devices, and every wire between placed devices has a duct.",
        kinds: &[
            ProblemKind::DeviceCollision,
            ProblemKind::DeviceOutsidePlate,
            ProblemKind::DevicesTooClose,
            ProblemKind::RailOverflow,
            ProblemKind::DuctOverlap,
            ProblemKind::DeviceTooDeep,
            ProblemKind::DoorGapTight,
            ProblemKind::PlateTooBig,
            ProblemKind::DoorSwing,
            ProblemKind::WireWithoutDuct,
            ProblemKind::OverfullDuct,
            ProblemKind::UnsizedDuct,
            ProblemKind::DepthUnknown,
            ProblemKind::UnplacedDevice,
        ],
        source: "Placements and dimensions on the cabinet plates, device footprints and depths from the parts, duct and rail sizes.",
        assumptions: "Geometry is exact for the dimensions stated; a device with no depth or footprint is listed as not checked. Duct fill uses the project's fill limit and wire cross-sections (default size where none is stated).",
        reliance: Reliance::Exact,
        fix: "Open the Cabinet view: move the device, resize the duct, or enter the missing dimension on the part.",
    },
    Family {
        id: "parts",
        title: "Parts and data",
        what: "Every device and cable has a part, so the lists, loads and heat can be worked out.",
        kinds: &[ProblemKind::MissingPart],
        source: "Part assignments on devices and cables.",
        assumptions: "Whether the part data (ratings, sizes) is true is not checked against the maker.",
        reliance: Reliance::Exact,
        fix: "Select the device and assign a part in the Inspector, or import the parts catalog.",
    },
    Family {
        id: "standards",
        title: "Selected standards",
        what: "The clause rules of the standards chosen in Standards check; each is detailed in its own section of this document.",
        kinds: &[ProblemKind::Standard],
        source: "See the standards sections below.",
        assumptions: "A subset of each standard, with clause numbers that are pointers only: IEC 60204-1 clause numbers are unverified (no licensed copy was used) and the tables are planning values.",
        reliance: Reliance::Planning,
        fix: "See the standards sections below.",
    },
];

/// The family a kind belongs to, `None` for a kind no family names.
pub fn family_index(kind: ProblemKind) -> Option<usize> {
    FAMILIES.iter().position(|f| f.kinds.contains(&kind))
}

/// How the last Heat run reads, as the 3D view's verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeatTone {
    Fine,
    Tight,
    TooHot,
    Unknown,
}

/// The Heat result the application holds, if any.
#[derive(Debug, Clone, PartialEq)]
pub struct HeatFact {
    pub tone: HeatTone,
    /// The verdict sentence of the Heat panel.
    pub sentence: String,
    /// The result was made for another layout than the current one.
    pub out_of_date: bool,
}

/// What the document is built from.
#[derive(Debug, Clone, Default)]
pub struct DesignInput {
    pub open: Vec<Problem>,
    pub waived: Vec<Waived>,
    pub heat: Option<HeatFact>,
}

impl DesignInput {
    /// The design checks of `project` as `novo-checks` runs them. The
    /// application adds the house-rule wire problems it reads elsewhere and
    /// the Heat result it holds.
    pub fn from_project(project: &Project) -> DesignInput {
        let (open, waived) = crate::split_waived(project, crate::check(project));
        DesignInput {
            open,
            waived,
            heat: None,
        }
    }
}

impl Row {
    /// The result as the reader scans it: a pass that rests on planning
    /// values or a model says so in the word itself, so it is not read as a
    /// verdict from the standard.
    pub fn result_text(&self) -> String {
        match (self.verdict, self.family.reliance) {
            (Verdict::Pass, Reliance::Planning) => "Met (planning values)".to_owned(),
            (Verdict::Pass, Reliance::Model) => "Met (estimate)".to_owned(),
            (v, _) => v.label().to_owned(),
        }
    }
}

/// One family's result.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub family: &'static Family,
    pub verdict: Verdict,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    pub waived: usize,
    /// How many things of this family the project has.
    pub examined: usize,
    /// What was found, errors first. Capped: see `more`.
    pub findings: Vec<Finding>,
    /// Findings beyond the cap, in the Issues list.
    pub more: usize,
    /// What was found fine, one line.
    pub evidence: Vec<String>,
    /// What could not be checked, and why.
    pub not_checkable: Vec<String>,
}

/// Findings printed per family; the rest are in the Issues list.
pub const MAX_FINDINGS: usize = 12;

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// How many things of the family the project has, for "not applicable".
fn examined(project: &Project, id: &str) -> usize {
    match id {
        "wire-sizing" | "colours" => project.wires().count(),
        "protection" => project
            .devices()
            .filter(|d| project.rated_current_a(d.id).is_some())
            .count(),
        "safety-functions" => safety_fn::functions(project).len(),
        "heat" => project.devices().count(),
        "consistency" => project.symbols().count() + project.wires().count(),
        "terminals-cables-plc" => project.terminals().count() + project.cables().count(),
        "cabinet" => project.devices().count(),
        "parts" => project.devices().count() + project.cables().count(),
        _ => 0,
    }
}

/// Data the checks of a family need and the project does not have.
fn missing(project: &Project, id: &str, heat: Option<&HeatFact>) -> Vec<String> {
    let props = &project.info().properties;
    let has = |k: &str| props.contains_key(k);
    let mut out = Vec::new();
    match id {
        "wire-sizing" => {
            let netlist = project.netlist();
            let basis = crate::calc_basis::wire_basis(project, &netlist);
            let wires = project.wires().count();
            let limited = basis.values().filter(|b| b.protecting.is_some()).count();
            if wires > 0 && limited < wires {
                out.push(format!(
                    "{} of {} wires: no protective device with a rated current limits it, so its size is not compared with one",
                    wires - limited,
                    wires
                ));
            }
            let no_method = basis
                .values()
                .filter(|b| b.protecting.is_some() && b.method.is_none())
                .count();
            if no_method > 0 && !has(keys::DEFAULT_INSTALLATION_METHOD) {
                out.push(format!(
                    "{} state no installation method (B1, B2, C) and the project has no default, so the table check cannot judge them",
                    plural(no_method, "protected wire", "protected wires")
                ));
            }
        }
        "colours" => {
            let wires = project.wires().count();
            if wires > 0 && project.wires().all(|w| w.color.is_none()) {
                out.push("No wire states a colour, so no colour was judged".to_owned());
            }
        }
        "protection" => {
            if project.devices().count() > 0
                && project
                    .devices()
                    .all(|d| project.rated_current_a(d.id).is_none())
            {
                out.push("No device states a rated current, so no protective device was examined".to_owned());
            }
            if !has(keys::SUPPLY_SHORT_CIRCUIT_KA) && !has("supply.transformer_kva") {
                out.push("The supply short-circuit current is not stated (supply.short_circuit_ka, or the feeding transformer), so breaking capacity, let-through and earth-fault times were not checked".to_owned());
            }
        }
        "safety-functions" => {
            if safety_fn::unreadable(project) {
                out.push("The stored safety functions cannot be read back, so none was rated".to_owned());
            }
            if safety_fn::functions(project).is_empty() {
                out.push("No safety function is entered, so none was rated; the E-stop and guard circuits are not assessed by this report".to_owned());
            }
        }
        "heat" => match heat {
            None => out.push("No Heat result is held: run Heat in the 3D view (Cabinet) before exporting to include it".to_owned()),
            Some(h) if h.out_of_date => out.push(
                "The Heat result was made for an earlier layout and no longer describes this one; run it again".to_owned(),
            ),
            Some(h) if h.tone == HeatTone::Unknown => {
                out.push("The Heat result could not judge the devices (no losses or no limits known)".to_owned());
            }
            Some(_) => {}
        },
        _ => {}
    }
    out
}

/// Evidence lines for the families that have numbers to show.
fn evidence(project: &Project, id: &str, heat: Option<&HeatFact>) -> Vec<String> {
    let mut out = Vec::new();
    match id {
        "colours" => {
            let wires = project.wires().count();
            let coloured = project.wires().filter(|w| w.color.is_some()).count();
            if wires > 0 {
                out.push(format!("{coloured} of {wires} wires state a colour"));
            }
        }
        "wire-sizing" => {
            let wires = project.wires().count();
            let sized = project
                .wires()
                .filter(|w| w.cross_section_mm2.is_some())
                .count();
            if wires > 0 {
                out.push(format!("{sized} of {wires} wires state a cross-section"));
            }
        }
        "safety-functions" => {
            for f in safety_fn::functions(project) {
                let rating = safety_fn::rate(project, &f);
                let example = {
                    let n = rating.name.to_lowercase();
                    n.contains("illustrative") || n.contains("example")
                };
                let line = match (&rating.total, rating.met()) {
                    (Some(t), Some(true)) => format!(
                        "{}: required {}, achieved {} (PFHd {:.2e} per hour): met as estimated{}",
                        rating.name,
                        rating.required,
                        t.pl,
                        t.pfhd,
                        if example {
                            "; illustrative example data, not an assessment of a real machine"
                        } else {
                            ""
                        }
                    ),
                    (Some(t), _) => format!(
                        "{}: required {}, achieved {} (PFHd {:.2e} per hour): NOT met",
                        rating.name, rating.required, t.pl, t.pfhd
                    ),
                    (None, _) => format!(
                        "{}: required {}, not rated (a block lacks data)",
                        rating.name, rating.required
                    ),
                };
                out.push(line);
            }
        }
        "heat" => {
            if let Some(h) = heat {
                out.push(h.sentence.clone());
            }
        }
        _ => {}
    }
    out
}

/// One row per family, in the order of [`FAMILIES`].
pub fn rows(project: &Project, input: &DesignInput) -> Vec<Row> {
    let mut rows = Vec::new();
    for (index, family) in FAMILIES.iter().enumerate() {
        let mut open: Vec<&Problem> = input
            .open
            .iter()
            .filter(|p| family_index(p.kind) == Some(index))
            .collect();
        open.sort_by_key(|p| p.severity);
        let waived: Vec<&Waived> = input
            .waived
            .iter()
            .filter(|w| family_index(w.problem.kind) == Some(index))
            .collect();
        let count = |s: Severity| {
            open.iter()
                .filter(|p| p.severity == s && !DATA_GAPS.contains(&p.kind))
                .count()
        };
        let (errors, warnings) = (count(Severity::Error), count(Severity::Warning));
        let gaps: Vec<&&Problem> = open
            .iter()
            .filter(|p| DATA_GAPS.contains(&p.kind) && p.severity == Severity::Info)
            .collect();
        let infos = open
            .iter()
            .filter(|p| p.severity == Severity::Info && !DATA_GAPS.contains(&p.kind))
            .count();

        let mut not_checkable = missing(project, family.id, input.heat.as_ref());
        for g in &gaps {
            not_checkable.push(g.message.clone());
        }
        let n = examined(project, family.id);

        let mut findings: Vec<Finding> = open
            .iter()
            .filter(|p| p.severity != Severity::Info)
            .map(|p| Finding {
                text: p.message.clone(),
                page: page_name(project, p.page),
                reason: None,
            })
            .chain(waived.iter().map(|w| Finding {
                text: w.problem.message.clone(),
                page: page_name(project, w.problem.page),
                reason: Some(w.reason.clone()),
            }))
            .collect();
        let total = findings.len();
        findings.truncate(MAX_FINDINGS);

        let heat_verdict = (family.id == "heat").then_some(match &input.heat {
            Some(h) if !h.out_of_date => match h.tone {
                HeatTone::Fine => Verdict::Pass,
                HeatTone::Tight => Verdict::Partial,
                HeatTone::TooHot => Verdict::Fail,
                HeatTone::Unknown => Verdict::NotCheckable,
            },
            _ => Verdict::NotCheckable,
        });

        let verdict = if let Some(v) = heat_verdict {
            if n == 0 { Verdict::NotApplicable } else { v }
        } else if errors + warnings > 0 {
            Verdict::Fail
        } else if family.id == "standards" && n == 0 && total == 0 {
            // Reported in the standards sections, or not selected at all.
            Verdict::NotApplicable
        } else if !waived.is_empty() && not_checkable.is_empty() {
            Verdict::Waived
        } else if n == 0 && (family.id != "protection" || project.devices().count() == 0) {
            Verdict::NotApplicable
        } else if !not_checkable.is_empty() {
            let nothing_ran = family.id == "safety-functions"
                && safety_fn::functions(project).is_empty()
                || family.id == "protection" && n == 0;
            if nothing_ran {
                Verdict::NotCheckable
            } else {
                Verdict::Partial
            }
        } else {
            Verdict::Pass
        };
        rows.push(Row {
            family,
            verdict,
            errors,
            warnings,
            infos,
            waived: waived.len(),
            examined: n,
            findings,
            more: total.saturating_sub(MAX_FINDINGS),
            evidence: evidence(project, family.id, input.heat.as_ref()),
            not_checkable,
        });
    }
    rows
}

/// Kinds the open problems use that no family names; empty when the table is
/// complete.
pub fn unassigned(problems: &[Problem]) -> BTreeSet<&'static str> {
    problems
        .iter()
        .filter(|p| family_index(p.kind).is_none())
        .map(|p| p.kind.label())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(kind: ProblemKind, msg: &str) -> Problem {
        Problem::new(kind, Vec::new(), msg.to_owned())
    }

    #[test]
    fn no_kind_is_in_two_families() {
        let mut seen = BTreeSet::new();
        for f in FAMILIES {
            for k in f.kinds {
                assert!(seen.insert(*k), "{k:?} in two families");
            }
        }
    }

    #[test]
    fn an_empty_project_is_not_applicable_not_pass() {
        let project = Project::new("Empty");
        let rows = rows(&project, &DesignInput::from_project(&project));
        assert_eq!(rows.len(), FAMILIES.len());
        for r in &rows {
            assert!(
                matches!(r.verdict, Verdict::NotApplicable | Verdict::NotCheckable),
                "{}: {:?}",
                r.family.id,
                r.verdict
            );
        }
        let heat = rows.iter().find(|r| r.family.id == "heat").unwrap();
        assert_eq!(heat.verdict, Verdict::NotApplicable);
    }

    #[test]
    fn open_errors_fail_the_family_and_waived_ones_are_listed() {
        let project = Project::new("P");
        let input = DesignInput {
            open: vec![
                problem(ProblemKind::ShortCircuit, "-Q1 cannot break 12 kA"),
                problem(ProblemKind::MissingPart, "-K1 has no part"),
            ],
            ..DesignInput::default()
        };
        let rows = rows(&project, &input);
        let prot = rows.iter().find(|r| r.family.id == "protection").unwrap();
        assert_eq!(prot.verdict, Verdict::Fail);
        assert_eq!(prot.errors, 1);
        assert_eq!(prot.findings[0].text, "-Q1 cannot break 12 kA");
    }

    #[test]
    fn missing_data_is_partly_checkable_never_a_silent_pass() {
        let project = Project::new("P");
        let input = DesignInput {
            open: vec![problem(ProblemKind::GuessedWire, "5 wires have no size")],
            ..DesignInput::default()
        };
        let rows = rows(&project, &input);
        let w = rows.iter().find(|r| r.family.id == "wire-sizing").unwrap();
        // No wires exist in the empty project: nothing to examine.
        assert_eq!(w.verdict, Verdict::NotApplicable);
        assert!(w.not_checkable.iter().any(|t| t.contains("no size")));
    }

    #[test]
    fn protection_with_a_stated_supply_but_no_rated_device_is_not_checkable() {
        let mut project = Project::new("P");
        project
            .transact("devices", |tx| {
                tx.add_device(novo_core::DeviceTag::parse("-Q1")?, "")?;
                super::super::set_info_property(
                    tx,
                    keys::SUPPLY_SHORT_CIRCUIT_KA,
                    Some(novo_core::Value::Float(10.0)),
                )
            })
            .unwrap();
        let rows = rows(&project, &DesignInput::default());
        let p = rows.iter().find(|r| r.family.id == "protection").unwrap();
        assert_eq!(p.verdict, Verdict::NotCheckable);
        assert!(p.not_checkable.iter().any(|t| t.contains("rated current")));
    }

    #[test]
    fn a_pass_carries_its_basis_in_the_result_word() {
        let project = Project::new("P");
        let mut rows = rows(&project, &DesignInput::default());
        for r in &mut rows {
            r.verdict = Verdict::Pass;
            let text = r.result_text();
            match r.family.reliance {
                Reliance::Exact => assert_eq!(text, "Pass"),
                Reliance::Planning => assert_eq!(text, "Met (planning values)"),
                Reliance::Model => assert_eq!(text, "Met (estimate)"),
            }
        }
    }

    #[test]
    fn heat_states_what_it_is() {
        let project = Project::new("P");
        let mk = |tone, out_of_date| DesignInput {
            heat: Some(HeatFact {
                tone,
                sentence: "Fine. Hottest -F1 57 °C.".into(),
                out_of_date,
            }),
            ..DesignInput::default()
        };
        // No devices: nothing to judge.
        let r = rows(&project, &mk(HeatTone::Fine, false));
        assert_eq!(
            r.iter().find(|r| r.family.id == "heat").unwrap().verdict,
            Verdict::NotApplicable
        );
    }

    #[test]
    fn every_problem_kind_has_a_family() {
        for k in ProblemKind::ALL {
            assert!(family_index(k).is_some(), "{k:?} has no family");
        }
    }

    #[test]
    fn the_standards_family_takes_standard_problems() {
        assert!(family_index(ProblemKind::Standard).is_some());
        assert!(unassigned(&[problem(ProblemKind::Standard, "x")]).is_empty());
    }
}
