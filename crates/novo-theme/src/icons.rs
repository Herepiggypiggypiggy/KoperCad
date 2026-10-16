//! Interface icons (Phosphor, MIT licence), drawn as font glyphs.
//!
//! Use the constants as text: `ui.label(format!("{} Save", icons::action::SAVE))`.
//! The full set is re-exported; the names below are the ones KoperCAD
//! uses, so every screen picks the same icon for the same action.

pub use egui_phosphor::regular::*;

/// Icon for each command, so the command field, the mark menu and the dock
/// agree.
pub mod action {
    use egui_phosphor::regular as p;

    pub const NEW_PROJECT: &str = p::FILE_PLUS;
    pub const OPEN_PROJECT: &str = p::FOLDER_OPEN;
    pub const SAVE: &str = p::FLOPPY_DISK;
    pub const EXPORT_PDF: &str = p::FILE_PDF;
    pub const PRINT: &str = p::PRINTER;
    pub const UNDO: &str = p::ARROW_COUNTER_CLOCKWISE;
    pub const REDO: &str = p::ARROW_CLOCKWISE;
    pub const CUT: &str = p::SCISSORS;
    pub const COPY: &str = p::COPY;
    pub const PASTE: &str = p::CLIPBOARD;
    pub const DELETE: &str = p::TRASH;
    pub const FIND: &str = p::MAGNIFYING_GLASS;
    pub const COMMAND: &str = p::COMMAND;
    pub const ZOOM_IN: &str = p::MAGNIFYING_GLASS_PLUS;
    pub const ZOOM_OUT: &str = p::MAGNIFYING_GLASS_MINUS;
    pub const ZOOM_FIT: &str = p::CORNERS_OUT;
    pub const GRID: &str = p::GRID_FOUR;
    pub const SNAP: &str = p::MAGNET;
    pub const ACCEPT: &str = p::CHECK;
    pub const RESET: &str = p::ARROW_COUNTER_CLOCKWISE;
    pub const PIN: &str = p::PUSH_PIN;
    pub const CROSS_REFERENCE: &str = p::ARROWS_LEFT_RIGHT;
    pub const PAGE_ADD: &str = p::FILE_PLUS;
    pub const CHECK_DESIGN: &str = p::SEAL_CHECK;
    pub const SIMULATE: &str = p::PLAY;
    pub const SCRIPT: &str = p::TERMINAL_WINDOW;
    pub const SETTINGS: &str = p::GEAR_SIX;
    pub const HELP: &str = p::QUESTION;
}

/// The four tools in the dock. Terminals, cables and potentials are derived
/// from connections, so they have no tool.
pub mod tool {
    use egui_phosphor::regular as p;

    pub const SELECT: &str = p::CURSOR;
    pub const PLACE: &str = p::CIRCUITRY;
    pub const CONNECT: &str = p::LINE_SEGMENT;
    pub const TEXT: &str = p::TEXT_T;
    /// Wiring mode: right-angle corners.
    pub const WIRE_CORNERS: &str = p::ARROW_ELBOW_RIGHT_DOWN;
    /// Wiring mode: straight lines at any angle.
    pub const WIRE_LINES: &str = p::ARROW_UP_RIGHT;
}

/// The canvases and workspaces.
pub mod canvas {
    use egui_phosphor::regular as p;

    pub const SCHEMATIC: &str = p::LINE_SEGMENTS;
    pub const CABINET: &str = p::SQUARES_FOUR;
    pub const REPORTS: &str = p::FILE_TEXT;
    pub const COMMISSIONING: &str = p::CLIPBOARD_TEXT;
    pub const VIEW_3D: &str = p::CUBE;
}

/// The outline and its modes, the inspector and the drawer tabs.
pub mod panel {
    use egui_phosphor::regular as p;

    pub const OUTLINE: &str = p::SIDEBAR_SIMPLE;
    pub const PAGES: &str = p::FILES;
    pub const ISSUES: &str = p::WARNING_DIAMOND;
    pub const CHANGES: &str = p::GIT_DIFF;
    pub const INSPECTOR: &str = p::INFO;
    pub const TERMINALS: &str = p::DOTS_SIX_VERTICAL;
    pub const CABLES: &str = p::PLUGS;
    pub const DEVICES: &str = p::PACKAGE;
}

/// Marks on the drawing that are not problems.
pub mod mark {
    use egui_phosphor::regular as p;

    pub const PROPOSAL: &str = p::LIGHTBULB;
    pub const OVERRIDE: &str = p::DIAMOND;
}

/// Icon for each message level.
pub mod status {
    use egui_phosphor::regular as p;

    pub const INFO: &str = p::INFO;
    pub const WARNING: &str = p::WARNING;
    pub const ERROR: &str = p::WARNING_OCTAGON;
    pub const OK: &str = p::CHECK_CIRCLE;
}
