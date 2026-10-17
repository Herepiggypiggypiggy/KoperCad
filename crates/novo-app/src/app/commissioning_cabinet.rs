//! Open as-built redlines on the Cabinet plate: a device (or the strip a
//! terminal sits in) with something waiting to be applied is outlined, and
//! hovering the outline says what.

use eframe::egui;
use novo_canvas::Viewport;
use novo_commissioning::{CommissioningStore, RedlineState, Target};
use novo_core::Project;
use novo_layout::state::CabinetLayout;

/// One outlined device: its footprint in mm and what the redlines on it say.
#[derive(Debug, PartialEq)]
pub(super) struct CabinetMark {
    /// `(x, y, width, height)`.
    pub(super) footprint: (f64, f64, f64, f64),
    pub(super) text: String,
}

/// The marks for the open redlines on the placed devices. Wires are not
/// drawn on the plate, so their redlines have no mark here.
pub(super) fn marks(
    store: &CommissioningStore,
    project: &Project,
    layout: &CabinetLayout,
) -> Vec<CabinetMark> {
    let mut out: Vec<CabinetMark> = Vec::new();
    for redline in store.redlines().filter(|r| r.state == RedlineState::Open) {
        let device = match redline.target {
            Target::Device(d) => d.raw(),
            Target::Terminal(t) => match project.terminal(t) {
                Some(terminal) => terminal.strip.raw(),
                None => continue,
            },
            Target::Wire(_) => continue,
        };
        let Some(placement) = layout.placements.iter().find(|p| p.id == device) else {
            continue;
        };
        let f = placement.footprint();
        let line = format!("Redline: {}", redline.summary());
        match out.iter_mut().find(|m| m.footprint == (f.x, f.y, f.w, f.h)) {
            Some(mark) => {
                mark.text.push('\n');
                mark.text.push_str(&line);
            }
            None => out.push(CabinetMark {
                footprint: (f.x, f.y, f.w, f.h),
                text: line,
            }),
        }
    }
    out
}

/// Paints the outlines over the plate (`rect` is the canvas widget's screen
/// rectangle) and returns the text of the one under `pointer`, if any.
pub(super) fn draw(
    painter: &egui::Painter,
    rect: egui::Rect,
    view: &Viewport,
    marks: &[CabinetMark],
    color: egui::Color32,
    pointer: Option<egui::Pos2>,
) -> Option<String> {
    let to_screen = |x: f64, y: f64| {
        let s = view.world_to_screen(novo_canvas::vec2(x as f32, y as f32));
        egui::pos2(rect.min.x + s.x, rect.min.y + s.y)
    };
    let mut hovered = None;
    for mark in marks {
        let (x, y, w, h) = mark.footprint;
        let area = egui::Rect::from_two_pos(to_screen(x, y), to_screen(x + w, y + h)).expand(3.0);
        painter.rect_filled(area, 2.0, color.gamma_multiply(0.15));
        painter.rect_stroke(
            area,
            2.0,
            egui::Stroke::new(2.0, color),
            egui::StrokeKind::Outside,
        );
        if pointer.is_some_and(|p| area.contains(p)) {
            hovered = Some(mark.text.clone());
        }
    }
    hovered
}
