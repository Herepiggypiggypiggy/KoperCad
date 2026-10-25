//! Build order in the 3D view: the cabinet shown after each step of the
//! workshop's build order, with a strip at the bottom to step through it.
//! The order is `novo_layout::assembly::steps` (the one the printed assembly
//! pack lists); the picture side is `novo_view3d::assembly`. There is no
//! animation: a step is a plain change of the picture, so Reduce motion has
//! nothing to switch off.

use eframe::egui;
use novo_core::{Device, DeviceId, Project, StructureId};
use novo_layout::assembly::{Phase, steps};
use novo_view3d::assembly::Step;
use novo_view3d::{ItemId, ItemKind};

/// Margin from the view's edge, in points.
const MARGIN: f32 = 12.0;
/// Widest the step text gets before it wraps, in points.
const TEXT_WIDTH: f32 = 520.0;

fn id() -> egui::Id {
    egui::Id::new("view3d_build_order")
}

/// How much of the view's height the strip covers at the bottom, as a
/// fraction, for the fit to leave clear. Before it has been drawn once, a
/// typical height.
pub(super) fn inset(ctx: &egui::Context, view: egui::Rect, on: bool) -> f32 {
    if !on {
        return 0.0;
    }
    let covered = ctx
        .memory(|m| m.area_rect(id()))
        .map_or(96.0, |r| view.bottom() - r.top());
    (covered / view.height().max(1.0)).clamp(0.0, 0.35)
}

/// The steps of one cabinet: rails, ducts and devices in the workshop order,
/// then a closing step for the wires. Labels are left out (nothing to see).
pub(super) fn sequence(project: &Project, enclosure: StructureId) -> Vec<Step> {
    let Some(layout) = novo_layout::model::load(project, enclosure) else {
        return Vec::new();
    };
    let name = |id: u64| {
        project
            .get::<Device>(DeviceId::from(id))
            .map(|d| d.tag.to_string())
            .unwrap_or_else(|| format!("device {id}"))
    };
    let mut out: Vec<Step> = steps(&layout, &name, &[])
        .into_iter()
        .filter_map(|s| {
            let kind = match s.phase {
                Phase::Rails => ItemKind::Rail,
                Phase::Ducts => ItemKind::Duct,
                Phase::Devices => ItemKind::Device,
                Phase::Wires | Phase::Labels => return None,
            };
            Some(Step {
                part: s.subject.map(|raw| ItemId::new(kind, raw)),
                mark: s.mark,
                text: s.text,
            })
        })
        .collect();
    out.push(Step {
        part: None,
        mark: "Wires".to_owned(),
        text: "Run the wires. Parts the order does not name appear here too.".to_owned(),
    });
    out
}

/// Words for the closing step once the picture knows how many wires it can
/// draw (those with both ends on the plate and a path).
pub(super) fn closing_text(drawn: usize) -> String {
    if drawn == 0 {
        "Run the wires. None is drawn here: no wire has both ends on this plate yet. \
         Place the devices and use Route all to see them."
            .to_owned()
    } else {
        format!(
            "Run the wires: the {drawn} wire{} drawn in blue. Parts the order does not name \
             appear here too.",
            if drawn == 1 { " is" } else { "s are" }
        )
    }
}

/// The strip: where we are, what the step does, and the way forward and back.
/// `position` is 0 (shell and plate) to `steps.len()`.
pub(super) fn strip(
    ctx: &egui::Context,
    view: egui::Rect,
    position: &mut Option<usize>,
    steps: &[Step],
) -> bool {
    let mut print = false;
    let Some(mut at) = *position else {
        return false;
    };
    if steps.is_empty() {
        // No layout to read an order from: the whole cabinet stays in view,
        // and the strip says why there is nothing to step through.
        let p = crate::theme::ui_palette();
        let mut leave = false;
        egui::Area::new(id())
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::CENTER_BOTTOM)
            .fixed_pos(egui::pos2(view.center().x, view.bottom() - MARGIN))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "Build order: this cabinet has no layout to read an order from. \
                                 Place devices on its plate first.",
                            )
                            .color(p.text_secondary),
                        );
                        leave = ui.button("Show all").clicked();
                    });
                });
            });
        if leave {
            *position = None;
        }
        return false;
    }
    at = at.min(steps.len());
    let p = crate::theme::ui_palette();
    let mut leave = false;
    egui::Area::new(id())
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(view.center().x, view.bottom() - MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(TEXT_WIDTH.min(view.width() - 2.0 * MARGIN));
                let (title, text) = match at.checked_sub(1).and_then(|i| steps.get(i)) {
                    Some(step) => (
                        format!("Step {at} of {}: {}", steps.len(), step.mark),
                        step.text.as_str(),
                    ),
                    None => (
                        format!("Start (0 of {})", steps.len()),
                        "The enclosure and the mounting plate (the plate is blue). Next puts on the first part.",
                    ),
                };
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(at > 0, |ui| {
                        if ui.button("Previous").clicked() {
                            at -= 1;
                        }
                    });
                    ui.label(egui::RichText::new(title).strong().color(p.text_strong));
                    ui.add_enabled_ui(at < steps.len(), |ui| {
                        if ui.button("Next").clicked() {
                            at += 1;
                        }
                    });
                    if ui
                        .button("Show all")
                        .on_hover_text("Leave the build order and show the whole cabinet")
                        .clicked()
                    {
                        leave = true;
                    }
                    if ui
                        .button("Work cards (PDF)")
                        .on_hover_text(super::WORKPACK_HINT)
                        .clicked()
                    {
                        print = true;
                    }
                });
                ui.label(egui::RichText::new(text).color(p.text_secondary));
                ui.horizontal(|ui| {
                    let mut pos = at;
                    ui.spacing_mut().slider_width = 220.0;
                    if ui
                        .add(egui::Slider::new(&mut pos, 0..=steps.len()).show_value(false))
                        .on_hover_text("Drag to any step")
                        .changed()
                    {
                        at = pos;
                    }
                    ui.label(
                        egui::RichText::new("The part this step adds is shown in blue.")
                            .small()
                            .color(p.text_secondary),
                    );
                });
            });
        });
    *position = if leave { None } else { Some(at) };
    print
}

/// The next position for a command: steps from the start when it is off.
pub(super) fn step_by(position: Option<usize>, last: usize, forward: bool) -> Option<usize> {
    match position {
        None => Some(0),
        Some(at) if forward => Some((at + 1).min(last)),
        Some(at) => Some(at.saturating_sub(1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_closing_step_says_how_many_wires_are_drawn_or_why_none_is() {
        assert!(closing_text(0).contains("None is drawn"));
        assert!(closing_text(0).contains("Route all"));
        assert!(closing_text(1).contains("the 1 wire is drawn in blue"));
        assert!(closing_text(12).contains("the 12 wires are drawn in blue"));
    }

    #[test]
    fn commands_step_from_the_start_and_stop_at_both_ends() {
        assert_eq!(step_by(None, 5, true), Some(0), "opens at the start");
        assert_eq!(step_by(Some(0), 5, false), Some(0));
        assert_eq!(step_by(Some(2), 5, true), Some(3));
        assert_eq!(step_by(Some(5), 5, true), Some(5));
        assert_eq!(step_by(Some(5), 5, false), Some(4));
    }
}
