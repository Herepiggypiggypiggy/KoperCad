//! STEP model health: does a file read cleanly, and what does the reader do
//! with what it cannot read exactly?
//!
//! [`step_health`] runs the same reader the 3D view uses and reports, in plain
//! words: entities found, geometry entity kinds the reader does not know,
//! solids and faces (and how many faces were only drawn flat from their
//! edges), the size, the product names and the colours. It reads text only and
//! never changes a file.
//!
//! A folder of files (the owner's downloads, which stay private and out of the
//! repository) is run with [`health_of_folder`]; [`markdown_report`] writes the
//! result as a table, and the window and `kopercad-cli parts step-health`
//! both show it.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::build::{self, BuildError, StepOptions};
use super::parse::StepFile;
use super::style::{self, StepStyle, hex};

/// Entity kinds the reader reads (geometry), in upper case.
const READ: &[&str] = &[
    "ADVANCED_BREP_SHAPE_REPRESENTATION",
    "ADVANCED_FACE",
    "BREP_WITH_VOIDS",
    "B_SPLINE_CURVE",
    "B_SPLINE_CURVE_WITH_KNOTS",
    "B_SPLINE_SURFACE",
    "B_SPLINE_SURFACE_WITH_KNOTS",
    "CIRCLE",
    "CLOSED_SHELL",
    "CONICAL_SURFACE",
    "CYLINDRICAL_SURFACE",
    "EDGE_CURVE",
    "ELLIPSE",
    "FACETED_BREP",
    "FACETED_BREP_SHAPE_REPRESENTATION",
    "FACE_SURFACE",
    "INTERSECTION_CURVE",
    "LINE",
    "MANIFOLD_SOLID_BREP",
    "MANIFOLD_SURFACE_SHAPE_REPRESENTATION",
    "OPEN_SHELL",
    "PCURVE",
    "PLANE",
    "POLYLINE",
    "RATIONAL_B_SPLINE_CURVE",
    "RATIONAL_B_SPLINE_SURFACE",
    "SEAM_CURVE",
    "SHELL_BASED_SURFACE_MODEL",
    "SPHERICAL_SURFACE",
    "SURFACE_CURVE",
    "TESSELLATED_SHELL",
    "TESSELLATED_SOLID",
    "TOROIDAL_SURFACE",
    "TRIANGULATED_FACE",
    "TRIMMED_CURVE",
];

/// What happens to a geometry kind the reader has no exact formula for.
const KNOWN_GAPS: &[(&str, &str)] = &[
    (
        "SURFACE_OF_REVOLUTION",
        "revolved surface: drawn flat from its edges",
    ),
    (
        "SURFACE_OF_LINEAR_EXTRUSION",
        "extruded surface: drawn flat from its edges",
    ),
    (
        "OFFSET_SURFACE",
        "offset surface: drawn flat from its edges",
    ),
    (
        "DEGENERATE_TOROIDAL_SURFACE",
        "spindle torus: drawn flat from its edges",
    ),
    ("CURVE_BOUNDED_SURFACE", "bounded surface: not read"),
    (
        "COMPOSITE_CURVE",
        "composite curve: edges using it may be missing",
    ),
    ("BOUNDED_SURFACE", "bounded surface: not read"),
    (
        "BOUNDED_CURVE",
        "bounded curve: edges using it may be missing",
    ),
];

/// Longest the reader may take for one file.
const DEADLINE: Duration = Duration::from_secs(30);
/// Larger files are not read.
const MAX_BYTES: u64 = super::MAX_STEP_BYTES as u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Verdict {
    /// Everything read exactly.
    Clean,
    /// Read, but some faces or entities were approximated or left out.
    Partial,
    /// Nothing usable came out.
    Unreadable,
}

impl Verdict {
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Clean => "Reads cleanly",
            Verdict::Partial => "Reads with gaps",
            Verdict::Unreadable => "Cannot be read",
        }
    }
}

/// A geometry kind the reader does not read exactly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported {
    pub entity: String,
    pub count: usize,
    pub effect: String,
}

#[derive(Debug, Clone)]
pub struct StepHealth {
    pub verdict: Verdict,
    /// Why nothing could be read, in plain words.
    pub error: Option<String>,
    pub schema: Option<String>,
    /// The program that wrote the file (from `FILE_NAME`).
    pub originator: Option<String>,
    pub entities: usize,
    pub kinds: usize,
    /// The most common entity kinds, most first.
    pub top_entities: Vec<(String, usize)>,
    pub unsupported: Vec<Unsupported>,
    pub solids: u32,
    pub placements: u32,
    pub faces: u32,
    /// Faces drawn flat from their boundary instead of exactly.
    pub approximated_faces: u32,
    pub skipped_faces: u32,
    pub triangles: u32,
    /// Box around the solids in millimetres: lowest and highest corner.
    pub bbox_mm: Option<([f64; 3], [f64; 3])>,
    /// A length unit was declared that is not known; millimetres assumed.
    pub unit_unknown: bool,
    pub style: StepStyle,
    /// Plain-language notes, most important first.
    pub notes: Vec<String>,
}

impl StepHealth {
    fn empty() -> StepHealth {
        StepHealth {
            verdict: Verdict::Unreadable,
            error: None,
            schema: None,
            originator: None,
            entities: 0,
            kinds: 0,
            top_entities: Vec::new(),
            unsupported: Vec::new(),
            solids: 0,
            placements: 0,
            faces: 0,
            approximated_faces: 0,
            skipped_faces: 0,
            triangles: 0,
            bbox_mm: None,
            unit_unknown: false,
            style: StepStyle::default(),
            notes: Vec::new(),
        }
    }

    pub fn size_mm(&self) -> Option<[f64; 3]> {
        self.bbox_mm
            .map(|(lo, hi)| [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]])
    }

    /// One line for a list.
    pub fn summary(&self) -> String {
        match (&self.error, self.size_mm()) {
            (Some(e), _) => format!("{}: {e}", self.verdict.word()),
            (None, Some(s)) => format!(
                "{}: {} solids, {} faces, {:.1} x {:.1} x {:.1} mm",
                self.verdict.word(),
                self.solids,
                self.faces,
                s[0],
                s[1],
                s[2]
            ),
            (None, None) => self.verdict.word().to_owned(),
        }
    }
}

fn header_text(text: &str, keyword: &str) -> Option<Vec<String>> {
    let start = text.find(keyword)?;
    let rest = &text[start + keyword.len()..];
    let end = rest.find(");")?;
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut inq = false;
    for ch in rest[..end].chars() {
        match (ch, inq) {
            ('\'', false) => inq = true,
            ('\'', true) => {
                inq = false;
                out.push(std::mem::take(&mut cur));
            }
            (c, true) => cur.push(c),
            _ => {}
        }
    }
    Some(out)
}

/// Runs the reader on a STEP text and says how it went. Never panics: a file
/// that trips the reader ends as `Unreadable` with a message.
pub fn step_health(text: &str) -> StepHealth {
    let mut h = StepHealth::empty();
    let file = match StepFile::parse(text) {
        Ok(f) => f,
        Err(e) => {
            h.error = Some(e.to_string());
            return h;
        }
    };
    h.schema = header_text(text, "FILE_SCHEMA(").and_then(|v| v.into_iter().next());
    // FILE_NAME(name, time, author, org, preprocessor, originating system, auth)
    h.originator = header_text(text, "FILE_NAME(")
        .and_then(|v| v.get(5).cloned())
        .filter(|s| !s.is_empty());

    // Entity census: one scan of the index.
    let names = entity_names(&file);
    h.entities = file.len();
    h.kinds = names.len();
    let mut top: Vec<(String, usize)> = names.iter().map(|(n, c)| (n.clone(), *c)).collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    top.truncate(10);
    h.top_entities = top;
    h.unsupported = unsupported(&names);
    h.style = style::read_style(&file);

    let opts = StepOptions {
        chord_tol_mm: 0.5,
        deadline: Some(Instant::now() + DEADLINE),
        ..StepOptions::default()
    };
    let built =
        std::panic::catch_unwind(|| build::step_soup_checked(text, &opts, &AtomicBool::new(false)));
    match built {
        Err(_) => h.error = Some("the file has geometry that crashes the reader".into()),
        Ok(Err(e)) => {
            h.error = Some(match e {
                BuildError::NoGeometry => format!(
                    "{e} ({} entities were found, none of them a solid)",
                    h.entities
                ),
                other => other.to_string(),
            });
        }
        Ok(Ok((soup, stats, unit_unknown))) => {
            h.solids = stats.solids;
            h.placements = stats.placements;
            h.faces = stats.faces;
            h.approximated_faces = stats.approximated_faces;
            h.skipped_faces = stats.skipped_faces;
            h.triangles = stats.raw_triangles;
            h.unit_unknown = unit_unknown;
            let mut used = vec![false; soup.pos.len()];
            for t in &soup.tris {
                for &i in t {
                    used[i as usize] = true;
                }
            }
            let mut lo = [f64::INFINITY; 3];
            let mut hi = [f64::NEG_INFINITY; 3];
            for (p, _) in soup.pos.iter().zip(&used).filter(|(_, u)| **u) {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            if (0..3).all(|k| (hi[k] - lo[k]).is_finite()) {
                h.bbox_mm = Some((lo, hi));
            }
        }
    }
    finish(&mut h);
    h
}

fn entity_names(file: &StepFile) -> BTreeMap<String, usize> {
    let mut out = BTreeMap::new();
    for name in file.names() {
        out.insert(name.to_owned(), file.ids_of(name).len());
    }
    out
}

fn unsupported(names: &BTreeMap<String, usize>) -> Vec<Unsupported> {
    let mut out = Vec::new();
    for (name, &count) in names {
        if READ.contains(&name.as_str()) {
            continue;
        }
        let effect = if let Some((_, e)) = KNOWN_GAPS.iter().find(|(k, _)| k == name) {
            (*e).to_owned()
        } else if name.contains("STYLE") || name.contains("PRESENTATION") {
            continue;
        } else if name.ends_with("_SURFACE") || name.ends_with("_CURVE") {
            "geometry kind not known to KoperCAD".to_owned()
        } else if name.ends_with("_BREP") || name.ends_with("_SOLID") {
            "solid kind not known to KoperCAD: not drawn".to_owned()
        } else {
            continue;
        };
        out.push(Unsupported {
            entity: name.clone(),
            count,
            effect,
        });
    }
    out.sort_by(|a, b| b.count.cmp(&a.count).then(a.entity.cmp(&b.entity)));
    out
}

fn finish(h: &mut StepHealth) {
    if h.error.is_some() {
        h.verdict = Verdict::Unreadable;
        return;
    }
    if h.approximated_faces > 0 {
        h.notes.push(format!(
            "{} of {} faces were drawn flat from their edges (curved faces that are not planes, cylinders, cones, spheres, tori or B-splines read exactly).",
            h.approximated_faces, h.faces
        ));
    }
    if h.skipped_faces > 0 {
        h.notes.push(format!(
            "{} faces could not be read and are missing from the 3D model.",
            h.skipped_faces
        ));
    }
    if h.unit_unknown {
        h.notes
            .push("The length unit is not one KoperCAD knows; millimetres were assumed, so the size may be wrong.".into());
    }
    for u in &h.unsupported {
        h.notes
            .push(format!("{} x {}: {}.", u.count, u.entity, u.effect));
    }
    if h.style.coloured_solids() == 0 {
        h.notes
            .push("The file has no colours; the part is drawn in the default colour.".into());
    }
    let gap = h.approximated_faces > 0
        || h.skipped_faces > 0
        || h.unit_unknown
        || !h.unsupported.is_empty();
    h.verdict = if gap {
        Verdict::Partial
    } else {
        Verdict::Clean
    };
}

/// The health of one file on disk.
pub fn health_of_file(path: &Path) -> StepHealth {
    let mut h = StepHealth::empty();
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_BYTES => {
            h.error = Some("the file is too large to read".into());
            return h;
        }
        Err(e) => {
            h.error = Some(format!("cannot open the file: {e}"));
            return h;
        }
        Ok(_) => {}
    }
    match std::fs::read(path) {
        Ok(b) => step_health(&String::from_utf8_lossy(&b)),
        Err(e) => {
            h.error = Some(format!("cannot read the file: {e}"));
            h
        }
    }
}

/// Environment variable that names the folder of sample files.
pub const SAMPLES_ENV: &str = "KOPERCAD_STEP_SAMPLES";

/// The folder the owner drops sample files into:
/// `KOPERCAD_STEP_SAMPLES`, else `Documents\KoperCAD-tests\step-samples`.
pub fn default_samples_folder() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os(SAMPLES_ENV).filter(|p| !p.is_empty()) {
        return Some(PathBuf::from(p));
    }
    dirs::document_dir().map(|d| d.join("KoperCAD-tests").join("step-samples"))
}

/// Whether a file name is a STEP file.
pub fn is_step_name(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.ends_with(".stp") || n.ends_with(".step") || n.ends_with(".p21")
}

/// STEP files in `dir` and its subfolders, sorted by path.
pub fn step_files_in(dir: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if depth < 4 {
                    walk(&p, depth + 1, out);
                }
            } else if p
                .file_name()
                .is_some_and(|n| is_step_name(&n.to_string_lossy()))
            {
                out.push(p);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, 0, &mut out);
    out.sort();
    out
}

/// Health of every STEP file in a folder.
pub fn health_of_folder(dir: &Path) -> Vec<(PathBuf, StepHealth)> {
    step_files_in(dir)
        .into_iter()
        .map(|p| {
            let h = health_of_file(&p);
            (p, h)
        })
        .collect()
}

fn dims(s: Option<[f64; 3]>) -> String {
    s.map_or("-".into(), |s| {
        format!("{:.1} x {:.1} x {:.1}", s[0], s[1], s[2])
    })
}

/// A readable report of a run, in Markdown.
pub fn markdown_report(results: &[(PathBuf, StepHealth)], folder: &Path) -> String {
    let mut s = String::from("# STEP model health\n\n");
    s.push_str(&format!("Folder: `{}`\n\n", folder.display()));
    if results.is_empty() {
        s.push_str("No STEP files (.stp, .step) found.\n");
        return s;
    }
    let count = |v: Verdict| results.iter().filter(|(_, h)| h.verdict == v).count();
    s.push_str(&format!(
        "{} files: {} read cleanly, {} with gaps, {} cannot be read.\n\n",
        results.len(),
        count(Verdict::Clean),
        count(Verdict::Partial),
        count(Verdict::Unreadable)
    ));
    s.push_str("| File | Result | Solids | Faces | Flat faces | Size mm | Colour | Name |\n| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for (p, h) in results {
        let name = p
            .file_name()
            .map_or(String::new(), |n| n.to_string_lossy().into_owned());
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            name.replace('|', "/"),
            h.verdict.word(),
            h.solids,
            h.faces,
            h.approximated_faces,
            dims(h.size_mm()),
            h.style.dominant.map_or("-".into(), hex),
            h.style.part_name().unwrap_or("-").replace('|', "/"),
        ));
    }
    for (p, h) in results {
        s.push_str(&format!(
            "\n## {}\n\n",
            p.file_name()
                .map_or(String::new(), |n| n.to_string_lossy().into_owned())
        ));
        if let Some(e) = &h.error {
            s.push_str(&format!("- Not read: {e}\n"));
        }
        if let Some(sc) = &h.schema {
            s.push_str(&format!("- Schema: {sc}\n"));
        }
        if let Some(o) = &h.originator {
            s.push_str(&format!("- Written by: {o}\n"));
        }
        s.push_str(&format!("- {} entities of {} kinds\n", h.entities, h.kinds));
        if !h.top_entities.is_empty() {
            let t: Vec<String> = h
                .top_entities
                .iter()
                .map(|(n, c)| format!("{n} {c}"))
                .collect();
            s.push_str(&format!("- Most common: {}\n", t.join(", ")));
        }
        if !h.style.product_names.is_empty() {
            let names: Vec<&str> = h
                .style
                .product_names
                .iter()
                .take(12)
                .map(String::as_str)
                .collect();
            s.push_str(&format!("- Product names: {}\n", names.join(", ")));
        }
        let mut cols: HashMap<[u8; 3], usize> = HashMap::new();
        for c in h.style.solids.iter().filter_map(|x| x.colour) {
            *cols.entry(c).or_default() += 1;
        }
        let mut cols: Vec<_> = cols.into_iter().collect();
        cols.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        if !cols.is_empty() {
            let t: Vec<String> = cols
                .iter()
                .map(|(c, n)| format!("{} x{n}", hex(*c)))
                .collect();
            s.push_str(&format!("- Colours: {}\n", t.join(", ")));
        }
        for n in &h.notes {
            s.push_str(&format!("- {n}\n"));
        }
    }
    s
}

#[cfg(test)]
mod tests;
