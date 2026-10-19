//! Crane configurator projects: finding the newest output of a custom
//! project and reading what the configurator wrote beside the `.epj`.
//!
//! The configurator (a separate tool that generates EPLAN projects for
//! cranes) writes `output/custom/<name>/vNN/<order>.epj` and, next to it, a
//! `<order>_Projectcontrole.md` (its own check of the project), a
//! `hernoemen.json` (device renaming summary) and, in
//! `custom/<name>/project.json`, the order data. This module finds those
//! files and turns them into report lines. The `.epj` itself is read by
//! [`super::pxf`]; nothing here is EPLAN content.
//!
//! Exact: the file names and the numbers the configurator printed, quoted as
//! written. Inferred: which folder is "the" project when several versions
//! exist (the highest `vNN`).

use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use novo_core::Project;
use thiserror::Error;

use super::pxf::{PxfError, PxfFile, PxfOptions, PxfPlan, plan_pxf, read_pxf};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CraneError {
    #[error("{0} does not exist")]
    Missing(String),
    #[error("no .epj file found in {0}")]
    NoEpj(String),
    #[error("{0} holds several projects ({1}); pick one of them")]
    Several(String, String),
    #[error("reading {0} failed: {1}")]
    Read(String, String),
    #[error("{0}")]
    Pxf(#[from] PxfError),
}

/// Where one configurator output sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CraneSource {
    pub epj: PathBuf,
    /// The folder holding the `.epj` (`.../v95`).
    pub dir: PathBuf,
    /// `95` for `v95`, when the folder is named so.
    pub version: Option<u32>,
    /// The project folder name in `output/custom/<name>/v95`.
    pub name: Option<String>,
    /// Other versions of the same project that were not picked.
    pub other_versions: Vec<u32>,
}

/// What the configurator says about the project, as it printed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CraneSummary {
    pub project: String,
    pub customer: String,
    pub brand: String,
    pub kind: String,
    /// `GOED MET OPMERKINGEN` and the like, from the check report.
    pub verdict: String,
    /// The check report's own count line.
    pub checks: String,
    /// The renaming summary line, built from `hernoemen.json`.
    pub renaming: String,
}

fn version_of(name: &str) -> Option<u32> {
    let n = name.strip_prefix('v').or_else(|| name.strip_prefix('V'))?;
    n.parse().ok()
}

fn first_epj(dir: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("epj")))
        .collect();
    found.sort();
    found.into_iter().next()
}

fn versions(dir: &Path) -> Vec<(u32, PathBuf)> {
    let Ok(rd) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut v: Vec<(u32, PathBuf)> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let n = version_of(p.file_name()?.to_str()?)?;
            first_epj(&p).map(|_| (n, p))
        })
        .collect();
    v.sort();
    v
}

/// Finds the `.epj` for a path the person gave: the file itself, a version
/// folder, a project folder with several versions (the newest wins), or the
/// `output/custom` folder above one project.
pub fn locate(path: &Path) -> Result<CraneSource, CraneError> {
    if !path.exists() {
        return Err(CraneError::Missing(path.display().to_string()));
    }
    if path.is_file() {
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_default();
        return Ok(describe(path.to_path_buf(), dir, Vec::new()));
    }
    if let Some(epj) = first_epj(path) {
        return Ok(describe(epj, path.to_path_buf(), Vec::new()));
    }
    let mut vs = versions(path);
    if let Some((_, newest)) = vs.pop() {
        let others = vs.iter().map(|(n, _)| *n).collect();
        let epj = first_epj(&newest).expect("version folders are filtered by having an epj");
        return Ok(describe(epj, newest, others));
    }
    // `output/custom` or `output`: one project folder below.
    for base in [path.to_path_buf(), path.join("custom")] {
        let Ok(rd) = fs::read_dir(&base) else {
            continue;
        };
        let projects: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.is_dir() && !versions(p).is_empty())
            .collect();
        match projects.len() {
            0 => {}
            1 => return locate(&projects[0]),
            _ => {
                let names: Vec<String> = projects
                    .iter()
                    .filter_map(|p| p.file_name()?.to_str().map(str::to_string))
                    .collect();
                return Err(CraneError::Several(
                    base.display().to_string(),
                    names.join(", "),
                ));
            }
        }
    }
    Err(CraneError::NoEpj(path.display().to_string()))
}

fn describe(epj: PathBuf, dir: PathBuf, other_versions: Vec<u32>) -> CraneSource {
    let version = dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(version_of);
    let name = dir
        .parent()
        .and_then(|p| p.file_name())
        .and_then(|n| n.to_str())
        .filter(|_| version.is_some())
        .map(str::to_string);
    CraneSource {
        epj,
        dir,
        version,
        name,
        other_versions,
    }
}

fn text_field(v: &serde_json::Value, key: &str) -> String {
    v.get(key)
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Reads what sits beside the `.epj`. Every file is optional: a missing or
/// unreadable one leaves its fields empty.
pub fn read_summary(src: &CraneSource) -> CraneSummary {
    let mut s = CraneSummary::default();

    // The configurator's check report, next to the epj.
    let stem = src
        .epj
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    let md = src.dir.join(format!("{stem}_Projectcontrole.md"));
    if let Ok(text) = fs::read_to_string(md) {
        for line in text.lines() {
            let l = line.trim();
            if let Some(v) = l.strip_prefix("**Oordeel:") {
                s.verdict = v.trim_end_matches("**").trim().to_string();
            } else if l.contains("automatische controles:") && s.checks.is_empty() {
                s.checks = l.to_string();
            }
        }
    }

    if let Ok(text) = fs::read_to_string(src.dir.join("hernoemen.json"))
        && let Ok(v) = serde_json::from_str::<serde_json::Value>(&text)
        && let Some(sum) = v.get("summary")
    {
        let n = |k: &str| sum.get(k).and_then(serde_json::Value::as_u64).unwrap_or(0);
        s.renaming = format!(
            "{} devices checked, {} renamed, {} unchanged, {} exempt, {} collisions",
            n("devices"),
            n("renamed"),
            n("unchanged"),
            n("exempt"),
            n("collisions")
        );
    }

    // The order data lives in `custom/<name>/project.json`, a few levels up
    // from `output/custom/<name>/vNN`.
    if let Some(name) = &src.name {
        let mut up = src.dir.parent();
        for _ in 0..5 {
            let Some(dir) = up else { break };
            let candidates = [
                dir.join("custom").join(name).join("project.json"),
                dir.join("project.json"),
            ];
            if let Some(v) = candidates
                .iter()
                .find_map(|c| fs::read_to_string(c).ok())
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
            {
                s.project = text_field(&v, "project");
                s.customer = text_field(&v, "customer");
                s.brand = text_field(&v, "brand");
                s.kind = text_field(&v, "kind");
                break;
            }
            up = dir.parent();
        }
    }
    s
}

impl CraneSummary {
    /// Report notes quoting the configurator, in its own words.
    pub fn notes(&self, src: &CraneSource) -> Vec<String> {
        let mut n = Vec::new();
        let mut head = String::from("Crane configurator output");
        if let Some(name) = &src.name {
            head.push_str(&format!(" \"{name}\""));
        }
        if let Some(v) = src.version {
            head.push_str(&format!(", version {v}"));
        }
        if !src.other_versions.is_empty() {
            let list: Vec<String> = src.other_versions.iter().map(|v| format!("v{v}")).collect();
            head.push_str(&format!(
                " (the newest of {} versions; not read: {})",
                src.other_versions.len() + 1,
                list.join(", ")
            ));
        }
        head.push('.');
        n.push(head);
        if !self.project.is_empty() || !self.customer.is_empty() {
            n.push(format!(
                "Order data from the configurator: project {}, customer {}, {} {}.",
                or_dash(&self.project),
                or_dash(&self.customer),
                self.brand,
                self.kind
            ));
        }
        if !self.verdict.is_empty() {
            n.push(format!(
                "The configurator's own project check says: {} ({}). KoperCAD does not repeat that check.",
                self.verdict,
                self.checks.trim_end_matches('.')
            ));
        }
        if !self.renaming.is_empty() {
            n.push(format!(
                "Device renaming in the configurator: {}.",
                self.renaming
            ));
        }
        n
    }
}

/// What is read exactly and what is inferred, said in the report so nobody
/// has to guess.
const EXACT: &str = "Exact (as the file writes it): device tags, part numbers, terminal texts, cable and core numbers, page numbers and titles, project number and title.";
const INFERRED: &str = "Inferred (EPLAN's rules, rebuilt): which object is a device, terminal or cable, and which pages are circuit diagrams or generated lists. Re-check any of these that matter.";

/// Reads the configurator output at `path` (a file, a version folder, a
/// project folder or `output`): the export itself and what sits beside it.
pub fn read_crane(path: &Path) -> Result<(PxfFile, CraneSource, CraneSummary), CraneError> {
    let src = locate(path)?;
    let f = fs::File::open(&src.epj)
        .map_err(|e| CraneError::Read(src.epj.display().to_string(), e.to_string()))?;
    let file = read_pxf(BufReader::with_capacity(1 << 20, f))?;
    let summary = read_summary(&src);
    Ok((file, src, summary))
}

/// The report notes a crane import adds to the generic PXF ones: the
/// configurator's own words, then what is exact and what is inferred.
pub fn crane_notes(src: &CraneSource, summary: &CraneSummary) -> Vec<String> {
    let mut n = summary.notes(src);
    n.push(EXACT.to_string());
    n.push(INFERRED.to_string());
    n
}

/// [`read_crane`] and the plan for `project`, notes included.
pub fn plan_crane(
    project: &Project,
    path: &Path,
    opts: &PxfOptions,
) -> Result<(PxfPlan, CraneSource, CraneSummary), CraneError> {
    let (file, src, summary) = read_crane(path)?;
    let mut plan = plan_pxf(project, &file, opts);
    for n in crane_notes(&src, &summary) {
        plan.note(n);
    }
    Ok((plan, src, summary))
}

fn or_dash(s: &str) -> &str {
    if s.is_empty() { "-" } else { s }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tree() -> tempfile::TempDir {
        let t = tempfile::tempdir().unwrap();
        for v in ["v9", "v10"] {
            let d = t.path().join("output/custom/demo").join(v);
            fs::create_dir_all(&d).unwrap();
            fs::write(d.join("705-1.epj"), "<x/>").unwrap();
        }
        let v10 = t.path().join("output/custom/demo/v10");
        fs::write(
            v10.join("705-1_Projectcontrole.md"),
            "# x\n\n72 automatische controles: 54 in orde.\n\n**Oordeel: GOED MET OPMERKINGEN**\n",
        )
        .unwrap();
        fs::write(
            v10.join("hernoemen.json"),
            r#"{"summary":{"devices":98,"renamed":3,"unchanged":65,"exempt":30,"collisions":0}}"#,
        )
        .unwrap();
        fs::create_dir_all(t.path().join("custom/demo")).unwrap();
        fs::write(
            t.path().join("custom/demo/project.json"),
            r#"{"project":"705-1","customer":"Acme","brand":"B","kind":"crane"}"#,
        )
        .unwrap();
        t
    }

    #[test]
    fn the_newest_version_wins_whichever_folder_is_given() {
        let t = tree();
        for p in [
            t.path().join("output"),
            t.path().join("output/custom"),
            t.path().join("output/custom/demo"),
            t.path().join("output/custom/demo/v10"),
            t.path().join("output/custom/demo/v10/705-1.epj"),
        ] {
            let s = locate(&p).unwrap();
            assert_eq!(s.version, Some(10), "{p:?}");
            assert_eq!(s.name.as_deref(), Some("demo"));
        }
        let s = locate(&t.path().join("output/custom/demo")).unwrap();
        assert_eq!(s.other_versions, [9]);
    }

    #[test]
    fn an_older_version_can_still_be_asked_for() {
        let t = tree();
        let s = locate(&t.path().join("output/custom/demo/v9")).unwrap();
        assert_eq!(s.version, Some(9));
    }

    #[test]
    fn the_summary_quotes_the_configurator() {
        let t = tree();
        let src = locate(&t.path().join("output")).unwrap();
        let s = read_summary(&src);
        assert_eq!(s.verdict, "GOED MET OPMERKINGEN");
        assert_eq!(s.customer, "Acme");
        let notes = s.notes(&src).join("\n");
        assert!(notes.contains("version 10"), "{notes}");
        assert!(notes.contains("98 devices checked, 3 renamed"), "{notes}");
        assert!(notes.contains("GOED MET OPMERKINGEN"), "{notes}");
    }

    #[test]
    fn nothing_there_is_said_plainly() {
        let t = tempfile::tempdir().unwrap();
        assert!(matches!(locate(t.path()), Err(CraneError::NoEpj(_))));
        assert!(matches!(
            locate(&t.path().join("nope")),
            Err(CraneError::Missing(_))
        ));
    }
}
