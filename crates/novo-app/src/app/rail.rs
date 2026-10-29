//! The tool rail (docs/design/visual-redesign.md section 3.2): a narrow column
//! on the left of a workspace with three zones separated by hairlines, tools
//! (what the cursor does), assists (what the program does for you) and panels
//! (toggles). It knows nothing about any workspace: the app hands it
//! [`Entry`] values built from command ids and reads back what was clicked.
//!
//! A corner notch marks an entry with a flyout, a small card beside the rail.
//! Click opens it, Esc or a click elsewhere closes it, a second click on the
//! entry toggles it. The rail expands (icon, name and key) with the button at
//! its foot. Everything is reachable from the keyboard: Tab, the up and down
//! arrows between entries, Enter, Esc. High contrast draws a border on every
//! entry and the focus ring is a heavy outline, not a colour. Nothing
//! animates, so Reduce motion needs no branch.

use eframe::egui::{self, Align, Layout, Rect, Sense, Stroke, Vec2};
use novo_theme::presentation::Glyph;

use crate::theme::ui_palette;

/// The collapsed width of the rail.
pub(super) const WIDTH: f32 = 48.0;
/// The expanded width of the rail.
pub(super) const WIDTH_EXPANDED: f32 = 228.0;
/// The height of one entry (a 24 pt icon with room around it).
pub(super) const ENTRY: f32 = 34.0;
/// The side of the icon inside an entry.
const ICON: f32 = 20.0;
/// At most this many entries in one zone (design doc section 2, count test).
#[cfg(test)]
pub(super) const MAX_PER_ZONE: usize = 6;

/// A count on an entry. Warning coloured only when `warn` (issues); the
/// tooltip says it in words, so the badge is never the only carrier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Badge {
    pub count: usize,
    pub warn: bool,
}

/// One row of a flyout.
#[derive(Clone, Debug)]
pub(super) struct FlyEntry {
    /// What a click reports.
    pub id: &'static str,
    /// The lesson anchor of the row, if it has one.
    pub anchor: Option<&'static str>,
    pub glyph: Glyph,
    pub label: String,
    pub tip: String,
    pub key: Option<String>,
    /// The choice in force, marked with a tick and the accent wash.
    pub current: bool,
    pub enabled: bool,
    /// A caption printed above this row (a new group in the card).
    pub caption: Option<&'static str>,
}

/// One entry of the rail.
#[derive(Clone, Debug)]
pub(super) struct Entry {
    /// A command id, or a rail-local id; what a click reports.
    pub id: &'static str,
    /// The lesson anchor of the entry, if it has one.
    pub anchor: Option<&'static str>,
    pub glyph: Glyph,
    pub label: String,
    pub tip: String,
    pub key: Option<String>,
    /// The tool in hand, or the panel that is open.
    pub active: bool,
    pub enabled: bool,
    pub badge: Option<Badge>,
    /// Non-empty: the entry has a flyout.
    pub flyout: Vec<FlyEntry>,
}

/// The entries of one rail, by zone. `history` is the foot cluster that
/// holds undo and redo until the top bar (lane V2a) takes them over.
#[derive(Clone, Debug, Default)]
pub(super) struct Rail {
    pub tools: Vec<Entry>,
    pub assists: Vec<Entry>,
    pub panels: Vec<Entry>,
    pub history: Vec<Entry>,
}

/// What a frame of the rail reports.
#[derive(Debug)]
pub(super) struct Shown {
    /// The entry or flyout row that was clicked or activated by the keyboard.
    pub clicked: Option<&'static str>,
    /// Where lesson anchors are: id, rect, clicked.
    pub spots: Vec<(&'static str, Rect, bool)>,
    /// The panel's own rect.
    pub rect: Rect,
    /// An entry whose flyout was asked for: the notch, a right click, or a
    /// click on the tool already in hand.
    pub asked: Option<&'static str>,
    /// The height the entries needed, for the fit test.
    pub content_height: f32,
}

fn flyout_id() -> egui::Id {
    egui::Id::new("rail_flyout")
}

/// The id of the flyout that is open, if any.
pub(super) fn open_flyout(ctx: &egui::Context) -> Option<String> {
    ctx.data(|d| d.get_temp::<String>(flyout_id()))
}

fn set_flyout(ctx: &egui::Context, id: Option<&str>) {
    ctx.data_mut(|d| {
        if let Some(id) = id {
            d.insert_temp(flyout_id(), id.to_owned());
        } else {
            d.remove::<String>(flyout_id());
        }
    });
}

/// Opens the flyout of entry `id` (an entry whose click is only to open it).
pub(super) fn open(ctx: &egui::Context, id: &str) {
    set_flyout(ctx, Some(id));
}

/// Closes any open flyout (a tool was picked elsewhere, the workspace
/// changed).
pub(super) fn close_flyout(ctx: &egui::Context) {
    set_flyout(ctx, None);
}

/// Opens the flyout of `id`, or closes it when it is the one open (an entry
/// whose click has nothing to run but its card).
pub(super) fn toggle_flyout(ctx: &egui::Context, id: &str) {
    let open = open_flyout(ctx).as_deref() == Some(id);
    set_flyout(ctx, if open { None } else { Some(id) });
}

/// Shows the rail as the left panel of `ui` and reports what happened.
pub(super) fn show(ui: &mut egui::Ui, rail: &Rail, expanded: &mut bool) -> Shown {
    let pal = ui_palette();
    // A card left open by another workspace's rail is shut, not carried over.
    if let Some(open) = open_flyout(ui.ctx())
        && !rail
            .tools
            .iter()
            .chain(&rail.assists)
            .chain(&rail.panels)
            .chain(&rail.history)
            .any(|e| e.id == open && !e.flyout.is_empty())
    {
        set_flyout(ui.ctx(), None);
    }
    let width = if *expanded { WIDTH_EXPANDED } else { WIDTH };
    let mut shown = Shown {
        clicked: None,
        spots: Vec::new(),
        rect: Rect::NOTHING,
        asked: None,
        content_height: 0.0,
    };
    let frame = egui::Frame::new()
        .fill(pal.panel)
        .stroke(Stroke::new(1.0, pal.border_subtle))
        .inner_margin(egui::Margin::symmetric(6, 6));
    let panel = egui::Panel::left("rail")
        .exact_size(width)
        .resizable(false)
        .frame(frame)
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
            let mut order: Vec<egui::Id> = Vec::new();
            let mut anchors_of_open: Option<(Rect, &Entry)> = None;
            let open = open_flyout(ui.ctx());
            // The foot first, from the bottom up, so the zones above it get
            // what is left; the zones scroll rather than clip on a short
            // window.
            let foot_height = ENTRY * (rail.history.len() as f32 + 1.0) + 8.0;
            let zones_height = (ui.available_height() - foot_height).max(ENTRY);
            let mut used = 0.0;
            egui::ScrollArea::vertical()
                .id_salt("rail_zones")
                .max_height(zones_height)
                .auto_shrink([false, false])
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
                .show(ui, |ui| {
                    let zones = [&rail.tools, &rail.assists, &rail.panels];
                    let mut first = true;
                    for zone in zones {
                        if zone.is_empty() {
                            continue;
                        }
                        if !first {
                            hairline(ui);
                            used += 9.0;
                        }
                        first = false;
                        for entry in zone {
                            let (rect, clicked) = entry_button(
                                ui,
                                entry,
                                *expanded,
                                open.as_deref(),
                                &mut order,
                                &mut shown.asked,
                            );
                            used += ENTRY + 2.0;
                            if let Some(a) = entry.anchor {
                                shown.spots.push((a, rect, clicked));
                            }
                            if clicked {
                                shown.clicked = Some(entry.id);
                            }
                            if !entry.flyout.is_empty() && open.as_deref() == Some(entry.id) {
                                anchors_of_open = Some((rect, entry));
                            }
                        }
                    }
                });
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                // Bottom up: the expand button is lowest.
                let tip = if *expanded {
                    "Collapse the rail"
                } else {
                    "Expand the rail: names and keys"
                };
                let toggle = Entry {
                    id: "rail.expand",
                    anchor: None,
                    glyph: Glyph::Phosphor(if *expanded {
                        novo_theme::icons::CARET_DOUBLE_LEFT
                    } else {
                        novo_theme::icons::CARET_DOUBLE_RIGHT
                    }),
                    label: if *expanded { "Collapse" } else { "Expand" }.to_owned(),
                    tip: tip.to_owned(),
                    key: None,
                    active: false,
                    enabled: true,
                    badge: None,
                    flyout: Vec::new(),
                };
                let (_, clicked) =
                    entry_button(ui, &toggle, *expanded, None, &mut order, &mut shown.asked);
                if clicked {
                    *expanded = !*expanded;
                }
                for entry in rail.history.iter().rev() {
                    let (rect, clicked) = entry_button(
                        ui,
                        entry,
                        *expanded,
                        open.as_deref(),
                        &mut order,
                        &mut shown.asked,
                    );
                    if let Some(a) = entry.anchor {
                        shown.spots.push((a, rect, clicked));
                    }
                    if clicked {
                        shown.clicked = Some(entry.id);
                    }
                }
                hairline(ui);
            });
            shown.content_height = used;
            arrows(ui, &order);
            if let Some((rect, entry)) = anchors_of_open {
                flyout(ui.ctx(), rect, entry, &mut shown);
            }
        });
    shown.rect = panel.response.rect;
    // A click on a flyout entry opens or closes it; any other pick closes
    // the open one.
    let ctx = ui.ctx().clone();
    if let Some(id) = shown.asked {
        let now_open = open_flyout(&ctx).as_deref() == Some(id);
        set_flyout(&ctx, if now_open { None } else { Some(id) });
    } else if let Some(id) = shown.clicked
        && id != "rail.expand"
        && !is_flyout_row(rail, id)
    {
        set_flyout(&ctx, None);
    }
    if open_flyout(&ctx).is_some() {
        let outside = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !shown.rect.contains(p))
        });
        let flyout_rect = ctx
            .memory(|m| m.area_rect(egui::Id::new("rail_flyout_card")))
            .unwrap_or(Rect::NOTHING);
        let on_card = ctx
            .input(|i| i.pointer.interact_pos())
            .is_some_and(|p| flyout_rect.contains(p));
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape))
            || (outside && !on_card)
        {
            set_flyout(&ctx, None);
        }
    }
    shown
}

fn is_flyout_row(rail: &Rail, id: &str) -> bool {
    rail.tools
        .iter()
        .chain(&rail.assists)
        .chain(&rail.panels)
        .chain(&rail.history)
        .any(|e| e.flyout.iter().any(|f| f.id == id))
}

fn hairline(ui: &mut egui::Ui) {
    let pal = ui_palette();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    let y = rect.center().y;
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + 4.0, y),
            egui::pos2(rect.right() - 4.0, y),
        ],
        Stroke::new(1.0, pal.border_subtle),
    );
}

/// Draws one entry; returns its rect and whether it was activated.
fn entry_button(
    ui: &mut egui::Ui,
    entry: &Entry,
    expanded: bool,
    open: Option<&str>,
    order: &mut Vec<egui::Id>,
    asked: &mut Option<&'static str>,
) -> (Rect, bool) {
    let pal = ui_palette();
    let width = ui.available_width();
    let size = Vec2::new(if expanded { width } else { ENTRY.min(width) }, ENTRY);
    let sense = if entry.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    order.push(response.id);
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, entry.enabled, &entry.label)
    });
    let opened = open == Some(entry.id);
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let on = entry.active || opened;
        let hovered = response.hovered() && entry.enabled;
        if on {
            painter.rect_filled(rect, 4, pal.accent_wash);
            // The copper bar on the rail's outer edge: the one copper thing.
            let bar = Rect::from_min_size(
                egui::pos2(rect.left() - 5.0, rect.top() + 6.0),
                Vec2::new(2.0, rect.height() - 12.0),
            );
            painter.rect_filled(bar, 1, pal.accent);
        } else if hovered {
            painter.rect_filled(rect, 4, pal.hover);
        }
        if pal.high_contrast {
            painter.rect_stroke(
                rect,
                4,
                Stroke::new(1.0, pal.border_control),
                egui::StrokeKind::Inside,
            );
        }
        let color = if !entry.enabled {
            pal.text_tertiary
        } else if on {
            pal.accent_hover
        } else if hovered {
            pal.text_strong
        } else {
            pal.text_secondary
        };
        let icon_rect = Rect::from_center_size(
            egui::pos2(rect.left() + ENTRY / 2.0, rect.center().y),
            Vec2::splat(ICON),
        );
        entry.glyph.paint(painter, icon_rect, color);
        if !entry.flyout.is_empty() {
            // The corner notch: a small triangle at the lower right of the
            // icon square.
            let corner = egui::pos2(rect.left() + ENTRY - 3.0, rect.bottom() - 3.0);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    corner,
                    corner - Vec2::new(6.0, 0.0),
                    corner - Vec2::new(0.0, 6.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
        if let Some(badge) = entry.badge.filter(|b| b.count > 0) {
            badge_pill(painter, icon_rect.right_top(), badge, ui);
        }
        if expanded {
            let font = egui::TextStyle::Button.resolve(ui.style());
            let text_color = if !entry.enabled {
                pal.text_tertiary
            } else if on {
                pal.text_strong
            } else {
                pal.text_primary
            };
            let x = rect.left() + ENTRY + 4.0;
            painter.text(
                egui::pos2(x, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &entry.label,
                font,
                text_color,
            );
            if let Some(key) = &entry.key {
                key_cap(painter, rect, key, ui);
            }
        }
        if response.has_focus() {
            // A heavy outline, not a colour: it reads in High contrast and
            // for people who do not see the copper.
            painter.rect_stroke(
                rect.expand(1.0),
                5,
                Stroke::new(2.0, pal.text_strong),
                egui::StrokeKind::Outside,
            );
        }
    }
    let response = if entry.tip.is_empty() {
        response
    } else {
        response.on_hover_text(&entry.tip)
    };
    // A plain click runs the tool, as the dock's button did. The flyout is
    // asked for by the corner notch, a right click, or a click on the tool
    // that is already in hand.
    let on_notch = response.interact_pointer_pos().is_some_and(|p| {
        Rect::from_min_max(
            egui::pos2(rect.left() + ENTRY - 14.0, rect.bottom() - 14.0),
            egui::pos2(rect.left() + ENTRY, rect.bottom()),
        )
        .contains(p)
    });
    let ask = !entry.flyout.is_empty()
        && entry.enabled
        && (response.secondary_clicked()
            || (response.clicked() && (entry.active || opened || on_notch)));
    if ask {
        *asked = Some(entry.id);
    }
    let clicked = response.clicked() && entry.enabled && !ask;
    if (clicked || ask) && ui.input(|i| i.pointer.primary_released()) {
        // The keys stay with the drawing after a click; keyboard use keeps
        // its focus.
        response.surrender_focus();
    }
    (rect, clicked)
}

fn badge_pill(painter: &egui::Painter, at: egui::Pos2, badge: Badge, ui: &egui::Ui) {
    let pal = ui_palette();
    let text = if badge.count > 99 {
        "99+".to_owned()
    } else {
        badge.count.to_string()
    };
    let fill = if badge.warn { pal.warning } else { pal.accent };
    let font = egui::FontId::proportional(10.0);
    let galley = painter.layout_no_wrap(text, font, pal.on_accent);
    let size = Vec2::new((galley.size().x + 6.0).max(14.0), 14.0);
    let pill = Rect::from_min_size(at + Vec2::new(-size.x + 6.0, -6.0), size);
    painter.rect_filled(pill, 7, fill);
    painter.galley(pill.center() - galley.size() / 2.0, galley, pal.on_accent);
    let _ = ui;
}

fn key_cap(painter: &egui::Painter, row: Rect, key: &str, ui: &egui::Ui) {
    let pal = ui_palette();
    let font = egui::TextStyle::Small.resolve(ui.style());
    let galley = painter.layout_no_wrap(key.to_owned(), font, pal.text_secondary);
    let size = galley.size() + Vec2::new(10.0, 2.0);
    let cap = Rect::from_min_size(
        egui::pos2(row.right() - size.x - 6.0, row.center().y - size.y / 2.0),
        size,
    );
    painter.rect_filled(cap, 3, pal.control);
    painter.rect_stroke(
        cap,
        3,
        Stroke::new(1.0, pal.border_subtle),
        egui::StrokeKind::Inside,
    );
    painter.galley(
        cap.center() - galley.size() / 2.0,
        galley,
        pal.text_secondary,
    );
}

/// Up and down move between entries while one has the keyboard.
fn arrows(ui: &mut egui::Ui, order: &[egui::Id]) {
    let Some(at) = order.iter().position(|id| ui.memory(|m| m.has_focus(*id))) else {
        return;
    };
    let step = ui.input_mut(|i| {
        if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
            Some(1)
        } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
            Some(-1)
        } else {
            None
        }
    });
    if let Some(step) = step {
        let to = (at as isize + step).clamp(0, order.len() as isize - 1) as usize;
        ui.memory_mut(|m| m.request_focus(order[to]));
    }
}

/// The flyout card beside `owner`.
fn flyout(ctx: &egui::Context, owner: Rect, entry: &Entry, shown: &mut Shown) {
    let pal = ui_palette();
    let mut picked = None;
    let mut order: Vec<egui::Id> = Vec::new();
    egui::Area::new(egui::Id::new("rail_flyout_card"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(owner.right() + 8.0, owner.top()))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style())
                .fill(pal.raised)
                .stroke(Stroke::new(1.0, pal.border_strong))
                .show(ui, |ui| {
                    ui.set_width(220.0);
                    ui.spacing_mut().item_spacing = Vec2::new(0.0, 2.0);
                    ui.label(
                        egui::RichText::new(&entry.label)
                            .strong()
                            .color(pal.text_strong),
                    );
                    for row in &entry.flyout {
                        if let Some(caption) = row.caption {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(caption)
                                    .small()
                                    .color(pal.text_secondary),
                            );
                        }
                        let (rect, clicked) = fly_row(ui, row, &mut order);
                        if let Some(a) = row.anchor {
                            shown.spots.push((a, rect, clicked));
                        }
                        if clicked {
                            picked = Some(row.id);
                        }
                    }
                    arrows(ui, &order);
                });
        });
    if let Some(id) = picked {
        shown.clicked = Some(id);
        set_flyout(ctx, None);
    }
}

fn fly_row(ui: &mut egui::Ui, row: &FlyEntry, order: &mut Vec<egui::Id>) -> (Rect, bool) {
    let pal = ui_palette();
    let size = Vec2::new(ui.available_width(), 28.0);
    let sense = if row.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);
    order.push(response.id);
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            row.enabled,
            row.current,
            &row.label,
        )
    });
    let painter = ui.painter();
    if row.current {
        painter.rect_filled(rect, 4, pal.accent_wash);
    } else if response.hovered() && row.enabled {
        painter.rect_filled(rect, 4, pal.hover);
    }
    if pal.high_contrast {
        painter.rect_stroke(
            rect,
            4,
            Stroke::new(1.0, pal.border_control),
            egui::StrokeKind::Inside,
        );
    }
    let color = if !row.enabled {
        pal.text_tertiary
    } else if row.current {
        pal.accent_hover
    } else {
        pal.text_primary
    };
    row.glyph.paint(
        painter,
        Rect::from_center_size(
            egui::pos2(rect.left() + 16.0, rect.center().y),
            Vec2::splat(16.0),
        ),
        color,
    );
    painter.text(
        egui::pos2(rect.left() + 32.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &row.label,
        egui::TextStyle::Button.resolve(ui.style()),
        color,
    );
    // The choice in force is a tick as well as a wash.
    if row.current {
        painter.text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            novo_theme::icons::CHECK,
            egui::FontId::proportional(14.0),
            color,
        );
    } else if let Some(key) = &row.key {
        key_cap(painter, rect, key, ui);
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.expand(1.0),
            5,
            Stroke::new(2.0, pal.text_strong),
            egui::StrokeKind::Outside,
        );
    }
    let response = if row.tip.is_empty() {
        response
    } else {
        response.on_hover_text(&row.tip)
    };
    let clicked = response.clicked() && row.enabled;
    if clicked && ui.input(|i| i.pointer.primary_released()) {
        response.surrender_focus();
    }
    (rect, clicked)
}
