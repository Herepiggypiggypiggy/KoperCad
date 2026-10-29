//! The centres: Import and Export are workbenches, not menus of formats
//! (`docs/design/visual-redesign.md` section 6). This file says which
//! commands the Export centre lists and how it groups them; the window is in
//! `novo-app`. A command listed here has no menu row: its home is the centre
//! (`homes::Home::Centre`), and File > Export... opens it.

/// One group of tiles in the Export centre.
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub name: &'static str,
    /// One sentence under the group name.
    pub about: &'static str,
    pub ids: &'static [&'static str],
}

/// The command that opens the Export centre.
pub const EXPORT_CENTRE: &str = "file.export_centre";

/// The Export centre, group by group. Every `export.*` command is in exactly
/// one group (a test keeps it so), except Print preview, which keeps its row
/// in File as the print entry and is also listed here.
pub const EXPORT_SECTIONS: &[Section] = &[
    Section {
        name: "Drawings",
        about: "The schematic, the plate and the cabinet as PDF, DXF or STEP.",
        ids: &[
            "export.pdf",
            "export.pdf_page",
            "export.variant_pdf",
            "export.dxf_page",
            "export.dxf_all",
            "export.plate",
            "export.plate_dxf",
            "export.wiring_dxf",
            "export.drill_plan",
            "export.step",
        ],
    },
    Section {
        name: "Print",
        about: "Paper size, scale and plot sets; a ruler to check the scale.",
        ids: &[
            "export.print_preview",
            "export.pdf_options",
            "export.scale_check",
        ],
    },
    Section {
        name: "Reports",
        about: "Lists, records and the package for the customer.",
        ids: &[
            "export.handover",
            "export.bom",
            "export.reports",
            "export.report_pdf",
            "export.change_report",
            "export.commissioning",
            "export.punch_list",
            "export.safety_sheets",
            "export.standards_report",
            "export.standards_xlsx",
        ],
    },
    Section {
        name: "Workshop and bench",
        about: "What is printed and carried to the bench: cut lists, cards, labels.",
        ids: &[
            "export.workshop",
            "export.workpack",
            "export.bench_packet",
            "export.bench_cut",
            "export.wireit",
            "export.kitting",
            "export.assembly",
            "export.ferrules",
            "export.labels",
            "export.wirecut",
            "export.wirecut_xlsx",
            "export.wirecut_page",
        ],
    },
    Section {
        name: "PLC",
        about: "I/O lists and tags for the PLC programming tool.",
        ids: &[
            "export.plc_csv",
            "export.plc_address_plan",
            "export.plc_plcopen",
            "export.plc_tia",
            "export.plc_studio5000",
        ],
    },
    Section {
        name: "Exchange",
        about: "Spreadsheets and other tools' project files.",
        ids: &[
            "export.sheets",
            "export.sheets_csv",
            "export.spec",
            "export.eplan",
            "export.aml",
        ],
    },
];

/// The commands that keep their File menu row although the centre lists
/// them too.
pub const ALSO_IN_MENU: &[&str] = &["export.print_preview"];

/// Whether the Export centre lists `id`.
pub fn in_export_centre(id: &str) -> bool {
    EXPORT_SECTIONS.iter().any(|s| s.ids.contains(&id))
}

/// Whether the centre is the only home of the menu row of `id`: it is
/// listed in the centre and has no row of its own any more.
pub fn replaces_menu_row(id: &str) -> bool {
    in_export_centre(id) && !ALSO_IN_MENU.contains(&id)
}

/// The command ids of the Export centre, in the order it draws them.
pub fn export_ids() -> impl Iterator<Item = &'static str> {
    EXPORT_SECTIONS.iter().flat_map(|s| s.ids.iter().copied())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{COMMANDS, get};
    use std::collections::HashSet;

    #[test]
    fn every_export_command_is_in_exactly_one_section() {
        let mut seen = HashSet::new();
        for id in export_ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
        }
        for info in COMMANDS {
            if info.id.starts_with("export.") {
                assert!(seen.contains(info.id), "{} is in no section", info.id);
            }
        }
    }

    #[test]
    fn sections_have_a_name_and_a_sentence_and_the_opener_is_a_command() {
        for s in EXPORT_SECTIONS {
            assert!(!s.name.is_empty() && !s.about.is_empty() && !s.ids.is_empty());
        }
        assert!(get(EXPORT_CENTRE).is_some());
    }
}
