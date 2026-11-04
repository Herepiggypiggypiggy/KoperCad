//! `kopercad-cli audit`: checks meant for scripts and CI. Every verb only
//! reads; exit 0 is clean, 1 means findings (`--strict` makes warnings count),
//! 2 a bad call, 3 an unreadable input.
//!
//! - `links`: sheet links (interruption points) with no partner, unnamed or
//!   all on one page, and whether each link's printed label names every
//!   partner. The same lone-link problem shows in the app under Check.
//! - `symbols`: lint symbol files (a pack, a company folder or one file):
//!   parse errors, id/path mismatch, every rule of `novo_symbols::validate`,
//!   duplicate ids and aliases, plus warnings for missing description,
//!   keywords or artwork. Command line only.
//! - `nets`: every net with its pins, wires and potentials, flagging nets
//!   with a single pin.
//! - `search`: the project search (tags, terminals, pages) as a list.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Subcommand;
use novo_core::{Project, SymbolRole};
use novo_symbols::{Symbol, validate};
use serde_json::{Value, json};

use crate::{Failure, exit, print, read};

#[derive(Debug, Subcommand)]
pub(crate) enum Audit {
    /// Sheet links (interruption points): which have no partner, are
    /// unnamed or point to the same page, and whether the label drawn next
    /// to each names all its partners.
    Links {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Exit 1 on warnings too.
        #[arg(long)]
        strict: bool,
    },
    /// Lint symbol files: one `.toml` file, or a folder (ids are the paths
    /// below it, like `iec/contacts/no`). Each file is OK, WARN or FAIL.
    Symbols {
        /// Files or folders.
        #[arg(required = true)]
        paths: Vec<PathBuf>,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
        /// Exit 1 on warnings too.
        #[arg(long)]
        strict: bool,
    },
    /// Every net: its pins, wires and potentials. Nets with one pin are
    /// marked open.
    Nets {
        file: PathBuf,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Find devices, cables, terminals and pages by tag, designation or
    /// page name (the Ctrl+K search), best match first.
    Search {
        file: PathBuf,
        text: String,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Level {
    Ok,
    Warn,
    Fail,
}

impl Level {
    fn word(self) -> &'static str {
        match self {
            Level::Ok => "OK",
            Level::Warn => "WARN",
            Level::Fail => "FAIL",
        }
    }
}

fn verdict(worst: Level, strict: bool) -> i32 {
    if worst == Level::Fail || (strict && worst == Level::Warn) {
        exit::PROBLEMS
    } else {
        exit::OK
    }
}

pub(crate) fn run(cmd: Audit, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Audit::Links { file, json, strict } => links(&read(&file)?, json, strict, out),
        Audit::Symbols {
            paths,
            json,
            strict,
        } => symbols(&paths, json, strict, out),
        Audit::Nets { file, json } => nets(&read(&file)?, json, out),
        Audit::Search { file, text, json } => search(&read(&file)?, &text, json, out),
    }
}

/// One line of a report.
struct Row {
    level: Level,
    subject: String,
    message: String,
}

fn report(
    out: &mut dyn Write,
    rows: &[Row],
    json: bool,
    noun: &str,
    strict: bool,
) -> Result<i32, Failure> {
    let worst = rows.iter().map(|r| r.level).max().unwrap_or(Level::Ok);
    let count = |l| rows.iter().filter(|r| r.level == l).count();
    if json {
        let value = json!({
            "checked": rows.len(),
            "fail": count(Level::Fail),
            "warn": count(Level::Warn),
            "items": rows.iter().map(|r| json!({
                "level": r.level.word(),
                "subject": r.subject,
                "message": r.message,
            })).collect::<Vec<_>>(),
        });
        print(out, &format!("{value:#}"))?;
    } else {
        for r in rows {
            let tail = if r.message.is_empty() {
                String::new()
            } else {
                format!(": {}", r.message)
            };
            print(out, &format!("{:<4} {}{tail}", r.level.word(), r.subject))?;
        }
        print(
            out,
            &format!(
                "{} {noun}, {} failed, {} with warnings",
                rows.len(),
                count(Level::Fail),
                count(Level::Warn)
            ),
        )?;
    }
    Ok(verdict(worst, strict))
}

fn links(project: &Project, json: bool, strict: bool, out: &mut dyn Write) -> Result<i32, Failure> {
    let library = novo_py::library().map_err(|e| Failure::new(exit::UNAVAILABLE, e.to_string()))?;
    let xref = novo_xref::XrefIndex::build(project, library);
    // name -> (symbol, page name, column)
    let mut groups: BTreeMap<String, Vec<_>> = BTreeMap::new();
    for s in project.symbols() {
        if let SymbolRole::Interruption { name } = &s.role {
            groups.entry(name.clone()).or_default().push(s.id);
        }
    }
    let mut rows = Vec::new();
    for (name, members) in &groups {
        for &id in members {
            let Some(here) = project.location_of(id) else {
                continue;
            };
            let subject = if name.is_empty() {
                format!("unnamed link at {here}")
            } else {
                format!("link {name} at {here}")
            };
            let mut level = Level::Ok;
            let mut notes: Vec<String> = Vec::new();
            let mut raise = |l: Level, text: String| {
                level = level.max(l);
                notes.push(text);
            };
            if name.is_empty() {
                raise(Level::Fail, "has no name, so it links to nothing".into());
            } else if members.len() == 1 {
                raise(Level::Fail, "no partner with the same name".into());
            } else {
                let partners = xref.partners(id);
                if partners.iter().all(|p| p.location.page == here.page) {
                    raise(
                        Level::Warn,
                        "every partner is on the same page; a link is for leaving the page".into(),
                    );
                }
                let label = xref.label(id);
                let text = label
                    .as_ref()
                    .map(|l| l.lines.join(" "))
                    .unwrap_or_default();
                if label.is_none() {
                    raise(Level::Warn, "has partners but no label is drawn".into());
                }
                for p in &partners {
                    let target = p.target_from(&here.structure);
                    if !text.split_whitespace().any(|w| w == target) {
                        raise(
                            Level::Fail,
                            format!("label `{text}` does not name partner {target}"),
                        );
                    }
                }
                if notes.is_empty() {
                    notes.push(format!("-> {text}"));
                }
            }
            rows.push(Row {
                level,
                subject,
                message: notes.join("; "),
            });
        }
    }
    report(out, &rows, json, "sheet links", strict)
}

/// Where the lint reads a file's expected id from.
struct Source {
    path: PathBuf,
    /// The id the path implies (folder mode), else none.
    expected: Option<String>,
}

fn collect(path: &Path, base: Option<&Path>, out: &mut Vec<Source>) -> Result<(), Failure> {
    let unreadable = |e: std::io::Error| {
        Failure::new(exit::READ, format!("cannot read {}: {e}", path.display()))
    };
    if path.is_dir() {
        let mut entries = Vec::new();
        for e in std::fs::read_dir(path).map_err(unreadable)? {
            entries.push(e.map_err(unreadable)?.path());
        }
        entries.sort();
        let base = base.unwrap_or(path);
        for e in entries {
            if e.is_dir() || e.extension().is_some_and(|x| x == "toml") {
                collect(&e, Some(base), out)?;
            }
        }
    } else {
        let expected = base.map(|b| {
            let rel = path.strip_prefix(b).unwrap_or(path).with_extension("");
            let parts: Vec<_> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            parts.join("/")
        });
        out.push(Source {
            path: path.to_path_buf(),
            expected,
        });
    }
    Ok(())
}

fn lint(symbol: &Symbol) -> Vec<(Level, String)> {
    let mut found: Vec<(Level, String)> = validate(symbol)
        .into_iter()
        .map(|i| (Level::Fail, i.message))
        .collect();
    if symbol.description.trim().is_empty() {
        found.push((Level::Warn, "description is empty".into()));
    }
    if symbol.keywords.is_empty() {
        found.push((Level::Warn, "no search keywords".into()));
    }
    if symbol.graphics.is_empty() {
        found.push((Level::Warn, "no artwork".into()));
    }
    found
}

fn symbols(
    paths: &[PathBuf],
    json: bool,
    strict: bool,
    out: &mut dyn Write,
) -> Result<i32, Failure> {
    let mut sources = Vec::new();
    for p in paths {
        if !p.exists() {
            return Err(Failure::new(
                exit::READ,
                format!("{} does not exist", p.display()),
            ));
        }
        collect(p, None, &mut sources)?;
    }
    let mut rows = Vec::new();
    let mut ids: BTreeMap<String, PathBuf> = BTreeMap::new();
    let mut aliases: BTreeMap<String, String> = BTreeMap::new();
    for src in &sources {
        let name = src.path.display().to_string();
        let text = match std::fs::read_to_string(&src.path) {
            Ok(t) => t,
            Err(e) => {
                rows.push(Row {
                    level: Level::Fail,
                    subject: name,
                    message: format!("cannot read: {e}"),
                });
                continue;
            }
        };
        let symbol = match Symbol::from_toml(&text) {
            Ok(s) => s,
            Err(e) => {
                let msg = e.to_string().replace('\n', " ");
                rows.push(Row {
                    level: Level::Fail,
                    subject: name,
                    message: format!("not a valid symbol file: {}", msg.trim()),
                });
                continue;
            }
        };
        let mut found = lint(&symbol);
        match &src.expected {
            Some(expected) if *expected != symbol.id => found.push((
                Level::Fail,
                format!("id `{}` must equal its path `{expected}`", symbol.id),
            )),
            None => {
                let stem = src.path.file_stem().map(|s| s.to_string_lossy());
                if symbol.id.rsplit('/').next() != stem.as_deref() {
                    found.push((
                        Level::Fail,
                        format!("id `{}` does not end in the file name", symbol.id),
                    ));
                }
            }
            _ => {}
        }
        if let Some(first) = ids.insert(symbol.id.clone(), src.path.clone()) {
            found.push((
                Level::Fail,
                format!("id defined more than once (also {})", first.display()),
            ));
        }
        for alias in &symbol.aliases {
            // Aliases are unique within a pack (`iec/`, `nfpa/`), not across.
            let pack = symbol.id.split('/').next().unwrap_or_default();
            if let Some(other) = aliases.insert(format!("{pack}:{alias}"), symbol.id.clone())
                && other != symbol.id
            {
                found.push((
                    Level::Fail,
                    format!("alias `{alias}` is also used by {other}"),
                ));
            }
        }
        let level = found.iter().map(|f| f.0).max().unwrap_or(Level::Ok);
        let message = found
            .iter()
            .map(|f| f.1.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        rows.push(Row {
            level,
            subject: name,
            message,
        });
    }
    report(out, &rows, json, "symbol files", strict)
}

fn nets(project: &Project, json: bool, out: &mut dyn Write) -> Result<i32, Failure> {
    let value = novo_py::nets_json(project);
    let list: &[Value] = value.as_array().map(Vec::as_slice).unwrap_or(&[]);
    if json {
        print(out, &format!("{value:#}"))?;
        return Ok(exit::OK);
    }
    let mut open = 0;
    for net in list {
        let pins: Vec<&str> = net["pins"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let potentials: Vec<&str> = net["potentials"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let wires = net["wires"].as_array().map_or(0, Vec::len);
        let mut line = pins.join(" ");
        if !potentials.is_empty() {
            line.push_str(&format!("  [{}]", potentials.join(", ")));
        }
        if pins.len() < 2 {
            open += 1;
            line.push_str("  (open)");
        }
        print(out, &format!("{line}  ({wires} wires)"))?;
    }
    print(out, &format!("{} nets, {open} open", list.len()))?;
    Ok(exit::OK)
}

fn search(project: &Project, text: &str, json: bool, out: &mut dyn Write) -> Result<i32, Failure> {
    let hits = project.search(text);
    if json {
        let value: Vec<Value> = hits
            .iter()
            .map(|h| {
                json!({
                    "kind": format!("{:?}", h.id.kind()),
                    "text": h.text,
                    "match": format!("{:?}", h.quality),
                })
            })
            .collect();
        print(out, &format!("{:#}", Value::Array(value)))?;
    } else {
        for h in &hits {
            print(
                out,
                &format!(
                    "{:<8} {:<9} {}",
                    format!("{:?}", h.id.kind()),
                    format!("{:?}", h.quality),
                    h.text
                ),
            )?;
        }
        print(out, &format!("{} found", hits.len()))?;
    }
    Ok(if hits.is_empty() {
        exit::PROBLEMS
    } else {
        exit::OK
    })
}
