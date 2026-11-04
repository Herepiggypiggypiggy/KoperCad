//! Cooling from the chosen enclosure (`handoff/cabinet-start/CHAIN.md`,
//! row "Cooling selection"): the outside size that sets the effective
//! surface comes from the [`EnclosureSpec`], not from typed numbers, and the
//! answer says how far to trust it. Pure. Millimetres.

use crate::sizing::{Installation, Material, SizingRequest, Thermal, effective_surface, thermal};
use crate::spec::{EnclosureSpec, Missing, Source};

/// Outside size of the chosen enclosure and the weakest source among the
/// three numbers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Outside {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub weakest: Source,
}

/// Outside size from the record; names the first missing number instead of
/// guessing a size or a depth.
pub fn outside(spec: &EnclosureSpec) -> Result<Outside, Missing> {
    let w = spec.width.ok_or(Missing("outside width"))?;
    let h = spec.height.ok_or(Missing("outside height"))?;
    let d = spec.depth.ok_or(Missing("outside depth"))?;
    Ok(Outside {
        width: w.value,
        height: h.value,
        depth: d.value,
        weakest: [w, h, d]
            .iter()
            .map(|f| f.source)
            .max()
            .unwrap_or(Source::Default),
    })
}

/// Effective cooling surface (m2) of the chosen enclosure.
pub fn surface_m2(spec: &EnclosureSpec, install: Installation) -> Result<f64, Missing> {
    let o = outside(spec)?;
    Ok(effective_surface(o.width, o.height, o.depth, install))
}

/// Heat balance of the chosen enclosure carrying `loss` watts.
#[derive(Debug, Clone, PartialEq)]
pub struct SpecThermal {
    pub thermal: Thermal,
    /// Weakest source of the size the balance used.
    pub weakest: Source,
}

/// Heat balance with the size taken from the record.
pub fn thermal_for(
    spec: &EnclosureSpec,
    material: Material,
    loss: f64,
    req: &SizingRequest,
) -> Result<SpecThermal, Missing> {
    let o = outside(spec)?;
    let size = crate::sizing::EnclosureSize {
        name: spec.article.clone(),
        width: o.width,
        height: o.height,
        depth: o.depth,
        plate_width: 0.0,
        plate_height: 0.0,
        margin: 0.0,
        material,
    };
    Ok(SpecThermal {
        thermal: thermal(&size, loss, req),
        weakest: o.weakest,
    })
}

/// One line saying where the size of the cooling answer comes from.
pub fn provenance_line(weakest: Source) -> String {
    format!("Enclosure size: {}.", weakest.words())
}

/// The lines to show with a cooling answer: where the size comes from and,
/// while any number of the record is a catalogue or default value, the
/// estimate notice ([`EnclosureSpec::notice`]). The weakest source alone
/// would hide a catalogue width behind a typed depth. Empty when the record
/// has no outside size.
pub fn size_lines(spec: &EnclosureSpec) -> Vec<String> {
    let Ok(o) = outside(spec) else {
        return Vec::new();
    };
    let mut lines = vec![provenance_line(o.weakest)];
    if let Some(n) = spec.notice() {
        lines.push(n.to_owned());
    }
    lines
}
