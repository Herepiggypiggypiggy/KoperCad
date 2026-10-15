//! The `kopercad-cli` command-line tool (owned by WS-28).
//!
//! Every subcommand opens the project read-only ([`ProjectFile::read`]): it
//! never writes to the project file (`import sheet` writes a new project
//! file only when given `--output`, never over an existing one), takes no lock that outlives the run,
//! and writes nothing but the output path it was given. Edits belong to the
//! app and to Python scripts, which go through the command API.
//!
//! Exit codes are stable so batch files can test them; see [`exit`].

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use novo_checks::{Problem, ProblemKind, Severity, Summary};
use novo_core::{PageId, Project};
use novo_editor::art::LibraryArt;
use novo_pdf::{NoteAnchor, PageSet, PdfOptions, SymbolNote};
use novo_py::ReportFormat;
use novo_py::{comparison_json, group_name, kind_name, page_label, revisions_json};
use novo_reports::CsvOptions;
use novo_revisions::{Comparison, FieldChange, Side, group_of};
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
                  Project files are only read, never changed (`import sheet --output` writes a new one).\n\
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
    /// Revisions: list them, or compare two states of the project.
    #[command(subcommand)]
    Revisions(Revisions),
    /// Bring edited data back into a project.
    #[command(subcommand)]
    Import(Import),
    /// Run the design checks; exits with 1 when there are errors or
    /// warnings (infos such as missing parts do not count).
    Check {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Revisions {
    /// List the issued revisions: index, date, author, description.
    List {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// What changed between two states: a revision index (`B`) or `now`.
    /// Differences are not an error; the exit code is 0 either way.
    Diff {
        file: PathBuf,
        /// The older state: a revision index, or `now`.
        from: String,
        /// The newer state: a revision index, or `now`.
        to: String,
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
    /// Wire list, with the cut length of every wire.
    Wires(ReportArgs),
    /// Cut list: the wires grouped by size and colour, with end allowances.
    Cutlist(ReportArgs),
    /// Terminal diagram: every terminal strip with wires in and out, bridges,
    /// destinations and pages.
    Terminals(ReportArgs),
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
    /// Schematic pages as DXF drawings (millimetres, y up), for CAD
    /// programs and laser or plotter shops.
    Dxf {
        file: PathBuf,
        /// Output: a `.dxf` file for one page, otherwise a directory that
        /// gets one `<page>.dxf` per page.
        #[arg(short, long)]
        output: PathBuf,
        /// Only this page (by name, e.g. `3`); repeat for several. All pages
        /// when left out.
        #[arg(long = "page", value_name = "NAME")]
        pages: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum Import {
    /// Preview an edited spreadsheet (`.xlsx` or CSV from the spreadsheet
    /// export) against a project: every cell that would change, conflicts
    /// with later edits and rows that cannot be used, with line and reason.
    /// Nothing is written unless `--output` names a new project file; the
    /// project given is never changed. Exits with 1 when rows were rejected.
    Sheet {
        /// The project the sheet was exported from.
        project: PathBuf,
        /// The edited `.xlsx` or CSV file.
        sheet: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Write the project with the import applied (one undo step) to
        /// this new `.novo` file. It must not exist yet.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Settle conflicts (cells the project changed since the export):
        /// take the sheet's value or keep the project's. Left unsettled,
        /// conflicts are not applied.
        #[arg(long, value_enum)]
        resolve: Option<Side>,
        /// Leave out one row, as `Devices:5` (sheet name, row); repeat for
        /// several.
        #[arg(long = "skip", value_name = "SHEET:ROW")]
        skip: Vec<String>,
    },
}

/// Which side wins a conflict.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum Side {
    Sheet,
    Project,
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
        Cmd::Export(Export::Cutlist(args)) => export_reports(&args.file, &["cutlist"], &args, out),
        Cmd::Export(Export::Terminals(args)) => {
            export_reports(&args.file, &["terminals"], &args, out)
        }
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
        Cmd::Revisions(Revisions::List { file, json }) => {
            let project = read(&file)?;
            let file_ = ProjectFile::read_only(&file).map_err(|e| read_failure(&file, &e))?;
            let kept = file_.snapshot_revisions().unwrap_or_default();
            let rows: Vec<_> = project
                .revisions()
                .map(|r| (r, kept.contains(&r.id)))
                .collect();
            if json {
                print(out, &format!("{:#}", revisions_json(&project, &kept)))?;
            } else if rows.is_empty() {
                print(out, "No revisions issued yet")?;
            } else {
                for (r, kept) in rows {
                    let note = if kept { "" } else { "  (no kept copy)" };
                    print(
                        out,
                        &format!(
                            "{}  {}  {}  {}{note}",
                            r.index, r.date, r.author, r.description
                        ),
                    )?;
                }
            }
            Ok(exit::OK)
        }
        Cmd::Revisions(Revisions::Diff {
            file,
            from,
            to,
            json,
        }) => {
            let file_ = ProjectFile::read_only(&file).map_err(|e| read_failure(&file, &e))?;
            let project = file_.load().map_err(|e| read_failure(&file, &e))?;
            let side = |name: &str| -> Result<Side, Failure> {
                if name.eq_ignore_ascii_case("now") {
                    return Ok(Side::Now);
                }
                project
                    .revision_by_index(name)
                    .map(|r| Side::Revision(r.id))
                    .ok_or_else(|| {
                        Failure::new(
                            exit::USAGE,
                            format!("no revision `{name}` (use an index from `revisions list`, or `now`)"),
                        )
                    })
            };
            let (a, b) = (side(&from)?, side(&to)?);
            let comparison = novo_revisions::compare(&project, |id| file_.snapshot(id), a, b)
                .map_err(|e| Failure::new(exit::READ, e.to_string()))?;
            if json {
                print(
                    out,
                    &format!("{:#}", comparison_json(&project, &from, &to, &comparison)),
                )?;
            } else {
                for line in comparison_text(&project, &from, &to, &comparison) {
                    print(out, &line)?;
                }
            }
            Ok(exit::OK)
        }
        Cmd::Import(Import::Sheet {
            project,
            sheet,
            json,
            output,
            resolve,
            skip,
        }) => import_sheet(
            &project,
            &sheet,
            json,
            output.as_deref(),
            resolve,
            &skip,
            out,
        ),
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

/// `kopercad-cli import sheet`: previews the sheet, and with `output`
/// writes the imported project to a new file.
fn import_sheet(
    file: &Path,
    sheet: &Path,
    json: bool,
    output: Option<&Path>,
    resolve: Option<Side>,
    skip: &[String],
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    use novo_interop::sheets::{Resolution, plan_import_with, read_file};

    let project = read(file)?;
    let name = sheet
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let bytes = std::fs::read(sheet)
        .map_err(|e| Failure::new(exit::READ, format!("cannot read {}: {e}", sheet.display())))?;
    let book = read_file(&name, &bytes)
        .map_err(|e| Failure::new(exit::READ, format!("cannot import {name}: {e}")))?;
    let mut skipped = Vec::new();
    for spec in skip {
        let parsed = spec.split_once(':').and_then(|(table, line)| {
            let kind = novo_interop::sheets::TableKind::from_name(table)?;
            Some((kind, line.trim().parse::<usize>().ok()?))
        });
        match parsed {
            Some(row) => skipped.push(row),
            None => {
                return Err(Failure::new(
                    exit::USAGE,
                    format!("--skip wants SHEET:ROW such as Devices:5, not {spec:?}"),
                ));
            }
        }
    }
    // Check the output before any work is shown as done.
    if let Some(path) = output
        && (!novo_py::is_project_path(path) || path.exists())
    {
        let why = if path.exists() {
            "already exists; import never replaces a project file"
        } else {
            "is not a .novo project file name"
        };
        return Err(Failure::new(
            exit::WRITE,
            format!("cannot write {}: it {why}", path.display()),
        ));
    }
    let catalog = novo_parts::Catalog::load_default().unwrap_or_default();
    let mut plan = plan_import_with(&project, &book, &catalog);
    match resolve {
        Some(Side::Sheet) => plan.resolve(Resolution::Sheet),
        Some(Side::Project) => plan.resolve(Resolution::Project),
        None => {}
    }
    for index in 0..plan.changes.len() {
        let change = &plan.changes[index];
        if skipped.contains(&(change.table, change.line)) {
            plan.select(index, false);
        }
    }
    for index in 0..plan.new_devices.len() {
        if skipped.contains(&(
            novo_interop::sheets::TableKind::Devices,
            plan.new_devices[index].line,
        )) {
            plan.select_new(index, false);
        }
    }
    let mut written = false;
    if let Some(path) = output
        && let Some(command) = plan
            .command(&project)
            .map_err(|e| Failure::new(exit::USAGE, e.to_string()))?
    {
        let mut doc = novo_core::Document::new(project.clone());
        doc.execute(command)
            .map_err(|e| Failure::new(exit::USAGE, e.to_string()))?;
        ProjectFile::create(path, doc.project()).map_err(|e| {
            Failure::new(exit::WRITE, format!("cannot write {}: {e}", path.display()))
        })?;
        written = true;
    }
    if json {
        let changes: Vec<Value> = plan
            .changes
            .iter()
            .map(|c| {
                json!({
                    "sheet": c.table.name(),
                    "row": c.line,
                    "item": c.item,
                    "column": c.header,
                    "before": c.before,
                    "after": c.after,
                    "conflict": c.conflict.is_some(),
                    "applied": c.selected,
                })
            })
            .collect();
        let new_devices: Vec<Value> = plan
            .new_devices
            .iter()
            .map(|n| {
                json!({
                    "row": n.line,
                    "tag": n.tag.to_string(),
                    "description": n.description,
                    "kind": format!("{:?}", n.kind),
                    "order_number": n.order_number(),
                    "applied": n.selected,
                })
            })
            .collect();
        let rejected: Vec<Value> = plan
            .rejected
            .iter()
            .map(|r| {
                json!({
                    "sheet": r.table.name(),
                    "row": r.line,
                    "item": r.item,
                    "reasons": r.reasons,
                })
            })
            .collect();
        let value = json!({
            "summary": plan.summary(),
            "changes": changes,
            "new_devices": new_devices,
            "rejected": rejected,
            "warnings": plan.warnings,
            "notes": plan.notes,
            "rows_unchanged": plan.rows_unchanged,
            "written": written,
        });
        print(out, &format!("{value:#}"))?;
    } else {
        for c in &plan.changes {
            let mark = match (&c.conflict, c.selected) {
                (Some(_), _) => "conflict",
                (None, true) => "change",
                (None, false) => "skipped",
            };
            let shown = |v: &str| {
                if v.is_empty() {
                    "(empty)".to_owned()
                } else {
                    v.to_owned()
                }
            };
            print(
                out,
                &format!(
                    "{mark}: {} row {} {}: {} = {} -> {}",
                    c.table.name(),
                    c.line,
                    c.item,
                    c.header,
                    shown(&c.before),
                    shown(&c.after)
                ),
            )?;
        }
        for n in &plan.new_devices {
            let mark = if n.selected { "new" } else { "skipped" };
            let order = n
                .order_number()
                .map_or(String::new(), |o| format!(", order number {o}"));
            print(
                out,
                &format!(
                    "{mark}: Devices row {} {}: {:?}{order}, {:?}",
                    n.line, n.tag, n.description, n.kind
                ),
            )?;
        }
        for r in &plan.rejected {
            let item = if r.item.is_empty() {
                String::new()
            } else {
                format!(" ({})", r.item)
            };
            print(
                out,
                &format!(
                    "rejected: {} row {}{item}: {}",
                    r.table.name(),
                    r.line,
                    r.reasons.join("; ")
                ),
            )?;
        }
        for text in &plan.warnings {
            print(out, &format!("warning: {text}"))?;
        }
        for text in &plan.notes {
            print(out, &format!("note: {text}"))?;
        }
        print(out, &plan.summary())?;
        match (output, written) {
            (Some(path), true) => print(out, &format!("wrote {}", path.display()))?,
            (Some(_), false) => print(out, "nothing to apply, no file written")?,
            (None, _) => print(out, "preview only: give --output <new.novo> to apply")?,
        }
    }
    Ok(if plan.rejected.is_empty() {
        exit::OK
    } else {
        exit::PROBLEMS
    })
}

fn read(path: &Path) -> Result<Project, Failure> {
    ProjectFile::read(path).map_err(|e| read_failure(path, &e))
}

fn read_failure(path: &Path, e: &novo_storage::StorageError) -> Failure {
    Failure::new(exit::READ, format!("cannot read {}: {e}", path.display()))
}

/// One field as `name: old -> new`, with the plain-words note after it.
fn field_text(f: &FieldChange) -> String {
    let show = |v: &Option<String>| v.clone().unwrap_or_else(|| "(none)".into());
    let values = format!("{}: {} -> {}", f.name, show(&f.old), show(&f.new));
    match &f.note {
        Some(note) => format!("{values} ({note})"),
        None => values,
    }
}

fn comparison_text(project: &Project, from: &str, to: &str, c: &Comparison) -> Vec<String> {
    if c.is_empty() {
        return vec![format!("No differences between {from} and {to}")];
    }
    let mut lines = vec![format!("{from} -> {to}")];
    let mut rows: Vec<_> = c.rows.iter().collect();
    // Stable, so rows keep their page order within a group.
    rows.sort_by_key(|r| group_of(r.id));
    let mut group = None;
    for r in rows {
        let g = group_of(r.id);
        if group != Some(g) {
            lines.push(format!("{}:", group_name(g)));
            group = Some(g);
        }
        let page = page_label(project, r.page).map_or_else(String::new, |p| format!(" (page {p})"));
        lines.push(format!("  {} {}{page}", kind_name(&r.kind), r.label));
        for f in &r.fields {
            lines.push(format!("    {}", field_text(f)));
        }
    }
    for f in &c.info {
        lines.push(format!("project info {}", field_text(f)));
    }
    for s in &c.sheets {
        lines.push(format!("sheet {} ({}): {:?}", s.name, s.report, s.kind));
    }
    lines
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

/// The pages to export (all when `names` is empty), as (name, id).
fn select_pages(project: &Project, names: &[String]) -> Result<Vec<(String, PageId)>, Failure> {
    if names.is_empty() {
        return Ok(project.pages().map(|p| (p.name.clone(), p.id)).collect());
    }
    names
        .iter()
        .map(|name| {
            project
                .pages()
                .find(|p| &p.name == name)
                .map(|p| (p.name.clone(), p.id))
                .ok_or_else(|| Failure::new(exit::USAGE, format!("there is no page `{name}`")))
        })
        .collect()
}

/// A page name as a file name: path separators and other characters
/// Windows refuses are replaced.
fn file_stem(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| {
            if "\\/:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    if stem.trim().is_empty() {
        "page".into()
    } else {
        stem
    }
}

/// Writes the pages as DXF: one `.dxf` file when `output` ends in `.dxf`
/// (exactly one page), otherwise one file per page in the directory.
fn export_dxf(
    project: &Project,
    input: &Path,
    output: &Path,
    names: &[String],
) -> Result<(), Failure> {
    let pages = select_pages(project, names)?;
    let library = novo_py::library().map_err(|e| Failure::new(exit::UNAVAILABLE, e.to_string()))?;
    let style = novo_pdf::print_style();
    let art = LibraryArt::new(library, style.symbol);
    let render = |id: PageId| {
        let drawing = novo_interop::bridge::page_drawing(project, id, &art, style)
            .ok_or_else(|| Failure::new(exit::USAGE, "the page does not exist"))?;
        Ok::<_, Failure>(novo_interop::dxf::write_dxf(&drawing))
    };
    let single = output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("dxf"));
    if single {
        let [(_, id)] = pages.as_slice() else {
            return Err(Failure::new(
                exit::USAGE,
                "a .dxf output holds one page: pick it with --page, or give a directory",
            ));
        };
        return write_output(input, output, render(*id)?.as_bytes());
    }
    std::fs::create_dir_all(output).map_err(|e| {
        Failure::new(
            exit::WRITE,
            format!("cannot create {}: {e}", output.display()),
        )
    })?;
    let mut used = std::collections::HashSet::new();
    for (name, id) in pages {
        let mut stem = file_stem(&name);
        while !used.insert(stem.to_lowercase()) {
            stem.push('_');
        }
        write_output(
            input,
            &output.join(format!("{stem}.dxf")),
            render(id)?.as_bytes(),
        )?;
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
