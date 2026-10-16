//! Wire runs in the 3D view: the wires are drawn along the routes the Cabinet
//! and the wire list use (through the ducts, rail to rail), and here each one
//! can be read. Hover a wire for its number, ends and length; click it (or a
//! row of the Wire runs list) to select it and see the total of the
//! selection. The lengths are the routing engine's own, the figure the wire
//! list shows as the cut length (a service loop at each end included), so the
//! workshop can cut a wire straight from what the placement says.
//!
//! Where the user finds it: hover a wire in the 3D view; View > 3D > Wire
//! runs (Ctrl+K "wire runs") opens the list of every wire on the plate.

use std::collections::{BTreeMap, BTreeSet};

use eframe::egui;
use novo_core::{Project, WireId, natural_cmp};
use novo_routing::Kind;
use novo_routing::model::Routed;
use novo_view3d::scene::{ItemId, ItemKind};

/// Margin from the view's edge, in points.
const MARGIN: f32 = 12.0;

/// What one routed wire comes to.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct WireFacts {
    pub id: u64,
    /// The wire number, empty when the wire is not numbered yet.
    pub number: String,
    pub from: String,
    pub to: String,
    pub colour: Option<String>,
    pub section_mm2: Option<f64>,
    /// Cut length with the loops at both ends, whole millimetres up.
    pub length_mm: f64,
    pub bends: usize,
    /// How many ducts it lies in.
    pub ducts: usize,
    /// No duct reaches both ends: a straight line, flagged by the checks.
    pub free_air: bool,
    /// A device depth is unknown, so the length is too short by the rise to
    /// the duct.
    pub flat: bool,
}

impl WireFacts {
    /// The name shown: the wire number, else its ends.
    pub(super) fn name(&self) -> String {
        if self.number.is_empty() {
            format!("{} → {}", self.from, self.to)
        } else {
            self.number.clone()
        }
    }

    /// The lines of the hover card.
    pub(super) fn lines(&self) -> Vec<String> {
        let mut out = vec![format!("{} → {}", self.from, self.to)];
        out.push(format!("Length {:.0} mm", self.length_mm));
        let mut how = if self.free_air {
            "No duct reaches both ends: straight line".to_owned()
        } else if self.ducts == 0 {
            "On the plate".to_owned()
        } else if self.ducts == 1 {
            "Through 1 duct".to_owned()
        } else {
            format!("Through {} ducts", self.ducts)
        };
        if self.bends > 0 {
            how.push_str(&format!(", {} bends", self.bends));
        }
        out.push(how);
        let mut kind = Vec::new();
        if let Some(c) = &self.colour {
            kind.push(c.clone());
        }
        if let Some(s) = self.section_mm2 {
            kind.push(format!("{s} mm²"));
        }
        if !kind.is_empty() {
            out.push(kind.join("  "));
        }
        if self.flat {
            out.push("A device depth is unknown: the length is a little short".to_owned());
        }
        out
    }
}

/// The wires routed on the plate of an enclosure, by wire id.
pub(super) fn facts(
    project: &Project,
    routed: &Routed,
    on_plate: &BTreeSet<u64>,
) -> BTreeMap<u64, WireFacts> {
    routed
        .routes
        .iter()
        .filter(|(id, _)| on_plate.contains(id))
        .filter_map(|(id, route)| {
            let wire = project.wire(WireId::from(*id))?;
            let pin = |e: &novo_core::Endpoint| {
                e.as_pin()
                    .and_then(|p| project.pin_designation(p))
                    .unwrap_or_default()
            };
            Some((
                *id,
                WireFacts {
                    id: *id,
                    number: wire.number.clone().unwrap_or_default(),
                    from: pin(&wire.from),
                    to: pin(&wire.to),
                    colour: wire.color.clone(),
                    section_mm2: wire.cross_section_mm2,
                    length_mm: route.length_mm.ceil(),
                    bends: route.bends,
                    ducts: route.ducts.len(),
                    free_air: route.kind == Kind::FreeAir,
                    flat: routed.unknown_height.contains(id),
                },
            ))
        })
        .collect()
}

/// The facts in list order: wire number, natural order, unnumbered last.
pub(super) fn sorted(all: &BTreeMap<u64, WireFacts>) -> Vec<&WireFacts> {
    let mut rows: Vec<&WireFacts> = all.values().collect();
    rows.sort_by(|a, b| {
        a.number
            .is_empty()
            .cmp(&b.number.is_empty())
            .then_with(|| natural_cmp(&a.number, &b.number))
            .then(a.id.cmp(&b.id))
    });
    rows
}

/// The wires among the selected items.
pub(super) fn selected<'a>(
    all: &'a BTreeMap<u64, WireFacts>,
    selection: &BTreeSet<ItemId>,
) -> Vec<&'a WireFacts> {
    selection
        .iter()
        .filter(|i| i.kind == ItemKind::Wire)
        .filter_map(|i| all.get(&i.raw))
        .collect()
}

/// Metres with two decimals, from millimetres.
pub(super) fn metres(mm: f64) -> String {
    format!("{:.2} m", mm / 1000.0)
}

/// The hover card of a wire, beside the pointer.
pub(super) fn hover_card(ctx: &egui::Context, at: egui::Pos2, f: &WireFacts) {
    egui::Area::new(egui::Id::new("view3d_wire_hover"))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .fixed_pos(at + egui::vec2(16.0, 16.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.strong(f.name());
                for (n, line) in f.lines().iter().enumerate() {
                    if n == 1 {
                        ui.label(line);
                    } else {
                        ui.weak(line);
                    }
                }
            });
        });
}

/// What the Wire runs list asked for this frame.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Asked {
    pub select: Option<u64>,
    pub close: bool,
    pub cut_list: bool,
}

/// The Wire runs list, left side of the view.
pub(super) fn list(
    ctx: &egui::Context,
    view: egui::Rect,
    all: &BTreeMap<u64, WireFacts>,
    selection: &BTreeSet<ItemId>,
) -> Asked {
    let mut asked = Asked::default();
    let chosen = selected(all, selection);
    egui::Area::new(egui::Id::new("view3d_wire_runs"))
        .order(egui::Order::Middle)
        .pivot(egui::Align2::LEFT_CENTER)
        .fixed_pos(egui::pos2(view.left() + MARGIN, view.center().y))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(250.0);
                ui.horizontal(|ui| {
                    ui.strong("Wire runs");
                    if ui.small_button("Close").clicked() {
                        asked.close = true;
                    }
                });
                if all.is_empty() {
                    ui.weak("No wire is routed on this plate yet");
                }
                egui::ScrollArea::vertical()
                    .max_height((view.height() * 0.5).max(120.0))
                    .show(ui, |ui| {
                        for f in sorted(all) {
                            let on = chosen.iter().any(|c| c.id == f.id);
                            let text = format!("{}   {:.0} mm", f.name(), f.length_mm);
                            let row = ui
                                .selectable_label(on, text)
                                .on_hover_text(f.lines().join("\n"));
                            if row.clicked() {
                                asked.select = Some(f.id);
                            }
                        }
                    });
                ui.separator();
                let total: f64 = all.values().map(|f| f.length_mm).sum();
                ui.weak(format!("{} wires, {} in all", all.len(), metres(total)));
                if !chosen.is_empty() {
                    let sum: f64 = chosen.iter().map(|f| f.length_mm).sum();
                    ui.label(format!("Selected {}: {}", chosen.len(), metres(sum)));
                }
                if ui
                    .small_button("Open the cut list")
                    .on_hover_text("The Cut list report: every wire to cut, by size and colour")
                    .clicked()
                {
                    asked.cut_list = true;
                }
            });
        });
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: u64, number: &str, length: f64) -> WireFacts {
        WireFacts {
            id,
            number: number.into(),
            from: "-F1:2".into(),
            to: "-K1:1".into(),
            colour: Some("BK".into()),
            section_mm2: Some(2.5),
            length_mm: length,
            bends: 2,
            ducts: 1,
            free_air: false,
            flat: false,
        }
    }

    fn all() -> BTreeMap<u64, WireFacts> {
        [f(1, "W10", 400.0), f(2, "W2", 150.0), f(3, "", 320.0)]
            .into_iter()
            .map(|w| (w.id, w))
            .collect()
    }

    #[test]
    fn the_list_is_in_wire_number_order_with_unnumbered_last() {
        let a = all();
        let order: Vec<u64> = sorted(&a).iter().map(|w| w.id).collect();
        assert_eq!(order, vec![2, 1, 3], "W2 before W10, unnumbered last");
    }

    #[test]
    fn a_wire_is_named_by_its_number_else_its_ends() {
        assert_eq!(f(1, "W10", 1.0).name(), "W10");
        assert_eq!(f(3, "", 1.0).name(), "-F1:2 → -K1:1");
    }

    #[test]
    fn the_card_gives_ends_length_route_and_kind() {
        let lines = f(1, "W10", 412.0).lines();
        assert_eq!(lines[0], "-F1:2 → -K1:1");
        assert_eq!(lines[1], "Length 412 mm");
        assert_eq!(lines[2], "Through 1 duct, 2 bends");
        assert_eq!(lines[3], "BK  2.5 mm²");
    }

    #[test]
    fn free_air_and_flat_wires_say_so() {
        let mut w = f(1, "W1", 100.0);
        w.free_air = true;
        w.flat = true;
        let lines = w.lines();
        assert!(lines[2].starts_with("No duct reaches both ends"));
        assert!(lines.last().unwrap().contains("depth is unknown"));
    }

    #[test]
    fn the_selection_sums_only_the_wires_on_this_plate() {
        let a = all();
        let sel: BTreeSet<ItemId> = [
            ItemId::new(ItemKind::Wire, 1),
            ItemId::new(ItemKind::Wire, 2),
            ItemId::new(ItemKind::Wire, 99),
            ItemId::new(ItemKind::Device, 3),
        ]
        .into();
        let chosen = selected(&a, &sel);
        assert_eq!(chosen.len(), 2);
        let total: f64 = chosen.iter().map(|w| w.length_mm).sum();
        assert_eq!(metres(total), "0.55 m");
    }
}
