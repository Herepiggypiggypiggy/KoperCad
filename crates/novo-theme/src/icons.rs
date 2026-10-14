//! Interface icons (Phosphor, MIT licence), drawn as font glyphs.
//!
//! Use the constants as text: `ui.button(format!("{} Save", icons::FLOPPY_DISK))`.
//! The full set is re-exported; the names below are the ones NovoElectric
//! uses, so every screen picks the same icon for the same action.

pub use egui_phosphor::regular::*;

/// Icon for each common action, so menus, toolbars and the command palette
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
    pub const COMMAND_PALETTE: &str = p::COMMAND;
    pub const ZOOM_IN: &str = p::MAGNIFYING_GLASS_PLUS;
    pub const ZOOM_OUT: &str = p::MAGNIFYING_GLASS_MINUS;
    pub const ZOOM_FIT: &str = p::CORNERS_OUT;
    pub const GRID: &str = p::GRID_FOUR;
    pub const SNAP: &str = p::MAGNET;
    pub const SELECT: &str = p::CURSOR;
    pub const PAN: &str = p::HAND;
    pub const WIRE: &str = p::LINE_SEGMENT;
    pub const SYMBOL: &str = p::CIRCUITRY;
    pub const TERMINAL: &str = p::DOTS_SIX_VERTICAL;
    pub const CABLE: &str = p::PLUGS;
    pub const TEXT: &str = p::TEXT_T;
    pub const CROSS_REFERENCE: &str = p::ARROWS_LEFT_RIGHT;
    pub const PAGE: &str = p::FILE;
    pub const PAGE_ADD: &str = p::FILE_PLUS;
    pub const PARTS: &str = p::PACKAGE;
    pub const REPORTS: &str = p::TABLE;
    pub const CHECK_DESIGN: &str = p::SEAL_CHECK;
    pub const SIMULATE: &str = p::PLAY;
    pub const SCRIPT: &str = p::TERMINAL_WINDOW;
    pub const SETTINGS: &str = p::GEAR_SIX;
    pub const HELP: &str = p::QUESTION;
}

/// Icon for each panel.
pub mod panel {
    use egui_phosphor::regular as p;

    pub const PROJECT: &str = p::TREE_STRUCTURE;
    pub const PROPERTIES: &str = p::SLIDERS_HORIZONTAL;
    pub const MESSAGES: &str = p::CHAT_TEXT;
    pub const SYMBOLS: &str = p::SHAPES;
}

/// Icon for each message level.
pub mod status {
    use egui_phosphor::regular as p;

    pub const INFO: &str = p::INFO;
    pub const WARNING: &str = p::WARNING;
    pub const ERROR: &str = p::WARNING_OCTAGON;
    pub const OK: &str = p::CHECK_CIRCLE;
}
