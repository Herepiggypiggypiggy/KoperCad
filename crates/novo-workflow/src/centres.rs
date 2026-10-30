//! The centres: Import and Export are workbenches, not menus of formats
//! (`docs/design/visual-redesign.md` section 6), and the Start centre holds
//! the templates and examples a new project can begin from. This file says
//! which commands each centre lists and how it groups them; the window is in
//! `novo-app`. A command listed here has no menu row: its home is the centre
//! (`homes::Home::Centre`), and File > Export..., File > Import... and
//! File > Templates and examples... open them.

/// Which of the two centres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Centre {
    Export,
    Import,
    Start,
    Project,
    Numbering,
    Parts,
}

/// Every centre, in the order their openers are listed.
pub const CENTRES: [Centre; 6] = [
    Centre::Export,
    Centre::Import,
    Centre::Start,
    Centre::Project,
    Centre::Numbering,
    Centre::Parts,
];

impl Centre {
    /// The command that opens the centre.
    pub fn opener(self) -> &'static str {
        match self {
            Centre::Export => EXPORT_CENTRE,
            Centre::Import => IMPORT_CENTRE,
            Centre::Start => START_CENTRE,
            Centre::Project => PROJECT_CENTRE,
            Centre::Numbering => NUMBERING_CENTRE,
            Centre::Parts => PARTS_CENTRE,
        }
    }

    /// The window title.
    pub fn title(self) -> &'static str {
        match self {
            Centre::Export => "Export",
            Centre::Import => "Import",
            Centre::Start => "Templates and examples",
            Centre::Project => "Project settings",
            Centre::Numbering => "Numbering",
            Centre::Parts => "Part tools",
        }
    }

    /// The hint in the search field.
    pub fn search_hint(self) -> &'static str {
        match self {
            Centre::Export => "Search formats: pdf, dxf, excel, bench, plc",
            Centre::Import => "Search: dxf, eplan, parts, excel, plc",
            Centre::Start => "Search: crane, pump, motor control, plc, emc",
            Centre::Project => "Search: standard, rules, options, variants, languages",
            Centre::Numbering => "Search: wires, devices, tags, potentials, I/O, interruption",
            Centre::Parts => "Search: catalog, assign, describe, unused, step",
        }
    }

    /// What the footer button says while nothing is picked.
    pub fn idle_button(self) -> &'static str {
        match self {
            Centre::Start | Centre::Project => "Open",
            Centre::Numbering => "Run",
            Centre::Parts => "Run",
            _ => self.title(),
        }
    }

    /// What the footer button says when `id` is picked.
    pub fn button(self, id: &str) -> &'static str {
        match (self, id) {
            (Centre::Export, _) => "Export",
            (Centre::Import, "file.remove_background") => "Remove",
            (Centre::Import, "file.choose_dwg_converter") => "Choose",
            (Centre::Import, "file.eplan_export_script") => "Get script",
            (Centre::Import, _) => "Import",
            (Centre::Start, id) if id.starts_with("file.new_") => "Create",
            (Centre::Start, _) => "Open",
            (Centre::Project, "project.untie") => "Apply",
            (Centre::Project, "project.generate") => "Generate",
            (Centre::Project, _) => "Open",
            (
                Centre::Numbering,
                "wires.numbering" | "wires.renumber" | "edit.renumber" | "wires.set_number",
            ) => "Open",
            (Centre::Numbering, _) => "Run",
            (Centre::Parts, "parts.browser") => "Open",
            (Centre::Parts, "parts.import_step") => "Import",
            (Centre::Parts, "parts.auto_assign") => "Assign",
            (Centre::Parts, "parts.describe") => "Describe",
            (Centre::Parts, _) => "Remove",
        }
    }

    pub fn sections(self) -> &'static [Section] {
        match self {
            Centre::Export => EXPORT_SECTIONS,
            Centre::Import => IMPORT_SECTIONS,
            Centre::Start => START_SECTIONS,
            Centre::Project => PROJECT_SECTIONS,
            Centre::Numbering => NUMBERING_SECTIONS,
            Centre::Parts => PARTS_SECTIONS,
        }
    }

    /// The command ids of the centre, in the order it draws them.
    pub fn ids(self) -> impl Iterator<Item = &'static str> {
        self.sections().iter().flat_map(|s| s.ids.iter().copied())
    }

    /// The centre that lists `id`, if any.
    pub fn of(id: &str) -> Option<Centre> {
        CENTRES
            .into_iter()
            .find(|c| c.sections().iter().any(|s| s.ids.contains(&id)))
    }
}

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

/// The command that opens the Import centre.
pub const IMPORT_CENTRE: &str = "file.import_centre";

/// The command that opens the Start centre.
pub const START_CENTRE: &str = "file.start_centre";

/// The Start centre: what File > New from template and File > Examples held.
pub const START_SECTIONS: &[Section] = &[
    Section {
        name: "Templates",
        about: "A new project with the pages, tags and structure of a kind of machine.",
        ids: &["file.new_crane", "file.new_mcc", "file.new_pump"],
    },
    Section {
        name: "Examples",
        about: "Finished projects to open, look through and learn from.",
        ids: &[
            "file.example",
            "file.example_crane",
            "file.example_mcc",
            "file.example_plc",
            "file.example_pump",
            "file.example_emc",
        ],
    },
];

/// The command that opens the Project settings centre.
pub const PROJECT_CENTRE: &str = "file.project_centre";

/// The Project settings centre: what File > Project and the report
/// languages row held. `project.variant`, `project.option` and `project.tie`
/// take a name in the command field and open Project options without one,
/// so they are palette-only.
pub const PROJECT_SECTIONS: &[Section] = &[
    Section {
        name: "Drawing and reports",
        about: "How the drawings are made and numbered, and what language the reports use.",
        ids: &["project.standard", "project.rules", "project.languages"],
    },
    Section {
        name: "Options and variants",
        about: "Machine options and variants: what is drawn with which configuration.",
        ids: &["project.options", "project.untie"],
    },
    Section {
        name: "Generate",
        about: "Pages and devices made from a spec sheet.",
        ids: &["project.generate"],
    },
];

/// The command that opens the Numbering centre.
pub const NUMBERING_CENTRE: &str = "edit.numbering_centre";

/// The Numbering centre: what Edit > Wire numbers and Edit > Renumber held.
pub const NUMBERING_SECTIONS: &[Section] = &[
    Section {
        name: "Wires",
        about: "Numbers on wires and potentials: give them, renumber, check, fix one by hand.",
        ids: &[
            "wires.number",
            "wires.renumber",
            "wires.check_numbers",
            "wires.numbering",
            "wires.set_number",
            "wires.unlock_number",
        ],
    },
    Section {
        name: "Devices and points",
        about: "Tags of devices, interruption points and PLC I/O.",
        ids: &["edit.renumber", "interruption.renumber", "plc.renumber_io"],
    },
];

/// The command that opens the Part tools centre.
pub const PARTS_CENTRE: &str = "edit.parts_centre";

/// The Part tools centre: what File > Library and Edit > Parts held.
pub const PARTS_SECTIONS: &[Section] = &[
    Section {
        name: "Catalog",
        about: "The part catalog, and 3D models for parts.",
        ids: &["parts.browser", "parts.import_step"],
    },
    Section {
        name: "Parts on the devices",
        about: "Give the devices of this project their parts, or take them away.",
        ids: &[
            "parts.auto_assign",
            "parts.describe",
            "parts.clear",
            "parts.remove_unused",
        ],
    },
];

/// The Import centre, group by group: what File > Import and File > Import
/// parts held, by what comes in.
pub const IMPORT_SECTIONS: &[Section] = &[
    Section {
        name: "Drawings and backgrounds",
        about: "A DXF or DWG drawing behind the page, to trace over.",
        ids: &[
            "file.import_dxf_background",
            "file.import_dwg",
            "file.choose_dwg_converter",
            "file.remove_background",
        ],
    },
    Section {
        name: "Projects",
        about: "Another tool's project or plant, brought in as pages and devices.",
        ids: &[
            "file.import_eplan",
            "file.eplan_export_script",
            "file.import_crane",
            "file.import_aml",
            "file.import_controller",
        ],
    },
    Section {
        name: "Parts",
        about: "Part data for the catalog, from files you have.",
        ids: &[
            "parts.import_folder",
            "parts.import_table",
            "parts.import_eplan",
        ],
    },
    Section {
        name: "Lists and tags",
        about: "Spreadsheets for bulk edit, and the tags of a PLC programmer.",
        ids: &["import.spreadsheet", "plc.import_tags"],
    },
];

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
pub const ALSO_IN_MENU: &[&str] = &["export.print_preview", "file.example"];

/// One sentence about what the export writes, for the tile and the detail
/// pane. It says what the command's own label says, in plain words.
pub fn describe(id: &str) -> &'static str {
    match id {
        "export.pdf" => "Every page as one PDF with bookmarks and links.",
        "export.pdf_page" => "Only the page you are on, as a PDF.",
        "export.variant_pdf" => "A PDF of one variant of the project.",
        "export.dxf_page" => "The page you are on as a DXF drawing.",
        "export.dxf_all" => "Every page as DXF drawings.",
        "export.plate" => "The mounting plate drawing.",
        "export.plate_dxf" => "The mounting plate drawing as DXF.",
        "export.wiring_dxf" => "The wiring plan as DXF.",
        "export.drill_plan" => "The plan for drilling the plate by hand.",
        "export.step" => "The cabinet as a STEP 3D model.",
        "export.print_preview" => "Paper size, scale, margins and plot sets, with a preview.",
        "export.pdf_options" => "A PDF with page range, bookmarks and other options.",
        "export.scale_check" => "A page with a ruler of known length to check the print scale.",
        "export.handover" => "Drawings, lists and records in one folder, with a contents page.",
        "export.bom" => "The bill of materials.",
        "export.reports" => "Every generated report in one Excel file.",
        "export.report_pdf" => "One report as a PDF.",
        "export.change_report" => "What changed between revisions.",
        "export.commissioning" => "The commissioning record.",
        "export.punch_list" => "The open items of the punch list.",
        "export.safety_sheets" => "One sheet for each safety function.",
        "export.standards_report" => "How the design stands against the checked standards.",
        "export.standards_xlsx" => "The same standards report as an Excel file.",
        "export.workshop" => "Everything the workshop needs, in one folder.",
        "export.workpack" => "Work cards for the bench.",
        "export.bench_packet" => "The bench pages together in one PDF packet.",
        "export.bench_cut" => "The bench cut list as a PDF.",
        "export.wireit" => "The Wire-it list, wire by wire, as a PDF.",
        "export.kitting" => "What to pull from stock, as a PDF list.",
        "export.assembly" => "A checklist for assembling the cabinet, as a PDF.",
        "export.ferrules" => "The ferrules needed, as a PDF table.",
        "export.labels" => "Label sheets as a PDF.",
        "export.wirecut" => "The wire cut list as a CSV file.",
        "export.wirecut_xlsx" => "Wire production data as an Excel file.",
        "export.wirecut_page" => "Adds a wire production page to the project, as a PDF.",
        "export.plc_csv" => "The PLC I/O list as a CSV file.",
        "export.plc_address_plan" => "The PLC address plan.",
        "export.plc_plcopen" => "PLC tags as PLCopen XML.",
        "export.plc_tia" => "PLC tags for TIA Portal.",
        "export.plc_studio5000" => "PLC tags for Studio 5000.",
        "export.sheets" => "Tables for bulk edit in Excel.",
        "export.sheets_csv" => "Tables for bulk edit as CSV files.",
        "export.spec" => "The spec sheet as an Excel file.",
        "export.eplan" => "The project as an EPLAN PXF file.",
        "export.aml" => "The plant as AutomationML.",
        "file.import_dxf_background" => "A DXF drawing behind the page, to trace over.",
        "file.import_dwg" => "A DWG drawing behind the page, through a free converter.",
        "file.choose_dwg_converter" => "Pick the program that turns DWG into DXF.",
        "file.remove_background" => "Takes the DXF drawing away from this page.",
        "file.import_eplan" => "An EPLAN project export (PXF) as pages and devices.",
        "file.eplan_export_script" => "The script that exports an EPLAN project for this import.",
        "file.import_crane" => "A crane configurator project as a new schematic.",
        "file.import_aml" => "A plant in AutomationML.",
        "file.import_controller" => "A controller profile built from an I/O table.",
        "parts.import_folder" => "A whole folder of maker catalog files.",
        "parts.import_table" => "Parts from a CSV or Excel table, with your own columns.",
        "parts.import_eplan" => "Parts from a CAD parts export or maker XML files.",
        "import.spreadsheet" => "Changes made in Excel or CSV, checked before they apply.",
        "plc.import_tags" => "The tags a PLC programmer wrote, matched to the I/O list.",
        "file.new_crane" => "An overhead crane project with its structure tree and covers.",
        "file.new_mcc" => "A motor control centre with feeders, control supply and cabinet.",
        "file.new_pump" => "A pump station with its motor starters and control.",
        "file.example" => "A small motor starter, to try the program on.",
        "file.example_crane" => "An overhead crane cabinet: 2-speed hoist, drives, 48 V control.",
        "file.example_mcc" => "A motor control centre: 4 feeders, 24 V control, full cabinet.",
        "file.example_plc" => "A PLC cabinet: CPU, I/O cards and a 24 V supply.",
        "file.example_pump" => "A pump system with its starter and control.",
        "file.example_emc" => "A drive with a motor cable and signal wires, for the EMC checks.",
        "project.standard" => "The drawing standard: sheet, title block, tag and wire style.",
        "project.rules" => "The rules for wire numbers, tags and numbering of this project.",
        "project.languages" => "The languages the reports and lists are written in.",
        "project.options" => "Machine options and variants, and what each one draws.",
        "project.untie" => "Makes the selected items appear whatever the options are.",
        "parts.browser" => "Search, compare and import parts of the makers' catalogs.",
        "parts.import_step" => "A maker's STEP model, attached to a part for the 3D view.",
        "parts.auto_assign" => "Gives every device without a part the best matching part.",
        "parts.describe" => "Writes the description of each device from its part.",
        "parts.clear" => "Takes the parts off the selected devices.",
        "parts.remove_unused" => "Removes the parts no device uses any more.",
        "wires.number" => "Gives every wire without a number the next free one.",
        "wires.renumber" => "Numbers all wires again, from the scheme and the locks.",
        "wires.check_numbers" => "Lists wires that share a number or break the scheme.",
        "wires.numbering" => "The numbering scheme, ranges and locks of this project.",
        "wires.set_number" => "Types a number on the selected wire's potential and locks it.",
        "wires.unlock_number" => "Lets numbering runs change the selected wire's number again.",
        "edit.renumber" => "Gives the devices new tags in order, page by page.",
        "interruption.renumber" => "Numbers the interruption points of the wires automatically.",
        "plc.renumber_io" => "Gives the PLC inputs and outputs new addresses in order.",
        "project.generate" => "Pages and devices made from an Excel or CSV spec sheet.",
        _ => "",
    }
}

/// Whether the Export centre lists `id`.
pub fn in_export_centre(id: &str) -> bool {
    Centre::of(id) == Some(Centre::Export)
}

/// Whether a centre lists `id`.
pub fn in_centre(id: &str) -> bool {
    Centre::of(id).is_some()
}

/// Whether the centre is the only home of the menu row of `id`: it is
/// listed in a centre and has no row of its own any more.
pub fn replaces_menu_row(id: &str) -> bool {
    in_centre(id) && !ALSO_IN_MENU.contains(&id)
}

/// The command ids of the Export centre, in the order it draws them.
pub fn export_ids() -> impl Iterator<Item = &'static str> {
    Centre::Export.ids()
}

/// The command ids of both centres.
pub fn centre_ids() -> impl Iterator<Item = &'static str> {
    CENTRES.into_iter().flat_map(Centre::ids)
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
    fn the_import_centre_lists_the_old_import_rows_once() {
        let mut seen = HashSet::new();
        for id in Centre::Import.ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
            assert!(!in_export_centre(id), "{id} is in both centres");
        }
        assert_eq!(seen.len(), 14);
    }

    #[test]
    fn the_start_centre_lists_the_templates_and_examples_once() {
        let mut seen = HashSet::new();
        for id in Centre::Start.ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
            assert_eq!(Centre::of(id), Some(Centre::Start), "{id}");
        }
        assert_eq!(seen.len(), 9);
        // Only the small example keeps its row (the Open submenu); the rest live here.
        assert!(replaces_menu_row("file.new_crane") && replaces_menu_row("file.example_emc"));
        assert!(!replaces_menu_row("file.example"));
        assert_eq!(Centre::Start.button("file.new_pump"), "Create");
        assert_eq!(Centre::Start.button("file.example_pump"), "Open");
    }

    #[test]
    fn the_project_centre_lists_the_project_commands_and_the_name_taking_ones_are_palette_only() {
        let mut seen = HashSet::new();
        for id in Centre::Project.ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
            assert_eq!(Centre::of(id), Some(Centre::Project), "{id}");
            assert!(replaces_menu_row(id), "{id}");
        }
        assert_eq!(seen.len(), 6);
        for id in ["project.variant", "project.option", "project.tie"] {
            assert!(!seen.contains(id), "{id} takes a name, it is not a tile");
        }
        assert_eq!(Centre::Project.button("project.generate"), "Generate");
        assert_eq!(Centre::Project.button("project.rules"), "Open");
    }

    #[test]
    fn the_numbering_centre_lists_the_nine_numbering_commands_once() {
        let mut seen = HashSet::new();
        for id in Centre::Numbering.ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
            assert_eq!(Centre::of(id), Some(Centre::Numbering), "{id}");
            assert!(replaces_menu_row(id), "{id}");
        }
        assert_eq!(seen.len(), 9);
        assert_eq!(Centre::Numbering.button("wires.renumber"), "Open");
        assert_eq!(Centre::Numbering.button("wires.number"), "Run");
    }

    #[test]
    fn the_parts_centre_lists_the_six_part_commands_once() {
        let mut seen = HashSet::new();
        for id in Centre::Parts.ids() {
            assert!(get(id).is_some(), "{id} is not a command");
            assert!(seen.insert(id), "{id} is in two sections");
            assert_eq!(Centre::of(id), Some(Centre::Parts), "{id}");
            assert!(replaces_menu_row(id), "{id}");
        }
        assert_eq!(seen.len(), 6);
        assert_eq!(Centre::Parts.button("parts.auto_assign"), "Assign");
        assert_eq!(Centre::Parts.button("parts.remove_unused"), "Remove");
    }

    #[test]
    fn every_listed_command_has_a_sentence() {
        for id in centre_ids() {
            let d = describe(id);
            assert!(d.ends_with('.') && d.len() <= 70, "{id}: {d:?}");
        }
    }

    #[test]
    fn sections_have_a_name_and_a_sentence_and_the_opener_is_a_command() {
        for c in CENTRES {
            for s in c.sections() {
                assert!(!s.name.is_empty() && !s.about.is_empty() && !s.ids.is_empty());
            }
            assert!(get(c.opener()).is_some());
            assert!(!c.button("x").is_empty());
        }
    }
}
