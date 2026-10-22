//! The openings of the Heat panel in the project file: one text property on
//! the enclosure holding them as JSON, so they save with the project, travel
//! in a copy and undo with it. No model change: a structure property is what
//! the layout already uses for the enclosure's interior and door.

use novo_core::{Project, StructureId, Value};

use super::heat_settings::VentSpec;

/// The structure property that holds the openings.
pub(super) const KEY: &str = "heat.openings";

/// The text the openings are stored as; empty for none.
pub(super) fn text(vents: &[VentSpec]) -> String {
    if vents.is_empty() {
        String::new()
    } else {
        serde_json::to_string(vents).unwrap_or_default()
    }
}

/// What the project holds for `id` as text; empty for none.
pub(super) fn stored_text(project: &Project, id: StructureId) -> String {
    match project
        .get::<novo_core::Structure>(id)
        .and_then(|s| s.properties.get(KEY))
    {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    }
}

/// The openings in `stored`; a damaged text gives none (the file still opens).
pub(super) fn parse(stored: &str) -> Vec<VentSpec> {
    if stored.trim().is_empty() {
        return Vec::new();
    }
    serde_json::from_str(stored).unwrap_or_default()
}

/// Stores `text` (empty clears the property) on the enclosure.
pub(super) fn write(
    tx: &mut novo_core::Tx<'_>,
    id: StructureId,
    text: &str,
) -> novo_core::Result<()> {
    let value = (!text.is_empty()).then(|| Value::Text(text.to_owned()));
    tx.set_property(id, KEY, value)
}

impl super::NovoApp {
    /// Keeps the Heat panel's openings and the project's copy of them equal,
    /// once a frame before the 3D view reads either: a change made in the
    /// panel is written when the gesture ends (one undo step in the project);
    /// a change made to the project (opened, undone there) is read back.
    pub(super) fn sync_openings(&mut self, ctx: &eframe::egui::Context) {
        // A drag or a typed value is one step; it ends when nothing is held.
        if !ctx.input(|i| i.pointer.any_down()) && ctx.memory(|m| m.focused().is_none()) {
            self.cabinet.heat.settings.end_edit();
        }
        let Some(document) = self.backend.document() else {
            return;
        };
        let Some(id) = super::view3d::enclosure(document.project(), self.cabinet.location.as_ref())
        else {
            return;
        };
        let held = stored_text(document.project(), id);
        let settings = &mut self.cabinet.heat.settings;
        let known = settings
            .stored
            .as_ref()
            .filter(|(s, _)| *s == id)
            .map(|(_, t)| t.clone());
        // Another enclosure (a second cabinet, another project) or the first
        // look: the project is the truth, never our list.
        let Some(known) = known else {
            settings.load(parse(&held));
            settings.stored = Some((id, held));
            return;
        };
        let now = text(&settings.vents);
        if known == held {
            // The project is as we last knew it: our side may have moved on.
            if now != held && !settings.is_editing() {
                if let Some(doc) = self.backend.document_mut() {
                    let _ = doc.transact("Vents and fans", |tx| write(tx, id, &now));
                }
                self.cabinet.heat.settings.stored = Some((id, now));
            }
        } else {
            // The project changed under us (opened, undone).
            settings.load(parse(&held));
            settings.stored = Some((id, held));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_settings::{Kind, Wall};

    #[test]
    fn openings_round_trip_through_text() {
        let mut fan = VentSpec::new(Wall::Door);
        fan.kind = Kind::FilterFan;
        fan.u_mm = 321.5;
        let mut g = VentSpec::new(Wall::Right);
        g.free_fraction = 0.55;
        let t = text(&[fan, g]);
        assert_eq!(parse(&t), vec![fan, g]);
        assert_eq!(text(&[]), "");
        assert!(parse("").is_empty());
    }

    #[test]
    fn a_damaged_or_newer_text_does_not_stop_the_project_opening() {
        assert!(parse("{not json").is_empty());
        // A field this version does not know, and one it lacks, are fine.
        let v = parse(r#"[{"kind":"Fan","wall":"Top","future":1}]"#);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, Kind::Fan);
        assert_eq!(v[0].wall, Wall::Top);
        assert_eq!(v[0].width_mm, VentSpec::new(Wall::Top).width_mm);
    }
}
