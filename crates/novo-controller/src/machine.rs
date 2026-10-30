//! Machine targets: a signal bound to a port of a module instance of the machine model (lane L5b).
//!
//! The text of a target is `module@port`, for example `hoist/=A+B-M1@up`. The module part is a
//! path of segments separated by `/` (`runway1/bridge2/crab1/hoistA`); its last segment names the
//! module instance, the segments before it only say where it sits. The port is a lower case word
//! (`up`, `down`, `fast`, `forward`, `permit`, `deadman`, ...).
//!
//! This file knows the syntax and a plain-data [`MachineCatalog`] (which modules and ports exist
//! and what kind each port is). It does not know the machine engine: the host builds the catalog
//! from the project's machine model and maps the resolved levels onto the engine's inputs. That
//! keeps the crate free of the data model and wasm-clean.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Separates the module path from the port name.
pub const PORT_SEP: char = '@';
/// Separates the segments of a module path.
pub const SEGMENT_SEP: char = '/';
/// Longest text of a machine target.
pub const MAX_LEN: usize = 200;

/// Why a machine target text is not valid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathError {
    Empty,
    TooLong,
    /// A segment between two `/` (or at the ends) is empty.
    EmptySegment,
    /// A character that cannot be in a segment or a port name.
    BadChar(char),
    /// More than one `@`.
    ManyPorts,
    /// The port name after `@` is empty or not made of `a-z`, `0-9` and `_`.
    BadPort(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Empty => write!(f, "the machine path is empty"),
            PathError::TooLong => write!(f, "the machine path is longer than {MAX_LEN} characters"),
            PathError::EmptySegment => write!(f, "the machine path has an empty segment"),
            PathError::BadChar(c) => write!(f, "the machine path may not contain {c:?}"),
            PathError::ManyPorts => write!(f, "the machine path has more than one {PORT_SEP}"),
            PathError::BadPort(p) => write!(
                f,
                "the port name {p:?} must be lower case letters, digits and underscores"
            ),
        }
    }
}

impl std::error::Error for PathError {}

/// A parsed machine target: a module path and, once chosen, a port of that module.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MachinePath {
    module: String,
    port: Option<String>,
}

impl MachinePath {
    /// `module@port` or just `module` (a legacy target that names no port yet).
    pub fn parse(text: &str) -> Result<Self, PathError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(PathError::Empty);
        }
        if text.chars().count() > MAX_LEN {
            return Err(PathError::TooLong);
        }
        let mut parts = text.split(PORT_SEP);
        let module = parts.next().unwrap_or_default();
        let port = parts.next();
        if parts.next().is_some() {
            return Err(PathError::ManyPorts);
        }
        Self::build(module, port)
    }

    /// A module path and a port name, validated.
    pub fn new(module: &str, port: &str) -> Result<Self, PathError> {
        Self::build(module.trim(), Some(port.trim()))
    }

    fn build(module: &str, port: Option<&str>) -> Result<Self, PathError> {
        if module.is_empty() {
            return Err(PathError::Empty);
        }
        for segment in module.split(SEGMENT_SEP) {
            if segment.is_empty() {
                return Err(PathError::EmptySegment);
            }
            if let Some(c) = segment
                .chars()
                .find(|c| c.is_whitespace() || c.is_control() || *c == PORT_SEP)
            {
                return Err(PathError::BadChar(c));
            }
        }
        let port = match port {
            None => None,
            Some(p) => {
                if p.is_empty()
                    || !p
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                {
                    return Err(PathError::BadPort(p.to_owned()));
                }
                Some(p.to_owned())
            }
        };
        Ok(MachinePath {
            module: module.to_owned(),
            port,
        })
    }

    /// The module path, without the port.
    pub fn module(&self) -> &str {
        &self.module
    }

    /// The module instance: the last segment of the module path.
    pub fn module_id(&self) -> &str {
        self.module
            .rsplit(SEGMENT_SEP)
            .next()
            .unwrap_or(&self.module)
    }

    pub fn port(&self) -> Option<&str> {
        self.port.as_deref()
    }
}

impl fmt::Display for MachinePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.port {
            Some(p) => write!(f, "{}{PORT_SEP}{p}", self.module),
            None => write!(f, "{}", self.module),
        }
    }
}

/// What a port does with the level it gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    /// A held command: high while the function is wanted (up, down, fast, forward, brake release).
    Command,
    /// An E-stop or safety chain: high means the module may run, low stops it. Bind a chain
    /// signal (ungated, `combine = "and"` when shared).
    Chain,
    /// An enabling input such as a dead-man bar: the module only moves while it is high.
    Enable,
}

impl PortKind {
    pub fn word(self) -> &'static str {
        match self {
            PortKind::Command => "command",
            PortKind::Chain => "chain",
            PortKind::Enable => "enable",
        }
    }
}

/// One port of a module, as the catalog lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortEntry {
    pub name: String,
    pub label: String,
    pub kind: PortKind,
}

/// One module instance and its ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleEntry {
    /// The canonical path, such as `hoist/=A+B-M1`.
    pub path: String,
    /// Words for the picker, such as "Hoist =A+B-M1 (two-speed)".
    pub label: String,
    pub ports: Vec<PortEntry>,
}

impl ModuleEntry {
    pub fn port(&self, name: &str) -> Option<&PortEntry> {
        self.ports.iter().find(|p| p.name == name)
    }

    fn id(&self) -> &str {
        self.path.rsplit(SEGMENT_SEP).next().unwrap_or(&self.path)
    }
}

/// The modules and ports a drawing's machine model offers, built by the host.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MachineCatalog {
    pub modules: Vec<ModuleEntry>,
}

impl MachineCatalog {
    /// The module a path names: the canonical path, or any path whose last segment is the module
    /// instance (the segments before it describe where it sits and are not checked).
    pub fn module(&self, path: &MachinePath) -> Option<&ModuleEntry> {
        self.modules
            .iter()
            .find(|m| m.path == path.module())
            .or_else(|| self.modules.iter().find(|m| m.id() == path.module_id()))
    }

    /// The canonical `module@port` for a path that resolves, `None` when it does not.
    pub fn canonical(&self, path: &MachinePath) -> Option<MachinePath> {
        let module = self.module(path)?;
        let port = path.port()?;
        module.port(port)?;
        MachinePath::new(&module.path, port).ok()
    }

    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_parses_displays_and_round_trips() {
        for text in [
            "hoist/=A+B-M1@up",
            "runway1/bridge2/crab1/hoistA@fast",
            "crane@permit",
            "runway1/bridge2",
        ] {
            let p = MachinePath::parse(text).unwrap();
            assert_eq!(p.to_string(), text);
            assert_eq!(MachinePath::parse(&p.to_string()).unwrap(), p);
        }
        let p = MachinePath::parse("runway1/bridge2/crab1/hoistA@fast").unwrap();
        assert_eq!(p.module(), "runway1/bridge2/crab1/hoistA");
        assert_eq!(p.module_id(), "hoistA");
        assert_eq!(p.port(), Some("fast"));
        assert_eq!(MachinePath::parse(" a/b ").unwrap().port(), None);
    }

    #[test]
    fn bad_targets_are_refused_with_the_reason() {
        let cases = [
            ("", PathError::Empty),
            ("   ", PathError::Empty),
            ("a//b@up", PathError::EmptySegment),
            ("/a@up", PathError::EmptySegment),
            ("a/@up", PathError::EmptySegment),
            ("a b@up", PathError::BadChar(' ')),
            ("a@up@down", PathError::ManyPorts),
            ("a@", PathError::BadPort(String::new())),
            ("a@Up", PathError::BadPort("Up".into())),
            ("@up", PathError::Empty),
        ];
        for (text, error) in cases {
            assert_eq!(MachinePath::parse(text), Err(error), "{text:?}");
        }
        assert_eq!(
            MachinePath::parse(&"a".repeat(MAX_LEN + 1)),
            Err(PathError::TooLong)
        );
    }

    fn catalog() -> MachineCatalog {
        MachineCatalog {
            modules: vec![ModuleEntry {
                path: "hoist/=A-M1".into(),
                label: "Hoist =A-M1".into(),
                ports: vec![PortEntry {
                    name: "up".into(),
                    label: "Up".into(),
                    kind: PortKind::Command,
                }],
            }],
        }
    }

    #[test]
    fn the_catalog_finds_a_module_by_canonical_path_or_by_its_last_segment() {
        let c = catalog();
        for text in ["hoist/=A-M1@up", "runway1/crab1/=A-M1@up", "=A-M1@up"] {
            let p = MachinePath::parse(text).unwrap();
            assert_eq!(
                c.canonical(&p).unwrap().to_string(),
                "hoist/=A-M1@up",
                "{text}"
            );
        }
        assert!(
            c.canonical(&MachinePath::parse("hoist/=A-M2@up").unwrap())
                .is_none()
        );
        assert!(
            c.canonical(&MachinePath::parse("hoist/=A-M1@down").unwrap())
                .is_none()
        );
        assert!(
            c.canonical(&MachinePath::parse("hoist/=A-M1").unwrap())
                .is_none()
        );
    }
}
