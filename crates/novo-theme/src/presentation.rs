//! How a command looks wherever it is shown as a picture: the icon and the
//! short label. One table, read by the rail, the menus, the command palette
//! and the centres, so none keeps its own list (docs/design/visual-redesign.md
//! sections 2 and 5.2).
//!
//! This is deliberately not a field of `novo_workflow::commands::CommandInfo`:
//! the workflow crate stays free of presentation, and the table sits here next
//! to the icons. Tests in `novo-workflow` hold the two together: every
//! command has a row, every row names a command, no short label is longer
//! than [`MAX_SHORT`] characters. A new command adds one line to [`TABLE`].
//!
//! **Add-ons** (the add-on manager, the controller panel) do not edit the
//! table: they call [`register`] at load, with an icon from
//! [`crate::drawn::register`] or a Phosphor glyph, and [`unregister_prefix`]
//! at unload. [`of`], [`glyph`] and [`short_label`] see both, so every surface
//! that reads this table shows an add-on's command with no further code.
//!
//! The short label is what a menu row, a rail tooltip title and a palette
//! result show; the long sentence stays on `CommandInfo::label` and becomes
//! the tooltip.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

use egui::{Align2, Color32, FontId, Painter, Rect, Response, Sense, Ui, Vec2};

use crate::drawn::{self, Icon};
use crate::icons;

/// The longest short label (label policy, design doc section 5.2).
pub const MAX_SHORT: usize = 28;

/// A picture for a command: one of the drawn icons, or a Phosphor glyph.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Glyph {
    /// An electrical or workspace icon the designer drew.
    Drawn(&'static Icon),
    /// A generic action (a Phosphor Regular character).
    Phosphor(&'static str),
}

impl Glyph {
    /// Paints the glyph in the largest square of `rect`, in `color`.
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        match self {
            Glyph::Drawn(icon) => drawn::paint(painter, rect, icon, color),
            Glyph::Phosphor(text) => {
                let size = rect.width().min(rect.height());
                painter.text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    text,
                    FontId::proportional(size),
                    color,
                );
            }
        }
    }

    /// Reserves a `size` point square, paints the glyph in it.
    pub fn show(self, ui: &mut Ui, size: f32, color: Color32) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, color);
        }
        response
    }

    /// True for the icons drawn for KoperCAD (as opposed to Phosphor).
    pub fn is_drawn(self) -> bool {
        matches!(self, Glyph::Drawn(_))
    }
}

/// One command's presentation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Presentation {
    /// The command id (`novo_workflow::commands::CommandInfo::id`).
    pub id: &'static str,
    pub glyph: Glyph,
    /// At most [`MAX_SHORT`] characters.
    pub short: &'static str,
}

const fn p(glyph: &'static str) -> Glyph {
    Glyph::Phosphor(glyph)
}

const fn d(icon: &'static Icon) -> Glyph {
    Glyph::Drawn(icon)
}

const fn row(id: &'static str, glyph: Glyph, short: &'static str) -> Presentation {
    Presentation { id, glyph, short }
}

/// The presentation of the command called `id`: its row in [`TABLE`], or the
/// one an add-on registered ([`register`]).
pub fn of(id: &str) -> Option<Presentation> {
    static INDEX: OnceLock<HashMap<&'static str, usize>> = OnceLock::new();
    let index = INDEX.get_or_init(|| TABLE.iter().enumerate().map(|(i, r)| (r.id, i)).collect());
    if let Some(&i) = index.get(id) {
        return Some(TABLE[i]);
    }
    let runtime = RUNTIME.read().unwrap_or_else(|e| e.into_inner());
    runtime.iter().find(|r| r.id == id).copied()
}

static RUNTIME: RwLock<Vec<Presentation>> = RwLock::new(Vec::new());

/// The picture of an add-on command that names no icon, or one nobody knows:
/// a puzzle piece (Phosphor), so an add-on is recognisable as one.
pub const ADDON_ICON: Glyph = Glyph::Phosphor(icons::PUZZLE_PIECE);

/// `addon.<addon-id>.<name>`: lower case letters, digits, `-` and `_`.
pub fn is_extension_id(id: &str) -> bool {
    let part = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    id.strip_prefix("addon.")
        .and_then(|rest| rest.split_once('.'))
        .is_some_and(|(addon, name)| part(addon) && part(name))
}

/// Why an add-on's presentation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterError {
    /// The id is not `addon.<addon-id>.<name>`.
    BadId,
    /// The short label is empty or longer than [`MAX_SHORT`].
    BadShort,
}

impl std::fmt::Display for RegisterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            RegisterError::BadId => "the command id must be addon.<addon-id>.<name>",
            RegisterError::BadShort => "the short label must be 1 to 28 characters",
        })
    }
}

impl std::error::Error for RegisterError {}

/// The extension point for add-ons: gives the command `id`
/// (`addon.<addon-id>.<name>`) an icon and a short label, so the rail, the menus and the
/// palette show it like any built-in command. `glyph` is a drawn icon (built-in
/// or registered with [`crate::drawn::register`]) or a Phosphor glyph.
/// Registering an id again replaces the earlier row, which is how an add-on
/// reloads. The same rules as the built-in table hold: a short label is at
/// most [`MAX_SHORT`] characters, and a built-in id is never an `addon.` id, so nothing can shadow one.
///
/// The strings are kept for the rest of the run (they are leaked so rows stay
/// `Copy`); [`unregister_prefix`] makes an unloaded add-on's rows unfindable.
pub fn register(id: &str, glyph: Glyph, short: &str) -> Result<Presentation, RegisterError> {
    if !is_extension_id(id) {
        return Err(RegisterError::BadId);
    }
    let short = short.trim();
    let n = short.chars().count();
    if n == 0 || n > MAX_SHORT || short.ends_with(['.', '…']) {
        return Err(RegisterError::BadShort);
    }
    let row = Presentation {
        id: Box::leak(id.to_owned().into_boxed_str()),
        glyph,
        short: Box::leak(short.to_owned().into_boxed_str()),
    };
    let mut runtime = RUNTIME.write().unwrap_or_else(|e| e.into_inner());
    runtime.retain(|r| r.id != id);
    runtime.push(row);
    Ok(row)
}

/// [`register`] for an add-on that names its icon: the drawn icon called
/// `icon` (built-in, or registered by the manager with
/// [`crate::drawn::register`]), or [`ADDON_ICON`] when the name is empty or
/// unknown. Add-ons ship no pixels in v1; they pick from the set.
pub fn register_extension(
    id: &str,
    icon: Option<&str>,
    short: &str,
) -> Result<Presentation, RegisterError> {
    let glyph = icon
        .and_then(Icon::by_name)
        .map_or(ADDON_ICON, Glyph::Drawn);
    register(id, glyph, short)
}

/// Forgets every registered row (the add-on set changed). Returns how many.
pub fn clear() -> usize {
    let mut runtime = RUNTIME.write().unwrap_or_else(|e| e.into_inner());
    let n = runtime.len();
    runtime.clear();
    n
}

/// Forgets every registered row whose id starts with `prefix` (`"addon.controller."`
/// when that add-on unloads). Returns how many were removed.
pub fn unregister_prefix(prefix: &str) -> usize {
    let mut runtime = RUNTIME.write().unwrap_or_else(|e| e.into_inner());
    let before = runtime.len();
    runtime.retain(|r| !r.id.starts_with(prefix));
    before - runtime.len()
}

/// The rows add-ons registered, in registration order.
pub fn registered() -> Vec<Presentation> {
    RUNTIME.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// The glyph of the command: its row, the add-on icon for an add-on command
/// with no row, or a plain dot for a built-in one that has no row yet.
pub fn glyph(id: &str) -> Glyph {
    of(id).map_or_else(
        || {
            if is_extension_id(id) {
                ADDON_ICON
            } else {
                Glyph::Phosphor(icons::DOT_OUTLINE)
            }
        },
        |r| r.glyph,
    )
}

/// The short label of the command, if it has a row.
pub fn short_label(id: &str) -> Option<&'static str> {
    of(id).map(|r| r.short)
}

/// Every command, in the order of the command registry.
pub const TABLE: &[Presentation] = &[
    row("file.new", p(icons::FILE_PLUS), "New project"),
    row("file.open", p(icons::FOLDER_OPEN), "Open project"),
    row(
        "file.open_read_only",
        p(icons::LOCK_SIMPLE),
        "Open read-only",
    ),
    row("file.recover", p(icons::FIRST_AID_KIT), "Recover project"),
    row("file.example", p(icons::LIGHTBULB), "Open example"),
    row("file.example_emc", d(&drawn::EMC), "Open EMC example"),
    row(
        "file.example_mcc",
        d(&drawn::MOTOR),
        "Open motor centre example",
    ),
    row(
        "file.example_crane",
        d(&drawn::BRIDGE),
        "Open crane cabinet example",
    ),
    row(
        "file.example_plc",
        d(&drawn::PLC),
        "Open PLC cabinet example",
    ),
    row("file.example_pump", p(icons::DROP), "Open pump example"),
    row("file.practice", p(icons::KEYBOARD), "Try the fast keys"),
    row("project.languages", p(icons::TRANSLATE), "Report languages"),
    row("file.save", p(icons::FLOPPY_DISK), "Save"),
    row("file.save_as", p(icons::FLOPPY_DISK_BACK), "Save as"),
    row("file.close", p(icons::X_SQUARE), "Close project"),
    row("page.new", p(icons::FILE_PLUS), "New page"),
    row("page.rename", p(icons::PENCIL_SIMPLE), "Rename page"),
    row("page.duplicate", p(icons::COPY), "Duplicate page"),
    row("page.delete", p(icons::TRASH), "Delete page"),
    row("page.move_up", p(icons::ARROW_UP), "Move page up"),
    row("page.move_down", p(icons::ARROW_DOWN), "Move page down"),
    row("project.rules", p(icons::RULER), "Rules"),
    row("project.standard", p(icons::SEAL_CHECK), "Drawing standard"),
    row("macro.browser", d(&drawn::MACRO), "Macros"),
    row("macro.import", d(&drawn::MACRO), "Import macros"),
    row("macro.export_pack", d(&drawn::MACRO), "Export macro pack"),
    row("app.preferences", p(icons::GEAR_SIX), "Preferences"),
    row("parts.browser", p(icons::PACKAGE), "Parts"),
    row(
        "parts.import_folder",
        p(icons::FOLDER_OPEN),
        "Import parts folder",
    ),
    row("parts.import_table", p(icons::TABLE), "Import parts table"),
    row(
        "parts.import_eplan",
        p(icons::DOWNLOAD_SIMPLE),
        "Import CAD parts export",
    ),
    row(
        "parts.import_step",
        p(icons::CUBE),
        "Import 3D model (STEP)",
    ),
    row(
        "project.options",
        p(icons::SLIDERS_HORIZONTAL),
        "Project options and variants",
    ),
    row("project.variant", p(icons::GIT_BRANCH), "Switch variant"),
    row(
        "project.option",
        p(icons::TOGGLE_LEFT),
        "Switch a machine option",
    ),
    row("project.tie", p(icons::LINK), "Tie selection to option"),
    row(
        "project.untie",
        p(icons::LINK_BREAK),
        "Always show selection",
    ),
    row(
        "project.generate",
        p(icons::MAGIC_WAND),
        "Generate pages from spec",
    ),
    row("revision.issue", p(icons::FLAG), "Issue revision"),
    row("revision.compare", p(icons::GIT_DIFF), "Compare revisions"),
    row("revision.lock_page", p(icons::LOCK_SIMPLE), "Lock page"),
    row(
        "revision.unlock_page",
        p(icons::LOCK_SIMPLE_OPEN),
        "Unlock page",
    ),
    row(
        "revision.restore_page",
        p(icons::CLOCK_COUNTER_CLOCKWISE),
        "Restore page",
    ),
    row("file.quit", p(icons::SIGN_OUT), "Quit"),
    row("help.about", p(icons::INFO), "About"),
    row(
        "file.import_dxf_background",
        p(icons::IMAGE),
        "Import DXF background",
    ),
    row("file.import_dwg", p(icons::IMAGE), "Import DWG background"),
    row(
        "file.eplan_export_script",
        p(icons::SCROLL),
        "Get EPLAN export script",
    ),
    row(
        "file.choose_dwg_converter",
        p(icons::WRENCH),
        "DWG converter",
    ),
    row(
        "file.import_aml",
        p(icons::DOWNLOAD_SIMPLE),
        "Import AutomationML",
    ),
    row(
        "file.import_controller",
        d(&drawn::PLC),
        "Controller from I/O table",
    ),
    row("file.import_crane", p(icons::CRANE), "Import crane project"),
    row(
        "file.import_eplan",
        p(icons::DOWNLOAD_SIMPLE),
        "Import EPLAN project",
    ),
    row(
        "file.remove_background",
        p(icons::IMAGE_BROKEN),
        "Remove background",
    ),
    row("import.spreadsheet", p(icons::TABLE), "Import spreadsheet"),
    row("tool.repeat", p(icons::REPEAT), "Repeat last tool"),
    row("tool.place", d(&drawn::PLACE), "Place symbol"),
    row("tool.connect", p(icons::LINE_SEGMENT), "Connect"),
    row("wire.mode", p(icons::ARROW_ELBOW_RIGHT_DOWN), "Wiring mode"),
    row("tool.cable", d(&drawn::CABLE), "Cable"),
    row("draw.text", p(icons::TEXT_T), "Add text"),
    row("cable.rename", d(&drawn::CABLE), "Rename cable"),
    row("cable.delete", d(&drawn::CABLE), "Delete cable"),
    row(
        "cable.retype",
        d(&drawn::CABLE_SECTION),
        "Change cable type",
    ),
    row(
        "cable.restore_proposals",
        d(&drawn::CABLE),
        "Restore cable proposals",
    ),
    row("wire.end_on_part", p(icons::TARGET), "End wire on part"),
    row(
        "wire.next_pin",
        p(icons::ARROW_FAT_LINE_RIGHT),
        "Next connection point",
    ),
    row("edit.rotate", p(icons::ARROW_CLOCKWISE), "Rotate"),
    row(
        "edit.reset_note",
        p(icons::ARROW_COUNTER_CLOCKWISE),
        "Reset note position",
    ),
    row("edit.mirror", p(icons::FLIP_HORIZONTAL), "Mirror"),
    row("plc.add_card", d(&drawn::PLC), "Add PLC card"),
    row("plc.overview", d(&drawn::PLC), "PLC overview"),
    row("plc.to_plc", d(&drawn::PLC), "Send signal to PLC"),
    row("proposal.accept", p(icons::CHECK), "Accept proposal"),
    row(
        "proposal.accept_all",
        p(icons::CHECKS),
        "Accept all proposals",
    ),
    row("proposal.next", p(icons::LIGHTBULB), "Next proposal"),
    row("draw.tidy", p(icons::SPARKLE), "Tidy routes"),
    row("edit.undo", p(icons::ARROW_COUNTER_CLOCKWISE), "Undo"),
    row("edit.redo", p(icons::ARROW_CLOCKWISE), "Redo"),
    row("edit.copy", p(icons::COPY), "Copy"),
    row("edit.cut", p(icons::SCISSORS), "Cut"),
    row("edit.paste", p(icons::CLIPBOARD), "Paste"),
    row(
        "edit.paste_keep_tags",
        p(icons::CLIPBOARD_TEXT),
        "Paste, keep tags",
    ),
    row("edit.paste_in_place", p(icons::CLIPBOARD), "Paste in place"),
    row("edit.duplicate", p(icons::COPY_SIMPLE), "Duplicate"),
    row("macro.save", d(&drawn::MACRO), "Save selection as macro"),
    row("macro.place", d(&drawn::MACRO), "Place macro"),
    row("macro.edit", d(&drawn::MACRO), "Edit macro parameters"),
    row(
        "edit.select_all",
        p(icons::SELECTION_ALL),
        "Select all on page",
    ),
    row("edit.delete", p(icons::TRASH), "Delete"),
    row(
        "edit.shared_properties",
        p(icons::SLIDERS),
        "Selection properties",
    ),
    row("edit.rename", p(icons::PENCIL_SIMPLE), "Rename"),
    row(
        "parts.auto_assign",
        p(icons::PACKAGE),
        "Assign parts to devices",
    ),
    row(
        "parts.describe",
        p(icons::PACKAGE),
        "Describe devices from parts",
    ),
    row(
        "parts.clear",
        p(icons::PACKAGE),
        "Remove parts from devices",
    ),
    row(
        "parts.remove_unused",
        p(icons::BROOM),
        "Remove unused parts",
    ),
    row("edit.renumber", p(icons::LIST_NUMBERS), "Renumber devices"),
    row("plc.renumber_io", d(&drawn::PLC), "Renumber I/O"),
    row("xref.link", d(&drawn::XREF), "Link cross reference"),
    row("net.select", d(&drawn::NET), "Select net"),
    row("net.trace", d(&drawn::NET), "Pages and devices on net"),
    row("edit.escape", p(icons::X), "Cancel"),
    row("terminal.connect", d(&drawn::TERMINAL), "Connect terminals"),
    row(
        "terminal.toggle_pe",
        d(&drawn::GROUND),
        "Toggle PE terminal",
    ),
    row(
        "terminal.disconnect",
        d(&drawn::TERMINAL),
        "Disconnect terminals",
    ),
    row("terminal.reset_pe", d(&drawn::GROUND), "Reset PE override"),
    row("terminal.strip_up", p(icons::ARROW_UP), "Strip number up"),
    row(
        "terminal.strip_down",
        p(icons::ARROW_DOWN),
        "Strip number down",
    ),
    row(
        "terminal.strip_lowest",
        p(icons::ARROW_LINE_DOWN),
        "Strip lowest free number",
    ),
    row(
        "terminal.add_spares",
        d(&drawn::TERMINAL_STRIP),
        "Add spare terminals",
    ),
    row(
        "terminal.rename",
        d(&drawn::TERMINAL_STRIP),
        "Rename terminals",
    ),
    row(
        "terminal.renumber",
        d(&drawn::TERMINAL_STRIP),
        "Renumber terminals",
    ),
    row(
        "terminal.join_duplicates",
        d(&drawn::BRIDGE),
        "Join duplicate terminals",
    ),
    row(
        "terminal.set_kind",
        d(&drawn::TERMINAL),
        "Set terminal kind",
    ),
    row(
        "terminal.set_level",
        d(&drawn::TERMINAL),
        "Set terminal level",
    ),
    row(
        "terminal.set_jumper",
        d(&drawn::BRIDGE),
        "Set jumper number",
    ),
    row("terminal.order_up", p(icons::ARROW_UP), "Move terminal up"),
    row(
        "terminal.order_down",
        p(icons::ARROW_DOWN),
        "Move terminal down",
    ),
    row(
        "terminal.style",
        d(&drawn::TERMINAL_STRIP),
        "Terminal strip style",
    ),
    row(
        "terminal.restore_declined",
        d(&drawn::TERMINAL_STRIP),
        "Restore terminal proposals",
    ),
    row(
        "interruption.renumber",
        d(&drawn::XREF),
        "Number interruption points",
    ),
    row("wires.autopilot", p(icons::ROBOT), "Autopilot"),
    row("wires.number", d(&drawn::WIRE_NUMBER), "Number wires"),
    row("wires.numbering", d(&drawn::WIRE_NUMBER), "Wire numbering"),
    row("wires.renumber", d(&drawn::WIRE_NUMBER), "Renumber wires"),
    row(
        "wires.check_numbers",
        d(&drawn::WIRE_NUMBER),
        "Check wire numbers",
    ),
    row(
        "wires.set_number",
        d(&drawn::WIRE_NUMBER),
        "Set wire number",
    ),
    row(
        "wires.unlock_number",
        d(&drawn::WIRE_NUMBER),
        "Unlock wire number",
    ),
    row("nav.partner", p(icons::ARROWS_LEFT_RIGHT), "Go to partner"),
    row(
        "nav.show_schematic",
        d(&drawn::SCHEMATIC),
        "Show in schematic",
    ),
    row("nav.show_cabinet", d(&drawn::CABINET), "Show in Cabinet"),
    row("nav.show_3d", p(icons::CUBE), "Show in 3D"),
    row("nav.all_references", d(&drawn::XREF), "Every occurrence"),
    row("nav.back", p(icons::ARROW_LEFT), "Back"),
    row("nav.forward", p(icons::ARROW_RIGHT), "Forward"),
    row("nav.previous_page", p(icons::CARET_UP), "Previous page"),
    row("nav.next_page", p(icons::CARET_DOWN), "Next page"),
    row(
        "view.command_palette",
        p(icons::MAGNIFYING_GLASS),
        "Command field",
    ),
    row(
        "view.command_field_here",
        p(icons::CURSOR_TEXT),
        "Command field at the pointer",
    ),
    row("view.fit", p(icons::CORNERS_OUT), "Zoom to fit"),
    row("view.fit_sheet", p(icons::FRAME_CORNERS), "Fit whole sheet"),
    row("view.schematic", d(&drawn::SCHEMATIC), "Schematic"),
    row("view.cabinet", d(&drawn::CABINET), "Cabinet"),
    row("view.3d", p(icons::CUBE), "3D view"),
    row("view.3d_door", p(icons::DOOR_OPEN), "Door in 3D"),
    row(
        "view.3d_shell",
        p(icons::CUBE_TRANSPARENT),
        "Enclosure in 3D",
    ),
    row("view.3d_wires", d(&drawn::NET), "Wires in 3D"),
    row(
        "view.3d_slotted_ducts",
        d(&drawn::DUCT),
        "Slotted ducts in 3D",
    ),
    row("view.3d_shadows", p(icons::SUN_HORIZON), "Shadows in 3D"),
    row(
        "view.3d_assembly",
        p(icons::LIST_NUMBERS),
        "Build order in 3D",
    ),
    row(
        "view.3d_assembly_next",
        p(icons::SKIP_FORWARD),
        "Build order: next step",
    ),
    row(
        "view.3d_assembly_previous",
        p(icons::SKIP_BACK),
        "Build order: previous",
    ),
    row("view.3d_wire_it", d(&drawn::BENCH), "Wire-it in 3D"),
    row(
        "view.3d_wire_it_next",
        p(icons::SKIP_FORWARD),
        "Wire-it: next wire",
    ),
    row(
        "view.3d_wire_it_previous",
        p(icons::SKIP_BACK),
        "Wire-it: previous wire",
    ),
    row(
        "view.3d_wire_it_done",
        p(icons::CHECK_CIRCLE),
        "Wire-it: wire done",
    ),
    row(
        "view.3d_finish",
        p(icons::PAINT_BRUSH),
        "Surface finish in 3D",
    ),
    row("view.3d_measure", p(icons::RULER), "Measure in 3D"),
    row("view.3d_align_left", p(icons::ALIGN_LEFT), "Align left"),
    row(
        "view.3d_align_hcentre",
        p(icons::ALIGN_CENTER_HORIZONTAL),
        "Centre across",
    ),
    row("view.3d_align_right", p(icons::ALIGN_RIGHT), "Align right"),
    row("view.3d_align_top", p(icons::ALIGN_TOP), "Align top"),
    row(
        "view.3d_align_vcentre",
        p(icons::ALIGN_CENTER_VERTICAL),
        "Centre up and down",
    ),
    row(
        "view.3d_align_bottom",
        p(icons::ALIGN_BOTTOM),
        "Align bottom",
    ),
    row(
        "view.3d_spread_h",
        p(icons::ARROWS_HORIZONTAL),
        "Spread across",
    ),
    row("view.3d_spread_v", p(icons::ARROWS_VERTICAL), "Spread down"),
    row("view.emc_nets", d(&drawn::EMC), "EMC colours"),
    row("view.emc", d(&drawn::EMC), "EMC view"),
    row("view.3d_wire_runs", d(&drawn::CABLE), "Wire runs in 3D"),
    row("view.3d_ssao", p(icons::SUN_DIM), "Ambient occlusion"),
    row(
        "view.3d_duplicate",
        p(icons::COPY_SIMPLE),
        "Duplicate in 3D",
    ),
    row("view.3d_position", p(icons::CROSSHAIR), "Position box"),
    row("view.3d_place", d(&drawn::PLACE), "Place device in 3D"),
    row("view.3d_dimensions", p(icons::RULER), "Dimensions in 3D"),
    row("view.3d_quality", p(icons::SPARKLE), "Restore 3D quality"),
    row("view.3d_stats", p(icons::CHART_BAR), "3D statistics"),
    row("view.3d_section", p(icons::SCISSORS), "Section plane"),
    row("view.3d_save_view", p(icons::CAMERA), "Save 3D view"),
    row("view.3d_hints", p(icons::QUESTION), "3D hints"),
    row(
        "view.3d_select_devices",
        p(icons::SELECTION_ALL),
        "Select all devices",
    ),
    row(
        "view.3d_select_rails",
        d(&drawn::DIN_RAIL),
        "Select all rails",
    ),
    row("view.3d_select_ducts", d(&drawn::DUCT), "Select all ducts"),
    row("view.3d_hide", p(icons::EYE_SLASH), "Hide selection"),
    row("view.3d_isolate", p(icons::EYE), "Isolate selection"),
    row("view.3d_show_all", p(icons::EYE), "Show all"),
    row("view.iso", p(icons::CUBE), "Isometric view"),
    row("view.front", p(icons::SQUARE), "Front view"),
    row("view.top", p(icons::SQUARE_HALF), "Top view"),
    row(
        "view.terminals_drawer",
        d(&drawn::TERMINAL_STRIP),
        "Terminals",
    ),
    row("view.cables_drawer", d(&drawn::CABLE), "Cables"),
    row("view.devices_drawer", p(icons::PACKAGE), "Devices"),
    row("view.focus", p(icons::ARROWS_IN), "Focus mode"),
    row("view.pin_outline", p(icons::PUSH_PIN), "Pin outline"),
    row("view.full_screen", p(icons::ARROWS_OUT), "Full screen"),
    row(
        "cabinet.place-all",
        p(icons::SQUARES_FOUR),
        "Place all devices",
    ),
    row(
        "cabinet.size-enclosure",
        d(&drawn::ENCLOSURE),
        "Size the enclosure",
    ),
    row("cabinet.enclosures", d(&drawn::ENCLOSURE), "Enclosures"),
    row("view.protection", d(&drawn::BREAKER), "Protection"),
    row("ai.assistant", p(icons::SPARKLE), "Ask Claude"),
    row(
        "simulate.motor-start",
        d(&drawn::MOTOR),
        "Simulate motor start",
    ),
    row("view.cabinet_table", p(icons::TABLE), "Cabinet as table"),
    row(
        "heat.toggle",
        p(icons::THERMOMETER_HOT),
        "Show or hide heat",
    ),
    row(
        "heat.simulate",
        p(icons::THERMOMETER_HOT),
        "Simulate cabinet heat",
    ),
    row("heat.cooling_advice", p(icons::SNOWFLAKE), "Cooling advice"),
    row(
        "heat.cooling_whatif",
        p(icons::SNOWFLAKE),
        "Cooling what-if wizard",
    ),
    row(
        "heat.cooler_layout",
        p(icons::SNOWFLAKE),
        "Suggest cooler layout",
    ),
    row("heat.airflow", p(icons::WIND), "Show or hide airflow"),
    row("heat.vents", p(icons::FAN), "Vents and fans"),
    row("heat.add_filter_fan", p(icons::FAN), "Add filter fan"),
    row("heat.add_grille", p(icons::LIST_DASHES), "Add grille"),
    row("heat.add_exhaust_fan", p(icons::FAN), "Add exhaust fan"),
    row(
        "heat.contours",
        p(icons::CHART_LINE),
        "Show or hide heat contours",
    ),
    row(
        "view.toggle_marks",
        p(icons::LIGHTBULB),
        "Show or hide marks",
    ),
    row(
        "view.toggle_references",
        p(icons::EYE),
        "Show or fade cross refs",
    ),
    row(
        "view.hide_references_page",
        p(icons::EYE_SLASH),
        "Hide cross refs on this page",
    ),
    row(
        "view.hide_references_device",
        p(icons::EYE_SLASH),
        "Hide cross refs of device",
    ),
    row("view.changes", p(icons::GIT_DIFF), "Show changes"),
    row(
        "view.commissioning",
        p(icons::CLIPBOARD_TEXT),
        "Commissioning",
    ),
    row("sim.live", d(&drawn::LIVE), "Simulate live potentials"),
    row("sim.why", p(icons::QUESTION), "Why is this lit or dead?"),
    row("sim.plc_program", d(&drawn::PLC), "Simulate PLC program"),
    row("sim.faults", d(&drawn::FAULT), "Simulate fault scenarios"),
    row("sim.add_fault", d(&drawn::FAULT), "Add fault to selection"),
    row(
        "view.override_marks",
        p(icons::DIAMOND),
        "Show override marks",
    ),
    row("view.left_out", p(icons::GHOST), "Ghost left-out parts"),
    row("help.keymap_file", p(icons::KEYBOARD), "Open keymap file"),
    row(
        "help.open_log_folder",
        p(icons::FOLDER_OPEN),
        "Open log folder",
    ),
    row(
        "help.copy_problem_report",
        p(icons::BUG),
        "Copy problem report",
    ),
    row("help.show_hints", p(icons::LIGHTBULB), "Show hints again"),
    row("layout.draw", d(&drawn::SCHEMATIC), "Layout: Draw"),
    row("layout.cabinet", d(&drawn::CABINET), "Layout: Cabinet"),
    row("layout.3d", p(icons::CUBE), "Layout: 3D and heat"),
    row("layout.review", p(icons::CLIPBOARD_TEXT), "Layout: Review"),
    row("layout.reports", d(&drawn::REPORT), "Layout: Reports"),
    row(
        "layout.reset",
        p(icons::ARROW_COUNTER_CLOCKWISE),
        "Reset this workspace layout",
    ),
    row(
        "layout.reset_all",
        p(icons::ARROW_COUNTER_CLOCKWISE),
        "Reset all workspace layouts",
    ),
    row("help.numbers", p(icons::CALCULATOR), "About the numbers"),
    row("view.icon_gallery", p(icons::IMAGES_SQUARE), "Icon gallery"),
    row("help.manual", p(icons::BOOK_OPEN), "User manual"),
    row("help.context", p(icons::QUESTION), "Help for this panel"),
    row("help.whats_new", p(icons::SPARKLE), "What's new"),
    row(
        "view.reset_panels",
        p(icons::ARROW_COUNTER_CLOCKWISE),
        "Reset panel layout",
    ),
    row("help.walkthrough", p(icons::KEYBOARD), "Fast keys card"),
    row("help.next_lesson", p(icons::GRADUATION_CAP), "Next lesson"),
    row("help.learn", p(icons::GRADUATION_CAP), "Learn"),
    row("learn.next", p(icons::CARET_RIGHT), "Lesson: next step"),
    row("learn.back", p(icons::CARET_LEFT), "Lesson: previous step"),
    row(
        "learn.restart",
        p(icons::ARROW_COUNTER_CLOCKWISE),
        "Lesson: start over",
    ),
    row("learn.close", p(icons::X), "Lesson: close"),
    row("view.shortcuts", p(icons::KEYBOARD), "Keyboard shortcuts"),
    row(
        "view.background",
        p(icons::IMAGE),
        "Show or hide background",
    ),
    row("cabinet.routes", d(&drawn::DUCT), "Route all wires"),
    row("cabinet.emc", d(&drawn::EMC), "EMC on the plate"),
    row("cabinet.rules", p(icons::RULER), "Placement rules"),
    row("cabinet.plate", d(&drawn::PLATE), "Plate setup"),
    row(
        "cabinet.cutlist",
        d(&drawn::DIN_RAIL),
        "Rail and duct cut list",
    ),
    row("cabinet.rails", d(&drawn::DIN_RAIL), "Rails and ducts"),
    row(
        "cabinet.surface",
        p(icons::PAINT_BUCKET),
        "Cabinet surfaces",
    ),
    row("cabinet.door", p(icons::DOOR), "Inside size and door"),
    row(
        "view.background_place",
        p(icons::IMAGE),
        "Move and scale background",
    ),
    row("view.reports", d(&drawn::REPORT), "Reports"),
    row("report.toc", p(icons::LIST_BULLETS), "Table of contents"),
    row("report.bom", p(icons::PACKAGE), "Bill of materials"),
    row("report.devices", p(icons::LIST_DASHES), "Device list"),
    row("report.wires", d(&drawn::WIRE_NUMBER), "Wire list"),
    row(
        "report.terminals",
        d(&drawn::TERMINAL_STRIP),
        "Terminal diagram",
    ),
    row("report.cables", d(&drawn::CABLE), "Cable diagram"),
    row("report.cutlist", d(&drawn::DIN_RAIL), "Cut list"),
    row("report.labels", d(&drawn::FERRULE), "Labels"),
    row("report.wirecut", d(&drawn::FERRULE), "Wire production"),
    row("report.io", d(&drawn::PLC), "I/O list"),
    row(
        "report.cable_schedule",
        d(&drawn::CABLE_SECTION),
        "Cable schedule",
    ),
    row("report.plugs", p(icons::PLUGS), "Plug connection list"),
    row(
        "report.singleline",
        d(&drawn::SCHEMATIC),
        "Single-line diagram",
    ),
    row(
        "report.add_page",
        p(icons::FILE_PLUS),
        "Add as drawing page",
    ),
    row("check.problems", p(icons::WARNING_DIAMOND), "Problems"),
    row("notes.panel", p(icons::NOTE), "Review notes"),
    row("notes.add", p(icons::NOTE_PENCIL), "Add review note"),
    row("notes.next", p(icons::ARROW_FAT_RIGHT), "Next open note"),
    row("check.emc", d(&drawn::EMC), "EMC check"),
    row("check.standards", p(icons::SEAL_CHECK), "Standards check"),
    row("check.calculations", p(icons::CALCULATOR), "Calculations"),
    row(
        "check.electrical_inputs",
        d(&drawn::PSU),
        "Electrical inputs",
    ),
    row("check.safety", d(&drawn::ESTOP), "Safety functions"),
    row("check.next_problem", p(icons::CARET_DOWN), "Next problem"),
    row(
        "check.previous_problem",
        p(icons::CARET_UP),
        "Previous problem",
    ),
    row(
        "export.dxf_page",
        p(icons::FILE_DASHED),
        "Export this page as DXF",
    ),
    row("export.dxf_all", p(icons::FILES), "Export all pages as DXF"),
    row("export.step", p(icons::CUBE), "Export cabinet as STEP"),
    row("export.aml", p(icons::EXPORT), "Export AutomationML"),
    row("export.eplan", p(icons::EXPORT), "Export EPLAN project"),
    row("export.pdf", p(icons::FILE_PDF), "Export PDF"),
    row("export.print_preview", p(icons::PRINTER), "Print preview"),
    row(
        "export.pdf_options",
        p(icons::FILE_PDF),
        "Export PDF options",
    ),
    row(
        "export.pdf_page",
        p(icons::FILE_PDF),
        "Export this page as PDF",
    ),
    row(
        "export.report_pdf",
        p(icons::FILE_PDF),
        "Export report (PDF)",
    ),
    row(
        "export.change_report",
        p(icons::GIT_DIFF),
        "Export change report",
    ),
    row(
        "export.commissioning",
        p(icons::CLIPBOARD_TEXT),
        "Export commissioning",
    ),
    row(
        "export.punch_list",
        p(icons::LIST_CHECKS),
        "Export punch list",
    ),
    row(
        "export.standards_report",
        p(icons::SEAL_CHECK),
        "Export standards report",
    ),
    row(
        "export.standards_xlsx",
        p(icons::FILE_XLS),
        "Export standards (Excel)",
    ),
    row("export.plate", d(&drawn::PLATE), "Export plate drawing"),
    row(
        "export.plate_dxf",
        d(&drawn::PLATE),
        "Export plate drawing as DXF",
    ),
    row("export.bom", p(icons::PACKAGE), "Export bill of materials"),
    row(
        "export.reports",
        p(icons::FILE_XLS),
        "Export reports to Excel",
    ),
    row("export.sheets", p(icons::FILE_XLS), "Export for bulk edit"),
    row(
        "export.wirecut",
        d(&drawn::FERRULE),
        "Export wire cut list (CSV)",
    ),
    row(
        "export.wirecut_xlsx",
        p(icons::FILE_XLS),
        "Export wire production",
    ),
    row(
        "export.wiring_dxf",
        d(&drawn::NET),
        "Export wiring plan as DXF",
    ),
    row("view.bench", d(&drawn::BENCH), "Bench output"),
    row("view.build", p(icons::HAMMER), "Build mode"),
    row("report.designer", p(icons::PEN_NIB), "Report designer"),
    row(
        "export.bench_cut",
        d(&drawn::BENCH),
        "Export bench cut list (PDF)",
    ),
    row(
        "export.ferrules",
        d(&drawn::FERRULE),
        "Export ferrule table (PDF)",
    ),
    row("export.labels", p(icons::TAG), "Export label sheets (PDF)"),
    row(
        "export.wirecut_page",
        d(&drawn::FERRULE),
        "Add wire production page",
    ),
    row(
        "export.assembly",
        p(icons::LIST_CHECKS),
        "Export assembly checklist",
    ),
    row(
        "export.handover",
        p(icons::PACKAGE),
        "Export handover package",
    ),
    row(
        "export.workpack",
        p(icons::CARDS),
        "Export work cards (PDF)",
    ),
    row("export.wireit", d(&drawn::BENCH), "Export Wire-it list"),
    row("export.workshop", p(icons::TOOLBOX), "Export workshop pack"),
    row("export.scale_check", p(icons::RULER), "Export scale check"),
    row(
        "export.bench_packet",
        d(&drawn::BENCH),
        "Export bench packet (PDF)",
    ),
    row(
        "export.kitting",
        p(icons::PACKAGE),
        "Export kitting list (PDF)",
    ),
    row(
        "export.safety_sheets",
        d(&drawn::ESTOP),
        "Export safety sheets",
    ),
    row(
        "export.variant_pdf",
        p(icons::FILE_PDF),
        "Export PDF of a variant",
    ),
    row("export.spec", p(icons::FILE_XLS), "Export spec sheet"),
    row(
        "export.sheets_csv",
        p(icons::FILE_CSV),
        "Export bulk edit (CSV)",
    ),
    row("plc.import_list", d(&drawn::PLC), "Import I/O list"),
    row("plc.import_tags", d(&drawn::PLC), "Import PLC tags"),
    row(
        "export.plc_csv",
        d(&drawn::PLC),
        "Export PLC I/O list (CSV)",
    ),
    row("export.plc_plcopen", d(&drawn::PLC), "Export PLCopen XML"),
    row("export.plc_tia", d(&drawn::PLC), "Export for TIA Portal"),
    row(
        "export.plc_studio5000",
        d(&drawn::PLC),
        "Export for Studio 5000",
    ),
    row(
        "export.plc_address_plan",
        d(&drawn::PLC),
        "Export PLC address plan",
    ),
    row("addons.manage", p(icons::PUZZLE_PIECE), "Add-ons"),
    row(
        "view.3d_door_layer",
        p(icons::DOOR_OPEN),
        "Door layer in 3D",
    ),
    row(
        "view.3d_device_shapes",
        p(icons::SHAPES),
        "Device shapes in 3D",
    ),
    row("view.3d_surfaces", p(icons::SQUARES_FOUR), "Surfaces in 3D"),
    row("view.3d_duct_fill", d(&drawn::DUCT), "Duct fill in 3D"),
    row(
        "view.3d_pins",
        p(icons::DOT_OUTLINE),
        "Connection points in 3D",
    ),
    row(
        "view.3d_missing_data",
        p(icons::WARNING),
        "Missing part data in 3D",
    ),
    row(
        "view.3d_trackpad_pans",
        p(icons::HAND_SWIPE_RIGHT),
        "Trackpad pans in 3D",
    ),
    row(
        "view.3d_move",
        p(icons::ARROWS_OUT_CARDINAL),
        "Move mode in 3D",
    ),
    row("sim.hoist", d(&drawn::MOTOR), "Simulate hoist run"),
    row("sim.drive_run", d(&drawn::DRIVE), "Simulate drive run"),
    row(
        "sim.controller",
        p(icons::GAME_CONTROLLER),
        "Controller panel",
    ),
    row(
        "sim.motor_curves",
        d(&drawn::DRIVE),
        "Motor at variable frequency",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_are_unique_and_lookup_finds_each() {
        let mut seen = HashSet::new();
        for r in TABLE {
            assert!(seen.insert(r.id), "{} appears twice", r.id);
            assert_eq!(of(r.id), Some(*r));
        }
        assert_eq!(of("no.such.command"), None);
    }

    #[test]
    fn short_labels_are_short_and_plain() {
        for r in TABLE {
            let n = r.short.chars().count();
            assert!(
                (1..=MAX_SHORT).contains(&n),
                "{}: {:?} is {n} long",
                r.id,
                r.short
            );
            assert_eq!(r.short, r.short.trim(), "{}", r.id);
            assert!(
                !r.short.ends_with(['.', '…']),
                "{}: no full stop or ellipsis in {:?}",
                r.id,
                r.short
            );
        }
    }

    #[test]
    fn phosphor_glyphs_are_one_private_use_character() {
        for r in TABLE {
            if let Glyph::Phosphor(text) = r.glyph {
                assert_eq!(text.chars().count(), 1, "{}", r.id);
            }
        }
    }

    #[test]
    fn the_drawn_icons_are_used_for_the_electrical_commands() {
        let drawn_rows = TABLE.iter().filter(|r| r.glyph.is_drawn()).count();
        assert!(drawn_rows >= 60, "{drawn_rows} rows use a drawn icon");
        // 29 of the 40 icons are the picture of at least one command.
        let used: HashSet<_> = TABLE
            .iter()
            .filter_map(|r| match r.glyph {
                Glyph::Drawn(i) => Some(i.name),
                Glyph::Phosphor(_) => None,
            })
            .collect();
        let unused: Vec<_> = drawn::ALL
            .iter()
            .map(|i| i.name)
            .filter(|n| !used.contains(n))
            .collect();
        // The rest are parts, not commands: the place palette's categories (V5).
        assert_eq!(
            unused,
            [
                "coil",
                "contact-no",
                "contact-nc",
                "contactor",
                "fuse",
                "overload",
                "pushbutton",
                "lamp",
                "transformer",
                "sensor",
                "potential"
            ]
        );
    }

    #[test]
    fn a_missing_row_falls_back_to_a_dot() {
        assert_eq!(
            glyph("no.such.command"),
            Glyph::Phosphor(icons::DOT_OUTLINE)
        );
        assert_eq!(glyph("file.save"), Glyph::Phosphor(icons::FLOPPY_DISK));
    }

    #[test]
    fn an_add_on_registers_a_row_and_every_lookup_sees_it() {
        let icon = crate::drawn::register(crate::drawn::IconSpec {
            name: "pt-demo.pendant".into(),
            title: "Pendant".into(),
            shapes: vec![crate::drawn::ShapeSpec::Rect {
                x: 6.0,
                y: 3.0,
                w: 12.0,
                h: 18.0,
                r: 2.0,
            }],
        })
        .unwrap();
        let row = register("addon.pt-demo.panel", Glyph::Drawn(icon), "Pendant panel").unwrap();
        assert_eq!(of("addon.pt-demo.panel"), Some(row));
        assert_eq!(short_label("addon.pt-demo.panel"), Some("Pendant panel"));
        assert_eq!(glyph("addon.pt-demo.panel"), Glyph::Drawn(icon));
        assert!(registered().iter().any(|r| r.id == "addon.pt-demo.panel"));
        // Registering again replaces (a reload).
        register(
            "addon.pt-demo.panel",
            Glyph::Phosphor(icons::GAME_CONTROLLER),
            "Pendant",
        )
        .unwrap();
        assert_eq!(short_label("addon.pt-demo.panel"), Some("Pendant"));
        assert_eq!(
            registered()
                .iter()
                .filter(|r| r.id == "addon.pt-demo.panel")
                .count(),
            1
        );
        // Unloading makes it unfindable and falls back to the dot.
        assert_eq!(unregister_prefix("addon.pt-demo."), 1);
        assert_eq!(of("addon.pt-demo.panel"), None);
        // An add-on command without a row gets the add-on icon, a built-in
        // one the dot.
        assert_eq!(glyph("addon.pt-demo.panel"), ADDON_ICON);
        assert_eq!(glyph("file.nothing"), Glyph::Phosphor(icons::DOT_OUTLINE));
        crate::drawn::unregister_prefix("pt-demo.");
    }

    #[test]
    fn an_add_on_cannot_break_the_rules_of_the_table() {
        let g = Glyph::Phosphor(icons::DOT_OUTLINE);
        assert_eq!(
            register("controller.panel", g, "X"),
            Err(RegisterError::BadId)
        );
        assert_eq!(register("a b.c", g, "X"), Err(RegisterError::BadId));
        // A built-in id is not an extension id, so it can never be shadowed.
        assert_eq!(register("file.save", g, "Mine"), Err(RegisterError::BadId));
        assert_eq!(
            register("addon.pt-rules.x", g, ""),
            Err(RegisterError::BadShort)
        );
        assert_eq!(
            register("addon.pt-rules.x", g, &"x".repeat(MAX_SHORT + 1)),
            Err(RegisterError::BadShort)
        );
        assert_eq!(
            register("addon.pt-rules.x", g, "Open…"),
            Err(RegisterError::BadShort)
        );
        assert!(register("addon.pt-rules.x", g, &"x".repeat(MAX_SHORT)).is_ok());
        unregister_prefix("addon.pt-rules.");
        // The built-in command is untouched.
        assert_eq!(short_label("file.save"), Some("Save"));
    }

    #[test]
    fn extension_rows_name_their_icon_and_fall_back_to_the_add_on_icon() {
        let known = register_extension("addon.pt-ext.a", Some("contactor"), "Known").unwrap();
        assert_eq!(known.glyph, Glyph::Drawn(&drawn::CONTACTOR));
        let unknown =
            register_extension("addon.pt-ext.b", Some("no-such-icon"), "Unknown").unwrap();
        assert_eq!(unknown.glyph, ADDON_ICON);
        let none = register_extension("addon.pt-ext.c", None, "None").unwrap();
        assert_eq!(none.glyph, ADDON_ICON);
        assert!(is_extension_id("addon.controller.panel"));
        assert!(!is_extension_id("controller.panel"));
        assert_eq!(unregister_prefix("addon.pt-ext."), 3);
    }
}
