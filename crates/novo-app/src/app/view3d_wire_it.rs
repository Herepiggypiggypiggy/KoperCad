//! Wire-it in the 3D view: the wiring done one connection at a time, in the
//! bench sequence of `novo_reports::wireit`. The strip at the bottom names the
//! wire, both ends, the cut, strip, ferrule and label, and the duct fill after
//! it; the picture shows the wires run so far, the wire to run next in warm
//! white along its ducts with a marker at each end, and the ducts it lies in
//! outlined in their fill colour.
//!
//! Where the user finds it: 3D > Layers > Wire-it, View > 3D > Wire-it, or the
//! command palette (`wire-it`). Done marks are kept in the project, so a
//! half-wired cabinet opens at the next wire.
//!
//! The path is the route of the wire production job: the one the cut list
//! measured, with the same fill steering, so the picture, the cut length and
//! the duct fill of the strip never disagree. A connection with no route on
//! this cabinet's plate (to another cabinet, an unplaced device) has a step
//! and a sheet row but no picture; the strip says so.

use std::sync::Arc;

use eframe::egui;
use glam::Vec3;
use novo_core::{Project, StructureId};
use novo_reports::{WireItPlan, WireItStep, wire_it_done, wire_it_plan, wire_it_progress};
use novo_view3d::build::{WireInput, WireKind};
use novo_view3d::{Overlay, OverlayLines, Scene};

use super::cabinet_routes::{DuctFill, Tone, tone};

/// Margin from the view's edge, in points.
const MARGIN: f32 = 12.0;
/// Widest the strip gets before its text wraps, in points.
const TEXT_WIDTH: f32 = 640.0;
/// Scene wire ids of a step: far above any project wire id, so a step never
/// takes the id of a drawn wire.
const SCENE_ID_BASE: u64 = 1 << 48;
/// "Open at the next wire to run": the position a toggle asks for before the
/// plan is at hand.
pub(super) const RESUME: usize = usize::MAX;

/// Warm white, as the Run card's lit path.
const PATH: [u8; 4] = [255, 232, 170, 255];
/// End A (the `from` end): the copper accent.
const END_A: [u8; 4] = [232, 150, 80, 255];
/// End B (the `to` end).
const END_B: [u8; 4] = [90, 170, 235, 255];

fn id() -> egui::Id {
    egui::Id::new("view3d_wire_it")
}

/// A tick the person asked for, applied between frames (the view holds the
/// project while it draws).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Tick {
    pub step: WireItStep,
    pub on: bool,
}

/// What the strip asked for.
#[derive(Debug, Default)]
pub(super) struct Asked {
    pub ticks: Vec<Tick>,
    pub print: bool,
    pub leave: bool,
}

/// The plan, kept across the ticks of its own edits: a tick changes the
/// project revision but not the order, so it is not worked out again (that
/// routes the whole project).
#[derive(Debug, Default)]
pub(super) struct State {
    cached: Option<(u64, Arc<WireItPlan>)>,
    pub(super) pending: Vec<Tick>,
    /// The overlays drawn, and what they were built for (enclosure, project
    /// revision, pose revision, step).
    shown: Option<(StructureId, u64, u64, Option<usize>)>,
    pub(super) marks: Vec<Overlay>,
    /// The step the camera was last framed on.
    framed: Option<usize>,
}

impl State {
    /// The box to frame when the step changed, once per step; `None` when
    /// the camera is already there, Wire-it is off or the wire has no path
    /// in the scene.
    pub(super) fn frame(&mut self, at: Option<usize>, scene: &Scene) -> Option<novo_view3d::Aabb> {
        if self.framed == at {
            return None;
        }
        self.framed = at;
        let step = self.step(at?)?;
        let wire = scene
            .wires
            .iter()
            .find(|w| w.id.raw == scene_id(step.number))?;
        let mut b = novo_view3d::Aabb::EMPTY;
        for p in &wire.points {
            b.extend(*p);
        }
        // A wire along a duct is a thin box: give it some body.
        let pad = Vec3::new(60.0, 60.0, 60.0);
        Some(novo_view3d::Aabb::new(b.min - pad, b.max + pad))
    }

    /// How many wires the kept plan has.
    pub(super) fn len(&self) -> usize {
        self.cached.as_ref().map_or(0, |(_, p)| p.steps.len())
    }

    /// The step at `at` of the kept plan.
    pub(super) fn step(&self, at: usize) -> Option<WireItStep> {
        let (_, plan) = self.cached.as_ref()?;
        plan.steps
            .get(at.min(plan.steps.len().saturating_sub(1)))
            .cloned()
    }
}

impl State {
    /// The plan for `project`: the kept one while the revision is the one
    /// known to carry only ticks, else worked out afresh.
    pub(super) fn plan(&mut self, project: &Project) -> Arc<WireItPlan> {
        let revision = project.revision();
        if let Some((r, plan)) = &self.cached
            && *r == revision
        {
            return plan.clone();
        }
        let plan = wire_it_plan(project);
        self.cached = Some((revision, plan.clone()));
        plan
    }

    /// The project is at `revision` after the tick the person made: the plan
    /// kept for the revision before it is still the plan.
    pub(super) fn ticked(&mut self, before: u64, after: u64) {
        if let Some((r, _)) = &mut self.cached
            && *r == before
        {
            *r = after;
        }
    }

    pub(super) fn shown(&self) -> Option<&(StructureId, u64, u64, Option<usize>)> {
        self.shown.as_ref()
    }

    /// The project revision the kept plan stands for.
    #[cfg(test)]
    pub(super) fn cached_revision(&self) -> Option<u64> {
        self.cached.as_ref().map(|(r, _)| *r)
    }

    pub(super) fn set_shown(&mut self, key: (StructureId, u64, u64, Option<usize>)) {
        self.shown = Some(key);
    }
}

/// How much of the view's height the strip covers at the bottom, as a
/// fraction, for the fit to leave clear.
pub(super) fn inset(ctx: &egui::Context, view: egui::Rect, on: bool) -> f32 {
    if !on {
        return 0.0;
    }
    let covered = ctx
        .memory(|m| m.area_rect(id()))
        .map_or(150.0, |r| view.bottom() - r.top());
    (covered / view.height().max(1.0)).clamp(0.0, 0.4)
}

/// The position a toggle opens at: the first wire not run yet (the last one
/// when everything is), or the start of an empty plan.
pub(super) fn resume_at(project: &Project, plan: &WireItPlan) -> usize {
    wire_it_progress(project, plan)
        .next
        .unwrap_or_else(|| plan.steps.len().saturating_sub(1))
}

/// The next position for a command, held within the plan.
pub(super) fn step_by(at: usize, last: usize, forward: bool) -> usize {
    if forward {
        (at + 1).min(last)
    } else {
        at.saturating_sub(1)
    }
}

/// The scene wires of the picture: every wire run so far and the current one,
/// each as the production job routed it on this plate.
pub(super) fn inputs(
    project: &Project,
    plan: &WireItPlan,
    plate: StructureId,
    at: usize,
) -> Vec<WireInput> {
    plan.steps
        .iter()
        .enumerate()
        .filter(|(i, s)| *i == at || wire_it_done(project, s))
        .filter_map(|(_, s)| {
            let path = s.path.as_ref().filter(|p| p.plate == plate)?;
            Some(WireInput {
                id: SCENE_ID_BASE + s.number as u64,
                kind: if path.free_air {
                    WireKind::FreeAir
                } else {
                    WireKind::Ducted
                },
                points: path.points.clone(),
                ducts: s.ducts.iter().map(|d| d.duct).collect(),
                diameter_mm: novo_routing::model::wire_diameter_mm(s.cross_section_mm2),
                colour: first_colour(&s.colour),
            })
        })
        .collect()
}

fn first_colour(text: &str) -> Option<novo_view3d::build::WireColour> {
    text.split(',')
        .find_map(|t| novo_checks::standards::Colour::parse(t.trim()))
        .map(novo_view3d::build::model::insulation)
}

/// The scene id of the wire of step `number`.
pub(super) fn scene_id(number: usize) -> u64 {
    SCENE_ID_BASE + number as u64
}

fn tone_colour(t: Tone) -> [u8; 4] {
    let p = crate::theme::ui_palette();
    let c = match t {
        Tone::Idle | Tone::NoSize => p.text_secondary,
        Tone::Ok => p.success,
        Tone::Busy => p.warning,
        Tone::Over => p.error,
    };
    [c.r(), c.g(), c.b(), 255]
}

/// A diamond with a cross, flat on the plate side of the wire's end.
fn marker(lines: &mut OverlayLines, at: Vec3, colour: [u8; 4]) {
    let r = 11.0;
    let (dx, dy) = (Vec3::X * r, Vec3::Y * r);
    let mut poly = |pts: &[Vec3]| {
        lines.starts.push(lines.positions.len() as u32);
        for p in pts {
            lines.positions.push(*p);
            lines.colors.push(colour);
        }
    };
    poly(&[at + dx, at + dy, at - dx, at - dy, at + dx]);
    poly(&[at - dx, at + dx]);
    poly(&[at - dy, at + dy]);
}

/// How far the lit path floats above the highest thing it passes, mm: the
/// overlay is depth tested, so a path inside a duct or behind a slot would be
/// hidden. It is drawn in front instead, tied to each end by a post.
const LIFT_MM: f32 = 14.0;

/// The scene wire of a step and the ducts of it that are in the scene, with
/// their fill.
fn drawn<'a>(
    scene: &'a Scene,
    step: &'a WireItStep,
) -> Option<(
    &'a novo_view3d::scene::Wire,
    Vec<(&'a novo_view3d::Item, &'a novo_reports::DuctStep)>,
)> {
    let wire = scene
        .wires
        .iter()
        .find(|w| w.id.raw == scene_id(step.number))?;
    let ducts = step
        .ducts
        .iter()
        .filter_map(|d| {
            let item = scene
                .items
                .iter()
                .find(|i| i.id.kind == novo_view3d::ItemKind::Duct && i.id.raw == d.duct)?;
            Some((item, d))
        })
        .collect();
    Some((wire, ducts))
}

/// The path floating in front: up from the first end, along at the lift
/// height, down to the last end.
fn floating(
    scene: &Scene,
    wire: &novo_view3d::scene::Wire,
    ducts: &[(&novo_view3d::Item, &novo_reports::DuctStep)],
) -> (Vec<Vec3>, f32) {
    let mut top = wire.points.iter().map(|p| p.z).fold(0.0_f32, f32::max);
    for (item, _) in ducts {
        top = top.max(scene.world_bounds(item).max.z);
    }
    let lift = top + LIFT_MM;
    let mut line = Vec::new();
    if let Some(first) = wire.points.first() {
        line.push(*first);
    }
    line.extend(wire.points.iter().map(|p| Vec3::new(p.x, p.y, lift)));
    if let Some(last) = wire.points.last() {
        line.push(*last);
    }
    (line, lift)
}

/// The overlays of the current step: its path lit (floating just in front of
/// the ducts it lies in, with a post down to each end), a marker at each end,
/// and the ducts it lies in outlined by the fill the wire leaves them at.
pub(super) fn overlays(scene: &Scene, step: &WireItStep) -> Vec<Overlay> {
    let Some((wire, ducts)) = drawn(scene, step) else {
        return Vec::new();
    };
    let (line, lift) = floating(scene, wire, &ducts);
    let up = |p: &Vec3| Vec3::new(p.x, p.y, lift);
    let mut out = Vec::new();
    let mut path = OverlayLines {
        width_px: 4.0,
        order: 21,
        ..OverlayLines::default()
    };
    path.starts.push(0);
    for p in line {
        path.positions.push(p);
        path.colors.push(PATH);
    }
    out.push(Overlay::Lines(path));
    let mut ends = OverlayLines {
        width_px: 3.0,
        order: 22,
        ..OverlayLines::default()
    };
    if let (Some(a), Some(b)) = (wire.points.first(), wire.points.last()) {
        marker(&mut ends, up(a), END_A);
        marker(&mut ends, up(b), END_B);
    }
    out.push(Overlay::Lines(ends));
    let mut outline = OverlayLines {
        width_px: 2.0,
        order: 20,
        ..OverlayLines::default()
    };
    for (item, d) in &ducts {
        let fill = DuctFill {
            rail: d.duct,
            rect: (0.0, 0.0, 0.0, 0.0),
            ratio: d.fill_after,
            wires: d.wires_after,
        };
        super::view3d_runs::box_edges(&mut outline, scene, item, tone_colour(tone(&fill)));
    }
    if !outline.positions.is_empty() {
        out.push(Overlay::Lines(outline));
    }
    out
}

/// The wires run so far, floating in front in the success colour, so the
/// progress shows even where the ducts hide the wires themselves.
pub(super) fn run_overlay(scene: &Scene, run: &[&WireItStep]) -> Option<Overlay> {
    let p = crate::theme::ui_palette().success;
    let colour = [p.r(), p.g(), p.b(), 235];
    let mut lines = OverlayLines {
        width_px: 2.5,
        order: 19,
        ..OverlayLines::default()
    };
    for step in run {
        let Some((wire, ducts)) = drawn(scene, step) else {
            continue;
        };
        let (line, _) = floating(scene, wire, &ducts);
        lines.starts.push(lines.positions.len() as u32);
        for point in line {
            lines.positions.push(point);
            lines.colors.push(colour);
        }
    }
    (!lines.positions.is_empty()).then_some(Overlay::Lines(lines))
}

/// Words for what the picture can and cannot say about a step.
pub(super) fn picture_note(step: &WireItStep, drawn_here: bool) -> Option<&'static str> {
    if drawn_here {
        None
    } else if step.cable_core && step.path.is_none() {
        Some("A core of a cable: the cable is pulled once, so no path is drawn for the core.")
    } else if step.path.is_none() {
        Some(
            "No path on this plate: the wire runs to another cabinet, outside, or a device that is \
             not placed. It is on the sheet; wire it from the list.",
        )
    } else {
        Some("This wire runs on another cabinet's plate: open that cabinet to see it.")
    }
}

/// The strip. `at` is the current step; the plan and project say what is run.
pub(super) fn strip(
    ctx: &egui::Context,
    view: egui::Rect,
    at: &mut usize,
    project: &Project,
    plan: &WireItPlan,
    drawn_here: bool,
) -> Asked {
    let mut asked = Asked::default();
    let p = crate::theme::ui_palette();
    let progress = wire_it_progress(project, plan);
    let orphans = novo_reports::wire_it_orphaned(project, plan);
    egui::Area::new(id())
        .order(egui::Order::Foreground)
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(egui::pos2(view.center().x, view.bottom() - MARGIN))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_max_width(TEXT_WIDTH.min(view.width() - 2.0 * MARGIN));
                if plan.steps.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "Wire-it: no wires to run yet. Draw wires between device pins in \
                                 the schematic.",
                            )
                            .color(p.text_secondary),
                        );
                        if ui.button("Show all").clicked() {
                            asked.leave = true;
                        }
                    });
                    return;
                }
                let last = plan.steps.len() - 1;
                *at = (*at).min(last);
                let step = &plan.steps[*at];
                let done = wire_it_done(project, step);
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(*at > 0, |ui| {
                        let back = ui.button("Previous");
                        super::cabinet_bar::note_ctx(ctx, "wire_it.previous", "Previous", &back);
                        if back.clicked() {
                            *at -= 1;
                        }
                    });
                    ui.label(
                        egui::RichText::new(format!(
                            "Wire {} of {}: {}",
                            *at + 1,
                            plan.steps.len(),
                            step.class.name()
                        ))
                        .strong()
                        .color(p.text_strong),
                    );
                    ui.add_enabled_ui(*at < last, |ui| {
                        let next = ui.button("Next");
                        super::cabinet_bar::note_ctx(ctx, "wire_it.next", "Next", &next);
                        if next.clicked() {
                            *at += 1;
                        }
                    });
                    let all = ui
                        .button("Show all")
                        .on_hover_text("Leave Wire-it and show the whole cabinet");
                    super::cabinet_bar::note_ctx(ctx, "wire_it.show_all", "Show all", &all);
                    if all.clicked() {
                        asked.leave = true;
                    }
                    let print = ui.button("Print list (PDF)").on_hover_text(
                        "Every wire in this order on landscape A4, with a box to tick, the \
                             cut, strip, ferrule, labels and the duct fill after each wire",
                    );
                    super::cabinet_bar::note_ctx(ctx, "wire_it.print", "Print list (PDF)", &print);
                    if print.clicked() {
                        asked.print = true;
                    }
                });
                let name = if step.wire_number.is_empty() {
                    "(no wire number)".to_owned()
                } else {
                    step.wire_number.clone()
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(
                        egui::RichText::new(name)
                            .heading()
                            .strong()
                            .color(p.text_strong),
                    );
                    ui.label(egui::RichText::new(step.kind_text()).color(p.text_secondary));
                    if done {
                        ui.label(egui::RichText::new("run").strong().color(p.success));
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label(egui::RichText::new("from").small().color(rgb(END_A)));
                    ui.label(egui::RichText::new(&step.from).color(p.text_strong));
                    ui.label(egui::RichText::new("to").small().color(rgb(END_B)));
                    ui.label(egui::RichText::new(&step.to).color(p.text_strong));
                });
                let tilde = if step.estimated() { "~" } else { "" };
                let strip_text = step.strip_text();
                let cut_line = if step.cable_core {
                    format!(
                        "{} (a core: run with its cable, not cut)   {}   {}",
                        sentence(step.run_or_cut_text()),
                        if strip_text.is_empty() {
                            "strip: size not set".to_owned()
                        } else {
                            strip_text
                        },
                        step.fitting_text(),
                    )
                } else {
                    format!(
                        "Cut {tilde}{} mm (routed {} + {} each end)   {}   {}",
                        novo_layout::drill::mm(step.cut_mm),
                        novo_layout::drill::mm(step.length_mm),
                        novo_layout::drill::mm(step.allowance_mm),
                        if strip_text.is_empty() {
                            "strip: size not set".to_owned()
                        } else {
                            strip_text
                        },
                        step.fitting_text(),
                    )
                };
                ui.label(egui::RichText::new(cut_line).color(p.text_strong));
                let labels = if step.label_from == step.label_to {
                    format!("Label both ends: {}", step.label_from)
                } else {
                    format!(
                        "Label: {}  |  other end: {}",
                        step.label_from, step.label_to
                    )
                };
                ui.label(egui::RichText::new(labels).color(p.text_secondary));
                let route = format!("Route {}.  {}", step.track, step.fill_text());
                ui.label(egui::RichText::new(route).color(if step.over_filled() {
                    p.error
                } else {
                    p.text_secondary
                }))
                .on_hover_text(
                    "Duct fill after this wire, in the planned bench order. It does not track \
                     what is physically in the duct if wires are run in another order.",
                );
                if step.estimated() {
                    ui.label(
                        egui::RichText::new(format!(
                            "The length is a planning figure ({}), not a measurement of the \
                             finished wire.",
                            step.basis
                        ))
                        .small()
                        .color(p.warning),
                    );
                }
                if let Some(note) = picture_note(step, drawn_here) {
                    ui.label(egui::RichText::new(note).small().color(p.warning));
                }
                if orphans > 0 {
                    ui.label(
                        egui::RichText::new(format!(
                            "{orphans} run mark{} kept on wires that are no connection any \
                             more (deleted or rewired after they were run).",
                            if orphans == 1 { " is" } else { "s are" }
                        ))
                        .small()
                        .color(p.text_secondary),
                    );
                }
                ui.horizontal(|ui| {
                    if done {
                        let undo = ui.button("Not run yet");
                        if undo.clicked() {
                            asked.ticks.push(Tick {
                                step: step.clone(),
                                on: false,
                            });
                        }
                    } else {
                        let go = ui
                            .button(egui::RichText::new("Run, next wire").strong())
                            .on_hover_text("Mark this wire run and go to the next one");
                        super::cabinet_bar::note_ctx(ctx, "wire_it.run", "Run, next wire", &go);
                        if go.clicked() {
                            asked.ticks.push(Tick {
                                step: step.clone(),
                                on: true,
                            });
                            if *at < last {
                                *at += 1;
                            }
                        }
                        let only = ui.button("Mark run");
                        if only.clicked() {
                            asked.ticks.push(Tick {
                                step: step.clone(),
                                on: true,
                            });
                        }
                    }
                    ui.label(
                        egui::RichText::new(format!(
                            "{} of {} run, {:.1} m of wire still to cut",
                            progress.done,
                            progress.total,
                            plan.cut_m_left(project)
                        ))
                        .color(p.text_secondary),
                    );
                });
                ui.horizontal(|ui| {
                    let mut pos = *at;
                    ui.spacing_mut().slider_width = 240.0;
                    if ui
                        .add(egui::Slider::new(&mut pos, 0..=last).show_value(false))
                        .on_hover_text("Drag to any wire")
                        .changed()
                    {
                        *at = pos;
                    }
                    if let Some(next) = progress.next
                        && next != *at
                    {
                        let jump = ui
                            .button("Next not run")
                            .on_hover_text("Jump to the first wire not run yet");
                        super::cabinet_bar::note_ctx(
                            ctx,
                            "wire_it.next_open",
                            "Next not run",
                            &jump,
                        );
                        if jump.clicked() {
                            *at = next;
                        }
                    }
                    ui.label(
                        egui::RichText::new("Orange: from end. Blue: to end. White: the path.")
                            .small()
                            .color(p.text_secondary),
                    );
                });
            });
        });
    asked
}

fn rgb(c: [u8; 4]) -> egui::Color32 {
    egui::Color32::from_rgb(c[0], c[1], c[2])
}

/// Capitalises the first letter of a bench instruction.
fn sentence(mut text: String) -> String {
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text
}
