//! Cable connection screens: for each cable, every core with what it lands
//! on at both ends; and the cable overview, one row per cable.
//!
//! The left end of a cable is the cabinet side: of a wire's two ends, the
//! one in the location with a terminal strip (else more devices) goes
//! left. Rows are plain data for the Cables drawer (Ctrl+4) and the
//! generated cable pages; [`CableScreen::to_text`] gives a text form for
//! tests and quick looks.

use std::collections::BTreeMap;

use novo_core::{Cable, CableId, Endpoint, Project, WireId, natural_cmp};

use crate::edit::{ShieldConnection, is_spare, shield_connection};
use crate::ends;
use crate::kind::type_label;
use crate::strings;

/// What one end of a core lands on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoreEnd {
    /// The location as text: `+CAB`.
    pub location: String,
    /// The device pin or terminal: `=A1+CAB-X1:1`, or `PE` for a shield on
    /// earth.
    pub target: String,
}

/// One core of a cable screen.
#[derive(Debug, Clone, PartialEq)]
pub struct CoreRow {
    /// Index in `Cable::cores`.
    pub index: usize,
    /// `BN`, `1`, `GNYE`, `SH`.
    pub core: String,
    pub cross_section_mm2: Option<f64>,
    pub shield: bool,
    pub spare: bool,
    pub wire: Option<WireId>,
    pub wire_number: Option<String>,
    /// The potential the wire carries, when it has exactly one.
    pub potential: Option<String>,
    /// Cabinet end, then field end.
    pub ends: [Option<CoreEnd>; 2],
}

/// One cable with every core and both ends.
#[derive(Debug, Clone, PartialEq)]
pub struct CableScreen {
    pub cable: CableId,
    pub tag: String,
    /// `4G1.5`, or the part's type number.
    pub type_name: String,
    /// `Lapp 1119304`, when the cable has a part.
    pub part: Option<String>,
    pub length_m: Option<f64>,
    /// The locations of the two ends (`+CAB`, `+FLD`), empty when unknown.
    pub from: String,
    pub to: String,
    pub rows: Vec<CoreRow>,
}

/// One row of the cable overview.
#[derive(Debug, Clone, PartialEq)]
pub struct OverviewRow {
    pub cable: CableId,
    pub tag: String,
    pub type_name: String,
    /// Length in metres; `None` until cable lengths come from the layout.
    pub length_m: Option<f64>,
    pub from: String,
    pub to: String,
    /// Conductors (not shields) carrying a wire.
    pub used: usize,
    /// Conductors in all.
    pub cores: usize,
    /// Conductors marked spare.
    pub spare: usize,
}

fn core_end(project: &Project, end: Endpoint) -> Option<CoreEnd> {
    Some(CoreEnd {
        location: ends::location(project, end)
            .map(|l| ends::location_text(&l))
            .unwrap_or_default(),
        target: ends::designation(project, end)?,
    })
}

/// A wire's two ends, cabinet side first.
fn oriented(project: &Project, wire: WireId) -> Option<[Endpoint; 2]> {
    let w = project.wire(wire)?;
    let rank = |end: Endpoint| {
        ends::location(project, end).map(|l| (ends::rank(project, &l), ends::location_text(&l)))
    };
    let (ra, rb) = (rank(w.from), rank(w.to));
    let swap = match (&ra, &rb) {
        (Some((a, la)), Some((b, lb))) => b > a || (b == a && natural_cmp(lb, la).is_lt()),
        (None, Some(_)) => true,
        _ => false,
    };
    Some(if swap { [w.to, w.from] } else { [w.from, w.to] })
}

fn most_common(values: impl Iterator<Item = String>) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for v in values.filter(|v| !v.is_empty()) {
        *counts.entry(v).or_default() += 1;
    }
    counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))
        .map(|(v, _)| v)
        .unwrap_or_default()
}

fn shield_ends(project: &Project, cable: &Cable, core: usize) -> [Option<CoreEnd>; 2] {
    match shield_connection(cable, core) {
        Some(ShieldConnection::Pe) => [
            Some(CoreEnd {
                location: String::new(),
                target: strings::SHIELD_ON_PE.into(),
            }),
            None,
        ],
        Some(ShieldConnection::Terminal(t)) => {
            let location = project
                .terminal(t)
                .and_then(|t| project.device(t.strip))
                .map(|d| ends::location_text(&d.tag.location))
                .unwrap_or_default();
            [
                project
                    .terminal_designation(t)
                    .map(|target| CoreEnd { location, target }),
                None,
            ]
        }
        // A wire on the shield core shows like any other core.
        Some(ShieldConnection::Wire(_)) | None => [None, None],
    }
}

/// The connection screen of one cable.
pub fn cable_screen(project: &Project, cable: CableId) -> Option<CableScreen> {
    let c = project.cable(cable)?;
    let netlist = project.netlist();
    let mut rows = Vec::with_capacity(c.cores.len());
    for (index, core) in c.cores.iter().enumerate() {
        let wire = core.wire.and_then(|w| project.wire(w));
        let ends = match wire.and_then(|w| oriented(project, w.id)) {
            Some([a, b]) => [core_end(project, a), core_end(project, b)],
            None if core.shield => shield_ends(project, c, index),
            None => [None, None],
        };
        let potential = wire
            .and_then(|w| {
                w.potential
                    .or_else(|| netlist.potential_of(w.from))
                    .or_else(|| netlist.potential_of(w.to))
            })
            .and_then(|p| project.potential(p))
            .map(|p| p.name.clone());
        rows.push(CoreRow {
            index,
            core: core.name.clone(),
            cross_section_mm2: core.cross_section_mm2,
            shield: core.shield,
            spare: is_spare(c, index),
            wire: core.wire,
            wire_number: wire.and_then(|w| w.number.clone()),
            potential,
            ends,
        });
    }
    let from = most_common(
        rows.iter()
            .filter(|r| !r.shield)
            .filter_map(|r| r.ends[0].as_ref().map(|e| e.location.clone())),
    );
    let to = most_common(
        rows.iter()
            .filter(|r| !r.shield)
            .filter_map(|r| r.ends[1].as_ref().map(|e| e.location.clone())),
    );
    Some(CableScreen {
        cable,
        tag: c.tag.to_string(),
        type_name: type_label(project, c),
        part: c
            .part
            .and_then(|p| project.part(p))
            .map(|p| format!("{} {}", p.manufacturer, p.order_number)),
        length_m: c.length_m,
        from,
        to,
        rows,
    })
}

/// Every cable's screen, in tag order.
pub fn cable_screens(project: &Project) -> Vec<CableScreen> {
    sorted_cables(project)
        .into_iter()
        .filter_map(|c| cable_screen(project, c))
        .collect()
}

fn sorted_cables(project: &Project) -> Vec<CableId> {
    let mut cables: Vec<&Cable> = project.cables().collect();
    cables
        .sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()).then(a.id.cmp(&b.id)));
    cables.into_iter().map(|c| c.id).collect()
}

/// The cable overview: one row per cable, in tag order.
pub fn overview(project: &Project) -> Vec<OverviewRow> {
    cable_screens(project)
        .into_iter()
        .map(|s| {
            let conductors = || s.rows.iter().filter(|r| !r.shield);
            OverviewRow {
                cable: s.cable,
                used: conductors().filter(|r| r.wire.is_some()).count(),
                cores: conductors().count(),
                spare: conductors().filter(|r| r.spare).count(),
                tag: s.tag,
                type_name: s.type_name,
                length_m: s.length_m,
                from: s.from,
                to: s.to,
            }
        })
        .collect()
}

impl CableScreen {
    /// A plain-text table: a header line, then one line per core.
    pub fn to_text(&self) -> String {
        let mut out = format!(
            "{} {} {} -> {}",
            self.tag,
            self.type_name,
            or_dash(&self.from),
            or_dash(&self.to)
        );
        if let Some(length) = self.length_m {
            out.push_str(&format!(" {length} m"));
        }
        out.push('\n');
        for row in &self.rows {
            let end = |e: &Option<CoreEnd>| {
                e.as_ref()
                    .map_or_else(|| "-".to_owned(), |e| e.target.clone())
            };
            let mut note = String::new();
            if row.spare {
                note.push_str(strings::SPARE);
            } else if row.wire.is_none() && !row.shield {
                note.push_str(strings::FREE);
            }
            if let Some(p) = &row.potential {
                note.push_str(p);
            }
            out.push_str(&format!(
                "{:<5} {:<22} {:<22} {}\n",
                row.core,
                end(&row.ends[0]),
                end(&row.ends[1]),
                note
            ));
        }
        out
    }
}

fn or_dash(text: &str) -> &str {
    if text.is_empty() { "-" } else { text }
}
