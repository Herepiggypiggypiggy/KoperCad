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
use novo_pdf::{NoteAnchor, NoteDensity, NoteGlyph, NoteRow, PageSet, PdfOptions, SymbolNote};
use novo_py::ReportFormat;
use novo_py::{comparison_json, group_name, kind_name, page_label, revisions_json};
use novo_reports::CsvOptions;
use novo_revisions::{Comparison, FieldChange, Side, group_of};
use novo_storage::ProjectFile;
use serde_json::{Value, json};

mod eplan;
mod macros;
mod parts;
mod testuser;

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
    /// The macro library: list, export, import, install the starter set.
    #[command(subcommand)]
    Macros(macros::Macros),
    /// The parts catalog: import every downloaded catalog file in one folder.
    #[command(subcommand)]
    Parts(parts::Parts),
    /// The user-run "Export from EPLAN" helper: write the script, check an
    /// export.
    #[command(subcommand)]
    Eplan(eplan::Eplan),
    /// An MCP server that lets an AI assistant use the real KoperCAD window
    /// (screenshots, mouse, keyboard) as a first-time user. See
    /// docs/testing/visual-test-user.md.
    TestUser(testuser::TestUser),
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
    /// The commissioning report: progress per cabinet and page, every
    /// check, the punch list and a sign-off block. `.pdf`, `.csv` or
    /// `.xlsx` by the output's extension; CSV on standard output when left
    /// out.
    CommissioningReport {
        file: PathBuf,
        /// Output file.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// CSV field separator; `;` suits Excel in locales with a decimal
        /// comma.
        #[arg(long, default_value_t = ',')]
        delimiter: char,
        /// Language of titles and headers (`en`, `nl`); the project's
        /// report language when left out.
        #[arg(long, value_name = "CODE")]
        language: Option<String>,
    },
    /// Run the design checks; exits with 1 when there are errors or
    /// warnings (infos such as missing parts do not count).
    Check {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Judge the circuits of a supply described in a JSON file: cable
    /// rating, voltage drop, breaking capacity, short-circuit withstand,
    /// disconnection and protective conductor, each Pass, Fail or Unknown.
    /// Exits with 1 when a circuit fails (unknown does not fail).
    Calc {
        /// The supply and circuits as JSON (see the novo-checks `calc::spec`
        /// documentation).
        file: PathBuf,
        /// Cable tables from your licensed copy of IEC 60364-5-52 (CSV).
        #[arg(long)]
        tables: Option<PathBuf>,
        /// Maker selectivity and back-up tables (CSV).
        #[arg(long)]
        pairs: Option<PathBuf>,
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
    /// `,` when left out, except for `export wirecut` into a file, which
    /// writes `;` like File > Export wire cut in the app.
    #[arg(long)]
    delimiter: Option<char>,
    /// Language of titles and headers (`en`, `nl`); the project's report
    /// language when left out.
    #[arg(long, value_name = "CODE")]
    language: Option<String>,
}

/// Which labels, and in which layout.
#[derive(Debug, clap::Args)]
struct LabelArgs {
    file: PathBuf,
    /// Output file (.csv); standard output when left out.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Which labels: wires, terminals, devices or cables. Repeat for
    /// several; all when left out.
    #[arg(short, long = "kind")]
    kinds: Vec<String>,
    /// CSV layout: `generic` (header row, for any software) or `brother`
    /// (P-touch Editor database: no header, tab separated). Use one kind
    /// per file with `brother`.
    #[arg(long, default_value = "generic")]
    layout: String,
}

#[derive(Debug, Subcommand)]
enum Export {
    /// Labels for a label printer: wire, terminal, device and cable labels.
    Labels(LabelArgs),
    /// Bill of materials: every part with its quantity, plus devices
    /// without a part.
    Bom(ReportArgs),
    /// Device list.
    Devices(ReportArgs),
    /// Wire list, with the cut length of every wire.
    Wires(ReportArgs),
    /// Cut list: the wires grouped by size and colour, with end allowances.
    Cutlist(ReportArgs),
    /// Wire production: the wires to cut in machine order with end
    /// processing and labels. CSV is the plain job file (one row per
    /// wire); .xlsx adds the stock, ferrule and duct fill sheets.
    Wirecut(ReportArgs),
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
    /// The project as an EPLAN PXF file (`.epj`) for EPLAN 2024 to import.
    /// Devices and terminals on drawn symbols, pages, cables and cores;
    /// wires and texts are counted as not written. Never opened in a real
    /// EPLAN yet: import it into a test project first.
    Eplan {
        file: PathBuf,
        /// Output file (.epj); never replaces an existing file.
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
    /// The mounting plate drawing of an enclosure as a PDF: plate, door
    /// sheet when devices need one, and the drill table.
    Plate {
        file: PathBuf,
        /// Output file (.pdf).
        #[arg(short, long)]
        output: PathBuf,
        /// The enclosure by tag (e.g. `+CAB1`); not needed when the project
        /// has exactly one enclosure with a plate.
        #[arg(long, value_name = "TAG")]
        enclosure: Option<String>,
    },
    /// The PLC I/O as the file a PLC programming tool imports: one file per
    /// station for `plcopen`, `tia` and `studio5000`, one table for `csv`.
    Plc {
        file: PathBuf,
        /// `csv`, `plcopen` (CODESYS and compatible), `tia` (TIA Portal
        /// tag table, .xlsx), `studio5000` (tag CSV) or `hardware` (the
        /// address plan).
        #[arg(long, default_value = "csv")]
        format: String,
        /// Directory to write into; it is created when missing.
        #[arg(short, long)]
        output: PathBuf,
        /// Also write a tag for every spare channel.
        #[arg(long)]
        spares: bool,
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
        resolve: Option<ResolveSide>,
        /// Leave out one row, as `Devices:5` (sheet name, row); repeat for
        /// several.
        #[arg(long = "skip", value_name = "SHEET:ROW")]
        skip: Vec<String>,
    },
    /// Preview an EPLAN project export (`.epj`) or page macro (`.emp`, both
    /// PXF) as a KoperCAD import: what would be added, what already exists
    /// and what is not carried, counted. Nothing is written unless
    /// `--output` names a new project file; the project given is never
    /// changed.
    Eplan {
        /// The PXF file.
        pxf: PathBuf,
        /// Add to this project; a new empty project when left out.
        #[arg(long)]
        project: Option<PathBuf>,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Write the project with the import applied (one undo step) to
        /// this new `.novo` file. It must not exist yet.
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Language of titles and type texts, e.g. `en_US`.
        #[arg(long, default_value = "en_US")]
        lang: String,
    },
}

/// Which side wins a conflict.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum ResolveSide {
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
        Cmd::Macros(cmd) => macros::run(cmd, out),
        Cmd::Eplan(cmd) => eplan::run(cmd, out),
        Cmd::TestUser(opts) => testuser::run(opts, out),
        Cmd::Parts(cmd) => parts::run(cmd, out),
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
        Cmd::CommissioningReport {
            file,
            output,
            delimiter,
            language,
        } => commissioning_report(
            &file,
            output.as_deref(),
            delimiter,
            language.as_deref(),
            out,
        ),
        Cmd::Export(Export::Eplan { file, output }) => {
            if output.exists() {
                return Err(Failure::new(
                    exit::WRITE,
                    format!(
                        "cannot write {}: it already exists; export never replaces a file",
                        output.display()
                    ),
                ));
            }
            let project = read(&file)?;
            let export = novo_interop::eplan::pxf::write_pxf(
                &project,
                &novo_interop::eplan::pxf::PxfWriteOptions::default(),
            );
            std::fs::write(&output, export.text).map_err(|e| {
                Failure::new(
                    exit::WRITE,
                    format!("cannot write {}: {e}", output.display()),
                )
            })?;
            print(out, &export.report.text())?;
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
        Cmd::Export(Export::Labels(args)) => export_labels(&args, out),
        Cmd::Export(Export::Bom(args)) => export_reports(&args.file, &["bom"], &args, out),
        Cmd::Export(Export::Devices(args)) => export_reports(&args.file, &["devices"], &args, out),
        Cmd::Export(Export::Wires(args)) => export_reports(&args.file, &["wires"], &args, out),
        Cmd::Export(Export::Cutlist(args)) => export_reports(&args.file, &["cutlist"], &args, out),
        Cmd::Export(Export::Wirecut(args)) => export_wirecut(&args, out),
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
                delimiter: Some(','),
                language: None,
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
        Cmd::Export(Export::Plate {
            file,
            output,
            enclosure,
        }) => {
            let project = read(&file)?;
            let bytes = plate_pdf(&project, enclosure.as_deref())?;
            write_output(&file, &output, &bytes)?;
            Ok(exit::OK)
        }
        Cmd::Export(Export::Plc {
            file,
            format,
            output,
            spares,
        }) => {
            let project = read(&file)?;
            export_plc(&project, &file, &format, &output, spares, out)?;
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
        Cmd::Import(Import::Eplan {
            pxf,
            project,
            json,
            output,
            lang,
        }) => import_eplan(
            &pxf,
            project.as_deref(),
            json,
            output.as_deref(),
            &lang,
            out,
        ),
        Cmd::Calc {
            file,
            tables,
            pairs,
            json,
        } => calc(&file, tables.as_deref(), pairs.as_deref(), json, out),
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

/// `kopercad-cli import eplan`: previews a PXF file as an import, and with
/// `output` writes the imported project to a new file.
fn import_eplan(
    pxf: &Path,
    project: Option<&Path>,
    json: bool,
    output: Option<&Path>,
    lang: &str,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    use novo_interop::eplan::pxf::{PxfOptions, plan_pxf, read_pxf};

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
    let base = match project {
        Some(path) => read(path)?,
        None => Project::new(
            pxf.file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "EPLAN import".into()),
        ),
    };
    let file = std::fs::File::open(pxf)
        .map_err(|e| Failure::new(exit::READ, format!("cannot read {}: {e}", pxf.display())))?;
    let parsed = read_pxf(std::io::BufReader::new(file))
        .map_err(|e| Failure::new(exit::READ, format!("cannot import {}: {e}", pxf.display())))?;
    let plan = plan_pxf(
        &base,
        &parsed,
        &PxfOptions {
            lang: lang.to_string(),
        },
    );
    let report = plan.report();
    let mut written = false;
    if let Some(path) = output {
        let mut doc = novo_core::Document::new(base);
        plan.apply(&mut doc)
            .map_err(|e| Failure::new(exit::USAGE, e.to_string()))?;
        ProjectFile::create(path, doc.project()).map_err(|e| {
            Failure::new(exit::WRITE, format!("cannot write {}: {e}", path.display()))
        })?;
        written = true;
    }
    if json {
        let moved: Vec<Value> = report
            .moved
            .iter()
            .map(|(what, n)| json!({"what": what, "count": n}))
            .collect();
        let not_carried: Vec<Value> = report
            .lossy
            .iter()
            .map(|l| json!({"what": l.what, "count": l.count, "why": l.why}))
            .collect();
        let doc = json!({
            "summary": plan.summary(),
            "creates": plan.creates(),
            "existing": plan.existing,
            "carried": moved,
            "not_carried": not_carried,
            "notes": report.notes,
            "written": written,
        });
        print(out, &doc.to_string())?;
    } else {
        print(out, &report.text())?;
        if written {
            print(out, "Written.")?;
        }
    }
    Ok(exit::OK)
}

/// `kopercad-cli import sheet`: previews the sheet, and with `output`
/// writes the imported project to a new file.
fn import_sheet(
    file: &Path,
    sheet: &Path,
    json: bool,
    output: Option<&Path>,
    resolve: Option<ResolveSide>,
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
        Some(ResolveSide::Sheet) => plan.resolve(Resolution::Sheet),
        Some(ResolveSide::Project) => plan.resolve(Resolution::Project),
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

/// `kopercad-cli calc`: reads the plant JSON, optionally the licensed
/// tables, and prints one report per circuit.
fn calc(
    file: &Path,
    tables: Option<&Path>,
    pairs: Option<&Path>,
    json: bool,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    use novo_checks::calc::selectivity::PairTable;
    use novo_checks::calc::spec;
    use novo_checks::calc::tables::TableSet;

    let usage = |m: String| Failure::new(exit::USAGE, m);
    let text = std::fs::read_to_string(file)
        .map_err(|e| Failure::new(exit::READ, format!("cannot read {}: {e}", file.display())))?;
    let root: Value = serde_json::from_str(&text)
        .map_err(|e| usage(format!("{} is not valid JSON: {e}", file.display())))?;
    let plant = spec::parse(&root).map_err(|e| usage(format!("{}: {e}", file.display())))?;
    let set = match tables {
        None => TableSet::builtin(),
        Some(path) => {
            let csv = std::fs::read_to_string(path).map_err(|e| {
                Failure::new(exit::READ, format!("cannot read {}: {e}", path.display()))
            })?;
            let name = path.file_name().map_or_else(
                || "licensed tables".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            );
            TableSet::builtin()
                .with_csv(&format!("licensed copy {name}"), &csv)
                .map_err(|e| usage(format!("{}: {e}", path.display())))?
        }
    };
    let pair_table = match pairs {
        None => None,
        Some(path) => {
            let csv = std::fs::read_to_string(path).map_err(|e| {
                Failure::new(exit::READ, format!("cannot read {}: {e}", path.display()))
            })?;
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            Some(
                PairTable::from_csv(&name, &csv)
                    .map_err(|e| usage(format!("{}: {e}", path.display())))?,
            )
        }
    };
    let reports = spec::evaluate_all(&plant, &set, pair_table.as_ref());
    if json {
        let list: Vec<Value> = reports
            .iter()
            .map(|r| {
                json!({
                    "circuit": r.name,
                    "rows": r.rows.iter().map(|row| json!({
                        "check": row.title,
                        "status": format!("{:?}", row.status).to_lowercase(),
                        "value": row.value,
                        "limit": row.limit,
                        "unit": row.unit,
                        "text": row.text,
                    })).collect::<Vec<_>>(),
                    "assumptions": r.assumptions,
                    "tables": r.tables,
                })
            })
            .collect();
        print(out, &format!("{:#}", Value::Array(list)))?;
    } else {
        for r in &reports {
            print(out, r.to_text().trim_end())?;
        }
    }
    Ok(if reports.iter().any(|r| r.has_failure()) {
        exit::PROBLEMS
    } else {
        exit::OK
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
        delimiter: args.delimiter.unwrap_or(','),
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
            let bytes = novo_py::report_bytes_in(&project, ids, format, args.language.as_deref())
                .map_err(usage)?;
            write_output(file, path, &bytes)?;
        }
        None => {
            // On a terminal or in a pipe a byte order mark is only noise.
            let format = ReportFormat::Csv(CsvOptions { bom: false, ..csv });
            let bytes = novo_py::report_bytes_in(&project, ids, format, args.language.as_deref())
                .map_err(usage)?;
            out.write_all(&bytes)
                .map_err(|e| Failure::new(exit::WRITE, format!("cannot write output: {e}")))?;
        }
    }
    Ok(exit::OK)
}

/// The wire production export: the plain job file as CSV (to a `.csv` path
/// or standard output), the whole report as a workbook for `.xlsx`.
fn export_wirecut(args: &ReportArgs, out: &mut dyn Write) -> Result<i32, Failure> {
    let csv_path = args
        .output
        .as_ref()
        .is_none_or(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("csv")));
    if !csv_path {
        return export_reports(&args.file, &["wirecut"], args, out);
    }
    let project = read(&args.file)?;
    let jobs = novo_reports::wire_cut_jobs(&project);
    // The same job file as the app writes: `;` and a byte order mark in a
    // file, nothing but the plain header and rows on standard output. Like
    // the app, no file is written when there is nothing to cut.
    if jobs.is_empty() && args.output.is_some() {
        print(out, "No wires to cut yet: connect two device pins")?;
        return Ok(exit::OK);
    }
    let options = CsvOptions {
        delimiter: args
            .delimiter
            .unwrap_or(if args.output.is_some() { ';' } else { ',' }),
        bom: args.output.is_some(),
        ..CsvOptions::default()
    };
    let text = novo_reports::wire_cut_csv(&jobs, &options);
    match &args.output {
        Some(path) => write_output(&args.file, path, text.as_bytes())?,
        None => out
            .write_all(text.as_bytes())
            .map_err(|e| Failure::new(exit::WRITE, format!("cannot write output: {e}")))?,
    }
    Ok(exit::OK)
}

/// The commissioning report of `file`: its ticked checks from the
/// `commissioning` document, or an untouched list when nothing was ticked
/// yet.
fn commissioning_report(
    file: &Path,
    output: Option<&Path>,
    delimiter: char,
    language: Option<&str>,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    let handle = ProjectFile::read_only(file).map_err(|e| read_failure(file, &e))?;
    let project = handle.load().map_err(|e| read_failure(file, &e))?;
    let json = handle
        .sidecar("commissioning")
        .map_err(|e| read_failure(file, &e))?;
    let store = match json {
        None => novo_commissioning::CommissioningStore::new(),
        Some(json) => novo_commissioning::CommissioningStore::from_json(&json).map_err(|e| {
            Failure::new(
                exit::READ,
                format!(
                    "cannot read the commissioning data of {}: {e}",
                    file.display()
                ),
            )
        })?,
    };
    let input = novo_commissioning::to_report_input(
        &project,
        &store,
        novo_reports::commissioning::default_sign_offs(""),
    );
    let report = novo_reports::commissioning::commissioning_report(&input);
    let translator = novo_py::report_translator(&project, language)
        .map_err(|e| Failure::new(exit::USAGE, e.to_string()))?;
    let csv = CsvOptions {
        delimiter,
        ..CsvOptions::default()
    };
    let options = novo_pdf::ReportPdfOptions {
        title: format!("{}, commissioning", project.info().name),
        language: translator.clone(),
        ..Default::default()
    };
    match output {
        Some(path) => {
            let format = match novo_pdf::ReportFileFormat::from_path(path) {
                Some(novo_pdf::ReportFileFormat::Csv(_)) => novo_pdf::ReportFileFormat::Csv(csv),
                Some(format) => format,
                None => {
                    return Err(Failure::new(
                        exit::USAGE,
                        "give an output ending in .pdf, .csv or .xlsx",
                    ));
                }
            };
            let bytes = novo_pdf::report_bytes(&report, format, &options)
                .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
            write_output(file, path, &bytes)?;
        }
        None => {
            let csv = CsvOptions { bom: false, ..csv };
            let report = novo_reports::i18n::translate_output(&report, &translator);
            out.write_all(novo_reports::to_csv(&report, &csv).as_bytes())
                .map_err(|e| Failure::new(exit::WRITE, format!("cannot write output: {e}")))?;
        }
    }
    Ok(exit::OK)
}

fn export_labels(args: &LabelArgs, out: &mut dyn Write) -> Result<i32, Failure> {
    let usage = |m: String| Failure::new(exit::USAGE, m);
    let layout = novo_reports::LabelLayout::parse(&args.layout).ok_or_else(|| {
        usage(format!(
            "unknown label layout `{}` (generic or brother)",
            args.layout
        ))
    })?;
    for kind in &args.kinds {
        if !novo_reports::Labels::KINDS.contains(&kind.as_str()) {
            return Err(usage(format!(
                "unknown label kind `{kind}` ({})",
                novo_reports::Labels::KINDS.join(", ")
            )));
        }
    }
    let project = read(&args.file)?;
    let labels = novo_reports::Report::generate(&novo_reports::Labels, &project);
    let kinds: Vec<&str> = args.kinds.iter().map(String::as_str).collect();
    let mut text = novo_reports::labels_csv(&labels, &kinds, layout);
    match &args.output {
        Some(path) => {
            if novo_py::is_project_path(path) {
                return write_output(&args.file, path, &[]).map(|()| exit::OK);
            }
            write_output(&args.file, path, text.as_bytes())?;
        }
        None => {
            // A byte order mark is only noise on a terminal or in a pipe.
            if let Some(stripped) = text.strip_prefix('\u{feff}') {
                text = stripped.to_owned();
            }
            out.write_all(text.as_bytes())
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
            let notes = xref.labels_on_page(p.id).into_iter().map(note_of);
            (p.id, notes.collect())
        })
        .collect();
    novo_pdf::export_pdf_with_art(project, &options, &art)
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))
}

/// A cross-reference label as a note: a coil's contact image becomes the
/// contact table, its warnings stay text below it.
fn note_of(l: novo_xref::Label) -> SymbolNote {
    use novo_xref::ContactKind;
    let rows: Vec<NoteRow> = l
        .rows
        .iter()
        .map(|r| NoteRow {
            glyph: match r.kind {
                ContactKind::Power => NoteGlyph::Power,
                ContactKind::NormallyClosed => NoteGlyph::NormallyClosed,
                ContactKind::Changeover => NoteGlyph::Changeover,
                ContactKind::NormallyOpen | ContactKind::Other => NoteGlyph::NormallyOpen,
            },
            pins: r.pins.clone(),
            location: r.location.clone(),
        })
        .collect();
    let lines = if rows.is_empty() {
        l.lines
    } else {
        l.extra_lines().to_vec()
    };
    SymbolNote {
        symbol: l.symbol,
        lines,
        warning: l.warning,
        anchor: match l.anchor {
            novo_xref::LabelAnchor::Below => NoteAnchor::Below,
            novo_xref::LabelAnchor::Right => NoteAnchor::Right,
        },
        rows,
        density: NoteDensity::default(),
    }
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

/// The plate PDF of one enclosure: the one named by `tag`, or the only
/// enclosure that has a plate.
fn plate_pdf(project: &Project, tag: Option<&str>) -> Result<Vec<u8>, Failure> {
    let usage = |text: String| Failure::new(exit::USAGE, text);
    let with_plate: Vec<_> = project
        .structures()
        .filter(|s| s.enclosure && s.plate.is_some())
        .collect();
    let chosen = match tag {
        Some(tag) => {
            let wanted = tag.trim().to_lowercase();
            let all = project.structures().filter(|s| s.enclosure);
            let found = all
                .into_iter()
                .find(|s| s.tag.to_string().to_lowercase() == wanted);
            found.ok_or_else(|| usage(format!("there is no enclosure `{tag}`")))?
        }
        None => match with_plate.as_slice() {
            [one] => one,
            [] => return Err(usage("no enclosure has a mounting plate".into())),
            _ => {
                let tags: Vec<String> = with_plate.iter().map(|s| s.tag.to_string()).collect();
                return Err(usage(format!(
                    "several enclosures have a plate, pick one with --enclosure: {}",
                    tags.join(", ")
                )));
            }
        },
    };
    let (docs, _table) = novo_layout::model::plate_documents(project, chosen.id)
        .ok_or_else(|| usage(format!("{} has no mounting plate", chosen.tag)))?;
    Ok(novo_pdf::plate::export_plate_pdf(
        &docs,
        &project.info().name,
    ))
}

/// Writes the pages as DXF: one `.dxf` file when `output` ends in `.dxf`
/// (exactly one page), otherwise one file per page in the directory.
fn export_plc(
    project: &Project,
    input: &Path,
    format: &str,
    output: &Path,
    spares: bool,
    out: &mut dyn Write,
) -> Result<(), Failure> {
    let format = novo_plc::link::LinkFormat::from_key(format).ok_or_else(|| {
        Failure::new(
            exit::USAGE,
            format!("unknown format {format:?}: use csv, plcopen, tia, studio5000 or hardware"),
        )
    })?;
    let library = novo_py::library().map_err(|e| Failure::new(exit::UNAVAILABLE, e.to_string()))?;
    let index = novo_plc::PlcIndex::build(project, library);
    if index.cards().is_empty() {
        return Err(Failure::new(exit::USAGE, "the project has no PLC cards"));
    }
    let options = novo_plc::link::ExportOptions {
        include_spares: spares,
        stamp: String::new(),
        project: project.info().name.clone(),
    };
    let exported = novo_plc::link::export(project, &index, format, &options);
    std::fs::create_dir_all(output).map_err(|e| {
        Failure::new(
            exit::WRITE,
            format!("cannot create {}: {e}", output.display()),
        )
    })?;
    let stem = file_stem(&input.file_stem().unwrap_or_default().to_string_lossy());
    for file in &exported.files {
        let name = if file.suffix.is_empty() {
            format!("{stem} PLC.{}", file.extension)
        } else {
            format!("{stem} PLC {}.{}", file_stem(&file.suffix), file.extension)
        };
        let path = output.join(name);
        write_output(input, &path, &file.bytes)?;
        print(out, &format!("{} ({} tags)", path.display(), file.tags))?;
    }
    for warning in &exported.warnings {
        print(out, &format!("note: {warning}"))?;
    }
    Ok(())
}

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
