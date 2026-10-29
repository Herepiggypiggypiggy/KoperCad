//! One calm way to read the airflow of a heat result: a single Flow control
//! (Off, Arrows, Streamlines, Speed map) with a strength slider, one colour
//! scale in m/s that every mode shares, plain words for how fast the air is,
//! and the through-flow of the openings with a verdict. Pure data and
//! geometry; `view3d.rs` and `heat_legend.rs` call it.
//!
//! What is exact and what is a model: the speeds are the solver's (a coarse
//! planning-level turbulence model, not measured); the m³/h of an opening is
//! read from the speed one cell inside its face; the air warming is an energy
//! balance (`loss / (rho cp flow)`) that ignores the heat the walls and the
//! plate give off, so it is on the high side. The factor is `FAN_FACTOR` (3.1
//! m³·K per W·h: air at sea level, density 1.2 kg/m³, with about 4 % margin).
//! The words and the verdict
//! thresholds are planning values, not a standard.

use eframe::egui;
use glam::Vec3;
use novo_cfd::CfdSnapshot;
use novo_heatview::cfd::{self, Axis, Plane};
use novo_view3d::{Overlay, OverlayLines, OverlayMesh};

use super::heat_3d::{Flow, FlowLine, Place, speed_rgb};
use super::heat_openings::Role;
use super::heat_settings::VentSpec;

/// What the Flow control draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum FlowMode {
    #[default]
    Off,
    /// A coarse grid of arrows on the slice plane.
    Arrows,
    /// Few thick lines that follow the air through the cabinet.
    Streamlines,
    /// The slice plane coloured by air speed instead of temperature.
    SpeedMap,
}

impl FlowMode {
    pub(crate) const ALL: [FlowMode; 4] = [
        FlowMode::Off,
        FlowMode::Arrows,
        FlowMode::Streamlines,
        FlowMode::SpeedMap,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            FlowMode::Off => "Off",
            FlowMode::Arrows => "Arrows",
            FlowMode::Streamlines => "Streamlines",
            FlowMode::SpeedMap => "Speed map",
        }
    }

    pub(crate) fn hint(self) -> &'static str {
        match self {
            FlowMode::Off => "No airflow drawn",
            FlowMode::Arrows => {
                "Arrows on the slice plane: longer, thicker and greener is faster air"
            }
            FlowMode::Streamlines => {
                "A few lines that follow the air through the cabinet: thicker and greener is faster air"
            }
            FlowMode::SpeedMap => {
                "The slice plane coloured by air speed in m/s instead of temperature"
            }
        }
    }
}

/// The Flow control: a mode and how much of it to show.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FlowView {
    pub mode: FlowMode,
    /// 0 (only the main streams) to 1 (every line and arrow found).
    pub strength: f32,
}

impl Default for FlowView {
    fn default() -> Self {
        FlowView {
            mode: FlowMode::Off,
            strength: DEFAULT_STRENGTH,
        }
    }
}

/// A calm first look: about two in five of the lines.
pub(crate) const DEFAULT_STRENGTH: f32 = 0.25;

/// Round scale tops, m/s: the colour scale ends on one of these, so the
/// numbers on the bar are round and the same colour means the same speed
/// across Arrows, Streamlines and the Speed map of one result.
const NICE_TOPS: [f32; 8] = [0.1, 0.2, 0.5, 1.0, 2.0, 3.0, 5.0, 10.0];

/// Where the words change, m/s: below the first the air is still, above the
/// last it is fast. 0.1 m/s is the draught you just notice on skin; 1 m/s is a
/// clear breeze; a cabinet fan's outlet runs at several m/s.
const BANDS: [(f32, &str); 4] = [
    (0.05, "still"),
    (0.2, "gentle"),
    (0.5, "moderate"),
    (1.5, "brisk"),
];

/// The word for an air speed, m/s.
pub(crate) fn speed_word(v: f32) -> &'static str {
    BANDS
        .iter()
        .find(|(top, _)| v < *top)
        .map_or("fast", |(_, word)| word)
}

/// The top of the colour scale for a result: the speed 98 % of the air stays
/// under, rounded up to a round number. One fast jet at a fan cannot flatten
/// the colours of the rest; cells above the top get the top colour.
pub(crate) fn speed_scale(snap: &CfdSnapshot) -> f32 {
    if !snap.is_consistent() {
        return NICE_TOPS[0];
    }
    let speeds: Vec<f32> = snap
        .velocity
        .iter()
        .zip(&snap.material)
        .filter(|(_, m)| **m == 0)
        .map(|(v, _)| (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
        .collect();
    speed_scale_of(&speeds)
}

/// [`speed_scale`] for plain speeds, m/s (the flat view's plane).
pub(crate) fn speed_scale_of(speeds: &[f32]) -> f32 {
    let mut speeds: Vec<f32> = speeds.iter().copied().filter(|s| s.is_finite()).collect();
    if speeds.is_empty() {
        return NICE_TOPS[0];
    }
    let k = ((speeds.len() as f32 * 0.98) as usize).min(speeds.len() - 1);
    let p98 = *speeds.select_nth_unstable_by(k, f32::total_cmp).1;
    nice_top(p98)
}

/// The first round scale top at or above `top`.
pub(crate) fn nice_top(top: f32) -> f32 {
    let top = if top.is_finite() { top } else { 0.0 };
    NICE_TOPS
        .iter()
        .copied()
        .find(|n| *n >= top)
        .unwrap_or_else(|| (top / 5.0).ceil() * 5.0)
}

/// Colour of air at `speed` on a scale ending at `scale_top`.
pub(crate) fn colour(speed: f32, scale_top: f32) -> [u8; 3] {
    speed_rgb(speed / scale_top.max(1e-6))
}

/// Air slower than this share of the scale is not drawn as a line or arrow.
pub(crate) const QUIET_SHARE: f32 = 0.03;

/// The lines of `flow` worth drawing at `strength`: the ones that carry the
/// most air (mean speed times length) first, never fewer than `MIN_LINES`,
/// and none that is nearly still. Order is kept.
pub(crate) fn thinned(flow: &Flow, strength: f32, scale_top: f32) -> Flow {
    const MIN_LINES: usize = 6;
    let n = flow.lines.len();
    if n == 0 {
        return Flow::default();
    }
    let share = 0.2 + 0.8 * strength.clamp(0.0, 1.0);
    let keep = ((n as f32 * share).round() as usize).clamp(MIN_LINES.min(n), n);
    let mean = |l: &FlowLine| l.speed.iter().sum::<f32>() / l.speed.len().max(1) as f32;
    let weight = |l: &FlowLine| mean(l) * l.along.last().copied().unwrap_or(0.0);
    let mut rank: Vec<usize> = (0..n).collect();
    rank.sort_by(|a, b| weight(&flow.lines[*b]).total_cmp(&weight(&flow.lines[*a])));
    let quiet = QUIET_SHARE * scale_top;
    let mut chosen: Vec<usize> = rank
        .into_iter()
        .take(keep)
        .filter(|i| mean(&flow.lines[*i]) >= quiet)
        .collect();
    chosen.sort_unstable();
    let lines: Vec<FlowLine> = chosen.into_iter().map(|i| flow.lines[i].clone()).collect();
    let top = lines
        .iter()
        .flat_map(|l| l.speed.iter())
        .copied()
        .fold(0.0, f32::max);
    Flow { lines, top }
}

/// How many of `n` lines a strength keeps: from a fifth (at least six) to all.
pub(crate) fn kept_count(n: usize, strength: f32) -> usize {
    ((n as f32 * (0.2 + 0.8 * strength.clamp(0.0, 1.0))).round() as usize)
        .max(6)
        .min(n)
}

/// A small key in the bottom left corner of a painter's clip rectangle: a
/// title, the speed bar and one line with its range in m/s and the word for
/// its top end, for the flat Cabinet view.
pub(crate) fn paint_key(painter: &egui::Painter, scale_top: f32, title: &str) {
    let pal = crate::theme::ui_palette();
    let clip = painter.clip_rect();
    let origin = egui::pos2(clip.min.x + 12.0, clip.max.y - 38.0);
    let bar = egui::Rect::from_min_size(origin, egui::vec2(150.0, 8.0));
    painter.rect_filled(
        egui::Rect::from_min_max(
            egui::pos2(bar.min.x - 6.0, bar.min.y - 18.0),
            egui::pos2(bar.max.x + 150.0, bar.max.y + 20.0),
        ),
        4.0,
        pal.raised.gamma_multiply(0.85),
    );
    let step = bar.width() / 32.0;
    for i in 0..32 {
        let [r, g, b] = speed_rgb((i as f32 + 0.5) / 32.0);
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(bar.min.x + i as f32 * step, bar.min.y),
                egui::vec2(step + 0.5, bar.height()),
            ),
            0.0,
            // Speed ramp: data colour, not a theme colour.
            egui::Color32::from_rgb(r, g, b),
        );
    }
    let font = egui::FontId::proportional(10.0);
    painter.text(
        egui::pos2(bar.min.x, bar.min.y - 4.0),
        egui::Align2::LEFT_BOTTOM,
        title,
        font.clone(),
        pal.text_primary,
    );
    painter.text(
        egui::pos2(bar.min.x, bar.max.y + 3.0),
        egui::Align2::LEFT_TOP,
        format!(
            "0 to {} m/s, top end is {} air (model)",
            trim(scale_top),
            speed_word(scale_top)
        ),
        font,
        pal.text_secondary,
    );
}

/// Line widths, px, for slow, medium and fast air, and the share of the scale
/// where each starts.
const WIDTHS: [(f32, f32); 3] = [(0.0, 1.8), (0.2, 3.0), (0.5, 4.4)];

fn width_class(share: f32) -> usize {
    WIDTHS
        .iter()
        .rposition(|(from, _)| share >= *from)
        .unwrap_or(0)
}

/// Distance between arrowheads on a line, mm, and the size of one.
fn head_spacing_mm(strength: f32) -> f32 {
    260.0 - 130.0 * strength.clamp(0.0, 1.0)
}
const HEAD_LEN_MM: f32 = 18.0;

/// The streamlines as three overlays by speed: thin for slow air, thick for
/// fast, each line coloured by its speed on the shared scale, with an
/// arrowhead now and then that points downwind. Slower air is drawn fainter.
pub(crate) fn stream_overlays(
    flow: &Flow,
    view: &FlowView,
    scale_top: f32,
    order: i32,
) -> Vec<Overlay> {
    let flow = thinned(flow, view.strength, scale_top);
    let mut classes: [OverlayLines; 3] = std::array::from_fn(|i| OverlayLines {
        width_px: WIDTHS[i].1,
        order,
        ..Default::default()
    });
    let rgba = |v: f32| {
        let c = colour(v, scale_top);
        let share = (v / scale_top.max(1e-6)).clamp(0.0, 1.0);
        [c[0], c[1], c[2], (150.0 + 105.0 * share) as u8]
    };
    let heads_every = head_spacing_mm(view.strength);
    for line in &flow.lines {
        let mean = line.speed.iter().sum::<f32>() / line.speed.len().max(1) as f32;
        let class = width_class(mean / scale_top.max(1e-6));
        let out = &mut classes[class];
        out.starts.push(out.positions.len() as u32);
        for (p, v) in line.points.iter().zip(&line.speed) {
            out.positions.push(*p);
            out.colors.push(rgba(*v));
        }
        let len = line.along.last().copied().unwrap_or(0.0);
        let mut s = 0.5 * heads_every;
        while s < len {
            let (a, b) = (at(line, (s - 3.0).max(0.0)), at(line, (s + 3.0).min(len)));
            let v = line.speed[line
                .along
                .partition_point(|x| *x <= s)
                .min(line.speed.len() - 1)];
            let dir = (b - a).normalize_or_zero();
            if dir != Vec3::ZERO && v >= QUIET_SHARE * scale_top {
                let tip = at(line, s);
                let up = if dir.y.abs() > 0.95 { Vec3::X } else { Vec3::Y };
                let side = dir.cross(up).normalize_or_zero() * (0.5 * HEAD_LEN_MM);
                let back = -dir * HEAD_LEN_MM;
                out.starts.push(out.positions.len() as u32);
                for p in [tip + back + side, tip, tip + back - side] {
                    out.positions.push(p);
                    out.colors.push(rgba(v));
                }
            }
            s += heads_every;
        }
    }
    classes
        .into_iter()
        .filter(|c| !c.positions.is_empty())
        .map(Overlay::Lines)
        .collect()
}

/// The point at distance `s` along the line.
fn at(line: &FlowLine, s: f32) -> Vec3 {
    let i = line
        .along
        .partition_point(|a| *a <= s)
        .clamp(1, line.points.len() - 1);
    let (a, b) = (line.along[i - 1], line.along[i]);
    let t = if b > a {
        ((s - a) / (b - a)).clamp(0.0, 1.0)
    } else {
        0.0
    };
    line.points[i - 1].lerp(line.points[i], t)
}

/// The slice plane coloured by air speed on the shared scale. Cells inside
/// solids are left out so devices stay visible.
pub(crate) fn speed_map(
    snap: &CfdSnapshot,
    place: Place,
    axis: Axis,
    at: f32,
    scale_top: f32,
    alpha: u8,
) -> Option<OverlayMesh> {
    const SLICE_CELLS: f64 = 120.0;
    let offset = super::heat_3d::slice_offset(snap, axis, at);
    let window = cfd::whole_window(snap, axis);
    let cell = (window.w.max(window.h) / SLICE_CELLS).max(4.0);
    let s = cfd::slice(
        snap,
        Plane {
            axis,
            offset_mm: offset,
        },
        window,
        cell,
    )?;
    let g = s.speed.grid;
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
    for r in 0..g.rows {
        for c in 0..g.cols {
            if s.solid[r * g.cols + c] {
                continue;
            }
            let rgb = colour(s.speed.values[r * g.cols + c], scale_top);
            let colour = [rgb[0], rgb[1], rgb[2], alpha];
            let (u0, v0) = (
                window.u0 + c as f64 * g.cell_w,
                window.v0 + r as f64 * g.cell_h,
            );
            let (u1, v1) = (u0 + g.cell_w, v0 + g.cell_h);
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
// Through-flow: how much air goes through the openings, and whether it is
// enough.

/// Less air than this through all openings counts as none, m3/h.
const NO_FLOW_M3H: f64 = 5.0;
/// Air warms by `FAN_FACTOR * loss / flow` K (flow in m3/h, loss in W): the
/// sizing rule the rest of the program uses.
const FAN_FACTOR: f64 = novo_layout::sizing::FAN_FACTOR;
/// Verdict limits for the warming of the air passing through, K.
const ADEQUATE_UP_TO_K: f64 = 5.0;
const LOW_UP_TO_K: f64 = 10.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verdict {
    /// No opening, or none that moves air: the cabinet gives its heat off
    /// through its walls alone.
    None,
    VeryLow,
    Low,
    Adequate,
}

impl Verdict {
    pub(crate) fn word(self) -> &'static str {
        match self {
            Verdict::None => "none",
            Verdict::VeryLow => "very low",
            Verdict::Low => "low",
            Verdict::Adequate => "adequate",
        }
    }
}

/// One opening's share of the air.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VentFlow {
    pub name: String,
    /// Positive: air in. m3/h.
    pub m3h: f64,
}

/// The air through all openings of a result.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Through {
    pub vents: Vec<VentFlow>,
    pub inflow_m3h: f64,
    pub outflow_m3h: f64,
    pub loss_w: f64,
    /// The heat the air carries out through the openings, W: a model value
    /// from the air speed and temperature beside them.
    pub carried_w: Option<f64>,
    pub verdict: Verdict,
    /// How much the air warms passing through, K; `None` without flow or
    /// without losses.
    pub warming_k: Option<f64>,
}

fn vent_name(v: &VentSpec) -> String {
    let kind = if v.kind.is_fan() { "Fan" } else { "Grille" };
    format!("{kind} {}", super::heat_openings::wall_phrase(v.wall))
}

/// The summary for `specs` and the flows measured through them (same order,
/// m3/h, positive into the cabinet) in a result with `loss_w` of device loss.
/// `None` when there is no opening or no reading to summarise.
pub(crate) fn through(
    specs: &[VentSpec],
    flows: &[Option<f64>],
    loss_w: f64,
    carried_w: Option<f64>,
) -> Option<Through> {
    if specs.is_empty() || flows.is_empty() {
        return None;
    }
    let (mut inflow, mut outflow) = (0.0_f64, 0.0_f64);
    let mut vents = Vec::new();
    for (v, q) in specs.iter().zip(flows) {
        let Some(q) = q else { continue };
        match super::heat_openings::role_of(v, Some(*q)) {
            Role::In => inflow += q.abs(),
            Role::Out => outflow += q.abs(),
            Role::Open => {}
        }
        vents.push(VentFlow {
            name: vent_name(v),
            m3h: *q,
        });
    }
    if vents.is_empty() {
        return None;
    }
    let flow = 0.5 * (inflow + outflow);
    let warming_k = (flow >= NO_FLOW_M3H && loss_w > 0.0).then(|| FAN_FACTOR * loss_w / flow);
    let verdict = match warming_k {
        _ if flow < NO_FLOW_M3H => Verdict::None,
        None => Verdict::Adequate,
        Some(k) if k <= ADEQUATE_UP_TO_K => Verdict::Adequate,
        Some(k) if k <= LOW_UP_TO_K => Verdict::Low,
        Some(_) => Verdict::VeryLow,
    };
    Some(Through {
        vents,
        inflow_m3h: inflow,
        outflow_m3h: outflow,
        loss_w,
        carried_w,
        verdict,
        warming_k,
    })
}

impl Through {
    /// The through-flow in m3/h: the mean of what goes in and what comes out
    /// (the solver conserves air; the two differ by the reading only).
    pub(crate) fn total_m3h(&self) -> f64 {
        0.5 * (self.inflow_m3h + self.outflow_m3h)
    }

    /// The one-line summary with the verdict.
    pub(crate) fn text(&self) -> String {
        let total = self.total_m3h();
        let head = format!(
            "Through-flow {total:.0} m³/h (air in {:.0}, out {:.0})",
            self.inflow_m3h, self.outflow_m3h
        );
        match (self.verdict, self.warming_k) {
            (Verdict::None, _) => format!(
                "{head}: negligible (under {NO_FLOW_M3H:.0} m³/h, below the solver's reading limit), so the heat leaves mostly through the walls (model)"
            ),
            (v, Some(k)) => format!(
                "{head} for {:.0} W of device loss: the air warms about {k:.0} K passing through, {} cooling{} (model)",
                self.loss_w,
                v.word(),
                self.carried_text()
            ),
            (v, None) => format!("{head}: {} cooling (model)", v.word()),
        }
    }

    /// How much of the loss the air carries out, when it is known: the rest
    /// leaves through the walls, the plate and by radiation.
    fn carried_text(&self) -> String {
        match self.carried_w {
            Some(w) if self.loss_w > 0.0 && w.is_finite() => format!(
                "; the air carries out about {:.0} W ({:.0} %)",
                w.max(0.0),
                (100.0 * w / self.loss_w).clamp(0.0, 999.0)
            ),
            _ => String::new(),
        }
    }

    /// What the verdict rests on, for the hover.
    pub(crate) fn hover(&self) -> String {
        let mut s = String::from("Per opening (positive is into the cabinet): ");
        let parts: Vec<String> = self
            .vents
            .iter()
            .map(|v| format!("{} {:+.0} m³/h", v.name, v.m3h))
            .collect();
        s.push_str(&parts.join(", "));
        s.push_str(&format!(
            ". Verdict from how much the air warms passing through: up to {ADEQUATE_UP_TO_K:.0} K adequate, up to {LOW_UP_TO_K:.0} K low, more very low. Model values from the solved air speed beside each opening; the warming ignores heat the walls give off, so it is on the high side. Planning limits, not a standard."
        ));
        s
    }
}

// ---------------------------------------------------------------------------
// The control and the legend.

/// The Flow control of the Heat strip: mode and strength. True when changed.
pub(crate) fn control(ui: &mut egui::Ui, view: &mut FlowView) -> bool {
    let before = *view;
    ui.label("Flow");
    egui::ComboBox::from_id_salt("heat_flow_mode")
        .selected_text(view.mode.label())
        .show_ui(ui, |ui| {
            for m in FlowMode::ALL {
                ui.selectable_value(&mut view.mode, m, m.label())
                    .on_hover_text(m.hint());
            }
        })
        .response
        .on_hover_text("How the airflow is drawn. Colour, width and length all mean air speed in m/s; the key shows the scale");
    if view.mode != FlowMode::Off {
        ui.add(
            egui::Slider::new(&mut view.strength, 0.0..=1.0)
                .show_value(false)
                .text("Strength"),
        )
        .on_hover_text("How many lines and arrows to show: low keeps only the main streams, high shows every one found");
    }
    before != *view
}

/// The through-flow line of the Heat strip: the numbers and the verdict, the
/// verdict worded as well as coloured.
pub(crate) fn through_row(ui: &mut egui::Ui, t: &Through) {
    let pal = crate::theme::ui_palette();
    let colour = match t.verdict {
        Verdict::Adequate => pal.success,
        Verdict::Low => pal.warning,
        Verdict::VeryLow => pal.error,
        // Too little to read is not proof of no air: warning, not error.
        Verdict::None => pal.warning,
    };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
    ui.painter().circle_filled(rect.center(), 4.0, colour);
    ui.label(egui::RichText::new(t.text()).small())
        .on_hover_text(t.hover());
}

/// The key of the heat flow layer: a bar from nothing to the strongest flux
/// in W/m², in the colour the lines are drawn in, brighter for more.
pub(crate) fn flux_legend(ui: &mut egui::Ui, top_flux: f32) {
    let pal = crate::theme::ui_palette();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(BAR_LEN + 70.0, BAR_H + 28.0),
        egui::Sense::hover(),
    );
    response.on_hover_text(
        "Heat flow: orange lines follow the heat the air carries away, brighter is more heat per area of air, in W/m² (rho cp (T - ambient) u). Derived from the solved temperature and air speed; radiation is not included. A model value, not a measurement",
    );
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + 12.0, rect.min.y),
        egui::vec2(BAR_LEN, BAR_H),
    );
    let painter = ui.painter();
    let step = BAR_LEN / STEPS as f32;
    let [r, g, b] = super::heat_carry::RGB;
    for i in 0..STEPS {
        let share = (i as f32 + 0.5) / STEPS as f32;
        // The lines go from 60 to 255 in opacity over the scale.
        let alpha = (60.0 + 195.0 * share) as u8;
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(bar.min.x + i as f32 * step, bar.min.y),
                egui::vec2(step + 0.5, BAR_H),
            ),
            0.0,
            // Heat flow line colour (data, not theme) over the sunken panel.
            pal.sunken
                .blend(egui::Color32::from_rgba_unmultiplied(r, g, b, alpha)),
        );
    }
    let small = egui::FontId::proportional(10.0);
    for frac in [0.0_f32, 0.5, 1.0] {
        let x = bar.min.x + frac * BAR_LEN;
        painter.line_segment(
            [egui::pos2(x, bar.max.y), egui::pos2(x, bar.max.y + 3.0)],
            egui::Stroke::new(1.0, pal.text_secondary),
        );
        painter.text(
            egui::pos2(x, bar.max.y + 4.0),
            egui::Align2::CENTER_TOP,
            format!("{:.0}", top_flux * frac),
            small.clone(),
            pal.text_secondary,
        );
    }
    painter.text(
        egui::pos2(bar.max.x + 8.0, bar.min.y - 1.0),
        egui::Align2::LEFT_TOP,
        "W/m²",
        small.clone(),
        pal.text_secondary,
    );
    painter.text(
        egui::pos2(bar.min.x + 0.5 * BAR_LEN, bar.max.y + 15.0),
        egui::Align2::CENTER_TOP,
        "heat carried by the air (model)",
        small,
        pal.text_tertiary,
    );
}

const BAR_LEN: f32 = 190.0;
const BAR_H: f32 = 10.0;
const STEPS: usize = 48;

/// The key of the flow (the `Color32`s of the bar are the speed ramp, data and
/// not theme colours): a bar from still air to the top of the scale in m/s,
/// the word for each stretch of it, and what is drawn from where.
pub(crate) fn legend(ui: &mut egui::Ui, view: &FlowView, scale_top: f32) {
    let pal = crate::theme::ui_palette();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(BAR_LEN + 70.0, BAR_H + 28.0),
        egui::Sense::hover(),
    );
    response.on_hover_text(legend_hover(view.mode));
    let bar = egui::Rect::from_min_size(
        egui::pos2(rect.min.x + 12.0, rect.min.y),
        egui::vec2(BAR_LEN, BAR_H),
    );
    let painter = ui.painter();
    let step = BAR_LEN / STEPS as f32;
    for i in 0..STEPS {
        let [r, g, b] = speed_rgb((i as f32 + 0.5) / STEPS as f32);
        painter.rect_filled(
            egui::Rect::from_min_size(
                egui::pos2(bar.min.x + i as f32 * step, bar.min.y),
                egui::vec2(step + 0.5, BAR_H),
            ),
            0.0,
            // Speed ramp: data colour, not a theme colour.
            egui::Color32::from_rgb(r, g, b),
        );
    }
    let small = egui::FontId::proportional(10.0);
    for frac in [0.0_f32, 0.25, 0.5, 0.75, 1.0] {
        let x = bar.min.x + frac * BAR_LEN;
        painter.line_segment(
            [egui::pos2(x, bar.max.y), egui::pos2(x, bar.max.y + 3.0)],
            egui::Stroke::new(1.0, pal.text_secondary),
        );
        painter.text(
            egui::pos2(x, bar.max.y + 4.0),
            egui::Align2::CENTER_TOP,
            trim(scale_top * frac),
            small.clone(),
            pal.text_secondary,
        );
    }
    // The words sit under the stretch of the bar they stand for, where they fit.
    let mut from = 0.0_f32;
    for (top, word) in BANDS
        .iter()
        .map(|(t, w)| (*t, *w))
        .chain([(f32::MAX, "fast")])
    {
        let to = (top / scale_top).min(1.0);
        if to - from >= 0.2 {
            painter.text(
                egui::pos2(bar.min.x + 0.5 * (from + to) * BAR_LEN, bar.max.y + 15.0),
                egui::Align2::CENTER_TOP,
                word,
                small.clone(),
                pal.text_tertiary,
            );
        }
        from = from.max(to);
        if from >= 1.0 {
            break;
        }
    }
    painter.text(
        egui::pos2(bar.max.x + 8.0, bar.min.y - 1.0),
        egui::Align2::LEFT_TOP,
        "m/s",
        small,
        pal.text_secondary,
    );
}

/// A number for a tick: no trailing zeros.
fn trim(v: f32) -> String {
    let s = format!("{v:.2}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

fn legend_hover(mode: FlowMode) -> String {
    let how = match mode {
        FlowMode::Arrows => "Arrow length, width and colour",
        FlowMode::Streamlines => "Line width and colour",
        FlowMode::SpeedMap => "The colour of the plane",
        FlowMode::Off => "Colour",
    };
    format!(
        "{how} show the air speed in m/s; the scale ends at the speed 98 % of the air stays under, rounded up (faster spots such as a fan jet get the end colour). Speeds are the solver's: a planning-level model, not a measurement. Air under 3 % of the scale is not drawn as a line or arrow. The temperature scale is a different colour ramp"
    )
}

/// One note for the legend row: what the mode draws, with the scale in words.
pub(crate) fn note(view: &FlowView, scale_top: f32) -> Option<String> {
    let what = match view.mode {
        FlowMode::Off => return None,
        FlowMode::Arrows => "arrows on the slice plane",
        FlowMode::Streamlines => "streamlines",
        FlowMode::SpeedMap => "slice plane coloured by speed",
    };
    Some(format!(
        "Flow: {what}, scale 0 to {} m/s, top end is {} air (model)",
        trim(scale_top),
        speed_word(scale_top)
    ))
}
