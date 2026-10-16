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
    "File", "Edit", "Draw", "View", "Check", "Reports", "Simulate", "Help",
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
    ("revision.issue", under("File", "Revisions")),
    ("revision.compare", under("File", "Revisions")),
    ("file.import_dxf_background", under("File", "Import")),
    ("file.remove_background", under("File", "Import")),
    ("file.import_aml", under("File", "Import")),
    ("file.import_eplan", under("File", "Import")),
    ("file.import_dwg", under("File", "Import")),
    ("file.choose_dwg_converter", under("File", "Import")),
    ("file.eplan_export_script", under("File", "Import")),
    ("parts.import_folder", under("File", "Import")),
    ("plc.import_tags", under("File", "Import")),
    ("help.walkthrough", at("Help")),
    ("help.show_hints", at("Help")),
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
    ("macro.save", under("Draw", "Macros")),
    ("macro.place", under("Draw", "Macros")),
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
    ("view.3d", under("View", "Workspace")),
    ("view.3d_door", under("View", "3D")),
    ("view.3d_shell", under("View", "3D")),
    ("view.3d_wires", under("View", "3D")),
    ("view.3d_shadows", under("View", "3D")),
    ("view.3d_stats", under("View", "3D")),
    ("view.iso", under("View", "3D")),
    ("view.front", under("View", "3D")),
    ("view.top", under("View", "3D")),
    ("view.cabinet_table", under("View", "Workspace")),
    ("cabinet.rails", under("View", "Workspace")),
    ("cabinet.rules", under("View", "Workspace")),
    ("cabinet.plate", under("View", "Workspace")),
    ("cabinet.cutlist", under("View", "Workspace")),
    ("cabinet.door", under("View", "Workspace")),
    ("view.terminals_drawer", under("View", "Panels")),
    ("view.cables_drawer", under("View", "Panels")),
    ("view.devices_drawer", under("View", "Panels")),
    ("view.pin_outline", under("View", "Panels")),
    ("view.reports", under("View", "Workspace")),
    // Simulate
    ("sim.live", at("Simulate")),
    ("sim.plc_program", at("Simulate")),
    ("simulate.motor-start", at("Simulate")),
    ("sim.faults", at("Simulate")),
    ("view.protection", at("Simulate")),
    // Reports
    ("export.reports", at("Reports")),
    ("export.assembly", at("Reports")),
    ("view.bench", at("Reports")),
    ("export.bench_cut", at("Reports")),
    ("export.ferrules", at("Reports")),
    ("export.labels", at("Reports")),
    ("report.toc", at("Reports")),
    ("report.bom", at("Reports")),
    ("report.devices", at("Reports")),
    ("report.wires", at("Reports")),
    ("report.terminals", at("Reports")),
    ("report.cables", at("Reports")),
    ("report.cutlist", at("Reports")),
    ("report.labels", at("Reports")),
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

/// Commands drawn at the bottom of their menu.
const LAST: &[&str] = &["file.quit"];

/// One row of a drawn menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MenuItem {
    Command(&'static CommandInfo),
    Submenu {
        name: &'static str,
        items: Vec<&'static CommandInfo>,
    },
    Separator,
}

/// What a menu draws: its plain commands in registry order, then its
/// submenus in the order they first appear, then the [`LAST`] commands, with
/// a separator between the blocks. An empty menu gives an empty list, and
/// the bar leaves it out.
pub fn layout(menu: &str) -> Vec<MenuItem> {
    let mut plain = Vec::new();
    let mut subs: Vec<(&'static str, Vec<&'static CommandInfo>)> = Vec::new();
    let mut last = Vec::new();
    for (info, place) in entries(menu) {
        if LAST.contains(&info.id) {
            last.push(MenuItem::Command(info));
        } else if let Some(name) = place.submenu {
            match subs.iter_mut().find(|(n, _)| *n == name) {
                Some((_, items)) => items.push(info),
                None => subs.push((name, vec![info])),
            }
        } else {
            plain.push(MenuItem::Command(info));
        }
    }
    let mut out = plain;
    let blocks = [
        subs.into_iter()
            .map(|(name, items)| MenuItem::Submenu { name, items })
            .collect::<Vec<_>>(),
        last,
    ];
    for block in blocks {
        if !block.is_empty() {
            if !out.is_empty() {
                out.push(MenuItem::Separator);
            }
            out.extend(block);
        }
    }
    out
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
    fn quit_ends_the_file_menu_and_submenus_follow_the_plain_commands() {
        let file = layout("File");
        assert_eq!(
            file.last(),
            Some(&MenuItem::Command(
                crate::commands::get("file.quit").unwrap()
            ))
        );
        let export = file
            .iter()
            .position(|i| matches!(i, MenuItem::Submenu { name: "Export", .. }))
            .expect("an Export submenu");
        let new = file
            .iter()
            .position(|i| matches!(i, MenuItem::Command(c) if c.id == "file.new"))
            .unwrap();
        assert!(new < export);
        // Every command appears exactly once across the menus.
        let mut count = 0;
        for menu in MENUS {
            for item in layout(menu) {
                match item {
                    MenuItem::Command(_) => count += 1,
                    MenuItem::Submenu { items, .. } => count += items.len(),
                    MenuItem::Separator => {}
                }
            }
        }
        let placed = COMMANDS.iter().filter(|c| place(c).is_some()).count();
        assert_eq!(count, placed);
    }

    #[test]
    fn file_and_view_lead_with_the_expected_commands() {
        let file: Vec<_> = entries("File").iter().map(|(c, _)| c.id).collect();
        assert!(file.contains(&"file.new") && file.contains(&"export.pdf"));
        let view: Vec<_> = entries("View").iter().map(|(c, _)| c.id).collect();
        assert!(view.contains(&"view.cabinet") && view.contains(&"nav.back"));
    }
}
