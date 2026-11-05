//! Wire end terminations and the sleeve text.
//!
//! A wire end is finished in one of a few ways: a ferrule, a ring lug, a fork
//! lug, a spade (flat) lug, a pin lug, or left bare with only the insulation
//! stripped. The designer states the way per end on the wire
//! ([`TERMINATION_A_KEY`], [`TERMINATION_B_KEY`]); where nothing is stated the
//! end keeps what the bench proposes (a ferrule when the size is known, else a
//! bare end), so a project that never uses this prints exactly as before.
//!
//! The crimp list ([`crimp_list`]) counts what the wireman has to crimp, per
//! kind and size, from the same jobs as the cut list. The sleeve text
//! ([`SLEEVE_TEXT_KEY`]) is the text printed on the wire's sleeve or flag: it
//! may differ from the permanent wire number, which the schedules keep.

use std::collections::BTreeMap;

use novo_core::{Project, Value, WireId};

use crate::bench::FerruleStandard;
use crate::wirecut::WireCutJob;

/// Wire property that states the termination of end A (`ring M5`).
pub const TERMINATION_A_KEY: &str = "termination_a";
/// Wire property that states the termination of end B.
pub const TERMINATION_B_KEY: &str = "termination_b";
/// Wire property with the text printed on the wire's sleeve or flag. Empty or
/// absent means the wire number is printed.
pub const SLEEVE_TEXT_KEY: &str = "sleeve_text";

/// How a wire end is finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TerminationKind {
    /// Wire end ferrule (crimped sleeve).
    Ferrule,
    /// Ring lug, closed eye on a stud.
    Ring,
    /// Fork (open U) lug.
    Fork,
    /// Spade or flat lug (flat tab, blade).
    Spade,
    /// Pin lug.
    Pin,
    /// Stripped only, goes straight into the clamp.
    Bare,
    /// A processing stated in free text that is none of the above.
    Other,
}

impl TerminationKind {
    /// The six kinds the designer picks from.
    pub const PICKABLE: [TerminationKind; 6] = [
        TerminationKind::Ferrule,
        TerminationKind::Ring,
        TerminationKind::Fork,
        TerminationKind::Spade,
        TerminationKind::Pin,
        TerminationKind::Bare,
    ];

    /// The word the property stores.
    pub fn key(self) -> &'static str {
        match self {
            TerminationKind::Ferrule => "ferrule",
            TerminationKind::Ring => "ring",
            TerminationKind::Fork => "fork",
            TerminationKind::Spade => "spade",
            TerminationKind::Pin => "pin",
            TerminationKind::Bare => "bare",
            TerminationKind::Other => "other",
        }
    }

    /// The name printed on lists.
    pub fn label(self) -> &'static str {
        match self {
            TerminationKind::Ferrule => "Ferrule",
            TerminationKind::Ring => "Ring lug",
            TerminationKind::Fork => "Fork lug",
            TerminationKind::Spade => "Spade lug",
            TerminationKind::Pin => "Pin lug",
            TerminationKind::Bare => "Bare",
            TerminationKind::Other => "Other",
        }
    }

    /// Whether the end is crimped (everything except a bare end).
    pub fn crimped(self) -> bool {
        self != TerminationKind::Bare
    }
}

/// A termination: the kind and what size it is (`M5` for a ring, `6.3` for a
/// spade tab, `A1.5-10` for a ferrule), empty when not stated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Termination {
    pub kind: TerminationKind,
    pub size: String,
}

impl Termination {
    pub fn new(kind: TerminationKind, size: &str) -> Termination {
        Termination {
            kind,
            size: size.trim().to_owned(),
        }
    }

    /// `Ring lug M5`, `Ferrule`, `Bare`.
    pub fn text(&self) -> String {
        if self.size.is_empty() {
            self.kind.label().to_owned()
        } else if self.kind == TerminationKind::Other {
            self.size.clone()
        } else {
            format!("{} {}", self.kind.label(), self.size)
        }
    }

    /// The text as stored on the wire: `ring M5`. Round-trips through
    /// [`Termination::parse`].
    pub fn stored(&self) -> String {
        if self.size.is_empty() {
            self.kind.key().to_owned()
        } else {
            format!("{} {}", self.kind.key(), self.size)
        }
    }

    /// Reads a stated termination: the first word names the kind (English
    /// names and the usual short forms), the rest is the size. `none`,
    /// `stripped` and `bare` mean a bare end. Empty text is `None`; text that
    /// names no kind is [`TerminationKind::Other`] with the text as its size.
    pub fn parse(text: &str) -> Option<Termination> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let mut words = text.split_whitespace();
        let first = words.next()?.to_ascii_lowercase();
        let kind = match first.trim_end_matches(['s', ',']) {
            "ferrule" | "sleeve" | "ferrul" | "wee" => TerminationKind::Ferrule,
            "ring" | "eyelet" | "eye" => TerminationKind::Ring,
            "fork" | "forked" | "u" | "u-lug" | "ulug" => TerminationKind::Fork,
            "spade" | "flat" | "blade" | "tab" | "faston" => TerminationKind::Spade,
            "pin" => TerminationKind::Pin,
            "bare" | "none" | "strip" | "stripped" => TerminationKind::Bare,
            _ => {
                return Some(Termination {
                    kind: TerminationKind::Other,
                    size: text.to_owned(),
                });
            }
        };
        let mut rest: Vec<&str> = words.collect();
        // "ring lug M5", "spade tab 6.3": the noun after the kind is no size.
        while rest
            .first()
            .is_some_and(|w| matches!(w.to_ascii_lowercase().as_str(), "lug" | "terminal" | "tab"))
        {
            rest.remove(0);
        }
        Some(Termination {
            kind,
            size: rest.join(" "),
        })
    }
}

fn text_property(project: &Project, wires: &[WireId], key: &str) -> Option<String> {
    wires
        .iter()
        .find_map(|w| match project.wire(*w)?.properties.get(key) {
            Some(Value::Text(t)) if !t.trim().is_empty() => Some(t.trim().to_owned()),
            _ => None,
        })
}

/// The termination the designer stated for end A or B of a connection made
/// of `wires`, if any.
pub fn stated_termination(project: &Project, wires: &[WireId], end_b: bool) -> Option<Termination> {
    let key = if end_b {
        TERMINATION_B_KEY
    } else {
        TERMINATION_A_KEY
    };
    text_property(project, wires, key).and_then(|t| Termination::parse(&t))
}

/// The sleeve text stated for a connection made of `wires`, if any.
pub fn stated_sleeve(project: &Project, wires: &[WireId]) -> Option<String> {
    text_property(project, wires, SLEEVE_TEXT_KEY)
}

/// One line of the crimp list: how many ends of one kind and size to crimp.
#[derive(Debug, Clone, PartialEq)]
pub struct CrimpLine {
    pub kind: TerminationKind,
    /// `A1.5-10`, `M5`, `6.3`, empty when the size is not stated.
    pub size: String,
    /// Conductor cross-section the piece is crimped on.
    pub mm2: Option<f64>,
    /// Wire ends that take it.
    pub ends: usize,
    /// Pieces to crimp. Twin ferrules count once for two ends.
    pub pieces: usize,
    /// Pieces to prepare: `pieces` with the standard's spare percentage,
    /// rounded up.
    pub to_prepare: usize,
}

/// The crimp list: every crimped termination of the jobs, counted per kind,
/// size and conductor cross-section, ferrules first (twin ferrules where two
/// wires share a clamp, the same count as the ferrule table), then the lugs.
/// Bare ends are not crimped and are not listed. Spares follow the ferrule
/// standard's percentage.
pub fn crimp_list(jobs: &[WireCutJob], standard: &FerruleStandard) -> Vec<CrimpLine> {
    let spare = |n: usize| crate::bench::with_spares(n, standard.spare_percent.max(0.0));
    let mut lines: Vec<CrimpLine> = crate::ferrule_counts(jobs, standard)
        .into_iter()
        .map(|f| CrimpLine {
            kind: TerminationKind::Ferrule,
            size: f.ferrule,
            mm2: f.mm2,
            ends: f.ends,
            pieces: f.pieces,
            to_prepare: spare(f.pieces),
        })
        .collect();
    // Lugs and the rest: no twin crimping, one piece per end.
    let mut others: BTreeMap<(TerminationKind, String, u64), (usize, Option<f64>)> =
        BTreeMap::new();
    for j in jobs {
        for end in [&j.end_a, &j.end_b] {
            let t = &end.termination;
            if matches!(t.kind, TerminationKind::Ferrule | TerminationKind::Bare) {
                continue;
            }
            let key = (
                t.kind,
                t.size.clone(),
                j.cross_section_mm2.map_or(u64::MAX, f64::to_bits),
            );
            let e = others.entry(key).or_insert((0, j.cross_section_mm2));
            e.0 += 1;
        }
    }
    for ((kind, size, _), (ends, mm2)) in others {
        lines.push(CrimpLine {
            kind,
            size,
            mm2,
            ends,
            pieces: ends,
            to_prepare: spare(ends),
        });
    }
    lines.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then(
                a.mm2
                    .unwrap_or(f64::INFINITY)
                    .total_cmp(&b.mm2.unwrap_or(f64::INFINITY)),
            )
            .then_with(|| a.size.cmp(&b.size))
    });
    lines
}

/// Ends that are bare (stripped only), for the note under the crimp list.
pub fn bare_ends(jobs: &[WireCutJob]) -> usize {
    jobs.iter()
        .flat_map(|j| [&j.end_a, &j.end_b])
        .filter(|e| e.termination.kind == TerminationKind::Bare)
        .count()
}
