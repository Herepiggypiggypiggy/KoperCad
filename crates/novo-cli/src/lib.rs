//! The `kopercad-cli` command-line tool (owned by WS-28).
//!
//! Every subcommand opens the project read-only ([`ProjectFile::read`]): it
//! never writes to the project file, takes no lock that outlives the run,
//! and writes nothing but the output path it was given. Edits belong to the
//! app and to Python scripts, which go through the command API.
//!
//! Exit codes are stable so batch files can test them; see [`exit`].

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use novo_checks::{Problem, ProblemKind, Severity, Summary};
use novo_core::Project;
use novo_editor::art::LibraryArt;
use novo_pdf::{NoteAnchor, PageSet, PdfOptions, SymbolNote};
use novo_py::ReportFormat;
use novo_reports::CsvOptions;
use novo_storage::ProjectFile;
use serde_json::{Value, json};

/// Process exit codes.
pub mod exit {
    /// Done.
    pub const OK: i32 = 0;
    /// `kopercad-cli check` found problems.
    pub const PROBLEMS: i32 = 1;
    /// The command line was not understood.
    pub const USAGE: i32 = 2;
    /// The project file could not be read.
    pub const READ: i32 = 3;
    /// The output could not be written.
    pub const WRITE: i32 = 4;
    /// The subcommand is not available in this build (kept for scripts that
    /// test for it; nothing returns it at the moment but a symbol library
    /// that fails to load).
    pub const UNAVAILABLE: i32 = 5;
}

#[derive(Debug, Parser)]
#[command(
    name = "kopercad-cli",
    version,
    about = "KoperCAD command-line tool: inspect, check and export project files",
    long_about = "KoperCAD command-line tool: inspect, check and export project files.\n\n\
                  Project files are only read, never changed.\n\
                  Exit codes: 0 done, 1 check found problems, 2 bad command line, \
                  3 project not readable, 4 output not writable, 5 not available in this build.\n\
                  An export is never written over a project file."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Show a summary of a project file.
    Info {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Export a project.
    #[command(subcommand)]
    Export(Export),
    /// Run the design checks; exits with 1 when there are errors or
    /// warnings (infos such as missing parts do not count).
    Check {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

/// Where a report goes.
#[derive(Debug, clap::Args)]
struct ReportArgs {
    file: PathBuf,
    /// Output file, `.xlsx` or `.csv`; CSV on standard output when left out.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// CSV field separator; `;` suits Excel in locales with a decimal comma.
    #[arg(long, default_value_t = ',')]
    delimiter: char,
}

#[derive(Debug, Subcommand)]
enum Export {
    /// Bill of materials: every part with its quantity, plus devices
    /// without a part.
    Bom(ReportArgs),
    /// Device list.
    Devices(ReportArgs),
    /// Wire list.
    Wires(ReportArgs),
    /// Table of contents (the pages).
    Toc(ReportArgs),
    /// Every report in one workbook, one sheet each.
    Reports {
        file: PathBuf,
        /// Output file (.xlsx).
        #[arg(short, long)]
        output: PathBuf,
    },
    /// The whole project as diffable JSON.
    Json {
        file: PathBuf,
        /// Output file; standard output when left out.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// The schematic pages as one vector PDF, with bookmarks and
    /// cross-reference links.
    Pdf {
        file: PathBuf,
        /// Output file.
        #[arg(short, long)]
        output: PathBuf,
        /// Only this page (by name, e.g. `3`); repeat for several. All pages
        /// when left out.
        #[arg(long = "page", value_name = "NAME")]
        pages: Vec<String>,
    },
    /// Schematic pages as DXF, millimetres, for mechanical colleagues. One
    /// page goes to the output file; several (or all) go into the output
    /// folder, one `<project> - <page>.dxf` each.
    Dxf {
        file: PathBuf,
        /// Output file (one page) or folder (several pages).
        #[arg(short, long)]
        output: PathBuf,
        /// Only this page (by name, e.g. `3`); repeat for several. All pages
        /// when left out.
        #[arg(long = "page", value_name = "NAME")]
        pages: Vec<String>,
    },
}

/// A failure with its exit code.
struct Failure {
    code: i32,
    message: String,
}

impl Failure {
    fn new(code: i32, message: impl Into<String>) -> Failure {
        Failure {
            code,
            message: message.into(),
        }
    }
}

/// Runs the tool with `args` (including the program name) and returns the
/// exit code. Normal output goes to `out`, messages to `err`.
pub fn run<I, T>(args: I, out: &mut dyn Write, err: &mut dyn Write) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(e) => {
            // Plain text: batch logs and old consoles show escape codes raw.
            let text = e.render().to_string();
            return if e.use_stderr() {
                let _ = write!(err, "{text}");
                exit::USAGE
            } else {
                let _ = write!(out, "{text}");
                exit::OK
            };
        }
    };
    match dispatch(cli.command, out) {
        Ok(code) => code,
        Err(failure) => {
            let _ = writeln!(err, "novo: {}", failure.message);
            failure.code
        }
    }
}

fn dispatch(command: Cmd, out: &mut dyn Write) -> Result<i32, Failure> {
    match command {
        Cmd::Info { file, json } => {
            let project = read(&file)?;
            let info = novo_py::info_json(&project, Some(&file));
            if json {
                let mut value = info;
                value["page_list"] = novo_py::pages_json(&project);
                print(out, &format!("{value:#}"))?;
            } else {
                print(out, &info_text(&project, &file))?;
            }
            Ok(exit::OK)
        }
        Cmd::Export(Export::Json { file, output }) => {
            let project = read(&file)?;
            let text = project.to_json();
            match output {
                Some(path) => write_output(&file, &path, text.as_bytes())?,
                None => print(out, &text)?,
            }
            Ok(exit::OK)
        }
        Cmd::Export(Export::Bom(args)) => export_reports(&args.file, &["bom"], &args, out),
        Cmd::Export(Export::Devices(args)) => export_reports(&args.file, &["devices"], &args, out),
        Cmd::Export(Export::Wires(args)) => export_reports(&args.file, &["wires"], &args, out),
        Cmd::Export(Export::Toc(args)) => export_reports(&args.file, &["toc"], &args, out),
        Cmd::Export(Export::Reports { file, output }) => {
            let args = ReportArgs {
                file,
                output: Some(output),
                delimiter: ',',
            };
            if !matches!(
                args.output.as_deref().map(ReportFormat::from_path),
                Some(Ok(ReportFormat::Xlsx))
            ) {
                return Err(Failure::new(
                    exit::USAGE,
                    "`kopercad-cli export reports` writes one workbook: give an .xlsx output",
                ));
            }
            export_reports(&args.file, &[], &args, out)
        }
        Cmd::Export(Export::Pdf {
            file,
            output,
            pages,
        }) => {
            let project = read(&file)?;
            let bytes = pdf(&project, &pages)?;
            write_output(&file, &output, &bytes)?;
            Ok(exit::OK)
        }
        Cmd::Export(Export::Dxf {
            file,
            output,
            pages,
        }) => {
            let project = read(&file)?;
            export_dxf(&project, &file, &output, &pages)?;
            Ok(exit::OK)
        }
        Cmd::Check { file, json } => {
            let project = read(&file)?;
            let problems = novo_checks::check(&project);
            let summary = Summary::of(&problems);
            if json {
                let list: Vec<Value> = problems.iter().map(|p| problem_json(&project, p)).collect();
                print(out, &format!("{:#}", Value::Array(list)))?;
            } else {
                for p in &problems {
                    print(out, &problem_text(&project, p))?;
                }
                print(out, &summary.pill_text())?;
            }
            Ok(if summary.is_clean() {
                exit::OK
            } else {
                exit::PROBLEMS
            })
        }
    }
}

fn read(path: &Path) -> Result<Project, Failure> {
    ProjectFile::read(path)
        .map_err(|e| Failure::new(exit::READ, format!("cannot read {}: {e}", path.display())))
}

fn print(out: &mut dyn Write, text: &str) -> Result<(), Failure> {
    writeln!(out, "{text}")
        .map_err(|e| Failure::new(exit::WRITE, format!("cannot write output: {e}")))
}

/// Writes an export through the shared guard: never over the input or any
/// other project or recovery file, atomically.
fn write_output(input: &Path, output: &Path, bytes: &[u8]) -> Result<(), Failure> {
    novo_py::write_export(output, bytes, Some(input))
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))
}

/// Writes reports (`ids`; every report when empty) to `output` as .xlsx or
/// .csv, or one report as CSV to `out` when there is no output path.
fn export_reports(
    file: &Path,
    ids: &[&str],
    args: &ReportArgs,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    let project = read(file)?;
    let usage = |e: novo_py::ScriptError| Failure::new(exit::USAGE, e.to_string());
    let csv = CsvOptions {
        delimiter: args.delimiter,
        ..CsvOptions::default()
    };
    match &args.output {
        Some(path) => {
            if novo_py::is_project_path(path) {
                // Say why before complaining about the file type.
                return write_output(file, path, &[]).map(|()| exit::OK);
            }
            let format = match ReportFormat::from_path(path).map_err(usage)? {
                ReportFormat::Csv(_) => ReportFormat::Csv(csv),
                xlsx => xlsx,
            };
            let bytes = novo_py::report_bytes(&project, ids, format).map_err(usage)?;
            write_output(file, path, &bytes)?;
        }
        None => {
            // On a terminal or in a pipe a byte order mark is only noise.
            let format = ReportFormat::Csv(CsvOptions { bom: false, ..csv });
            let bytes = novo_py::report_bytes(&project, ids, format).map_err(usage)?;
            out.write_all(&bytes)
                .map_err(|e| Failure::new(exit::WRITE, format!("cannot write output: {e}")))?;
        }
    }
    Ok(exit::OK)
}

/// The PDF of the given pages (all when empty), drawn with the symbol
/// library's artwork.
fn pdf(project: &Project, pages: &[String]) -> Result<Vec<u8>, Failure> {
    let mut options = PdfOptions::default();
    if !pages.is_empty() {
        let mut ids = Vec::new();
        for name in pages {
            let page = project
                .pages()
                .find(|p| &p.name == name)
                .ok_or_else(|| Failure::new(exit::USAGE, format!("there is no page `{name}`")))?;
            ids.push(page.id);
        }
        options.pages = PageSet::Pages(ids);
    }
    let library = novo_py::library().map_err(|e| Failure::new(exit::UNAVAILABLE, e.to_string()))?;
    let art = LibraryArt::new(library, options.style.symbol);
    // The cross references the canvas shows, printed the same way.
    let xref = novo_xref::XrefIndex::build(project, library);
    options.notes = project
        .pages()
        .map(|p| {
            let notes = xref.labels_on_page(p.id).into_iter().map(|l| SymbolNote {
                symbol: l.symbol,
                lines: l.lines,
                warning: l.warning,
                anchor: match l.anchor {
                    novo_xref::LabelAnchor::Below => NoteAnchor::Below,
                    novo_xref::LabelAnchor::Right => NoteAnchor::Right,
                },
            });
            (p.id, notes.collect())
        })
        .collect();
    novo_pdf::export_pdf_with_art(project, &options, &art)
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))
}

/// Writes the DXF of the given pages (all when empty): to `output` itself
/// for exactly one page, else one file per page inside the folder `output`.
fn export_dxf(
    project: &Project,
    input: &Path,
    output: &Path,
    names: &[String],
) -> Result<(), Failure> {
    let mut pages = Vec::new();
    if names.is_empty() {
        pages.extend(project.pages_in_order());
    } else {
        for name in names {
            let page = project
                .pages()
                .find(|p| &p.name == name)
                .ok_or_else(|| Failure::new(exit::USAGE, format!("there is no page `{name}`")))?;
            if !pages.iter().any(|p| p.id == page.id) {
                pages.push(page);
            }
        }
    }
    if pages.is_empty() {
        return Err(Failure::new(exit::USAGE, "the project has no pages"));
    }
    let library = novo_py::library().map_err(|e| Failure::new(exit::UNAVAILABLE, e.to_string()))?;
    let style = novo_pdf::print_style();
    let art = LibraryArt::new(library, style.symbol);
    let stem = input
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let single = pages.len() == 1;
    for page in pages {
        let text = novo_interop::bridge::page_dxf(project, page.id, &art, style)
            .ok_or_else(|| Failure::new(exit::WRITE, format!("page `{}` is gone", page.name)))?;
        let path = if single {
            output.to_path_buf()
        } else {
            output.join(novo_interop::bridge::page_file_name(&stem, &page.name))
        };
        if !single {
            std::fs::create_dir_all(output).map_err(|e| {
                Failure::new(
                    exit::WRITE,
                    format!("cannot create {}: {e}", output.display()),
                )
            })?;
        }
        write_output(input, &path, text.as_bytes())?;
    }
    Ok(())
}

fn info_text(p: &Project, path: &Path) -> String {
    let mut lines = vec![
        format!("Project:     {}", p.info().name),
        format!("File:        {}", path.display()),
    ];
    if !p.info().description.is_empty() {
        lines.push(format!("Description: {}", p.info().description));
    }
    let pages = p.pages_in_order();
    lines.push(format!("Pages:       {}", pages.len()));
    for page in pages {
        if page.title.is_empty() {
            lines.push(format!("  {}", page.name));
        } else {
            lines.push(format!("  {}  {}", page.name, page.title));
        }
    }
    lines.push(format!("Devices:     {}", p.devices().count()));
    lines.push(format!("Symbols:     {}", p.symbols().count()));
    lines.push(format!("Wires:       {}", p.wires().count()));
    lines.push(format!("Nets:        {}", p.netlist().nets().len()));
    lines.push(format!("Potentials:  {}", p.potentials().count()));
    lines.push(format!("Terminals:   {}", p.terminals().count()));
    lines.push(format!("Cables:      {}", p.cables().count()));
    lines.push(format!("Parts:       {}", p.parts().count()));
    lines.join("\n")
}

fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Info => "info",
    }
}

/// `DuplicateTag` as `duplicate_tag`, a stable key for scripts.
fn kind_key(kind: ProblemKind) -> String {
    let mut key = String::new();
    for (i, c) in format!("{kind:?}").chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            key.push('_');
        }
        key.push(c.to_ascii_lowercase());
    }
    key
}

fn page_name(project: &Project, problem: &Problem) -> Option<String> {
    problem
        .page
        .and_then(|id| project.page(id))
        .map(|p| p.name.clone())
}

fn problem_text(project: &Project, problem: &Problem) -> String {
    let at = page_name(project, problem)
        .map(|name| format!(" (page {name})"))
        .unwrap_or_default();
    format!(
        "{:<8}{}: {}{at}",
        severity_name(problem.severity),
        problem.kind.label(),
        problem.message
    )
}

fn problem_json(project: &Project, problem: &Problem) -> Value {
    json!({
        "severity": severity_name(problem.severity),
        "kind": kind_key(problem.kind),
        "label": problem.kind.label(),
        "message": problem.message,
        "page": page_name(project, problem),
        "at": problem.at,
    })
}
