//! The macro library: reusable circuits ([`BlockDefinition`]s) kept as files
//! in a folder, so they can be browsed, searched, renamed and shared between
//! projects and between people (DESIGN §7, "Where blocks live").
//!
//! - A macro is one `<name>.kcmacro` file ([`MacroFile`]): JSON that wraps a
//!   `novo_core::BlockDefinition` as it is, plus display fields (author, tags)
//!   and a `revision` that grows each time the file is replaced.
//! - A [`MacroLibrary`] is a folder of them; sub-folders are the user's
//!   folders. The per-user one is [`user_macros_dir`], a company folder is any
//!   readable path.
//! - A pack (`.kcmacropack`, [`MacroPack`]) carries several macros in one
//!   file so a team can pass a set around by mail or a share.
//!
//! Capture and placement are `novo_core` commands (`DefineBlock`,
//! `PlaceBlock`); this module only stores and finds definitions. Nothing here
//! touches a project. Every write goes through a temporary file and a rename,
//! and everything stays inside the library folder or the file the caller
//! names, so no admin rights are involved.

use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::UNIX_EPOCH;

use novo_core::{BlockDefinition, BlockSource};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::search::Matcher;

mod starter;
pub use starter::{STARTER_FOLDER, starter_macros};

/// The `format` value of a macro file.
pub const MACRO_FORMAT: &str = "kopercad-macro";
/// The `format` value of a pack file.
pub const MACRO_PACK_FORMAT: &str = "kopercad-macro-pack";
/// Version written to macro and pack files. A newer file is refused, not
/// guessed at.
pub const MACRO_FORMAT_VERSION: u32 = 1;
/// File extension of one macro, without the dot.
pub const MACRO_EXTENSION: &str = "kcmacro";
/// File extension of a pack, without the dot.
pub const MACRO_PACK_EXTENSION: &str = "kcmacropack";

#[derive(Debug, Error)]
pub enum MacroError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path}: not a macro file ({reason})")]
    Format { path: PathBuf, reason: String },
    #[error(
        "{path}: written by a newer KoperCAD (macro format {found}, this one reads up to {max})"
    )]
    TooNew { path: PathBuf, found: u32, max: u32 },
    #[error("'{0}' is not a usable macro or folder name")]
    BadName(String),
    #[error("'{0}' already exists")]
    Exists(String),
    #[error("'{0}' was not found")]
    NotFound(String),
    #[error("folder '{0}' is not empty")]
    NotEmpty(String),
}

type Result<T, E = MacroError> = std::result::Result<T, E>;

/// One macro file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroFile {
    pub format: String,
    pub version: u32,
    /// Display name; equals `definition.name`.
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Grows by one each time the library replaces the file; this is the
    /// `version` of the [`BlockSource`] a placement remembers.
    #[serde(default = "first_revision")]
    pub revision: u32,
    pub definition: BlockDefinition,
    /// Fields this version does not know (later additions such as `parts`
    /// or `cabinet`): kept as read and written back unchanged.
    #[serde(flatten)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

fn first_revision() -> u32 {
    1
}

impl MacroFile {
    pub fn new(definition: BlockDefinition) -> Self {
        MacroFile {
            format: MACRO_FORMAT.into(),
            version: MACRO_FORMAT_VERSION,
            name: definition.name.clone(),
            description: definition.description.clone(),
            author: String::new(),
            tags: Vec::new(),
            revision: 1,
            definition,
            extra: Default::default(),
        }
    }

    pub fn with_author(mut self, author: impl Into<String>) -> Self {
        self.author = author.into();
        self
    }

    pub fn with_tags(mut self, tags: &[&str]) -> Self {
        self.tags = tags.iter().map(|t| t.to_string()).collect();
        self
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("macro files always serialize") + "\n"
    }

    pub fn from_json(text: &str, path: &Path) -> Result<Self> {
        let bad = |reason: String| MacroError::Format {
            path: path.to_path_buf(),
            reason,
        };
        let value: serde_json::Value =
            serde_json::from_str(text).map_err(|e| bad(e.to_string()))?;
        if value.get("format").and_then(|f| f.as_str()) != Some(MACRO_FORMAT) {
            return Err(bad("format is not kopercad-macro".into()));
        }
        let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if version > MACRO_FORMAT_VERSION {
            return Err(MacroError::TooNew {
                path: path.to_path_buf(),
                found: version,
                max: MACRO_FORMAT_VERSION,
            });
        }
        serde_json::from_value(value).map_err(|e| bad(e.to_string()))
    }
}

/// Several macros in one file, each with the folder it belongs in.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroPack {
    pub format: String,
    pub version: u32,
    pub macros: Vec<PackEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackEntry {
    /// Library folder, `/`-separated, empty for the top level.
    #[serde(default)]
    pub folder: String,
    pub file: MacroFile,
}

/// What to do when an imported macro has the name of one already there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnConflict {
    /// Leave the existing one, report the import as skipped.
    Skip,
    /// Keep both: the new one gets " 2", " 3", ... after its name.
    KeepBoth,
    /// Replace the existing one; its revision goes up by one.
    Replace,
}

/// The result of one imported macro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Imported {
    Added(String),
    Replaced(String),
    Skipped(String),
}

/// One row of the browser.
#[derive(Debug, Clone, PartialEq)]
pub struct MacroEntry {
    /// Path relative to the library, `/`-separated, with extension. Stable
    /// identifier for rename, delete, load.
    pub path: String,
    /// Folder part of `path`, empty for the top level.
    pub folder: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub tags: Vec<String>,
    pub revision: u32,
    pub parameters: usize,
    pub members: usize,
    /// Seconds since the Unix epoch, if the file system says.
    pub modified: Option<u64>,
}

/// Where the user's own macros live: `%APPDATA%\KoperCAD\Macros` on Windows,
/// `$XDG_DATA_HOME/novoelectric/macros` (or `~/.local/share/...`) elsewhere.
/// Always inside the user's profile. `None` if no profile folder is known.
pub fn user_macros_dir() -> Option<PathBuf> {
    let var = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if cfg!(windows) {
        var("APPDATA").map(|d| PathBuf::from(d).join("KoperCAD").join("Macros"))
    } else {
        var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| var("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
            .map(|d| d.join("novoelectric").join("macros"))
    }
}

/// A folder of macro files.
#[derive(Debug, Clone)]
pub struct MacroLibrary {
    root: PathBuf,
    label: String,
}

impl MacroLibrary {
    /// A library in `root` (created on the first write). `label` names it in
    /// [`BlockSource::library`], e.g. `user` or `company`.
    pub fn new(root: impl Into<PathBuf>, label: impl Into<String>) -> Self {
        MacroLibrary {
            root: root.into(),
            label: label.into(),
        }
    }

    /// The per-user library, if the profile folder is known.
    pub fn user() -> Option<Self> {
        user_macros_dir().map(|d| Self::new(d, "user"))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Every readable macro, sorted by folder then name. Files that cannot be
    /// read as macros come back in the second list with the reason, so the
    /// browser can say so instead of hiding them.
    pub fn list(&self) -> (Vec<MacroEntry>, Vec<MacroError>) {
        let mut entries = Vec::new();
        let mut problems = Vec::new();
        self.walk(&self.root, "", &mut entries, &mut problems);
        entries.sort_by(|a, b| {
            (a.folder.to_lowercase(), a.name.to_lowercase())
                .cmp(&(b.folder.to_lowercase(), b.name.to_lowercase()))
        });
        (entries, problems)
    }

    fn walk(
        &self,
        dir: &Path,
        folder: &str,
        out: &mut Vec<MacroEntry>,
        problems: &mut Vec<MacroError>,
    ) {
        let Ok(read) = fs::read_dir(dir) else {
            return;
        };
        for item in read.flatten() {
            let path = item.path();
            let name = item.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || item.file_type().is_ok_and(|t| t.is_symlink()) {
                continue;
            }
            if path.is_dir() {
                let sub = join(folder, &name);
                self.walk(&path, &sub, out, problems);
            } else if has_extension(&path, MACRO_EXTENSION) {
                match read_macro(&path) {
                    Ok(file) => out.push(MacroEntry {
                        path: join(folder, &name),
                        folder: folder.to_string(),
                        name: file.name,
                        description: file.description,
                        author: file.author,
                        tags: file.tags,
                        revision: file.revision,
                        parameters: file.definition.parameters.len(),
                        members: file.definition.members.len(),
                        modified: item
                            .metadata()
                            .ok()
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs()),
                    }),
                    Err(e) => problems.push(e),
                }
            }
        }
    }

    /// Folders, including empty ones, `/`-separated and sorted.
    pub fn folders(&self) -> Vec<String> {
        fn go(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
            let Ok(read) = fs::read_dir(dir) else { return };
            for item in read.flatten() {
                if item.file_type().is_ok_and(|t| t.is_dir())
                    && !item.file_name().to_string_lossy().starts_with('.')
                {
                    let name = join(prefix, &item.file_name().to_string_lossy());
                    out.insert(name.clone());
                    go(&item.path(), &name, out);
                }
            }
        }
        let mut out = BTreeSet::new();
        go(&self.root, "", &mut out);
        out.into_iter().collect()
    }

    /// Entries that match every word of `query` in name, description, tags,
    /// author or folder; name hits first. An empty query returns everything.
    pub fn search(&self, query: &str) -> Vec<MacroEntry> {
        let matcher = Matcher::new(query);
        let (mut entries, _) = self.list();
        if matcher.is_empty() {
            return entries;
        }
        entries.retain(|e| {
            let tags = e.tags.join(" ");
            matcher.matches([
                e.name.as_str(),
                e.description.as_str(),
                tags.as_str(),
                e.author.as_str(),
                e.folder.as_str(),
            ])
        });
        entries.sort_by_key(|e| !matcher.matches([e.name.as_str()]));
        entries
    }

    pub fn load(&self, path: &str) -> Result<MacroFile> {
        let full = self.resolve(path)?;
        if !full.is_file() {
            return Err(MacroError::NotFound(path.into()));
        }
        read_macro(&full)
    }

    /// The definition to hand to `Command::PlaceBlock`'s project copy, with
    /// its source set so a newer library version is offered later.
    pub fn definition_for_project(&self, path: &str) -> Result<BlockDefinition> {
        let file = self.load(path)?;
        let mut def = file.definition;
        def.source = Some(BlockSource {
            library: format!("{}/{}", self.label, path),
            version: file.revision,
        });
        Ok(def)
    }

    /// Saves a new macro into `folder`. Fails if the name is taken.
    /// Returns the library path.
    pub fn save(&self, folder: &str, file: &MacroFile) -> Result<String> {
        let path = self.target(folder, &file.name)?;
        if self.resolve(&path)?.exists() {
            return Err(MacroError::Exists(file.name.clone()));
        }
        self.write(&path, file)?;
        Ok(path)
    }

    /// Writes `file` over the macro at `path` (same name), bumping its
    /// revision.
    pub fn replace(&self, path: &str, file: &MacroFile) -> Result<()> {
        let old = self.load(path)?;
        let mut next = file.clone();
        next.revision = old.revision + 1;
        self.write(path, &next)
    }

    /// Gives a macro a new name (file and contents). Returns the new path.
    pub fn rename(&self, path: &str, new_name: &str) -> Result<String> {
        let mut file = self.load(path)?;
        let folder = parent(path);
        let new_path = self.target(&folder, new_name)?;
        if new_path != path && self.resolve(&new_path)?.exists() {
            return Err(MacroError::Exists(new_name.into()));
        }
        file.name = new_name.trim().into();
        file.definition.name = file.name.clone();
        self.write(&new_path, &file)?;
        if new_path != path {
            self.delete(path)?;
        }
        Ok(new_path)
    }

    /// Moves a macro to another folder (`""` for the top level).
    pub fn move_to(&self, path: &str, folder: &str) -> Result<String> {
        let file = self.load(path)?;
        let new_path = self.target(folder, &file.name)?;
        if new_path == path {
            return Ok(new_path);
        }
        if self.resolve(&new_path)?.exists() {
            return Err(MacroError::Exists(file.name));
        }
        let from = self.resolve(path)?;
        let to = self.resolve(&new_path)?;
        make_parent(&to)?;
        fs::rename(&from, &to).map_err(|e| io_err(&from, e))?;
        Ok(new_path)
    }

    /// Removes a macro for good. [`MacroLibrary::trash`] is the gentler way.
    pub fn delete(&self, path: &str) -> Result<()> {
        let full = self.resolve(path)?;
        fs::remove_file(&full).map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => MacroError::NotFound(path.into()),
            _ => io_err(&full, e),
        })
    }

    /// Moves a macro into the library's hidden `.trash` folder, so a wrong
    /// delete can be undone with [`MacroLibrary::restore`]. Returns the
    /// trash path.
    pub fn trash(&self, path: &str) -> Result<String> {
        let full = self.resolve(path)?;
        if !full.is_file() {
            return Err(MacroError::NotFound(path.into()));
        }
        let stem = path
            .trim_end_matches(&format!(".{MACRO_EXTENSION}"))
            .replace('/', " - ");
        let mut n = 1;
        let slot = loop {
            let name = if n == 1 {
                format!("{stem}.{MACRO_EXTENSION}")
            } else {
                format!("{stem} {n}.{MACRO_EXTENSION}")
            };
            if !self.root.join(".trash").join(&name).exists() {
                break name;
            }
            n += 1;
        };
        let to = self.root.join(".trash").join(&slot);
        make_parent(&to)?;
        fs::rename(&full, &to).map_err(|e| io_err(&full, e))?;
        Ok(format!(".trash/{slot}"))
    }

    /// Macros in the trash, as library paths starting `.trash/`.
    pub fn trashed(&self) -> Vec<String> {
        let Ok(read) = fs::read_dir(self.root.join(".trash")) else {
            return Vec::new();
        };
        let mut out: Vec<String> = read
            .flatten()
            .filter(|i| has_extension(&i.path(), MACRO_EXTENSION))
            .map(|i| format!(".trash/{}", i.file_name().to_string_lossy()))
            .collect();
        out.sort();
        out
    }

    /// Puts a trashed macro back into `folder`. Returns its new path.
    pub fn restore(&self, trash_path: &str, folder: &str) -> Result<String> {
        if !trash_path.starts_with(".trash/") {
            return Err(MacroError::BadName(trash_path.into()));
        }
        let name = trash_path.trim_start_matches(".trash/");
        let src = self.root.join(".trash").join(clean_name(name)?);
        let file = read_macro(&src)?;
        let new_path = self.target(folder, &file.name)?;
        if self.resolve(&new_path)?.exists() {
            return Err(MacroError::Exists(file.name));
        }
        let to = self.resolve(&new_path)?;
        make_parent(&to)?;
        fs::rename(&src, &to).map_err(|e| io_err(&src, e))?;
        Ok(new_path)
    }

    pub fn create_folder(&self, folder: &str) -> Result<()> {
        let full = self.resolve_dir(folder)?;
        fs::create_dir_all(&full).map_err(|e| io_err(&full, e))
    }

    /// Removes an empty folder.
    pub fn delete_folder(&self, folder: &str) -> Result<()> {
        let full = self.resolve_dir(folder)?;
        let empty = fs::read_dir(&full)
            .map_err(|e| io_err(&full, e))?
            .next()
            .is_none();
        if !empty {
            return Err(MacroError::NotEmpty(folder.into()));
        }
        fs::remove_dir(&full).map_err(|e| io_err(&full, e))
    }

    /// Copies one macro out as a `.kcmacro` file at `dest`.
    pub fn export(&self, path: &str, dest: &Path) -> Result<()> {
        let file = self.load(path)?;
        write_atomic(dest, &file.to_json())
    }

    /// Writes the macros at `paths` (library paths) into one pack file.
    pub fn export_pack(&self, paths: &[&str], dest: &Path) -> Result<()> {
        let mut macros = Vec::new();
        for path in paths {
            macros.push(PackEntry {
                folder: parent(path),
                file: self.load(path)?,
            });
        }
        let pack = MacroPack {
            format: MACRO_PACK_FORMAT.into(),
            version: MACRO_FORMAT_VERSION,
            macros,
        };
        let text = serde_json::to_string_pretty(&pack).expect("packs always serialize") + "\n";
        write_atomic(dest, &text)
    }

    /// Imports a `.kcmacro` file (into `folder`) or a `.kcmacropack` (each
    /// macro into its own folder, below `folder`).
    pub fn import(
        &self,
        source: &Path,
        folder: &str,
        on_conflict: OnConflict,
    ) -> Result<Vec<Imported>> {
        let text = fs::read_to_string(source).map_err(|e| io_err(source, e))?;
        let items: Vec<(String, MacroFile)> = if has_extension(source, MACRO_PACK_EXTENSION) {
            let pack: MacroPack = serde_json::from_str(&text).map_err(|e| MacroError::Format {
                path: source.to_path_buf(),
                reason: e.to_string(),
            })?;
            if pack.format != MACRO_PACK_FORMAT {
                return Err(MacroError::Format {
                    path: source.to_path_buf(),
                    reason: "format is not kopercad-macro-pack".into(),
                });
            }
            if pack.version > MACRO_FORMAT_VERSION {
                return Err(MacroError::TooNew {
                    path: source.to_path_buf(),
                    found: pack.version,
                    max: MACRO_FORMAT_VERSION,
                });
            }
            pack.macros
                .into_iter()
                .map(|e| (join(folder, &e.folder), e.file))
                .collect()
        } else {
            vec![(folder.to_string(), MacroFile::from_json(&text, source)?)]
        };

        let mut done = Vec::new();
        for (target_folder, mut file) in items {
            let path = self.target(&target_folder, &file.name)?;
            let exists = self.resolve(&path)?.exists();
            let result = match (exists, on_conflict) {
                (false, _) => {
                    self.write(&path, &file)?;
                    Imported::Added(path)
                }
                (true, OnConflict::Skip) => Imported::Skipped(path),
                (true, OnConflict::Replace) => {
                    self.replace(&path, &file)?;
                    Imported::Replaced(path)
                }
                (true, OnConflict::KeepBoth) => {
                    let base = file.name.clone();
                    let mut n = 2;
                    let path = loop {
                        let name = format!("{base} {n}");
                        let p = self.target(&target_folder, &name)?;
                        if !self.resolve(&p)?.exists() {
                            file.name = name.clone();
                            file.definition.name = name;
                            break p;
                        }
                        n += 1;
                    };
                    self.write(&path, &file)?;
                    Imported::Added(path)
                }
            };
            done.push(result);
        }
        Ok(done)
    }

    // ---- paths ----

    /// The library path a macro called `name` gets in `folder`.
    fn target(&self, folder: &str, name: &str) -> Result<String> {
        let stem = clean_name(name)?;
        let folder = clean_folder(folder)?;
        Ok(join(&folder, &format!("{stem}.{MACRO_EXTENSION}")))
    }

    fn resolve(&self, path: &str) -> Result<PathBuf> {
        let clean = clean_folder(path)?;
        if clean.is_empty() {
            return Err(MacroError::BadName(path.into()));
        }
        Ok(self.root.join(clean))
    }

    fn resolve_dir(&self, folder: &str) -> Result<PathBuf> {
        let clean = clean_folder(folder)?;
        if clean.is_empty() {
            return Err(MacroError::BadName(folder.into()));
        }
        Ok(self.root.join(clean))
    }

    fn write(&self, path: &str, file: &MacroFile) -> Result<()> {
        write_atomic(&self.resolve(path)?, &file.to_json())
    }
}

fn read_macro(path: &Path) -> Result<MacroFile> {
    let text = fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    MacroFile::from_json(&text, path)
}

fn io_err(path: &Path, source: io::Error) -> MacroError {
    MacroError::Io {
        path: path.to_path_buf(),
        source,
    }
}

fn make_parent(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| io_err(dir, e))?;
    }
    Ok(())
}

fn write_atomic(dest: &Path, text: &str) -> Result<()> {
    make_parent(dest)?;
    let tmp = dest.with_extension("tmp-write");
    fs::write(&tmp, text).map_err(|e| io_err(&tmp, e))?;
    fs::rename(&tmp, dest).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        io_err(dest, e)
    })
}

fn has_extension(path: &Path, ext: &str) -> bool {
    path.extension()
        .is_some_and(|e| e.to_string_lossy().eq_ignore_ascii_case(ext))
}

fn join(folder: &str, name: &str) -> String {
    if folder.is_empty() {
        name.to_string()
    } else if name.is_empty() {
        folder.to_string()
    } else {
        format!("{folder}/{name}")
    }
}

fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map(|(p, _)| p.to_string())
        .unwrap_or_default()
}

const RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// A name that is a safe file stem on Windows and elsewhere, or an error.
/// Characters Windows forbids become `_`; dots and spaces at the end go.
fn clean_name(name: &str) -> Result<String> {
    let mapped: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let stem = mapped.trim_end_matches(['.', ' ']).to_string();
    let upper = stem.split('.').next().unwrap_or("").to_uppercase();
    if stem.is_empty() || stem.len() > 120 || RESERVED.contains(&upper.as_str()) {
        return Err(MacroError::BadName(name.into()));
    }
    Ok(stem)
}

/// A relative `/`-separated folder path with every part checked; `..`, drive
/// letters and absolute paths are refused.
fn clean_folder(folder: &str) -> Result<String> {
    let mut parts = Vec::new();
    for part in folder.split(['/', '\\']).filter(|p| !p.is_empty()) {
        if Path::new(part)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(MacroError::BadName(folder.into()));
        }
        parts.push(clean_name(part).map_err(|_| MacroError::BadName(folder.into()))?);
    }
    Ok(parts.join("/"))
}
