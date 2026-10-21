//! The Gate 1 engines hosted in the Workbench: the command field and keys
//! (novo-workflow), cross references (novo-xref), the Terminals drawer and
//! terminal proposals (novo-terminals), the issue pill (novo-checks) and
//! PDF export (novo-pdf).
//!
//! Everything here reads the model through queries and changes it through
//! the backend's document, one undo step per gesture, and tells the canvas
//! and storage about every change ([`NovoApp::changed_outside_canvas`]).

use std::path::PathBuf;

use eframe::egui;
use novo_canvas::SelectOp;
use novo_canvas::model::item_id;
use novo_checks::Problem;
use novo_core::{AnyId, ChangeSet, DeviceId, Document, PageId, SymbolId};
use novo_editor::EditorKey;
use novo_editor::tool::WiringMode;
use novo_plc::link::LinkFormat;
use novo_symbols::Library;
use novo_workflow::commands::Context;
use novo_workflow::keymap::{FAVOURITE_COMMANDS, Focus, Keymap};
use novo_workflow::palette::{LessonEntry, MacroEntry, Mode, Sources, Target};
use novo_xref::{LiveXref, Navigator, Place, Reference};

use novo_view3d::camera::ViewPreset;
use novo_view3d::scene::ItemKind;

use super::standards_report::StandardsFormat;
use super::view3d::Pending;
use super::workspace::Workspace;
use super::{Level, NovoApp};
use crate::actions::Action;
use crate::command_field::CommandField;
use crate::keys;

/// Drawer height as a share of the window (docs/DESIGN.md §3: at most 35%).
pub(super) const DRAWER_SHARE: f32 = 0.3;

/// The drawer's tabs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Drawer {
    Terminals,
    Cables,
    Devices,
}

/// State of the hosted engines, owned by [`NovoApp`].
pub(crate) struct Workbench {
    pub(super) field: CommandField,
    pub(super) keymap: Keymap,
    pub(super) xref: LiveXref,
    /// The symbol library as canvas art, for the inspector's symbol
    /// previews; built once, when first needed.
    pub(super) preview_art: Option<novo_editor::art::LibraryArt>,
    /// The generated pages' reports, live.
    pub(super) reports: super::reports::Reports,
    /// The place wheel, when open.
    pub(super) wheel: Option<super::wheel::Wheel>,
    /// A key that opens the wheel went down at this time (egui seconds),
    /// with the keys then held: a tap runs Place, a hold opens the wheel.
    pub(super) place_hold: Option<(f64, Vec<egui::Key>)>,
    /// The keys of a hold that opened the wheel, and since when all were up.
    pub(super) place_latch: Option<(Vec<egui::Key>, Option<f64>)>,
    /// Commands the canvas asked for, run next frame by id.
    pub(super) queued: Vec<&'static str>,
    /// The `renumber io` preview waiting for Enter or Esc, per station.
    pub(super) renumber: Option<Vec<novo_plc::commands::RenumberPlan>>,
    /// The PLC tag import preview (file name and plan), waiting for Enter or Esc.
    pub(super) plc_import: Option<(String, novo_plc::link::ImportPlan)>,
    /// The Preferences window is open.
    pub(super) preferences: bool,
    /// The shortcut sheet (`?`).
    pub(super) shortcuts: super::shortcuts_sheet::ShortcutSheet,
    /// The macro browser window.
    pub(super) macros: super::macro_browser::MacroBrowser,
    /// The Parts window (browser and one-click import), while open.
    pub(super) parts: Option<super::parts::PartsWindow>,
    /// The catalog the part pickers search (inspector, Cables drawer).
    pub(super) part_catalog: super::part_picker::PickerCatalog,
    /// The cable controls of the Cables drawer.
    pub(super) cable_edit: super::cable_controls::CableEdit,
    /// The Save as macro window, while it is open.
    pub(super) macro_save: Option<super::macro_actions::SaveDraft>,
    /// The Place macro window.
    pub(super) macro_place: super::macro_actions::PlaceWindow,
    /// The Edit macro parameters window.
    pub(super) macro_editor: super::macro_editor::MacroEditor,
    /// The Project settings window (drawing standard) is open.
    pub(super) project_settings: bool,
    /// The Issue revision window, while it is open.
    pub(super) issue: Option<super::revisions::IssueDraft>,
    pub(super) restore_ask: Option<super::revisions::RestoreAsk>,
    /// The Compare revisions window, while it is open.
    pub(super) compare: Option<super::compare::CompareWindow>,
    /// The Fault scenarios window, while it is open.
    pub(super) faults: Option<super::faults::FaultsWindow>,
    /// The Add fault window (`sim.add_fault`), while it is open.
    pub(super) add_fault: Option<super::faults::AddFault>,
    /// The Calculations window, while it is open.
    pub(super) calculations: Option<super::calculations::CalcWindow>,
    /// The Electrical inputs window, while it is open.
    pub(super) electrical: Option<super::electrical::ElectricalWindow>,
    /// The Safety functions window, while it is open.
    pub(super) safety: Option<super::safety::SafetyWindow>,
    pub(super) languages: Option<super::languages::LanguagesWindow>,
    /// The Renumber devices window, while it is open.
    pub(super) device_renumber: Option<super::device_renumber::DeviceRenumber>,
    /// The shared property sheet of the selection (window and inspector).
    pub(super) selection_sheet: super::selection_sheet::SelectionSheet,
    /// The Project rules window is open.
    pub(super) project_rules: bool,
    /// The Wire numbering window, while it is open.
    pub(super) numbering: Option<super::numbering::NumberingWindow>,
    /// Text of the open "Set wire number" prompt.
    pub(super) number_prompt: Option<String>,
    /// The net trace window (`net.trace`), while it is open.
    pub(super) net_trace: Option<super::net_trace::NetTrace>,
    /// The new pages, group and panel form, while open.
    pub(super) page_form: Option<super::pages::PageForm>,
    /// The spreadsheet import preview, while it is open.
    pub(super) sheet_import: Option<super::sheet_import::SheetImport>,
    /// Which sheet of a generated page the canvas shows (0 for the first).
    pub(super) sheet: usize,
    /// The Project options and variants window, while open.
    pub(super) options: Option<super::variants::OptionsWindow>,
    /// The Generate pages from a spec sheet window, while open.
    pub(super) generate: Option<super::variants::GenerateWindow>,
    pub(super) nav: Navigator,
    /// Where to select and centre once the canvas shows its page.
    pub(super) pending: Option<Place>,
    /// An item to select and centre once its page is shown (a row of the
    /// Changes list).
    pub(super) pending_item: Option<(PageId, AnyId)>,
    /// The person's symbol folder as last loaded into the catalog.
    pub(super) custom_symbols: super::symbol_folder::CustomSymbols,
    /// Several items to select together on a page, once it is shown (a
    /// cable's core wires from the outline).
    pub(super) pending_items: Option<(PageId, Vec<AnyId>)>,
    /// The drawing's layer and rect last frame: a focused widget there is
    /// the canvas, anywhere else a panel.
    pub(super) canvas_area: Option<(egui::LayerId, egui::Rect)>,
    /// Shift+G: every occurrence of the selection's device.
    pub(super) occurrences: Option<Vec<Reference>>,
    pub(super) drawer: Option<Drawer>,
    /// The cable whose cores the Cables tab shows.
    pub(super) drawer_cable: Option<novo_core::CableId>,
    /// The strip editor of the Terminals tab.
    pub(super) terminal_edit: super::terminal_edit::TerminalEdit,
    /// The terminal proposal Enter accepts, an index into this page's.
    pub(super) proposal: usize,
    /// Where the pointer was when Space went down, until it comes up.
    pub(super) space_down: Option<egui::Pos2>,
    /// The pointer on the drawing (mm), when it is over the canvas.
    pub(super) pointer_world: Option<novo_core::Point>,
    /// Where a Space-held pan last moved the drawing from, once the
    /// pointer has gone more than 4 px from `space_down`.
    pub(super) space_pan: Option<egui::Pos2>,
    /// The tool dock's size last frame, to tell where it would sit.
    pub(super) dock_size: egui::Vec2,
    /// The inspector's fields while they are edited.
    pub(super) inspector: super::inspector::InspectorState,
    /// The wire panel's text fields while they are edited.
    pub(super) wire_panel: super::wire_panel::WirePanel,
    /// Shift+P: false hides the marks on the drawing (counters stay).
    pub(super) marks: bool,
    /// Shift+O: false hides the override diamonds (PE set by hand).
    pub(super) override_marks: bool,
    /// Copy, cut and paste.
    pub(super) clip: super::clipboard::ClipState,
    /// Design-check results for one project revision.
    pub(super) problems: Option<(u64, Vec<Problem>)>,
    /// Where the issue pill was drawn, so its own click does not close
    /// the outline it opens.
    pub(super) pill_rect: egui::Rect,
    /// The outline's Issues filter chips: errors, warnings, infos shown.
    pub(super) issue_filter: [bool; 3],
    /// F8 position in the problem list.
    pub(super) problem_at: Option<usize>,
    /// A problem being waived (its index in the list) and the reason typed.
    pub(super) waiving: Option<(usize, String)>,
    /// The last PDF written, for tests and the toast.
    pub(super) last_export: Option<PathBuf>,
    /// Hash of each spreadsheet export this session wrote, by path: only a
    /// file still identical to what we wrote is replaced by the next export.
    pub(super) sheet_exports: std::collections::BTreeMap<PathBuf, u64>,
    /// The live potentials view (Shift+L).
    pub(super) live: super::live::LiveView,
    /// Electrical values for the live view and the motor start window.
    pub(super) sim: super::simulate::SimState,
    /// Changes mode, while it is on.
    pub(super) changes: Option<super::changes::ChangesMode>,
    /// The card's close was clicked while the outline shows Changes: the tab
    /// then waits for "Turn on" instead of switching the mode back on.
    pub(super) changes_closed: bool,
    /// Commissioning mode's data and panel state, once opened.
    pub(super) commissioning: Option<super::commissioning::CommissioningMode>,
    /// The background drawing last handed to the canvas: page, project
    /// revision and the key it has.
    /// The DXF background panel, while open; inside, the values being
    /// edited before they are committed.
    pub(super) background_panel: Option<Option<super::background_panel::Placement>>,
    /// The Claude assistant window and its conversation.
    pub(super) assistant: super::assistant::State,
    /// The Bench output window (cut list, ferrules, labels), while open.
    pub(super) bench: Option<super::bench::BenchState>,
    /// The Autopilot review list, while open.
    pub(super) autopilot: Option<super::autopilot::AutopilotState>,
    /// Paper of the workshop pack's drilling template.
    pub(super) workshop_paper: novo_layout::template::Paper,
    /// The PDF options dialog (File > Export print > Export PDF with options...), while open.
    pub(super) pdf_options: Option<super::pdf_options::PdfOptionsState>,
    /// The print preview and plot sets window (File > Export print > Print preview and plot sets...), while open.
    pub(super) print_preview: Option<Box<super::print_preview::PrintPreview>>,
    /// Test hook: the folder PDFs go to instead of `<Documents>\KoperCAD`.
    #[cfg(test)]
    pub(super) pdf_dir: Option<std::path::PathBuf>,
    /// The Report designer window, while open.
    pub(super) designer: Option<super::designer::DesignerState>,
    pub(super) underlay_seen: Option<(novo_core::PageId, u64, Option<u64>)>,
    /// An AutomationML import waiting for the person to confirm.
    pub(super) aml_import: Option<super::interop::AmlImport>,
    /// The result of a parts bulk import, until closed.
    pub(super) parts_bulk: Option<super::parts_bulk::BulkReport>,
    /// Part proposals waiting for Accept all or Cancel.
    pub(super) auto_assign: Option<super::parts_bulk::AutoAssignPreview>,
    /// An EPLAN (PXF) import waiting for the person to confirm.
    pub(super) eplan_import: Option<super::eplan_import::EplanImport>,
    /// Parts read from an EPLAN parts export, waiting for Add.
    pub(super) eplan_parts: Option<super::eplan_parts::EplanPartsImport>,
    /// The EPLAN export options being chosen.
    pub(super) eplan_export: Option<super::eplan_export::EplanExport>,
    /// The options the last EPLAN export used, offered again next time.
    pub(super) eplan_export_last: Option<super::eplan_export::EplanExport>,
    /// The Add PLC card layout dialog.
    pub(super) plc_card: Option<super::plc_card::PlcCardForm>,
    /// Whether the PLC overview window is open.
    pub(super) plc_overview: bool,
    /// The "Get the EPLAN export script" guide, open after the files are
    /// written.
    pub(super) eplan_guide: Option<super::interop::EplanGuide>,
    /// A DWG being converted in the background.
    pub(super) dwg_job: Option<super::interop::DwgJob>,
}

impl Workbench {
    /// The command field remembers the commands run in earlier sessions
    /// (newest first), so they lead its empty menu again.
    pub(super) fn with_recent(ids: &[String]) -> Self {
        let mut wb = Workbench::default();
        wb.field
            .palette
            .restore_recent_commands(ids.iter().map(String::as_str));
        wb
    }
}

impl Default for Workbench {
    fn default() -> Self {
        Workbench {
            field: CommandField::default(),
            keymap: Keymap::workflow(),
            xref: LiveXref::default(),
            preview_art: None,
            reports: Default::default(),
            wheel: None,
            place_hold: None,
            place_latch: None,
            queued: Vec::new(),
            renumber: None,
            plc_import: None,
            preferences: false,
            shortcuts: Default::default(),
            macros: Default::default(),
            parts: None,
            macro_save: None,
            macro_place: Default::default(),
            macro_editor: Default::default(),
            project_settings: false,
            issue: None,
            restore_ask: None,
            compare: None,
            faults: None,
            add_fault: None,
            calculations: None,
            electrical: None,
            safety: None,
            languages: None,
            numbering: None,
            number_prompt: None,
            net_trace: None,
            device_renumber: None,
            selection_sheet: Default::default(),
            project_rules: false,
            page_form: None,
            sheet_import: None,
            sheet: 0,
            options: None,
            generate: None,
            nav: Navigator::default(),
            pending: None,
            pending_item: None,
            custom_symbols: Default::default(),
            pending_items: None,
            canvas_area: None,
            occurrences: None,
            drawer: None,
            drawer_cable: None,
            terminal_edit: Default::default(),
            part_catalog: Default::default(),
            cable_edit: Default::default(),
            proposal: 0,
            space_down: None,
            pointer_world: None,
            space_pan: None,
            dock_size: egui::Vec2::ZERO,
            inspector: Default::default(),
            wire_panel: Default::default(),
            marks: true,
            override_marks: true,
            clip: Default::default(),
            problems: None,
            pill_rect: egui::Rect::NOTHING,
            issue_filter: [true; 3],
            problem_at: None,
            waiving: None,
            last_export: None,
            sheet_exports: Default::default(),
            live: Default::default(),
            sim: super::simulate::SimState::new(),
            changes: None,
            changes_closed: false,
            background_panel: None,
            assistant: super::assistant::State::new(),
            bench: None,
            autopilot: None,
            workshop_paper: novo_layout::template::Paper::A4,
            pdf_options: None,
            print_preview: None,
            #[cfg(test)]
            pdf_dir: None,
            designer: None,
            commissioning: None,
            underlay_seen: None,
            aml_import: None,
            parts_bulk: None,
            auto_assign: None,
            eplan_import: None,
            eplan_parts: None,
            eplan_export: None,
            eplan_export_last: None,
            plc_card: None,
            plc_overview: false,
            eplan_guide: None,
            dwg_job: None,
        }
    }
}

/// Commands the editor reads from the keyboard itself: the shell leaves
/// those key presses alone (they still run from the command field).
const EDITOR_KEYS: &[&str] = &[
    "edit.escape",
    "edit.rename",
    "edit.delete",
    "edit.rotate",
    "edit.mirror",
    "tool.connect",
    "nav.previous_page",
    "nav.next_page",
];

/// Commands the app runs besides the shell actions (`Action`) and the
/// favourites; the command field shows the rest as "Comes later".
pub(super) const APP_COMMANDS: &[&str] = &[
    "tool.place",
    "tool.repeat",
    "tool.connect",
    "tool.cable",
    "draw.text",
    "net.select",
    "net.trace",
    "help.keymap_file",
    "help.open_log_folder",
    "help.copy_problem_report",
    "wire.end_on_part",
    "wire.next_pin",
    "view.command_field_here",
    "edit.rotate",
    "edit.mirror",
    "edit.delete",
    "edit.duplicate",
    "edit.copy",
    "edit.cut",
    "edit.paste",
    "edit.paste_keep_tags",
    "edit.paste_in_place",
    "view.override_marks",
    "macro.save",
    "macro.place",
    "macro.edit",
    "edit.rename",
    "edit.escape",
    "nav.next_page",
    "nav.previous_page",
    "page.new",
    "page.rename",
    "page.duplicate",
    "page.delete",
    "page.move_up",
    "page.move_down",
    "learn.next",
    "learn.back",
    "learn.restart",
    "learn.close",
    "nav.partner",
    "nav.show_schematic",
    "nav.show_cabinet",
    "nav.show_3d",
    "nav.all_references",
    "nav.back",
    "nav.forward",
    "proposal.accept",
    "proposal.next",
    "proposal.accept_all",
    "view.terminals_drawer",
    "view.cables_drawer",
    "view.devices_drawer",
    "view.toggle_marks",
    "sim.live",
    "sim.why",
    "sim.plc_program",
    "sim.faults",
    "sim.add_fault",
    "view.changes",
    "view.commissioning",
    "check.problems",
    "check.calculations",
    "check.electrical_inputs",
    "check.safety",
    "project.languages",
    "check.next_problem",
    "check.previous_problem",
    "export.pdf",
    "export.print_preview",
    "export.pdf_options",
    "export.change_report",
    "export.commissioning",
    "export.punch_list",
    "export.standards_report",
    "export.standards_xlsx",
    "export.plate",
    "export.plate_dxf",
    "export.pdf_page",
    "export.report_pdf",
    "report.add_page",
    "export.assembly",
    "export.workshop",
    "export.safety_sheets",
    "export.sheets",
    "export.sheets_csv",
    "export.wirecut",
    "export.wirecut_xlsx",
    "export.wiring_dxf",
    "export.wirecut_page",
    "view.bench",
    "report.designer",
    "export.bench_cut",
    "export.ferrules",
    "export.labels",
    "cabinet.routes",
    "cabinet.rails",
    "cabinet.rules",
    "cabinet.plate",
    "cabinet.cutlist",
    "cabinet.door",
    "cabinet.surface",
    "export.bom",
    "export.reports",
    "view.reports",
    "view.3d",
    "view.3d_door",
    "view.3d_shell",
    "view.3d_wires",
    "view.3d_shadows",
    "view.3d_align_left",
    "view.3d_align_hcentre",
    "view.3d_align_right",
    "view.3d_align_top",
    "view.3d_align_vcentre",
    "view.3d_align_bottom",
    "view.3d_spread_h",
    "view.3d_spread_v",
    "view.3d_ssao",
    "view.3d_dimensions",
    "view.3d_quality",
    "view.3d_stats",
    "view.3d_measure",
    "view.3d_section",
    "view.3d_save_view",
    "view.3d_wire_runs",
    "view.3d_hints",
    "view.3d_select_devices",
    "view.3d_select_rails",
    "view.3d_select_ducts",
    "view.3d_hide",
    "view.3d_isolate",
    "view.3d_show_all",
    "view.3d_duplicate",
    "view.3d_position",
    "view.3d_place",
    "view.iso",
    "view.front",
    "view.top",
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
    "file.import_dxf_background",
    "view.background",
    "view.background_place",
    "ai.assistant",
    "file.remove_background",
    "export.dxf_page",
    "export.dxf_all",
    "import.spreadsheet",
    "export.step",
    "export.aml",
    "export.eplan",
    "file.import_aml",
    "file.import_crane",
    "file.import_eplan",
    "file.import_dwg",
    "file.choose_dwg_converter",
    "file.eplan_export_script",
    "plc.add_card",
    "plc.overview",
    "parts.import_eplan",
    "plc.to_plc",
    "plc.renumber_io",
    "plc.import_tags",
    "export.plc_csv",
    "export.plc_plcopen",
    "export.plc_tia",
    "export.plc_studio5000",
    "export.plc_address_plan",
    "xref.link",
    "app.preferences",
    "view.shortcuts",
    "help.show_hints",
    "help.next_lesson",
    "help.whats_new",
    "help.numbers",
    "view.reset_panels",
    "layout.draw",
    "layout.cabinet",
    "layout.3d",
    "layout.review",
    "layout.reports",
    "layout.reset",
    "layout.reset_all",
    "parts.browser",
    "parts.import_step",
    "parts.import_folder",
    "parts.auto_assign",
    "edit.shared_properties",
    "parts.clear",
    "parts.remove_unused",
    "project.standard",
    "project.rules",
    "edit.renumber",
    "draw.tidy",
    "macro.browser",
    "macro.import",
    "macro.export_pack",
    "project.options",
    "project.variant",
    "project.generate",
    "export.variant_pdf",
    "export.spec",
    "revision.issue",
    "revision.compare",
    "revision.lock_page",
    "revision.unlock_page",
    "revision.restore_page",
    "terminal.toggle_pe",
    "terminal.reset_pe",
    "wire.mode",
    "edit.reset_note",
    "interruption.renumber",
    "wires.autopilot",
    "wires.number",
    "wires.numbering",
    "wires.renumber",
    "wires.check_numbers",
    "wires.set_number",
    "wires.unlock_number",
    "cable.rename",
    "cable.delete",
    "cable.retype",
    "cable.restore_proposals",
    "edit.select_all",
    "view.cabinet_table",
    "view.fit_sheet",
    "cabinet.place-all",
    "heat.toggle",
    "heat.airflow",
    "heat.contours",
    "heat.simulate",
    "terminal.connect",
    "terminal.disconnect",
    "terminal.strip_up",
    "terminal.strip_down",
    "terminal.strip_lowest",
    "terminal.add_spares",
    "terminal.rename",
    "terminal.renumber",
    "terminal.join_duplicates",
    "terminal.set_kind",
    "terminal.set_level",
    "terminal.set_jumper",
    "terminal.order_up",
    "terminal.order_down",
    "terminal.style",
    "terminal.restore_declined",
];

/// Whether the app can run a command now (not "comes later").
pub(crate) fn app_runs(id: &str) -> bool {
    APP_COMMANDS.contains(&id) || FAVOURITE_COMMANDS.contains(&id) || Action::from_id(id).is_some()
}

impl NovoApp {
    pub(super) fn library(&self) -> Option<&Library> {
        self.canvas
            .schematic_ref()
            .map(|view| view.editor.catalog.library())
    }

    /// The canvas selection as model ids.
    pub fn selection(&self) -> Vec<AnyId> {
        self.canvas
            .schematic_ref()
            .map(|view| view.selection())
            .unwrap_or_default()
    }

    pub(super) fn selected_symbol(&self) -> Option<SymbolId> {
        match self.selection().as_slice() {
            [AnyId::Symbol(s)] => Some(*s),
            _ => None,
        }
    }

    /// How many symbols are selected (a link needs a contact and a coil).
    pub(super) fn selected_symbol_count(&self) -> usize {
        self.selection()
            .iter()
            .filter(|id| matches!(id, AnyId::Symbol(_)))
            .count()
    }

    pub(super) fn here(&self) -> Option<Place> {
        Some(Place {
            page: self.current_page?,
            symbol: self.selected_symbol(),
        })
    }

    /// What availability in the command field is decided on.
    pub(super) fn command_context(&self) -> Context {
        let doc = self.backend.document();
        let selection = self.selection();
        let devices = doc.map_or(0, |d| {
            let project = d.project();
            let mut devices: Vec<DeviceId> = selection
                .iter()
                .filter_map(|&id| novo_editor::edit::device_of(project, id))
                .collect();
            devices.dedup();
            devices.len()
        });
        Context {
            has_project: doc.is_some(),
            has_page: self.current_page.is_some(),
            selection: selection.len(),
            has_device: devices == 1,
            can_undo: doc.is_some_and(Document::can_undo),
            can_redo: doc.is_some_and(Document::can_redo),
            // Paste from the menu and the command field uses the last copy made
            // here (a key press reads the system clipboard itself).
            clipboard: self.wb.clip.last.is_some(),
            can_go_back: self.wb.nav.history.can_go_back(),
            can_go_forward: self.wb.nav.history.can_go_forward(),
        }
    }

    // ------------------------------------------------------------------
    // Keys and the command field

    /// Who has the keyboard, for the keymap.
    pub(super) fn key_focus(&self, ctx: &egui::Context) -> Focus {
        if let Some(focus) = self.wb.field.focus() {
            focus
        } else if ctx.text_edit_focused() {
            Focus::TextField
        } else if self.is_open()
            && self.workspace() != Workspace::Reports
            && !self.panel_has_focus(ctx)
        {
            // The Reports table has the keyboard, not the hidden drawing.
            Focus::Canvas
        } else {
            Focus::Panel
        }
    }

    /// A button or other widget outside the drawing has the keyboard, so
    /// Enter, Tab and G go to it rather than to the canvas.
    fn panel_has_focus(&self, ctx: &egui::Context) -> bool {
        let Some(id) = ctx.memory(|m| m.focused()) else {
            return false;
        };
        let Some(widget) = ctx.read_response(id) else {
            return false;
        };
        match self.wb.canvas_area {
            Some((layer, rect)) => widget.layer_id != layer || !rect.contains_rect(widget.rect),
            None => true,
        }
    }

    /// Enter, Tab and Delete belong to the editor's wire proposals while it
    /// has any; otherwise to the terminal proposals shown on this page.
    pub(super) fn terminal_proposals_have_keys(&self) -> bool {
        let wires = self
            .canvas
            .schematic_ref()
            .is_some_and(|v| !v.editor.proposals().is_empty());
        !wires && self.wb.marks && !self.page_proposals().is_empty()
    }

    /// Delete with the pointer on the highlighted terminal proposal
    /// declines it rather than deleting anything; elsewhere Delete deletes
    /// (as the editor does for wire proposals).
    fn declines_proposal(&self) -> bool {
        self.terminal_proposals_have_keys() && self.pointer_on_proposal()
    }

    /// Where the pointer is on the drawing, in millimetres, for the keys
    /// that act on what is under it.
    pub(super) fn track_pointer(&mut self, ctx: &egui::Context) {
        let at = ctx.pointer_hover_pos();
        self.wb.pointer_world = match (at, self.wb.canvas_area, self.canvas.schematic_ref()) {
            (Some(at), Some((_, rect)), Some(view)) if rect.contains(at) => {
                let local = at - rect.min;
                let w = view
                    .canvas
                    .viewport
                    .screen_to_world(novo_canvas::Vec2::new(local.x, local.y));
                Some(novo_core::Point::new(f64::from(w.x), f64::from(w.y)))
            }
            _ => None,
        };
    }

    /// Whether the shell runs `command` from a key press itself.
    pub(super) fn claims_key(&self, command: &str) -> bool {
        // Delete with the pointer on the highlighted terminal proposal
        // declines it (DESIGN.md §5); otherwise it is the editor's.
        // The Cabinet has no editor to read these keys itself, so the shell
        // takes them and hands them to the plate (the 3D view edits the same plate).
        if matches!(self.workspace(), Workspace::Cabinet | Workspace::View3d)
            && matches!(command, "edit.rotate" | "edit.delete" | "edit.mirror")
        {
            return true;
        }
        if command == "edit.delete" {
            return self.declines_proposal();
        }
        if EDITOR_KEYS.contains(&command) {
            return false;
        }
        match command {
            // Enter and Tab belong to the editor's wire proposals while it
            // has any; otherwise to the terminal proposals on this page.
            "proposal.accept" | "proposal.next" => self.terminal_proposals_have_keys(),
            id if FAVOURITE_COMMANDS.contains(&id) => true,
            id => match Action::from_id(id) {
                Some(action) => self.is_enabled(action),
                None => novo_workflow::commands::is_known(id),
            },
        }
    }

    /// Escape closes the command field, a list or the outline flyout over
    /// the canvas first. Taken before the canvas runs, which would read it
    /// as its own Escape ladder while the pointer is over the drawing.
    pub(super) fn escape_popups(&mut self, ctx: &egui::Context) {
        let open = self.wb.field.is_open() || self.wb.occurrences.is_some() || self.outline_open;
        if open && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
            if self.wb.field.is_open() {
                self.wb.field.close();
            } else if self.wb.occurrences.is_some() {
                self.wb.occurrences = None;
            } else {
                self.outline_open = false;
            }
        }
    }

    /// The commands this frame's key presses run, taken out of the input.
    pub(super) fn key_commands(&self, ctx: &egui::Context) -> Vec<&'static str> {
        let focus = self.key_focus(ctx);
        // A wire being drawn gives Q and Tab their own meaning.
        let wiring = self
            .canvas
            .schematic_ref()
            .is_some_and(|v| v.editor.is_wiring());
        keys::take_commands(ctx, &self.wb.keymap, focus, wiring, |c| self.claims_key(c))
    }

    /// Runs a command from a key press. Space only arms the command field:
    /// it opens on the release, unless the pointer moved more than 4 px in
    /// between (a held Space pans); every other key runs its command.
    pub(super) fn run_key_command(&mut self, ctx: &egui::Context, id: &str) {
        crate::diag::note_command(id);
        if id == "tool.place" {
            self.learn_command(id);
            self.hint_command_run(id);
            self.tour_tick("place");
            self.arm_place_hold(ctx);
        } else if id == "view.command_field_here" {
            self.wb.space_down = ctx.pointer_latest_pos().or(Some(egui::Pos2::ZERO));
        } else {
            self.run_command(ctx, id, None);
        }
    }

    /// Runs a command by its novo-workflow id, from a key or the command
    /// field. `argument` is what was typed after its verb.
    pub(super) fn run_command(&mut self, ctx: &egui::Context, id: &str, argument: Option<&str>) {
        crate::diag::note_command(id);
        self.learn_command(id);
        // The other Simulate commands open their own views: the live view
        // does not stay switched on behind them.
        if matches!(
            id,
            "simulate.motor-start" | "view.protection" | "heat.simulate"
        ) {
            self.wb.live.leave();
        }
        if id == "help.learn" || id.starts_with("learn.") {
            self.learn_run_command(id, argument);
            return;
        }
        if let Some(slot) = FAVOURITE_COMMANDS.iter().position(|&c| c == id) {
            let digit = (slot + 1).to_string();
            let target = self.with_field(|field, sources| field.pick_shortcut(&digit, sources));
            if let Some(target) = target {
                self.run_target(ctx, target);
            }
            return;
        }
        if let Some(action) = Action::from_id(id) {
            if action == Action::CommandPalette {
                self.tour_tick("commands");
                self.wb.field.open(Mode::All, None);
            } else {
                self.run(ctx, action);
            }
            return;
        }
        if id == "edit.duplicate" {
            self.tour_tick("duplicate");
        }
        if self.workspace() == Workspace::Cabinet && matches!(id, "edit.rotate" | "edit.delete") {
            self.cabinet.keys.push(id.to_owned());
            return;
        }
        let pointer = ctx.pointer_latest_pos();
        match id {
            "view.cabinet_table" if self.is_open() => {
                let showing = self.workspace() == Workspace::Cabinet && self.cabinet.table;
                self.cabinet.table = !showing;
                self.switch_workspace(Workspace::Cabinet);
            }
            "view.fit_sheet" if self.is_open() => {
                if self.workspace() == Workspace::View3d {
                    self.view3d.pending = Some(super::view3d::Pending::Fit);
                } else {
                    self.canvas.zoom_to_sheet();
                }
            }
            "cabinet.place-all" if self.is_open() => {
                self.switch_workspace(Workspace::Cabinet);
                self.cabinet.table = false;
                self.cabinet.keys.push(id.to_owned());
            }
            "heat.toggle" | "heat.airflow" | "heat.contours" | "heat.simulate"
                if self.is_open() =>
            {
                // Simulate stays in the 3D view when it is the one open.
                if !(id == "heat.simulate" && self.workspace() == Workspace::View3d) {
                    self.switch_workspace(Workspace::Cabinet);
                    self.cabinet.table = false;
                } else {
                    // The run's progress and stop button live in the panel.
                    self.view3d.controls.heat_panel = true;
                }
                self.cabinet.heat.command(id);
            }
            "tool.place" => match argument.filter(|a| !a.trim().is_empty()) {
                Some(text) => self.wb.field.open_with(Mode::Place, text, pointer),
                None => self.wb.field.open(Mode::Place, pointer),
            },
            "view.command_field_here" => self.wb.field.open(Mode::All, pointer),
            "tool.connect" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_wire();
                }
            }
            "tool.repeat" => self.editor_key(EditorKey::Repeat),
            "tool.cable" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_cable();
                }
            }
            "draw.text" => {
                if let Some(view) = self.canvas.schematic() {
                    view.editor.begin_text();
                }
            }
            "net.select" => self.select_net(),
            "net.trace" => self.trace_net(),
            "help.keymap_file" => self.open_keymap_file(),
            "help.open_log_folder" => self.open_log_folder(),
            "help.copy_problem_report" => self.copy_problem_report(ctx),
            "wire.end_on_part" => self.editor_key(EditorKey::EndOnPart),
            "wire.next_pin" => {
                // Tab must not leave the drawing: egui's own focus move to
                // the next widget is undone, as for the next proposal.
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
                self.editor_key(EditorKey::NextPin)
            }
            "edit.rotate" => self.editor_key(EditorKey::Rotate),
            "edit.mirror" => self.editor_key(EditorKey::Mirror),
            "edit.duplicate" if self.workspace() == Workspace::View3d => self.view3d_duplicate(),
            "view.3d_duplicate" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d_duplicate();
            }
            "edit.reset_note" => {
                let selection = self.selection();
                self.with_editor(|editor, doc| editor.reset_notes(doc, &selection));
            }
            "edit.duplicate" => self.duplicate_selection(),
            "edit.copy" => self.copy_selection(ctx, false),
            "edit.cut" => self.copy_selection(ctx, true),
            "edit.paste" | "edit.paste_keep_tags" | "edit.paste_in_place" => self.paste_command(id),
            "view.override_marks" => self.wb.override_marks ^= true,
            "macro.save" => self.open_macro_save(argument),
            "macro.place" => self.open_macro_place(argument),
            "macro.edit" => self.open_macro_editor(argument),
            "edit.delete" if self.declines_proposal() => self.dismiss_proposal(),
            "edit.delete" => self.editor_key(EditorKey::Delete),
            "nav.next_page" | "nav.previous_page" => {
                let step = if id == "nav.next_page" { 1 } else { -1 };
                if let (Some(doc), Some(page)) = (self.backend.document(), self.current_page)
                    && let Some(to) = novo_editor::pages::neighbour(doc.project(), page, step)
                {
                    self.show_page(to);
                }
            }
            "page.new" => self.new_page(),
            "page.rename" | "page.duplicate" | "page.delete" | "page.move_up"
            | "page.move_down" => self.page_command(id),
            "nav.partner" => self.go_partner(),
            "nav.show_schematic" => self.show_in_schematic(),
            "nav.show_cabinet" => self.show_on_plate(Workspace::Cabinet),
            "nav.show_3d" => self.show_on_plate(Workspace::View3d),
            "nav.all_references" => self.list_occurrences(),
            "nav.back" | "nav.forward" => self.go_history(id == "nav.back"),
            "proposal.accept" => self.accept_proposal(),
            "proposal.next" => {
                // Tab is "next proposal" on the drawing: egui's own focus
                // move to the next widget is undone.
                ctx.memory_mut(|m| {
                    if let Some(id) = m.focused() {
                        m.surrender_focus(id);
                    }
                });
                self.next_proposal();
            }
            "proposal.accept_all" => self.accept_all_proposals(),
            "edit.select_all" => self.select_all_on_page(),
            "terminal.connect"
            | "terminal.toggle_pe"
            | "terminal.disconnect"
            | "terminal.reset_pe"
            | "terminal.strip_up"
            | "terminal.strip_down"
            | "terminal.strip_lowest" => self.terminal_command(id),
            "terminal.add_spares"
            | "terminal.rename"
            | "terminal.renumber"
            | "terminal.join_duplicates"
            | "terminal.set_kind"
            | "terminal.set_level"
            | "terminal.set_jumper"
            | "terminal.order_up"
            | "terminal.order_down"
            | "terminal.style"
            | "terminal.restore_declined" => self.terminal_edit_command(id, argument),
            "interruption.renumber" => self.renumber_interruptions(),
            "wires.autopilot" => self.open_autopilot(),
            "wires.number" => self.number_wires(),
            "wires.numbering" => {
                self.open_numbering(novo_numbering::Mode::Fill, super::numbering::Tab::Preview)
            }
            "wires.renumber" => self.open_numbering(
                novo_numbering::Mode::Renumber,
                super::numbering::Tab::Preview,
            ),
            "wires.check_numbers" => {
                self.open_numbering(novo_numbering::Mode::Fill, super::numbering::Tab::Conflicts)
            }
            "wires.set_number" => self.set_wire_number(argument),
            "wires.unlock_number" => self.unlock_wire_number(),
            "cable.rename" => self.rename_cable_command(argument),
            "cable.delete" => self.delete_cable_command(),
            "cable.retype" => self.retype_cable_command(argument),
            "cable.restore_proposals" => self.restore_declined_cables(),
            "view.terminals_drawer" => self.toggle_drawer(Drawer::Terminals),
            "view.cables_drawer" => self.toggle_drawer(Drawer::Cables),
            "view.devices_drawer" => self.toggle_drawer(Drawer::Devices),
            "view.toggle_marks" => self.wb.marks ^= true,
            "view.reports" => self.switch_workspace(super::workspace::Workspace::Reports),
            "view.3d" => self.switch_workspace(Workspace::View3d),
            "view.3d_shadows" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.controls.shadows ^= true;
            }
            "view.3d_quality" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.view.reset_quality();
            }
            "view.3d_dimensions" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.controls.dimensions ^= true;
            }
            "view.3d_ssao" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.controls.ssao ^= true;
            }
            "view.3d_door" | "view.3d_shell" | "view.3d_wires" => {
                self.switch_workspace(Workspace::View3d);
                let limit = self.view3d_door_limit();
                let controls = &mut self.view3d.controls;
                match id {
                    "view.3d_door" => {
                        controls.layers.door = true;
                        controls.toggle_door(limit);
                    }
                    "view.3d_shell" => controls.layers.shell = !controls.layers.shell,
                    _ => controls.layers.wires = !controls.layers.wires,
                }
            }
            "view.3d_measure" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.measure.toggle();
            }
            "view.3d_section" => {
                self.switch_workspace(Workspace::View3d);
                let section = &mut self.view3d.view.state.section;
                *section = match section {
                    Some(_) => None,
                    None => Some(novo_view3d::Section::default()),
                };
            }
            "view.3d_save_view" => {
                // Saved in the view's next frame, once the camera is fitted.
                self.switch_workspace(Workspace::View3d);
                self.view3d.save_view = true;
            }
            "view.3d_align_left"
            | "view.3d_align_hcentre"
            | "view.3d_align_right"
            | "view.3d_align_top"
            | "view.3d_align_vcentre"
            | "view.3d_align_bottom"
            | "view.3d_spread_h"
            | "view.3d_spread_v" => {
                self.switch_workspace(Workspace::View3d);
                if let Some(op) = super::view3d_align::Op::from_command(id) {
                    self.view3d_arrange(op);
                }
            }
            "view.3d_wire_runs" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.wire_runs ^= true;
                if self.view3d.wire_runs {
                    self.view3d.controls.layers.wires = true;
                }
            }
            "view.3d_hints" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.hints.toggle();
            }
            "view.3d_select_devices" | "view.3d_select_rails" | "view.3d_select_ducts" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d_select_kind(match id {
                    "view.3d_select_devices" => ItemKind::Device,
                    "view.3d_select_rails" => ItemKind::Rail,
                    _ => ItemKind::Duct,
                });
            }
            "view.3d_hide" | "view.3d_isolate" | "view.3d_show_all" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d_visibility(id);
            }
            "view.3d_position" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.position.toggle();
            }
            "view.3d_place" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.place.toggle();
            }
            "view.3d_stats" => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.stats.toggle();
            }
            "view.iso" | "view.front" | "view.top" if self.is_open() => {
                self.switch_workspace(Workspace::View3d);
                self.view3d.pending = Some(match id {
                    "view.iso" => Pending::Preset(ViewPreset::Iso),
                    "view.front" => Pending::Preset(ViewPreset::Front),
                    _ => Pending::Preset(ViewPreset::Top),
                });
            }
            "export.bom" => {
                self.export_report("bom", super::reports_view::ReportFormat::Excel);
            }
            "export.reports" => {
                self.export_all_reports();
            }
            "sim.live" | "sim.why" | "sim.plc_program" => {
                // The live view and the PLC window draw only over the
                // schematic: go there first so the command is not silent.
                if self.workspace() != Workspace::Schematic && self.is_open() {
                    self.switch_workspace(Workspace::Schematic);
                    self.message(
                        Level::Info,
                        "Simulation runs on the schematic: switched to it",
                    );
                }
                if id == "sim.live" {
                    self.toggle_live();
                } else if id == "sim.why" {
                    self.ask_why();
                } else {
                    self.open_plc_program();
                }
            }
            _ if super::reports_view::REPORT_COMMANDS
                .iter()
                .any(|(c, _)| *c == id) =>
            {
                if let Some((_, report)) = super::reports_view::REPORT_COMMANDS
                    .iter()
                    .find(|(c, _)| *c == id)
                {
                    self.show_report(report);
                }
            }
            "sim.faults" => self.toggle_faults(),
            "sim.add_fault" => self.begin_add_fault(),
            "view.changes" => self.toggle_changes(argument),
            "view.commissioning" if self.workspace() == Workspace::Commissioning => {
                self.switch_workspace(Workspace::Schematic);
            }
            "view.commissioning" => self.switch_workspace(Workspace::Commissioning),
            "check.problems" => self.show_issues(false),
            "check.calculations" => self.toggle_calculations(),
            "check.electrical_inputs" => self.toggle_electrical(),
            "check.safety" => self.toggle_safety(),
            "project.languages" => self.toggle_languages(),
            "check.next_problem" => self.step_problem(true),
            "check.previous_problem" => self.step_problem(false),
            "edit.escape" => {
                // The same ladder Esc climbs on the drawing: the step in
                // progress, then the tool, then the selection.
                if let Some(view) = self.canvas.schematic()
                    && !view.editor.cancel_step()
                    && !view.editor.leave_tool()
                {
                    view.canvas.selection.clear();
                }
            }
            "edit.properties" | "edit.rename" => {
                // The inspector shows with the selection; the cursor goes in
                // its tag.
                let selection = self.selection();
                let device = self
                    .backend
                    .document()
                    .and_then(|d| super::inspector::selected_device(d.project(), &selection));
                if device.is_some() {
                    ctx.memory_mut(|m| m.request_focus(egui::Id::new("inspector_tag")));
                }
            }
            "tool.place_wheel" => self.open_wheel(ctx, Vec::new()),
            "wire.mode" => {
                let mode = match self.settings.wiring_mode() {
                    WiringMode::Corners => WiringMode::Lines,
                    WiringMode::Lines => WiringMode::Corners,
                };
                self.set_wiring_mode(mode, false);
            }
            "export.pdf" => {
                self.export_pdf();
            }
            "export.print_preview" => self.open_print_preview(),
            "export.pdf_options" => self.open_pdf_options(),
            "export.change_report" => {
                self.export_change_report();
            }
            "export.commissioning" => self.export_commissioning_pdf(),
            "export.punch_list" => self.export_punch_list_pdf(),
            "export.standards_report" => {
                self.export_standards_report(StandardsFormat::Pdf);
            }
            "export.standards_xlsx" => {
                self.export_standards_report(StandardsFormat::Excel);
            }
            "export.plate" => {
                self.export_plate_pdf();
            }
            "export.plate_dxf" => {
                self.export_plate_dxf();
            }
            "export.pdf_page" => {
                self.export_pdf_page();
            }
            "export.report_pdf" => {
                self.export_selected_report_pdf();
            }
            "report.add_page" => self.add_report_page_command(argument),
            "export.assembly" => {
                self.export_assembly();
            }
            "export.safety_sheets" => {
                self.export_safety_sheets();
            }
            "export.workshop" => {
                self.export_workshop();
            }
            "export.sheets" => {
                self.export_spreadsheet();
            }
            "cabinet.routes" => self.toggle_cabinet_routes(),
            "cabinet.rails" => self.toggle_cabinet_rails(),
            "cabinet.rules" if self.is_open() => {
                self.switch_workspace(Workspace::Cabinet);
                self.cabinet.table = false;
                self.cabinet.rules.editor_open ^= true;
            }
            "cabinet.cutlist" if self.is_open() => {
                self.switch_workspace(Workspace::Cabinet);
                self.cabinet.table = false;
                self.cabinet.rules.cut_open ^= true;
            }
            "cabinet.plate" if self.is_open() => {
                self.switch_workspace(Workspace::Cabinet);
                self.cabinet.table = false;
                self.cabinet.keys.push(id.to_owned());
            }
            "cabinet.door" => self.open_cabinet_door(),
            "cabinet.surface" if self.is_open() => {
                self.switch_workspace(Workspace::Cabinet);
                self.cabinet.table = false;
                self.open_cabinet_surfaces();
            }
            "export.wirecut" => {
                self.export_wire_cut();
            }
            "export.wirecut_xlsx" => {
                self.export_wire_production();
            }
            "export.wiring_dxf" => self.export_wiring_dxf_of_panel(),
            "export.wirecut_page" => self.add_wire_production_page(),
            "view.bench" => self.open_bench(),
            "report.designer" => self.open_designer(),
            "export.bench_cut" => {
                self.export_bench_cut();
            }
            "export.ferrules" => {
                self.export_ferrules();
            }
            "export.labels" => {
                self.export_labels();
            }
            "export.sheets_csv" => {
                self.export_spreadsheet_csv();
            }
            "file.import_dxf_background" => self.import_dxf_background(),
            "view.background" => self.toggle_background(),
            "view.background_place" => self.open_background_panel(),
            "ai.assistant" => self.open_assistant(),
            "file.remove_background" => self.remove_background(),
            "export.dxf_page" => {
                self.export_dxf(super::dxf::DxfScope::CurrentPage);
            }
            "export.dxf_all" => {
                self.export_dxf(super::dxf::DxfScope::AllPages);
            }
            "import.spreadsheet" => self.import_sheet(),
            "export.step" => {
                self.export_step();
            }
            "export.eplan" => self.open_eplan_export(),
            "export.aml" => {
                self.export_aml();
            }
            "file.import_aml" => self.import_aml(),
            "file.import_crane" => self.import_crane(),
            "file.import_eplan" => self.import_eplan(),
            "file.import_dwg" => self.import_dwg(),
            "file.choose_dwg_converter" => self.choose_dwg_converter(),
            "file.eplan_export_script" => self.eplan_export_script(),
            "plc.add_card" => self.plc_add_card(argument),
            "plc.to_plc" => self.plc_to_plc(),
            "plc.renumber_io" => self.plc_renumber_io(),
            "plc.import_tags" => self.plc_import_tags(),
            "export.plc_csv" => {
                self.export_plc(LinkFormat::Csv);
            }
            "export.plc_plcopen" => {
                self.export_plc(LinkFormat::PlcOpen);
            }
            "export.plc_tia" => {
                self.export_plc(LinkFormat::Tia);
            }
            "export.plc_studio5000" => {
                self.export_plc(LinkFormat::Studio5000);
            }
            "export.plc_address_plan" => {
                self.export_plc(LinkFormat::Hardware);
            }
            "xref.link" => self.xref_link(argument),
            "app.preferences" => {
                // Preferences is a modal: a floating panel left open behind
                // it reads as two windows stacked on a small screen.
                self.sizing.panel_open = false;
                self.wb.preferences = true;
            }
            "view.shortcuts" => self.toggle_shortcuts(),
            "help.show_hints" => self.show_hints_again(),
            "help.whats_new" => self.show_whats_new = true,
            "help.next_lesson" => self.learn_run_command(id, None),
            "help.numbers" => self.show_numbers = true,
            "view.reset_panels" => super::panel::reset_layout(ctx),
            "layout.draw" | "layout.cabinet" | "layout.3d" | "layout.review" | "layout.reports"
            | "layout.reset" | "layout.reset_all" => {
                self.run_layout_command(id);
            }
            "parts.browser" => self.open_parts_browser(),
            "parts.import_step" => self.import_part_model(),
            "parts.import_eplan" => self.import_eplan_parts(),
            "plc.overview" => self.wb.plc_overview = true,
            "parts.import_folder" => self.import_parts_folder(),
            "parts.auto_assign" => self.start_auto_assign(),
            "edit.shared_properties" => self.open_selection_sheet(),
            "parts.clear" => self.clear_selected_parts(),
            "parts.remove_unused" => self.remove_unused_project_parts(),
            "project.standard" => self.wb.project_settings = true,
            "project.rules" => self.wb.project_rules = true,
            "edit.renumber" => self.open_device_renumber(argument),
            "draw.tidy" => self.tidy_selection(),
            "macro.browser" => self.wb.macros.show(),
            "macro.import" => self.macro_import(),
            "macro.export_pack" => self.macro_export_pack(),
            "project.options" => self.open_options(),
            "project.variant" => self.switch_variant(argument),
            "project.generate" => self.open_generate(),
            "export.variant_pdf" => {
                self.export_variant_pdf(argument);
            }
            "export.spec" => {
                self.export_spec();
            }
            "revision.issue" => self.open_issue_revision(argument),
            "revision.compare" => self.open_compare(argument),
            "revision.lock_page" => self.set_page_lock(true),
            "revision.unlock_page" => self.set_page_lock(false),
            "revision.restore_page" => self.restore_page_command(argument),
            other => {
                let label = novo_workflow::commands::get(other).map_or(other, |c| c.label);
                self.message(Level::Info, format!("{label} is not in the app yet"));
            }
        }
    }

    /// Selects everything drawn on the current page (Ctrl+A). Selection is
    /// not a model edit, so there is nothing to undo.
    pub(super) fn select_all_on_page(&mut self) {
        let (Some(page), Some(doc)) = (self.current_page, self.backend.document()) else {
            return;
        };
        let project = doc.project();
        let ids: Vec<AnyId> = project
            .symbols()
            .filter(|s| s.page == page)
            .map(|s| AnyId::Symbol(s.id))
            .chain(
                project
                    .junctions()
                    .filter(|j| j.page == page)
                    .map(|j| AnyId::Junction(j.id)),
            )
            .chain(
                project
                    .wires()
                    .filter(|w| w.page == page)
                    .map(|w| AnyId::Wire(w.id)),
            )
            .chain(
                project
                    .texts()
                    .filter(|t| t.page == page)
                    .map(|t| AnyId::Text(t.id)),
            )
            .collect();
        if let Some(view) = self.canvas.schematic() {
            view.select_ids(&ids);
        }
    }

    /// Does what picking a command field row asks.
    pub(super) fn run_target(&mut self, ctx: &egui::Context, target: Target) {
        match target {
            Target::Command(id) => {
                self.settings.remember_action(id);
                self.run_command(ctx, id, None);
            }
            Target::Run { command, argument } => self.run_command(ctx, command, Some(&argument)),
            Target::Page(page) => self.show_page(page),
            Target::Device(device) => {
                let to = self.device_main(device);
                match to {
                    Some(place) => self.jump(place, true),
                    None => self.message(Level::Info, "That device is not drawn on any page"),
                }
            }
            Target::Location { symbol, .. } => {
                let page = self
                    .backend
                    .document()
                    .and_then(|d| d.project().symbol(symbol))
                    .map(|s| s.page);
                if let Some(page) = page {
                    self.jump(
                        Place {
                            page,
                            symbol: Some(symbol),
                        },
                        true,
                    );
                }
            }
            Target::Lesson(id) => self.learn_run_command_lesson(&id),
            // The place window opens on the macro, to be confirmed there.
            Target::Macro(name) => self.open_macro_place(Some(&name)),
            Target::Cable(cable) => self.show_cable(cable),
            Target::Wire(wire) => self.go_to_wire(wire),
            Target::Symbol(symbol) => self.begin_place(&symbol, None),
            Target::Child { symbol, device } => {
                let tag = self
                    .backend
                    .document()
                    .and_then(|d| d.project().device(device))
                    .map(|d| d.tag.to_string());
                self.begin_place(&symbol, tag.as_deref());
            }
        }
    }

    pub(super) fn begin_place(&mut self, symbol: &str, parent: Option<&str>) {
        if self.current_page.is_none() {
            return;
        }
        if let Some(view) = self.canvas.schematic() {
            view.editor.begin_place(symbol, parent);
        }
    }

    pub(super) fn editor_key(&mut self, key: EditorKey) {
        let selection = self.selection();
        self.with_editor(|editor, doc| editor.key(doc, key, &selection));
    }

    pub(super) fn new_page(&mut self) {
        let Some(after) = self.current_page else {
            return;
        };
        if let Some(Some(page)) = self.with_editor(|editor, doc| editor.new_page(doc, after)) {
            self.show_page(page);
        }
    }

    /// Runs an editor edit from outside the canvas frame (a command or a
    /// key the shell handles) and records what it changed at once: storage
    /// and the views hear about it now, so a save or autosave before the
    /// next frame cannot miss it. `None` without an editor or a project.
    pub(super) fn with_editor<R>(
        &mut self,
        edit: impl FnOnce(&mut novo_editor::Editor, &mut Document) -> R,
    ) -> Option<R> {
        let view = self.canvas.schematic()?;
        let doc = self.backend.document_mut()?;
        let result = edit(&mut view.editor, doc);
        let changes = view.editor.take_changes();
        self.changed_outside_canvas(&changes);
        Some(result)
    }

    /// Shows the command field when open and runs its pick.
    pub(super) fn command_field(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.field.is_open() {
            return;
        }
        self.wb.field.ask_enabled = self.settings.ai.enabled;
        let picked = self.with_field(|field, sources| field.show(ctx, canvas, sources));
        if let Some(rect) = self.wb.field.rect() {
            self.anchor("palette.field", rect, "Command field");
        }
        if let Some(question) = self.wb.field.asked.take() {
            self.ask_claude(ctx, question);
        }
        if let Some(target) = picked {
            self.run_target(ctx, target);
        }
    }

    /// Opens the command field, types `query` and presses Enter, as a user
    /// would; a step row (`no `) only completes the text. Returns whether a
    /// row ran. For scripts and tests that drive the whole window.
    pub fn type_command(&mut self, ctx: &egui::Context, mode: Mode, query: &str) -> bool {
        self.wb.field.open(mode, None);
        self.wb.field.type_text(query);
        let picked = self.with_field(|field, sources| field.enter(sources));
        match picked {
            Some(target) => {
                self.run_target(ctx, target);
                true
            }
            None => false,
        }
    }

    /// Runs `f` with the command field and what it searches.
    pub(super) fn with_field<R>(
        &mut self,
        f: impl FnOnce(&mut CommandField, &Sources<'_>) -> R,
    ) -> R {
        let mut field = std::mem::take(&mut self.wb.field);
        let lessons = self.learn.palette_entries();
        let macros = self.wb.macro_place.palette_entries();
        let result = {
            let sources = sources(self, &lessons, &macros);
            f(&mut field, &sources)
        };
        self.wb.field = field;
        result
    }

    // ------------------------------------------------------------------
    // Changes made outside the canvas

    /// Records a change made by the shell (not the canvas): the canvas and
    /// storage hear about it, and the shown page stays valid.
    pub(super) fn changed_outside_canvas(&mut self, changes: &ChangeSet) {
        if changes.is_empty() {
            return;
        }
        if let Some(doc) = self.backend.document() {
            self.canvas.document_changed(doc, changes);
        }
        self.backend.record(changes);
        self.reports_heard(changes);
        self.fix_current_page();
    }

    /// Jumps to a place: opens its page, then selects and centres its
    /// symbol. `record` puts where we were in the back history.
    pub(super) fn jump(&mut self, to: Place, record: bool) {
        if record && let Some(here) = self.here() {
            self.wb.nav.history.record(here);
        }
        self.show_page(to.page);
        self.wb.pending = Some(to);
    }

    /// After the canvas showed its page: selects and centres a pending
    /// jump target, and lights up what the selection is linked to.
    pub(super) fn after_canvas_frame(&mut self) {
        if let Some(place) = self.wb.pending
            && Some(place.page) == self.current_page
        {
            self.wb.pending = None;
            if let (Some(symbol), Some(view)) = (place.symbol, self.canvas.schematic()) {
                let item = item_id(AnyId::Symbol(symbol));
                view.canvas.selection.clear();
                view.canvas.selection.apply([item], SelectOp::Replace);
                view.canvas.reveal([item]);
            }
        }
        if let Some((page, id)) = self.wb.pending_item
            && Some(page) == self.current_page
        {
            self.wb.pending_item = None;
            if let Some(view) = self.canvas.schematic() {
                let item = item_id(id);
                view.canvas.selection.clear();
                view.canvas.selection.apply([item], SelectOp::Replace);
                view.canvas.reveal([item]);
            }
        }
        if let Some((_, ids)) = self
            .wb
            .pending_items
            .take_if(|(p, _)| Some(*p) == self.current_page)
            && let Some(view) = self.canvas.schematic()
        {
            let items: Vec<_> = ids.iter().map(|&id| item_id(id)).collect();
            view.canvas.selection.clear();
            view.canvas
                .selection
                .apply(items.iter().copied(), SelectOp::Replace);
            view.canvas.reveal(items);
        }
        self.highlight_related();
    }

    /// Fills the canvas marks for the shown page before it draws: terminal
    /// proposals and problems. Shift+P hides them (the counters stay).
    pub(super) fn update_marks(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let mut list = super::terminals::proposal_marks(&self.page_proposals(), self.wb.proposal);
        list.extend(self.problem_marks(page));
        if self.wb.override_marks {
            list.extend(self.pe_override_marks(page));
        }
        list.extend(self.changes_marks(page));
        list.extend(self.commissioning_marks(page));
        let hidden = !self.wb.marks;
        // Cross references are printed drawing content: they go into the
        // page scene, like the tags, so the PDF shows them the same way.
        let notes = self.xref_notes(page);
        if let Some(view) = self.canvas.schematic() {
            view.canvas.marks.list = list;
            view.canvas.marks.hidden = hidden;
            view.set_notes(notes);
        }
    }

    /// Space held and dragged more than 4 px pans the drawing with the
    /// pointer (DESIGN.md §4); a tap opens the field on release.
    pub(super) fn space_pan(&mut self, ctx: &egui::Context) {
        let Some(down) = self.wb.space_down else {
            self.wb.space_pan = None;
            return;
        };
        let Some(pointer) = ctx.input(|i| i.pointer.latest_pos()) else {
            return;
        };
        let from = match self.wb.space_pan {
            Some(from) => from,
            None if pointer.distance(down) > 4.0 => down,
            None => return,
        };
        let delta = pointer - from;
        let by = novo_canvas::Vec2::new(delta.x, delta.y);
        match self.workspace() {
            Workspace::Cabinet => self.cabinet.view.viewport.pan_by_screen(by),
            Workspace::Reports | Workspace::View3d => {}
            _ => {
                if let Some(view) = self.canvas.schematic() {
                    view.canvas.viewport.pan_by_screen(by);
                }
            }
        }
        self.wb.space_pan = Some(pointer);
        ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        ctx.request_repaint();
    }

    /// Opens the field for a tapped Space once it is released.
    pub(super) fn space_released(&mut self, ctx: &egui::Context) {
        let Some(down) = self.wb.space_down else {
            return;
        };
        let (released, pointer) = ctx.input(|i| {
            let up = i.events.iter().any(|e| {
                matches!(
                    e,
                    egui::Event::Key {
                        key: egui::Key::Space,
                        pressed: false,
                        ..
                    }
                )
            });
            (up, i.pointer.latest_pos())
        });
        // A Space let go while another window had the keyboard sends no
        // release; it is no longer down either way.
        let held = ctx.input(|i| i.key_down(egui::Key::Space));
        if !released && held {
            return;
        }
        self.wb.space_down = None;
        let panned = self.wb.space_pan.take().is_some();
        let moved = pointer.map_or(0.0, |p| p.distance(down));
        if released && !panned && moved <= 4.0 {
            // At the cabinet a tap of Space ticks the wire in front of you.
            if !self.commissioning_space() {
                self.wb.field.open(Mode::All, pointer.or(Some(down)));
            }
        }
    }
}

/// What the command field searches, from the app's parts.
fn sources<'a>(
    app: &'a NovoApp,
    lessons: &'a [LessonEntry],
    macros: &'a [MacroEntry],
) -> Sources<'a> {
    Sources {
        lessons,
        macros,
        project: app.backend.document().map(Document::project),
        library: app.library(),
        keymap: &app.wb.keymap,
        context: app.command_context(),
    }
}
