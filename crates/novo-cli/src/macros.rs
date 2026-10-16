//! `kopercad-cli macros`: the macro library from the command line, so a team
//! can script sharing circuits (list, export, import, install the starter
//! set). It reads and writes only the library folder and the files it is
//! given; no project is opened.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use novo_parts::{
    Imported, MACRO_PACK_EXTENSION, MacroLibrary, OnConflict, STARTER_FOLDER, user_macros_dir,
};
use serde_json::json;

use crate::{Failure, exit, print};

/// Which library to work on.
#[derive(Debug, Args)]
pub(crate) struct LibraryArg {
    /// A macro library folder (a company folder, or a copy to test with).
    /// Default: the per-user library, `%APPDATA%\KoperCAD\Macros`.
    #[arg(long, value_name = "DIR", global = true)]
    library: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Macros {
    /// List the macros: path, name, revision, members, parts.
    List {
        #[command(flatten)]
        lib: LibraryArg,
        /// Only macros whose name, tags or description match.
        query: Option<String>,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Write one macro as a `.kcmacro` file, or several as a
    /// `.kcmacropack` (by the output's extension, or when more than one
    /// macro is named).
    Export {
        #[command(flatten)]
        lib: LibraryArg,
        /// Library paths as `list` prints them, e.g. `Starter/Motor.kcmacro`.
        #[arg(required = true)]
        macros: Vec<String>,
        /// Output file.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Add a `.kcmacro` or `.kcmacropack` to the library. A name that is
    /// taken is kept as a copy unless `--replace` or `--skip` says otherwise.
    Import {
        #[command(flatten)]
        lib: LibraryArg,
        file: PathBuf,
        /// Library folder to import into.
        #[arg(long, default_value = "")]
        folder: String,
        /// Replace a macro of the same name (its revision goes up by one).
        #[arg(long, conflicts_with = "skip")]
        replace: bool,
        /// Leave a macro of the same name alone.
        #[arg(long)]
        skip: bool,
    },
    /// Put the starter macros in the library (folder `Starter`); existing
    /// ones are never overwritten.
    InstallStarters {
        #[command(flatten)]
        lib: LibraryArg,
    },
}

fn library(arg: &LibraryArg) -> Result<MacroLibrary, Failure> {
    match &arg.library {
        Some(dir) => Ok(MacroLibrary::new(dir, "company")),
        None => user_macros_dir()
            .map(|d| MacroLibrary::new(d, "user"))
            .ok_or_else(|| {
                Failure::new(
                    exit::READ,
                    "no profile folder on this computer; name a library with --library",
                )
            }),
    }
}

fn read_failure(e: impl std::fmt::Display) -> Failure {
    Failure::new(exit::READ, e.to_string())
}

fn write_failure(e: impl std::fmt::Display) -> Failure {
    Failure::new(exit::WRITE, e.to_string())
}

pub(crate) fn run(cmd: Macros, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Macros::List { lib, query, json } => {
            let lib = library(&lib)?;
            let entries = lib.search(query.as_deref().unwrap_or(""));
            if json {
                let rows: Vec<_> = entries
                    .iter()
                    .map(|e| {
                        json!({
                            "path": e.path, "name": e.name, "description": e.description,
                            "author": e.author, "tags": e.tags, "revision": e.revision,
                            "parameters": e.parameters, "members": e.members, "parts": e.parts,
                        })
                    })
                    .collect();
                print(
                    out,
                    &serde_json::to_string_pretty(&rows).unwrap_or_default(),
                )?;
            } else {
                for e in &entries {
                    print(
                        out,
                        &format!(
                            "{}\t{}\trev {}\t{} members\t{} parts",
                            e.path, e.name, e.revision, e.members, e.parts
                        ),
                    )?;
                }
                if entries.is_empty() {
                    print(out, "No macros.")?;
                }
            }
            Ok(exit::OK)
        }
        Macros::Export {
            lib,
            macros,
            output,
        } => {
            let lib = library(&lib)?;
            let pack = macros.len() > 1 || has_pack_extension(&output);
            if pack {
                let paths: Vec<&str> = macros.iter().map(String::as_str).collect();
                lib.export_pack(&paths, &output).map_err(write_failure)?;
            } else {
                lib.export(&macros[0], &output).map_err(write_failure)?;
            }
            print(out, &format!("Wrote {}", output.display()))?;
            Ok(exit::OK)
        }
        Macros::Import {
            lib,
            file,
            folder,
            replace,
            skip,
        } => {
            let lib = library(&lib)?;
            let on = if replace {
                OnConflict::Replace
            } else if skip {
                OnConflict::Skip
            } else {
                OnConflict::KeepBoth
            };
            let done = lib.import(&file, &folder, on).map_err(read_failure)?;
            report(out, &done)?;
            Ok(exit::OK)
        }
        Macros::InstallStarters { lib } => {
            let lib = library(&lib)?;
            let done = lib.install_starters().map_err(write_failure)?;
            print(out, &format!("Starter macros in folder {STARTER_FOLDER}:"))?;
            report(out, &done)?;
            Ok(exit::OK)
        }
    }
}

fn has_pack_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case(MACRO_PACK_EXTENSION))
}

fn report(out: &mut dyn Write, done: &[Imported]) -> Result<(), Failure> {
    for item in done {
        let line = match item {
            Imported::Added(p) => format!("added {p}"),
            Imported::Replaced(p) => format!("replaced {p}"),
            Imported::Skipped(p) => format!("skipped {p}"),
        };
        print(out, &line)?;
    }
    Ok(())
}
