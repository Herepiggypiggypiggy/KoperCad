//! The outline's Pages section: a real tree of page groups, cabinets and
//! pages that is worked with the mouse and the keyboard.
//!
//! - Right click on any row, and on empty space, opens a full menu.
//! - F2 renames, Delete deletes (undo brings it back), Ctrl+Up/Down moves a
//!   page, the arrow keys walk the tree, Ctrl+D duplicates a page and Insert
//!   adds one.
//! - Pages drag onto a group or a cabinet to file them there, onto another
//!   page to take its place; a cabinet drags onto a group.
//! - New groups and cabinets name themselves from the pattern the project
//!   already uses (see [`tree::next_name`]); nothing has to be configured
//!   first.
//!
//! Under the tree, the Wires and Cables sections list every wire (by page)
//! and cable; a click opens the page and selects it ([`super::outline_items`]).
//!
//! What the tree shows and every edit are in [`super::outline_tree`], where
//! they are tested without a window.

use std::collections::BTreeSet;

use eframe::egui;
use novo_core::{AnyId, Aspect, Command, DeviceTag, MountingPlate, PageId, PageKind};
use novo_layout::sizing::EnclosureSize;
use novo_theme::{icons, tokens};

use super::outline_items as items;
use super::outline_tree::{
    self as tree, Act, CabinetSpec, DEFAULT_PLATE, Item, Key, Levels, Naming, RowKind, Target,
    TreeRow,
};
use super::pages::PageView;
use super::{Level, NovoApp};

const ROW_H: f32 = 24.0;
const INDENT: f32 = 14.0;

/// What a name box edits.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum RenameOf {
    Page(PageId),
    Group(Levels),
    Cabinet(Levels),
}

/// A name being typed in a row.
#[derive(Debug, Clone)]
pub(super) struct Rename {
    pub(super) of: RenameOf,
    pub(super) text: String,
}

/// What a drag in the tree carries.
#[derive(Debug, Clone, PartialEq)]
enum Drag {
    Page(PageId),
    /// A cabinet with the group it is listed under.
    Cabinet {
        location: Levels,
        group: Levels,
    },
}

/// What to select and show once the tree has been rebuilt after an edit.
#[derive(Debug, Clone)]
pub(super) enum Reveal {
    Page { page: PageId, rename: bool },
    Group(Levels),
    Cabinet(Levels),
}

/// The new cabinet window.
#[derive(Debug, Clone)]
pub(super) struct CabinetForm {
    name: String,
    group: Levels,
    /// Index into the user's enclosure list; `None` for the default plate.
    enclosure: Option<usize>,
    first_page: bool,
    focused: bool,
}

/// The properties window.
#[derive(Debug, Clone)]
pub(super) enum PropsForm {
    Page {
        page: PageId,
        number: String,
        title: String,
        function: Levels,
        location: Levels,
    },
    Level {
        aspect: Aspect,
        from: Levels,
        name: String,
        description: String,
        enclosure: Option<usize>,
        width: f64,
        height: f64,
        has_plate: bool,
    },
}

/// A question before something goes.
#[derive(Debug, Clone)]
pub(super) struct Confirm {
    title: String,
    body: String,
    yes: String,
    act: Act,
}

/// The tree's state, owned by the app.
pub(super) struct OutlineState {
    pub(super) collapsed: BTreeSet<String>,
    pub(super) selected: Option<String>,
    /// The tree took the last click, so its keys apply.
    pub(super) focused: bool,
    /// Where the tree was drawn last frame.
    pub(super) rect: Option<egui::Rect>,
    pub(super) rename: Option<Rename>,
    /// The rows last drawn, for the keys.
    pub(super) rows: Vec<TreeRow>,
    pub(super) cabinet_form: Option<CabinetForm>,
    pub(super) props: Option<PropsForm>,
    pub(super) confirm: Option<Confirm>,
    pub(super) reveal: Option<Reveal>,
    scroll_to: Option<String>,
    /// The offer to file pages by their device tags was declined.
    pub(super) filing_dismissed: bool,
    /// Where each row was drawn, for tests that click them.
    #[cfg(test)]
    pub(super) rects: std::collections::BTreeMap<String, egui::Rect>,
    /// The empty space under the rows, for tests.
    #[cfg(test)]
    pub(super) empty_space: Option<egui::Rect>,
}

impl OutlineState {
    pub(super) fn new() -> Self {
        Self {
            collapsed: BTreeSet::new(),
            selected: None,
            focused: false,
            rect: None,
            rename: None,
            rows: Vec::new(),
            cabinet_form: None,
            props: None,
            confirm: None,
            reveal: None,
            scroll_to: None,
            filing_dismissed: false,
            #[cfg(test)]
            rects: Default::default(),
            #[cfg(test)]
            empty_space: None,
        }
    }

    pub(super) fn dialog_open(&self) -> bool {
        self.cabinet_form.is_some() || self.props.is_some() || self.confirm.is_some()
    }

    fn row(&self, key: &str) -> Option<&TreeRow> {
        self.rows.iter().find(|r| r.key == key)
    }

    /// The row just before or after the selected one.
    fn neighbour(&self, down: bool) -> Option<&TreeRow> {
        let at = self
            .selected
            .as_ref()
            .and_then(|k| self.rows.iter().position(|r| &r.key == k));
        match (at, down) {
            (Some(i), true) => self.rows.get(i + 1),
            (Some(i), false) => i.checked_sub(1).and_then(|i| self.rows.get(i)),
            (None, true) => self.rows.first(),
            (None, false) => self.rows.last(),
        }
    }
}

/// The parents of a row key, nearest last: `g:G1/c:A1/p:3:0` is under
/// `g:G1` and `g:G1/c:A1`.
fn ancestors(key: &str) -> Vec<String> {
    let parts: Vec<&str> = key.split('/').collect();
    (1..parts.len()).map(|n| parts[..n].join("/")).collect()
}

fn group_name(levels: &[String]) -> String {
    DeviceTag {
        function: levels.to_vec(),
        ..Default::default()
    }
    .to_string()
}

fn cabinet_name(levels: &[String]) -> String {
    DeviceTag {
        location: levels.to_vec(),
        ..Default::default()
    }
    .to_string()
}

fn level_text(levels: &[String]) -> String {
    levels.join("")
}

impl NovoApp {
    /// The names new groups and cabinets start from.
    pub(super) fn naming(&self) -> Naming {
        Naming::from_prefixes(&self.settings.group_prefix, &self.settings.cabinet_prefix)
    }

    // ------------------------------------------------------------------
    // The section

    /// The Pages section of the outline: toolbar, filter, tree. True when a
    /// page was opened (the flyout closes).
    pub(super) fn outline_pages(&mut self, ui: &mut egui::Ui) -> bool {
        self.ol.rect = Some(ui.max_rect());
        self.anchor("outline.pages", ui.max_rect(), "Pages list");
        self.toolbar(ui);
        ui.horizontal(|ui| {
            let width = ui.available_width() - 36.0;
            ui.add(
                egui::TextEdit::singleline(&mut self.page_filter)
                    .hint_text("Filter pages")
                    .desired_width(width),
            );
            self.outline_more_menu(ui);
        });
        ui.add_space(4.0);
        self.filing_banner(ui);

        let page_rows = self.page_rows();
        self.resolve_reveal(&page_rows);
        let filter = self.page_filter.trim().to_owned();
        let tree_rows = match self.backend.document() {
            Some(doc) if filter.is_empty() => tree::build(
                doc.project(),
                &page_rows,
                self.page_view,
                &self.ol.collapsed,
            ),
            _ => Vec::new(),
        };
        let flat = match self.backend.document() {
            Some(doc) if !filter.is_empty() => tree::filtered(doc.project(), &page_rows, &filter),
            _ => Vec::new(),
        };
        self.ol.rows = tree_rows.clone();

        let mut acts: Vec<Act> = Vec::new();
        let mut opened = None;
        let mut select = None;
        let mut net_jump = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                self.auto_scroll(ui);
                if !filter.is_empty() {
                    if flat.is_empty() {
                        ui.weak("No page matches");
                    }
                    for (row, place) in &flat {
                        let r = TreeRow {
                            depth: 0,
                            key: tree::page_key(row.page, row.sheet),
                            kind: self.flat_kind(row),
                            foldable: false,
                        };
                        let out = self.tree_row(ui, &r, Some(place));
                        collect(out, &r, &mut acts, &mut opened, &mut select);
                    }
                    net_jump = self.net_sections(ui, &filter);
                    return;
                }
                for row in &tree_rows {
                    let out = self.tree_row(ui, row, None);
                    collect(out, row, &mut acts, &mut opened, &mut select);
                }
                net_jump = self.net_sections(ui, &filter);
                // The empty space under the rows is part of the tree: a
                // right click there makes something new.
                let rest = ui.available_height().max(48.0);
                let (rect, bg) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), rest),
                    egui::Sense::click(),
                );
                #[cfg(test)]
                {
                    self.ol.empty_space = Some(rect);
                }
                if tree_rows.len() < 4 {
                    ui.painter().text(
                        rect.center_top() + egui::vec2(0.0, 12.0),
                        egui::Align2::CENTER_TOP,
                        "Right click for New page, New page group or New cabinet",
                        egui::FontId::proportional(tokens::FONT_SMALL),
                        crate::theme::ui_palette().text_tertiary,
                    );
                }
                if bg.secondary_clicked() || bg.clicked() {
                    self.ol.focused = true;
                    if bg.clicked() {
                        self.ol.selected = None;
                    }
                }
                if let Some(doc) = self.backend.document() {
                    let items = tree::menu(doc.project(), &Target::Background);
                    bg.context_menu(|ui| menu_items(ui, &items, &mut acts));
                }
            });
        if let Some(key) = select {
            self.ol.selected = Some(key);
            self.ol.focused = true;
        }
        let mut closed = false;
        if let Some((page, sheet)) = opened {
            self.show_sheet(page, sheet);
            closed = true;
        }
        match net_jump {
            Some(NetJump::Wire(wire)) => {
                self.go_to_wire(wire);
                closed = true;
            }
            Some(NetJump::Cable(wires)) => {
                self.go_to_cable_wires(&wires);
                closed = true;
            }
            Some(NetJump::EmptyCable(tag)) => self.message(
                super::Level::Info,
                format!("{tag} has no core wired yet, so there is no wire to go to"),
            ),
            None => {}
        }
        for act in acts {
            self.perform(act);
        }
        closed
    }

    /// The Wires and Cables sections under the page tree: every wire by
    /// page and every cable, each a row that jumps to it (a wire opens its
    /// page and selects it, a cable selects all its wired cores). With a
    /// filter the sections open on what matches.
    fn net_sections(&self, ui: &mut egui::Ui, filter: &str) -> Option<NetJump> {
        let project = self.backend.document()?.project();
        let filtering = !filter.is_empty();
        let force = filtering.then_some(true);
        let mut jump = None;
        ui.add_space(6.0);
        let wires = items::wire_count(project);
        egui::CollapsingHeader::new(format!("Wires ({wires})"))
            .id_salt("outline_wires")
            .open(force)
            .show(ui, |ui| {
                let groups = items::wire_groups(project, filter);
                if groups.is_empty() {
                    ui.weak("No wire matches");
                }
                for group in groups {
                    egui::CollapsingHeader::new(format!(
                        "Page {} ({})",
                        group.page_name,
                        group.wires.len()
                    ))
                    .id_salt(("outline_wires_page", group.page))
                    .open(force)
                    .show(ui, |ui| {
                        for wire in &group.wires {
                            if ui
                                .selectable_label(false, &wire.text)
                                .on_hover_text("Open its page and select the wire")
                                .clicked()
                            {
                                jump = Some(NetJump::Wire(wire.id));
                            }
                        }
                    });
                }
            });
        let cables = project.cables().count();
        egui::CollapsingHeader::new(format!("Cables ({cables})"))
            .id_salt("outline_cables")
            .open(force)
            .show(ui, |ui| {
                let list = items::cable_items(project, filter);
                if list.is_empty() {
                    ui.weak("No cable matches");
                }
                for cable in list {
                    if ui
                        .selectable_label(false, &cable.text)
                        .on_hover_text("Select all wired cores of this cable")
                        .clicked()
                    {
                        jump = Some(if cable.wires.is_empty() {
                            NetJump::EmptyCable(cable.text.clone())
                        } else {
                            NetJump::Cable(cable.wires.clone())
                        });
                    }
                }
            });
        jump
    }

    /// "N pages are in no group or cabinet" with a button that files them by
    /// the tags of their devices. Quiet once there is nothing to propose.
    fn filing_banner(&mut self, ui: &mut egui::Ui) {
        if self.ol.filing_dismissed {
            return;
        }
        let Some(n) = self
            .backend
            .document()
            .map(|d| tree::suggest_filing(d.project()).len())
            .filter(|n| *n > 0)
        else {
            return;
        };
        let mut file = false;
        let mut dismiss = false;
        egui::Frame::new()
            .fill(crate::theme::ui_palette().raised)
            .stroke(egui::Stroke::new(
                1.0,
                crate::theme::ui_palette().border_subtle,
            ))
            .corner_radius(4.0)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new(format!(
                        "{n} {} in no group or cabinet. Their devices say where they belong.",
                        if n == 1 { "page is" } else { "pages are" }
                    ))
                    .size(tokens::FONT_SMALL + 1.0)
                    .color(crate::theme::ui_palette().text_secondary),
                );
                ui.horizontal(|ui| {
                    file = novo_theme::widgets::primary_button(ui, "File them").clicked();
                    dismiss = ui.small_button("Not now").clicked();
                });
            });
        ui.add_space(4.0);
        if dismiss {
            self.ol.filing_dismissed = true;
        }
        if file {
            self.perform(Act::AutoFile);
        }
    }

    fn flat_kind(&self, row: &crate::backend::PageRow) -> RowKind {
        let project = self.backend.document().map(|d| d.project());
        let page = project.and_then(|p| p.page(row.page));
        RowKind::Page {
            page: row.page,
            sheet: row.sheet,
            label: row.label.clone(),
            title: page.map(|p| p.title.clone()).unwrap_or_default(),
            number: page.map(|p| p.name.clone()).unwrap_or_default(),
            drawn: page.is_some_and(|p| !matches!(p.kind, PageKind::Generated { .. })),
        }
    }

    /// New, the view switch and the section's own menu.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let mut acts = Vec::new();
        ui.horizontal(|ui| {
            let structure = self.new_here();
            ui.menu_button(format!("{} New", icons::PLUS), |ui| {
                ui.set_min_width(190.0);
                let items = [
                    (
                        "Page",
                        Act::NewPage {
                            structure: structure.clone(),
                            after: None,
                        },
                    ),
                    (
                        "Page group",
                        Act::NewGroup {
                            in_cabinet: (!structure.location.is_empty())
                                .then(|| structure.location.clone()),
                        },
                    ),
                    (
                        "Cabinet…",
                        Act::NewCabinet {
                            in_group: (!structure.function.is_empty())
                                .then(|| structure.function.clone()),
                        },
                    ),
                ];
                for (label, act) in items {
                    if ui.button(label).clicked() {
                        acts.push(act);
                        ui.close();
                    }
                }
                ui.separator();
                if ui
                    .button("Several pages…")
                    .on_hover_text("Add numbered pages to groups and cabinets by name")
                    .clicked()
                {
                    let group = DeviceTag {
                        function: structure.function.clone(),
                        ..Default::default()
                    };
                    let cabinet = DeviceTag {
                        location: structure.location.clone(),
                        ..Default::default()
                    };
                    self.open_page_form(super::pages::PageForm::new(
                        &group.to_string(),
                        &cabinet.to_string(),
                    ));
                    ui.close();
                }
            })
            .response
            .on_hover_text("Page, page group or cabinet, filed next to what you have open");
            {
                let mut view = usize::from(self.page_view == PageView::Location);
                let labels = [PageView::Function.label(), PageView::Location.label()];
                let shown = self.page_view;
                let picked = ui.scope(|ui| novo_theme::widgets::segmented(ui, &mut view, &labels));
                picked.response.on_hover_text(shown.hover());
                if picked.inner {
                    self.page_view = if view == 0 {
                        PageView::Function
                    } else {
                        PageView::Location
                    };
                }
            }
        });
        for act in acts {
            self.perform(act);
        }
    }

    /// The "..." menu beside the filter: fold or unfold every group.
    fn outline_more_menu(&mut self, ui: &mut egui::Ui) {
        let mut acts = Vec::new();
        ui.menu_button(icons::DOTS_THREE, |ui| {
            ui.set_min_width(170.0);
            let items = vec![
                Item::Entry {
                    label: "Expand all".into(),
                    act: Act::FoldAll(false),
                    enabled: true,
                    keys: "",
                    why: "",
                },
                Item::Entry {
                    label: "Collapse all".into(),
                    act: Act::FoldAll(true),
                    enabled: true,
                    keys: "",
                    why: "",
                },
            ];
            menu_items(ui, &items, &mut acts);
        })
        .response
        .on_hover_text("More");
        for act in acts {
            self.perform(act);
        }
    }

    /// The group and cabinet a new page goes into by default: those of the
    /// selected row, else of the page on the canvas, so a new page lands next
    /// to what is being worked on.
    pub(super) fn new_here(&self) -> DeviceTag {
        if let Some(row) = self.ol.selected.as_deref().and_then(|k| self.ol.row(k)) {
            match &row.kind {
                RowKind::Group { context, .. } | RowKind::Cabinet { context, .. } => {
                    return context.clone();
                }
                RowKind::Page { page, .. } => {
                    if let Some(p) = self
                        .backend
                        .document()
                        .and_then(|d| d.project().page(*page))
                    {
                        return p.structure.clone();
                    }
                }
                _ => {}
            }
        }
        self.current_page
            .and_then(|id| self.backend.document().and_then(|d| d.project().page(id)))
            .map(|p| p.structure.clone())
            .unwrap_or_default()
    }

    /// Scrolls the list while a drag is held near its top or bottom edge.
    fn auto_scroll(&self, ui: &mut egui::Ui) {
        let ctx = ui.ctx();
        if !egui::DragAndDrop::has_any_payload(ctx) {
            return;
        }
        let Some(pos) = ctx.pointer_latest_pos() else {
            return;
        };
        let view = ui.clip_rect();
        if !view.x_range().contains(pos.x) {
            return;
        }
        let edge = 28.0;
        if pos.y < view.top() + edge {
            ui.scroll_with_delta(egui::vec2(0.0, 10.0));
        } else if pos.y > view.bottom() - edge {
            ui.scroll_with_delta(egui::vec2(0.0, -10.0));
        }
    }

    /// Finds the row an edit asked to show, opens the folds above it and
    /// selects it (and starts its name box).
    fn resolve_reveal(&mut self, page_rows: &[crate::backend::PageRow]) {
        let Some(reveal) = self.ol.reveal.take() else {
            return;
        };
        let Some(doc) = self.backend.document() else {
            return;
        };
        let open = tree::build(doc.project(), page_rows, self.page_view, &BTreeSet::new());
        let found = open.iter().find(|r| match (&reveal, &r.kind) {
            (
                Reveal::Page { page, .. },
                RowKind::Page {
                    page: p, sheet: 0, ..
                },
            ) => p == page,
            (Reveal::Group(g), RowKind::Group { function, .. }) => function == g,
            (Reveal::Cabinet(c), RowKind::Cabinet { location, .. }) => location == c,
            _ => false,
        });
        let Some(row) = found else {
            return;
        };
        for key in ancestors(&row.key) {
            self.ol.collapsed.remove(&key);
        }
        self.ol.selected = Some(row.key.clone());
        self.ol.scroll_to = Some(row.key.clone());
        self.ol.focused = true;
        if let Reveal::Page { page, rename: true } = reveal {
            self.ol.rename = Some(Rename {
                of: RenameOf::Page(page),
                text: doc
                    .project()
                    .page(page)
                    .map(|p| p.title.clone())
                    .unwrap_or_default(),
            });
        }
    }

    // ------------------------------------------------------------------
    // One row

    fn tree_row(&mut self, ui: &mut egui::Ui, row: &TreeRow, place: Option<&String>) -> RowOut {
        let mut out = RowOut::default();
        let (rect, resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ROW_H),
            egui::Sense::click_and_drag(),
        );
        #[cfg(test)]
        self.ol.rects.insert(row.key.clone(), rect);
        if !ui.is_rect_visible(rect) {
            return out;
        }
        if self.ol.scroll_to.as_deref() == Some(row.key.as_str()) {
            self.ol.scroll_to = None;
            ui.scroll_to_rect(rect, Some(egui::Align::Center));
        }
        let current = matches!(&row.kind, RowKind::Page { page, sheet, .. }
            if self.current_page == Some(*page) && self.wb.sheet == *sheet && self.workspace() != super::workspace::Workspace::Cabinet);
        let selected = self.ol.selected.as_deref() == Some(row.key.as_str());
        let x0 = rect.left() + 4.0 + row.depth as f32 * INDENT;

        // Background
        let painter = ui.painter_at(rect);
        let radius = 3.0;
        if current {
            painter.rect_filled(
                rect.shrink2(egui::vec2(2.0, 1.0)),
                radius,
                crate::theme::ui_palette().accent_wash,
            );
        } else if resp.hovered() || selected {
            painter.rect_filled(
                rect.shrink2(egui::vec2(2.0, 1.0)),
                radius,
                crate::theme::ui_palette().control,
            );
        }
        if selected {
            painter.rect_stroke(
                rect.shrink2(egui::vec2(2.0, 1.0)),
                radius,
                egui::Stroke::new(1.0, crate::theme::ui_palette().border_strong),
                egui::StrokeKind::Inside,
            );
        }

        // Fold arrow and icon
        let font = egui::FontId::proportional(tokens::FONT_BODY);
        let icon_font = egui::FontId::proportional(14.0);
        let mid = rect.center().y;
        let mut x = x0;
        let folded = self.ol.collapsed.contains(&row.key);
        if row.foldable {
            let arrow = if folded {
                icons::CARET_RIGHT
            } else {
                icons::CARET_DOWN
            };
            painter.text(
                egui::pos2(x + 6.0, mid),
                egui::Align2::CENTER_CENTER,
                arrow,
                egui::FontId::proportional(11.0),
                crate::theme::ui_palette().text_secondary,
            );
        }
        x += 14.0;
        let (icon, tint) = match &row.kind {
            RowKind::Group { .. } => (
                if folded {
                    icons::FOLDER_SIMPLE
                } else {
                    icons::FOLDER_OPEN
                },
                crate::theme::ui_palette().text_secondary,
            ),
            RowKind::Cabinet { enclosure, .. } => (
                if *enclosure {
                    icons::CUBE
                } else {
                    icons::MAP_PIN
                },
                crate::theme::ui_palette().accent,
            ),
            RowKind::Generated { .. } => (
                icons::LIST_NUMBERS,
                crate::theme::ui_palette().text_secondary,
            ),
            RowKind::Page { drawn: true, .. } => {
                (icons::FILE_TEXT, crate::theme::ui_palette().text_secondary)
            }
            RowKind::Page { .. } => (icons::FILE_DASHED, crate::theme::ui_palette().text_tertiary),
            RowKind::Empty => ("", crate::theme::ui_palette().text_tertiary),
        };
        if !icon.is_empty() {
            painter.text(
                egui::pos2(x + 7.0, mid),
                egui::Align2::CENTER_CENTER,
                icon,
                icon_font,
                tint,
            );
        }
        x += 20.0;

        // Rename box in place of the label
        let renaming = self
            .ol
            .rename
            .as_ref()
            .is_some_and(|r| match (&r.of, &row.kind) {
                (RenameOf::Page(p), RowKind::Page { page, sheet: 0, .. }) => p == page,
                (RenameOf::Group(g), RowKind::Group { function, .. }) => g == function,
                (RenameOf::Cabinet(c), RowKind::Cabinet { location, .. }) => c == location,
                _ => false,
            });
        if renaming {
            self.rename_box(
                ui,
                row,
                egui::Rect::from_min_max(
                    egui::pos2(x, rect.top() + 2.0),
                    egui::pos2(rect.right() - 4.0, rect.bottom() - 2.0),
                ),
            );
            return out;
        }

        // Label and the quiet text on the right
        let (text, weak, right): (String, String, String) = match &row.kind {
            RowKind::Group {
                function, pages, ..
            } => (group_name(function), String::new(), count_text(*pages)),
            RowKind::Cabinet {
                location, pages, ..
            } => (cabinet_name(location), String::new(), count_text(*pages)),
            RowKind::Generated { pages } => (
                "Contents and reports".into(),
                String::new(),
                count_text(*pages),
            ),
            RowKind::Page {
                label,
                title,
                number,
                drawn,
                ..
            } => {
                if *drawn {
                    (
                        number.clone(),
                        title.clone(),
                        place.cloned().unwrap_or_default(),
                    )
                } else {
                    (
                        label.clone(),
                        String::new(),
                        place.cloned().unwrap_or_default(),
                    )
                }
            }
            RowKind::Empty => ("No pages yet".into(), String::new(), String::new()),
        };
        let is_node = matches!(row.kind, RowKind::Group { .. } | RowKind::Cabinet { .. });
        let label_color = match &row.kind {
            RowKind::Empty => crate::theme::ui_palette().text_tertiary,
            RowKind::Page { drawn: false, .. } => crate::theme::ui_palette().text_secondary,
            _ => crate::theme::ui_palette().text_primary,
        };
        let locked = matches!(&row.kind, RowKind::Page { page, .. } if self
            .backend
            .document()
            .and_then(|d| d.project().page(*page))
            .is_some_and(|p| p.locked.is_some()));
        // Room for the padlock, so a long title ends before it.
        let right_w = if right.is_empty() { 0.0 } else { 56.0 } + if locked { 18.0 } else { 0.0 };
        let right_edge = rect.right() - 8.0;
        if !right.is_empty() {
            painter.text(
                egui::pos2(right_edge, mid),
                egui::Align2::RIGHT_CENTER,
                &right,
                egui::FontId::proportional(tokens::FONT_SMALL),
                crate::theme::ui_palette().text_tertiary,
            );
        }
        let clip = egui::Rect::from_min_max(
            rect.left_top(),
            egui::pos2(right_edge - right_w, rect.bottom()),
        );
        let painter = ui.painter_at(clip);
        let room = clip.right() - x;
        let ellipsis = novo_theme::widgets::paint_text_ellipsis;
        if is_node {
            ellipsis(
                &painter,
                egui::pos2(x, mid),
                &text,
                egui::FontId::new(tokens::FONT_BODY, novo_theme::fonts::strong_family()),
                label_color,
                room,
            );
        } else if matches!(row.kind, RowKind::Page { drawn: true, .. }) {
            let number = painter.text(
                egui::pos2(x, mid),
                egui::Align2::LEFT_CENTER,
                &text,
                egui::FontId::monospace(tokens::FONT_MONO),
                crate::theme::ui_palette().text_secondary,
            );
            let (title, color) = if weak.is_empty() {
                ("Untitled", crate::theme::ui_palette().text_tertiary)
            } else {
                (weak.as_str(), label_color)
            };
            let start = number.right() + 8.0;
            ellipsis(
                &painter,
                egui::pos2(start, mid),
                title,
                font.clone(),
                color,
                clip.right() - start,
            );
        } else {
            ellipsis(
                &painter,
                egui::pos2(x, mid),
                &text,
                font.clone(),
                label_color,
                room,
            );
        }
        if locked {
            ui.painter_at(rect).text(
                egui::pos2(rect.right() - 16.0, mid),
                egui::Align2::RIGHT_CENTER,
                novo_theme::icons::LOCK_SIMPLE,
                egui::FontId::proportional(tokens::FONT_SMALL),
                crate::theme::ui_palette().text_tertiary,
            );
        }
        if let RowKind::Page { page, .. } = &row.kind
            && self.page_has_changes(*page)
        {
            ui.painter_at(rect).circle_filled(
                egui::pos2(rect.right() - 4.0, mid),
                2.5,
                crate::theme::ui_palette().accent,
            );
        }

        // Drag source
        match &row.kind {
            RowKind::Page { page, .. } => resp.dnd_set_drag_payload(Drag::Page(*page)),
            RowKind::Cabinet {
                location, context, ..
            } if self.page_view == PageView::Function && !context.function.is_empty() => resp
                .dnd_set_drag_payload(Drag::Cabinet {
                    location: location.clone(),
                    group: context.function.clone(),
                }),
            _ => {}
        }

        // Clicks
        if resp.clicked() || resp.secondary_clicked() || resp.double_clicked() {
            out.select = true;
        }
        if resp.clicked() {
            match &row.kind {
                RowKind::Page { page, sheet, .. } => out.open = Some((*page, *sheet)),
                _ if row.foldable => out.acts.push(Act::Fold(row.key.clone(), !folded)),
                _ => {}
            }
        }
        if resp.double_clicked()
            && let RowKind::Page {
                page, drawn: true, ..
            } = &row.kind
        {
            out.acts.push(Act::RenamePage(*page));
        } else if resp.double_clicked() {
            match &row.kind {
                RowKind::Group { function, .. } => {
                    out.acts.push(Act::RenameGroup(function.clone()))
                }
                RowKind::Cabinet { location, .. } => {
                    out.acts.push(Act::RenameCabinet(location.clone()))
                }
                _ => {}
            }
        }

        // Drops
        self.row_drop(ui, &resp, row, &mut out);

        // Tooltip and menu
        if let Some(tip) = self.row_tip(row) {
            resp.clone().on_hover_text(tip);
        }
        if locked {
            let lock_rect = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 24.0, rect.center().y),
                egui::vec2(18.0, rect.height()),
            );
            let generated = matches!(&row.kind, RowKind::Page { page, .. } if self
                .backend
                .document()
                .and_then(|d| d.project().page(*page))
                .is_some_and(|p| matches!(p.kind, PageKind::Generated { .. })));
            let tip = if generated {
                "Locked with the issue. The content still regenerates from the drawings; the lock only stops edits to the page itself."
            } else {
                "Locked with the issue. Unlock it from the status bar to edit."
            };
            ui.interact(lock_rect, resp.id.with("lock"), egui::Sense::hover())
                .on_hover_text(tip);
        }
        if let Some(doc) = self.backend.document()
            && let Some(target) = target_of(row)
        {
            let items = tree::menu(doc.project(), &target);
            resp.context_menu(|ui| {
                ui.set_min_width(210.0);
                menu_items(ui, &items, &mut out.acts)
            });
        }
        out
    }

    fn row_tip(&self, row: &TreeRow) -> Option<String> {
        let project = self.backend.document()?.project();
        match &row.kind {
            RowKind::Page { page, .. } => {
                let p = project.page(*page)?;
                let place = p.structure.to_string();
                Some(if place.is_empty() {
                    format!("Page {} · in no group or cabinet", p.name)
                } else {
                    format!("Page {} · {place}", p.name)
                })
            }
            RowKind::Group { function, .. } => {
                let (pages, devices, _) = tree::level_usage(project, Aspect::Function, function);
                Some(format!("Page group · {pages} pages, {devices} devices"))
            }
            RowKind::Cabinet {
                location,
                enclosure,
                ..
            } => {
                let (pages, devices, _) = tree::level_usage(project, Aspect::Location, location);
                let plate = project
                    .structure_by_tag(&DeviceTag {
                        location: location.clone(),
                        ..Default::default()
                    })
                    .and_then(|s| s.plate)
                    .map(|p| format!(" · plate {} x {} mm", p.width, p.height))
                    .unwrap_or_default();
                Some(format!(
                    "{} · {pages} pages, {devices} devices{plate}",
                    if *enclosure { "Cabinet" } else { "Location" }
                ))
            }
            _ => None,
        }
    }

    /// The drop target half of a row: highlight while a drag is over it, and
    /// what a release does.
    fn row_drop(
        &mut self,
        ui: &mut egui::Ui,
        resp: &egui::Response,
        row: &TreeRow,
        out: &mut RowOut,
    ) {
        let Some(drag) = resp.dnd_hover_payload::<Drag>() else {
            return;
        };
        let drag = (*drag).clone();
        let stroke = ui.visuals().selection.stroke;
        let lower = ui
            .ctx()
            .pointer_latest_pos()
            .is_some_and(|p| p.y > resp.rect.center().y);
        let project = self.backend.document().map(|d| d.project());
        let plan =
            project.and_then(|project| drop_plan(project, &drag, row, lower, self.page_view));
        let Some(plan) = plan else {
            return;
        };
        match &plan {
            DropPlan::Reorder(_) => {
                let y = if lower {
                    resp.rect.bottom()
                } else {
                    resp.rect.top()
                };
                ui.painter().hline(resp.rect.x_range(), y, stroke);
            }
            _ => {
                ui.painter().rect_stroke(
                    resp.rect.shrink2(egui::vec2(1.0, 0.0)),
                    3.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
        }
        if resp.dnd_release_payload::<Drag>().is_some() {
            out.drops.push(plan);
        }
    }

    /// The text box that renames a page's title, a group or a cabinet.
    fn rename_box(&mut self, ui: &mut egui::Ui, row: &TreeRow, rect: egui::Rect) {
        let Some(rename) = self.ol.rename.as_mut() else {
            return;
        };
        let edit = ui.put(
            rect,
            egui::TextEdit::singleline(&mut rename.text)
                .hint_text(match rename.of {
                    RenameOf::Page(_) => "Page title",
                    RenameOf::Group(_) => "Group name",
                    RenameOf::Cabinet(_) => "Cabinet name",
                })
                .desired_width(f32::INFINITY),
        );
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        // Group and cabinet names are plain codes, shown without = or +.
        let escape = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if escape {
            self.ol.rename = None;
            self.ol.selected = Some(row.key.clone());
        } else if edit.lost_focus() {
            let done = self.ol.rename.take();
            if let Some(done) = done {
                self.finish_rename(done);
            }
        }
    }

    fn finish_rename(&mut self, rename: Rename) {
        match rename.of {
            RenameOf::Page(page) => {
                let unchanged = self
                    .backend
                    .document()
                    .and_then(|d| {
                        d.project()
                            .page(page)
                            .map(|p| p.title == rename.text.trim())
                    })
                    .unwrap_or(true);
                if !unchanged {
                    self.edit("Rename page", |tx| tree::set_title(tx, page, &rename.text));
                }
            }
            RenameOf::Group(from) => self.rename_level_to(Aspect::Function, &from, &rename.text),
            RenameOf::Cabinet(from) => self.rename_level_to(Aspect::Location, &from, &rename.text),
        }
    }

    fn rename_level_to(&mut self, aspect: Aspect, from: &Levels, text: &str) {
        let tag = DeviceTag {
            function: if aspect == Aspect::Function {
                from.clone()
            } else {
                Vec::new()
            },
            location: if aspect == Aspect::Location {
                from.clone()
            } else {
                Vec::new()
            },
            ..Default::default()
        };
        let to = super::pages::clean_level(text, aspect);
        if to.is_empty() {
            return;
        }
        self.rename_level(aspect, &tag, &to);
        self.ol.reveal = Some(match aspect {
            Aspect::Function => Reveal::Group(vec![to.trim_start_matches('=').to_owned()]),
            _ => Reveal::Cabinet(vec![to.trim_start_matches('+').to_owned()]),
        });
    }

    // ------------------------------------------------------------------
    // Doing things

    /// Runs one edit of the model as one undo step and tells the canvas and
    /// storage. Errors become a message.
    fn edit<T>(
        &mut self,
        label: &str,
        edit: impl FnOnce(&mut novo_core::Tx<'_>) -> novo_core::Result<T>,
    ) -> Option<T> {
        let doc = self.backend.document_mut()?;
        match doc.transact(label, edit) {
            Ok((value, changes)) => {
                self.changed_outside_canvas(&changes);
                self.fix_current_page();
                Some(value)
            }
            Err(e) => {
                self.message(Level::Error, format!("{label}: {e}"));
                None
            }
        }
    }

    /// Does what a menu entry, a key or a drop asked.
    pub(super) fn perform(&mut self, act: Act) {
        match act {
            Act::NewPage { structure, after } => {
                let page = self.edit("New page", |tx| tree::add_page(tx, &structure, "", after));
                if let Some(page) = page {
                    self.show_page(page);
                    self.ol.reveal = Some(Reveal::Page { page, rename: true });
                }
            }
            Act::NewGroup { in_cabinet } => {
                let name = self
                    .backend
                    .document()
                    .map(|d| self.naming().next_group(d.project()))
                    .unwrap_or_default();
                if name.is_empty() {
                    return;
                }
                let page = self.edit("New page group", |tx| {
                    tree::add_group(tx, &name, in_cabinet.as_ref())
                });
                if let Some(page) = page {
                    self.show_page(page);
                    self.ol.reveal = Some(Reveal::Group(vec![name.clone()]));
                    self.ol.rename = Some(Rename {
                        of: RenameOf::Group(vec![name.clone()]),
                        text: name,
                    });
                }
            }
            Act::NewCabinet { in_group } => self.open_cabinet_form(in_group.unwrap_or_default()),
            Act::RenamePage(page) => {
                let Some(p) = self.backend.document().and_then(|d| d.project().page(page)) else {
                    return;
                };
                if matches!(p.kind, PageKind::Generated { .. }) {
                    return;
                }
                self.ol.rename = Some(Rename {
                    of: RenameOf::Page(page),
                    text: p.title.clone(),
                });
                self.ol.reveal = Some(Reveal::Page { page, rename: true });
            }
            Act::RenameGroup(levels) => {
                self.ol.rename = Some(Rename {
                    text: level_text(&levels),
                    of: RenameOf::Group(levels.clone()),
                });
                self.ol.reveal = Some(Reveal::Group(levels));
            }
            Act::RenameCabinet(levels) => {
                self.ol.rename = Some(Rename {
                    text: level_text(&levels),
                    of: RenameOf::Cabinet(levels.clone()),
                });
                self.ol.reveal = Some(Reveal::Cabinet(levels));
            }
            Act::Properties(key) => self.open_props(key),
            Act::DuplicatePage(page) => {
                let copy = self.edit("Duplicate page", |tx| tree::duplicate_page(tx, page));
                if let Some(copy) = copy {
                    self.show_page(copy);
                    self.ol.reveal = Some(Reveal::Page {
                        page: copy,
                        rename: false,
                    });
                    self.message(Level::Info, "Duplicated the page, devices got new tags");
                }
            }
            Act::MoveUp(page) => self.step_page(page, false),
            Act::MoveDown(page) => self.step_page(page, true),
            Act::MoveTo {
                page,
                function,
                location,
            } => {
                self.edit("Move page", |tx| {
                    tree::move_page(tx, page, function, location).map(|_| ())
                });
                self.ol.reveal = Some(Reveal::Page {
                    page,
                    rename: false,
                });
            }
            Act::MoveToNew(page) => self.open_props(Key::Page(page)),
            Act::DeletePage(page) => self.delete_page(page),
            Act::RemoveLevel(aspect, levels) => {
                let kind = if aspect == Aspect::Function {
                    "group"
                } else {
                    "cabinet"
                };
                let n = self.edit("Remove", |tx| tree::remove_level(tx, aspect, &levels));
                if let Some(n) = n {
                    self.message(
                        Level::Info,
                        format!("Removed the {kind}, {n} pages kept (Ctrl+Z undoes)"),
                    );
                }
            }
            Act::DeleteLevelAndPages(aspect, levels) => self.ask_delete_level(aspect, levels),
            Act::Renumber(structure) => {
                let n = self.edit("Renumber pages", |tx| tree::renumber(tx, &structure));
                if let Some(n) = n {
                    self.message(
                        Level::Info,
                        if n == 0 {
                            "Page numbers are already in order".to_owned()
                        } else {
                            format!("Renumbered {n} pages")
                        },
                    );
                }
            }
            Act::AddContents(levels) => self.add_panel_contents(&DeviceTag {
                location: levels,
                ..Default::default()
            }),
            Act::OpenCabinet(levels) => {
                self.cabinet.location = Some(DeviceTag {
                    location: levels,
                    ..Default::default()
                });
                self.switch_workspace(super::workspace::Workspace::Cabinet);
            }
            Act::Fold(key, shut) => {
                if shut {
                    self.ol.collapsed.insert(key);
                } else {
                    self.ol.collapsed.remove(&key);
                }
            }
            Act::FoldAll(shut) => {
                if shut {
                    self.ol.collapsed = tree::foldable_keys(&self.ol.rows).into_iter().collect();
                } else {
                    self.ol.collapsed.clear();
                }
            }
            Act::Reorder(order) => self.reorder_pages(order),
            Act::MoveCabinet { location, from, to } => {
                let n = self.edit("Move cabinet", |tx| {
                    tree::move_cabinet(tx, &location, &from, &to)
                });
                if let Some(n) = n {
                    self.message(
                        Level::Info,
                        format!(
                            "Moved {} pages of {} into {}",
                            n,
                            cabinet_name(&location),
                            group_name(&to)
                        ),
                    );
                    self.ol.reveal = Some(Reveal::Group(to));
                }
            }
            Act::AutoFile => {
                let n = self.edit("File pages by device tags", tree::auto_file);
                if let Some(n) = n {
                    self.message(
                        Level::Info,
                        format!("Filed {n} pages by their device tags (Ctrl+Z undoes)"),
                    );
                }
            }
            Act::Confirmed(inner) => self.perform_confirmed(*inner),
        }
    }

    /// The `page.*` commands: the outline's page actions, on the page that
    /// is open, for the palette and the File > Pages menu. Generated pages
    /// are not drawn by hand, so the outline offers these on drawn pages
    /// only and so do the commands.
    pub(super) fn page_command(&mut self, id: &str) {
        let Some(page) = self.current_page else {
            return;
        };
        let drawn = self
            .backend
            .document()
            .and_then(|d| d.project().page(page))
            .is_some_and(|p| !matches!(p.kind, PageKind::Generated { .. }));
        if !drawn {
            self.message(
                Level::Info,
                "Generated pages are kept by the project: change them under Reports",
            );
            return;
        }
        let act = match id {
            "page.rename" => Act::RenamePage(page),
            "page.duplicate" => Act::DuplicatePage(page),
            "page.delete" => Act::DeletePage(page),
            "page.move_up" => Act::MoveUp(page),
            "page.move_down" => Act::MoveDown(page),
            _ => return,
        };
        self.perform(act);
    }

    fn step_page(&mut self, page: PageId, down: bool) {
        let Some(siblings) = self
            .backend
            .document()
            .map(|d| d.project().sibling_pages(page))
        else {
            return;
        };
        if let Some(order) = super::pages::order_after_step(&siblings, page, down) {
            self.reorder_pages(order);
            self.ol.reveal = Some(Reveal::Page {
                page,
                rename: false,
            });
        }
    }

    fn delete_page(&mut self, page: PageId) {
        let Some(p) = self.backend.document().and_then(|d| d.project().page(page)) else {
            return;
        };
        let label = format!("{} {}", p.name, p.title).trim().to_owned();
        // Where selection goes afterwards: the page below, else above.
        let siblings = self
            .backend
            .document()
            .map(|d| d.project().sibling_pages(page))
            .unwrap_or_default();
        let at = siblings.iter().position(|s| *s == page).unwrap_or(0);
        let next = siblings
            .get(at + 1)
            .or_else(|| at.checked_sub(1).and_then(|n| siblings.get(n)))
            .copied();
        if self
            .edit("Delete page", |tx| tx.delete_page(page))
            .is_some()
        {
            self.message(Level::Info, format!("Deleted page {label} (Ctrl+Z undoes)"));
            if self.current_page == Some(page)
                && let Some(next) = next
            {
                self.show_page(next);
            }
            self.ol.selected = None;
            if let Some(next) = next {
                self.ol.reveal = Some(Reveal::Page {
                    page: next,
                    rename: false,
                });
            }
        }
    }

    fn ask_delete_level(&mut self, aspect: Aspect, levels: Levels) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let (pages, devices, plates) = tree::level_usage(project, aspect, &levels);
        let (kind, name) = if aspect == Aspect::Function {
            ("page group", group_name(&levels))
        } else {
            ("cabinet", cabinet_name(&levels))
        };
        let mut body = format!("{pages} drawn pages in {name} are deleted.");
        if devices > 0 {
            body.push_str(&format!(
                " The {devices} devices keep their tags and stay in the project."
            ));
        }
        if plates > 0 && aspect == Aspect::Location {
            body.push_str(" The cabinet layout (plate and placed devices) goes too.");
        }
        body.push_str(" Ctrl+Z brings it all back.");
        self.ol.confirm = Some(Confirm {
            title: format!("Delete {kind} {name}?"),
            body,
            yes: "Delete".into(),
            act: Act::DeleteLevelAndPages(aspect, levels),
        });
    }

    fn perform_confirmed(&mut self, act: Act) {
        if let Act::DeleteLevelAndPages(aspect, levels) = act {
            let n = self.edit("Delete with pages", |tx| {
                tree::delete_level_and_pages(tx, aspect, &levels)
            });
            if let Some(n) = n {
                self.message(Level::Info, format!("Deleted {n} pages (Ctrl+Z undoes)"));
            }
            self.ol.selected = None;
        }
    }

    // ------------------------------------------------------------------
    // Keys

    /// The tree's keys, while it has the focus and no text box is open.
    /// Called before the app's own keys so Delete and Ctrl+D act on the
    /// selected row instead of the drawing.
    pub(super) fn outline_keys(&mut self, ctx: &egui::Context) {
        let rect = self.ol.rect.take();
        if ctx.input(|i| i.pointer.any_pressed())
            && let Some(pos) = ctx.input(|i| i.pointer.interact_pos())
        {
            // A press elsewhere (the canvas) hands the keys back.
            self.ol.focused = rect.is_some_and(|r| r.contains(pos));
        }
        if !self.ol.focused
            || rect.is_none()
            || self.ol.dialog_open()
            || self.outline_mode != super::OutlineMode::Pages
            || ctx.egui_wants_keyboard_input()
            || self.backend.document().is_none()
        {
            return;
        }
        use egui::{Key, Modifiers};
        let press = |m: Modifiers, k: Key| ctx.input_mut(|i| i.consume_key(m, k));
        let selected = self.ol.selected.clone();
        let row = selected.as_deref().and_then(|k| self.ol.row(k)).cloned();
        let mut acts: Vec<Act> = Vec::new();
        let mut open: Option<(PageId, usize)> = None;

        if press(Modifiers::NONE, Key::F2)
            && let Some(row) = &row
        {
            match &row.kind {
                RowKind::Page {
                    page, drawn: true, ..
                } => acts.push(Act::RenamePage(*page)),
                RowKind::Group { function, .. } => acts.push(Act::RenameGroup(function.clone())),
                RowKind::Cabinet { location, .. } => {
                    acts.push(Act::RenameCabinet(location.clone()))
                }
                _ => {}
            }
        }
        if press(Modifiers::NONE, Key::Delete)
            && let Some(row) = &row
        {
            match &row.kind {
                RowKind::Page {
                    page, drawn: true, ..
                } => acts.push(Act::DeletePage(*page)),
                RowKind::Group { function, .. } => {
                    acts.push(Act::DeleteLevelAndPages(Aspect::Function, function.clone()))
                }
                RowKind::Cabinet { location, .. } => {
                    acts.push(Act::DeleteLevelAndPages(Aspect::Location, location.clone()))
                }
                _ => {}
            }
        }
        if let Some(RowKind::Page { page, .. }) = row.as_ref().map(|r| &r.kind) {
            if press(Modifiers::COMMAND, Key::ArrowUp) {
                acts.push(Act::MoveUp(*page));
            }
            if press(Modifiers::COMMAND, Key::ArrowDown) {
                acts.push(Act::MoveDown(*page));
            }
            if press(Modifiers::COMMAND, Key::D) {
                acts.push(Act::DuplicatePage(*page));
            }
        }
        if press(Modifiers::NONE, Key::Insert) {
            acts.push(Act::NewPage {
                structure: self.new_here(),
                after: None,
            });
        }
        for (key, down) in [(Key::ArrowDown, true), (Key::ArrowUp, false)] {
            if press(Modifiers::NONE, key)
                && let Some(next) = self.ol.neighbour(down).cloned()
            {
                self.ol.selected = Some(next.key.clone());
                self.ol.scroll_to = Some(next.key.clone());
                if let Some((p, s)) = next.page() {
                    open = Some((p, s));
                }
            }
        }
        if let Some(row) = &row {
            let shut = self.ol.collapsed.contains(&row.key);
            if press(Modifiers::NONE, Key::ArrowRight) && row.foldable && shut {
                acts.push(Act::Fold(row.key.clone(), false));
            }
            if press(Modifiers::NONE, Key::ArrowLeft) {
                if row.foldable && !shut {
                    acts.push(Act::Fold(row.key.clone(), true));
                } else if let Some(parent) = ancestors(&row.key).last() {
                    self.ol.selected = Some(parent.clone());
                    self.ol.scroll_to = Some(parent.clone());
                }
            }
            if press(Modifiers::NONE, Key::Enter) {
                match row.page() {
                    Some((p, s)) => open = Some((p, s)),
                    None if row.foldable => acts.push(Act::Fold(row.key.clone(), !shut)),
                    None => {}
                }
            }
        }
        if let Some((p, s)) = open {
            self.show_sheet(p, s);
        }
        for act in acts {
            self.perform(act);
        }
    }
}

/// What a row asked for during a frame.
/// What a click on a Wires or Cables row asks for.
enum NetJump {
    Wire(novo_core::WireId),
    /// A cable with wired cores: all of them are selected together.
    Cable(Vec<novo_core::WireId>),
    /// A cable with no wired core; its text for the message.
    EmptyCable(String),
}

#[derive(Default)]
struct RowOut {
    acts: Vec<Act>,
    open: Option<(PageId, usize)>,
    select: bool,
    drops: Vec<DropPlan>,
}

fn collect(
    out: RowOut,
    row: &TreeRow,
    acts: &mut Vec<Act>,
    opened: &mut Option<(PageId, usize)>,
    select: &mut Option<String>,
) {
    acts.extend(out.acts);
    acts.extend(out.drops.into_iter().map(DropPlan::into_act));
    if out.open.is_some() {
        *opened = out.open;
    }
    if out.select {
        *select = Some(row.key.clone());
    }
}

fn count_text(pages: usize) -> String {
    match pages {
        0 => String::new(),
        1 => "1 page".into(),
        n => format!("{n} pages"),
    }
}

fn target_of(row: &TreeRow) -> Option<Target> {
    match &row.kind {
        RowKind::Group {
            function, context, ..
        } => Some(Target::Group {
            function: function.clone(),
            context: context.clone(),
        }),
        RowKind::Cabinet {
            location, context, ..
        } => Some(Target::Cabinet {
            location: location.clone(),
            context: context.clone(),
        }),
        RowKind::Page { page, .. } => Some(Target::Page(*page)),
        RowKind::Generated { .. } => Some(Target::GeneratedHeader),
        RowKind::Empty => Some(Target::Background),
    }
}

/// Fills a menu from its entries; a click pushes the entry's act.
fn menu_items(ui: &mut egui::Ui, items: &[Item], acts: &mut Vec<Act>) {
    for item in items {
        match item {
            Item::Separator => {
                ui.separator();
            }
            Item::Entry {
                label,
                act,
                enabled,
                keys,
                why,
            } => {
                let mut button = egui::Button::new(label.as_str());
                if !keys.is_empty() {
                    button = button.shortcut_text(*keys);
                }
                let r = ui.add_enabled(*enabled, button);
                let r = if !enabled && !why.is_empty() {
                    r.on_disabled_hover_text(*why)
                } else {
                    r
                };
                if r.clicked() {
                    acts.push(act.clone());
                    ui.close();
                }
            }
            Item::Submenu { label, items } => {
                ui.menu_button(label.as_str(), |ui| {
                    ui.set_min_width(180.0);
                    menu_items(ui, items, acts)
                });
            }
        }
    }
}

// ---------------------------------------------------------------------
// Drops

/// What a drop would do.
#[derive(Debug, Clone, PartialEq)]
enum DropPlan {
    /// Puts the page into a group and/or cabinet.
    File {
        page: PageId,
        function: Option<Levels>,
        location: Option<Levels>,
    },
    /// Puts the pages in this order.
    Reorder(Vec<PageId>),
    /// Moves all pages of a cabinet, as listed under `from`, into a group.
    Cabinet {
        location: Levels,
        from: Levels,
        to: Levels,
    },
}

impl DropPlan {
    fn into_act(self) -> Act {
        match self {
            DropPlan::File {
                page,
                function,
                location,
            } => Act::MoveTo {
                page,
                function,
                location,
            },
            DropPlan::Reorder(order) => Act::Reorder(order),
            DropPlan::Cabinet { location, from, to } => Act::MoveCabinet { location, from, to },
        }
    }
}

/// What dropping `drag` on `row` does, or `None` when it does nothing.
fn drop_plan(
    project: &novo_core::Project,
    drag: &Drag,
    row: &TreeRow,
    lower_half: bool,
    view: PageView,
) -> Option<DropPlan> {
    match (drag, &row.kind) {
        (
            Drag::Page(page),
            RowKind::Group {
                function, context, ..
            },
        ) => {
            let p = project.page(*page)?;
            let location = (view == PageView::Location).then(|| context.location.clone());
            let same = p.structure.function == *function
                && location.as_ref().is_none_or(|l| p.structure.location == *l);
            (!same).then(|| DropPlan::File {
                page: *page,
                function: Some(function.clone()),
                location,
            })
        }
        (
            Drag::Page(page),
            RowKind::Cabinet {
                location, context, ..
            },
        ) => {
            let p = project.page(*page)?;
            let function = (view == PageView::Function).then(|| context.function.clone());
            let same = p.structure.location == *location
                && function.as_ref().is_none_or(|f| p.structure.function == *f);
            (!same).then(|| DropPlan::File {
                page: *page,
                function,
                location: Some(location.clone()),
            })
        }
        (Drag::Page(page), RowKind::Page { page: target, .. }) => {
            if page == target {
                return None;
            }
            let siblings = project.sibling_pages(*target);
            if siblings.contains(page) {
                super::pages::order_after_drop(&siblings, *page, *target, lower_half)
                    .map(DropPlan::Reorder)
            } else {
                // Another group or cabinet: the page joins that one.
                let t = project.page(*target)?;
                Some(DropPlan::File {
                    page: *page,
                    function: Some(t.structure.function.clone()),
                    location: Some(t.structure.location.clone()),
                })
            }
        }
        (Drag::Cabinet { location, group }, RowKind::Group { function, .. })
            if function != group =>
        {
            Some(DropPlan::Cabinet {
                location: location.clone(),
                from: group.clone(),
                to: function.clone(),
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------
// Windows

impl NovoApp {
    pub(super) fn open_cabinet_form(&mut self, group: Levels) {
        self.sizing.ensure_loaded();
        self.ol.cabinet_form = Some(CabinetForm {
            name: String::new(),
            group,
            enclosure: None,
            first_page: true,
            focused: false,
        });
    }

    fn open_props(&mut self, key: Key) {
        self.sizing.ensure_loaded();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        self.ol.props = match key {
            Key::Page(page) => project.page(page).map(|p| PropsForm::Page {
                page,
                number: p.name.clone(),
                title: p.title.clone(),
                function: p.structure.function.clone(),
                location: p.structure.location.clone(),
            }),
            Key::Group(levels) => Some(PropsForm::Level {
                aspect: Aspect::Function,
                name: level_text(&levels),
                from: levels,
                description: String::new(),
                enclosure: None,
                width: 0.0,
                height: 0.0,
                has_plate: false,
            }),
            Key::Cabinet(levels) => {
                let s = project.structure_by_tag(&DeviceTag {
                    location: levels.clone(),
                    ..Default::default()
                });
                Some(PropsForm::Level {
                    aspect: Aspect::Location,
                    name: level_text(&levels),
                    from: levels,
                    description: s.map(|s| s.description.clone()).unwrap_or_default(),
                    enclosure: None,
                    width: s.and_then(|s| s.plate).map_or(600.0, |p| p.width),
                    height: s.and_then(|s| s.plate).map_or(800.0, |p| p.height),
                    has_plate: s.is_some_and(|s| s.plate.is_some()),
                })
            }
        };
    }

    /// The windows of the tree: new cabinet, properties and the question
    /// before a delete.
    pub(super) fn outline_dialogs(&mut self, ctx: &egui::Context) {
        self.cabinet_form_dialog(ctx);
        self.props_dialog(ctx);
        self.confirm_dialog(ctx);
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.ol.confirm.clone() else {
            return;
        };
        let modal = egui::Modal::new(egui::Id::new("outline_confirm")).show(ctx, |ui| {
            ui.set_width(360.0);
            ui.heading(&confirm.title);
            ui.add_space(4.0);
            ui.label(&confirm.body);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let yes = ui.button(format!("{} (Enter)", confirm.yes)).clicked();
                let no = ui.button("Cancel (Esc)").clicked();
                (yes, no)
            })
            .inner
        });
        let (yes, no) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if no || modal.should_close() {
            self.ol.confirm = None;
        } else if yes || enter {
            self.ol.confirm = None;
            self.perform(Act::Confirmed(Box::new(confirm.act)));
        }
    }

    fn cabinet_form_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.ol.cabinet_form.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        if form.name.is_empty() && !form.focused {
            form.name = self.naming().next_cabinet(project);
        }
        let groups: Vec<Levels> = project
            .page_groups()
            .into_iter()
            .map(|g| g.function)
            .collect();
        let catalog: Vec<EnclosureSize> = self.sizing.catalog.items.clone();
        let exists = tree::level_exists(project, Aspect::Location, &[form.name.trim().to_owned()]);
        let valid = super::pages::clean_level(&form.name, Aspect::Location);
        let mut open_list = false;
        let modal = egui::Modal::new(egui::Id::new("cabinet_form")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading("New cabinet");
            ui.add_space(6.0);
            egui::Grid::new("cabinet_form_grid")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Name");
                    ui.vertical(|ui| {
                        let name = ui.add(
                            egui::TextEdit::singleline(&mut form.name)
                                .desired_width(200.0)
                                .hint_text("A1"),
                        );
                        if !form.focused {
                            name.request_focus();
                            form.focused = true;
                        }
                        if valid.is_empty() {
                            ui.colored_label(crate::theme::ui_palette().error, "Give it a name");
                        } else if exists {
                            ui.colored_label(
                                crate::theme::ui_palette().warning,
                                format!("{valid} exists: its layout stays, a page is added"),
                            );
                        } else {
                            ui.weak(format!("Called {valid}, from the names in this project"));
                        }
                    });
                    ui.end_row();

                    ui.label("Enclosure");
                    ui.vertical(|ui| {
                        let current = match form.enclosure.and_then(|i| catalog.get(i)) {
                            Some(e) => enclosure_text(e),
                            None => format!(
                                "Standard plate {} x {} mm",
                                DEFAULT_PLATE.width, DEFAULT_PLATE.height
                            ),
                        };
                        egui::ComboBox::from_id_salt("cabinet_form_enclosure")
                            .selected_text(current)
                            .width(250.0)
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut form.enclosure,
                                    None,
                                    format!(
                                        "Standard plate {} x {} mm",
                                        DEFAULT_PLATE.width, DEFAULT_PLATE.height
                                    ),
                                );
                                for (i, e) in catalog.iter().enumerate() {
                                    ui.selectable_value(
                                        &mut form.enclosure,
                                        Some(i),
                                        enclosure_text(e),
                                    );
                                }
                            });
                        if catalog.is_empty() {
                            ui.weak("Your enclosure list is empty.");
                        }
                        if ui.small_button("Edit the enclosure list…").clicked() {
                            open_list = true;
                        }
                    });
                    ui.end_row();

                    ui.label("Page group");
                    egui::ComboBox::from_id_salt("cabinet_form_group")
                        .selected_text(if form.group.is_empty() {
                            "No group".to_owned()
                        } else {
                            group_name(&form.group)
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut form.group, Vec::new(), "No group");
                            for g in &groups {
                                ui.selectable_value(&mut form.group, g.clone(), group_name(g));
                            }
                        });
                    ui.end_row();

                    ui.label("");
                    ui.checkbox(&mut form.first_page, "Add its first page");
                    ui.end_row();
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ok = ui
                    .add_enabled(!valid.is_empty(), egui::Button::new("Create (Enter)"))
                    .clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (ok, cancel)
            })
            .inner
        });
        if open_list {
            self.sizing.editor_open = true;
        }
        let (ok, cancel) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if cancel || modal.should_close() {
            return;
        }
        if (ok || enter) && !valid.is_empty() {
            let plate = match form.enclosure.and_then(|i| catalog.get(i)) {
                Some(e) => MountingPlate {
                    width: e.plate_width,
                    height: e.plate_height,
                    margin: e.margin,
                },
                None => DEFAULT_PLATE,
            };
            let name = valid.trim_start_matches('+').to_owned();
            let spec = CabinetSpec {
                name: name.clone(),
                group: form.group.clone(),
                plate,
                first_page: form.first_page,
            };
            let enclosure_name = form
                .enclosure
                .and_then(|i| catalog.get(i))
                .map(|e| e.name.clone());
            let page = self.edit("New cabinet", |tx| {
                let page = tree::add_cabinet(tx, &spec)?;
                if let Some(enclosure) = &enclosure_name
                    && let Some(id) = tx
                        .project()
                        .structure_by_tag(&DeviceTag {
                            location: vec![spec.name.clone()],
                            ..Default::default()
                        })
                        .map(|s| s.id)
                {
                    tx.execute(Command::SetProperty {
                        target: AnyId::Structure(id),
                        key: tree::ENCLOSURE_PROPERTY.into(),
                        value: Some(novo_core::Value::Text(enclosure.clone())),
                    })?;
                }
                Ok(page)
            });
            if let Some(page) = page {
                if let Some(page) = page {
                    self.show_page(page);
                }
                self.ol.reveal = Some(Reveal::Cabinet(vec![name]));
                self.message(Level::Info, format!("Created cabinet {valid}"));
            }
            return;
        }
        self.ol.cabinet_form = Some(form);
    }

    fn props_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.ol.props.take() else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let groups: Vec<Levels> = project
            .page_groups()
            .into_iter()
            .map(|g| g.function)
            .collect();
        let cabinets: Vec<Levels> = project
            .panels()
            .into_iter()
            .map(|p| p.location.location)
            .collect();
        let catalog: Vec<EnclosureSize> = self.sizing.catalog.items.clone();
        let usage = match &form {
            PropsForm::Level { aspect, from, .. } => {
                let (pages, devices, _) = tree::level_usage(project, *aspect, from);
                let cables = project
                    .cables()
                    .filter(|c| c.tag.levels(*aspect).starts_with(from))
                    .count();
                format!("{pages} pages, {devices} devices, {cables} cables")
            }
            _ => String::new(),
        };
        let heading = match &form {
            PropsForm::Page { .. } => "Page properties",
            PropsForm::Level {
                aspect: Aspect::Function,
                ..
            } => "Page group properties",
            PropsForm::Level { .. } => "Cabinet properties",
        };
        let modal = egui::Modal::new(egui::Id::new("outline_props")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(heading);
            ui.add_space(6.0);
            egui::Grid::new("outline_props_grid")
                .num_columns(2)
                .spacing([10.0, 8.0])
                .show(ui, |ui| match &mut form {
                    PropsForm::Page {
                        number,
                        title,
                        function,
                        location,
                        ..
                    } => {
                        ui.label("Number");
                        ui.add(egui::TextEdit::singleline(number).desired_width(120.0));
                        ui.end_row();
                        ui.label("Title");
                        let t = ui.add(egui::TextEdit::singleline(title).desired_width(260.0));
                        if !t.has_focus() && ui.memory(|m| m.focused().is_none()) {
                            t.request_focus();
                        }
                        ui.end_row();
                        ui.label("Page group");
                        egui::ComboBox::from_id_salt("props_group")
                            .selected_text(if function.is_empty() {
                                "No group".to_owned()
                            } else {
                                group_name(function)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(function, Vec::new(), "No group");
                                for g in &groups {
                                    ui.selectable_value(function, g.clone(), group_name(g));
                                }
                            });
                        ui.end_row();
                        ui.label("Cabinet");
                        egui::ComboBox::from_id_salt("props_cabinet")
                            .selected_text(if location.is_empty() {
                                "No cabinet".to_owned()
                            } else {
                                cabinet_name(location)
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(location, Vec::new(), "No cabinet");
                                for c in &cabinets {
                                    ui.selectable_value(location, c.clone(), cabinet_name(c));
                                }
                            });
                        ui.end_row();
                    }
                    PropsForm::Level {
                        aspect,
                        name,
                        description,
                        enclosure,
                        width,
                        height,
                        has_plate,
                        ..
                    } => {
                        ui.label("Name");
                        ui.add(egui::TextEdit::singleline(name).desired_width(200.0));
                        ui.end_row();
                        ui.label("Used by");
                        ui.weak(&usage);
                        ui.end_row();
                        if *aspect == Aspect::Location {
                            ui.label("Description");
                            ui.add(egui::TextEdit::singleline(description).desired_width(260.0));
                            ui.end_row();
                            ui.label("Enclosure");
                            let current = match enclosure.and_then(|i| catalog.get(i)) {
                                Some(e) => enclosure_text(e),
                                None => "Keep the plate below".to_owned(),
                            };
                            egui::ComboBox::from_id_salt("props_enclosure")
                                .selected_text(current)
                                .width(250.0)
                                .show_ui(ui, |ui| {
                                    ui.selectable_value(enclosure, None, "Keep the plate below");
                                    for (i, e) in catalog.iter().enumerate() {
                                        ui.selectable_value(enclosure, Some(i), enclosure_text(e));
                                    }
                                });
                            ui.end_row();
                            ui.label("Plate (mm)");
                            ui.horizontal(|ui| {
                                ui.add(egui::DragValue::new(width).range(50.0..=5000.0).speed(5.0));
                                ui.label("x");
                                ui.add(
                                    egui::DragValue::new(height).range(50.0..=5000.0).speed(5.0),
                                );
                                if !*has_plate {
                                    ui.weak("none yet");
                                }
                            });
                            ui.end_row();
                        }
                    }
                });
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                let ok = ui.button("Apply (Enter)").clicked();
                let cancel = ui.button("Cancel (Esc)").clicked();
                (ok, cancel)
            })
            .inner
        });
        let (ok, cancel) = modal.inner;
        let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
        if cancel || modal.should_close() {
            return;
        }
        if ok || enter {
            self.apply_props(form, &catalog);
            return;
        }
        self.ol.props = Some(form);
    }

    fn apply_props(&mut self, form: PropsForm, catalog: &[EnclosureSize]) {
        match form {
            PropsForm::Page {
                page,
                number,
                title,
                function,
                location,
            } => {
                let number = number.trim().to_owned();
                let ok = self.edit("Page properties", |tx| {
                    if !number.is_empty() {
                        tx.execute(Command::RenamePage {
                            page,
                            name: number.clone(),
                        })?;
                    }
                    tree::set_title(tx, page, &title)?;
                    let keep_number = tx.project().page(page).map(|p| p.name.clone());
                    tree::move_page(tx, page, Some(function), Some(location))?;
                    // A number the person typed wins over the automatic one.
                    if let Some(n) = keep_number
                        && !number.is_empty()
                        && n == number
                    {
                        tx.execute(Command::RenamePage { page, name: n })?;
                    }
                    Ok(())
                });
                if ok.is_some() {
                    self.ol.reveal = Some(Reveal::Page {
                        page,
                        rename: false,
                    });
                }
            }
            PropsForm::Level {
                aspect,
                from,
                name,
                description,
                enclosure,
                width,
                height,
                ..
            } => {
                let renamed = super::pages::clean_level(&name, aspect);
                let current = if aspect == Aspect::Function {
                    group_name(&from)
                } else {
                    cabinet_name(&from)
                };
                let mut shown = from.clone();
                if !renamed.is_empty() && renamed != current {
                    let tag = DeviceTag {
                        function: if aspect == Aspect::Function {
                            from.clone()
                        } else {
                            Vec::new()
                        },
                        location: if aspect == Aspect::Location {
                            from.clone()
                        } else {
                            Vec::new()
                        },
                        ..Default::default()
                    };
                    self.rename_level(aspect, &tag, &renamed);
                    shown = vec![renamed.trim_start_matches(['=', '+']).to_owned()];
                }
                if aspect == Aspect::Location {
                    let chosen = enclosure.and_then(|i| catalog.get(i)).cloned();
                    let plate = match &chosen {
                        Some(e) => MountingPlate {
                            width: e.plate_width,
                            height: e.plate_height,
                            margin: e.margin,
                        },
                        None => MountingPlate {
                            width,
                            height,
                            margin: DEFAULT_PLATE.margin,
                        },
                    };
                    let location = shown.clone();
                    self.edit("Cabinet properties", |tx| {
                        let tag = DeviceTag {
                            location: location.clone(),
                            ..Default::default()
                        };
                        let id = match tx.project().structure_by_tag(&tag).map(|s| s.id) {
                            Some(id) => id,
                            None => tx.add_structure(tag, "", true)?,
                        };
                        let text = description.trim().to_owned();
                        tx.update::<novo_core::Structure>(id, |s| s.description = text)?;
                        let existing = tx
                            .project()
                            .structures()
                            .find(|s| s.id == id)
                            .and_then(|s| s.plate);
                        match (&chosen, existing) {
                            (Some(_), _) => tx.set_plate(id, Some(plate))?,
                            (None, Some(p)) if (p.width, p.height) != (width, height) => {
                                tx.set_plate(
                                    id,
                                    Some(MountingPlate {
                                        width,
                                        height,
                                        margin: p.margin,
                                    }),
                                )?;
                            }
                            _ => {}
                        }
                        if let Some(e) = &chosen {
                            tx.execute(Command::SetProperty {
                                target: AnyId::Structure(id),
                                key: tree::ENCLOSURE_PROPERTY.into(),
                                value: Some(novo_core::Value::Text(e.name.clone())),
                            })?;
                        }
                        Ok(())
                    });
                    self.ol.reveal = Some(Reveal::Cabinet(shown));
                } else {
                    self.ol.reveal = Some(Reveal::Group(shown));
                }
            }
        }
    }
}

fn enclosure_text(e: &EnclosureSize) -> String {
    format!("{} · {} x {} x {} mm", e.name, e.width, e.height, e.depth)
}
