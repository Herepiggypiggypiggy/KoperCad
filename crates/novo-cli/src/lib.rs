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
    /// Revisions: list them, or compare two states of the project.
    #[command(subcommand)]
    Revisions(Revisions),
    /// Convert a DWG to DXF with the free converter on this computer
    /// (LibreDWG's dwg2dxf or the ODA File Converter), ready for the DXF
    /// import. KoperCAD never bundles either one.
    ConvertDwg {
        /// The DWG file.
        file: PathBuf,
        /// Output `.dxf` file.
        #[arg(short, long)]
        output: PathBuf,
        /// The converter program, when it is not found by itself.
        #[arg(long, value_name = "PROGRAM")]
        converter: Option<PathBuf>,
    },
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
    /// Cable schedule: one row per cable core with its two ends.
    Cables(ReportArgs),
    /// Plug connection lists: every pin of every plug with its wire.
    Plugs(ReportArgs),
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
    /// The cabinet as a STEP assembly (AP214, millimetres) for mechanical
    /// CAD: housing, mounting plate, rails, ducts, devices and door, each a
    /// named, coloured solid.
    Step {
        file: PathBuf,
        /// Output: a `.stp` file for one enclosure, otherwise a directory
        /// that gets one `<tag>.stp` per enclosure.
        #[arg(short, long)]
        output: PathBuf,
        /// Only this enclosure (its tag, e.g. `+CAB1`); all with a mounting
        /// plate when left out.
        #[arg(long, value_name = "TAG")]
        enclosure: Option<String>,
    },
    /// The plant (structure, devices, terminals, cables, nets) as
    /// AutomationML (IEC 62714, CAEX 2.15).
    Aml {
        file: PathBuf,
        /// Output file (`.aml`).
        #[arg(short, long)]
        output: PathBuf,
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
        Cmd::Export(Export::Cables(args)) => {
            export_reports(&args.file, &["cableschedule"], &args, out)
        }
        Cmd::Export(Export::Plugs(args)) => export_reports(&args.file, &["plugs"], &args, out),
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
        Cmd::Export(Export::Step {
            file,
            output,
            enclosure,
        }) => {
            let project = read(&file)?;
            export_step(&project, &file, &output, enclosure.as_deref(), out)?;
            Ok(exit::OK)
        }
        Cmd::Export(Export::Aml { file, output }) => {
            let project = read(&file)?;
            let export = novo_interop::aml::export_project(
                &project,
                &novo_interop::aml::AmlOptions {
                    created: novo_interop::step::iso_utc(now_secs()),
                    ..Default::default()
                },
            );
            write_output(&file, &output, export.text.as_bytes())?;
            print(out, &export.report.text())?;
            Ok(exit::OK)
        }
        Cmd::ConvertDwg {
            file,
            output,
            converter,
        } => {
            let Some(conv) = novo_interop::dwg::find_installed(converter.as_deref()) else {
                return Err(Failure::new(
                    exit::UNAVAILABLE,
                    format!("no DWG converter found; {}", novo_interop::dwg::guidance()),
                ));
            };
            let work = std::env::temp_dir().join(format!("kopercad-dwg-{}", std::process::id()));
            let result = conv
                .convert(&file, &work, std::time::Duration::from_secs(180))
                .map_err(|e| Failure::new(exit::READ, e.to_string()))
                .and_then(|dxf| {
                    let bytes = std::fs::read(&dxf)
                        .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", dxf.display())))?;
                    write_output(&file, &output, &bytes)
                });
            let _ = std::fs::remove_dir_all(&work);
            result?;
            print(out, &format!("Converted with {}", conv.kind.label()))?;
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

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// STEP files of the project's enclosures: one `.stp` file, or a directory
/// with one file per enclosure.
fn export_step(
    project: &Project,
    input: &Path,
    output: &Path,
    only: Option<&str>,
    out: &mut dyn Write,
) -> Result<(), Failure> {
    let options = novo_interop::step::StepOptions {
        created: novo_interop::step::iso_utc(now_secs()),
        ..Default::default()
    };
    let mut files = novo_interop::step::export_all(project, &options);
    if let Some(tag) = only {
        let wanted = novo_core::DeviceTag::parse(tag)
            .map_err(|e| Failure::new(exit::USAGE, format!("--enclosure {tag}: {e}")))?
            .to_string();
        files.retain(|f| f.tag == wanted);
    }
    if files.is_empty() {
        return Err(Failure::new(
            exit::USAGE,
            "no enclosure with a mounting plate to export (set its plate size first)",
        ));
    }
    let is_file = output
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("stp") || e.eq_ignore_ascii_case("step"));
    if is_file {
        let [one] = files.as_slice() else {
            return Err(Failure::new(
                exit::USAGE,
                "several enclosures: give a directory as output, or pick one with --enclosure",
            ));
        };
        write_output(input, output, one.export.text.as_bytes())?;
        print(out, &one.export.report.text())?;
    } else {
        std::fs::create_dir_all(output).map_err(|e| {
            Failure::new(
                exit::WRITE,
                format!("cannot create {}: {e}", output.display()),
            )
        })?;
        for f in &files {
            let name = format!("{}.stp", file_stem(&f.tag));
            write_output(input, &output.join(name), f.export.text.as_bytes())?;
        }
        print(out, &format!("{} STEP files written", files.len()))?;
    }
    Ok(())
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
