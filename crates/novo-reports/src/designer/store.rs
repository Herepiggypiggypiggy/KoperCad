//! Saving templates: JSON files in a folder (the user's template folder, or
//! any folder) and a single string for the project's own templates.
//!
//! A project carries its templates in the project property
//! [`PROJECT_KEY`], so a file sent to a colleague brings its forms along.
//! The user folder is `%APPDATA%\KoperCAD\templates` (the platform data
//! folder elsewhere), per user, no admin rights. A name used in both places
//! resolves to the project's template.

use std::fs;
use std::path::{Path, PathBuf};

use novo_core::{Project, Value};

use super::template::{FORMAT_VERSION, Template};

/// Project property holding the project's templates (a JSON array).
pub const PROJECT_KEY: &str = "designer.templates";
/// File name suffix of a template file.
pub const EXTENSION: &str = ".kdt.json";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("template file is not valid: {0}")]
    Parse(#[from] serde_json::Error),
    #[error(
        "template was written by a newer version (format {found}, this one reads {FORMAT_VERSION})"
    )]
    TooNew { found: u32 },
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
}

fn check(t: Template) -> Result<Template, StoreError> {
    if t.version > FORMAT_VERSION {
        return Err(StoreError::TooNew { found: t.version });
    }
    Ok(t)
}

/// One template as pretty JSON, ready for a file.
pub fn to_json(template: &Template) -> String {
    serde_json::to_string_pretty(template).unwrap_or_default()
}

pub fn from_json(text: &str) -> Result<Template, StoreError> {
    check(serde_json::from_str(text)?)
}

/// The file name for a template: the name with anything outside letters,
/// digits, `-` and `_` turned into `_`, plus [`EXTENSION`].
pub fn file_name(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    format!(
        "{}{EXTENSION}",
        if stem.is_empty() { "template" } else { &stem }
    )
}

/// Writes the template into `dir` (created if needed); returns the path.
pub fn save_to_dir(dir: &Path, template: &Template) -> Result<PathBuf, StoreError> {
    fs::create_dir_all(dir)?;
    let path = dir.join(file_name(&template.name));
    fs::write(&path, to_json(template))?;
    Ok(path)
}

/// Every template file in `dir`, sorted by name, and the files that could
/// not be read (never silently skipped). A missing folder is empty.
pub fn load_dir(dir: &Path) -> (Vec<Template>, Vec<(PathBuf, StoreError)>) {
    let mut found = Vec::new();
    let mut bad = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return (found, bad);
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with(EXTENSION))
        })
        .collect();
    paths.sort();
    for path in paths {
        match fs::read_to_string(&path)
            .map_err(StoreError::from)
            .and_then(|t| from_json(&t))
        {
            Ok(t) => found.push(t),
            Err(e) => bad.push((path, e)),
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    (found, bad)
}

/// The user's template folder, if the platform has a data folder.
pub fn user_dir() -> Option<PathBuf> {
    Some(dirs::data_dir()?.join("KoperCAD").join("templates"))
}

/// The project's templates as the string stored in [`PROJECT_KEY`].
pub fn to_property(templates: &[Template]) -> String {
    serde_json::to_string(templates).unwrap_or_default()
}

pub fn from_property(text: &str) -> Result<Vec<Template>, StoreError> {
    serde_json::from_str::<Vec<Template>>(text)?
        .into_iter()
        .map(check)
        .collect()
}

/// The templates saved in a project; none if the property is absent.
pub fn project_templates(project: &Project) -> Result<Vec<Template>, StoreError> {
    match project.info().properties.get(PROJECT_KEY) {
        Some(Value::Text(text)) => from_property(text),
        _ => Ok(Vec::new()),
    }
}

/// Where a template came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    Project,
    User,
    Builtin,
}

/// Every template available, project first, then user, then built-in; a
/// name already listed hides the later ones.
pub fn all<'a>(project: &'a [Template], user: &'a [Template]) -> Vec<(Origin, Template)> {
    let builtin = super::builtin::all();
    let mut out: Vec<(Origin, Template)> = Vec::new();
    let sources = project
        .iter()
        .map(|t| (Origin::Project, t.clone()))
        .chain(user.iter().map(|t| (Origin::User, t.clone())))
        .chain(builtin.into_iter().map(|t| (Origin::Builtin, t)));
    for (origin, t) in sources {
        if !out.iter().any(|(_, o)| o.name == t.name) {
            out.push((origin, t));
        }
    }
    out
}

/// The template called `name`, from [`all`].
pub fn resolve(project: &[Template], user: &[Template], name: &str) -> Option<(Origin, Template)> {
    all(project, user).into_iter().find(|(_, t)| t.name == name)
}
