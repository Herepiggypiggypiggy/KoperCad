//! Where every command sits in the menu bar (docs/design/workspaces.md §5).
//!
//! The menu bar is a view of [`crate::commands::COMMANDS`], not a second
//! list: a command gets the menu of its group unless [`PLACEMENTS`] says
//! otherwise, and a command that is only a key or a pointer gesture is named
//! in [`PALETTE_ONLY`] with the reason. The tests below fail for a command
//! that has neither, which is how a feature gets a home (§1, principle 2).

use crate::commands::{COMMANDS, CommandInfo};

/// The menus, left to right. A menu with no commands yet is not drawn.
pub const MENUS: [&str; 8] = [
    "File", "Edit", "Draw", "View", "Check", "Reports", "Tools", "Help",
];

/// A command's place in the menu bar: a menu, and a submenu inside it when
/// the entry is one of a family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuPlace {
    pub menu: &'static str,
    pub submenu: Option<&'static str>,
}

const fn at(menu: &'static str) -> MenuPlace {
    MenuPlace {
        menu,
        submenu: None,
    }
}

const fn under(menu: &'static str, submenu: &'static str) -> MenuPlace {
    MenuPlace {
        menu,
        submenu: Some(submenu),
    }
}

/// The menu a command's group puts it in, when [`PLACEMENTS`] has no entry.
fn group_default(group: &str) -> Option<MenuPlace> {
    Some(match group {
        "Project" => at("File"),
        "Draw" => at("Draw"),
        "Edit" => at("Edit"),
        "Navigate" => under("View", "Go to"),
        "View" => at("View"),
        "Check" => at("Check"),
        "Export" => under("File", "Export"),
        _ => return None,
    })
}

/// Commands whose menu differs from their group's.
const PLACEMENTS: &[(&str, MenuPlace)] = &[
    ("revision.issue", at("Check")),
    ("help.about", at("Help")),
    ("view.shortcuts", at("Help")),
    // Edit
    ("edit.rotate", at("Edit")),
    ("edit.mirror", at("Edit")),
    ("edit.renumber", under("Edit", "Renumber")),
    ("interruption.renumber", under("Edit", "Renumber")),
    ("plc.renumber_io", under("Edit", "Renumber")),
    // Draw
    ("xref.link", at("Draw")),
    ("plc.add_card", under("Draw", "PLC")),
    ("plc.to_plc", under("Draw", "PLC")),
    ("terminal.connect", under("Draw", "Terminals")),
    ("terminal.disconnect", under("Draw", "Terminals")),
    ("terminal.toggle_pe", under("Draw", "Terminals")),
    ("terminal.reset_pe", under("Draw", "Terminals")),
    ("terminal.strip_up", under("Draw", "Terminals")),
    ("terminal.strip_down", under("Draw", "Terminals")),
    ("terminal.strip_lowest", under("Draw", "Terminals")),
    // View
    ("view.schematic", under("View", "Workspace")),
    ("view.cabinet", under("View", "Workspace")),
    ("view.cabinet_table", under("View", "Workspace")),
    ("view.terminals_drawer", under("View", "Panels")),
    ("view.cables_drawer", under("View", "Panels")),
    ("view.devices_drawer", under("View", "Panels")),
    ("view.pin_outline", under("View", "Panels")),
    // Reports
    ("export.bom", at("Reports")),
];

/// Commands that are a key or a pointer gesture and have no menu row.
const PALETTE_ONLY: &[(&str, &str)] = &[
    ("tool.repeat", "the key that repeats the last tool"),
    ("edit.escape", "Esc"),
    (
        "view.command_field_here",
        "opens at the pointer, so a menu cannot",
    ),
    ("wire.end_on_part", "only while a wire is being drawn"),
    ("wire.next_pin", "only while a wire is being drawn"),
    ("proposal.next", "Tab while proposals are showing"),
    (
        "nav.next_page",
        "paired with Previous page, which is in Go to",
    ),
];

/// The command's place in the menu bar, or `None` for the commands in
/// [`PALETTE_ONLY`].
pub fn place(info: &CommandInfo) -> Option<MenuPlace> {
    if palette_only_reason(info.id).is_some() {
        return None;
    }
    PLACEMENTS
        .iter()
        .find(|(id, _)| *id == info.id)
        .map(|(_, place)| *place)
        .or_else(|| group_default(info.group))
}

/// Why a command has no menu row, when it has none on purpose.
pub fn palette_only_reason(id: &str) -> Option<&'static str> {
    PALETTE_ONLY
        .iter()
        .find(|(i, _)| *i == id)
        .map(|(_, reason)| *reason)
}

/// The commands of `menu`, in registry order. Submenu entries are included
/// and carry their submenu name, so the bar groups them as it draws.
pub fn entries(menu: &str) -> Vec<(&'static CommandInfo, MenuPlace)> {
    COMMANDS
        .iter()
        .filter_map(|info| place(info).map(|p| (info, p)))
        .filter(|(_, p)| p.menu == menu)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_command_has_a_menu_or_a_reason() {
        for info in COMMANDS {
            let has_place = place(info).is_some();
            let has_reason = palette_only_reason(info.id).is_some();
            assert!(
                has_place != has_reason,
                "{} needs a menu row (PLACEMENTS or its group) or an entry in PALETTE_ONLY, not both",
                info.id
            );
        }
    }

    #[test]
    fn placements_name_real_commands_and_menus() {
        let ids: HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
        for (id, place) in PLACEMENTS {
            assert!(ids.contains(id), "{id} is not a command");
            assert!(MENUS.contains(&place.menu), "{id}: no menu {}", place.menu);
        }
        for (id, _) in PALETTE_ONLY {
            assert!(ids.contains(id), "{id} is not a command");
            assert!(
                PLACEMENTS.iter().all(|(p, _)| p != id),
                "{id} is both placed and palette-only"
            );
        }
        let mut seen = HashSet::new();
        for (id, _) in PLACEMENTS {
            assert!(seen.insert(id), "{id} is placed twice");
        }
    }

    #[test]
    fn labels_are_unique_within_a_menu_level() {
        let mut seen = HashSet::new();
        for menu in MENUS {
            for (info, p) in entries(menu) {
                assert!(
                    seen.insert((p.menu, p.submenu, info.label)),
                    "two entries called {:?} in {} {:?}",
                    info.label,
                    p.menu,
                    p.submenu
                );
            }
        }
    }

    #[test]
    fn the_groups_fit_the_menus() {
        for group in crate::commands::GROUPS {
            let place = group_default(group).unwrap_or_else(|| panic!("group {group}"));
            assert!(MENUS.contains(&place.menu));
        }
    }

    #[test]
    fn file_and_view_lead_with_the_expected_commands() {
        let file: Vec<_> = entries("File").iter().map(|(c, _)| c.id).collect();
        assert!(file.contains(&"file.new") && file.contains(&"export.pdf"));
        let view: Vec<_> = entries("View").iter().map(|(c, _)| c.id).collect();
        assert!(view.contains(&"view.cabinet") && view.contains(&"nav.back"));
    }
}
