//! Model health report for a STEP file (lane STEP-HEALTH).
//!
//! Answers "does KoperCAD read this file, and what does it leave out?" in
//! plain words, from the file's own text. It never changes the model and
//! never leaves the machine: the caller hands in the text, the report holds
//! counts, names and colours only (no geometry, nothing of the model itself
//! that could be passed on).
//!
//! The report has three parts:
//! - **Reading**: the schema, the length unit, how many solids and faces
//!   were found, which faces the reader draws exactly, which it only
//!   approximates from their outline and which entity types it does not
//!   know at all.
//! - **Appearance**: product names (door, mounting plate, gland plate...)
//!   and the colours the file gives its solids and faces, read from the
//!   `STYLED_ITEM` chain (`COLOUR_RGB`).
//! - **Findings**: a short list, each with a severity, that says what to do.
//!
//! Where it is in the app: Parts window > a part with a STEP model >
//! 3D model > Check model.

use std::collections::BTreeMap;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::build::{self, BuildError, StepOptions, StepStats};
use super::parse::{Entity, Param, StepFile};

/// How serious a finding is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    Info,
    Warning,
    Problem,
}

/// One line of advice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub text: String,
}

/// How a face surface type is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Drawn exactly (analytic surface or B-spline).
    Exact,
    /// Drawn flat from its boundary: the shape is close, not exact.
    Approximate,
}

/// Faces of one surface type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceCount {
    pub entity: String,
    pub faces: u32,
    pub support: Support,
}

/// A colour used in the file, as 8-bit sRGB (the file stores 0..1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColourUse {
    pub rgb: [u8; 3],
    /// Solids styled with it directly.
    pub solids: u32,
    /// Faces styled with it (a face style overrides its solid's).
    pub faces: u32,
}

impl ColourUse {
    pub fn hex(&self) -> String {
        hex(self.rgb)
    }
}

/// `#rrggbb`.
pub fn hex(rgb: [u8; 3]) -> String {
    format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2])
}

/// A named solid and its colour, when the file gives one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolidInfo {
    pub name: String,
    /// Directly styled colour, else the colour most of its faces have.
    pub rgb: Option<[u8; 3]>,
    pub faces: u32,
}

/// Names and colours read from the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Appearance {
    /// `PRODUCT` names, in file order, without empty or repeated ones.
    pub products: Vec<String>,
    pub solids: Vec<SolidInfo>,
    /// Most used first.
    pub colours: Vec<ColourUse>,
}

/// The report.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StepHealth {
    pub bytes: usize,
    pub entities: usize,
    /// `FILE_SCHEMA`, for example `AUTOMOTIVE_DESIGN`.
    pub schema: Option<String>,
    /// Millimetres per file unit; `None` when no length unit could be read.
    pub unit_mm: Option<f64>,
    /// Counts of the entity types that carry the file's weight, largest first
    /// (at most 12).
    pub top_entities: Vec<(String, usize)>,
    pub surfaces: Vec<SurfaceCount>,
    pub stats: Option<StepStats>,
    /// Box around the solids, millimetres.
    pub size_mm: Option<[f64; 3]>,
    pub read_ms: u64,
    /// Why the geometry could not be built, in plain words.
    pub build_error: Option<String>,
    pub appearance: Appearance,
    pub findings: Vec<Finding>,
}

impl StepHealth {
    /// The worst finding, `Info` when there are none.
    pub fn worst(&self) -> Severity {
        self.findings
            .iter()
            .map(|f| f.severity)
            .max()
            .unwrap_or(Severity::Info)
    }

    /// One line: how it went.
    pub fn headline(&self) -> String {
        match self.worst() {
            Severity::Problem => "This model does not read well.".into(),
            Severity::Warning => "This model reads, with some limits.".into(),
            Severity::Info => "This model reads completely.".into(),
        }
    }

    /// The report as plain text, for copy and paste into a bug report.
    pub fn to_text(&self) -> String {
        let mut o = String::new();
        o.push_str(&self.headline());
        o.push('\n');
        o.push_str(&format!(
            "File: {} bytes, {} entities, schema {}, unit {}\n",
            self.bytes,
            self.entities,
            self.schema.as_deref().unwrap_or("unknown"),
            self.unit_mm
                .map_or("unknown".to_owned(), |u| format!("{u} mm"))
        ));
        if let Some(s) = &self.stats {
            o.push_str(&format!(
                "Solids {}, placements {}, faces {} ({} approximated, {} skipped), {} triangles, {} ms\n",
                s.solids, s.placements, s.faces, s.approximated_faces, s.skipped_faces,
                s.raw_triangles, self.read_ms
            ));
        }
        if let Some(sz) = self.size_mm {
            o.push_str(&format!(
                "Size {:.1} x {:.1} x {:.1} mm\n",
                sz[0], sz[1], sz[2]
            ));
        }
        for s in &self.surfaces {
            o.push_str(&format!(
                "  {} x{} ({})\n",
                s.entity,
                s.faces,
                match s.support {
                    Support::Exact => "exact",
                    Support::Approximate => "approximated",
                }
            ));
        }
        if !self.appearance.products.is_empty() {
            o.push_str(&format!(
                "Products: {}\n",
                self.appearance.products.join(", ")
            ));
        }
        for c in &self.appearance.colours {
            o.push_str(&format!(
                "Colour {}: {} solids, {} faces\n",
                c.hex(),
                c.solids,
                c.faces
            ));
        }
        for f in &self.findings {
            o.push_str(&format!("{:?}: {}\n", f.severity, f.text));
        }
        o
    }
}

/// Time the geometry build may take for a health check.
const BUILD_LIMIT: Duration = Duration::from_secs(30);

/// Surface types the reader draws exactly.
fn exact_surface(name: &str) -> bool {
    matches!(
        name,
        "PLANE"
            | "CYLINDRICAL_SURFACE"
            | "CONICAL_SURFACE"
            | "SPHERICAL_SURFACE"
            | "TOROIDAL_SURFACE"
            | "B_SPLINE_SURFACE_WITH_KNOTS"
            | "B_SPLINE_SURFACE"
            | "RATIONAL_B_SPLINE_SURFACE"
            | "QUASI_UNIFORM_SURFACE"
            | "UNIFORM_SURFACE"
            | "BEZIER_SURFACE"
    )
}

/// Reads `bytes` and reports. Never fails: a file that is not STEP gives a
/// report with a `Problem`.
pub fn check_step(bytes: &[u8], cancel: &AtomicBool) -> StepHealth {
    let mut h = StepHealth {
        bytes: bytes.len(),
        ..StepHealth::default()
    };
    if bytes.len() > super::MAX_STEP_BYTES {
        h.build_error = Some("the file is larger than the 96 MB KoperCAD reads".into());
        finish(&mut h);
        return h;
    }
    let text = String::from_utf8_lossy(bytes);
    let file = match StepFile::parse(&text) {
        Ok(f) => f,
        Err(e) => {
            h.build_error = Some(e.to_string());
            finish(&mut h);
            return h;
        }
    };
    h.entities = file.len();
    h.schema = schema_of(&text);
    let mut counts: Vec<(String, usize)> =
        file.name_counts().map(|(n, c)| (n.to_owned(), c)).collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts.truncate(12);
    h.top_entities = counts;
    h.surfaces = surface_counts(&file);
    h.appearance = appearance(&file);

    // The geometry build, with the same reader the 3D view uses.
    let t0 = Instant::now();
    let opts = StepOptions {
        chord_tol_mm: 0.5,
        deadline: Some(t0 + BUILD_LIMIT),
        ..StepOptions::default()
    };
    let built = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build::step_soup_checked(&text, &opts, cancel)
    }));
    h.read_ms = t0.elapsed().as_millis() as u64;
    match built {
        Err(_) => h.build_error = Some("the reader failed on this file's geometry".into()),
        Ok(Err(e)) => h.build_error = Some(build_error_text(e)),
        Ok(Ok((soup, stats, unit_unknown))) => {
            h.unit_mm = if unit_unknown {
                None
            } else {
                Some(super::brep::Reader::new(&file, 0.5).scale)
            };
            h.stats = Some(stats);
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for t in &soup.tris {
                for &i in t {
                    let p = soup.pos[i as usize];
                    for k in 0..3 {
                        lo[k] = lo[k].min(p[k]);
                        hi[k] = hi[k].max(p[k]);
                    }
                }
            }
            if (0..3).all(|k| (hi[k] - lo[k]).is_finite()) {
                h.size_mm = Some([hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]]);
            }
        }
    }
    finish(&mut h);
    h
}

fn build_error_text(e: BuildError) -> String {
    match e {
        BuildError::Cancelled => "cancelled".into(),
        other => other.to_string(),
    }
}

/// `FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 ... }'))` to its first
/// schema name.
fn schema_of(text: &str) -> Option<String> {
    let head_end = text.to_ascii_uppercase().find("ENDSEC")?;
    let head = &text[..head_end];
    let at = head.to_ascii_uppercase().find("FILE_SCHEMA")?;
    let rest = &head[at..];
    let q1 = rest.find('\'')?;
    let rest = &rest[q1 + 1..];
    let q2 = rest.find('\'')?;
    let name = rest[..q2].split('{').next()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

fn surface_counts(file: &StepFile) -> Vec<SurfaceCount> {
    let mut by: BTreeMap<String, u32> = BTreeMap::new();
    for face_kind in ["ADVANCED_FACE", "FACE_SURFACE"] {
        for &id in file.ids_of(face_kind) {
            let Some(e) = file.entity(id) else { continue };
            // ADVANCED_FACE(name, bounds, face_geometry, same_sense)
            let Some(sref) = e.args().get(2).and_then(Param::as_ref) else {
                continue;
            };
            let name = match file.entity(sref) {
                Some(se) if super::brep::is_spline_surface(&se) => {
                    "B_SPLINE_SURFACE_WITH_KNOTS".to_owned()
                }
                Some(se) => se.name().to_owned(),
                None => "MISSING_SURFACE".to_owned(),
            };
            *by.entry(name).or_default() += 1;
        }
    }
    let mut v: Vec<SurfaceCount> = by
        .into_iter()
        .map(|(entity, faces)| SurfaceCount {
            support: if exact_surface(&entity) {
                Support::Exact
            } else {
                Support::Approximate
            },
            entity,
            faces,
        })
        .collect();
    v.sort_by(|a, b| b.faces.cmp(&a.faces).then_with(|| a.entity.cmp(&b.entity)));
    v
}

// ---- appearance -------------------------------------------------------

/// STEP text escapes: `\X2\0041\X0\` (UTF-16), `\X4\` (UTF-32), `\X\E9`
/// (Latin-1), `\S\` (shifted Latin-1) and doubled quotes.
pub fn decode_text(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '\\' && i + 3 < b.len() + 1 {
            let rest: String = b[i..].iter().take(4).collect();
            if rest.starts_with("\\X2\\") || rest.starts_with("\\X4\\") {
                let wide = rest.starts_with("\\X4\\");
                let width = if wide { 8 } else { 4 };
                let start = i + 4;
                let mut end = start;
                while end < b.len() && !(b[end] == '\\') {
                    end += 1;
                }
                let digits: String = b[start..end.min(b.len())].iter().collect();
                if wide {
                    for ch in digits.as_bytes().chunks_exact(width) {
                        if let Some(c) = std::str::from_utf8(ch)
                            .ok()
                            .and_then(|h| u32::from_str_radix(h, 16).ok())
                            .and_then(char::from_u32)
                        {
                            out.push(c);
                        }
                    }
                } else {
                    let units: Vec<u16> = digits
                        .as_bytes()
                        .chunks_exact(width)
                        .filter_map(|ch| {
                            std::str::from_utf8(ch)
                                .ok()
                                .and_then(|h| u16::from_str_radix(h, 16).ok())
                        })
                        .collect();
                    out.push_str(&String::from_utf16_lossy(&units));
                }
                // Skip the closing \X0\.
                i = end;
                if b.get(i..i + 4)
                    .is_some_and(|t| t.iter().collect::<String>() == "\\X0\\")
                {
                    i += 4;
                } else {
                    i = (i + 1).min(b.len());
                }
                continue;
            }
            if rest.starts_with("\\X\\") && i + 5 <= b.len() {
                let h: String = b[i + 3..i + 5].iter().collect();
                if let Some(c) = u32::from_str_radix(&h, 16).ok().and_then(char::from_u32) {
                    out.push(c);
                    i += 5;
                    continue;
                }
            }
            if rest.starts_with("\\S\\") && i + 4 <= b.len() {
                let c = b[i + 3] as u32 + 0x80;
                if let Some(c) = char::from_u32(c) {
                    out.push(c);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

fn text_arg(e: &Entity, idx: usize) -> Option<String> {
    let t = decode_text(e.args().get(idx)?.as_text()?).trim().to_owned();
    (!t.is_empty()).then_some(t)
}

/// Follows a style chain to a `COLOUR_RGB`.
fn style_colour(file: &StepFile, id: u32, depth: u8) -> Option<[u8; 3]> {
    if depth > 9 {
        return None;
    }
    let e = file.entity(id)?;
    if e.has("COLOUR_RGB") {
        let a = e.part("COLOUR_RGB")?;
        let ch = |i: usize| {
            a.get(i)
                .and_then(Param::as_f64)
                .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
        };
        return Some([ch(1)?, ch(2)?, ch(3)?]);
    }
    let follow = matches!(
        e.name(),
        "PRESENTATION_STYLE_ASSIGNMENT"
            | "PRESENTATION_STYLE_BY_CONTEXT"
            | "SURFACE_STYLE_USAGE"
            | "SURFACE_SIDE_STYLE"
            | "SURFACE_STYLE_FILL_AREA"
            | "FILL_AREA_STYLE"
            | "FILL_AREA_STYLE_COLOUR"
            | "SURFACE_STYLE_RENDERING"
            | "SURFACE_STYLE_RENDERING_WITH_PROPERTIES"
    );
    if !follow {
        return None;
    }
    let refs: Vec<u32> = e
        .args()
        .iter()
        .flat_map(|p| match p {
            Param::Ref(r) => vec![*r],
            Param::List(_) => p.refs(),
            _ => Vec::new(),
        })
        .collect();
    refs.into_iter()
        .find_map(|r| style_colour(file, r, depth + 1))
}

fn appearance(file: &StepFile) -> Appearance {
    let mut out = Appearance::default();
    for &id in file.ids_of("PRODUCT") {
        let Some(e) = file.entity(id) else { continue };
        // PRODUCT(id, name, description, frame_of_reference)
        for idx in [1, 0] {
            if let Some(n) = text_arg(&e, idx) {
                if !out.products.contains(&n) {
                    out.products.push(n);
                }
                break;
            }
        }
    }

    // Item id to colour, from every styled item.
    let mut item_colour: BTreeMap<u32, [u8; 3]> = BTreeMap::new();
    for kind in ["STYLED_ITEM", "OVER_RIDING_STYLED_ITEM"] {
        for &id in file.ids_of(kind) {
            let Some(e) = file.entity(id) else { continue };
            // STYLED_ITEM(name, styles, item)
            let Some(item) = e.args().get(2).and_then(Param::as_ref) else {
                continue;
            };
            let styles = e.args().get(1).map(Param::refs).unwrap_or_default();
            if let Some(c) = styles.iter().find_map(|&s| style_colour(file, s, 0)) {
                item_colour.insert(item, c);
            }
        }
    }

    let mut uses: BTreeMap<[u8; 3], (u32, u32)> = BTreeMap::new();
    for kind in [
        "MANIFOLD_SOLID_BREP",
        "BREP_WITH_VOIDS",
        "FACETED_BREP",
        "SHELL_BASED_SURFACE_MODEL",
    ] {
        for &id in file.ids_of(kind) {
            let Some(e) = file.entity(id) else { continue };
            let name = text_arg(&e, 0).unwrap_or_default();
            // The outer shell holds the faces.
            let shells: Vec<u32> = match e.args().get(1) {
                Some(Param::Ref(r)) => vec![*r],
                Some(p) => p.refs(),
                None => Vec::new(),
            };
            let faces: Vec<u32> = shells
                .iter()
                .filter_map(|&s| file.entity(s))
                .flat_map(|sh| sh.args().get(1).map(Param::refs).unwrap_or_default())
                .collect();
            let direct = item_colour.get(&id).copied();
            let mut face_tally: BTreeMap<[u8; 3], u32> = BTreeMap::new();
            for f in &faces {
                if let Some(c) = item_colour.get(f) {
                    *face_tally.entry(*c).or_default() += 1;
                }
            }
            if let Some(c) = direct {
                uses.entry(c).or_default().0 += 1;
            }
            for (c, n) in &face_tally {
                uses.entry(*c).or_default().1 += n;
            }
            let dominant = face_tally.iter().max_by_key(|(_, n)| **n).map(|(c, _)| *c);
            out.solids.push(SolidInfo {
                name,
                rgb: direct.or(dominant),
                faces: faces.len() as u32,
            });
        }
    }
    // Colours that style something else (an assembly node, a mapped item).
    if uses.is_empty() {
        for c in item_colour.values() {
            uses.entry(*c).or_default().0 += 1;
        }
    }
    let mut colours: Vec<ColourUse> = uses
        .into_iter()
        .map(|(rgb, (solids, faces))| ColourUse { rgb, solids, faces })
        .collect();
    colours.sort_by(|a, b| {
        (b.solids + b.faces)
            .cmp(&(a.solids + a.faces))
            .then(a.rgb.cmp(&b.rgb))
    });
    out.colours = colours;
    out
}

// ---- findings ---------------------------------------------------------

fn finish(h: &mut StepHealth) {
    let mut f: Vec<Finding> = Vec::new();
    let mut add = |severity, text: String| f.push(Finding { severity, text });

    if let Some(why) = &h.build_error {
        add(
            Severity::Problem,
            format!(
                "The geometry could not be built: {why}. The part is drawn as a box from its catalog size."
            ),
        );
    }
    if h.build_error.is_none() && h.unit_mm.is_none() {
        add(
            Severity::Warning,
            "No readable length unit in the file; millimetres were assumed. Check the size below."
                .into(),
        );
    }
    if let Some(s) = &h.stats {
        if s.skipped_faces > 0 {
            let pct = 100.0 * f64::from(s.skipped_faces) / f64::from(s.faces.max(1));
            add(
                if pct > 5.0 {
                    Severity::Problem
                } else {
                    Severity::Warning
                },
                format!(
                    "{} of {} faces ({pct:.1} %) could not be drawn and are missing from the 3D view.",
                    s.skipped_faces, s.faces
                ),
            );
        }
        if s.approximated_faces > 0 {
            let pct = 100.0 * f64::from(s.approximated_faces) / f64::from(s.faces.max(1));
            add(
                if pct > 25.0 {
                    Severity::Warning
                } else {
                    Severity::Info
                },
                format!(
                    "{} faces ({pct:.1} %) are drawn flat from their outline; curved shapes there are close, not exact.",
                    s.approximated_faces
                ),
            );
        }
    }
    let odd: Vec<&str> = h
        .surfaces
        .iter()
        .filter(|s| s.support == Support::Approximate)
        .map(|s| s.entity.as_str())
        .collect();
    if !odd.is_empty() {
        add(
            Severity::Info,
            format!("Surface types without exact support: {}.", odd.join(", ")),
        );
    }
    if let Some(sz) = h.size_mm {
        let longest = sz.iter().cloned().fold(0.0, f64::max);
        if longest > 5000.0 {
            add(
                Severity::Warning,
                format!("The model is {longest:.0} mm long. Wrong unit, or a whole assembly?"),
            );
        } else if longest > 0.0 && longest < 1.0 {
            add(
                Severity::Warning,
                format!("The model is only {longest:.2} mm long. Wrong unit?"),
            );
        }
    }
    if h.build_error.is_none() && h.appearance.colours.is_empty() {
        add(
            Severity::Info,
            "The file carries no colours; the part is drawn in the default colour.".into(),
        );
    }
    if h.build_error.is_none() && h.appearance.products.is_empty() {
        add(
            Severity::Info,
            "The file names no products, so sub-parts cannot be told apart by name.".into(),
        );
    }
    if f.is_empty() {
        f.push(Finding {
            severity: Severity::Info,
            text: "Nothing to fix.".into(),
        });
    }
    f.sort_by_key(|b| std::cmp::Reverse(b.severity));
    h.findings = f;
}
