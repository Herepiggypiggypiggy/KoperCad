//! Where a command can be found (docs/design/visual-redesign.md section 5.2).
//!
//! The old invariant was "every command is in a menu or says why not". The
//! redesign moves commands out of menus onto the tool rail, into the Import
//! and Export centres and onto the drawer toolbars, so the invariant widens:
//! **every command has a home**, one of [`Home`]. Every command is also
//! findable with Ctrl K whatever its home; a home says where a person who
//! does not search will look for it.
//!
//! The menu home is read from [`crate::menus`]; the other homes are tables
//! here ([`RELOCATED`]) that the build lanes fill when they move a command:
//! V2c for menu trimming, V3 for the rails, V4 for the centres. The test at
//! the bottom lists any command without a home, so a moved command cannot be
//! lost.

use crate::commands::{COMMANDS, CommandInfo};
use crate::menus::{self, MenuPlace};

/// One place a command is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Home {
    /// A row in the app menu (or the menu bar), under this menu.
    Menu(MenuPlace),
    /// A tool, assist or panel toggle on the tool rail, or in one of its
    /// flyouts.
    Rail,
    /// The Import centre, the Export centre or Settings.
    Centre,
    /// The toolbar of a drawer (terminals, cables).
    DrawerToolbar,
    /// A key or pointer gesture, or a design reference: only the command
    /// field lists it. The text says why.
    PaletteOnly(&'static str),
}

impl Home {
    /// The kind, for counting and for tests.
    pub fn kind(&self) -> HomeKind {
        match self {
            Home::Menu(_) => HomeKind::Menu,
            Home::Rail => HomeKind::Rail,
            Home::Centre => HomeKind::Centre,
            Home::DrawerToolbar => HomeKind::DrawerToolbar,
            Home::PaletteOnly(_) => HomeKind::PaletteOnly,
        }
    }
}

/// The five kinds of [`Home`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HomeKind {
    Menu,
    Rail,
    Centre,
    DrawerToolbar,
    PaletteOnly,
}

impl HomeKind {
    pub const ALL: [HomeKind; 5] = [
        HomeKind::Menu,
        HomeKind::Rail,
        HomeKind::Centre,
        HomeKind::DrawerToolbar,
        HomeKind::PaletteOnly,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HomeKind::Menu => "menu",
            HomeKind::Rail => "rail",
            HomeKind::Centre => "centre",
            HomeKind::DrawerToolbar => "drawer toolbar",
            HomeKind::PaletteOnly => "palette only",
        }
    }
}

/// Homes other than a menu row or "palette only", by command id. A command
/// may be listed more than once (a rail tool that also has a menu row is
/// found by its menu row; one listed here gets that home in addition).
///
/// The tool rails of the five workspaces are built (lane V3): every command a
/// rail carries is listed here, in addition to its menu row when it keeps one
/// and instead of it when [`menus::MOVED`] names it. The centres (V4) and the
/// drawer toolbars add theirs in the pull request that builds them. The test
/// in `novo-app` (`rail_homes`) keeps this list equal to what the rails draw.
pub const RELOCATED: &[(&str, Home)] = &[
    // Schematic rail
    ("tool.place", Home::Rail),
    ("tool.connect", Home::Rail),
    ("tool.cable", Home::Rail),
    ("draw.text", Home::Rail),
    ("wires.autopilot", Home::Rail),
    ("proposal.accept_all", Home::Rail),
    ("draw.tidy", Home::Rail),
    ("xref.link", Home::Rail),
    ("macro.place", Home::Rail),
    ("check.problems", Home::Rail),
    ("view.terminals_drawer", Home::Rail),
    ("view.cables_drawer", Home::Rail),
    ("view.devices_drawer", Home::Rail),
    ("edit.undo", Home::Rail),
    ("edit.redo", Home::Rail),
    // 3D rail
    ("view.3d_move", Home::Rail),
    ("view.3d_place", Home::Rail),
    ("view.3d_measure", Home::Rail),
    ("view.3d_section", Home::Rail),
    ("view.3d_save_view", Home::Rail),
    ("view.3d_assembly", Home::Rail),
    ("heat.toggle", Home::Rail),
    ("view.emc", Home::Rail),
    ("view.3d_position", Home::Rail),
    ("view.3d_stats", Home::Rail),
    ("view.3d_align_left", Home::Rail),
    ("view.3d_align_hcentre", Home::Rail),
    ("view.3d_align_right", Home::Rail),
    ("view.3d_align_top", Home::Rail),
    ("view.3d_align_vcentre", Home::Rail),
    ("view.3d_align_bottom", Home::Rail),
    ("view.3d_spread_h", Home::Rail),
    ("view.3d_spread_v", Home::Rail),
    ("view.3d_hide", Home::Rail),
    ("view.3d_isolate", Home::Rail),
    ("view.3d_show_all", Home::Rail),
    // Cabinet rail
    ("cabinet.place-all", Home::Rail),
    ("cabinet.rails", Home::Rail),
    ("cabinet.plate", Home::Rail),
    ("cabinet.surface", Home::Rail),
    ("cabinet.size-enclosure", Home::Rail),
    ("cabinet.routes", Home::Rail),
    ("cabinet.rules", Home::Rail),
    ("cabinet.emc", Home::Rail),
    ("cabinet.cutlist", Home::Rail),
    ("heat.vents", Home::Rail),
    ("view.cabinet_table", Home::Rail),
    // Reports and Commissioning rail
    ("report.add_page", Home::Rail),
    ("report.designer", Home::Rail),
    ("export.labels", Home::Rail),
    ("view.bench", Home::Rail),
    ("view.build", Home::Rail),
    ("export.print_preview", Home::Rail),
    ("export.reports", Home::Rail),
    ("export.report_pdf", Home::Rail),
    ("export.bom", Home::Rail),
    ("export.pdf", Home::Rail),
    ("export.commissioning", Home::Rail),
    ("export.punch_list", Home::Rail),
];

/// Every home of the command `id`, menu first. Empty only for a command that
/// has none, which the test below does not allow.
pub fn homes(id: &str) -> Vec<Home> {
    let mut out = Vec::new();
    if let Some(info) = COMMANDS.iter().find(|c| c.id == id) {
        out.extend(menus::place(info).map(Home::Menu));
    }
    out.extend(RELOCATED.iter().filter(|(i, _)| *i == id).map(|(_, h)| *h));
    if crate::centres::in_export_centre(id) {
        out.push(Home::Centre);
    }
    if let Some(reason) = menus::palette_only_reason(id) {
        out.push(Home::PaletteOnly(reason));
    }
    out
}

/// The commands with no home at all.
pub fn homeless() -> Vec<&'static CommandInfo> {
    COMMANDS.iter().filter(|c| homes(c.id).is_empty()).collect()
}

/// How many commands have a home of each kind (a command with two homes is
/// counted under both).
pub fn census() -> Vec<(HomeKind, usize)> {
    HomeKind::ALL
        .iter()
        .map(|&kind| {
            let n = COMMANDS
                .iter()
                .filter(|c| homes(c.id).iter().any(|h| h.kind() == kind))
                .count();
            (kind, n)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn every_command_has_a_home() {
        let lost: Vec<_> = homeless().iter().map(|c| c.id).collect();
        assert!(
            lost.is_empty(),
            "commands without a home (menu row, rail, centre, drawer toolbar or PALETTE_ONLY with a reason): {lost:?}"
        );
    }

    #[test]
    fn palette_only_is_the_only_home_and_has_a_reason() {
        for c in COMMANDS {
            let h = homes(c.id);
            if let Some(Home::PaletteOnly(reason)) =
                h.iter().find(|h| h.kind() == HomeKind::PaletteOnly)
            {
                assert_eq!(
                    h.len(),
                    1,
                    "{} is palette-only and also elsewhere: {h:?}",
                    c.id
                );
                assert!(!reason.is_empty(), "{}: say why", c.id);
            }
        }
    }

    #[test]
    fn relocated_rows_name_real_commands_and_real_other_homes() {
        let ids: HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
        for (id, home) in RELOCATED {
            assert!(ids.contains(id), "{id} is not a command");
            assert!(
                matches!(
                    home.kind(),
                    HomeKind::Rail | HomeKind::Centre | HomeKind::DrawerToolbar
                ),
                "{id}: RELOCATED is for rail, centre and drawer toolbar homes; menus come from menus.rs"
            );
        }
        let pairs: HashSet<_> = RELOCATED.iter().map(|(i, h)| (*i, h.kind())).collect();
        assert_eq!(pairs.len(), RELOCATED.len(), "a home is listed twice");
    }

    /// A command whose menu row moved must have another home to go to.
    #[test]
    fn a_command_moved_out_of_the_menus_has_a_home_elsewhere() {
        for id in menus::MOVED {
            assert!(
                RELOCATED.iter().any(|(i, _)| i == id),
                "{id} left the menus but no rail, centre or drawer toolbar lists it in RELOCATED"
            );
        }
    }

    #[test]
    fn the_census_counts_every_command_once_or_more() {
        let census = census();
        let total: usize = census.iter().map(|(_, n)| n).sum();
        assert!(total >= COMMANDS.len());
        let menu = census
            .iter()
            .find(|(k, _)| *k == HomeKind::Menu)
            .map(|(_, n)| *n);
        assert!(menu.is_some_and(|n| n > 250), "{census:?}");
    }
}
