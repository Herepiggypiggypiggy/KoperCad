//! Wires that have no size or colour yet, and filling them in one step.
//!
//! The bench cut list groups wires by cross-section and colour, and a wire
//! without a size gets no strip length and no ferrule. Drawing sets neither,
//! so a fresh project would print "size not set" on every line. The person
//! picks a size (and, if they like, a line colour) once and it goes onto the
//! wires that have none; a wire that already has a value is never touched.
//! Protective earth and neutral take the colours the standard gives them.

use novo_core::{PotentialKind, Project, Result, Tx, WireId};

use crate::wires::items;

/// Cross-sections offered when filling in a default, in mm².
pub const SIZES_MM2: [f64; 8] = [0.5, 0.75, 1.0, 1.5, 2.5, 4.0, 6.0, 10.0];

/// Colour code of a protective conductor (IEC 60757).
const PROTECTIVE: &str = "GNYE";
/// Colour code of neutral and 0 V.
const NEUTRAL: &str = "BU";

/// The wires of the cut list that still lack a value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UnsetWires {
    /// Wires of the cut list.
    pub total: usize,
    pub no_size: Vec<WireId>,
    pub no_colour: Vec<WireId>,
}

impl UnsetWires {
    pub fn is_complete(&self) -> bool {
        self.no_size.is_empty() && self.no_colour.is_empty()
    }
}

/// What to put on wires that have nothing yet.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WireDefaults {
    /// Cross-section in mm² for wires without one; `None` leaves sizes alone.
    pub size_mm2: Option<f64>,
    /// Colour code (`BK`) for line and control wires without a colour;
    /// `None` leaves them blank. Protective earth and neutral wires always
    /// get green-yellow and blue when colours are filled in.
    pub colour: Option<String>,
    /// Fill in colours at all (protective earth and neutral included).
    pub fill_colours: bool,
}

/// Which wires of the cut list have no size and no colour. A wire of a
/// cable core counts as sized when its core is.
pub fn unset_wires(project: &Project) -> UnsetWires {
    let mut out = UnsetWires::default();
    for item in items(project, true) {
        out.total += item.wires.len();
        for wire in &item.wires {
            if crate::wirecut::sections(project, &[*wire]).is_empty() {
                out.no_size.push(wire.id);
            }
            if wire.color.as_deref().is_none_or(|c| c.trim().is_empty()) {
                out.no_colour.push(wire.id);
            }
        }
    }
    out
}

/// The colour a potential's kind gives its wires, when the standard does.
fn standard_colour(kind: PotentialKind) -> Option<&'static str> {
    match kind {
        PotentialKind::ProtectiveEarth => Some(PROTECTIVE),
        PotentialKind::Neutral | PotentialKind::DcNegative => Some(NEUTRAL),
        _ => None,
    }
}

/// Puts the defaults on the wires that have none. Returns how many wires got
/// a size and how many got a colour. Call it in one transaction: one undo.
pub fn apply_wire_defaults(tx: &mut Tx<'_>, defaults: &WireDefaults) -> Result<(usize, usize)> {
    let unset = unset_wires(tx.project());
    let netlist = tx.project().netlist();
    let mut sized = 0;
    let mut coloured = 0;
    if let Some(size) = defaults.size_mm2.filter(|s| s.is_finite() && *s > 0.0) {
        for id in &unset.no_size {
            tx.update::<novo_core::Wire>(*id, |w| w.cross_section_mm2 = Some(size))?;
            sized += 1;
        }
    }
    if defaults.fill_colours {
        for id in &unset.no_colour {
            let kind = tx.project().wire(*id).and_then(|w| {
                let net = netlist.net(netlist.net_of_wire(w)?);
                tx.project().potential(net.potential()?).map(|p| p.kind)
            });
            let colour = kind
                .and_then(standard_colour)
                .map(str::to_owned)
                .or_else(|| defaults.colour.clone().filter(|c| !c.trim().is_empty()));
            if let Some(colour) = colour {
                tx.update::<novo_core::Wire>(*id, |w| w.color = Some(colour))?;
                coloured += 1;
            }
        }
    }
    Ok((sized, coloured))
}
