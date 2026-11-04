//! A maker's mounting plate for the panel, picked in the Cabinet inspector.
//!
//! Plate parts come from the Parts window: Rittal's STEP model (downloaded
//! from PARTcommunity by the user) is read at import and the part carries
//! its size and the holes it is delivered with (`novo_parts::body`). Here
//! the plate card lists those parts, "Use" gives the panel that size and
//! remembers the holes, and the hand drilling plan draws them as rings and
//! flags a planned hole that lands on one.
//!
//! Reach it: Cabinet, Plate setup (or click an empty spot of the plate),
//! Inspector, "Maker plate".

use eframe::egui;
use novo_core::Value;
use novo_parts::body::keys;
use novo_parts::{Catalog, CatalogPart};
use novo_theme::widgets;

use super::cabinet::Ask;

/// One mounting plate part the user has imported.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct PlateChoice {
    /// `Rittal 8806000`, as shown and stored with the panel.
    pub label: String,
    pub width: f64,
    pub height: f64,
    /// The `x,y,d; ...` text of the holes the plate has, when it has any.
    pub holes: Option<String>,
    pub hole_count: usize,
}

/// The plate parts of the user's catalog, loaded when the plate card opens.
#[derive(Debug, Default)]
pub(super) struct MakerPlates {
    loaded: bool,
    list: Vec<PlateChoice>,
    picked: usize,
}

fn number(part: &CatalogPart, key: &str) -> Option<f64> {
    match part.data.get(key) {
        Some(Value::Float(v)) => Some(*v),
        Some(Value::Int(v)) => Some(*v as f64),
        _ => None,
    }
}

/// The mounting plates among the catalog's parts, by label.
pub(super) fn choices(catalog: &Catalog) -> Vec<PlateChoice> {
    let mut out: Vec<PlateChoice> = catalog
        .parts()
        .iter()
        .filter(|p| {
            matches!(p.data.get(keys::BODY_KIND), Some(Value::Text(t)) if t == "mounting_plate")
        })
        .filter_map(|p| {
            let (width, height) = (
                number(p, keys::PLATE_WIDTH_MM)?,
                number(p, keys::PLATE_HEIGHT_MM)?,
            );
            let holes = match p.data.get(keys::FACTORY_HOLES) {
                Some(Value::Text(t)) if !t.trim().is_empty() => Some(t.clone()),
                _ => None,
            };
            Some(PlateChoice {
                label: format!("{} {}", p.manufacturer, p.order_number),
                width,
                height,
                hole_count: holes
                    .as_deref()
                    .map_or(0, |h| novo_parts::body::parse_factory_holes(h).len()),
                holes,
            })
        })
        .collect();
    out.sort_by(|a, b| a.label.cmp(&b.label));
    out
}

impl MakerPlates {
    /// Reads the user's catalog the first time after [`MakerPlates::reset`].
    pub(super) fn ensure(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        self.list = Catalog::load_default()
            .map(|c| choices(&c))
            .unwrap_or_default();
        self.picked = self.picked.min(self.list.len().saturating_sub(1));
    }

    /// Forget the list: the next [`MakerPlates::ensure`] reads it again
    /// (the card was closed, or a part was imported meanwhile).
    pub(super) fn reset(&mut self) {
        self.loaded = false;
    }
}

/// The "Maker plate" section of the plate card. `current` is the plate part
/// stored with the panel.
pub(super) fn show(
    ui: &mut egui::Ui,
    plates: &mut MakerPlates,
    structure: novo_core::StructureId,
    current: Option<&str>,
) -> Option<Ask> {
    let mut ask = None;
    widgets::collapsing_section(
        ui,
        "cabinet_maker_plate",
        "Maker plate",
        &plates.list.len().to_string(),
        current.is_some() || !plates.list.is_empty(),
        |ui| {
            if let Some(c) = current {
                widgets::field(ui, "In use", c);
            }
            if plates.list.is_empty() {
                ui.weak("No plate parts yet. Import a mounting plate's STEP model in Parts.");
                return;
            }
            let shown = plates.picked.min(plates.list.len() - 1);
            egui::ComboBox::from_id_salt("cabinet_maker_plate_pick")
                .selected_text(format!(
                    "{}  {} x {}",
                    plates.list[shown].label, plates.list[shown].width, plates.list[shown].height
                ))
                .show_ui(ui, |ui| {
                    for (i, c) in plates.list.iter().enumerate() {
                        ui.selectable_value(
                            &mut plates.picked,
                            i,
                            format!("{}  {} x {} mm", c.label, c.width, c.height),
                        );
                    }
                });
            let c = &plates.list[plates.picked.min(plates.list.len() - 1)];
            if ui
                .button(format!(
                    "Use {} x {} mm ({} hole{})",
                    c.width,
                    c.height,
                    c.hole_count,
                    if c.hole_count == 1 { "" } else { "s" }
                ))
                .on_hover_text(
                    "Sets the plate size and remembers the holes it comes with (one undo step)",
                )
                .clicked()
            {
                ask = Some(Ask::UsePlatePart(structure, c.clone()));
            }
        },
    );
    ask
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plate(order: &str, w: f64, holes: Option<&str>) -> CatalogPart {
        let mut p = CatalogPart::new("Rittal", order);
        p.data
            .insert(keys::BODY_KIND.into(), Value::Text("mounting_plate".into()));
        p.data.insert(keys::PLATE_WIDTH_MM.into(), Value::Float(w));
        p.data
            .insert(keys::PLATE_HEIGHT_MM.into(), Value::Float(700.0));
        if let Some(h) = holes {
            p.data
                .insert(keys::FACTORY_HOLES.into(), Value::Text(h.into()));
        }
        p
    }

    #[test]
    fn only_mounting_plate_parts_are_offered_sorted_by_label() {
        let mut c = Catalog::new();
        c.upsert(plate("8806500", 500.0, Some("25,25,9; 475,25,9")));
        c.upsert(plate("8806000", 400.0, None));
        c.upsert(CatalogPart::new("Rittal", "1500510"));
        let list = choices(&c);
        assert_eq!(
            list.iter().map(|p| p.label.as_str()).collect::<Vec<_>>(),
            ["Rittal 8806000", "Rittal 8806500"]
        );
        assert_eq!((list[1].width, list[1].hole_count), (500.0, 2));
        assert_eq!(list[0].holes, None);
    }
}
