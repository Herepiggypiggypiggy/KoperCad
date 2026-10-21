//! The building blocks of the Cabinet toolbar (docs/design/cabinet-toolbar.md):
//! a captioned group, an icon-plus-label button, the one copper primary and
//! the count pills of the status strip. Colours are `Palette` roles; sizes
//! come from the theme's control height.

use eframe::egui;
use novo_theme::{Palette, fonts, radius};

/// Width of the bar under which the group captions drop.
pub(super) const WIDE: f32 = 1100.0;

/// What may fold into More, in the order it folds: the first entry goes
/// first. Place all, Route all, the panel picker and the Plate/Table switch
/// never fold. Labels are never shortened: an item either fits or folds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Foldable {
    Export,
    Heat,
    CutList,
    Rails,
    Arrange,
    Rules,
}

/// Room kept free at the right edge so a group never touches the pane edge:
/// the measured label widths run short of the laid-out bar by up to about 70
/// points at 1366 (the More button stood 6 points past the window).
const MARGIN: f32 = 90.0;

/// Which items of the bar fit and which wait in More. Decided from the
/// measured widths of the real labels and the width of this pane, so a wide
/// pane folds nothing and a narrow one folds only what it must, never more
/// than a fixed breakpoint would.
pub(super) struct Fold {
    folded: Vec<Foldable>,
}

impl Fold {
    /// `fixed` is the width of what never folds; `items` are the foldable
    /// items that exist right now with their widths. `more` is the width of
    /// the More button, needed as soon as one item folds.
    pub(super) fn plan(avail: f32, fixed: f32, more: f32, items: &[(Foldable, f32)]) -> Self {
        let mut shown: Vec<(Foldable, f32)> = items.to_vec();
        let order = [
            Foldable::Export,
            Foldable::Heat,
            Foldable::CutList,
            Foldable::Rails,
            Foldable::Arrange,
            Foldable::Rules,
        ];
        let total = |shown: &[(Foldable, f32)], any_folded: bool| {
            fixed
                + MARGIN
                + if any_folded { more } else { 0.0 }
                + shown.iter().map(|i| i.1).sum::<f32>()
        };
        let mut folded = Vec::new();
        for f in order {
            if total(&shown, !folded.is_empty()) <= avail {
                break;
            }
            if let Some(at) = shown.iter().position(|i| i.0 == f) {
                shown.remove(at);
                folded.push(f);
            }
        }
        Self { folded }
    }

    /// Item `f` waits in More.
    pub(super) fn has(&self, f: Foldable) -> bool {
        self.folded.contains(&f)
    }

    /// Anything waits in More.
    pub(super) fn any(&self) -> bool {
        !self.folded.is_empty()
    }
}

/// Width of `text` in the button font.
pub(super) fn text_width(ui: &egui::Ui, text: &str) -> f32 {
    let font = egui::TextStyle::Button.resolve(ui.style());
    ui.painter()
        .layout_no_wrap(text.to_owned(), font, egui::Color32::PLACEHOLDER)
        .size()
        .x
}

/// Width [`button`] takes with this icon and label, with the gap after it.
pub(super) fn button_width(ui: &egui::Ui, icon: &str, label: &str) -> f32 {
    text_width(ui, &format!("{icon}  {label}"))
        + 2.0 * ui.spacing().button_padding.x
        + ui.spacing().item_spacing.x
}

/// Width one group takes besides its buttons: the gap before it and the
/// spacing after it.
pub(super) const GROUP_GAP: f32 = 8.0 + 8.0;

/// Group captions are never smaller than this (design review).
const CAPTION: f32 = 12.0;

/// A named group of buttons: the caption in small caps above (none once the
/// bar is folded), the buttons in a row below.
pub(super) fn group(ui: &mut egui::Ui, caption: Option<&str>, add: impl FnOnce(&mut egui::Ui)) {
    let p = Palette::of(ui);
    ui.add_space(8.0);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        if let Some(caption) = caption {
            ui.label(
                egui::RichText::new(caption.to_uppercase())
                    .size(CAPTION)
                    .color(p.text_tertiary),
            );
        }
        ui.horizontal(add);
    });
}

/// A bar button: icon, then label. `selected` is a toggle that is on.
pub(super) fn button(ui: &mut egui::Ui, icon: &str, label: &str, selected: bool) -> egui::Response {
    let text = egui::RichText::new(format!("{icon}  {label}"));
    ui.add(egui::Button::new(text).selected(selected))
}

/// One half of the Plate | Table switch.
pub(super) fn segment(ui: &mut egui::Ui, label: &str, selected: bool) -> egui::Response {
    ui.add(egui::Button::new(label).selected(selected))
}

/// The one copper control of the bar: icon, label and the key, at the size
/// of every other control (only the fill and the weight set it apart). When it
/// cannot be used it is drawn as a plain button so the position stays and
/// the bar never shows two copper fills.
pub(super) fn primary(
    ui: &mut egui::Ui,
    icon: &str,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
) -> egui::Response {
    let p = Palette::of(ui);
    let text = format!("{icon}  {label}");
    if !enabled {
        let mut b = egui::Button::new(text);
        if let Some(s) = shortcut {
            b = b.shortcut_text(s);
        }
        return ui.add_enabled(false, b);
    }
    let mut b = egui::Button::new(
        egui::RichText::new(text)
            .family(fonts::strong_family())
            .color(p.on_accent),
    )
    .fill(p.accent)
    .stroke(egui::Stroke::NONE);
    if let Some(s) = shortcut {
        b = b.shortcut_text(egui::RichText::new(s).color(p.on_accent));
    }
    ui.add(b)
}

/// A count in the status strip. `level` colours it when it needs a look.
pub(super) fn pill(
    ui: &mut egui::Ui,
    text: &str,
    level: Option<novo_theme::widgets::Level>,
) -> egui::Response {
    let p = Palette::of(ui);
    let (fill, color) = match level {
        Some(l) => (l.wash(p), l.color(p)),
        None => (p.control, p.text_secondary),
    };
    let r = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(radius::PILL))
        .inner_margin(egui::Margin::symmetric(9, 2))
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).size(12.0).color(color));
        })
        .response;
    // A pill that opens a list answers the pointer like a button.
    r.interact(egui::Sense::click())
}

/// A control or panel of the Cabinet a lesson may point at; reported to the
/// learning layer once the frame has released its borrow of the project.
#[derive(Clone)]
pub(super) struct Anchor {
    pub(super) id: &'static str,
    pub(super) label: &'static str,
    pub(super) rect: egui::Rect,
    pub(super) clicked: bool,
}

fn sink() -> egui::Id {
    egui::Id::new("cabinet_lesson_anchors")
}

/// Remembers `rect` as the anchor `id` from code that has the context but
/// not the frame's list: windows, panels and the cards of the inspector.
/// [`take_noted`] collects them at the end of the frame.
pub(super) fn note_rect(
    ctx: &egui::Context,
    id: &'static str,
    label: &'static str,
    rect: egui::Rect,
    clicked: bool,
) {
    ctx.data_mut(|d| {
        d.get_temp_mut_or_default::<Vec<Anchor>>(sink())
            .push(Anchor {
                id,
                label,
                rect,
                clicked,
            });
    });
}

/// [`note_rect`] for a response, with its click.
pub(super) fn note_ctx(
    ctx: &egui::Context,
    id: &'static str,
    label: &'static str,
    response: &egui::Response,
) {
    note_rect(ctx, id, label, response.rect, response.clicked());
}

/// The anchors noted with [`note_rect`] since the last call.
pub(super) fn take_noted(ctx: &egui::Context) -> Vec<Anchor> {
    ctx.data_mut(|d| d.remove_temp::<Vec<Anchor>>(sink()))
        .unwrap_or_default()
}

/// Remembers `response` as the anchor `id` and hands it back.
pub(super) fn note(
    anchors: &mut Vec<Anchor>,
    id: &'static str,
    label: &'static str,
    response: &egui::Response,
) -> egui::Response {
    anchors.push(Anchor {
        id,
        label,
        rect: response.rect,
        clicked: response.clicked(),
    });
    response.clone()
}

#[cfg(test)]
mod tests {
    use super::{Fold, Foldable};

    fn items() -> Vec<(Foldable, f32)> {
        vec![
            (Foldable::Rules, 80.0),
            (Foldable::Rails, 130.0),
            (Foldable::CutList, 80.0),
            (Foldable::Export, 90.0),
            (Foldable::Arrange, 260.0),
            (Foldable::Heat, 80.0),
        ]
    }

    #[test]
    fn the_bar_keeps_room_at_the_right_edge() {
        // 1366 wide, as the layout measured it: items that fit with only a few
        // points to spare used to run the More button past the window.
        let f = Fold::plan(1300.0, 600.0, 90.0, &items());
        let shown: f32 = items()
            .iter()
            .filter(|i| !f.has(i.0))
            .map(|i| i.1)
            .sum::<f32>();
        assert!(600.0 + shown + 90.0 + 80.0 <= 1300.0, "{shown}");
    }

    #[test]
    fn a_wide_pane_folds_nothing() {
        let f = Fold::plan(2000.0, 600.0, 90.0, &items());
        assert!(!f.any());
    }

    #[test]
    fn items_fold_one_at_a_time_in_the_agreed_order() {
        // Everything is 720 wide; 600 fixed + 16 margin leaves 104 for items.
        let f = Fold::plan(1250.0, 600.0, 90.0, &items());
        assert!(f.has(Foldable::Export) && f.has(Foldable::Heat));
        assert!(!f.has(Foldable::Rules) && !f.has(Foldable::Arrange));
        // A little narrower and the next in line folds, not everything.
        let g = Fold::plan(1150.0, 600.0, 90.0, &items());
        assert!(g.has(Foldable::CutList));
        assert!(!g.has(Foldable::Rules));
    }

    #[test]
    fn rules_fold_last_and_the_fixed_part_never_does() {
        let f = Fold::plan(10.0, 600.0, 90.0, &items());
        for i in items() {
            assert!(f.has(i.0));
        }
    }
}
