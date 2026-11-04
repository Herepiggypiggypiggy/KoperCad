//! Choosing a real enclosure when a cabinet is made.
//!
//! The "New cabinet" window lists enclosures as cards: the enclosures the
//! user imported in Parts (a maker's STEP model read at import), the user's
//! own enclosure list, and shipped catalogue rows (none ship until the
//! maker's terms are checked, `handoff/cabinet-start/DESIGN.md`). Choosing a
//! card gives the cabinet an [`EnclosureSpec`] with the source of every
//! number, saved with the cabinet, so heat, door and reports read the same
//! enclosure (`handoff/cabinet-start/CHAIN.md`).
//!
//! Reach it: Outline, New cabinet, "Enclosure".

use eframe::egui::{self, Sense, Stroke, Vec2};
use novo_core::{MountingPlate, Value};
use novo_layout::sizing::EnclosureSize;
use novo_layout::spec::{CatalogueRow, EnclosureSpec, Field, Source};
use novo_parts::Catalog;
use novo_parts::body::keys;

use crate::theme::ui_palette;

/// Plate border when the enclosure does not say.
const DEFAULT_MARGIN_MM: f64 = 10.0;
/// Plate smaller than the inside by this much per side when the plate size
/// is not known (mounting plates stand off the walls); shown as a default.
const PLATE_GAP_W_MM: f64 = 100.0;
const PLATE_GAP_H_MM: f64 = 200.0;

/// One enclosure the user can pick.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Choice {
    /// Heading of the card.
    pub title: String,
    /// Where the card comes from, in words.
    pub origin: &'static str,
    pub spec: EnclosureSpec,
    pub plate: MountingPlate,
    /// The plate size is a default, not the maker's.
    pub plate_is_default: bool,
    /// The name in the user's enclosure list, kept as the old property.
    pub list_name: Option<String>,
}

impl Choice {
    pub fn size_text(&self) -> String {
        let d = |f: &Option<Field>| f.map_or("?".to_owned(), |f| format!("{}", f.value));
        format!(
            "{} x {} x {} mm",
            d(&self.spec.width),
            d(&self.spec.height),
            d(&self.spec.depth)
        )
    }

    fn matches(&self, query: &str) -> bool {
        let q = query.trim().to_lowercase();
        q.is_empty()
            || self.title.to_lowercase().contains(&q)
            || self.spec.article.to_lowercase().contains(&q)
            || self.size_text().contains(&q)
    }
}

fn number(part: &novo_parts::CatalogPart, key: &str) -> Option<f64> {
    match part.data.get(key) {
        Some(Value::Float(v)) => Some(*v),
        Some(Value::Int(v)) => Some(*v as f64),
        _ => None,
    }
}

/// Enclosures among the user's imported parts.
pub(super) fn from_parts(catalog: &Catalog) -> Vec<Choice> {
    let mut out: Vec<Choice> = catalog
        .parts()
        .iter()
        .filter(|p| matches!(p.data.get(keys::BODY_KIND), Some(Value::Text(t)) if t == "enclosure"))
        .filter_map(|p| {
            let (w, h, d) = (
                number(p, keys::OUTER_WIDTH_MM)?,
                number(p, keys::OUTER_HEIGHT_MM)?,
                number(p, keys::OUTER_DEPTH_MM)?,
            );
            let c = |v: f64| Some(Field::new(v, Source::Catalogue));
            Some(Choice {
                title: format!("{} {}", p.manufacturer, p.order_number),
                origin: "Your parts",
                spec: EnclosureSpec {
                    article: p.order_number.clone(),
                    series: p.type_number.clone(),
                    width: c(w),
                    height: c(h),
                    depth: c(d),
                    ..EnclosureSpec::default()
                },
                plate: MountingPlate {
                    width: (w - PLATE_GAP_W_MM).max(0.0),
                    height: (h - PLATE_GAP_H_MM).max(0.0),
                    margin: DEFAULT_MARGIN_MM,
                },
                plate_is_default: true,
                list_name: None,
            })
        })
        .collect();
    out.sort_by(|a, b| a.title.cmp(&b.title));
    out
}

/// The user's own list of sizes (typed by the user).
pub(super) fn from_list(items: &[EnclosureSize]) -> Vec<Choice> {
    items
        .iter()
        .map(|e| {
            let t = |v: f64| Some(Field::new(v, Source::Typed));
            Choice {
                title: e.name.clone(),
                origin: "Your list",
                spec: EnclosureSpec {
                    width: t(e.width),
                    height: t(e.height),
                    depth: t(e.depth),
                    ..EnclosureSpec::default()
                },
                plate: MountingPlate {
                    width: e.plate_width,
                    height: e.plate_height,
                    margin: e.margin,
                },
                plate_is_default: false,
                list_name: Some(e.name.clone()),
            }
        })
        .collect()
}

/// Shipped catalogue rows: none until the maker's terms are checked.
pub(super) fn from_rows(rows: &[CatalogueRow]) -> Vec<Choice> {
    rows.iter()
        .map(|r| Choice {
            title: format!("{} {}", r.series, r.article),
            origin: "Catalogue",
            spec: r.spec(),
            plate: MountingPlate {
                width: (r.width - PLATE_GAP_W_MM).max(0.0),
                height: (r.height - PLATE_GAP_H_MM).max(0.0),
                margin: DEFAULT_MARGIN_MM,
            },
            plate_is_default: true,
            list_name: None,
        })
        .collect()
}

/// Every card, in the order shown.
pub(super) fn all(catalog: Option<&Catalog>, list: &[EnclosureSize]) -> Vec<Choice> {
    let mut out = catalog.map(from_parts).unwrap_or_default();
    out.extend(from_rows(&[]));
    out.extend(from_list(list));
    out
}

/// What the picker keeps between frames.
#[derive(Debug, Default, Clone)]
pub(super) struct Picker {
    pub query: String,
    /// Index into the choices; `None` is the standard plate.
    pub picked: Option<usize>,
}

/// The cards. Returns true when the pick changed.
pub(super) fn show(ui: &mut egui::Ui, picker: &mut Picker, choices: &[Choice]) -> bool {
    let p = ui_palette();
    let before = picker.picked;
    ui.add(
        egui::TextEdit::singleline(&mut picker.query)
            .desired_width(f32::INFINITY)
            .hint_text("Search: 8455, 600 x 1800"),
    );
    ui.add_space(6.0);
    let mut shown = 0;
    egui::ScrollArea::vertical()
        .id_salt("enclosure_pick")
        .max_height(180.0)
        .show(ui, |ui| {
            let width = ui.available_width();
            for (i, c) in choices.iter().enumerate() {
                if !c.matches(&picker.query) {
                    continue;
                }
                shown += 1;
                let selected = picker.picked == Some(i);
                let (rect, response) =
                    ui.allocate_exact_size(Vec2::new(width, 52.0), Sense::click());
                let fill = if selected {
                    p.hover
                } else if response.hovered() {
                    p.control
                } else {
                    p.sunken
                };
                let stroke = if selected {
                    Stroke::new(1.5, p.border_strong)
                } else {
                    Stroke::new(1.0, p.border_subtle)
                };
                ui.painter()
                    .rect(rect, 6.0, fill, stroke, egui::StrokeKind::Inside);
                let text_at = rect.min + Vec2::new(12.0, 8.0);
                ui.painter().text(
                    text_at,
                    egui::Align2::LEFT_TOP,
                    &c.title,
                    egui::FontId::proportional(14.0),
                    p.text_strong,
                );
                ui.painter().text(
                    text_at + Vec2::new(0.0, 20.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}  ·  {}", c.size_text(), c.origin),
                    egui::FontId::proportional(12.0),
                    p.text_secondary,
                );
                if response.clicked() {
                    picker.picked = if selected { None } else { Some(i) };
                }
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::SelectableLabel,
                        true,
                        selected,
                        c.title.clone(),
                    )
                });
            }
        });
    if shown == 0 {
        ui.weak(if choices.is_empty() {
            "No enclosures yet"
        } else {
            "Nothing matches"
        });
    }
    picker.picked != before
}

/// The words next to a value: a catalogue number that came from the user's own
/// parts says so.
fn words(c: &Choice, f: &Field) -> &'static str {
    if f.source == Source::Catalogue && c.origin == "Your parts" {
        "from your parts"
    } else {
        f.source.words()
    }
}

/// One line per value saying where it came from (behind the "?").
pub(super) fn state_lines(c: &Choice) -> Vec<String> {
    let mut out = Vec::new();
    let mut line = |name: &str, f: &Option<Field>| {
        if let Some(f) = f {
            out.push(format!("{name} {} mm, {}", f.value, words(c, f)));
        }
    };
    line("Width", &c.spec.width);
    line("Height", &c.spec.height);
    line("Depth", &c.spec.depth);
    line("Wall", &c.spec.wall);
    out.push(format!(
        "Plate {} x {} mm, {}",
        c.plate.width,
        c.plate.height,
        if c.plate_is_default {
            Source::Default.words()
        } else {
            "your list"
        }
    ));
    out
}

/// The one line shown for a pick: size and plate.
pub(super) fn summary(c: &Choice) -> String {
    format!(
        "{}  ·  plate {} x {} mm",
        c.size_text(),
        c.plate.width,
        c.plate.height
    )
}

/// What the "?" says: the estimate sentence (when it applies) and where each
/// value came from.
pub(super) fn detail(c: &Choice) -> String {
    let mut text = String::new();
    if let Some(notice) = c.spec.notice() {
        text.push_str(notice);
        text.push_str("\n\n");
    }
    text.push_str(&state_lines(c).join("\n"));
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_parts::CatalogPart;

    fn part(order: &str, kind: &str, w: f64) -> CatalogPart {
        let mut p = CatalogPart {
            manufacturer: "Rittal".into(),
            order_number: order.into(),
            ..CatalogPart::default()
        };
        p.data
            .insert(keys::BODY_KIND.into(), Value::Text(kind.into()));
        p.data.insert(keys::OUTER_WIDTH_MM.into(), Value::Float(w));
        p.data
            .insert(keys::OUTER_HEIGHT_MM.into(), Value::Float(1800.0));
        p.data
            .insert(keys::OUTER_DEPTH_MM.into(), Value::Float(500.0));
        p
    }

    #[test]
    fn only_enclosure_parts_become_cards_with_catalogue_numbers() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        catalog.upsert(part("8806000", "mounting_plate", 500.0));
        let cards = from_parts(&catalog);
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0].title, "Rittal 8000");
        assert_eq!(cards[0].spec.width.unwrap().source, Source::Catalogue);
        assert!(cards[0].plate_is_default);
        assert_eq!(
            (cards[0].plate.width, cards[0].plate.height),
            (500.0, 1600.0)
        );
    }

    #[test]
    fn list_entries_are_typed_and_keep_their_plate() {
        let e = EnclosureSize {
            name: "Mine".into(),
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            plate_width: 500.0,
            plate_height: 700.0,
            margin: 5.0,
            material: novo_layout::sizing::Material::Steel,
        };
        let cards = from_list(&[e]);
        assert_eq!(cards[0].spec.depth.unwrap().source, Source::Typed);
        assert_eq!(cards[0].plate.margin, 5.0);
        assert!(!cards[0].plate_is_default);
        assert_eq!(cards[0].list_name.as_deref(), Some("Mine"));
    }

    #[test]
    fn search_finds_by_article_and_size() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8455600", "enclosure", 600.0));
        let cards = from_parts(&catalog);
        assert!(cards[0].matches("8455"));
        assert!(cards[0].matches("600 x 1800"));
        assert!(!cards[0].matches("9999"));
    }

    #[test]
    fn the_detail_leads_with_the_estimate_sentence_and_the_summary_is_one_line() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        let c = &from_parts(&catalog)[0];
        assert!(detail(c).starts_with("Estimate from catalogue data"));
        assert!(!summary(c).contains('\n'));
        assert!(summary(c).contains("plate 500 x 1600"));
    }

    #[test]
    fn a_list_entry_keeps_the_typed_words() {
        let e = EnclosureSize {
            name: "Mine".into(),
            width: 600.0,
            height: 800.0,
            depth: 250.0,
            plate_width: 500.0,
            plate_height: 700.0,
            margin: 5.0,
            material: novo_layout::sizing::Material::Steel,
        };
        assert!(state_lines(&from_list(&[e])[0])[0].contains("typed by you"));
    }

    #[test]
    fn state_lines_say_where_each_number_came_from() {
        let mut catalog = Catalog::default();
        catalog.upsert(part("8000", "enclosure", 600.0));
        let lines = state_lines(&from_parts(&catalog)[0]);
        assert!(lines[0].contains("from your parts"));
        assert!(lines.last().unwrap().contains("default"));
    }
}
