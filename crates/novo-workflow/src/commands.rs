//! Every user command, as data. Menus, the keymap and the palette all list
//! commands from here, so a command can't be in one and missing from the
//! other (J8). Ids match the app shell's action ids (`file.new`, ...).

use crate::keymap::{FAVOURITE_COMMANDS, Keymap};

/// One command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    /// Stable id, used by the keymap, recents and scripts.
    pub id: &'static str,
    pub label: &'static str,
    /// Menu it sits in, shown as a prefix in the palette.
    pub menu: &'static str,
    /// Extra words the palette matches ("renumber" also finds "numbering").
    pub keywords: &'static [&'static str],
    /// What it needs before it can run.
    pub needs: Needs,
}

/// What a command needs to be available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Needs {
    Nothing,
    Project,
    Page,
    Selection,
    /// One selected device (rename, partner, references).
    Device,
    Undo,
    Redo,
    Clipboard,
    History(Direction),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Back,
    Forward,
}

/// The state availability is decided on. The app fills it once per frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    pub has_project: bool,
    pub has_page: bool,
    pub selection: usize,
    /// Exactly one device is selected (or under the pointer for G).
    pub has_device: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub clipboard: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
}

/// Whether a command can run now; the error is the reason shown greyed in
/// menus and the palette.
pub fn availability(command: &CommandInfo, ctx: &Context) -> Result<(), &'static str> {
    let check = |ok: bool, reason| if ok { Ok(()) } else { Err(reason) };
    match command.needs {
        Needs::Nothing => Ok(()),
        Needs::Project => check(ctx.has_project, "No project is open"),
        Needs::Page => check(ctx.has_page, "No page is open"),
        Needs::Selection => check(ctx.selection > 0, "Nothing is selected"),
        Needs::Device => check(ctx.has_device, "Select one device first"),
        Needs::Undo => check(ctx.can_undo, "Nothing to undo"),
        Needs::Redo => check(ctx.can_redo, "Nothing to redo"),
        Needs::Clipboard => check(ctx.clipboard, "The clipboard is empty"),
        Needs::History(Direction::Back) => check(ctx.can_go_back, "No earlier jump"),
        Needs::History(Direction::Forward) => check(ctx.can_go_forward, "No later jump"),
    }
}

const fn cmd(
    id: &'static str,
    label: &'static str,
    menu: &'static str,
    keywords: &'static [&'static str],
    needs: Needs,
) -> CommandInfo {
    CommandInfo {
        id,
        label,
        menu,
        keywords,
        needs,
    }
}

/// Every command, in menu order.
pub const COMMANDS: &[CommandInfo] = &[
    cmd(
        "file.new",
        "New project",
        "File",
        &["create"],
        Needs::Nothing,
    ),
    cmd(
        "file.open",
        "Open project",
        "File",
        &["load"],
        Needs::Nothing,
    ),
    cmd("file.save", "Save", "File", &[], Needs::Project),
    cmd("file.save_as", "Save as", "File", &["copy"], Needs::Project),
    cmd("file.close", "Close project", "File", &[], Needs::Project),
    cmd("file.quit", "Quit", "File", &["exit"], Needs::Nothing),
    cmd("edit.undo", "Undo", "Edit", &[], Needs::Undo),
    cmd("edit.redo", "Redo", "Edit", &[], Needs::Redo),
    cmd("edit.copy", "Copy", "Edit", &[], Needs::Selection),
    cmd("edit.cut", "Cut", "Edit", &[], Needs::Selection),
    cmd(
        "edit.paste",
        "Paste with next free tags",
        "Edit",
        &["smart"],
        Needs::Clipboard,
    ),
    cmd(
        "edit.paste_keep_tags",
        "Paste with the original tags",
        "Edit",
        &[],
        Needs::Clipboard,
    ),
    cmd(
        "edit.duplicate",
        "Smart duplicate",
        "Edit",
        &["copy", "clone"],
        Needs::Selection,
    ),
    cmd(
        "edit.select_all",
        "Select all on page",
        "Edit",
        &[],
        Needs::Page,
    ),
    cmd(
        "edit.delete",
        "Delete",
        "Edit",
        &["remove"],
        Needs::Selection,
    ),
    cmd(
        "edit.rename",
        "Rename device",
        "Edit",
        &["tag", "edit tag"],
        Needs::Device,
    ),
    cmd(
        "edit.renumber",
        "Renumber devices",
        "Edit",
        &["numbering", "number"],
        Needs::Project,
    ),
    cmd(
        "edit.rotate",
        "Rotate 90°",
        "Edit",
        &["turn"],
        Needs::Selection,
    ),
    cmd("edit.mirror", "Mirror", "Edit", &["flip"], Needs::Selection),
    cmd(
        "edit.next_orientation",
        "Next orientation",
        "Edit",
        &["tab", "turn"],
        Needs::Selection,
    ),
    cmd(
        "edit.previous_orientation",
        "Previous orientation",
        "Edit",
        &[],
        Needs::Selection,
    ),
    cmd("edit.escape", "Cancel", "Edit", &["escape"], Needs::Nothing),
    cmd(
        "tool.repeat",
        "Repeat last tool",
        "Insert",
        &["again"],
        Needs::Page,
    ),
    cmd(
        "tool.insert",
        "Insert symbol",
        "Insert",
        &["place", "device", "component"],
        Needs::Page,
    ),
    cmd(
        "tool.wire",
        "Wire",
        "Insert",
        &["connect", "line"],
        Needs::Page,
    ),
    cmd(
        "tool.terminals",
        "Terminals",
        "Insert",
        &["clamp", "strip"],
        Needs::Page,
    ),
    cmd("tool.cable", "Cable", "Insert", &["cores"], Needs::Page),
    cmd(
        "tool.potential",
        "Potential or interruption point",
        "Insert",
        &["arrow", "off page"],
        Needs::Page,
    ),
    cmd("tool.move", "Move selection", "Edit", &[], Needs::Selection),
    cmd(
        "nav.partner",
        "Go to partner",
        "Go",
        &["cross reference", "coil", "contact"],
        Needs::Device,
    ),
    cmd(
        "nav.all_references",
        "All references",
        "Go",
        &["cross reference", "occurrences", "where used"],
        Needs::Device,
    ),
    cmd(
        "nav.back",
        "Back",
        "Go",
        &[],
        Needs::History(Direction::Back),
    ),
    cmd(
        "nav.forward",
        "Forward",
        "Go",
        &[],
        Needs::History(Direction::Forward),
    ),
    cmd("nav.previous_page", "Previous page", "Go", &[], Needs::Page),
    cmd("nav.next_page", "Next page", "Go", &[], Needs::Page),
    cmd(
        "page.new",
        "New page",
        "Page",
        &["add page"],
        Needs::Project,
    ),
    cmd(
        "view.fit",
        "Zoom to fit",
        "View",
        &["zoom", "extents"],
        Needs::Page,
    ),
    cmd(
        "view.project_panel",
        "Project panel",
        "View",
        &["tree"],
        Needs::Nothing,
    ),
    cmd(
        "view.properties_panel",
        "Properties panel",
        "View",
        &[],
        Needs::Nothing,
    ),
    cmd(
        "view.messages_panel",
        "Messages panel",
        "View",
        &["errors", "checks"],
        Needs::Nothing,
    ),
    cmd(
        "view.command_palette",
        "Command palette",
        "View",
        &["search", "find"],
        Needs::Nothing,
    ),
];

/// The command with this id.
pub fn get(id: &str) -> Option<&'static CommandInfo> {
    COMMANDS.iter().find(|c| c.id == id)
}

/// Whether an id names a command, including the favourite slots, which are
/// keymap-only (the insert search shows them as its favourites row).
pub fn is_known(id: &str) -> bool {
    get(id).is_some() || FAVOURITE_COMMANDS.contains(&id)
}

/// Every keymap command that is not in [`COMMANDS`]; empty when the two
/// agree.
pub fn unknown_bindings(keymap: &Keymap) -> Vec<&'static str> {
    let mut out: Vec<_> = keymap
        .bindings()
        .iter()
        .map(|b| b.command)
        .filter(|id| !is_known(id))
        .collect();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        for (i, a) in COMMANDS.iter().enumerate() {
            assert!(
                COMMANDS[i + 1..].iter().all(|b| b.id != a.id),
                "duplicate {}",
                a.id
            );
        }
    }

    #[test]
    fn every_binding_names_a_command() {
        assert!(unknown_bindings(&Keymap::workflow()).is_empty());
    }

    #[test]
    fn every_shortcut_map_row_has_a_shortcut() {
        let map = Keymap::workflow();
        for id in [
            "view.command_palette",
            "edit.duplicate",
            "nav.all_references",
            "nav.back",
            "edit.next_orientation",
        ] {
            assert!(map.shortcut_text(id).is_some(), "{id}");
        }
    }

    #[test]
    fn unavailable_commands_give_a_reason() {
        let undo = get("edit.undo").unwrap();
        assert_eq!(
            availability(undo, &Context::default()),
            Err("Nothing to undo")
        );
        let ctx = Context {
            can_undo: true,
            ..Context::default()
        };
        assert_eq!(availability(undo, &ctx), Ok(()));
    }
}
