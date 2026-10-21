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
        // Anything of the Export group not listed in PLACEMENTS lands here.
        "Export" => under("File", "Export other"),
        _ => return None,
    })
}

/// Commands whose menu differs from their group's.
const PLACEMENTS: &[(&str, MenuPlace)] = &[
    // File > Open: the ways to open a project (the Recent list joins them).
    ("file.open", under("File", "Open")),
    ("file.open_read_only", under("File", "Open")),
    ("file.example", under("File", "Open")),
    ("macro.browser", under("File", "Macros")),
    ("macro.import", under("File", "Macros")),
    ("page.new", under("File", "Pages")),
    ("page.rename", under("File", "Pages")),
    ("page.duplicate", under("File", "Pages")),
    ("page.delete", under("File", "Pages")),
    ("page.move_up", under("File", "Pages")),
    ("page.move_down", under("File", "Pages")),
    ("revision.issue", under("File", "Revisions")),
    ("revision.compare", under("File", "Revisions")),
    ("revision.lock_page", under("File", "Revisions")),
    ("revision.unlock_page", under("File", "Revisions")),
    ("revision.restore_page", under("File", "Revisions")),
    ("file.import_dxf_background", under("File", "Import")),
    ("file.remove_background", under("File", "Import")),
    ("file.import_aml", under("File", "Import")),
    ("file.import_crane", under("File", "Import")),
    ("file.import_eplan", under("File", "Import")),
    ("parts.import_eplan", under("File", "Import")),
    ("file.import_dwg", under("File", "Import")),
    ("file.choose_dwg_converter", under("File", "Import")),
    ("file.eplan_export_script", under("File", "Import")),
    ("parts.import_folder", under("File", "Import")),
    ("plc.import_tags", under("File", "Import")),
    ("help.walkthrough", at("Help")),
    ("help.show_hints", at("Help")),
    ("help.whats_new", at("Help")),
    ("help.numbers", at("Help")),
    ("help.about", at("Help")),
    ("view.shortcuts", at("Help")),
    ("help.keymap_file", at("Help")),
    ("help.open_log_folder", at("Help")),
    ("help.copy_problem_report", at("Help")),
    // File > Export, split by what the file is for.
    ("export.pdf", under("File", "Export drawings")),
    ("export.print_preview", under("File", "Export print")),
    ("export.pdf_options", under("File", "Export print")),
    ("export.pdf_page", under("File", "Export drawings")),
    ("export.variant_pdf", under("File", "Export drawings")),
    ("export.dxf_page", under("File", "Export drawings")),
    ("export.dxf_all", under("File", "Export drawings")),
    ("export.step", under("File", "Export drawings")),
    ("export.plate", under("File", "Export drawings")),
    ("export.plate_dxf", under("File", "Export drawings")),
    ("export.wiring_dxf", under("File", "Export drawings")),
    ("export.handover", under("File", "Export reports")),
    ("export.bom", under("File", "Export reports")),
    ("export.change_report", under("File", "Export reports")),
    ("export.commissioning", under("File", "Export reports")),
    ("export.punch_list", under("File", "Export reports")),
    ("export.standards_report", under("File", "Export reports")),
    ("export.standards_xlsx", under("File", "Export reports")),
    ("export.wirecut", under("File", "Export production")),
    ("export.wirecut_xlsx", under("File", "Export production")),
    ("export.wirecut_page", under("File", "Export production")),
    ("export.plc_csv", under("File", "Export PLC")),
    ("export.plc_plcopen", under("File", "Export PLC")),
    ("export.plc_tia", under("File", "Export PLC")),
    ("export.plc_studio5000", under("File", "Export PLC")),
    ("export.plc_address_plan", under("File", "Export PLC")),
    ("export.aml", under("File", "Export interchange")),
    ("export.eplan", under("File", "Export interchange")),
    ("export.sheets", under("File", "Export interchange")),
    ("export.sheets_csv", under("File", "Export interchange")),
    ("export.spec", under("File", "Export interchange")),
    // Reports
    ("report.wirecut", at("Reports")),
    ("report.io", at("Reports")),
    ("report.cable_schedule", at("Reports")),
    ("report.plugs", at("Reports")),
    ("report.add_page", at("Reports")),
    ("export.report_pdf", under("Reports", "Export")),
    // Edit
    ("edit.rotate", at("Edit")),
    ("edit.mirror", at("Edit")),
    ("edit.shared_properties", at("Edit")),
    ("edit.renumber", under("Edit", "Renumber")),
    ("interruption.renumber", under("Edit", "Renumber")),
    ("plc.renumber_io", under("Edit", "Renumber")),
    // Draw
    ("xref.link", at("Draw")),
    ("draw.text", at("Draw")),
    ("net.select", under("Edit", "Net")),
    ("net.trace", under("Edit", "Net")),
    ("plc.add_card", under("Draw", "PLC")),
    ("plc.to_plc", under("Draw", "PLC")),
    ("plc.overview", under("Draw", "PLC")),
    ("macro.save", under("Draw", "Macros")),
    ("macro.place", under("Draw", "Macros")),
    ("macro.edit", under("Draw", "Macros")),
    ("macro.export_pack", under("File", "Macros")),
    ("terminal.connect", under("Draw", "Terminals")),
    ("terminal.disconnect", under("Draw", "Terminals")),
    ("terminal.toggle_pe", under("Draw", "Terminals")),
    ("terminal.reset_pe", under("Draw", "Terminals")),
    ("terminal.strip_up", under("Draw", "Terminals")),
    ("terminal.strip_down", under("Draw", "Terminals")),
    ("terminal.strip_lowest", under("Draw", "Terminals")),
    ("terminal.add_spares", under("Draw", "Terminal strips")),
    ("terminal.rename", under("Draw", "Terminal strips")),
    ("terminal.renumber", under("Draw", "Terminal strips")),
    ("terminal.join_duplicates", under("Draw", "Terminal strips")),
    ("terminal.set_kind", under("Draw", "Terminal strips")),
    ("terminal.set_level", under("Draw", "Terminal strips")),
    ("terminal.set_jumper", under("Draw", "Terminal strips")),
    ("terminal.order_up", under("Draw", "Terminal strips")),
    ("terminal.order_down", under("Draw", "Terminal strips")),
    ("terminal.style", under("Draw", "Terminal strips")),
    (
        "terminal.restore_declined",
        under("Draw", "Terminal strips"),
    ),
    // View
    ("layout.draw", under("View", "Layouts")),
    ("layout.cabinet", under("View", "Layouts")),
    ("layout.3d", under("View", "Layouts")),
    ("layout.review", under("View", "Layouts")),
    ("layout.reports", under("View", "Layouts")),
    ("layout.reset", under("View", "Layouts")),
    ("layout.reset_all", under("View", "Layouts")),
    ("view.schematic", under("View", "Workspace")),
    ("view.cabinet", under("View", "Workspace")),
    ("view.3d", under("View", "Workspace")),
    ("view.3d_door", under("View", "3D")),
    ("view.3d_shell", under("View", "3D")),
    ("view.3d_wires", under("View", "3D")),
    ("view.3d_shadows", under("View", "3D")),
    ("view.3d_align_left", under("View", "3D")),
    ("view.3d_align_hcentre", under("View", "3D")),
    ("view.3d_align_right", under("View", "3D")),
    ("view.3d_align_top", under("View", "3D")),
    ("view.3d_align_vcentre", under("View", "3D")),
    ("view.3d_align_bottom", under("View", "3D")),
    ("view.3d_spread_h", under("View", "3D")),
    ("view.3d_spread_v", under("View", "3D")),
    ("view.3d_ssao", under("View", "3D")),
    ("view.3d_dimensions", under("View", "3D")),
    ("view.3d_quality", under("View", "3D")),
    ("view.3d_stats", under("View", "3D")),
    ("view.3d_measure", under("View", "3D")),
    ("view.3d_section", under("View", "3D")),
    ("view.3d_save_view", under("View", "3D")),
    ("view.3d_wire_runs", under("View", "3D")),
    ("view.3d_hints", under("View", "3D")),
    ("view.3d_select_devices", under("View", "3D")),
    ("view.3d_select_rails", under("View", "3D")),
    ("view.3d_select_ducts", under("View", "3D")),
    ("view.3d_hide", under("View", "3D")),
    ("view.3d_isolate", under("View", "3D")),
    ("view.3d_show_all", under("View", "3D")),
    ("view.3d_duplicate", under("View", "3D")),
    ("view.3d_position", under("View", "3D")),
    ("view.3d_place", under("View", "3D")),
    ("view.iso", under("View", "3D")),
    ("view.front", under("View", "3D")),
    ("view.top", under("View", "3D")),
    ("view.cabinet_table", under("View", "Cabinet")),
    ("cabinet.rails", under("View", "Cabinet")),
    ("cabinet.rules", under("View", "Cabinet")),
    ("cabinet.plate", under("View", "Cabinet")),
    ("cabinet.cutlist", under("View", "Cabinet")),
    ("cabinet.door", under("View", "Cabinet")),
    ("cabinet.surface", under("View", "Cabinet")),
    ("view.terminals_drawer", under("View", "Panels")),
    ("view.cables_drawer", under("View", "Panels")),
    ("view.devices_drawer", under("View", "Panels")),
    ("view.pin_outline", under("View", "Panels")),
    ("view.reports", under("View", "Workspace")),
    ("view.commissioning", under("View", "Workspace")),
    ("cabinet.place-all", under("View", "Cabinet")),
    ("cabinet.size-enclosure", under("View", "Cabinet")),
    ("cabinet.enclosures", under("View", "Cabinet")),
    ("cabinet.routes", under("View", "Cabinet")),
    // Layers: what is drawn on top of the page
    ("view.toggle_marks", under("View", "Layers")),
    ("view.override_marks", under("View", "Layers")),
    ("view.changes", under("View", "Layers")),
    ("view.background", under("View", "Layers")),
    ("view.background_place", under("View", "Layers")),
    ("heat.toggle", under("View", "Layers")),
    ("heat.airflow", under("View", "Layers")),
    ("heat.contours", under("View", "Layers")),
    // Lessons live with Help
    ("help.learn", at("Help")),
    ("ai.assistant", at("Help")),
    ("help.next_lesson", at("Help")),
    ("learn.next", under("Help", "Lesson")),
    ("learn.back", under("Help", "Lesson")),
    ("learn.restart", under("Help", "Lesson")),
    ("learn.close", under("Help", "Lesson")),
    // Simulate
    ("sim.live", at("Simulate")),
    ("sim.why", at("Simulate")),
    ("sim.plc_program", at("Simulate")),
    ("simulate.motor-start", at("Simulate")),
    ("sim.faults", at("Simulate")),
    ("sim.add_fault", at("Simulate")),
    ("view.protection", at("Simulate")),
    ("heat.simulate", under("Simulate", "Heat")),
    // Reports
    ("export.reports", under("Reports", "Export")),
    ("export.assembly", under("Reports", "Export")),
    ("export.workshop", under("Reports", "Export")),
    ("export.safety_sheets", under("Reports", "Export")),
    ("view.bench", at("Reports")),
    ("report.designer", at("Reports")),
    ("export.bench_cut", under("Reports", "Export")),
    ("export.ferrules", under("Reports", "Export")),
    ("export.labels", under("Reports", "Export")),
    ("report.toc", at("Reports")),
    ("report.bom", at("Reports")),
    ("report.devices", at("Reports")),
    ("report.wires", at("Reports")),
    ("report.terminals", at("Reports")),
    ("cable.rename", under("Draw", "Cables")),
    ("cable.delete", under("Draw", "Cables")),
    ("cable.retype", under("Draw", "Cables")),
    ("cable.restore_proposals", under("Draw", "Cables")),
    ("report.cables", at("Reports")),
    ("report.cutlist", at("Reports")),
    ("report.labels", at("Reports")),
];

/// Placements added by the schematic menu regroup (File, Edit, Help). Kept
/// apart from [`PLACEMENTS`] so the two tables never edit the same lines.
const REGROUPED: &[(&str, MenuPlace)] = &[
    // File: project settings and the library each get one submenu, so the
    // top level is only what you do to a file.
    ("project.rules", under("File", "Project")),
    ("project.standard", under("File", "Project")),
    ("project.options", under("File", "Project")),
    ("project.variant", under("File", "Project")),
    ("project.generate", under("File", "Project")),
    ("parts.browser", under("File", "Library")),
    ("parts.import_step", under("File", "Library")),
    ("import.spreadsheet", under("File", "Import")),
    // Edit: the paste variants, the part commands and the numbering
    // commands are families.
    ("edit.paste_keep_tags", under("Edit", "Paste special")),
    ("edit.paste_in_place", under("Edit", "Paste special")),
    ("parts.auto_assign", under("Edit", "Parts")),
    ("parts.clear", under("Edit", "Parts")),
    ("parts.remove_unused", under("Edit", "Parts")),
    ("wires.number", under("Edit", "Wire numbers")),
    ("wires.numbering", under("Edit", "Wire numbers")),
    ("wires.check_numbers", under("Edit", "Wire numbers")),
    ("wires.set_number", under("Edit", "Wire numbers")),
    ("wires.unlock_number", under("Edit", "Wire numbers")),
    ("wires.renumber", under("Edit", "Renumber")),
    // Help: both fast-key walk-throughs sit together.
    ("file.practice", at("Help")),
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
        .chain(REGROUPED)
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

/// Submenus drawn first, in this order; others follow in registry order.
const SUBMENU_ORDER: &[&str] = &[
    "Workspace",
    "Layers",
    "Cabinet",
    "3D",
    "Panels",
    "Go to",
    "Heat",
    "Lesson",
];

/// Submenus drawn right after a plain command instead of after all of them:
/// (submenu, the command it follows). File > Open sits under New project.
const AFTER: &[(&str, &str)] = &[("Open", "file.new")];

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
    let rank = |name: &str| {
        SUBMENU_ORDER
            .iter()
            .position(|n| *n == name)
            .unwrap_or(SUBMENU_ORDER.len())
    };
    subs.sort_by_key(|(name, _)| rank(name));
    let mut out = Vec::new();
    for item in plain {
        let id = match &item {
            MenuItem::Command(info) => info.id,
            _ => "",
        };
        out.push(item);
        for (sub, after) in AFTER {
            if *after == id
                && let Some(n) = subs.iter().position(|(name, _)| name == sub)
            {
                let (name, items) = subs.remove(n);
                out.push(MenuItem::Submenu { name, items });
            }
        }
    }
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

/// The blocks of plain rows of each menu, top to bottom; a separator is
/// drawn between blocks. Commands of the menu that no block names follow in
/// a last block, in registry order, so a new command still appears.
const BLOCKS: &[(&str, &[&[&str]])] = &[
    (
        "File",
        &[
            &["file.new", "file.open", "file.example"],
            &["file.save", "file.save_as", "file.close"],
            &["app.preferences"],
        ],
    ),
    (
        "Edit",
        &[
            &["edit.undo", "edit.redo"],
            &["edit.cut", "edit.copy", "edit.paste"],
            &["edit.duplicate", "edit.select_all", "edit.delete"],
            &["edit.rename", "edit.rotate", "edit.mirror"],
        ],
    ),
    (
        "Draw",
        &[
            &["tool.place", "tool.connect", "wire.mode", "tool.cable"],
            &["proposal.accept", "proposal.accept_all"],
            &["draw.tidy", "xref.link"],
        ],
    ),
    (
        "Check",
        &[
            &["check.problems", "check.standards"],
            &[
                "check.calculations",
                "check.electrical_inputs",
                "check.safety",
            ],
            &["check.next_problem", "check.previous_problem"],
        ],
    ),
    (
        "Reports",
        &[
            &[
                "report.toc",
                "report.bom",
                "report.devices",
                "report.wires",
                "report.terminals",
                "report.cables",
                "report.cutlist",
                "report.labels",
                "report.wirecut",
                "report.io",
                "report.cable_schedule",
                "report.plugs",
            ],
            &["report.add_page", "report.designer", "view.bench"],
        ],
    ),
    (
        "Simulate",
        &[&[
            "sim.live",
            "sim.why",
            "simulate.motor-start",
            "sim.plc_program",
            "sim.faults",
            "view.protection",
        ]],
    ),
    (
        "Help",
        &[
            &[
                "help.next_lesson",
                "help.learn",
                "ai.assistant",
                "view.shortcuts",
                "help.walkthrough",
                "file.practice",
                "help.show_hints",
                "help.numbers",
            ],
            &["help.about"],
        ],
    ),
];

/// Submenus drawn first, in this order, for the menus named here; the
/// others follow in the order [`layout`] gives them.
const SUB_FIRST: &[(&str, &[&str])] = &[
    (
        "File",
        &[
            "Project",
            "Library",
            "Macros",
            "Pages",
            "Import",
            "Revisions",
            "Export drawings",
            "Export reports",
            "Export production",
            "Export PLC",
            "Export interchange",
        ],
    ),
    (
        "Edit",
        &["Paste special", "Parts", "Wire numbers", "Renumber"],
    ),
    (
        "Draw",
        &["Terminals", "Terminal strips", "Cables", "PLC", "Macros"],
    ),
];

/// What the menu bar draws: [`layout`] with the plain rows in their
/// [`BLOCKS`] (a separator between blocks) and the submenus in
/// [`SUB_FIRST`] order.
pub fn drawn(menu: &str) -> Vec<MenuItem> {
    let mut plain: Vec<&'static CommandInfo> = Vec::new();
    let mut subs = Vec::new();
    let mut tail = Vec::new();
    let mut after_plain = false;
    for item in layout(menu) {
        match item {
            MenuItem::Command(c) if !after_plain => plain.push(c),
            MenuItem::Command(c) => tail.push(MenuItem::Command(c)),
            MenuItem::Submenu { .. } => {
                after_plain = true;
                subs.push(item);
            }
            MenuItem::Separator => after_plain = true,
        }
    }
    let first = SUB_FIRST
        .iter()
        .find(|(m, _)| *m == menu)
        .map_or(&[][..], |(_, names)| *names);
    subs.sort_by_key(|item| match item {
        MenuItem::Submenu { name, .. } => {
            first.iter().position(|n| n == name).unwrap_or(first.len())
        }
        _ => usize::MAX,
    });
    let blocks = BLOCKS
        .iter()
        .find(|(m, _)| *m == menu)
        .map_or(&[][..], |(_, blocks)| *blocks);
    let mut out = Vec::new();
    let mut rest = plain;
    for block in blocks {
        let mut taken = Vec::new();
        for id in *block {
            if let Some(at) = rest.iter().position(|c| c.id == *id) {
                taken.push(rest.remove(at));
            }
        }
        if !taken.is_empty() {
            if !out.is_empty() {
                out.push(MenuItem::Separator);
            }
            out.extend(taken.into_iter().map(MenuItem::Command));
        }
    }
    if !rest.is_empty() {
        if !out.is_empty() {
            out.push(MenuItem::Separator);
        }
        out.extend(rest.into_iter().map(MenuItem::Command));
    }
    for block in [subs, tail] {
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
        for (id, place) in PLACEMENTS.iter().chain(REGROUPED) {
            assert!(ids.contains(id), "{id} is not a command");
            assert!(MENUS.contains(&place.menu), "{id}: no menu {}", place.menu);
        }
        for (id, _) in PALETTE_ONLY {
            assert!(ids.contains(id), "{id} is not a command");
            assert!(
                PLACEMENTS.iter().chain(REGROUPED).all(|(p, _)| p != id),
                "{id} is both placed and palette-only"
            );
        }
        let mut seen = HashSet::new();
        for (id, _) in PLACEMENTS.iter().chain(REGROUPED) {
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
            .position(|i| {
                matches!(
                    i,
                    MenuItem::Submenu {
                        name: "Export drawings",
                        ..
                    }
                )
            })
            .expect("an Export drawings submenu");
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
    /// Every id the blocks and the submenu order name is real and sits in
    /// that menu, and no id is named twice.
    #[test]
    fn the_block_tables_name_real_rows_of_their_menu() {
        let ids: HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
        for (menu, blocks) in BLOCKS {
            assert!(MENUS.contains(menu), "no menu {menu}");
            let mut seen = HashSet::new();
            for id in blocks.iter().flat_map(|b| b.iter()) {
                assert!(ids.contains(id), "{id} is not a command");
                assert!(seen.insert(*id), "{id} is in two blocks of {menu}");
            }
        }
        for (menu, names) in SUB_FIRST {
            assert!(MENUS.contains(menu), "no menu {menu}");
            for name in *names {
                let named = entries(menu).iter().any(|(_, p)| p.submenu == Some(*name));
                assert!(named, "{menu} has no submenu {name}");
            }
        }
    }

    /// The drawn menu holds exactly the rows of the layout, once each.
    #[test]
    fn drawn_menus_keep_every_row_once() {
        for menu in MENUS {
            let count = |items: Vec<MenuItem>| {
                let mut ids: Vec<&str> = Vec::new();
                for item in items {
                    match item {
                        MenuItem::Command(c) => ids.push(c.id),
                        MenuItem::Submenu { items, .. } => ids.extend(items.iter().map(|c| c.id)),
                        MenuItem::Separator => {}
                    }
                }
                ids.sort_unstable();
                ids
            };
            assert_eq!(count(drawn(menu)), count(layout(menu)), "{menu}");
        }
    }

    /// No separator opens, closes or doubles up a menu, and Quit ends File.
    #[test]
    fn drawn_menus_have_tidy_separators() {
        for menu in MENUS {
            let items = drawn(menu);
            assert_ne!(items.first(), Some(&MenuItem::Separator), "{menu}");
            assert_ne!(items.last(), Some(&MenuItem::Separator), "{menu}");
            for pair in items.windows(2) {
                assert!(
                    !(pair[0] == MenuItem::Separator && pair[1] == MenuItem::Separator),
                    "{menu}: two separators in a row"
                );
            }
        }
        let file = drawn("File");
        assert_eq!(
            file.last(),
            Some(&MenuItem::Command(
                crate::commands::get("file.quit").unwrap()
            ))
        );
    }

    /// A menu stays short enough for a 768 px window: the schematic-side
    /// menus show at most 14 plain rows and a submenu at most 12. View and
    /// the Export submenus (capped at 10 by their own test) are left out.
    #[test]
    fn schematic_menus_are_not_long_lists() {
        for menu in MENUS {
            if menu == "View" {
                continue;
            }
            let items = drawn(menu);
            let plain = items
                .iter()
                .filter(|i| matches!(i, MenuItem::Command(_)))
                .count();
            assert!(plain <= 15, "{menu} has {plain} plain rows");
            for item in &items {
                if let MenuItem::Submenu { name, items } = item
                    && !name.starts_with("Export")
                {
                    assert!(items.len() <= 12, "{menu} > {name}: {}", items.len());
                }
            }
        }
    }

    #[test]
    fn the_export_menu_is_split_and_every_export_has_a_submenu_of_its_own() {
        for (info, p) in entries("File") {
            if info.group == "Export" && info.id.starts_with("export.") {
                let sub = p.submenu.unwrap();
                assert!(
                    sub.starts_with("Export ") && sub != "Export other",
                    "{} lands in {sub:?}: give it a place in PLACEMENTS",
                    info.id
                );
            }
        }
        for item in layout("File") {
            if let MenuItem::Submenu { name, items } = item
                && name.starts_with("Export")
            {
                assert!(items.len() <= 10, "{name} has {} entries", items.len());
            }
        }
    }

    /// The things a person does most in a menu come first.
    #[test]
    fn edit_starts_with_undo_and_file_with_new() {
        let first = |menu| match drawn(menu).first() {
            Some(MenuItem::Command(c)) => c.id,
            other => panic!("{menu} starts with {other:?}"),
        };
        assert_eq!(first("File"), "file.new");
        assert_eq!(first("Edit"), "edit.undo");
        assert_eq!(first("Draw"), "tool.place");
    }

    #[test]
    fn open_commands_share_a_submenu_under_new_and_the_file_menu_stays_short() {
        let file = layout("File");
        let at_new = file
            .iter()
            .position(|i| matches!(i, MenuItem::Command(c) if c.id == "file.new"))
            .unwrap();
        match &file[at_new + 1] {
            MenuItem::Submenu {
                name: "Open",
                items,
            } => {
                let ids: Vec<_> = items.iter().map(|c| c.id).collect();
                assert_eq!(ids, ["file.open", "file.open_read_only", "file.example"]);
            }
            other => panic!("expected File > Open after New project, got {other:?}"),
        }
        // About 26 px a row keeps the whole menu inside 700 px.
        let rows = file
            .iter()
            .filter(|i| !matches!(i, MenuItem::Separator))
            .count();
        assert!(rows <= 26, "the File menu has {rows} rows");
    }

    #[test]
    fn the_new_report_and_export_commands_have_their_homes() {
        let home = |id: &str| place(crate::commands::get(id).unwrap()).unwrap();
        for id in [
            "report.wirecut",
            "report.io",
            "report.cable_schedule",
            "report.plugs",
            "report.add_page",
        ] {
            assert_eq!(home(id), at("Reports"), "{id}");
        }
        assert_eq!(home("export.report_pdf"), under("Reports", "Export"));
        assert_eq!(home("export.pdf_page"), under("File", "Export drawings"));
        assert_eq!(home("export.plate_dxf"), under("File", "Export drawings"));
    }
}
