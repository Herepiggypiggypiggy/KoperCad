//! The size of the shown surface, edited in the Cabinet inspector: width,
//! height and margin (and, for a profile, where its corner is) of the door,
//! a wall, the roof, the floor, an extra plate or a profile. The value is the
//! `surface.<name>` property of the cabinet's structure that novo-layout
//! reads; a surface with no such property uses planning values from the
//! enclosure, and "Planning values" removes the property again. Reach it:
//! Cabinet, Surface group, pick a surface other than the plate, nothing
//! selected, Inspector.

use eframe::egui;
use novo_core::{AnyId, Project, StructureId, Value};
use novo_layout::surfaces::{SurfaceKind, SurfaceSize, size_key};
use novo_theme::widgets;

use super::cabinet::Ask;
use super::cabinet_surfaces::SurfaceSummary;

/// The widest a surface dimension can be set to, in mm.
const MAX_MM: f64 = 5000.0;
/// The smallest width or height, in mm.
const MIN_MM: f64 = 20.0;

/// What the card shows for one surface.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct SurfaceCard {
    pub structure: StructureId,
    pub name: String,
    /// "Door", "Roof", ...
    pub kind: &'static str,
    /// What is in force: the stored size, or the planning values.
    pub size: SurfaceSize,
    /// A `surface.<name>` property is stored.
    pub stored: bool,
    /// A profile has no planning values: its corner is part of its size.
    pub profile: bool,
}

/// The card for the shown surface `name`; `None` for the mounting plate
/// (which has its own form) and for a surface that is not there.
pub(super) fn card(
    project: &Project,
    structure: StructureId,
    summary: &SurfaceSummary,
    name: &str,
) -> Option<SurfaceCard> {
    let row = summary.row(name)?;
    let kind = SurfaceKind::from_name(name)?;
    if kind == SurfaceKind::Plate && name == novo_layout::surfaces::MAIN_PLATE {
        return None;
    }
    let stored_text = project
        .get::<novo_core::Structure>(structure)
        .and_then(|s| match s.properties.get(&size_key(name)) {
            Some(Value::Text(t)) => Some(t.clone()),
            _ => None,
        });
    let stored = stored_text.as_deref().and_then(SurfaceSize::parse);
    let profile = kind == SurfaceKind::Profile;
    Some(SurfaceCard {
        structure,
        name: name.to_owned(),
        kind: row.kind,
        size: SurfaceSize {
            width: row.plate.width,
            height: row.plate.height,
            margin: row.plate.margin,
            // A profile always carries its corner; the others only when the
            // stored size gave one.
            origin: stored
                .and_then(|s| s.origin)
                .or(profile.then_some(row.origin)),
        },
        stored: stored_text.is_some(),
        profile,
    })
}

/// Draws the card; returns the ask when something was changed.
pub(super) fn show(ui: &mut egui::Ui, card: &SurfaceCard) -> Option<Ask> {
    let mut ask = None;
    // "roof (mm)", but "plate plate2 (mm)": the name only when it adds to the kind.
    let title = if card.kind == card.name {
        card.name.clone()
    } else {
        format!("{} {}", card.kind, card.name)
    };
    widgets::section_label(ui, &format!("{title} (mm)"));
    ui.weak("Select a device to see its data");
    ui.add_space(6.0);
    let mut size = card.size;
    let mut changed = false;
    for (label, value, lo, hi) in [
        ("Width", &mut size.width, MIN_MM, MAX_MM),
        ("Height", &mut size.height, MIN_MM, MAX_MM),
        ("Margin", &mut size.margin, 0.0, 200.0),
    ] {
        ui.horizontal(|ui| {
            ui.label(label);
            changed |= ui.add(egui::DragValue::new(value).range(lo..=hi)).changed();
        });
    }
    if let Some(origin) = size.origin.as_mut() {
        for (label, value) in ["Corner x", "Corner y", "Corner z"]
            .into_iter()
            .zip(origin.iter_mut())
        {
            ui.horizontal(|ui| {
                ui.label(label);
                changed |= ui
                    .add(egui::DragValue::new(value).range(-MAX_MM..=MAX_MM))
                    .changed();
            });
        }
    }
    if changed {
        ask = Some(Ask::SetSurfaceSize(
            card.structure,
            card.name.clone(),
            Some(size),
        ));
    }
    ui.add_space(6.0);
    if card.stored {
        if !card.profile
            && ui
                .button("Planning values")
                .on_hover_text("Forget the stored size and use the cabinet's own")
                .clicked()
        {
            ask = Some(Ask::SetSurfaceSize(card.structure, card.name.clone(), None));
        }
    } else if !card.profile {
        ui.weak("Planning values from the cabinet. Change a number to store a size.");
    }
    ask
}

/// Stores `size` as the size of surface `name` of `structure`, or removes the
/// stored size for `None`. One edit, so one undo step.
pub(super) fn set(
    tx: &mut novo_core::Tx<'_>,
    structure: StructureId,
    name: &str,
    size: Option<SurfaceSize>,
) -> novo_core::Result<()> {
    let value = size.map(|s| Value::Text(s.format()));
    tx.set_property(AnyId::Structure(structure), size_key(name), value)
}
