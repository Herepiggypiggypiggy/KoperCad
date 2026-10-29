//! The pieces of the top bar that are buttons and pills of their own
//! (docs/design/visual-redesign.md section 3.1): undo and redo, and the page
//! switcher behind the breadcrumb. The menus are in `menu_bar.rs`, the
//! workspace segment in `workspace.rs`.

use eframe::egui;
use novo_theme::icons;

use super::NovoApp;
use crate::actions::Action;

/// What the page switcher remembers between frames.
#[derive(Default)]
pub(super) struct PageSwitcher {
    pub(super) filter: String,
    /// The popup was open last frame.
    pub(super) was_open: bool,
    fresh: bool,
}

/// `text` cut to `chars` characters with an ellipsis.
pub(super) fn cut(text: &str, chars: usize) -> String {
    if text.chars().count() <= chars {
        return text.to_owned();
    }
    let kept: String = text.chars().take(chars.saturating_sub(1)).collect();
    format!("{kept}…")
}

/// Widest the switcher's list is, and the most rows it shows before it scrolls.
const SWITCHER_WIDTH: f32 = 360.0;
const SWITCHER_ROWS: f32 = 10.0;

impl NovoApp {
    /// Undo and redo as icon buttons beside the mark. They are grey while
    /// there is nothing to undo or redo; the tooltip names the key.
    pub(super) fn undo_redo_buttons(&mut self, ui: &mut egui::Ui, picked: &mut Vec<Action>) {
        if !self.is_open() {
            return;
        }
        for (action, icon, name) in [
            (Action::Undo, icons::ARROW_U_UP_LEFT, "Undo"),
            (Action::Redo, icons::ARROW_U_UP_RIGHT, "Redo"),
        ] {
            let enabled = self.is_enabled(action);
            let response = ui.add_enabled(
                enabled,
                egui::Button::new(egui::RichText::new(icon).size(16.0)).frame(false),
            );
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
            let keys = action
                .shortcut()
                .map(|s| ui.ctx().format_shortcut(&s))
                .unwrap_or_default();
            let response = response.on_hover_text(format!("{name} ({keys})"));
            if response.clicked() {
                picked.push(action);
            }
        }
    }

    /// The caret after the page in the breadcrumb: a switcher with a search
    /// field, the pages of the project and an "Add page" row. Picking a page
    /// shows it; the breadcrumb's page name still opens the outline.
    pub(super) fn page_switcher_button(&mut self, ui: &mut egui::Ui) {
        let pal = crate::theme::ui_palette();
        let caret = ui.add(
            egui::Button::new(egui::RichText::new(icons::CARET_DOWN).color(pal.text_secondary))
                .frame(false),
        );
        caret.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Switch page")
        });
        self.anchor_response("page.switcher", &caret, "Page switcher");
        let id = egui::Popup::default_response_id(&caret);
        let open = egui::Popup::is_id_open(ui.ctx(), id);
        if open && !self.wb.page_switcher.was_open {
            self.wb.page_switcher.filter.clear();
            self.wb.page_switcher.fresh = true;
        }
        self.wb.page_switcher.was_open = open;
        let caret = caret.on_hover_text("Switch page");
        egui::Popup::menu(&caret)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| self.page_switcher_ui(ui));
    }

    fn page_switcher_ui(&mut self, ui: &mut egui::Ui) {
        let pal = crate::theme::ui_palette();
        ui.set_width(SWITCHER_WIDTH);
        let search = ui.add(
            egui::TextEdit::singleline(&mut self.wb.page_switcher.filter)
                .hint_text(format!("{} Find a page", icons::MAGNIFYING_GLASS))
                .desired_width(f32::INFINITY),
        );
        if std::mem::take(&mut self.wb.page_switcher.fresh) {
            search.request_focus();
        }
        let words: Vec<String> = self
            .wb
            .page_switcher
            .filter
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let rows: Vec<_> = self
            .page_rows()
            .into_iter()
            .filter(|r| {
                let label = r.label.to_lowercase();
                words.iter().all(|w| label.contains(w.as_str()))
            })
            .collect();
        let current = self.current_page;
        let sheet = self.wb.sheet;
        let mut pick = None;
        // Enter in the field opens the first match.
        let enter = ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Enter));
        if enter && let Some(first) = rows.first() {
            pick = Some((first.page, first.sheet));
        }
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .max_height(SWITCHER_ROWS * 28.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                if rows.is_empty() {
                    ui.label(egui::RichText::new("No page matches").color(pal.text_secondary));
                }
                for row in &rows {
                    let here = current == Some(row.page) && sheet == row.sheet;
                    let (rect, response) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), 28.0),
                        egui::Sense::click(),
                    );
                    response.widget_info(|| {
                        egui::WidgetInfo::selected(
                            egui::WidgetType::Button,
                            true,
                            here,
                            row.label.as_str(),
                        )
                    });
                    if here {
                        ui.painter().rect_filled(rect, 4, pal.accent_wash);
                        let bar =
                            egui::Rect::from_min_size(rect.min, egui::vec2(2.0, rect.height()));
                        ui.painter().rect_filled(bar, 1, pal.accent);
                    } else if response.hovered() {
                        ui.painter().rect_filled(rect, 4, pal.hover);
                    }
                    ui.painter().text(
                        egui::pos2(rect.left() + 10.0, rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        cut(&row.label, 44),
                        egui::TextStyle::Body.resolve(ui.style()),
                        if here {
                            pal.text_strong
                        } else {
                            pal.text_primary
                        },
                    );
                    if response.clicked() {
                        pick = Some((row.page, row.sheet));
                    }
                }
            });
        ui.separator();
        if ui
            .button(format!("{} Add page", icons::PLUS))
            .on_hover_text("Add a page after this one")
            .clicked()
        {
            self.wb.queued.push("page.new");
            ui.close();
        }
        if let Some((page, sheet)) = pick {
            self.show_sheet(page, sheet);
            ui.close();
        }
    }
}
