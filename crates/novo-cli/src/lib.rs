//! The `novo` command-line tool (owned by WS-28).
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
use novo_core::Project;
use novo_storage::ProjectFile;
use serde_json::{Value, json};

/// Process exit codes.
pub mod exit {
    /// Done.
    pub const OK: i32 = 0;
    /// `novo check` found problems.
    pub const PROBLEMS: i32 = 1;
    /// The command line was not understood.
    pub const USAGE: i32 = 2;
    /// The project file could not be read.
    pub const READ: i32 = 3;
    /// The output could not be written.
    pub const WRITE: i32 = 4;
    /// The subcommand is not available in this build.
    pub const UNAVAILABLE: i32 = 5;
}

#[derive(Debug, Parser)]
#[command(
    name = "novo",
    version,
    about = "NovoElectric command-line tool: inspect, check and export project files",
    long_about = "NovoElectric command-line tool: inspect, check and export project files.\n\n\
                  Project files are only read, never changed.\n\
                  Exit codes: 0 done, 1 check found problems, 2 bad command line, \
                  3 project not readable, 4 output not writable, 5 not available in this build."
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
    /// Run design checks; exits with 1 when there are problems.
    Check {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum Export {
    /// The whole project as diffable JSON.
    Json {
        file: PathBuf,
        /// Output file; standard output when left out.
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// The schematic pages as one PDF.
    Pdf {
        file: PathBuf,
        /// Output file.
        #[arg(short, long)]
        output: PathBuf,
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
            let text = e.render().ansi().to_string();
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
        Cmd::Export(Export::Pdf { file, output: _ }) => {
            read(&file)?;
            Err(Failure::new(
                exit::UNAVAILABLE,
                "PDF export is not in this build yet: it comes with novo-pdf \
                 (pull request #49). Use `novo export json` meanwhile.",
            ))
        }
        Cmd::Check { file, json } => {
            let project = read(&file)?;
            let problems = check(&project);
            if json {
                print(out, &format!("{:#}", Value::Array(problems.clone())))?;
            } else if problems.is_empty() {
                print(out, "No problems found.")?;
            } else {
                for p in &problems {
                    print(
                        out,
                        &format!(
                            "{}: {}",
                            p["check"].as_str().unwrap_or(""),
                            p["message"].as_str().unwrap_or("")
                        ),
                    )?;
                }
            }
            Ok(if problems.is_empty() {
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

/// Writes an output file, refusing to overwrite the project it came from.
fn write_output(input: &Path, output: &Path, bytes: &[u8]) -> Result<(), Failure> {
    if let (Ok(a), Ok(b)) = (input.canonicalize(), output.canonicalize())
        && a == b
    {
        return Err(Failure::new(
            exit::WRITE,
            format!(
                "refusing to overwrite the project file {} with the export",
                input.display()
            ),
        ));
    }
    std::fs::write(output, bytes).map_err(|e| {
        Failure::new(
            exit::WRITE,
            format!("cannot write {}: {e}", output.display()),
        )
    })
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

/// The design checks this tool runs until the checks engine lands on main,
/// which will replace them: duplicate device tags and shorts between
/// potentials.
fn check(p: &Project) -> Vec<Value> {
    let mut problems = Vec::new();
    for (tag, ids) in p.duplicate_tags() {
        problems.push(json!({
            "check": "duplicate_tag",
            "message": format!("{tag} is used by {} objects", ids.len()),
            "objects": ids,
        }));
    }
    let netlist = p.netlist();
    for net in netlist.potential_conflicts() {
        let mut names: Vec<String> = net
            .defined
            .iter()
            .filter_map(|id| p.potential(*id).map(|pot| pot.name.clone()))
            .collect();
        names.sort();
        let at: Vec<String> = net
            .endpoints
            .iter()
            .filter(|e| e.as_pin().is_some())
            .map(|&e| novo_py::endpoint_name(p, e))
            .take(4)
            .collect();
        problems.push(json!({
            "check": "potential_short",
            "message": format!("short between {} at {}", names.join(" and "), at.join(", ")),
            "wires": net.wires,
        }));
    }
    problems
}
