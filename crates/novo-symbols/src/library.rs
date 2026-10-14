//! Loading symbols into a library, from the built-in set or a directory.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::{fs, io};

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
}

impl Library {
    /// The library compiled into this crate from `symbols/`.
    pub fn builtin() -> Result<Library, LoadError> {
        Library::from_sources(BUILTIN.iter().map(|(id, text)| (id.to_string(), *text)))
    }

    /// Loads every `*.toml` file below `dir`; ids are the paths relative to
    /// `dir` without the extension, using `/` separators.
    pub fn load_dir(dir: &Path) -> Result<Library, LoadError> {
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
        Library::from_sources(sources.iter().map(|(id, text)| (id.clone(), text.as_str())))
    }

    /// Builds a library from `(expected id, TOML text)` pairs, parsing and
    /// validating each one.
    pub fn from_sources<'a>(
        sources: impl IntoIterator<Item = (String, &'a str)>,
    ) -> Result<Library, LoadError> {
        let mut symbols = BTreeMap::new();
        let mut issues = Vec::new();
        for (path, text) in sources {
            let symbol = Symbol::from_toml(text).map_err(|error| LoadError::Parse {
                path: path.clone(),
                error,
            })?;
            if symbol.id != path {
                return Err(LoadError::IdMismatch {
                    path,
                    id: symbol.id,
                });
            }
            issues.extend(validate(&symbol));
            symbols.insert(symbol.id.clone(), symbol);
        }
        if issues.is_empty() {
            Ok(Library { symbols })
        } else {
            Err(LoadError::Invalid(issues))
        }
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

    /// Number of symbols.
    pub fn len(&self) -> usize {
        self.symbols.len()
    }

    /// Whether the library is empty.
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty()
    }
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
