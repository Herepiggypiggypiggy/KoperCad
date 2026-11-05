//! The openings of the Heat panel in the project file: one text property on
//! the enclosure holding them as JSON, so they save with the project, travel
//! in a copy and undo with it. No model change: a structure property is what
//! the layout already uses for the enclosure's interior and door.

use novo_core::{Project, StructureId, Value};

use super::heat_settings::VentSpec;
use novo_thermal::gaps::Seal;

/// What is stored: the bare list (no seal, as before this version) or, once a
/// door seal is set, an object holding both.
#[derive(serde::Deserialize)]
#[serde(untagged)]
enum Stored {
    List(Vec<VentSpec>),
    Both {
        #[serde(default)]
        vents: Vec<VentSpec>,
        #[serde(default)]
        seal: Option<Seal>,
    },
}

/// The structure property that holds the openings.
pub(super) const KEY: &str = "heat.openings";

/// The text the openings are stored as; empty for none.
pub(super) fn text(vents: &[VentSpec], seal: Option<Seal>) -> String {
    if vents.is_empty() && seal.is_none() {
        return String::new();
    }
    // No NaN in the text: it would be stored as null and lose the list.
    let clean: Vec<VentSpec> = vents.iter().map(|v| v.finite()).collect();
    match seal {
        None => serde_json::to_string(&clean).unwrap_or_default(),
        Some(seal) => serde_json::json!({ "vents": clean, "seal": seal }).to_string(),
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
pub(super) fn parse(stored: &str) -> (Vec<VentSpec>, Option<Seal>) {
    if stored.trim().is_empty() {
        return (Vec::new(), None);
    }
    match serde_json::from_str(stored) {
        Ok(Stored::List(v)) => (v, None),
        Ok(Stored::Both { vents, seal }) => (vents, seal),
        Err(_) => (Vec::new(), None),
    }
}

/// True when `stored` holds something but no list can be read from it: it is
/// left alone until the person adds an opening, instead of being cleared.
pub(super) fn is_damaged(stored: &str) -> bool {
    !stored.trim().is_empty() && serde_json::from_str::<Stored>(stored).is_err()
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
    /// once a frame before the 3D view reads either. Driven by the 3D view (called when it is drawn): a change made on the
    /// panel is written when the gesture ends, and a project that changed under
    /// the panel (opened, undone, another enclosure) is read back. The project
    /// is the truth for an enclosure the panel has not read yet.
    pub(super) fn sync_openings(&mut self, ctx: &eframe::egui::Context) {
        // A drag or a typed value is one step; it ends when nothing is held.
        if !ctx.input(|i| i.pointer.any_down()) && ctx.memory(|m| m.focused().is_none()) {
            self.cabinet.heat.settings.end_edit();
        }
        self.sync_openings_now();
    }

    /// The reading and writing of [`Self::sync_openings`], for a caller that
    /// has no frame (the Cooling advice adding a pair from the Cabinet).
    pub(super) fn sync_openings_now(&mut self) {
        self.sync_inside_fans_now();
        self.sync_shell_now();
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
            let (v, seal) = parse(&held);
            settings.load(v, seal);
            settings.stored = Some((id, held));
            return;
        };
        let now = text(&settings.vents, settings.seal);
        if known == held {
            // The project is as we last knew it: our side may have moved on.
            let keep_damaged = now.is_empty() && is_damaged(&held);
            if now != held && !settings.is_editing() && !keep_damaged {
                if let Some(doc) = self.backend.document_mut() {
                    let _ = doc.transact("Vents and fans", |tx| write(tx, id, &now));
                }
                self.cabinet.heat.settings.stored = Some((id, now));
            }
        } else {
            // The project changed under us (opened, undone).
            let (v, seal) = parse(&held);
            settings.load(v, seal);
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
        let t = text(&[fan, g], None);
        assert_eq!(parse(&t), (vec![fan, g], None));
        assert_eq!(text(&[], None), "");
        assert!(parse("").0.is_empty());
    }

    #[test]
    fn the_door_seal_and_a_filter_survive_a_round_trip_and_old_files_still_open() {
        let mut g = VentSpec::new(Wall::Left);
        g.filter = Some(novo_thermal::gaps::FilterClass::G4);
        g.filter_load = 0.4;
        let seal = Seal {
            ip: novo_thermal::gaps::IpClass::Ip65,
            glands: 5,
        };
        let t = text(&[g], Some(seal));
        assert_eq!(parse(&t), (vec![g], Some(seal)));
        // The seal alone, with no openings, is kept too.
        assert_eq!(parse(&text(&[], Some(seal))), (vec![], Some(seal)));
        // A list from before filters and seals has neither.
        let old = parse(r#"[{"kind":"Grille","wall":"Top"}]"#);
        assert_eq!(old.0[0].filter, None);
        assert_eq!(old.1, None);
    }

    #[test]
    fn a_damaged_or_newer_text_does_not_stop_the_project_opening() {
        assert!(parse("{not json").0.is_empty());
        // A field this version does not know, and one it lacks, are fine.
        let (v, _) = parse(r#"[{"kind":"Fan","wall":"Top","future":1}]"#);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].kind, Kind::Fan);
        assert_eq!(v[0].wall, Wall::Top);
        assert_eq!(v[0].width_mm, VentSpec::new(Wall::Top).width_mm);
    }

    #[test]
    fn a_nan_is_not_stored_and_a_damaged_text_is_kept_until_something_is_added() {
        let mut v = VentSpec::new(Wall::Top);
        v.u_mm = f64::NAN;
        let t = text(&[v], None);
        assert!(!t.contains("null"), "{t}");
        assert_eq!(parse(&t).0.len(), 1);
        assert!(is_damaged("{not json"));
        assert!(!is_damaged("") && !is_damaged("[]") && !is_damaged(&t));
    }
}
