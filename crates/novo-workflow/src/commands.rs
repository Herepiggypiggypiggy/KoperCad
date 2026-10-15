//! Every user command, as data. The keymap and the command field list
//! commands from here; with an empty query the command field lists them all
//! by group, which is the menu (there is no menu bar). Ids match the app
//! shell's action ids (`file.new`, ...).

use crate::keymap::{FAVOURITE_COMMANDS, Keymap};

/// One command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandInfo {
    /// Stable id, used by the keymap, recents and scripts.
    pub id: &'static str,
    pub label: &'static str,
    /// One of [`GROUPS`]; the empty command field lists commands by group.
    pub group: &'static str,
    /// Extra words the search matches ("renumber" also finds "numbering").
    pub keywords: &'static [&'static str],
    /// Phrases that make this command the verb of a typed command
    /// (`renumber page`, `export pdf`). Matched as whole words at the start
    /// of the query; what follows is the command's argument.
    pub verbs: &'static [&'static str],
    /// Values the command field offers as chips after the verb, so the
    /// command can be built by picking rows instead of typing.
    pub arguments: &'static [&'static str],
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

/// Command groups, in the order the empty command field lists them.
pub const GROUPS: [&str; 7] = [
    "Project", "Draw", "Edit", "Navigate", "View", "Check", "Export",
];

const fn cmd(
    id: &'static str,
    label: &'static str,
    group: &'static str,
    keywords: &'static [&'static str],
    needs: Needs,
) -> CommandInfo {
    CommandInfo {
        id,
        label,
        group,
        keywords,
        verbs: &[],
        arguments: &[],
        needs,
    }
}

const fn with_arguments(mut info: CommandInfo, arguments: &'static [&'static str]) -> CommandInfo {
    info.arguments = arguments;
    info
}

const fn verb(mut info: CommandInfo, verbs: &'static [&'static str]) -> CommandInfo {
    info.verbs = verbs;
    info
}

/// Every command, by group.
pub const COMMANDS: &[CommandInfo] = &[
    // Project
    cmd(
        "file.new",
        "New project",
        "Project",
        &["create"],
        Needs::Nothing,
    ),
    cmd(
        "file.open",
        "Open project",
        "Project",
        &["load"],
        Needs::Nothing,
    ),
    cmd(
        "file.example",
        "Open example",
        "Project",
        &["sample", "motor starter", "demo"],
        Needs::Nothing,
    ),
    cmd(
        "file.practice",
        "Try the fast keys",
        "Project",
        &["practice", "tutorial", "learn"],
        Needs::Nothing,
    ),
    cmd("file.save", "Save", "Project", &[], Needs::Project),
    cmd(
        "file.save_as",
        "Save as",
        "Project",
        &["copy"],
        Needs::Project,
    ),
    cmd(
        "file.close",
        "Close project",
        "Project",
        &[],
        Needs::Project,
    ),
    verb(
        cmd(
            "page.new",
            "New page",
            "Project",
            &["add page", "page"],
            Needs::Project,
        ),
        // Not a bare "page": `page 2` goes to page 2.
        &["new page", "add page"],
    ),
    verb(
        cmd(
            "project.rules",
            "Rules",
            "Project",
            &["numbering", "settings"],
            Needs::Project,
        ),
        &["rules"],
    ),
    verb(
        cmd(
            "project.standard",
            "Drawing standard",
            "Project",
            &["iec", "nfpa", "standard", "symbols", "project settings"],
            Needs::Project,
        ),
        &["standard"],
    ),
    verb(
        cmd(
            "app.preferences",
            "Preferences",
            "Project",
            &["settings", "options", "auto connections", "autoconnect"],
            Needs::Nothing,
        ),
        &["preferences", "prefs"],
    ),
    verb(
        cmd(
            "parts.browser",
            "Parts",
            "Project",
            &[
                "part library",
                "catalog",
                "import part",
                "wago",
                "phoenix",
                "schneider",
                "siemens",
            ],
            Needs::Nothing,
        ),
        &["parts", "part library"],
    ),
    verb(
        cmd(
            "revision.issue",
            "Issue revision",
            "Project",
            &["release", "checkpoint", "new revision", "freeze"],
            Needs::Project,
        ),
        &["issue", "revision"],
    ),
    verb(
        cmd(
            "revision.compare",
            "Compare revisions",
            "Project",
            &["diff", "what changed", "between revisions", "revision list"],
            Needs::Project,
        ),
        &["compare"],
    ),
    cmd("file.quit", "Quit", "Project", &["exit"], Needs::Nothing),
    verb(
        cmd(
            "file.import_dxf_background",
            "Import DXF as background",
            "Project",
            &["underlay", "drawing", "dwg", "reference"],
            Needs::Page,
        ),
        &["import dxf"],
    ),
    cmd(
        "file.remove_background",
        "Remove the DXF background",
        "Project",
        &["underlay", "dxf", "reference drawing"],
        Needs::Page,
    ),
    // Draw
    cmd(
        "tool.repeat",
        "Repeat last tool",
        "Draw",
        &["again"],
        Needs::Page,
    ),
    cmd(
        "tool.place",
        "Place symbol",
        "Draw",
        &["insert", "device", "component"],
        Needs::Page,
    ),
    cmd(
        "tool.connect",
        "Connect",
        "Draw",
        &["wire", "chain"],
        Needs::Page,
    ),
    // Shift+W with no argument switches; the argument names the mode. The
    // mode is a user preference, so no project or page is needed.
    with_arguments(
        verb(
            cmd(
                "wire.mode",
                "Wiring mode",
                "Draw",
                &[
                    "mode", "corners", "lines", "eplan", "autocad", "draw", "wire", "route",
                ],
                Needs::Nothing,
            ),
            &["wiring mode"],
        ),
        &["corners", "lines"],
    ),
    // C: draw a line; every wire it touches becomes a core of one cable.
    cmd(
        "tool.cable",
        "Cable",
        "Draw",
        &["cable", "cores", "bundle"],
        Needs::Page,
    ),
    // Only while a wire is being drawn (Keymap::resolve_while_wiring).
    cmd(
        "wire.end_on_part",
        "End wire on part",
        "Draw",
        &["terminate", "end wire", "finish on part"],
        Needs::Page,
    ),
    cmd(
        "wire.next_pin",
        "Next connection point",
        "Draw",
        &["cycle pin", "next pin", "next connection"],
        Needs::Page,
    ),
    cmd("edit.rotate", "Rotate", "Draw", &["turn"], Needs::Selection),
    cmd("edit.mirror", "Mirror", "Draw", &["flip"], Needs::Selection),
    // PLC and I/O (docs/design/plc.md section 4): the command surface only
    // names them; the app shell runs them through novo-plc.
    verb(
        cmd(
            "plc.add_card",
            "Add PLC card",
            "Draw",
            &["plc", "card", "module", "i/o"],
            Needs::Page,
        ),
        &["add card"],
    ),
    verb(
        cmd(
            "plc.to_plc",
            "Send signal to PLC",
            "Draw",
            &["plc", "channel", "input", "output", "i/o"],
            Needs::Selection,
        ),
        &["to plc"],
    ),
    verb(
        cmd(
            "proposal.accept",
            "Accept proposal",
            "Draw",
            &[],
            Needs::Page,
        ),
        &["accept"],
    ),
    verb(
        cmd(
            "proposal.accept_all",
            "Accept all proposals",
            "Draw",
            &[],
            Needs::Page,
        ),
        &["accept all"],
    ),
    cmd("proposal.next", "Next proposal", "Draw", &[], Needs::Page),
    verb(
        cmd(
            "draw.tidy",
            "Tidy routes",
            "Draw",
            &["reroute"],
            Needs::Selection,
        ),
        &["tidy"],
    ),
    // Edit
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
        "Duplicate",
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
        "Rename",
        "Edit",
        &["tag", "edit tag"],
        Needs::Device,
    ),
    with_arguments(
        verb(
            cmd(
                "edit.renumber",
                "Renumber devices",
                "Edit",
                &["numbering", "number"],
                Needs::Project,
            ),
            &["renumber"],
        ),
        &["page", "selection", "project"],
    ),
    verb(
        cmd(
            "plc.renumber_io",
            "Renumber I/O",
            "Edit",
            &["plc", "addresses", "i/o"],
            Needs::Project,
        ),
        &["renumber io"],
    ),
    // Bare `link` asks for the contact, then the coil (LINK_PICK_CONTACT,
    // LINK_PICK_COIL), previews and runs on Enter; `link -K3 -K1` names the
    // devices (see `palette::link_tags`). The app shell runs it through
    // novo-xref, which also gives the refusal text.
    verb(
        cmd(
            "xref.link",
            "Link cross reference",
            "Edit",
            &["contact", "coil", "join contact"],
            Needs::Page,
        ),
        &["link"],
    ),
    cmd("edit.escape", "Cancel", "Edit", &["escape"], Needs::Nothing),
    // Terminals
    cmd(
        "terminal.connect",
        "Connect terminals",
        "Edit",
        &["jumper", "bridge", "cross connect"],
        Needs::Selection,
    ),
    verb(
        cmd(
            "terminal.toggle_pe",
            "Toggle PE terminal",
            "Edit",
            &[
                "pe",
                "protective earth",
                "earth terminal",
                "ground terminal",
            ],
            Needs::Selection,
        ),
        &["pe"],
    ),
    cmd(
        "terminal.disconnect",
        "Disconnect terminals",
        "Edit",
        &["jumper", "bridge", "unbridge"],
        Needs::Selection,
    ),
    cmd(
        "terminal.reset_pe",
        "Reset PE override",
        "Edit",
        &["protective earth", "automatic pe"],
        Needs::Selection,
    ),
    cmd(
        "terminal.strip_up",
        "Terminal strip number up",
        "Edit",
        &["rename strip", "x number", "next strip"],
        Needs::Selection,
    ),
    cmd(
        "terminal.strip_down",
        "Terminal strip number down",
        "Edit",
        &["rename strip", "x number", "previous strip"],
        Needs::Selection,
    ),
    cmd(
        "terminal.strip_lowest",
        "Terminal strip lowest free number",
        "Edit",
        &["rename strip", "x number", "free strip"],
        Needs::Selection,
    ),
    cmd(
        "interruption.renumber",
        "Number interruption points automatically",
        "Edit",
        &["jump", "renumber", "continuation", "off page"],
        Needs::Project,
    ),
    // Navigate
    cmd(
        "nav.partner",
        "Go to partner",
        "Navigate",
        &["cross reference", "coil", "contact"],
        Needs::Device,
    ),
    cmd(
        "nav.all_references",
        "Every occurrence",
        "Navigate",
        &["cross reference", "references", "where used"],
        Needs::Device,
    ),
    cmd(
        "nav.back",
        "Back",
        "Navigate",
        &[],
        Needs::History(Direction::Back),
    ),
    cmd(
        "nav.forward",
        "Forward",
        "Navigate",
        &[],
        Needs::History(Direction::Forward),
    ),
    cmd(
        "nav.previous_page",
        "Previous page",
        "Navigate",
        &[],
        Needs::Page,
    ),
    cmd("nav.next_page", "Next page", "Navigate", &[], Needs::Page),
    verb(
        cmd(
            "nav.changes",
            "Changes since export",
            "Navigate",
            &["changed"],
            Needs::Project,
        ),
        &["changes"],
    ),
    // View
    cmd(
        "view.command_palette",
        "Command field",
        "View",
        &["search", "find", "palette"],
        Needs::Nothing,
    ),
    cmd(
        "view.command_field_here",
        "Command field at the pointer",
        "View",
        &[],
        Needs::Page,
    ),
    cmd(
        "view.fit",
        "Zoom to fit",
        "View",
        &["zoom", "extents"],
        Needs::Page,
    ),
    cmd(
        "view.schematic",
        "Schematic",
        "View",
        &["lens"],
        Needs::Project,
    ),
    cmd(
        "view.cabinet",
        "Cabinet",
        "View",
        &["lens", "layout"],
        Needs::Project,
    ),
    cmd(
        "view.terminals_drawer",
        "Terminals",
        "View",
        &["drawer", "strips", "connections"],
        Needs::Project,
    ),
    cmd(
        "view.cables_drawer",
        "Cables",
        "View",
        &["drawer", "cores", "connections"],
        Needs::Project,
    ),
    cmd(
        "view.devices_drawer",
        "Devices",
        "View",
        &["drawer", "parts", "locations"],
        Needs::Project,
    ),
    cmd(
        "view.focus",
        "Focus mode",
        "View",
        &["distraction free"],
        Needs::Nothing,
    ),
    cmd(
        "view.pin_outline",
        "Pin outline",
        "View",
        &["outline", "pages", "sidebar", "unpin"],
        Needs::Nothing,
    ),
    cmd(
        "view.full_screen",
        "Full screen",
        "View",
        &[],
        Needs::Nothing,
    ),
    cmd(
        "cabinet.size-enclosure",
        "Size the enclosure",
        "View",
        &["enclosure", "cabinet", "heat", "cooling", "plate"],
        Needs::Project,
    ),
    cmd(
        "cabinet.enclosures",
        "Enclosures",
        "View",
        &["enclosure", "list", "sizes", "catalog"],
        Needs::Project,
    ),
    cmd(
        "view.cabinet_table",
        "Cabinet as table",
        "View",
        &["table"],
        Needs::Project,
    ),
    cmd(
        "view.toggle_marks",
        "Show or hide marks",
        "View",
        &["problems", "proposals"],
        Needs::Page,
    ),
    verb(
        cmd(
            "view.changes",
            "Show changes since a revision",
            "View",
            &["compare", "what changed", "diff", "revision marks"],
            Needs::Project,
        ),
        &["changes"],
    ),
    verb(
        cmd(
            "view.commissioning",
            "Commissioning",
            "View",
            &[
                "commission",
                "punch list",
                "check off",
                "continuity",
                "torque",
            ],
            Needs::Project,
        ),
        &["commissioning"],
    ),
    cmd(
        "sim.live",
        "Show live potentials",
        "View",
        &["simulate", "contacts", "close", "energized", "try"],
        Needs::Page,
    ),
    cmd(
        "view.override_marks",
        "Show override marks",
        "View",
        &["overrides", "manual"],
        Needs::Page,
    ),
    cmd(
        "help.walkthrough",
        "Fast keys walk-through",
        "View",
        &["tour", "tutorial", "walkthrough", "help"],
        Needs::Nothing,
    ),
    cmd(
        "view.shortcuts",
        "Keyboard shortcuts",
        "View",
        &["keys", "help", "overlay"],
        Needs::Nothing,
    ),
    cmd(
        "view.background",
        "Show or hide the DXF background",
        "View",
        &["underlay", "dxf", "reference drawing"],
        Needs::Page,
    ),
    // Check
    verb(
        cmd(
            "check.problems",
            "Problems",
            "Check",
            &["errors", "checks"],
            Needs::Project,
        ),
        &["problems"],
    ),
    cmd(
        "check.next_problem",
        "Next problem",
        "Check",
        &[],
        Needs::Project,
    ),
    cmd(
        "check.previous_problem",
        "Previous problem",
        "Check",
        &[],
        Needs::Project,
    ),
    // Export
    verb(
        cmd(
            "export.dxf_page",
            "Export this page as DXF",
            "Export",
            &["cad", "autocad"],
            Needs::Page,
        ),
        &["export dxf"],
    ),
    cmd(
        "export.dxf_all",
        "Export all pages as DXF",
        "Export",
        &["cad", "autocad"],
        Needs::Project,
    ),
    verb(
        cmd(
            "export.pdf",
            "Export PDF",
            "Export",
            &["print"],
            Needs::Project,
        ),
        &["export pdf", "pdf"],
    ),
    verb(
        cmd(
            "import.spreadsheet",
            "Import spreadsheet",
            "Import",
            &["excel", "csv", "xlsx", "device list"],
            Needs::Project,
        ),
        &["import spreadsheet", "import sheet", "import excel"],
    ),
    verb(
        cmd(
            "export.bom",
            "Export bill of materials",
            "Export",
            &["parts list"],
            Needs::Project,
        ),
        &["export bom", "bom"],
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
    fn groups_are_known_and_in_order() {
        let mut last = 0;
        for c in COMMANDS {
            let g = GROUPS.iter().position(|g| *g == c.group).expect(c.id);
            assert!(g >= last, "{} is out of group order", c.id);
            last = g;
        }
    }

    #[test]
    fn every_key_row_has_a_shortcut() {
        let map = Keymap::workflow();
        for id in [
            "view.command_palette",
            "edit.duplicate",
            "nav.all_references",
            "nav.back",
            "proposal.next",
            "check.next_problem",
            "view.devices_drawer",
            "view.focus",
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
