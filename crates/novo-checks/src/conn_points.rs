//! Checks against what a part states about its connection points: the
//! conductor range, how many conductors it takes and whether it wants a
//! ferrule (novo-parts `ConnectionData`, project properties `pin.<name>.*`).
//!
//! The data comes from the maker's files where they were imported, or from
//! what a person typed. A part that states nothing about a point is not
//! judged there: nothing is assumed or invented, and a finding names where
//! the figures came from.

use std::collections::BTreeMap;

use novo_core::{AnyId, Endpoint, PinRef, Project, SymbolId, SymbolRole, Value, Wire};
use novo_parts::{ConnectionData, FerruleRule, connection_data, connection_source};

use crate::{Context, Problem, ProblemKind, Severity};

/// Wire property that turns the ferrule off (`none`) or names the one to use
/// (the key novo-reports reads for the cut list).
const FERRULE_KEY: &str = "ferrule";
/// Wire property that states the end processing in words (`Ring lug M5`).
const END_PROCESSING_KEY: &str = "end_processing";
const EPS: f64 = 1e-6;

/// What the part behind one connection point states, and where it came from.
#[derive(Debug, Clone, PartialEq)]
pub struct PointData {
    pub data: ConnectionData,
    /// `maker`, `typed`, `estimated` or `unknown`.
    pub source: String,
}

impl PointData {
    /// The source in words for a message: `maker data`, `typed-in data`.
    pub fn source_text(&self) -> &'static str {
        match self.source.as_str() {
            "maker" => "maker data",
            "typed" => "typed-in data",
            "estimated" => "estimated data",
            _ => "part data, source not stated",
        }
    }
}

/// The connection data of the part at `pin`, or `None` when the point has no
/// part or the part states nothing about it.
pub fn point_data(project: &Project, pin: PinRef) -> Option<PointData> {
    let symbol = project.symbol(pin.symbol)?;
    let name = &symbol.pins.get(usize::from(pin.pin))?.name;
    let part_ids = match &symbol.role {
        SymbolRole::Device { device, .. } => project.device(*device)?.parts.clone(),
        SymbolRole::Terminal { .. } => {
            let terminal = symbol.terminal_of_pin(pin.pin)?;
            novo_terminals::terminal_part(project, terminal)
                .into_iter()
                .collect()
        }
        _ => return None,
    };
    part_ids.into_iter().find_map(|id| {
        let props = &project.part(id)?.properties;
        let data = connection_data(props, name);
        (!data.is_empty()).then(|| PointData {
            data,
            source: connection_source(props).unwrap_or("unknown").to_owned(),
        })
    })
}

fn wire_section(project: &Project, wire: &Wire) -> Option<f64> {
    wire.cross_section_mm2.or_else(|| {
        let (cable, core) = project.core_of_wire(wire.id)?;
        project.cable(cable)?.cores.get(core)?.cross_section_mm2
    })
}

fn text<'a>(wire: &'a Wire, key: &str) -> Option<&'a str> {
    match wire.properties.get(key)? {
        Value::Text(t) if !t.trim().is_empty() => Some(t.trim()),
        _ => None,
    }
}

fn number(value: f64) -> String {
    format!("{}", (value * 100.0).round() / 100.0)
}

pub(crate) fn run(cx: &Context<'_>, out: &mut Vec<Problem>) {
    let project = cx.project;
    let mut count: BTreeMap<(SymbolId, u16), Vec<&Wire>> = BTreeMap::new();
    for wire in project.wires() {
        let section = wire_section(project, wire);
        for end in [wire.from, wire.to] {
            let Endpoint::Pin { symbol, pin } = end else {
                continue;
            };
            let at_pin = PinRef::new(symbol, pin);
            count.entry((symbol, pin)).or_default().push(wire);
            let Some(point) = point_data(project, at_pin) else {
                continue;
            };
            let Some(place) = project.pin_designation(at_pin) else {
                continue;
            };
            let label = wire
                .number
                .as_deref()
                .map_or_else(|| "A wire".to_owned(), |n| format!("Wire {n}"));
            let marks = || vec![AnyId::from(wire.id), AnyId::from(symbol)];
            if let (Some(mm2), Some((lo, hi))) = (section, point.data.wire_mm2) {
                let source = point.source_text();
                if mm2 > hi + EPS {
                    let mut p = Problem::new(
                        ProblemKind::WireRange,
                        marks(),
                        format!(
                            "{label} ({} mm²) is larger than {place} accepts ({}–{} mm², {source})",
                            number(mm2),
                            number(lo),
                            number(hi)
                        ),
                    );
                    p.severity = Severity::Error;
                    out.push(p);
                } else if mm2 + EPS < lo {
                    out.push(Problem::new(
                        ProblemKind::WireRange,
                        marks(),
                        format!(
                            "{label} ({} mm²) is smaller than {place} accepts ({}–{} mm², {source})",
                            number(mm2),
                            number(lo),
                            number(hi)
                        ),
                    ));
                }
            }
            let explicit_none =
                text(wire, FERRULE_KEY).is_some_and(|t| t.eq_ignore_ascii_case("none"));
            let named = text(wire, FERRULE_KEY).filter(|t| !t.eq_ignore_ascii_case("none"));
            let source = point.source_text();
            match point.data.ferrule {
                Some(FerruleRule::Required)
                    if explicit_none || text(wire, END_PROCESSING_KEY).is_some() =>
                {
                    out.push(Problem::new(
                        ProblemKind::FerruleUse,
                        marks(),
                        format!("{label} has no ferrule at {place}, which asks for one ({source})"),
                    ));
                }
                Some(FerruleRule::Forbidden) if named.is_some() => {
                    out.push(Problem::new(
                        ProblemKind::FerruleUse,
                        marks(),
                        format!(
                            "{label} has a ferrule at {place}, which does not allow one ({source})"
                        ),
                    ));
                }
                _ => {}
            }
        }
    }
    for ((symbol, pin), wires) in count {
        let Some(point) = point_data(project, PinRef::new(symbol, pin)) else {
            continue;
        };
        let Some(max) = point.data.max_wires else {
            continue;
        };
        // A wire drawn from a pin to the same pin would count twice.
        let ids: std::collections::BTreeSet<_> = wires.iter().map(|w| w.id).collect();
        if ids.len() <= usize::from(max) {
            continue;
        }
        let Some(place) = project.pin_designation(PinRef::new(symbol, pin)) else {
            continue;
        };
        let mut at: Vec<AnyId> = vec![symbol.into()];
        at.extend(ids.iter().map(|w| AnyId::from(*w)));
        out.push(Problem::new(
            ProblemKind::PointOccupied,
            at,
            format!(
                "{place} takes {max} {} but {} are connected ({})",
                if max == 1 { "conductor" } else { "conductors" },
                ids.len(),
                point.source_text()
            ),
        ));
    }
}
