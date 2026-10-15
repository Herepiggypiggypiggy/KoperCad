//! Report and file exports shared by the Python API and the `kopercad-cli` tool:
//! reports (BOM, device list, wire list, contents) as Excel or CSV, and the
//! one way exports reach the disk.

use std::io::Read;
use std::path::{Path, PathBuf};

use novo_core::Project;
use novo_reports::{CsvOptions, ReportOutput};
use novo_storage::{AUTOSAVE_SUFFIX, PROJECT_EXTENSION};
use serde_json::{Value, json};

use crate::{Result, ScriptError};

/// A report file format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ReportFormat {
    /// One sheet per report; numbers stay numbers.
    Xlsx,
    /// One report as CSV.
    Csv(CsvOptions),
}

impl ReportFormat {
    /// The format a file name asks for: `.xlsx` or `.csv`.
    pub fn from_path(path: &Path) -> Result<ReportFormat> {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_ascii_lowercase());
        match ext.as_deref() {
            Some("xlsx") => Ok(ReportFormat::Xlsx),
            Some("csv") => Ok(ReportFormat::Csv(CsvOptions::default())),
            _ => Err(ScriptError::UnknownValue {
                what: "report file type (use .xlsx or .csv)",
                value: path.display().to_string(),
            }),
        }
    }
}

/// Ids of the built-in reports, in export order: `toc`, `bom`, `devices`,
/// `wires`.
pub fn report_ids() -> Vec<String> {
    novo_reports::standard_reports()
        .iter()
        .map(|r| r.id().to_owned())
        .collect()
}

/// Generates one built-in report.
pub fn generate(project: &Project, id: &str) -> Result<ReportOutput> {
    novo_reports::standard_reports()
        .into_iter()
        .find(|r| r.id() == id)
        .map(|r| r.generate(project))
        .ok_or_else(|| ScriptError::UnknownValue {
            what: "report (use toc, bom, devices or wires)",
            value: id.into(),
        })
}

/// A report as plain data: its sections, each with column headers and rows
/// of cell text.
pub fn report_json(report: &ReportOutput) -> Value {
    json!({
        "id": report.report,
        "title": report.title,
        "sections": report.sections.iter().map(|s| json!({
            "key": s.key,
            "title": s.title,
            "columns": s.columns.iter().map(|c| c.header.as_str()).collect::<Vec<_>>(),
            "rows": s.rows.iter()
                .map(|r| r.cells.iter().map(|c| c.to_string()).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
    })
}

/// The given reports (every built-in one when `ids` is empty) as a file.
/// CSV holds exactly one report.
pub fn report_bytes(project: &Project, ids: &[&str], format: ReportFormat) -> Result<Vec<u8>> {
    let reports: Vec<ReportOutput> = if ids.is_empty() {
        novo_reports::generate_all(project)
    } else {
        ids.iter()
            .map(|id| generate(project, id))
            .collect::<Result<_>>()?
    };
    match format {
        ReportFormat::Xlsx => {
            let refs: Vec<&ReportOutput> = reports.iter().collect();
            novo_reports::xlsx_bytes(&refs).map_err(|e| ScriptError::Export(e.to_string()))
        }
        ReportFormat::Csv(options) => match reports.as_slice() {
            [one] => Ok(novo_reports::to_csv(one, &options).into_bytes()),
            _ => Err(ScriptError::UnknownValue {
                what: "CSV export (CSV holds one report; use .xlsx for several)",
                value: ids.join(", "),
            }),
        },
    }
}

/// The first bytes of every SQLite database, so of every project file.
const SQLITE_MAGIC: &[u8] = b"SQLite format 3\0";

/// Whether `path` is, or looks like, a project or recovery file: by name
/// (`*.novo`, `*.novo.autosave`) or, for an existing file, by content.
pub fn is_project_path(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let ext = format!(".{PROJECT_EXTENSION}");
    if name.ends_with(&ext) || name.ends_with(&format!("{ext}{AUTOSAVE_SUFFIX}")) {
        return true;
    }
    let mut head = [0u8; 16];
    std::fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut head))
        .is_ok_and(|()| head == SQLITE_MAGIC)
}

/// Writes an export to `output`, never over a project or recovery file
/// (nor over `source`, the project it came from). The bytes go to a
/// temporary file next to `output` that is moved into place only once
/// complete, so a failed write leaves an earlier export whole.
pub fn write_export(output: &Path, bytes: &[u8], source: Option<&Path>) -> Result<()> {
    let same_as_source = source.is_some_and(|source| {
        matches!(
            (source.canonicalize(), output.canonicalize()),
            (Ok(a), Ok(b)) if a == b
        )
    });
    if same_as_source || is_project_path(output) {
        return Err(ScriptError::ProjectFileTarget(output.to_path_buf()));
    }
    let mut temp = output.as_os_str().to_owned();
    temp.push(".novo-tmp");
    let temp = PathBuf::from(temp);
    let written = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, output));
    if let Err(e) = written {
        let _ = std::fs::remove_file(&temp);
        return Err(ScriptError::Io {
            path: output.to_path_buf(),
            source: e,
        });
    }
    Ok(())
}
