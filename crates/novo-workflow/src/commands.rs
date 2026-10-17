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
        "file.open_read_only",
        "Open read-only",
        "Project",
        &["view only", "look", "locked", "share"],
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
    cmd(
        "project.languages",
        "Report languages",
        "Project",
        &[
            "language",
            "translate",
            "dutch",
            "nederlands",
            "pdf language",
            "translation",
        ],
        Needs::Project,
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
    // The page-tree actions of the outline, for the palette and the menus.
    // They act on the page that is open.
    cmd(
        "page.rename",
        "Rename page",
        "Project",
        &["title", "page title"],
        Needs::Page,
    ),
    cmd(
        "page.duplicate",
        "Duplicate page",
        "Project",
        &["copy page", "clone page"],
        Needs::Page,
    ),
    cmd(
        "page.delete",
        "Delete page",
        "Project",
        &["remove page"],
        Needs::Page,
    ),
    cmd(
        "page.move_up",
        "Move page up",
        "Project",
        &["reorder", "earlier"],
        Needs::Page,
    ),
    cmd(
        "page.move_down",
        "Move page down",
        "Project",
        &["reorder", "later"],
        Needs::Page,
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
            "macro.browser",
            "Macros",
            "Project",
            &["macro", "block", "library", "circuit", "starter", "share"],
            Needs::Nothing,
        ),
        &["macros", "macro"],
    ),
    verb(
        cmd(
            "macro.import",
            "Import macros",
            "Project",
            &["macro", "import", "kcmacro", "share"],
            Needs::Nothing,
        ),
        &["import macro"],
    ),
    verb(
        cmd(
            "macro.export_pack",
            "Export macro pack",
            "Project",
            &["macro", "pack", "export", "kcmacropack", "share", "team"],
            Needs::Nothing,
        ),
        &["export macros", "macro pack"],
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
            "parts.import_folder",
            "Import parts from a folder",
            "Project",
            &[
                "bulk",
                "catalog",
                "bmecat",
                "csv",
                "inbox",
                "maker",
                "parts import",
            ],
            Needs::Nothing,
        ),
        &["import parts", "import catalog"],
    ),
    verb(
        cmd(
            "parts.import_eplan",
            "Import parts from a CAD parts export",
            "Project",
            &[
                "eplan parts",
                "parts",
                "artikel",
                "xml",
                "csv",
                "catalog",
                "migrate",
                "other tool",
            ],
            Needs::Nothing,
        ),
        &["import cad parts"],
    ),
    verb(
        cmd(
            "parts.import_step",
            "Import 3D model (STEP)",
            "Project",
            &["step", "stp", "3d model", "mesh", "cad model"],
            Needs::Nothing,
        ),
        &["import 3d", "import step"],
    ),
    verb(
        cmd(
            "project.options",
            "Project options and variants",
            "Project",
            &[
                "options",
                "variants",
                "configuration",
                "conveyor",
                "with without",
                "machine options",
            ],
            Needs::Project,
        ),
        &["options", "variants"],
    ),
    verb(
        cmd(
            "project.variant",
            "Switch variant",
            "Project",
            &["variant", "configuration", "machine"],
            Needs::Project,
        ),
        &["variant"],
    ),
    verb(
        cmd(
            "project.generate",
            "Generate pages from a spec sheet",
            "Project",
            &[
                "spec sheet",
                "generate project",
                "from excel",
                "from csv",
                "macros",
            ],
            Needs::Project,
        ),
        &["generate"],
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
    cmd(
        "help.about",
        "About",
        "Project",
        &["version", "credits", "kopervonk"],
        Needs::Nothing,
    ),
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
    verb(
        cmd(
            "file.import_dwg",
            "Import DWG as background",
            "Project",
            &["autocad", "converter", "libredwg", "oda"],
            Needs::Page,
        ),
        &["import dwg"],
    ),
    verb(
        cmd(
            "file.eplan_export_script",
            "Get the EPLAN export script",
            "Project",
            &[
                "eplan", "export", "pxf", "epj", "migrate", "bridge", "script",
            ],
            Needs::Nothing,
        ),
        &["get eplan script"],
    ),
    cmd(
        "file.choose_dwg_converter",
        "Choose the DWG converter",
        "Project",
        &["dwg2dxf", "libredwg", "oda file converter"],
        Needs::Nothing,
    ),
    verb(
        cmd(
            "file.import_aml",
            "Import AutomationML",
            "Project",
            &[
                "aml",
                "caex",
                "plant",
                "migrate",
                "other tool",
                "engineering",
            ],
            Needs::Project,
        ),
        &["import aml", "import automationml"],
    ),
    verb(
        cmd(
            "file.import_eplan",
            "Import EPLAN project (PXF)",
            "Project",
            &[
                "eplan",
                "pxf",
                "epj",
                "emp",
                "migrate",
                "other tool",
                "page macro",
            ],
            Needs::Project,
        ),
        &["import eplan", "import pxf"],
    ),
    cmd(
        "file.remove_background",
        "Remove the DXF background",
        "Project",
        &["underlay", "dxf", "reference drawing"],
        Needs::Page,
    ),
    // Draw
    verb(
        cmd(
            "import.spreadsheet",
            "Import spreadsheet",
            "Project",
            &["excel", "csv", "xlsx", "device list"],
            Needs::Project,
        ),
        &["import spreadsheet", "import sheet", "import excel"],
    ),
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
    // Shift+T: a click on the page, then the words of a free note.
    cmd(
        "draw.text",
        "Add text",
        "Draw",
        &["note", "annotation", "comment"],
        Needs::Page,
    ),
    // Cables that exist: the Cables drawer (Ctrl+4) shows the one they act on.
    verb(
        cmd(
            "cable.rename",
            "Rename cable",
            "Draw",
            &["cable tag", "-W1"],
            Needs::Project,
        ),
        &["rename cable"],
    ),
    cmd(
        "cable.delete",
        "Delete cable",
        "Draw",
        &["remove cable", "cores"],
        Needs::Project,
    ),
    verb(
        cmd(
            "cable.retype",
            "Change cable type",
            "Draw",
            &["cores", "4G1.5", "shielded", "section"],
            Needs::Project,
        ),
        &["cable type"],
    ),
    cmd(
        "cable.restore_proposals",
        "Restore declined cable proposals",
        "Draw",
        &["declined", "undo decline", "propose again"],
        Needs::Project,
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
    cmd(
        "edit.reset_note",
        "Reset note position",
        "Draw",
        &["note", "cross reference", "contact image", "label"],
        Needs::Selection,
    ),
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
    cmd(
        "plc.overview",
        "PLC overview (cards, channels, addresses)",
        "Draw",
        &[
            "plc",
            "cards",
            "channels",
            "addresses",
            "i/o",
            "address plan",
        ],
        Needs::Project,
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
        "Paste",
        "Edit",
        &["smart", "next free tags"],
        Needs::Clipboard,
    ),
    cmd(
        "edit.paste_keep_tags",
        "Paste, keep the original tags",
        "Edit",
        &[],
        Needs::Clipboard,
    ),
    cmd(
        "edit.paste_in_place",
        "Paste in place",
        "Edit",
        &["same position"],
        Needs::Clipboard,
    ),
    cmd(
        "edit.duplicate",
        "Duplicate",
        "Edit",
        &["copy", "clone"],
        Needs::Selection,
    ),
    verb(
        cmd(
            "macro.save",
            "Save selection as macro",
            "Edit",
            &["macro", "block", "reuse", "template", "library"],
            Needs::Selection,
        ),
        &["save macro"],
    ),
    verb(
        cmd(
            "macro.place",
            "Place macro",
            "Edit",
            &["macro", "block", "insert", "starter", "library"],
            Needs::Page,
        ),
        &["place macro", "macro"],
    ),
    verb(
        cmd(
            "macro.edit",
            "Edit macro parameters",
            "Edit",
            &[
                "macro",
                "block",
                "parameter",
                "default",
                "author",
                "library",
            ],
            Needs::Nothing,
        ),
        &["edit macro"],
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
        "edit.shared_properties",
        "Properties of the selection",
        "Edit",
        &[
            "property sheet",
            "bulk edit",
            "multiple",
            "shared",
            "inspector",
            "fields",
            "edit all",
        ],
        Needs::Selection,
    ),
    cmd(
        "edit.rename",
        "Rename",
        "Edit",
        &["tag", "edit tag"],
        Needs::Device,
    ),
    verb(
        cmd(
            "parts.auto_assign",
            "Assign parts to all devices",
            "Edit",
            &["auto assign", "propose", "catalog", "part", "fill"],
            Needs::Project,
        ),
        &["assign parts", "auto assign parts"],
    ),
    cmd(
        "parts.clear",
        "Remove parts from selected devices",
        "Edit",
        &["clear part", "unassign", "article"],
        Needs::Selection,
    ),
    cmd(
        "parts.remove_unused",
        "Remove unused parts",
        "Edit",
        &["unused", "clean up", "purge", "article"],
        Needs::Project,
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
    // The whole net of the selected wire, junction or part: select it on this
    // page, or list the devices and pages it reaches.
    cmd(
        "net.select",
        "Select net",
        "Edit",
        &["whole net", "connected wires"],
        Needs::Selection,
    ),
    cmd(
        "net.trace",
        "Pages and devices on this net",
        "Edit",
        &["where does this go", "net devices", "net pages"],
        Needs::Selection,
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
    verb(
        cmd(
            "terminal.add_spares",
            "Add spare terminals",
            "Edit",
            &["spare", "strip", "extra terminals"],
            Needs::Project,
        ),
        &["spares"],
    ),
    cmd(
        "terminal.rename",
        "Rename terminals (find and replace)",
        "Edit",
        &["strip", "find", "replace", "terminal name"],
        Needs::Project,
    ),
    cmd(
        "terminal.renumber",
        "Renumber terminals",
        "Edit",
        &["strip", "1 2 3", "close gaps", "terminal number"],
        Needs::Project,
    ),
    cmd(
        "terminal.join_duplicates",
        "Join duplicate terminals",
        "Edit",
        &["strip", "merge", "same name", "duplicate"],
        Needs::Project,
    ),
    with_arguments(
        cmd(
            "terminal.set_kind",
            "Set terminal kind",
            "Edit",
            &[
                "strip",
                "pe",
                "neutral",
                "fuse",
                "disconnect",
                "feed-through",
            ],
            Needs::Project,
        ),
        &["feed-through", "PE", "N", "fuse", "disconnect"],
    ),
    cmd(
        "terminal.order_up",
        "Move terminal up in strip",
        "Edit",
        &["strip", "reorder", "sort"],
        Needs::Project,
    ),
    cmd(
        "terminal.order_down",
        "Move terminal down in strip",
        "Edit",
        &["strip", "reorder", "sort"],
        Needs::Project,
    ),
    cmd(
        "terminal.style",
        "Terminal strip style",
        "Edit",
        &["strip", "symbol", "terminal look", "override"],
        Needs::Project,
    ),
    cmd(
        "terminal.restore_declined",
        "Restore declined terminal proposals",
        "Edit",
        &["undo decline", "proposal", "terminals", "declined"],
        Needs::Project,
    ),
    cmd(
        "interruption.renumber",
        "Number interruption points automatically",
        "Edit",
        &["jump", "renumber", "continuation", "off page"],
        Needs::Project,
    ),
    // Wire numbering (novo-numbering). `wires.number` fills what has no
    // number; the window shows the scheme, a preview and the conflicts.
    cmd(
        "wires.number",
        "Number wires",
        "Edit",
        &["wire number", "numbering", "potential", "automatic", "fill"],
        Needs::Project,
    ),
    verb(
        cmd(
            "wires.numbering",
            "Wire numbering",
            "Edit",
            &["scheme", "wire number", "numbering", "preview"],
            Needs::Project,
        ),
        &["wire numbering"],
    ),
    verb(
        cmd(
            "wires.renumber",
            "Renumber wires",
            "Edit",
            &["wire number", "numbering", "start over"],
            Needs::Project,
        ),
        &["renumber wires"],
    ),
    cmd(
        "wires.check_numbers",
        "Check wire numbers",
        "Edit",
        &["duplicate", "conflict", "wire number", "numbering"],
        Needs::Project,
    ),
    verb(
        cmd(
            "wires.set_number",
            "Set wire number",
            "Edit",
            &["type", "manual", "lock", "wire number"],
            Needs::Selection,
        ),
        &["number wire"],
    ),
    cmd(
        "wires.unlock_number",
        "Unlock wire number",
        "Edit",
        &["manual", "lock", "wire number"],
        Needs::Selection,
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
        "nav.show_schematic",
        "Show in schematic",
        "Navigate",
        &["cross probe", "find in drawing", "main symbol"],
        Needs::Project,
    ),
    cmd(
        "nav.show_cabinet",
        "Show in Cabinet",
        "Navigate",
        &["cross probe", "find on plate", "layout"],
        Needs::Project,
    ),
    cmd(
        "nav.show_3d",
        "Show in 3D",
        "Navigate",
        &["cross probe", "find in cabinet", "zoom to device"],
        Needs::Project,
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
        &["zoom", "extents", "drawing", "content"],
        Needs::Page,
    ),
    cmd(
        "view.fit_sheet",
        "Fit whole sheet",
        "View",
        &["zoom", "paper", "page", "extents"],
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
        "view.3d",
        "3D view",
        "View",
        &["3d", "cabinet in 3d", "orbit", "render"],
        Needs::Project,
    ),
    cmd(
        "view.3d_door",
        "Open or close the door in 3D",
        "View",
        &["door", "open door", "close door", "swing"],
        Needs::Project,
    ),
    cmd(
        "view.3d_shell",
        "Show or hide the enclosure in 3D",
        "View",
        &["shell", "enclosure", "box", "see through"],
        Needs::Project,
    ),
    cmd(
        "view.3d_wires",
        "Show or hide the wires in 3D",
        "View",
        &["wires", "routes", "cables"],
        Needs::Project,
    ),
    cmd(
        "view.3d_shadows",
        "Show or hide shadows in 3D",
        "View",
        &["shadows", "light", "soft shadows"],
        Needs::Project,
    ),
    cmd(
        "view.3d_measure",
        "Measure distances in 3D",
        "View",
        &["measure", "distance", "ruler", "gap", "clearance"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_left",
        "Align left in 3D",
        "View",
        &["align", "left", "edges"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_hcentre",
        "Centre devices across in 3D",
        "View",
        &["align", "centre", "center", "vertical line"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_right",
        "Align right in 3D",
        "View",
        &["align", "right", "edges"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_top",
        "Align top in 3D",
        "View",
        &["align", "top", "edges"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_vcentre",
        "Centre devices up and down in 3D",
        "View",
        &["align", "middle", "horizontal line"],
        Needs::Project,
    ),
    cmd(
        "view.3d_align_bottom",
        "Align bottom in 3D",
        "View",
        &["align", "bottom", "edges"],
        Needs::Project,
    ),
    cmd(
        "view.3d_spread_h",
        "Spread devices across in 3D",
        "View",
        &["distribute", "spread", "equal space", "gaps"],
        Needs::Project,
    ),
    cmd(
        "view.3d_spread_v",
        "Spread devices down in 3D",
        "View",
        &["distribute", "spread", "equal space", "gaps"],
        Needs::Project,
    ),
    cmd(
        "view.3d_wire_runs",
        "Show the wire runs and lengths in 3D",
        "View",
        &["wire runs", "wire lengths", "cut length", "routes", "ducts"],
        Needs::Project,
    ),
    cmd(
        "view.3d_ssao",
        "Show or hide ambient occlusion in 3D",
        "View",
        &["occlusion", "ao", "ssao", "contact shadows", "corners"],
        Needs::Project,
    ),
    cmd(
        "view.3d_duplicate",
        "Duplicate the selected devices in 3D",
        "View",
        &["duplicate", "copy", "clone", "same part"],
        Needs::Project,
    ),
    cmd(
        "view.3d_position",
        "Show or hide the Position box in 3D",
        "View",
        &["position", "coordinates", "offset", "nudge", "x y"],
        Needs::Project,
    ),
    cmd(
        "view.3d_place",
        "Place a device in 3D",
        "View",
        &["place", "put", "add device", "unplaced", "drop"],
        Needs::Project,
    ),
    cmd(
        "view.3d_dimensions",
        "Show or hide dimensions in 3D",
        "View",
        &["dimensions", "measure", "distance", "mm", "sizes"],
        Needs::Project,
    ),
    cmd(
        "view.3d_quality",
        "Restore 3D quality",
        "View",
        &["quality", "slow", "full quality", "reduced"],
        Needs::Project,
    ),
    cmd(
        "view.3d_stats",
        "Show or hide the 3D frame statistics",
        "View",
        &["frame rate", "fps", "performance", "gpu", "speed"],
        Needs::Project,
    ),
    cmd(
        "view.3d_section",
        "Cut the 3D view with a section plane",
        "View",
        &[
            "section",
            "cut",
            "clip",
            "slice",
            "see inside",
            "cross section",
        ],
        Needs::Project,
    ),
    cmd(
        "view.3d_save_view",
        "Save the current 3D camera as a view",
        "View",
        &["save view", "bookmark", "camera", "remember view"],
        Needs::Project,
    ),
    cmd(
        "view.3d_hints",
        "Show or hide the mouse and key hints in 3D",
        "View",
        &[
            "hints",
            "help",
            "mouse",
            "controls",
            "legend",
            "how to move",
        ],
        Needs::Project,
    ),
    cmd(
        "view.3d_select_devices",
        "Select all devices in 3D",
        "View",
        &["select devices", "all devices", "select all"],
        Needs::Project,
    ),
    cmd(
        "view.3d_select_rails",
        "Select all rails in 3D",
        "View",
        &["select rails", "all rails", "din rail"],
        Needs::Project,
    ),
    cmd(
        "view.3d_select_ducts",
        "Select all ducts in 3D",
        "View",
        &["select ducts", "all ducts", "wire duct"],
        Needs::Project,
    ),
    cmd(
        "view.3d_hide",
        "Hide the selection in 3D",
        "View",
        &["hide", "hide selection", "conceal"],
        Needs::Project,
    ),
    cmd(
        "view.3d_isolate",
        "Isolate the selection in 3D",
        "View",
        &["isolate", "only this", "focus selection"],
        Needs::Project,
    ),
    cmd(
        "view.3d_show_all",
        "Show everything hidden in 3D",
        "View",
        &["show all", "unhide", "reveal"],
        Needs::Project,
    ),
    cmd(
        "view.iso",
        "Isometric view (of the selection, if any)",
        "View",
        &["3d", "corner view", "perspective"],
        Needs::Project,
    ),
    cmd(
        "view.front",
        "Front view (of the selection, if any)",
        "View",
        &["3d", "door side", "look at the door"],
        Needs::Project,
    ),
    cmd(
        "view.top",
        "Top view (of the selection, if any)",
        "View",
        &["3d", "from above", "plan"],
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
    verb(
        cmd(
            "cabinet.place-all",
            "Place all devices (Cabinet)",
            "View",
            &["place", "fill plate", "auto place", "layout", "din rail"],
            Needs::Project,
        ),
        &["place all"],
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
        "view.protection",
        "Protection: curves, selectivity, rating",
        "View",
        &[
            "protection",
            "trip",
            "curve",
            "selectivity",
            "sccr",
            "breaker",
            "fuse",
        ],
        Needs::Project,
    ),
    cmd(
        "simulate.motor-start",
        "Simulate motor start",
        "View",
        &[
            "simulate",
            "motor",
            "start",
            "star-delta",
            "soft starter",
            "inrush",
        ],
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
        "heat.toggle",
        "Show or hide heat",
        "View",
        &["heat", "temperature", "thermal", "map", "cabinet"],
        Needs::Project,
    ),
    cmd(
        "heat.simulate",
        "Simulate heat of the cabinet",
        "View",
        &[
            "heat",
            "simulate",
            "thermal",
            "temperature",
            "cabinet",
            "loss",
        ],
        Needs::Project,
    ),
    cmd(
        "heat.airflow",
        "Show or hide airflow",
        "View",
        &["heat", "air", "flow", "streamlines", "cabinet"],
        Needs::Project,
    ),
    cmd(
        "heat.contours",
        "Show or hide heat contours",
        "View",
        &["heat", "contour", "lines", "isotherms", "cabinet"],
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
        "Simulate live potentials",
        "View",
        &["simulate", "contacts", "close", "energized", "try"],
        Needs::Page,
    ),
    cmd(
        "sim.plc_program",
        "Simulate PLC program",
        "View",
        &[
            "plc",
            "ladder",
            "structured text",
            "run program",
            "simulate plc",
        ],
        Needs::Page,
    ),
    cmd(
        "sim.faults",
        "Simulate fault scenarios",
        "View",
        &[
            "short circuit",
            "fault",
            "trip",
            "breaker",
            "selectivity",
            "protection",
            "waveform",
            "timing",
            "overload",
        ],
        Needs::Project,
    ),
    cmd(
        "sim.add_fault",
        "Add fault to selection",
        "View",
        &[
            "add fault",
            "open circuit",
            "short to earth",
            "stuck contact",
            "what if",
            "failure",
        ],
        Needs::Selection,
    ),
    cmd(
        "view.override_marks",
        "Show override marks",
        "View",
        &["overrides", "manual"],
        Needs::Page,
    ),
    cmd(
        "help.keymap_file",
        "Open keymap file",
        "View",
        &["rebind", "customise keys", "change shortcuts", "keymap.txt"],
        Needs::Nothing,
    ),
    cmd(
        "help.show_hints",
        "Show hints again",
        "View",
        &["hints", "tips", "reset", "help"],
        Needs::Nothing,
    ),
    cmd(
        "layout.draw",
        "Layout: Draw",
        "View",
        &["workspace", "preset", "schematic", "drawing"],
        Needs::Project,
    ),
    cmd(
        "layout.cabinet",
        "Layout: Cabinet",
        "View",
        &["workspace", "preset", "plate", "panel"],
        Needs::Project,
    ),
    cmd(
        "layout.3d",
        "Layout: 3D and heat",
        "View",
        &["workspace", "preset", "3d", "heat", "thermal"],
        Needs::Project,
    ),
    cmd(
        "layout.review",
        "Layout: Review",
        "View",
        &["workspace", "preset", "issues", "problems", "check"],
        Needs::Project,
    ),
    cmd(
        "layout.reports",
        "Layout: Reports",
        "View",
        &["workspace", "preset", "bom", "lists"],
        Needs::Project,
    ),
    cmd(
        "layout.reset",
        "Reset this workspace layout",
        "View",
        &[
            "layout",
            "panels",
            "default",
            "drawer",
            "outline",
            "window size",
        ],
        Needs::Nothing,
    ),
    cmd(
        "layout.reset_all",
        "Reset all workspace layouts",
        "View",
        &[
            "layout",
            "panels",
            "default",
            "every workspace",
            "every size",
        ],
        Needs::Nothing,
    ),
    cmd(
        "help.whats_new",
        "What's new",
        "View",
        &["new", "changes", "release", "news", "help"],
        Needs::Nothing,
    ),
    cmd(
        "view.reset_panels",
        "Reset panel layout",
        "View",
        &["panels", "windows", "layout", "reset", "position", "size"],
        Needs::Nothing,
    ),
    cmd(
        "help.walkthrough",
        "Fast keys walk-through",
        "View",
        &["tour", "tutorial", "walkthrough", "help"],
        Needs::Nothing,
    ),
    cmd(
        "help.next_lesson",
        "Next lesson",
        "View",
        &[
            "lesson", "tutorial", "training", "learn", "continue", "next",
        ],
        Needs::Nothing,
    ),
    verb(
        cmd(
            "help.learn",
            "Learn…",
            "View",
            &["lesson", "tutorial", "training", "learn", "teach", "course"],
            Needs::Nothing,
        ),
        &["learn", "lesson"],
    ),
    cmd(
        "learn.next",
        "Lesson: next step",
        "View",
        &["lesson", "tutorial", "training", "learn", "teach"],
        Needs::Nothing,
    ),
    cmd(
        "learn.back",
        "Lesson: previous step",
        "View",
        &["lesson", "tutorial", "training", "learn", "teach"],
        Needs::Nothing,
    ),
    cmd(
        "learn.restart",
        "Lesson: start over",
        "View",
        &["lesson", "tutorial", "training", "learn", "teach"],
        Needs::Nothing,
    ),
    cmd(
        "learn.close",
        "Lesson: close",
        "View",
        &["lesson", "tutorial", "training", "learn", "teach"],
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
    verb(
        cmd(
            "cabinet.routes",
            "Route all wires (Cabinet)",
            "View",
            &["route", "wire length", "duct fill", "cut length"],
            Needs::Project,
        ),
        &["route all", "route wires"],
    ),
    verb(
        cmd(
            "cabinet.rules",
            "Placement rules (Cabinet)",
            "View",
            &["place rules", "where devices go", "zones", "rail rule"],
            Needs::Project,
        ),
        &["placement rules"],
    ),
    verb(
        cmd(
            "cabinet.plate",
            "Plate setup (Cabinet)",
            "View",
            &["plate size", "mounting plate", "margin", "fill", "door"],
            Needs::Project,
        ),
        &["plate setup"],
    ),
    verb(
        cmd(
            "cabinet.cutlist",
            "Rail and duct cut list (Cabinet)",
            "View",
            &["cut list", "bars", "rail length", "duct length", "buy"],
            Needs::Project,
        ),
        &["cut list"],
    ),
    verb(
        cmd(
            "cabinet.rails",
            "Rails and ducts (Cabinet)",
            "View",
            &["rail", "duct", "trunk", "din rail", "add duct", "move duct"],
            Needs::Project,
        ),
        &["add rail", "add duct"],
    ),
    verb(
        cmd(
            "cabinet.surface",
            "Cabinet surfaces",
            "View",
            &["surface", "door", "wall", "roof", "floor", "side plate"],
            Needs::Project,
        ),
        &["pick surface", "show surfaces"],
    ),
    verb(
        cmd(
            "cabinet.door",
            "Inside size and door (Cabinet)",
            "View",
            &[
                "door",
                "door swing",
                "hinge",
                "inside size",
                "enclosure depth",
                "opens to",
                "setback",
            ],
            Needs::Project,
        ),
        &["door swing", "set door", "hinge side"],
    ),
    cmd(
        "view.background_place",
        "Move and scale the DXF background",
        "View",
        &["underlay", "dxf", "reference drawing", "offset", "position"],
        Needs::Page,
    ),
    // Check
    // Reports workspace (docs/design/workspaces.md section 3.4).
    cmd(
        "view.reports",
        "Reports",
        "View",
        &["tables", "lists", "workspace"],
        Needs::Project,
    ),
    verb(
        cmd(
            "report.toc",
            "Contents",
            "View",
            &["table of contents", "index", "page list"],
            Needs::Project,
        ),
        &["contents"],
    ),
    verb(
        cmd(
            "report.bom",
            "Bill of materials",
            "View",
            &["parts list", "bom", "quantities"],
            Needs::Project,
        ),
        &["bom", "bill of materials"],
    ),
    verb(
        cmd(
            "report.devices",
            "Device list",
            "View",
            &["devices", "tags", "locations"],
            Needs::Project,
        ),
        &["device list"],
    ),
    verb(
        cmd(
            "report.wires",
            "Wire list",
            "View",
            &["wires", "cores", "connections"],
            Needs::Project,
        ),
        &["wire list"],
    ),
    verb(
        cmd(
            "report.terminals",
            "Terminal diagram",
            "View",
            &["terminal list", "strips", "klemmen"],
            Needs::Project,
        ),
        &["terminal diagram"],
    ),
    verb(
        cmd(
            "report.cables",
            "Cable diagram",
            "View",
            &["cable list", "cable schedule", "cores"],
            Needs::Project,
        ),
        &["cable diagram"],
    ),
    verb(
        cmd(
            "report.cutlist",
            "Cut list (wire by the metre)",
            "View",
            &["cut length", "wire lengths", "sleeves"],
            Needs::Project,
        ),
        &["cut list"],
    ),
    verb(
        cmd(
            "report.labels",
            "Labels",
            "View",
            &["label printer", "wire labels", "terminal labels"],
            Needs::Project,
        ),
        &["labels"],
    ),
    verb(
        cmd(
            "report.wirecut",
            "Wire cut list (machine job)",
            "View",
            &["wire machine", "cut and strip", "production"],
            Needs::Project,
        ),
        &["wire cut report"],
    ),
    verb(
        cmd(
            "report.io",
            "I/O list",
            "View",
            &["plc", "inputs", "outputs", "addresses"],
            Needs::Project,
        ),
        &["io list", "i/o list"],
    ),
    verb(
        cmd(
            "report.cable_schedule",
            "Cable schedule",
            "View",
            &["cable list", "from to", "lengths"],
            Needs::Project,
        ),
        &["cable schedule"],
    ),
    verb(
        cmd(
            "report.plugs",
            "Plug list",
            "View",
            &["connectors", "plugs", "sockets", "mating"],
            Needs::Project,
        ),
        &["plug list"],
    ),
    verb(
        with_arguments(
            cmd(
                "report.add_page",
                "Add report as drawing page",
                "View",
                &[
                    "generated page",
                    "cut list page",
                    "labels page",
                    "io list page",
                    "strip diagram page",
                    "print report",
                ],
                Needs::Project,
            ),
            &[
                "toc",
                "bom",
                "devices",
                "wires",
                "cutlist",
                "wirecut",
                "terminals",
                "cables",
                "io",
                "cableschedule",
                "plugs",
                "labels",
            ],
        ),
        &["add report page", "add page"],
    ),
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
        "check.standards",
        "Standards check",
        "Check",
        &[
            "standards",
            "iec",
            "60204",
            "nfpa",
            "79",
            "ul",
            "508a",
            "sccr",
            "compliance",
        ],
        Needs::Project,
    ),
    cmd(
        "check.calculations",
        "Calculations",
        "Check",
        &[
            "calculation sheet",
            "cable rating",
            "voltage drop",
            "short circuit",
            "disconnection",
            "starter sizing",
            "sccr",
            "smallest section",
            "ampacity",
        ],
        Needs::Project,
    ),
    cmd(
        "check.electrical_inputs",
        "Electrical inputs",
        "Check",
        &[
            "supply",
            "transformer",
            "short circuit current",
            "ik",
            "system voltage",
            "voltage drop limit",
            "panel sccr",
            "machine data",
            "estop",
            "e-stop",
        ],
        Needs::Project,
    ),
    cmd(
        "check.safety",
        "Safety functions",
        "Check",
        &[
            "iso 13849",
            "performance level",
            "pl",
            "mttfd",
            "diagnostic coverage",
            "category",
            "b10d",
        ],
        Needs::Project,
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
            "export.step",
            "Export the cabinet as STEP",
            "Export",
            &[
                "3d",
                "mechanical",
                "solidworks",
                "inventor",
                "freecad",
                "stp",
                "step",
            ],
            Needs::Project,
        ),
        &["export step"],
    ),
    verb(
        cmd(
            "export.aml",
            "Export the plant as AutomationML",
            "Export",
            &["aml", "caex", "iec 62714", "exchange", "engineering"],
            Needs::Project,
        ),
        &["export aml", "export automationml"],
    ),
    verb(
        cmd(
            "export.eplan",
            "Export EPLAN project (PXF)",
            "Export",
            &[
                "eplan",
                "pxf",
                "epj",
                "hand back",
                "exchange",
                "engineering",
            ],
            Needs::Project,
        ),
        &["export eplan", "export pxf"],
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
            "export.pdf_options",
            "Export PDF...",
            "Export",
            &[
                "pdf options",
                "pdf dialog",
                "page range",
                "bookmarks",
                "links",
                "choose pages",
            ],
            Needs::Project,
        ),
        &["export pdf options", "pdf options"],
    ),
    verb(
        cmd(
            "export.pdf_page",
            "Export this page as PDF",
            "Export",
            &["current page", "one page", "print page"],
            Needs::Project,
        ),
        &["export page", "export this page"],
    ),
    verb(
        cmd(
            "export.report_pdf",
            "Export report (PDF)",
            "Export",
            &["report pdf", "bom pdf", "wire list pdf", "print report"],
            Needs::Project,
        ),
        &["export report"],
    ),
    cmd(
        "export.change_report",
        "Export change report",
        "Export",
        &["what changed", "revision report", "diff pdf"],
        Needs::Project,
    ),
    verb(
        cmd(
            "export.commissioning",
            "Export commissioning report",
            "Export",
            &["punch list", "sign-off", "handover", "inbedrijfstelling"],
            Needs::Project,
        ),
        &["export commissioning", "commissioning report"],
    ),
    verb(
        cmd(
            "export.standards_report",
            "Export standards compliance report",
            "Export",
            &[
                "iec 60204",
                "ul 508a",
                "nfpa 79",
                "compliance",
                "customer",
                "checklist",
            ],
            Needs::Project,
        ),
        &["export standards", "standards report"],
    ),
    verb(
        cmd(
            "export.standards_xlsx",
            "Export standards compliance report (Excel)",
            "Export",
            &[
                "compliance",
                "spreadsheet",
                "xlsx",
                "csv",
                "iec 60204",
                "ul 508a",
            ],
            Needs::Project,
        ),
        &["export standards excel"],
    ),
    verb(
        cmd(
            "export.plate",
            "Export plate drawing",
            "Export",
            &["mounting plate", "drill table", "drilling"],
            Needs::Project,
        ),
        &["export plate"],
    ),
    verb(
        cmd(
            "export.plate_dxf",
            "Export plate drawing as DXF",
            "Export",
            &["mounting plate", "drill table", "cnc", "cad"],
            Needs::Project,
        ),
        &["export plate dxf"],
    ),
    verb(
        cmd(
            "export.bom",
            "Export bill of materials",
            "Export",
            &["parts list"],
            Needs::Project,
        ),
        &["export bom"],
    ),
    cmd(
        "export.reports",
        "Export all reports to Excel",
        "Export",
        &["excel", "xlsx", "spreadsheet", "all reports"],
        Needs::Project,
    ),
    verb(
        cmd(
            "export.sheets",
            "Export for bulk edit (.xlsx)",
            "Export",
            &["excel", "xlsx", "spreadsheet", "round trip"],
            Needs::Project,
        ),
        &["export bulk edit", "export for editing"],
    ),
    verb(
        cmd(
            "export.wirecut",
            "Export wire cut list (CSV)",
            "Export",
            &[
                "wire cut",
                "wire machine",
                "ferrule",
                "labels",
                "production",
            ],
            Needs::Project,
        ),
        &["export wire cut", "wire cut"],
    ),
    verb(
        cmd(
            "export.wirecut_xlsx",
            "Export wire production (.xlsx)",
            "Export",
            &["wire cut", "stock", "ferrules", "duct fill"],
            Needs::Project,
        ),
        &["export wire production"],
    ),
    verb(
        cmd(
            "export.wiring_dxf",
            "Export wiring plan as DXF",
            "Export",
            &["routes", "ducts", "cabinet drawing"],
            Needs::Project,
        ),
        &["export wiring plan"],
    ),
    verb(
        cmd(
            "view.bench",
            "Bench output...",
            "Export",
            &[
                "bench",
                "ferrule",
                "ferrules",
                "labels",
                "label sheet",
                "cut list",
                "workshop",
                "print",
            ],
            Needs::Project,
        ),
        &["bench output", "open bench output"],
    ),
    verb(
        cmd(
            "report.designer",
            "Report designer...",
            "Export",
            &["report template", "template", "title block", "designer"],
            Needs::Project,
        ),
        &["report designer", "design a report", "edit report template"],
    ),
    verb(
        cmd(
            "export.bench_cut",
            "Export bench cut list (PDF)",
            "Export",
            &["wire cut", "bench", "workshop", "print", "tick", "strip"],
            Needs::Project,
        ),
        &["export bench cut list", "bench cut list"],
    ),
    verb(
        cmd(
            "export.ferrules",
            "Export ferrule table (PDF)",
            "Export",
            &["ferrule", "end sleeve", "crimp", "colour", "bench"],
            Needs::Project,
        ),
        &["export ferrules", "ferrule table"],
    ),
    verb(
        cmd(
            "export.labels",
            "Export label sheets (PDF)",
            "Export",
            &["labels", "stickers", "wire flags", "print", "tape"],
            Needs::Project,
        ),
        &["export labels", "print labels"],
    ),
    verb(
        cmd(
            "export.wirecut_page",
            "Add wire production page (PDF)",
            "Export",
            &["wire cut", "print", "pdf", "ferrule", "labels"],
            Needs::Project,
        ),
        &["add wire production page"],
    ),
    verb(
        cmd(
            "export.assembly",
            "Export assembly checklist (PDF)",
            "Export",
            &["workshop", "build sheet", "mounting", "drill", "cabinet"],
            Needs::Project,
        ),
        &["export assembly", "assembly checklist"],
    ),
    verb(
        cmd(
            "export.variant_pdf",
            "Export PDF of a variant",
            "Export",
            &["variant", "configuration", "pdf", "with conveyor"],
            Needs::Project,
        ),
        &["export variant"],
    ),
    verb(
        cmd(
            "export.spec",
            "Export the spec sheet (.xlsx)",
            "Export",
            &["spec sheet", "generated pages", "round trip", "excel"],
            Needs::Project,
        ),
        &["export spec"],
    ),
    verb(
        cmd(
            "export.sheets_csv",
            "Export for bulk edit (CSV files)",
            "Export",
            &["csv"],
            Needs::Project,
        ),
        &["export csv", "csv"],
    ),
    verb(
        cmd(
            "plc.import_tags",
            "Import PLC tags",
            "Export",
            &[
                "plc",
                "tia",
                "codesys",
                "studio 5000",
                "comments",
                "read back",
            ],
            Needs::Project,
        ),
        &["import plc", "import tags"],
    ),
    verb(
        cmd(
            "export.plc_csv",
            "Export PLC I/O list (CSV)",
            "Export",
            &["plc", "i/o", "tags", "excel"],
            Needs::Project,
        ),
        &["export plc", "export io"],
    ),
    verb(
        cmd(
            "export.plc_plcopen",
            "Export PLC tags (PLCopen XML)",
            "Export",
            &["plc", "codesys", "plcopen", "tags", "symbols"],
            Needs::Project,
        ),
        &["export plcopen", "export codesys"],
    ),
    verb(
        cmd(
            "export.plc_tia",
            "Export PLC tags for TIA Portal",
            "Export",
            &["plc", "siemens", "tia", "tag table", "s7"],
            Needs::Project,
        ),
        &["export tia", "export siemens"],
    ),
    verb(
        cmd(
            "export.plc_studio5000",
            "Export PLC tags for Studio 5000",
            "Export",
            &["plc", "rockwell", "allen-bradley", "logix", "tags"],
            Needs::Project,
        ),
        &["export studio 5000", "export rockwell"],
    ),
    verb(
        cmd(
            "export.plc_address_plan",
            "Export PLC address plan",
            "Export",
            &["plc", "hardware", "slots", "addresses"],
            Needs::Project,
        ),
        &["export address plan"],
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
