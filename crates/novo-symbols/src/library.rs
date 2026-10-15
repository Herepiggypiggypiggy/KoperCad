//! Loading symbols into a library, from the built-in set or a directory.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

use crate::pack::Pack;
use crate::symbol::Symbol;
use crate::validate::{Issue, validate};

include!(concat!(env!("OUT_DIR"), "/builtin.rs"));

/// Why a library failed to load.
#[derive(Debug)]
pub enum LoadError {
    /// A file could not be read.
    Io { path: PathBuf, error: io::Error },
    /// A file is not valid TOML or does not match the format.
    Parse {
        path: String,
        error: toml::de::Error,
    },
    /// A symbol's `id` field does not match its file path.
    IdMismatch { path: String, id: String },
    /// Two sources define the same id.
    Duplicate { id: String },
    /// Two symbols claim the same palette alias.
    DuplicateAlias {
        alias: String,
        id: String,
        other: String,
    },
    /// Symbols parsed but broke validation rules.
    Invalid(Vec<Issue>),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io { path, error } => write!(f, "{}: {error}", path.display()),
            LoadError::Parse { path, error } => write!(f, "{path}: {error}"),
            LoadError::IdMismatch { path, id } => {
                write!(f, "{path}: id '{id}' does not match the file path")
            }
            LoadError::Duplicate { id } => write!(f, "{id}: defined more than once"),
            LoadError::DuplicateAlias { alias, id, other } => {
                write!(f, "{id}: alias '{alias}' is already used by {other}")
            }
            LoadError::Invalid(issues) => {
                write!(f, "{} symbol issue(s)", issues.len())?;
                for issue in issues {
                    write!(f, "\n  {issue}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for LoadError {}

/// A set of symbols keyed by id.
#[derive(Debug, Clone, Default)]
pub struct Library {
    symbols: BTreeMap<String, Symbol>,
    /// Palette alias to symbol id.
    aliases: BTreeMap<String, String>,
}

impl Library {
    /// The IEC pack compiled into this crate from `symbols/iec/`. This is the
    /// default library; see [`Library::builtin_pack`] for other standards.
    pub fn builtin() -> Result<Library, LoadError> {
        Library::builtin_pack(Pack::Iec)
    }

    /// One built-in pack. Aliases and favourites are unique within the pack,
    /// not across packs, so packs are loaded one at a time.
    pub fn builtin_pack(pack: Pack) -> Result<Library, LoadError> {
        let sources = BUILTIN
            .iter()
            .filter(|(name, _)| *name == pack.id())
            .flat_map(|(_, list)| list.iter())
            .map(|(id, text)| (id.to_string(), *text));
        Library::from_sources(sources)
    }

    /// Loads the files of one pack from `dir/<pack>/…`; ids stay relative to
    /// `dir` (`nfpa/contacts/no`), as in the built-in set. For a user or
    /// company folder laid out like `symbols/`.
    pub fn load_dir_pack(dir: &Path, pack: Pack) -> Result<Library, LoadError> {
        let prefix = format!("{}/", pack.id());
        let sources = read_dir_sources(dir)?;
        Library::from_sources(
            sources
                .iter()
                .filter(|(id, _)| id.starts_with(&prefix))
                .map(|(id, text)| (id.clone(), text.as_str())),
        )
    }

    /// Loads every `*.toml` file below `dir` as one library (a folder of
    /// symbols of a single pack, or a user folder); ids are the paths relative to
    /// `dir` without the extension, using `/` separators. Fails on the first
    /// problem in any file.
    pub fn load_dir(dir: &Path) -> Result<Library, LoadError> {
        let sources = read_dir_sources(dir)?;
        Library::from_sources(sources.iter().map(|(id, text)| (id.clone(), text.as_str())))
    }

    /// Like [`Library::load_dir`], but keeps every valid symbol and returns
    /// the problems with the others alongside, so one bad file in a company
    /// library does not hide the rest. Fails only if a directory or file
    /// can't be read.
    pub fn load_dir_lenient(dir: &Path) -> Result<(Library, Vec<LoadError>), LoadError> {
        let sources = read_dir_sources(dir)?;
        Ok(Library::from_sources_lenient(
            sources.iter().map(|(id, text)| (id.clone(), text.as_str())),
        ))
    }

    /// Builds a library from `(expected id, TOML text)` pairs, parsing and
    /// validating each one. Fails if any source has a problem.
    pub fn from_sources<'a>(
        sources: impl IntoIterator<Item = (String, &'a str)>,
    ) -> Result<Library, LoadError> {
        let (library, mut problems) = Library::from_sources_lenient(sources);
        if problems.is_empty() {
            return Ok(library);
        }
        if let Some(i) = problems
            .iter()
            .position(|p| !matches!(p, LoadError::Invalid(_)))
        {
            return Err(problems.swap_remove(i));
        }
        let issues = problems
            .into_iter()
            .flat_map(|p| match p {
                LoadError::Invalid(issues) => issues,
                _ => Vec::new(),
            })
            .collect();
        Err(LoadError::Invalid(issues))
    }

    /// Builds a library from `(expected id, TOML text)` pairs, keeping the
    /// valid symbols and returning one problem per rejected source.
    pub fn from_sources_lenient<'a>(
        sources: impl IntoIterator<Item = (String, &'a str)>,
    ) -> (Library, Vec<LoadError>) {
        let mut symbols = BTreeMap::new();
        let mut aliases = BTreeMap::new();
        let mut problems = Vec::new();
        for (path, text) in sources {
            let symbol = match Symbol::from_toml(text) {
                Ok(symbol) => symbol,
                Err(error) => {
                    problems.push(LoadError::Parse { path, error });
                    continue;
                }
            };
            if symbol.id != path {
                problems.push(LoadError::IdMismatch {
                    path,
                    id: symbol.id,
                });
                continue;
            }
            if symbols.contains_key(&symbol.id) {
                problems.push(LoadError::Duplicate { id: symbol.id });
                continue;
            }
            let issues = validate(&symbol);
            if !issues.is_empty() {
                problems.push(LoadError::Invalid(issues));
                continue;
            }
            if let Some((alias, other)) = symbol
                .aliases
                .iter()
                .find_map(|a| aliases.get(a).map(|other: &String| (a, other)))
            {
                problems.push(LoadError::DuplicateAlias {
                    alias: alias.clone(),
                    id: symbol.id,
                    other: other.clone(),
                });
                continue;
            }
            for alias in &symbol.aliases {
                aliases.insert(alias.clone(), symbol.id.clone());
            }
            symbols.insert(symbol.id.clone(), symbol);
        }
        (Library { symbols, aliases }, problems)
    }

    /// Looks up a symbol by id.
    pub fn get(&self, id: &str) -> Option<&Symbol> {
        self.symbols.get(id)
    }

    /// All symbols, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &Symbol> {
        self.symbols.values()
    }

    /// Symbols whose function type is `function` or starts with
    /// `function.`, so `contact` finds `contact.no` and `contact.nc`.
    pub fn by_function<'a>(&'a self, function: &'a str) -> impl Iterator<Item = &'a Symbol> {
        self.iter().filter(move |s| {
            s.function == function
                || s.function
                    .strip_prefix(function)
                    .is_some_and(|rest| rest.starts_with('.'))
        })
    }

    /// The symbol whose `aliases` contain `alias`, so the palette can insert
    /// `mcb3` or `no` without a search. Surrounding spaces and letter case
    /// are ignored (`NO` finds `no`); otherwise the match is exact.
    pub fn by_alias(&self, alias: &str) -> Option<&Symbol> {
        self.aliases
            .get(&alias.trim().to_ascii_lowercase())
            .and_then(|id| self.symbols.get(id))
    }

    /// Symbols marked `favorite`, ordered by id.
    pub fn favorites(&self) -> impl Iterator<Item = &Symbol> {
        self.iter().filter(|s| s.favorite)
    }

    /// Number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
}

fn read_dir_sources(dir: &Path) -> Result<Vec<(String, String)>, LoadError> {
    let mut files = Vec::new();
    walk(dir, &mut files)?;
    let mut sources = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).map_err(|error| LoadError::Io {
            path: file.clone(),
            error,
        })?;
        let rel = file.strip_prefix(dir).unwrap_or(&file).with_extension("");
        let id: Vec<_> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        sources.push((id.join("/"), text));
    }
    Ok(sources)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), LoadError> {
    let io_err = |error| LoadError::Io {
        path: dir.to_path_buf(),
        error,
    };
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(io_err)? {
        entries.push(entry.map_err(io_err)?.path());
    }
    entries.sort();
    for path in entries {
        if path.is_dir() {
            walk(&path, out)?;
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
    Ok(())
}
