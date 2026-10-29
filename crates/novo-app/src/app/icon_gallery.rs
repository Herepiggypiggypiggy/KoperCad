//! The icon gallery (`view.icon_gallery`, Ctrl K "Icon gallery"): the drawn
//! icon set at every size it is used at, and the table that gives each command
//! its icon, short label and home.
//!
//! This is the first visible use of the visual redesign's foundations (lane
//! V0, docs/design/visual-redesign.md section 14): the icon engine in
//! `novo-theme`, the command presentation table and the home invariant of
//! `novo-workflow`. The rails, menus and palettes of lanes V2 to V5 read the
//! same tables; this window is where a person (and a reviewer) checks the
//! pictures, at 16 and 20 points first, in both themes.
//!
//! Where the user finds it: Ctrl K, "Icon gallery".

use eframe::egui;
use novo_theme::drawn::{self, Group, Icon};
use novo_theme::presentation;
use novo_workflow::commands::COMMANDS;
use novo_workflow::homes::{self, Home};

use super::NovoApp;

/// Sizes the gallery shows each icon at, in points. 12 and 14 are left out
/// on purpose: the finest details only read from 16 (design doc section 12).
const SIZES: [f32; 4] = [16.0, 20.0, 24.0, 32.0];

/// Outer width of one icon cell: 150 inside, 8 padding and 1 line each side.
const CELL: f32 = 150.0 + 2.0 * 9.0;

const TABS: [&str; 2] = ["Drawn icons", "Commands"];

/// State of the open window.
#[derive(Default)]
pub(crate) struct IconGallery {
    pub open: bool,
    tab: usize,
    filter: String,
}

impl IconGallery {
    #[cfg(test)]
    pub(super) fn set_tab(&mut self, tab: usize) {
        self.tab = tab;
    }
}

/// The group's heading in the window.
fn group_name(group: Group) -> &'static str {
    match group {
        Group::Workspace => "Workspaces",
        Group::Component => "Components",
        Group::Wiring => "Wiring and data",
        Group::Cabinet => "Cabinet",
        Group::Production => "Production",
        Group::Workflow => "Workflow",
        Group::Analysis => "Analysis",
        Group::Addon => "Add-ons",
    }
}

/// Where a command is found, in a few words for the table.
pub(super) fn home_text(id: &str) -> String {
    let homes = homes::homes(id);
    let mut parts: Vec<String> = homes
        .iter()
        .map(|h| match h {
            Home::Menu(p) => match p.submenu {
                Some(sub) => format!("{} > {}", p.menu, sub),
                None => p.menu.to_owned(),
            },
            other => other.kind().label().to_owned(),
        })
        .collect();
    if parts.is_empty() {
        parts.push("no home".to_owned());
    }
    parts.join(", ")
}

/// Commands that match the filter, by id, label, short label or home.
pub(super) fn matching(filter: &str) -> Vec<&'static novo_workflow::commands::CommandInfo> {
    let needle = filter.trim().to_lowercase();
    COMMANDS
        .iter()
        .filter(|c| {
            needle.is_empty() || {
                let short = presentation::short_label(c.id).unwrap_or("");
                c.id.to_lowercase().contains(&needle)
                    || c.label.to_lowercase().contains(&needle)
                    || short.to_lowercase().contains(&needle)
                    || home_text(c.id).to_lowercase().contains(&needle)
            }
        })
        .collect()
}

impl NovoApp {
    /// `view.icon_gallery`: opens the window, or closes it when it is open.
    pub(super) fn toggle_icon_gallery(&mut self) {
        self.wb.icon_gallery.open = !self.wb.icon_gallery.open;
    }

    pub(super) fn icon_gallery_window(&mut self, ctx: &egui::Context) {
        if !self.wb.icon_gallery.open {
            return;
        }
        let mut open = true;
        // Opens in the middle of the window, never wider or taller than it.
        let screen = ctx.content_rect();
        let size = egui::vec2(
            (screen.width() - 80.0).clamp(420.0, 880.0),
            (screen.height() - 120.0).clamp(300.0, 700.0),
        );
        egui::Window::new("Icon gallery")
            .open(&mut open)
            .resizable(true)
            .default_size(size)
            .default_pos(screen.center() - size / 2.0 + egui::vec2(0.0, 20.0))
            .min_size([420.0, 300.0])
            .show(ctx, |ui| {
                let state = &mut self.wb.icon_gallery;
                novo_theme::widgets::tabs(ui, &mut state.tab, &TABS);
                ui.add_space(6.0);
                if state.tab == 0 {
                    drawn_tab(ui);
                } else {
                    commands_tab(ui, &mut state.filter);
                }
            });
        if !open {
            self.wb.icon_gallery.open = false;
        }
    }
}

fn drawn_tab(ui: &mut egui::Ui) {
    let pal = crate::theme::ui_palette();
    ui.weak(format!(
        "{} icons drawn on a 24 grid, stroke 1.5, one colour. Phosphor stays for generic actions.",
        drawn::ALL.len()
    ));
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            // How they sit in a rail or a menu row: every icon at the two sizes
            // the redesign uses most, side by side, to judge legibility.
            ui.add_space(8.0);
            novo_theme::widgets::section_label(ui, "All at 16 and 20 pt");
            ui.add_space(4.0);
            for size in [16.0, 20.0] {
                strip(ui, size, pal);
                ui.add_space(6.0);
            }
            for group in Group::ALL {
                ui.add_space(8.0);
                novo_theme::widgets::section_label(ui, group_name(group));
                ui.add_space(2.0);
                // Rows by hand: a wrapped layout does not wrap frames.
                let gap = 10.0;
                let columns =
                    (((ui.available_width() + gap) / (CELL + gap)).floor() as usize).max(1);
                let icons: Vec<&'static Icon> = Icon::of_group(group).collect();
                for row in icons.chunks(columns) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
                        for icon in row {
                            icon_cell(ui, icon, pal);
                        }
                    });
                    ui.add_space(gap - 4.0);
                }
            }
            let addon = drawn::registered();
            if !addon.is_empty() {
                ui.add_space(8.0);
                novo_theme::widgets::section_label(ui, group_name(Group::Addon));
                ui.add_space(2.0);
                for row in addon.chunks(columns_for(ui)) {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
                        for icon in row {
                            icon_cell(ui, icon, pal);
                        }
                    });
                }
            }
            ui.add_space(8.0);
        });
}

/// How many icon cells fit in the width of `ui`.
fn columns_for(ui: &egui::Ui) -> usize {
    (((ui.available_width() + 10.0) / (CELL + 10.0)).floor() as usize).max(1)
}

/// Every icon at one size, in rows that fill the width.
fn strip(ui: &mut egui::Ui, size: f32, pal: &novo_theme::Palette) {
    let gap = 10.0;
    let per_row = (((ui.available_width() + gap) / (size + gap)).floor() as usize).max(1);
    for row in drawn::ALL.chunks(per_row) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(gap, gap);
            for icon in row {
                drawn::show(ui, icon, size, pal.text_primary);
            }
        });
    }
}

/// One icon at every size, over its name.
fn icon_cell(ui: &mut egui::Ui, icon: &'static Icon, pal: &novo_theme::Palette) {
    let frame = egui::Frame::new()
        .fill(pal.sunken)
        .stroke(egui::Stroke::new(1.0, pal.border_subtle))
        .corner_radius(novo_theme::radius::SURFACE)
        .inner_margin(egui::Margin::same(8));
    frame.show(ui, |ui| {
        ui.vertical(|ui| {
            ui.set_width(150.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                for size in SIZES {
                    ui.vertical(|ui| {
                        // Same baseline for every size.
                        ui.set_height(SIZES[SIZES.len() - 1]);
                        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
                            drawn::show(ui, icon, size, pal.text_primary);
                        });
                    });
                }
            });
            ui.add_space(2.0);
            ui.label(egui::RichText::new(icon.title).color(pal.text_primary));
            ui.label(
                egui::RichText::new(icon.name)
                    .small()
                    .color(pal.text_tertiary),
            );
        });
    });
}

/// One row of the commands table: a command of the registry, or one an
/// add-on registered at run time.
pub(super) struct Row {
    pub(super) id: &'static str,
    /// The long label (the tooltip); the short label for an add-on command.
    pub(super) label: &'static str,
    pub(super) addon: bool,
}

/// The commands that match: the registry's, then those add-ons registered.
pub(super) fn rows_for(filter: &str) -> Vec<Row> {
    let needle = filter.trim().to_lowercase();
    let mut rows: Vec<Row> = matching(filter)
        .into_iter()
        .map(|c| Row {
            id: c.id,
            label: c.label,
            addon: false,
        })
        .collect();
    rows.extend(
        presentation::registered()
            .into_iter()
            .filter(|r| {
                needle.is_empty()
                    || r.id.to_lowercase().contains(&needle)
                    || r.short.to_lowercase().contains(&needle)
                    || "add-on".contains(&needle)
            })
            .map(|r| Row {
                id: r.id,
                label: r.short,
                addon: true,
            }),
    );
    rows
}

fn commands_tab(ui: &mut egui::Ui, filter: &mut String) {
    let pal = crate::theme::ui_palette();
    ui.add(
        egui::TextEdit::singleline(filter)
            .hint_text("Filter by id, label, short label or home")
            .desired_width(ui.available_width()),
    );
    let rows = rows_for(filter);
    let drawn_rows = rows
        .iter()
        .filter(|r| presentation::glyph(r.id).is_drawn())
        .count();
    let addons = rows.iter().filter(|r| r.addon).count();
    ui.weak(format!(
        "{} of {} commands (and {} from add-ons), {} with a drawn icon. Short labels are at most {} characters; the long label is the tooltip.",
        rows.len() - addons,
        COMMANDS.len(),
        addons,
        drawn_rows,
        presentation::MAX_SHORT
    ));
    ui.add_space(4.0);
    let row_height = 24.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for r in &rows[range] {
                ui.horizontal(|ui| {
                    ui.set_height(row_height);
                    presentation::glyph(r.id).show(ui, 18.0, pal.text_primary);
                    column(ui, 190.0, row_height, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(
                                    presentation::short_label(r.id).unwrap_or("(no row)"),
                                )
                                .color(pal.text_primary),
                            )
                            .truncate(),
                        )
                        .on_hover_text(r.label);
                    });
                    column(ui, 190.0, row_height, |ui| {
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(r.id).small().color(pal.text_tertiary),
                            )
                            .truncate(),
                        );
                    });
                    let home = if r.addon {
                        "add-on".to_owned()
                    } else {
                        home_text(r.id)
                    };
                    ui.label(egui::RichText::new(home).small().color(pal.text_secondary));
                });
            }
        });
}

/// A left-aligned cell of fixed width, so the columns line up.
fn column(ui: &mut egui::Ui, width: f32, height: f32, add: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, height),
        egui::Layout::left_to_right(egui::Align::Center),
        add,
    );
}
