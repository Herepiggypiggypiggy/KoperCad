//! What the chosen enclosure ([`crate::spec::EnclosureSpec`]) gives the
//! drilling plan, the bill of materials and the cabinet sheet of the
//! reports (`handoff/cabinet-start/CHAIN.md`, rows "Drill and cut output",
//! "BOM and part list", "Reports and handover").
//!
//! Pure, millimetres. A field the spec lacks is named ([`Missing`]) and never
//! replaced by a default. Every number says where it came from
//! ([`Source`]).

use crate::spec::{EnclosureSpec, Field, Missing, Source};

/// A mounting plate size, as stored on the enclosure.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlateSize {
    pub width: f64,
    pub height: f64,
}

/// One row of the cabinet sheet.
#[derive(Debug, Clone, PartialEq)]
pub struct SheetRow {
    pub label: &'static str,
    pub value: String,
    /// `None` for text rows (article, series, STEP file).
    pub source: Option<Source>,
}

fn mm(v: f64) -> String {
    if (v - v.round()).abs() < 0.05 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

fn field_row(label: &'static str, f: Option<Field>) -> SheetRow {
    match f {
        Some(f) => SheetRow {
            label,
            value: format!("{} mm", mm(f.value)),
            source: Some(f.source),
        },
        None => SheetRow {
            label,
            value: "not known".into(),
            source: None,
        },
    }
}

/// The cabinet sheet: article, size, wall, plate and where each number
/// comes from. Empty when no enclosure was chosen.
pub fn cabinet_sheet(spec: &EnclosureSpec, plate: Option<PlateSize>) -> Vec<SheetRow> {
    if spec.is_empty() {
        return Vec::new();
    }
    let text = |label: &'static str, v: &str| SheetRow {
        label,
        value: v.to_owned(),
        source: None,
    };
    let mut rows = vec![text("Article", &spec.article), text("Series", &spec.series)];
    rows.push(field_row("Outside width", spec.width));
    rows.push(field_row("Outside height", spec.height));
    rows.push(field_row("Outside depth", spec.depth));
    rows.push(field_row("Wall thickness", spec.wall));
    rows.push(field_row("Plate setback", spec.plate_setback));
    rows.push(field_row("Door inner depth", spec.door_inner_depth));
    if let Some(p) = plate {
        rows.push(SheetRow {
            label: "Mounting plate",
            value: format!("{} x {} mm", mm(p.width), mm(p.height)),
            source: None,
        });
    }
    if !spec.step_name.is_empty() {
        rows.push(text("STEP file", &spec.step_name));
    }
    rows
}

/// Bill of materials line for the enclosure and its plate.
#[derive(Debug, Clone, PartialEq)]
pub struct BomLine {
    /// `enclosure` or `plate`.
    pub role: &'static str,
    /// Maker article; empty when the record has none (never invented).
    pub article: String,
    pub description: String,
    pub quantity: u32,
    /// Weakest source of the numbers in the description.
    pub source: Option<Source>,
}

/// The enclosure and its plate as parts. Empty without a chosen enclosure.
pub fn bom_lines(spec: &EnclosureSpec, plate: Option<PlateSize>) -> Vec<BomLine> {
    if spec.is_empty() {
        return Vec::new();
    }
    let size = match (spec.width, spec.height, spec.depth) {
        (Some(w), Some(h), Some(d)) => {
            format!("{} x {} x {} mm", mm(w.value), mm(h.value), mm(d.value))
        }
        _ => "size not known".into(),
    };
    let series = if spec.series.is_empty() {
        "Enclosure".to_owned()
    } else {
        format!("Enclosure {}", spec.series)
    };
    let sources = [spec.width, spec.height, spec.depth]
        .iter()
        .filter_map(|f| f.map(|f| f.source))
        .max();
    let mut lines = vec![BomLine {
        role: "enclosure",
        article: spec.article.clone(),
        description: format!("{series}, {size}"),
        quantity: 1,
        source: sources,
    }];
    if let Some(p) = plate {
        lines.push(BomLine {
            role: "plate",
            article: String::new(),
            description: format!("Mounting plate {} x {} mm", mm(p.width), mm(p.height)),
            quantity: 1,
            source: None,
        });
    }
    lines
}

/// Lines for the top of the drilling plan: which enclosure the plan is for,
/// where its numbers come from, and whether the plate fits. Missing fields
/// are named, never defaulted. Empty when no enclosure was chosen.
pub fn drill_lines(spec: &EnclosureSpec, plate: Option<PlateSize>) -> Vec<String> {
    if spec.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    let name = if spec.article.is_empty() {
        "Enclosure".to_owned()
    } else {
        format!("Enclosure {}", spec.article)
    };
    match spec.interior() {
        Ok(i) => {
            out.push(format!(
                "{name}: inside {} x {} x {} mm ({}).",
                mm(i.width),
                mm(i.height),
                mm(i.depth),
                i.weakest.words()
            ));
            if let Some(p) = plate
                && (p.width > i.width + 0.5 || p.height > i.height + 0.5)
            {
                out.push(format!(
                    "The plate {} x {} mm does not fit inside {} x {} mm.",
                    mm(p.width),
                    mm(p.height),
                    mm(i.width),
                    mm(i.height)
                ));
            }
            match spec.plate_setback {
                Some(s) => out.push(format!(
                    "Free depth in front of the plate: {} mm ({}).",
                    mm((i.depth - s.value).max(0.0)),
                    i.weakest.max(s.source).words()
                )),
                None => out.push(Missing("plate setback").to_string()),
            }
        }
        Err(m) => out.push(m.to_string()),
    }
    if let Some(n) = spec.notice() {
        out.push(n.to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(w: f64, source: Source) -> EnclosureSpec {
        let f = |v| Some(Field::new(v, source));
        EnclosureSpec {
            article: "8455.600".into(),
            series: "TS 8".into(),
            width: f(w),
            height: f(1800.0),
            depth: f(500.0),
            wall: f(2.0),
            plate_setback: f(30.0),
            ..EnclosureSpec::default()
        }
    }

    const PLATE: PlateSize = PlateSize {
        width: 560.0,
        height: 1700.0,
    };

    #[test]
    fn changing_the_spec_changes_every_output() {
        let (a, b) = (
            spec(600.0, Source::Catalogue),
            spec(800.0, Source::Catalogue),
        );
        assert_ne!(
            bom_lines(&a, Some(PLATE))[0].description,
            bom_lines(&b, Some(PLATE))[0].description
        );
        assert_ne!(drill_lines(&a, Some(PLATE)), drill_lines(&b, Some(PLATE)));
        assert_ne!(cabinet_sheet(&a, None), cabinet_sheet(&b, None));
        assert!(
            bom_lines(&a, None)[0]
                .description
                .contains("600 x 1800 x 500")
        );
    }

    #[test]
    fn plate_that_does_not_fit_is_reported() {
        let narrow = spec(500.0, Source::Typed);
        let lines = drill_lines(&narrow, Some(PLATE));
        assert!(
            lines.iter().any(|l| l.contains("does not fit")),
            "{lines:?}"
        );
        let wide = spec(600.0, Source::Typed);
        assert!(
            !drill_lines(&wide, Some(PLATE))
                .iter()
                .any(|l| l.contains("does not fit"))
        );
    }

    #[test]
    fn provenance_and_notice_follow_the_source() {
        let cat = drill_lines(&spec(600.0, Source::Catalogue), None);
        assert!(cat.iter().any(|l| l.contains("catalogue")));
        assert!(cat.iter().any(|l| l.starts_with("Estimate")));
        let step = drill_lines(&spec(600.0, Source::Step), None);
        assert!(step.iter().any(|l| l.contains("from your STEP")));
        assert!(!step.iter().any(|l| l.starts_with("Estimate")));
    }

    #[test]
    fn missing_fields_are_named_not_defaulted() {
        let mut s = spec(600.0, Source::Catalogue);
        s.depth = None;
        let lines = drill_lines(&s, None);
        assert!(lines[0].contains("outside depth"), "{lines:?}");
        s.depth = Some(Field::new(500.0, Source::Catalogue));
        s.plate_setback = None;
        assert!(
            drill_lines(&s, None)
                .iter()
                .any(|l| l.contains("plate setback"))
        );
    }

    #[test]
    fn no_enclosure_gives_nothing() {
        let e = EnclosureSpec::default();
        assert!(drill_lines(&e, Some(PLATE)).is_empty());
        assert!(bom_lines(&e, Some(PLATE)).is_empty());
        assert!(cabinet_sheet(&e, None).is_empty());
    }
}
