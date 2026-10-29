//! The Export centre (lane V4, `docs/design/visual-redesign.md` section 6):
//! one window for every format the program writes. The formats are tiles with
//! their icon and a sentence, grouped by what they are for; picking one shows
//! what it writes and the one button that writes it. File > Export opens it,
//! and so does the command field.
//!
//! Where the user finds it: File > Export..., or Ctrl+K and "export". Every
//! export command still runs from the command field by its own name.

use eframe::egui;
use novo_theme::{icons, presentation, widgets};
use novo_workflow::centres::{self, EXPORT_SECTIONS, Section};
use novo_workflow::commands::{self, CommandInfo};

use super::NovoApp;

const TILE_HEIGHT: f32 = 64.0;
const TILE_MIN_WIDTH: f32 = 200.0;
const DETAIL_WIDTH: f32 = 280.0;
const FOOTER: f32 = 44.0;

/// What the centre remembers while it is open.
#[derive(Debug, Default, Clone)]
pub(super) struct ExportCentre {
    pub query: String,
    pub selected: Option<&'static str>,
    /// The search field takes the keyboard on the first frame and again after
    /// a tile is picked, so typing keeps narrowing the list.
    shown: bool,
    last_selected: Option<&'static str>,
}

/// The tiles that match `query`, group by group; a group with none is left
/// out. The words are matched in the label, the short label, the sentence,
/// the keywords and the group name.
pub(super) fn visible(query: &str) -> Vec<(&'static Section, Vec<&'static CommandInfo>)> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    EXPORT_SECTIONS
        .iter()
        .filter_map(|section| {
            let rows: Vec<&CommandInfo> = section
                .ids
                .iter()
                .filter_map(|id| commands::get(id))
                .filter(|info| {
                    let hay = format!(
                        "{} {} {} {} {}",
                        info.label,
                        presentation::short_label(info.id).unwrap_or(""),
                        centres::describe(info.id),
                        info.keywords.join(" "),
                        section.name
                    )
                    .to_lowercase();
                    words.iter().all(|w| hay.contains(w.as_str()))
                })
                .collect();
            (!rows.is_empty()).then_some((section, rows))
        })
        .collect()
}

impl NovoApp {
    /// Opens the centre with nothing picked.
    pub(super) fn open_export_centre(&mut self) {
        self.wb.export_centre = Some(ExportCentre::default());
    }

    pub(super) fn export_centre_window(&mut self, ctx: &egui::Context) {
        let Some(mut state) = self.wb.export_centre.take() else {
            return;
        };
        let mut run: Option<&'static str> = None;
        let mut close = false;
        let modal = super::panel::modal_fill(
            ctx,
            "export_centre",
            "Export",
            [novo_theme::dialog_width::CENTRE, 640.0],
            |ui| {
                self.export_centre_body(ui, &mut state, &mut run, &mut close);
            },
        );
        if let Some(id) = run {
            self.settings.remember_action(id);
            self.wb.queued.push(id);
        } else if !close && !modal.should_close() {
            self.wb.export_centre = Some(state);
        }
    }

    fn export_centre_body(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut ExportCentre,
        run: &mut Option<&'static str>,
        close: &mut bool,
    ) {
        let search = ui.add(
            egui::TextEdit::singleline(&mut state.query)
                .hint_text("Search formats: pdf, dxf, excel, bench, plc")
                .desired_width(f32::INFINITY),
        );
        if !state.shown {
            search.request_focus();
            state.shown = true;
        }
        if state.selected != state.last_selected {
            state.last_selected = state.selected;
            search.request_focus();
        }
        if search.changed()
            && let Some(sel) = state.selected
            && !visible(&state.query)
                .iter()
                .any(|(_, rows)| rows.iter().any(|r| r.id == sel))
        {
            state.selected = None;
        }
        ui.add_space(6.0);
        let height = (ui.available_height() - FOOTER).max(120.0);
        let width = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(width, height),
            egui::Layout::left_to_right(egui::Align::Min),
            |ui| {
                let list_width = (width - DETAIL_WIDTH - 12.0).max(260.0);
                ui.allocate_ui_with_layout(
                    egui::vec2(list_width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("export_centre_tiles")
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.export_tiles(ui, state, run));
                    },
                );
                ui.separator();
                ui.allocate_ui_with_layout(
                    egui::vec2(DETAIL_WIDTH, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.export_detail(ui, state),
                );
            },
        );
        ui.add_space(6.0);
        let selected_ok = state
            .selected
            .and_then(commands::get)
            .is_some_and(|info| self.command_unavailable(info).is_none());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let go = ui
                .add_enabled_ui(selected_ok, |ui| widgets::primary_button(ui, "Export"))
                .inner;
            if go.clicked()
                || (selected_ok
                    && !search.has_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                || (selected_ok
                    && search.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            {
                *run = state.selected;
            }
            if widgets::ghost_button(ui, "Close").clicked() {
                *close = true;
            }
        });
    }

    fn export_tiles(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut ExportCentre,
        run: &mut Option<&'static str>,
    ) {
        let pal = crate::theme::ui_palette();
        let groups = visible(&state.query);
        if groups.is_empty() {
            ui.add_space(24.0);
            ui.label(
                egui::RichText::new("No format matches. Try pdf, dxf, excel, bench or plc.")
                    .color(pal.text_secondary),
            );
            return;
        }
        let gap = 6.0;
        let width = ui.available_width() - 8.0;
        let columns = ((width + gap) / (TILE_MIN_WIDTH + gap)).floor().max(1.0) as usize;
        let tile_width = (width - gap * (columns as f32 - 1.0)) / columns as f32;
        for (section, rows) in groups {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(section.name).strong());
            ui.label(
                egui::RichText::new(section.about)
                    .small()
                    .color(pal.text_secondary),
            );
            ui.add_space(4.0);
            for line in rows.chunks(columns) {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = gap;
                    for info in line {
                        self.export_tile(ui, state, run, info, tile_width);
                    }
                });
                ui.add_space(gap);
            }
        }
    }

    fn export_tile(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut ExportCentre,
        run: &mut Option<&'static str>,
        info: &'static CommandInfo,
        width: f32,
    ) {
        let pal = crate::theme::ui_palette();
        let reason = self.command_unavailable(info);
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(width, TILE_HEIGHT), egui::Sense::click());
        let short = presentation::short_label(info.id).unwrap_or(info.label);
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, reason.is_none(), short)
        });
        let picked = state.selected == Some(info.id);
        let fill = if picked {
            pal.accent_wash
        } else if response.hovered() {
            pal.hover
        } else {
            pal.control
        };
        let stroke = if picked {
            egui::Stroke::new(1.5, pal.accent)
        } else {
            egui::Stroke::new(1.0, pal.border_subtle)
        };
        ui.painter()
            .rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
        let (ink, soft) = if reason.is_none() {
            (pal.text_primary, pal.text_secondary)
        } else {
            (pal.text_tertiary, pal.text_tertiary)
        };
        presentation::glyph(info.id).paint(
            ui.painter(),
            egui::Rect::from_center_size(
                egui::pos2(rect.left() + 22.0, rect.center().y),
                egui::Vec2::splat(20.0),
            ),
            soft,
        );
        let mut title = egui::text::LayoutJob::simple(
            short.to_owned(),
            egui::TextStyle::Body.resolve(ui.style()),
            ink,
            (rect.width() - 52.0).max(20.0),
        );
        title.wrap.max_rows = 1;
        title.wrap.break_anywhere = true;
        let title = ui.painter().layout_job(title);
        ui.painter()
            .galley(egui::pos2(rect.left() + 44.0, rect.top() + 9.0), title, ink);
        let mut job = egui::text::LayoutJob::simple(
            centres::describe(info.id).to_owned(),
            egui::TextStyle::Small.resolve(ui.style()),
            soft,
            (rect.width() - 52.0).max(20.0),
        );
        job.wrap.max_rows = 2;
        let galley = ui.painter().layout_job(job);
        ui.painter().galley(
            egui::pos2(rect.left() + 44.0, rect.top() + 33.0),
            galley,
            soft,
        );
        self.anchor(&format!("cmd.{}", info.id), rect, info.label);
        let response = response.on_hover_text(info.label);
        if response.clicked() {
            state.selected = Some(info.id);
            self.learn_event(novo_learn::Event::Clicked(format!("cmd.{}", info.id)));
        }
        if response.double_clicked() && reason.is_none() {
            *run = Some(info.id);
        }
    }

    fn export_detail(&mut self, ui: &mut egui::Ui, state: &ExportCentre) {
        let pal = crate::theme::ui_palette();
        ui.add_space(4.0);
        let Some(info) = state.selected.and_then(commands::get) else {
            ui.add_space(24.0);
            ui.label(
                egui::RichText::new("Choose a format on the left to see what it writes.")
                    .color(pal.text_secondary),
            );
            return;
        };
        ui.horizontal(|ui| {
            presentation::glyph(info.id).show(ui, 28.0, pal.accent);
            ui.label(
                egui::RichText::new(presentation::short_label(info.id).unwrap_or(info.label))
                    .heading(),
            );
        });
        ui.add_space(6.0);
        if presentation::short_label(info.id).is_some_and(|short| short != info.label) {
            ui.label(egui::RichText::new(info.label).color(pal.text_primary));
            ui.add_space(4.0);
        }
        ui.label(egui::RichText::new(centres::describe(info.id)).color(pal.text_secondary));
        if let Some(hint) = super::menu_bar::hint_for(info.id) {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(hint).small().color(pal.text_secondary));
        }
        if let Some(keys) = self.command_shortcut_text(ui.ctx(), info.id) {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(format!("{} {keys}", icons::KEYBOARD))
                    .small()
                    .color(pal.text_tertiary),
            );
        }
        if let Some(reason) = self.command_unavailable(info) {
            ui.add_space(10.0);
            ui.label(egui::RichText::new(reason).color(pal.warning));
        }
        ui.add_space(10.0);
        ui.label(
            egui::RichText::new("Any export also runs from Ctrl+K by its name.")
                .small()
                .color(pal.text_tertiary),
        );
    }
}
