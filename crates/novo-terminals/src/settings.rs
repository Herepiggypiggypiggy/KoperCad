//! The project rule for terminals that share a name.
//!
//! The rule lives in the project info properties, so it is saved with the
//! project, undoable, and readable from scripts:
//!
//! | Key | Values | Default |
//! | --- | --- | --- |
//! | `terminals.same_name` | `join`, `refuse` | `join` |
//! | `terminals.join_link` | `connector`, `jumper` | `connector` |

use novo_core::{Project, Result, Tx, Value};

/// Property key of [`SameName`] in the project info.
pub const SAME_NAME_KEY: &str = "terminals.same_name";
/// Property key of [`JoinLink`] in the project info.
pub const JOIN_LINK_KEY: &str = "terminals.join_link";

/// What happens when one terminal name (`-X2:1`) is used in several places.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SameName {
    /// The occurrences are one terminal: one row in the strip editor, one
    /// node electrically, built from 1-in/1-out clamps linked together.
    #[default]
    Join,
    /// Every occurrence after the first is flagged as a duplicate.
    Refuse,
}

/// How the clamps of a joined terminal are linked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum JoinLink {
    /// A connector piece between neighbouring clamps.
    #[default]
    Connector,
    /// A plug-in jumper bar across the clamps.
    Jumper,
}

impl SameName {
    fn as_str(self) -> &'static str {
        match self {
            SameName::Join => "join",
            SameName::Refuse => "refuse",
        }
    }
}

impl JoinLink {
    fn as_str(self) -> &'static str {
        match self {
            JoinLink::Connector => "connector",
            JoinLink::Jumper => "jumper",
        }
    }

    /// Plain name for screens and reports.
    pub fn label(self) -> &'static str {
        match self {
            JoinLink::Connector => "connector",
            JoinLink::Jumper => "jumper",
        }
    }
}

/// The terminal rules of a project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Settings {
    pub same_name: SameName,
    pub join_link: JoinLink,
}

impl Settings {
    /// Reads the rules from the project. Missing or unknown values give the
    /// defaults, so an old project behaves like a new one.
    pub fn of(project: &Project) -> Settings {
        let props = &project.info().properties;
        let text = |key: &str| match props.get(key) {
            Some(Value::Text(t)) => t.trim().to_ascii_lowercase(),
            _ => String::new(),
        };
        Settings {
            same_name: match text(SAME_NAME_KEY).as_str() {
                "refuse" => SameName::Refuse,
                _ => SameName::Join,
            },
            join_link: match text(JOIN_LINK_KEY).as_str() {
                "jumper" => JoinLink::Jumper,
                _ => JoinLink::Connector,
            },
        }
    }

    /// Stores the rules in the project as part of a transaction.
    pub fn store(self, tx: &mut Tx<'_>) -> Result<()> {
        tx.set_info(|info| {
            info.properties
                .insert(SAME_NAME_KEY.into(), self.same_name.as_str().into());
            info.properties
                .insert(JOIN_LINK_KEY.into(), self.join_link.as_str().into());
        })
    }
}

impl Settings {
    /// The rule as one plain sentence, for the Terminals drawer.
    pub fn sentence(self) -> String {
        match self.same_name {
            SameName::Join => format!(
                "Terminals with the same name are one terminal, joined with {}s.",
                self.join_link.label()
            ),
            SameName::Refuse => "Each terminal name may be used once per strip.".to_owned(),
        }
    }
}
