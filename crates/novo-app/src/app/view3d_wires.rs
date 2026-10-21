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
use novo_core::{Project, StructureId, WireId, natural_cmp};
use novo_routing::model::Routed;
use novo_routing::{Kind, Problem};
use novo_view3d::scene::{ItemId, ItemKind};

use super::cabinet_routes::{DuctFill, Tone, duct_fills, tone};
use super::view3d_run_checks::{HotDevice, RunChecks, run_checks};
use super::view3d_runs::{Basis, EntryFacts, Undrawn};

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
    /// The ducts it lies in with their fill, in the order it passes them.
    pub duct_use: Vec<DuctUse>,
    /// What the checks say about this run, in words.
    pub problems: Vec<(Tone, String)>,
    /// Set when the wire is drawn to the cabinet entry point because its
    /// other end is outside the cabinet.
    pub entry: Option<EntryFacts>,
    /// How the length was found; read when a card needs it
    /// (`view3d_runs::ensure`).
    pub basis: Option<Basis>,
    /// The bend radius and heat neighbour checks (models, labelled so).
    pub checks: Option<RunChecks>,
}

/// One duct a wire lies in.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DuctUse {
    pub rail: u64,
    /// Where the duct is: `Duct at y 250 mm`, `Duct at x 100 mm`.
    pub place: String,
    /// Fill over the allowed fill (above 1.0 is overfull); `None` for a duct
    /// without a cross-section.
    pub ratio: Option<f64>,
    /// Share of the duct's cross-section the wires take, per cent.
    pub share_pct: Option<f64>,
    /// The allowed share, per cent.
    pub limit_pct: f64,
    /// Wires lying in it.
    pub wires: usize,
    pub tone: Tone,
}

impl DuctUse {
    /// `Duct 3: 31 % full (limit 50 %), 4 wires`.
    pub(super) fn text(&self) -> String {
        match (self.ratio, self.share_pct) {
            (Some(_), Some(share)) => format!(
                "{}: {:.0} % full of {:.0} % allowed, {} wires",
                self.place, share, self.limit_pct, self.wires
            ),
            _ => format!(
                "{}: no cross-section set, fill not checked, {} wires",
                self.place, self.wires
            ),
        }
    }

    /// Whether this duct needs the designer's eye.
    pub(super) fn is_problem(&self) -> bool {
        matches!(self.tone, Tone::Over | Tone::NoSize)
    }
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
        out.push(match &self.basis {
            Some(b) if b.length_mm.round() != self.length_mm.round() => format!(
                "Length {:.0} mm ({}), inside the cabinet {:.0} mm",
                b.length_mm, b.text, self.length_mm
            ),
            Some(b) => format!("Length {:.0} mm ({})", self.length_mm, b.text),
            None => format!("Length {:.0} mm", self.length_mm),
        });
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
        if let Some(e) = &self.entry {
            out.push(e.line());
        }
        for d in self.duct_use.iter().filter(|d| d.is_problem()) {
            out.push(d.text());
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
    structure: StructureId,
    routed: &Routed,
    on_plate: &BTreeSet<u64>,
    hot: &[HotDevice],
) -> BTreeMap<u64, WireFacts> {
    let plan = routed
        .plans
        .iter()
        .find(|(s, _)| *s == structure)
        .map(|(_, p)| p);
    let rules = novo_routing::WireRules::of(project);
    let limit_pct = rules.routing.fill_limit * 100.0;
    let ducts = novo_routing::model::ducts(project, structure);
    let fills: Vec<DuctFill> = plan
        .map(|p| duct_fills(project, structure, p))
        .unwrap_or_default();
    let entry = novo_routing::entry::entry_point(project, structure);
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
            let mut duct_use: Vec<DuctUse> = Vec::new();
            for rail in &route.ducts {
                if duct_use.iter().any(|d| d.rail == *rail) {
                    continue;
                }
                let Some(f) = fills.iter().find(|f| f.rail == *rail) else {
                    continue;
                };
                duct_use.push(DuctUse {
                    rail: *rail,
                    place: duct_place(f),
                    ratio: f.ratio,
                    share_pct: f.ratio.map(|r| r * limit_pct),
                    limit_pct,
                    wires: f.wires,
                    tone: tone(f),
                });
            }
            let mut problems = Vec::new();
            for p in plan.map(|p| p.problems.as_slice()).unwrap_or_default() {
                match p {
                    Problem::GuessedWire { job } if job == id => problems.push((
                        Tone::Busy,
                        "No wire size set: a 3 mm wire is assumed in the fill".into(),
                    )),
                    Problem::OverfullDuct { duct, ratio, jobs } if jobs.contains(id) => {
                        let place = fills
                            .iter()
                            .find(|f| f.rail == *duct)
                            .map_or_else(|| format!("Duct {duct}"), duct_place);
                        problems.push((
                            Tone::Over,
                            format!(
                                "{place} carries {:.0} % of what it may: move wires out of it",
                                ratio * 100.0
                            ),
                        ));
                    }
                    _ => {}
                }
            }
            let to_entry = routed.entry_legs.contains_key(id);
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
                    duct_use,
                    problems,
                    entry: to_entry.then(|| EntryFacts {
                        at: entry,
                        outside_mm: novo_routing::entry::outside_mm(project, structure),
                    }),
                    basis: None,
                    checks: Some(run_checks(project, wire, route, &ducts, hot, &rules)),
                },
            ))
        })
        .collect()
}

/// Where a duct is, in words a person can find on the plate: its middle line.
fn duct_place(f: &DuctFill) -> String {
    let (x, y, w, h) = f.rect;
    if w >= h {
        format!("Duct at y {:.0} mm", y + h / 2.0)
    } else {
        format!("Duct at x {:.0} mm", x + w / 2.0)
    }
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

/// The Wire runs list, as the body of the Wire runs panel
/// (`view3d_runs::panel`). `height` is the room the panel has, in points;
/// `compact` keeps the rows to a few lines so a Run card below them is in view.
pub(super) fn list_body(
    ui: &mut egui::Ui,
    height: f32,
    compact: bool,
    all: &BTreeMap<u64, WireFacts>,
    undrawn: &[Undrawn],
    selection: &BTreeSet<ItemId>,
) -> Asked {
    let mut asked = Asked::default();
    let chosen = selected(all, selection);
    ui.horizontal(|ui| {
        ui.strong("Wire runs");
        if ui.small_button("Close").clicked() {
            asked.close = true;
        }
    });
    if all.is_empty() {
        ui.weak("No wire is routed on this plate yet");
    }
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
    ui.separator();
    egui::ScrollArea::vertical()
        .id_salt("view3d_wire_runs_rows")
        .auto_shrink([false, true])
        .max_height(if compact {
            100.0
        } else {
            (height * 0.3).max(90.0)
        })
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
    if !undrawn.is_empty() {
        ui.separator();
        egui::CollapsingHeader::new(format!("Not drawn ({})", undrawn.len()))
            .default_open(false)
            .show(ui, |ui| {
                ui.weak("No path in the picture: how long each is and why");
                egui::ScrollArea::vertical()
                    .id_salt("view3d_wire_runs_undrawn")
                    .max_height((height * 0.2).max(60.0))
                    .show(ui, |ui| {
                        for u in undrawn {
                            ui.label(format!("{}   {:.0} mm, {}", u.name, u.length_mm, u.text))
                                .on_hover_text(u.why);
                        }
                    });
            });
    }
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
            duct_use: Vec::new(),
            problems: Vec::new(),
            entry: None,
            basis: None,
            checks: None,
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
