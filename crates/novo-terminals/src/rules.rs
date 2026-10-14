//! The project rule for terminals that share a name.
//!
//! The rule is novo-core's typed [`TerminalRules`] in
//! `ProjectInfo.rules.terminals`, so it is saved with the project, undoable,
//! readable from scripts, and the netlist follows it:
//!
//! - [`SameName::Join`] (new projects): records with the same name and level
//!   on a strip are one electrical point. The strip editor shows them as one
//!   row built from 1-in/1-out clamps linked by [`JoinLink`].
//! - [`SameName::Refuse`]: novo-core refuses a second record with a name
//!   already on the strip ([`novo_core::ModelError::DuplicateTerminal`]).
//! - [`SameName::Off`] (files without a rule): same-named records stay
//!   separate terminals, and [`crate::check`] lists them.
//!
//! Before novo-core had the typed rule, this crate kept it in two project
//! info properties. [`migrate_legacy_rules`] moves a project saved that way
//! onto the typed rule.

use novo_core::{Command, JoinLink, Project, Result, SameName, TerminalRules, Tx, Value};

/// Former property key of the same-name rule (`join` or `refuse`). Only
/// read by [`migrate_legacy_rules`].
pub const LEGACY_SAME_NAME_KEY: &str = "terminals.same_name";
/// Former property key of the join link (`connector` or `jumper`). Only
/// read by [`migrate_legacy_rules`].
pub const LEGACY_JOIN_LINK_KEY: &str = "terminals.join_link";

/// The terminal rules of a project.
pub fn rules(project: &Project) -> TerminalRules {
    project.info().rules.terminals
}

/// Sets the terminal rules through novo-core's project info command, as
/// part of a transaction. Switching to [`SameName::Refuse`] while a strip
/// has same-named records is refused by novo-core; [`crate::join_duplicates`]
/// merges them first.
pub fn set_rules(tx: &mut Tx<'_>, rules: TerminalRules) -> Result<()> {
    let mut info = tx.project().info().clone();
    if info.rules.terminals == rules {
        return Ok(());
    }
    info.rules.terminals = rules;
    tx.execute(Command::SetProjectInfo { info }).map(|_| ())
}

/// Moves the rule from the former project info properties
/// ([`LEGACY_SAME_NAME_KEY`], [`LEGACY_JOIN_LINK_KEY`]) onto the typed rule
/// and removes the properties. Returns whether anything changed. A project
/// without those properties is left alone.
pub fn migrate_legacy_rules(tx: &mut Tx<'_>) -> Result<bool> {
    let props = &tx.project().info().properties;
    let text = |key: &str| match props.get(key) {
        Some(Value::Text(t)) => Some(t.trim().to_ascii_lowercase()),
        Some(_) => Some(String::new()),
        None => None,
    };
    let (same_name, join_link) = (text(LEGACY_SAME_NAME_KEY), text(LEGACY_JOIN_LINK_KEY));
    if same_name.is_none() && join_link.is_none() {
        return Ok(false);
    }
    let mut rules = rules(tx.project());
    match same_name.as_deref() {
        Some("refuse") => rules.same_name = SameName::Refuse,
        // The former rule had no "off": a missing or unknown value was join.
        Some(_) | None => rules.same_name = SameName::Join,
    }
    if let Some(link) = join_link {
        rules.join_link = if link == "jumper" {
            JoinLink::Jumper
        } else {
            JoinLink::Connector
        };
    }
    let mut info = tx.project().info().clone();
    info.properties.remove(LEGACY_SAME_NAME_KEY);
    info.properties.remove(LEGACY_JOIN_LINK_KEY);
    info.rules.terminals = rules;
    tx.execute(Command::SetProjectInfo { info })?;
    Ok(true)
}
