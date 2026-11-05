//! One "?" per window: explanation text does not sit in a window body, it is
//! handed to [`add`] and shown in a popup behind the "?" in the window's header,
//! next to the dock and fold buttons.
//!
//! A window's content calls `window_help::add(ui, "text")` (any number of
//! times; paragraphs are joined). The header of the window that is being built
//! reads the text of the previous frame, so the "?" appears one frame after the
//! window opens. Honesty caveats (model estimate, not validated) go through the
//! same call and stay reachable there, next to a tiny inline marker.

use eframe::egui;

fn current_id() -> egui::Id {
    egui::Id::new("window_help_current")
}

fn building_id(name: &str) -> egui::Id {
    egui::Id::new(("window_help_building", name))
}

fn shown_id(name: &str) -> egui::Id {
    egui::Id::new(("window_help_shown", name))
}

/// Builds the window `name`: help added inside `f` belongs to it.
pub(super) fn scope<R>(ctx: &egui::Context, name: &str, f: impl FnOnce() -> R) -> R {
    let previous: Option<String> = ctx.data(|d| d.get_temp(current_id()));
    ctx.data_mut(|d| {
        d.insert_temp(current_id(), name.to_owned());
        d.insert_temp(building_id(name), Vec::<String>::new());
    });
    let out = f();
    let built: Vec<String> = ctx
        .data_mut(|d| d.remove_temp(building_id(name)))
        .unwrap_or_default();
    let text = built.join("\n\n");
    let before: Option<String> = ctx.data(|d| d.get_temp(shown_id(name)));
    if before.as_deref().unwrap_or("") != text {
        ctx.data_mut(|d| {
            if text.is_empty() {
                d.remove_temp::<String>(shown_id(name));
            } else {
                d.insert_temp(shown_id(name), text);
            }
        });
        ctx.request_repaint();
    }
    ctx.data_mut(|d| {
        match previous {
            Some(p) => {
                d.insert_temp(current_id(), p);
            }
            None => {
                d.remove_temp::<String>(current_id());
            }
        };
    });
    out
}

/// Adds a paragraph to the "?" of the window being built. With no window
/// around (a strip, a canvas overlay) nothing is kept: use [`mark`] there.
pub(super) fn add(ctx: &egui::Context, text: impl Into<String>) {
    let text = text.into();
    if text.is_empty() {
        return;
    }
    let Some(name) = ctx.data(|d| d.get_temp::<String>(current_id())) else {
        return;
    };
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<String>>(building_id(&name))
            .push(text)
    });
}

/// The help text of the window `name` as of its last frame.
pub(super) fn text(ctx: &egui::Context, name: &str) -> Option<String> {
    ctx.data(|d| d.get_temp(shown_id(name)))
}

/// A "?" with `text` in a popup, for a strip or card that has no header.
pub(super) fn mark(ui: &mut egui::Ui, text: &str) {
    novo_theme::widgets::help_mark(ui, text);
}

/// The "?" in the header of a docked pane, at `rect`.
pub(super) fn header_button(ui: &mut egui::Ui, rect: egui::Rect, text: &str) {
    ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
        novo_theme::widgets::help_mark(ui, text);
    });
}

fn open_id(name: &str) -> egui::Id {
    egui::Id::new(("window_help_open", name))
}

/// The "?" in the title bar of a floating window, painted at `rect` on the
/// window's layer and hit-tested by hand like the dock buttons. `true` when
/// the pointer is over it (the caller shows nothing else there).
pub(super) fn title_button(
    ctx: &egui::Context,
    layer: egui::LayerId,
    name: &str,
    rect: egui::Rect,
    text: &str,
) {
    let pal = crate::theme::ui_palette();
    let pointer = ctx.input(|i| i.pointer.hover_pos());
    let clicked = ctx.input(|i| i.pointer.primary_clicked());
    let hovered = pointer.is_some_and(|p| rect.contains(p))
        && pointer.and_then(|p| ctx.layer_id_at(p)) == Some(layer);
    let open = ctx
        .data(|d| d.get_temp::<bool>(open_id(name)))
        .unwrap_or(false);
    let painter = ctx.layer_painter(layer);
    if hovered {
        painter.rect_filled(rect, 4.0, pal.hover);
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        novo_theme::icons::QUESTION,
        egui::FontId::proportional(14.0),
        if hovered || open {
            pal.text_strong
        } else {
            pal.text_secondary
        },
    );
    if clicked && hovered {
        ctx.data_mut(|d| d.insert_temp(open_id(name), !open));
    } else if open {
        let area = egui::Area::new(egui::Id::new(("window_help_popup", name)))
            .order(egui::Order::Tooltip)
            .fixed_pos(rect.left_bottom() + egui::vec2(0.0, 4.0))
            .constrain(true)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(320.0);
                    novo_theme::widgets::help_text(ui, text);
                });
            });
        let outside = clicked && !area.response.contains_pointer();
        if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ctx.data_mut(|d| d.insert_temp(open_id(name), false));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_added_while_a_window_is_built_is_kept_for_its_header() {
        let ctx = egui::Context::default();
        assert_eq!(text(&ctx, "w"), None);
        scope(&ctx, "w", || {
            add(&ctx, "first");
            add(&ctx, "");
            add(&ctx, "second");
        });
        assert_eq!(text(&ctx, "w").as_deref(), Some("first\n\nsecond"));
        // A window that stops explaining loses its "?".
        scope(&ctx, "w", || {});
        assert_eq!(text(&ctx, "w"), None);
    }

    #[test]
    fn nested_windows_keep_their_own_help_and_outside_help_is_dropped() {
        let ctx = egui::Context::default();
        add(&ctx, "nobody's");
        scope(&ctx, "outer", || {
            add(&ctx, "outer text");
            scope(&ctx, "inner", || add(&ctx, "inner text"));
            add(&ctx, "outer again");
        });
        assert_eq!(
            text(&ctx, "outer").as_deref(),
            Some("outer text\n\nouter again")
        );
        assert_eq!(text(&ctx, "inner").as_deref(), Some("inner text"));
    }
}
