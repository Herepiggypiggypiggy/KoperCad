//! The Heat view of the Cabinet plate: a temperature map, contours, hot
//! spots, airflow and device badges drawn over the plate. Spec:
//! `docs/design/thermal-visuals.md`.
//!
//! Until the solver (`novo-thermal`) is wired in, the field comes from the
//! synthetic model of `novo-heatview` built from the placed devices and their
//! `power_loss_w`; the view says so. Drawing is thin: all numbers come from
//! `novo-heatview`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use novo_canvas::Viewport;
use novo_heatview::contour::{self, Contour};
use novo_heatview::hotspot::{self, Peak};
use novo_heatview::range::{self, Range};
use novo_heatview::streamline::{self, Spacing, Streamline};
use novo_heatview::synthetic::{SynthDevice, SynthScene};
use novo_heatview::{Scalar, Vector};
use novo_theme::heat::{self, Verdict};
use novo_theme::tokens as t;

/// Opacity of the map over the drawing; the devices stay readable through it.
const MAP_ALPHA: f32 = 0.62;
/// Solver cell for the synthetic model, mm.
const CELL_MM: f64 = 10.0;

/// One placed device as the view needs it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Item {
    pub id: u64,
    pub tag: String,
    /// Footprint in plate mm, `y` down.
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub loss_w: f64,
}

/// What the view draws from: the plate and its devices.
pub(super) struct Input {
    pub plate: (f64, f64),
    /// Outside air temperature, degrees C (the sizing request's).
    pub ambient: f64,
    pub items: Vec<Item>,
}

pub(super) struct HeatState {
    pub on: bool,
    pub map: bool,
    pub contours: bool,
    pub airflow: bool,
    pub badges: bool,
    cache: Option<Cache>,
}

struct Cache {
    key: u64,
    smooth: Scalar,
    range: Range,
    contours: Vec<(Contour, bool)>,
    peaks: Vec<(Peak, String)>,
    mean: f32,
    streams: Vec<Streamline>,
    scene: SynthScene,
}

impl HeatState {
    pub(super) fn new() -> HeatState {
        HeatState {
            on: false,
            map: true,
            contours: true,
            airflow: false,
            badges: true,
            cache: None,
        }
    }

    /// `heat.toggle`, `heat.airflow` and `heat.contours`: the layer commands
    /// switch the Heat view on when they show something.
    pub(super) fn command(&mut self, id: &str) {
        match id {
            "heat.toggle" => self.on = !self.on,
            "heat.airflow" => {
                self.airflow = !(self.on && self.airflow);
                self.on |= self.airflow;
            }
            "heat.contours" => {
                self.contours = !(self.on && self.contours);
                self.on |= self.contours;
            }
            _ => {}
        }
    }

    /// The map is drawn under the canvas, which then must not paint its own
    /// background.
    pub(super) fn under_canvas(&self) -> bool {
        self.on && self.map
    }

    fn refresh(&mut self, input: &Input, dragging: bool) -> bool {
        // A device being dragged changes the layout every frame: keep the
        // last field until it lands.
        let stale = self.cache.as_ref().is_none_or(|c| c.key != key(input));
        if stale && (!dragging || self.cache.is_none()) {
            self.cache = Some(build(input));
        }
        self.cache.is_some()
    }
}

fn key(input: &Input) -> u64 {
    let mut h = DefaultHasher::new();
    (
        input.plate.0.to_bits(),
        input.plate.1.to_bits(),
        input.ambient.to_bits(),
    )
        .hash(&mut h);
    for i in &input.items {
        (
            i.id,
            i.x.to_bits(),
            i.y.to_bits(),
            i.w.to_bits(),
            i.h.to_bits(),
            i.loss_w.to_bits(),
        )
            .hash(&mut h);
    }
    h.finish()
}

fn build(input: &Input) -> Cache {
    let scene = SynthScene {
        width: input.plate.0,
        height: input.plate.1,
        ambient: input.ambient,
        devices: input
            .items
            .iter()
            .map(|i| SynthDevice {
                id: i.id,
                x: i.x,
                y: i.y,
                w: i.w,
                h: i.h,
                loss_w: i.loss_w,
            })
            .collect(),
    };
    let smooth = scene.temperature_field(CELL_MM).filled();
    let (_, hottest) = smooth.extent().unwrap_or((35.0, 45.0));
    let range = range::map_range(scene.ambient, f64::from(hottest), None);
    let levels: Vec<f64> = range::contour_levels(range, 8)
        .iter()
        .map(|l| l.0)
        .collect();
    let labelled: Vec<bool> = range::contour_levels(range, 8)
        .iter()
        .map(|l| l.1)
        .collect();
    let mut contours = Vec::new();
    for (level, lab) in levels.iter().zip(labelled) {
        for c in contour::contours_at(&smooth, *level as f32) {
            contours.push((c, lab));
        }
    }
    let mean = smooth.values.iter().sum::<f32>() / smooth.values.len().max(1) as f32;
    let peaks = hotspot::peaks(&smooth, 3, 60.0, 2.0)
        .into_iter()
        .map(|p| {
            let tag = input
                .items
                .iter()
                .min_by(|a, b| distance(a, &p).total_cmp(&distance(b, &p)))
                .map(|i| i.tag.clone())
                .unwrap_or_default();
            (p, tag)
        })
        .collect();
    let velocity: Vector = scene.velocity_field(CELL_MM);
    let mut seeds: Vec<(f64, f64)> = Vec::new();
    let (w, h) = input.plate;
    seeds.extend((0..6).map(|i| (2.0, h - 20.0 - i as f64 * 30.0)));
    seeds.extend((0..8).map(|i| (40.0 + i as f64 * w / 8.0, h - 2.0)));
    for d in &scene.devices {
        if d.loss_w > 20.0 {
            seeds.extend([0.2, 0.5, 0.8].map(|f| (d.x + d.w * f, d.y - 3.0)));
        }
    }
    let streams = streamline::evenly_spaced(&velocity, &seeds, Spacing::default(), &|x, y| {
        scene.solid(x, y)
    });
    Cache {
        key: key(input),
        smooth,
        mean,
        range,
        contours,
        peaks,
        streams,
        scene,
    }
}

/// Distance from a peak to a device rectangle, mm (0 inside).
fn distance(i: &Item, p: &Peak) -> f64 {
    let dx = (i.x - p.x).max(0.0).max(p.x - (i.x + i.w));
    let dy = (i.y - p.y).max(0.0).max(p.y - (i.y + i.h));
    dx.hypot(dy)
}

fn rgb(c: [u8; 3], alpha: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        c[0],
        c[1],
        c[2],
        (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
    )
}

/// The Heat chip and layer chips of the Cabinet header.
pub(super) fn chips(ui: &mut egui::Ui, state: &mut HeatState) {
    if ui
        .selectable_label(state.on, "Heat")
        .on_hover_text("Show how heat spreads over the plate (Shift+H)")
        .clicked()
    {
        state.on = !state.on;
    }
    if state.on {
        let wide = ui.available_width() > 360.0;
        let layers = |ui: &mut egui::Ui, state: &mut HeatState| {
            let chip = |ui: &mut egui::Ui, on: &mut bool, label: &str| {
                if ui.selectable_label(*on, label).clicked() {
                    *on = !*on;
                }
            };
            chip(ui, &mut state.map, "Map");
            chip(ui, &mut state.contours, "Contours");
            chip(ui, &mut state.airflow, "Airflow");
            chip(ui, &mut state.badges, "Badges");
        };
        if wide {
            layers(ui, state);
        } else {
            ui.menu_button("Layers", |ui| layers(ui, state));
        }
    }
}

fn screen_fn<'a>(rect: Rect, viewport: &'a Viewport) -> impl Fn(f64, f64) -> Pos2 + 'a {
    move |x, y| {
        let s = viewport.world_to_screen(novo_canvas::Vec2::new(x as f32, y as f32));
        rect.min + Vec2::new(s.x, s.y)
    }
}

/// Under the canvas, before it is shown: a backdrop in the canvas colour and
/// the temperature map, so devices, wires and marks draw over the map.
pub(super) fn paint_under(
    ui: &egui::Ui,
    rect: Rect,
    viewport: &Viewport,
    state: &mut HeatState,
    input: &Input,
    dragging: bool,
    backdrop: Color32,
) {
    if !state.under_canvas() || input.plate.0 <= 0.0 || input.plate.1 <= 0.0 {
        return;
    }
    if !state.refresh(input, dragging) {
        return;
    }
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, backdrop);
    if let Some(cache) = state.cache.as_ref() {
        paint_map(&painter, cache, &screen_fn(rect, viewport));
    }
}

/// Over the canvas: contours, airflow, device rims, badges, hot spots and
/// the legend.
pub(super) fn paint_over(
    ui: &egui::Ui,
    rect: Rect,
    viewport: &Viewport,
    state: &mut HeatState,
    input: &Input,
    dragging: bool,
) {
    if !state.on || input.plate.0 <= 0.0 || input.plate.1 <= 0.0 || !state.refresh(input, dragging)
    {
        return;
    }
    let Some(cache) = state.cache.as_ref() else {
        return;
    };
    let painter = ui.painter_at(rect);
    let to_screen = screen_fn(rect, viewport);
    if state.airflow {
        paint_airflow(&painter, cache, &to_screen);
    }
    if state.contours {
        paint_contours(&painter, cache, &to_screen);
    }
    paint_rims(&painter, cache, input, &to_screen);
    if state.badges {
        paint_badges(&painter, cache, input, &to_screen, viewport.zoom);
    }
    paint_peaks(&painter, cache, &to_screen);
    legend(ui.ctx(), rect, cache);
}

fn paint_map(painter: &egui::Painter, cache: &Cache, to_screen: &dyn Fn(f64, f64) -> Pos2) {
    // A mesh with a colour per vertex: the GPU blends between them, so the
    // field reads smooth whatever the cell size.
    let (w, h) = (cache.scene.width, cache.scene.height);
    let (cols, rows) = (
        ((w / 5.0) as usize).clamp(2, 160),
        ((h / 5.0) as usize).clamp(2, 160),
    );
    let grid = cache.smooth.resampled(cols, rows);
    let mut mesh = egui::Mesh::default();
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = grid.grid.centre(c, r);
            let v = f64::from(grid.at(c, r));
            let color = rgb(heat::ramp(cache.range.t(v)), MAP_ALPHA);
            // Cell centres stretched to the plate edge so the map fills it.
            let (px, py) = (
                if c == 0 {
                    0.0
                } else if c == cols - 1 {
                    w
                } else {
                    x
                },
                if r == 0 {
                    0.0
                } else if r == rows - 1 {
                    h
                } else {
                    y
                },
            );
            mesh.colored_vertex(to_screen(px, py), color);
        }
    }
    for r in 0..rows - 1 {
        for c in 0..cols - 1 {
            let i = (r * cols + c) as u32;
            let n = cols as u32;
            mesh.add_triangle(i, i + 1, i + n);
            mesh.add_triangle(i + 1, i + n + 1, i + n);
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn paint_contours(painter: &egui::Painter, cache: &Cache, to_screen: &dyn Fn(f64, f64) -> Pos2) {
    for (c, labelled) in &cache.contours {
        let pts: Vec<Pos2> = c.points.iter().map(|p| to_screen(p.0, p.1)).collect();
        let alpha = if *labelled { 0.65 } else { 0.32 };
        painter.add(Shape::line(
            pts,
            Stroke::new(
                if *labelled { 1.2 } else { 0.8 },
                rgb([0x0E, 0x0E, 0x10], alpha),
            ),
        ));
        if *labelled && let Some((at, _)) = c.label_spot(20.0) {
            let pos = to_screen(at.0, at.1);
            let text = format!("{:.0}", c.level);
            let font = FontId::monospace(t::FONT_SMALL);
            for d in [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)] {
                painter.text(
                    pos + Vec2::new(d.0, d.1),
                    Align2::CENTER_CENTER,
                    &text,
                    font.clone(),
                    t::BG_0,
                );
            }
            painter.text(pos, Align2::CENTER_CENTER, text, font, t::TEXT);
        }
    }
}

fn paint_airflow(painter: &egui::Painter, cache: &Cache, to_screen: &dyn Fn(f64, f64) -> Pos2) {
    let top = cache
        .streams
        .iter()
        .flat_map(|s| s.speeds.iter())
        .copied()
        .fold(0.0, f32::max)
        .max(1e-6);
    for s in &cache.streams {
        let pts: Vec<Pos2> = s.points.iter().map(|p| to_screen(p.0, p.1)).collect();
        let mean = s.speeds.iter().sum::<f32>() / s.speeds.len().max(1) as f32;
        let width = 1.0 + 1.4 * (mean / top).clamp(0.0, 1.0);
        painter.add(Shape::line(
            pts.clone(),
            Stroke::new(width + 1.6, rgb([0x0E, 0x0E, 0x10], 0.5)),
        ));
        painter.add(Shape::line(pts.clone(), Stroke::new(width, t::INFO)));
        // An arrowhead about every 70 points of screen length.
        let mut run = 0.0;
        for w in pts.windows(2) {
            run += (w[1] - w[0]).length();
            if run >= 70.0 {
                run = 0.0;
                let d = (w[1] - w[0]).normalized();
                let n = Vec2::new(-d.y, d.x);
                painter.add(Shape::line(
                    vec![w[1] - d * 6.0 + n * 3.5, w[1], w[1] - d * 6.0 - n * 3.5],
                    Stroke::new(width, t::INFO),
                ));
            }
        }
    }
}

fn paint_rims(
    painter: &egui::Painter,
    cache: &Cache,
    input: &Input,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
) {
    for i in &input.items {
        let temp = device_temp(cache, i);
        let a = to_screen(i.x, i.y);
        let b = to_screen(i.x + i.w, i.y + i.h);
        let color = rgb(heat::ramp(cache.range.t(temp)), 1.0);
        painter.rect_stroke(
            Rect::from_two_pos(a, b),
            2.0,
            Stroke::new(2.0, color),
            egui::StrokeKind::Inside,
        );
    }
}

/// The hottest air cell around the device, degrees C.
fn device_temp(cache: &Cache, i: &Item) -> f64 {
    let g = cache.smooth.grid;
    let (c0, r0) = g.cell_at(i.x - CELL_MM, i.y - CELL_MM);
    let (c1, r1) = g.cell_at(i.x + i.w + CELL_MM, i.y + i.h + CELL_MM);
    let mut best = f32::MIN;
    for r in r0..=r1 {
        for c in c0..=c1 {
            best = best.max(cache.smooth.at(c, r));
        }
    }
    f64::from(best)
}

fn paint_badges(
    painter: &egui::Painter,
    cache: &Cache,
    input: &Input,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    zoom: f32,
) {
    let font = FontId::monospace(t::FONT_SMALL);
    for i in input.items.iter().filter(|i| i.loss_w > 0.0) {
        // Only where the pill fits inside the device.
        if (i.w as f32) * zoom < 96.0 || (i.h as f32) * zoom < 30.0 {
            continue;
        }
        let temp = device_temp(cache, i);
        let (verdict, dot) = (
            heat::verdict(cache.scene.ambient, temp, None),
            t::TEXT_MUTED,
        );
        let dot = match verdict {
            Verdict::Ok => t::SUCCESS,
            Verdict::Near => t::WARNING,
            Verdict::Over => t::ERROR,
            Verdict::Unknown => dot,
        };
        let br = to_screen(i.x + i.w, i.y + i.h);
        let pill = Rect::from_min_size(br - Vec2::new(94.0, 26.0), Vec2::new(88.0, 20.0));
        painter.rect(
            pill,
            10.0,
            t::BG_2,
            Stroke::new(1.0, t::BORDER_STRONG),
            egui::StrokeKind::Inside,
        );
        painter.circle_filled(pill.left_center() + Vec2::new(11.0, 0.0), 4.0, dot);
        painter.text(
            pill.left_center() + Vec2::new(20.0, 0.0),
            Align2::LEFT_CENTER,
            format!("{temp:.0} °C"),
            font.clone(),
            t::TEXT,
        );
    }
}

fn paint_peaks(painter: &egui::Painter, cache: &Cache, to_screen: &dyn Fn(f64, f64) -> Pos2) {
    let title = FontId::proportional(t::FONT_SMALL);
    let mut placed: Vec<Rect> = Vec::new();
    for (p, tag) in &cache.peaks {
        let at = to_screen(p.x, p.y);
        painter.circle_stroke(at, 4.0, Stroke::new(1.5, t::TEXT));
        let mut card = Rect::from_min_size(at + Vec2::new(14.0, -30.0), Vec2::new(150.0, 34.0));
        // Cards never overlap each other: step down until there is room.
        while placed.iter().any(|q| q.intersects(card.expand(2.0))) {
            card = card.translate(Vec2::new(0.0, 38.0));
        }
        placed.push(card);
        painter.line_segment([at, card.left_center()], Stroke::new(1.0, t::TEXT));
        painter.rect(
            card,
            6.0,
            t::BG_2,
            Stroke::new(1.0, t::BORDER_STRONG),
            egui::StrokeKind::Inside,
        );
        let head = if tag.is_empty() {
            format!("{:.0} °C", p.value)
        } else {
            format!("{tag}  {:.0} °C", p.value)
        };
        painter.text(
            card.min + Vec2::new(8.0, 5.0),
            Align2::LEFT_TOP,
            head,
            title.clone(),
            t::TEXT,
        );
        painter.text(
            card.min + Vec2::new(8.0, 19.0),
            Align2::LEFT_TOP,
            format!("{:.0} K above the plate mean", p.value - cache.mean),
            title.clone(),
            t::TEXT_MUTED,
        );
    }
}

fn legend(ctx: &egui::Context, rect: Rect, cache: &Cache) {
    egui::Area::new(egui::Id::new("heat_legend"))
        .fixed_pos(rect.right_top() + Vec2::new(-176.0, 12.0))
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).show(ui, |ui| {
                ui.set_width(140.0);
                ui.label(
                    egui::RichText::new("TEMPERATURE")
                        .small()
                        .strong()
                        .color(t::TEXT_MUTED),
                );
                let height = 180.0;
                let (bar, _) =
                    ui.allocate_exact_size(Vec2::new(14.0, height), egui::Sense::hover());
                let mut mesh = egui::Mesh::default();
                let steps = 24;
                for s in 0..=steps {
                    let f = s as f32 / steps as f32;
                    let y = bar.bottom() - f * height;
                    let c = rgb(heat::ramp(f), 1.0);
                    mesh.colored_vertex(Pos2::new(bar.left(), y), c);
                    mesh.colored_vertex(Pos2::new(bar.right(), y), c);
                }
                for s in 0..steps as u32 {
                    let i = s * 2;
                    mesh.add_triangle(i, i + 1, i + 2);
                    mesh.add_triangle(i + 1, i + 3, i + 2);
                }
                let p = ui.painter();
                p.add(Shape::mesh(mesh));
                for tick in range::ticks(cache.range) {
                    let y = bar.bottom() - cache.range.t(tick) * height;
                    p.line_segment(
                        [Pos2::new(bar.right(), y), Pos2::new(bar.right() + 5.0, y)],
                        Stroke::new(1.0, t::TEXT_MUTED),
                    );
                    p.text(
                        Pos2::new(bar.right() + 9.0, y),
                        Align2::LEFT_CENTER,
                        format!("{tick:.0}"),
                        FontId::monospace(t::FONT_SMALL),
                        t::TEXT_MUTED,
                    );
                }
                ui.label(
                    egui::RichText::new(format!("°C · ambient {:.0}", cache.scene.ambient))
                        .small()
                        .color(t::TEXT_MUTED),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Preview model from the part losses, not a solver result.")
                        .small()
                        .color(t::TEXT_MUTED),
                );
            });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input {
            plate: (600.0, 800.0),
            ambient: 35.0,
            items: vec![
                Item {
                    id: 1,
                    tag: "-U1".into(),
                    x: 100.0,
                    y: 100.0,
                    w: 150.0,
                    h: 200.0,
                    loss_w: 80.0,
                },
                Item {
                    id: 2,
                    tag: "-K1".into(),
                    x: 350.0,
                    y: 100.0,
                    w: 45.0,
                    h: 90.0,
                    loss_w: 2.0,
                },
            ],
        }
    }

    #[test]
    fn the_view_builds_from_devices() {
        let c = build(&input());
        assert!(c.range.hi > c.range.lo);
        assert!(!c.contours.is_empty());
        assert!(!c.peaks.is_empty());
        assert!(!c.streams.is_empty());
        assert!(c.smooth.extent().is_some());
    }

    #[test]
    fn the_cache_key_follows_the_layout() {
        let a = input();
        let mut b = input();
        assert_eq!(key(&a), key(&b));
        b.items[0].x += 10.0;
        assert_ne!(key(&a), key(&b));
        b.items[0].x -= 10.0;
        b.items[0].loss_w += 1.0;
        assert_ne!(key(&a), key(&b));
    }

    #[test]
    fn ambient_is_part_of_the_key_and_the_scene() {
        let a = input();
        let mut b = input();
        b.ambient = 45.0;
        assert_ne!(key(&a), key(&b));
        assert_eq!(build(&b).scene.ambient, 45.0);
    }

    #[test]
    fn the_field_waits_for_a_drag_to_end() {
        let mut state = HeatState::new();
        let mut i = input();
        assert!(state.refresh(&i, false));
        let first = state.cache.as_ref().unwrap().key;
        i.items[0].x += 20.0;
        state.refresh(&i, true);
        assert_eq!(
            state.cache.as_ref().unwrap().key,
            first,
            "kept while dragging"
        );
        state.refresh(&i, false);
        assert_ne!(
            state.cache.as_ref().unwrap().key,
            first,
            "rebuilt on release"
        );
    }

    #[test]
    fn layer_commands_switch_the_view_on() {
        let mut s = HeatState::new();
        s.command("heat.toggle");
        assert!(s.on);
        s.command("heat.toggle");
        assert!(!s.on);
        s.command("heat.airflow");
        assert!(s.on && s.airflow);
        s.command("heat.airflow");
        assert!(!s.airflow);
        s.on = false;
        s.command("heat.contours");
        assert!(s.on && s.contours);
    }

    #[test]
    fn peaks_are_named_after_the_nearest_device() {
        let c = build(&input());
        assert_eq!(c.peaks[0].1, "-U1");
    }

    #[test]
    fn the_drive_runs_hotter_than_the_relay() {
        let i = input();
        let c = build(&i);
        assert!(device_temp(&c, &i.items[0]) > device_temp(&c, &i.items[1]));
    }
}
