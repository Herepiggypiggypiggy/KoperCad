//! Reading where the heat goes, with the same calm as the airflow control:
//! one Heat flow mode (Off, Air, Solids, Air and solids, Flux map), a per
//! device split of each device's heat into air, plate and radiation, and a
//! cloud that moves with the air.
//!
//! Where the user finds it: 3D > Heat strip. "Heat flow" is the mode menu,
//! "Paths" pins the split on the devices (the list is in the Heat panel,
//! "Where each device's heat goes"), and "Moves" under the Cloud plays the
//! cloud with the solved air flow.
//!
//! What is exact and what is a model. The modes draw quantities derived from
//! the solved temperature and air speed (`novo_heatview::flux`): carried heat
//! `rho cp (T - ambient) u` in the air and conducted heat `-k grad T` in
//! solids, without surface radiation. The split per device is the solver's own
//! (`CfdSnapshot::heat_paths`); a result that does not split it falls back to
//! the plate share of the device and says radiation is not split. The moving
//! cloud is a frozen-flow transport model (`novo_heatview::advect`): the solved
//! flow carries the heat from a cold start; it shows how and how fast the heat
//! moves, it is not a second solve and not a measurement.

use std::time::{Duration, Instant};

use eframe::egui;
use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::advect::Advector;
use novo_heatview::cfd::{self, Axis};
use novo_heatview::flux;
use novo_view3d::scene::ItemKind;
use novo_view3d::widget::View3d;
use novo_view3d::{OverlayMesh, Scene};

use super::heat_3d::Place;

/// What the Heat flow control draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HeatFlowMode {
    #[default]
    Off,
    /// Lines and moving dashes along the heat the air carries away.
    Air,
    /// Arrows inside plate, rails and devices: heat conducted through them.
    Solids,
    /// Both of the above.
    Both,
    /// The slice plane coloured by the heat the air carries, in W/m².
    Map,
}

impl HeatFlowMode {
    pub(crate) const ALL: [HeatFlowMode; 5] = [
        HeatFlowMode::Off,
        HeatFlowMode::Air,
        HeatFlowMode::Solids,
        HeatFlowMode::Both,
        HeatFlowMode::Map,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            HeatFlowMode::Off => "Off",
            HeatFlowMode::Air => "Carried by air",
            HeatFlowMode::Solids => "Through solids",
            HeatFlowMode::Both => "Air and solids",
            HeatFlowMode::Map => "Flux map",
        }
    }

    pub(crate) fn hint(self) -> &'static str {
        match self {
            HeatFlowMode::Off => "No heat flow drawn",
            HeatFlowMode::Air => {
                "Orange lines and moving dashes follow the heat the air carries away from hot parts; brighter is more heat per area"
            }
            HeatFlowMode::Solids => {
                "Arrows inside the plate, rails and devices: heat conducted through them (-k grad T)"
            }
            HeatFlowMode::Both => "Carried by the air and conducted through solids together",
            HeatFlowMode::Map => {
                "The slice plane coloured by the heat the air carries, in W/m², instead of temperature"
            }
        }
    }

    /// The lines and dashes of the air are drawn.
    pub(crate) fn draws_air(self) -> bool {
        matches!(self, HeatFlowMode::Air | HeatFlowMode::Both)
    }

    /// The conduction arrows of the solids are drawn.
    pub(crate) fn draws_solids(self) -> bool {
        matches!(
            self,
            HeatFlowMode::Solids | HeatFlowMode::Both | HeatFlowMode::Map
        )
    }

    /// The slice is a flux map.
    pub(crate) fn draws_map(self) -> bool {
        self == HeatFlowMode::Map
    }

    /// The flux field has to be derived.
    pub(crate) fn needs_flux(self) -> bool {
        self != HeatFlowMode::Off
    }
}

// ---------------------------------------------------------------------------
// The flux map.

/// Cells across the longer side of the map.
const MAP_CELLS: f64 = 100.0;
/// Colour of the map: dark orange to yellow-white (data, not theme).
fn map_rgb(share: f32) -> [u8; 3] {
    let s = share.clamp(0.0, 1.0);
    [
        (120.0 + 135.0 * s) as u8,
        (40.0 + 190.0 * s * s) as u8,
        (10.0 + 120.0 * s * s * s) as u8,
    ]
}

/// The slice plane at `at` (0 to 1 along `axis`) coloured by the length of the
/// heat the air carries, on a scale from 0 to `top` W/m². Cells inside solids
/// are left out so the devices stay visible. `field` is
/// [`flux::flux_field`].
pub(crate) fn flux_map(
    snap: &CfdSnapshot,
    field: &[[f32; 3]],
    place: Place,
    axis: Axis,
    at: f32,
    top: f32,
    alpha: u8,
) -> Option<OverlayMesh> {
    if !snap.is_consistent() || field.len() != snap.grid.cells() || top <= 0.0 {
        return None;
    }
    let offset = super::heat_3d::slice_offset(snap, axis, at);
    let window = cfd::whole_window(snap, axis);
    let cell = (window.w.max(window.h) / MAP_CELLS).max(4.0);
    let cols = (window.w / cell).ceil().max(1.0) as usize;
    let rows = (window.h / cell).ceil().max(1.0) as usize;
    let (cw, ch) = (window.w / cols as f64, window.h / rows as f64);
    let point = |u: f64, v: f64| match axis {
        Axis::Z => [u, v, offset],
        Axis::X => [offset, v, u],
        Axis::Y => [u, offset, v],
    };
    let mut m = OverlayMesh {
        translucent: true,
        order: 0,
        ..Default::default()
    };
    for r in 0..rows {
        for c in 0..cols {
            let (u0, v0) = (window.u0 + c as f64 * cw, window.v0 + r as f64 * ch);
            let centre = point(u0 + 0.5 * cw, v0 + 0.5 * ch);
            if !cfd::inside(snap, centre) || cfd::solid_at(snap, centre) {
                continue;
            }
            let q = flux::magnitude(flux::flux_at(snap, field, centre));
            if q <= 0.0 {
                continue;
            }
            let share = (q / top).clamp(0.0, 1.0);
            let rgb = map_rgb(share);
            let colour = [
                rgb[0],
                rgb[1],
                rgb[2],
                (f32::from(alpha) * share.sqrt()) as u8,
            ];
            let (u1, v1) = (u0 + cw, v0 + ch);
            let base = m.positions.len() as u32;
            for (u, v) in [(u0, v0), (u1, v0), (u1, v1), (u0, v1)] {
                m.positions.push(place.world(point(u, v)));
                m.colors.push(colour);
            }
            m.indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (!m.indices.is_empty()).then_some(m)
}

// ---------------------------------------------------------------------------
// Where each device's heat goes.

/// One device's heat by path, W.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PathRow {
    pub id: u64,
    pub total_w: f64,
    /// Into the air by convection.
    pub air_w: f64,
    /// Through the mounting plate and the solids it touches.
    pub plate_w: f64,
    pub radiation_w: f64,
    /// The solver split radiation out; false when it did not and the
    /// radiation share is part of the air's.
    pub radiation_split: bool,
}

impl PathRow {
    /// Shares of the total: air, plate, radiation.
    pub(crate) fn shares(&self) -> [f64; 3] {
        if self.total_w <= 0.0 {
            return [0.0; 3];
        }
        [
            self.air_w / self.total_w,
            self.plate_w / self.total_w,
            self.radiation_w / self.total_w,
        ]
    }
}

/// Devices below this loss have no row, W.
const MIN_LOSS_W: f64 = 0.05;

/// The rows of a result, most heat first. From the solver's split when it
/// has one; otherwise from the plate share of each device, with radiation
/// counted in the air's share.
pub(crate) fn path_rows(snap: &CfdSnapshot) -> Vec<PathRow> {
    let mut rows: Vec<PathRow> = if snap.heat_paths.is_empty() {
        snap.devices
            .iter()
            .filter(|d| d.loss_w >= MIN_LOSS_W)
            .map(|d| {
                let plate = (d.loss_w * d.plate_share.clamp(0.0, 1.0)).max(0.0);
                PathRow {
                    id: d.id,
                    total_w: d.loss_w,
                    air_w: d.loss_w - plate,
                    plate_w: plate,
                    radiation_w: 0.0,
                    radiation_split: false,
                }
            })
            .collect()
    } else {
        snap.heat_paths
            .iter()
            .filter(|p| p.total_w >= MIN_LOSS_W)
            .map(|p| {
                // A device that takes heat from the plate (a terminal on a hot
                // rail) can show more than its own loss through it: keep the
                // shares inside 0 to 100 %.
                let plate = p.conduction_w.clamp(0.0, p.total_w);
                let rad = p.radiation_w.clamp(0.0, p.total_w - plate);
                PathRow {
                    id: p.id,
                    total_w: p.total_w,
                    air_w: p.total_w - plate - rad,
                    plate_w: plate,
                    radiation_w: rad,
                    radiation_split: snap.radiation_on,
                }
            })
            .collect()
    };
    rows.sort_by(|a, b| b.total_w.total_cmp(&a.total_w));
    rows
}

/// The one line of a row.
pub(crate) fn path_text(r: &PathRow, tag: &str) -> String {
    let [a, p, rad] = r.shares();
    if r.radiation_split {
        format!(
            "{tag}  {:.1} W: air {:.0} % · plate {:.0} % · radiation {:.0} %",
            r.total_w,
            a * 100.0,
            p * 100.0,
            rad * 100.0
        )
    } else {
        format!(
            "{tag}  {:.1} W: air {:.0} % · plate {:.0} %",
            r.total_w,
            (a + rad) * 100.0,
            p * 100.0
        )
    }
}

/// Colours of the three paths (data, not theme): air, plate, radiation.
pub(crate) const PATH_RGB: [[u8; 3]; 3] = [[255, 150, 50], [225, 80, 80], [165, 110, 230]];

/// Most devices pinned at once.
pub(crate) const PINNED: usize = 6;

/// A stacked bar of the three shares in `rect`.
fn share_bar(painter: &egui::Painter, rect: egui::Rect, r: &PathRow) {
    let mut x = rect.min.x;
    let [a, p, rad] = r.shares();
    for (share, rgb) in [(a, PATH_RGB[0]), (p, PATH_RGB[1]), (rad, PATH_RGB[2])] {
        let w = rect.width() * share as f32;
        if w >= 0.5 {
            painter.rect_filled(
                egui::Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(w, rect.height())),
                0.0,
                egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
            );
            x += w;
        }
    }
}

/// Pins the split on the devices that give off the most heat: tag, watts and
/// a bar of air, plate and radiation. `dim` greys them (out of date result).
pub(crate) fn paint_paths(
    ui: &egui::Ui,
    view: &View3d,
    rect: egui::Rect,
    scene: &Scene,
    rows: &[PathRow],
    tag_of: &dyn Fn(u64) -> String,
    dim: bool,
) {
    let pal = crate::theme::ui_palette();
    let font = egui::FontId::proportional(11.5);
    let painter = ui.painter().with_clip_rect(rect);
    let mut placed: Vec<egui::Rect> = Vec::new();
    for r in rows.iter().take(PINNED) {
        let Some(item) = scene
            .items
            .iter()
            .find(|i| i.id.kind == ItemKind::Device && i.id.raw == r.id)
        else {
            continue;
        };
        // The top of the device's front face.
        let top = scene.world_matrix(item).transform_point3(Vec3::new(
            0.0,
            item.size.y * 0.5,
            item.size.z,
        ));
        let Some(at) = view.project(rect, scene, top) else {
            continue;
        };
        let text = path_text(r, &tag_of(r.id));
        let galley = painter.layout_no_wrap(text, font.clone(), pal.text_primary);
        let size = egui::vec2(galley.size().x + 12.0, galley.size().y + 12.0);
        let mut chip = egui::Rect::from_center_size(at - egui::vec2(0.0, size.y * 0.6), size);
        while placed.iter().any(|p| p.intersects(chip)) && chip.max.y < rect.max.y {
            chip = chip.translate(egui::vec2(0.0, size.y + 2.0));
        }
        placed.push(chip);
        painter.rect(
            chip,
            4.0,
            pal.raised.gamma_multiply(0.92),
            egui::Stroke::new(
                1.0,
                if dim {
                    pal.text_secondary
                } else {
                    pal.border_control
                },
            ),
            egui::StrokeKind::Inside,
        );
        painter.galley(chip.min + egui::vec2(6.0, 3.0), galley, pal.text_primary);
        let bar = egui::Rect::from_min_max(
            egui::pos2(chip.min.x + 6.0, chip.max.y - 7.0),
            egui::pos2(chip.max.x - 6.0, chip.max.y - 3.0),
        );
        if dim {
            painter.rect_filled(bar, 0.0, pal.text_secondary.gamma_multiply(0.4));
        } else {
            share_bar(&painter, bar, r);
        }
    }
}

/// The list in the Heat panel: every device with its split, and the key.
pub(crate) fn paths_table(ui: &mut egui::Ui, rows: &[PathRow], tag_of: &dyn Fn(u64) -> String) {
    let pal = crate::theme::ui_palette();
    if rows.is_empty() {
        ui.label(
            egui::RichText::new("No device gives off heat in this result.")
                .small()
                .color(pal.text_secondary),
        );
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (name, rgb) in ["air", "plate", "radiation"].iter().zip(PATH_RGB) {
            let (dot, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(dot, 2.0, egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
            ui.label(egui::RichText::new(*name).small());
        }
    });
    for r in rows {
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width().max(120.0), 28.0),
            egui::Sense::hover(),
        );
        response.on_hover_text(path_hover(r));
        let text = path_text(r, &tag_of(r.id));
        ui.painter().text(
            rect.min,
            egui::Align2::LEFT_TOP,
            text,
            egui::FontId::proportional(11.5),
            pal.text_primary,
        );
        let bar = egui::Rect::from_min_max(
            egui::pos2(rect.min.x, rect.max.y - 9.0),
            egui::pos2(rect.max.x - 4.0, rect.max.y - 4.0),
        );
        share_bar(ui.painter(), bar, r);
    }
    let split = rows.iter().any(|r| r.radiation_split);
    ui.label(
        egui::RichText::new(if split {
            "The solver's own split of each device's loss (a model, not a measurement)."
        } else {
            "From the plate share of each device; radiation is not split in this result and counts as air. A model, not a measurement."
        })
        .small()
        .color(pal.text_secondary),
    );
}

fn path_hover(r: &PathRow) -> String {
    format!(
        "{:.2} W in total: {:.2} W to the air by convection, {:.2} W through the mounting plate{}",
        r.total_w,
        r.air_w,
        r.plate_w,
        if r.radiation_split {
            format!(", {:.2} W by radiation", r.radiation_w)
        } else {
            String::from("; radiation not split in this result")
        }
    )
}

// ---------------------------------------------------------------------------
// The cloud that moves.

/// What the person sets for the moving cloud.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MoveControls {
    pub playing: bool,
    /// Model seconds per real second.
    pub speed: f32,
    /// Start again from cold air; cleared when done.
    pub restart: bool,
    /// The line under the controls, set by the running model.
    pub status: String,
}

impl Default for MoveControls {
    fn default() -> Self {
        MoveControls {
            playing: true,
            speed: 4.0,
            restart: false,
            status: String::new(),
        }
    }
}

/// Fastest and slowest play speed, model seconds per real second.
pub(crate) const SPEED_RANGE: std::ops::RangeInclusive<f32> = 1.0..=40.0;
/// Work per frame, so the view stays responsive.
const FRAME_BUDGET: Duration = Duration::from_millis(9);
/// The longest real gap counted in one frame, s: a stalled window does not
/// make the cloud jump.
const MAX_GAP_S: f64 = 0.1;
/// The picture is rebuilt at most this often, s.
const REBUILD_EVERY_S: f64 = 0.08;

/// The cloud on the move for one result.
pub(crate) struct MovingCloud {
    /// The result it belongs to.
    pub result: u64,
    adv: Advector,
    /// The solved result with the model's air temperature in it, what the
    /// cloud is drawn from.
    work: CfdSnapshot,
    last: Option<f64>,
    changed_at: f64,
    /// The model could not keep up with the clock last frame.
    behind: bool,
    /// Real time one frame may spend stepping the model.
    budget: Duration,
}

impl MovingCloud {
    pub(crate) fn new(result: u64, snap: &CfdSnapshot, ambient: f64) -> Option<MovingCloud> {
        let adv = Advector::new(snap, ambient)?;
        let mut work = snap.clone();
        work.temperature_c.copy_from_slice(adv.temperature_c());
        Some(MovingCloud {
            result,
            adv,
            work,
            last: None,
            changed_at: f64::NEG_INFINITY,
            behind: false,
            budget: FRAME_BUDGET,
        })
    }

    /// A different per frame budget (a debug build in a test is slow).
    #[cfg(test)]
    pub(crate) fn with_budget(mut self, budget: Duration) -> MovingCloud {
        self.budget = budget;
        self
    }

    /// Model time since the cold start, s.
    #[cfg(test)]
    pub(crate) fn time_s(&self) -> f64 {
        self.adv.time_s()
    }

    /// Advances to the real time `now` (s). True when the picture changed
    /// enough to rebuild.
    pub(crate) fn tick(&mut self, now: f64, controls: &mut MoveControls) -> bool {
        let mut changed = false;
        if controls.restart {
            controls.restart = false;
            self.adv.restart();
            self.work
                .temperature_c
                .copy_from_slice(self.adv.temperature_c());
            changed = true;
        }
        let gap = self.last.map_or(0.0, |l| (now - l).clamp(0.0, MAX_GAP_S));
        self.last = Some(now);
        if controls.playing && gap > 0.0 {
            let wanted = gap * f64::from(controls.speed);
            let start = Instant::now();
            let mut done = 0.0;
            let step = self.adv.step_limit_s();
            while done < wanted && start.elapsed() < self.budget {
                done += self.adv.advance((wanted - done).min(step), 1);
            }
            self.behind = done + 1e-9 < wanted;
            if done > 0.0 {
                changed = true;
            }
        }
        if changed && now - self.changed_at >= REBUILD_EVERY_S {
            self.work
                .temperature_c
                .copy_from_slice(self.adv.temperature_c());
            self.changed_at = now;
            controls.status = self.status(controls);
            return true;
        }
        false
    }

    /// The solved result carrying the model's air temperature.
    pub(crate) fn snapshot(&self) -> &CfdSnapshot {
        &self.work
    }

    fn status(&self, controls: &MoveControls) -> String {
        let mut s = format!(
            "Moving cloud: {:.0} s after a cold start at ×{:.0}; the model is {:.1} K from the solved air",
            self.adv.time_s(),
            controls.speed,
            self.adv.match_rmse_k()
        );
        if self.behind {
            s.push_str(" (this computer cannot keep up: slower than asked)");
        }
        s
    }
}

/// The note under the strip for the moving cloud.
pub(crate) const MOVING_NOTE: &str = "Moving cloud: the solved air flow carries the heat from a cold start. A transport model on the solved flow, not a second solve; devices are held at their solved temperature and the flow does not react to the warming. It shows how and how fast the heat moves.";
