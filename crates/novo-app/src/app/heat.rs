//! The Heat view of the Cabinet plate: a temperature map, contours, hot
//! spots, airflow and device badges drawn over the plate. Spec:
//! `docs/design/thermal-visuals.md`.
//!
//! Before a run the field comes from the synthetic model of `novo-heatview`
//! built from the placed devices and their `power_loss_w`; the view says so.
//! "Simulate" (`heat.simulate`) builds a scenario from the project, runs the
//! solver on a worker thread and replaces the synthetic field with the
//! result; the legend names the fidelity. Drawing is thin: all numbers come
//! from `novo-heatview`.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Shape, Stroke, Vec2};
use novo_canvas::Viewport;
use novo_heatview::cfd::{self, Window};
use novo_heatview::contour::{self, Contour};
use novo_heatview::hotspot::{self, Peak};
use novo_heatview::range::{self, Range};
use novo_heatview::streamline::{self, Spacing, Streamline};
use novo_heatview::synthetic::{SynthDevice, SynthScene};
use novo_heatview::{Scalar, Vector};
use novo_theme::heat::{self, Verdict};
use novo_theme::icons;
use novo_theme::tokens as t;
use novo_thermal::Status;

use super::heat_compare::{self, Diff};
use super::heat_progress;
use super::heat_run::{Event, Runner, Solution};
use super::heat_settings::HeatSettings;
use novo_cfd::Progress;
use novo_cfd::sched::Outcome;

/// Opacity of the map over the drawing; the devices stay readable through it.
const MAP_ALPHA: f32 = 0.62;
/// Solver cell for the synthetic model, mm.
const CELL_MM: f64 = 10.0;
/// Gap between the two plates of the side by side compare, mm.
const SIDE_GAP_MM: f64 = 80.0;

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
    /// False when no loss figure exists for the device (drawn as "no data",
    /// not as a cool 0 W device).
    pub loss_known: bool,
    /// True when the loss figure is a planning estimate, not a maker value.
    pub loss_estimated: bool,
}

/// What the view draws from: the plate and its devices.
pub(super) struct Input {
    pub plate: (f64, f64),
    /// Outside air temperature, degrees C (the sizing request's).
    pub ambient: f64,
    pub items: Vec<Item>,
    /// The latest solver result, if a run has produced one.
    pub solution: Option<Arc<Solution>>,
    /// The result kept as A for comparison.
    pub kept: Option<Arc<Solution>>,
}

/// What a run needs to remember about the moment it started.
#[derive(Debug, Clone)]
pub(super) struct Meta {
    /// The scenario the run solves; a result is out of date once the
    /// cabinet's scenario differs from it.
    pub scenario_key: u64,
    pub layout_key: u64,
    pub ambient: f64,
    pub plate: [f64; 4],
    pub messages: Vec<String>,
}

pub(super) struct HeatState {
    pub on: bool,
    /// Show the difference map B minus A instead of the temperature.
    pub delta: bool,
    /// Wipe: A left of the split, B right of it.
    pub wipe: bool,
    /// Side by side: B on the plate, A on a copy of it to the right.
    pub side: bool,
    /// The view should fit both plates on the next frame.
    fit_wanted: bool,
    /// Where the wipe splits the plate, 0 to 1 of its width.
    pub split: f32,
    pub map: bool,
    pub contours: bool,
    pub airflow: bool,
    pub badges: bool,
    cache: Option<Cache>,
    /// Follows the cabinet while a result is shown: out of date marking and
    /// the automatic re-solve.
    pub(super) live: super::heat_live::Live,
    /// Counts the snapshots, so each one gets its own cache key.
    snap_seq: u64,
    #[cfg(test)]
    test_solver: Option<Arc<dyn novo_cfd::Solver>>,
    /// `heat.simulate` was asked for; the Cabinet view starts the run on its
    /// next frame, when it has the project.
    requested: bool,
    pub(super) run: Option<Runner>,
    /// Latest iteration report of the running level, with the residuals seen
    /// so far in this level for the sparkline.
    progress: Option<Progress>,
    history: Vec<f64>,
    pub(super) solution: Option<Arc<Solution>>,
    /// Result kept as A for comparison.
    kept: Option<Arc<Solution>>,
    pub(super) pending: Option<Meta>,
    runs: u64,
    /// The last run's outcome in words: failed, cancelled or the bridge's
    /// complaint.
    notice: Option<String>,
    /// Quality level, turbulence model and vents of the next run.
    pub settings: HeatSettings,
}

/// Where a cached picture came from.
enum Source {
    /// The synthetic model of the part losses.
    Synthetic,
    /// A solver result: per device reading, fidelity and the honesty text.
    Solved(Arc<Solution>),
}

struct Cache {
    key: u64,
    ambient: f64,
    /// Plate size, mm.
    size: (f64, f64),
    source: Source,
    smooth: Scalar,
    range: Range,
    contours: Vec<(Contour, bool)>,
    peaks: Vec<(Peak, String)>,
    /// B minus A when a result is kept and a different one is shown.
    diff: Option<Diff>,
    mean: f32,
    streams: Vec<Streamline>,
}

impl HeatState {
    pub(super) fn new() -> HeatState {
        HeatState {
            on: false,
            delta: false,
            wipe: false,
            side: false,
            fit_wanted: false,
            split: 0.5,
            kept: None,
            map: true,
            contours: true,
            airflow: false,
            badges: true,
            cache: None,
            requested: false,
            run: None,
            progress: None,
            history: Vec::new(),
            solution: None,
            pending: None,
            runs: 0,
            live: super::heat_live::Live::default(),
            snap_seq: 0,
            #[cfg(test)]
            test_solver: None,
            notice: None,
            settings: HeatSettings::default(),
        }
    }

    /// The difference map to draw, when Δ is on and there is one.
    fn delta_diff<'a>(&self, cache: &'a Cache) -> Option<&'a Diff> {
        if self.delta && !self.side {
            cache.diff.as_ref()
        } else {
            None
        }
    }

    /// The pair to show side by side, when Side is on and there is one.
    fn side_diff<'a>(&self, cache: &'a Cache) -> Option<&'a Diff> {
        if self.side { cache.diff.as_ref() } else { None }
    }

    /// True once after Side was switched on or off: fit the view to what
    /// is shown.
    pub(super) fn take_fit(&mut self) -> bool {
        std::mem::take(&mut self.fit_wanted)
    }

    /// Width of what the Heat view draws, in plate widths (2 with Side).
    pub(super) fn extent_mm(&self, plate_w: f64) -> f64 {
        if self.side {
            plate_w * 2.0 + SIDE_GAP_MM
        } else {
            plate_w
        }
    }

    /// The pair to wipe between, when Wipe is on and there is one.
    fn wipe_diff<'a>(&self, cache: &'a Cache) -> Option<&'a Diff> {
        if self.wipe && !self.delta && !self.side {
            cache.diff.as_ref()
        } else {
            None
        }
    }

    /// Keeps the shown result as A, to compare the next run against.
    pub(super) fn keep(&mut self) {
        self.kept = self.solution.clone();
        self.delta = false;
        self.wipe = false;
        self.set_side(false);
    }

    fn set_side(&mut self, on: bool) {
        if self.side != on {
            self.side = on;
            self.fit_wanted = true;
        }
    }

    pub(super) fn clear_kept(&mut self) {
        self.kept = None;
        self.delta = false;
        self.wipe = false;
        self.set_side(false);
    }

    /// The result the picture draws from.
    pub(super) fn solution(&self) -> Option<Arc<Solution>> {
        self.solution.clone()
    }

    pub(super) fn kept(&self) -> Option<Arc<Solution>> {
        self.kept.clone()
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    /// `heat.simulate`: show the Heat view and start a run on the next frame.
    pub(super) fn request_run(&mut self) {
        self.on = true;
        self.requested = true;
    }

    /// True once per request; the caller then calls [`HeatState::start`].
    pub(super) fn take_request(&mut self) -> bool {
        std::mem::take(&mut self.requested)
    }

    /// Starts a run unless one is going. The model can change while it
    /// runs; the result is then marked out of date.
    pub(super) fn start(&mut self, scenario: novo_thermal::Scenario, meta: Meta) {
        self.start_to(scenario, meta, self.settings.top);
    }

    /// [`HeatState::start`] with the highest level to go up to.
    pub(super) fn start_to(
        &mut self,
        scenario: novo_thermal::Scenario,
        meta: Meta,
        top: novo_cfd::Fidelity,
    ) {
        if self.run.is_some() {
            return;
        }
        self.notice = None;
        self.live.clear_superseded();
        self.runs += 1;
        self.pending = Some(meta);
        #[cfg(test)]
        let solver = self
            .test_solver
            .clone()
            .unwrap_or_else(|| self.settings.solver());
        #[cfg(not(test))]
        let solver = self.settings.solver();
        self.run = Some(Runner::start(solver, scenario, top));
        self.progress = None;
        self.history.clear();
    }

    #[cfg(test)]
    pub(super) fn set_solver_for_test(&mut self, s: Arc<dyn novo_cfd::Solver>) {
        self.test_solver = Some(s);
    }

    /// Why a run could not start, shown next to the Simulate button.
    pub(super) fn fail(&mut self, text: String) {
        self.notice = Some(text);
    }

    /// Stops the run and keeps the latest result, labelled not converged.
    pub(super) fn cancel(&mut self) {
        if let Some(r) = &self.run {
            r.cancel();
        }
    }

    /// Takes in what the worker has published. Called every frame.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
        let Some(run) = self.run.as_mut() else {
            return;
        };
        let mut done = false;
        for e in run.poll() {
            match e {
                Event::Snapshot(snapshot) => {
                    if let Some(m) = &self.pending {
                        self.snap_seq += 1;
                        self.solution = Some(Arc::new(Solution {
                            snapshot,
                            id: self.snap_seq,
                            scenario_key: m.scenario_key,
                            layout_key: m.layout_key,
                            ambient: m.ambient,
                            plate: m.plate,
                            messages: m.messages.clone(),
                        }));
                    }
                }
                Event::Progress(p) => {
                    if self
                        .progress
                        .as_ref()
                        .is_some_and(|q| q.fidelity != p.fidelity)
                    {
                        self.history.clear();
                    }
                    heat_progress::push(&mut self.history, &p);
                    self.progress = Some(p);
                }
                Event::Lost => {
                    done = true;
                    self.notice = Some("The simulation stopped unexpectedly".into());
                }
                Event::Finished(outcome) => {
                    done = true;
                    self.notice = match outcome {
                        Outcome::Completed => None,
                        // A run replaced by one for the edited cabinet is
                        // not worth a warning.
                        Outcome::Cancelled if self.live.take_superseded() => None,
                        Outcome::Cancelled => {
                            Some("Stopped: the last result is not converged".into())
                        }
                        Outcome::Failed(e) => Some(format!("The simulation failed: {e}")),
                    };
                }
            }
        }
        if done {
            self.run = None;
            self.pending = None;
            self.progress = None;
        } else {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    /// `heat.toggle`, `heat.airflow` and `heat.contours`: the layer commands
    /// switch the Heat view on when they show something.
    pub(super) fn command(&mut self, id: &str) {
        match id {
            "heat.toggle" => self.on = !self.on,
            "heat.simulate" => self.request_run(),
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

/// Hash of the layout and loads: what a result is valid for.
pub(super) fn layout_key(input: &Input) -> u64 {
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
            i.loss_known,
            i.loss_estimated,
        )
            .hash(&mut h);
    }
    h.finish()
}

/// Cache key: the layout and which result is drawn.
fn key(input: &Input) -> u64 {
    let mut h = DefaultHasher::new();
    (
        layout_key(input),
        input.solution.as_ref().map(|s| s.id),
        input.kept.as_ref().map(|s| s.id),
    )
        .hash(&mut h);
    h.finish()
}

fn build(input: &Input) -> Cache {
    if let Some(sol) = &input.solution
        && let Some(cache) = build_solved(input, sol)
    {
        return cache;
    }
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
    let velocity: Vector = scene.velocity_field(CELL_MM);
    assemble(
        input,
        smooth,
        velocity,
        input.ambient,
        &|x, y| scene.solid(x, y),
        Source::Synthetic,
    )
}

/// The picture of a solver result: the slice at the device front plane over
/// the plate. `None` when the snapshot cannot be sliced.
fn build_solved(input: &Input, sol: &Arc<Solution>) -> Option<Cache> {
    let [x, y, w, h] = sol.plate;
    let window = Window { u0: x, v0: y, w, h };
    let slice = cfd::as_2d(&sol.snapshot, window, CELL_MM)?;
    let smooth = slice.temperature.clone();
    let flow = slice.flow.clone();
    let diff = input
        .kept
        .as_ref()
        .filter(|a| a.id != sol.id)
        .and_then(|a| heat_compare::diff(a, sol, &smooth, CELL_MM));
    let mut cache = assemble(
        input,
        smooth,
        flow,
        sol.ambient,
        &|px, py| slice.is_solid(px + x, py + y),
        Source::Solved(sol.clone()),
    );
    cache.diff = diff;
    Some(cache)
}

/// Range, contours, hot spots and streamlines of a temperature and flow
/// field; the same for the synthetic model and a solver result.
fn assemble(
    input: &Input,
    smooth: Scalar,
    velocity: Vector,
    ambient: f64,
    solid: &dyn Fn(f64, f64) -> bool,
    source: Source,
) -> Cache {
    let (_, hottest) = smooth.extent().unwrap_or((35.0, 45.0));
    let range = range::map_range(ambient, f64::from(hottest), None);
    let levels = range::contour_levels(range, 8);
    let mut contours = Vec::new();
    for (level, lab) in levels {
        for c in contour::contours_at(&smooth, level as f32) {
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
    let mut seeds: Vec<(f64, f64)> = Vec::new();
    let (w, h) = input.plate;
    seeds.extend((0..6).map(|i| (2.0, h - 20.0 - i as f64 * 30.0)));
    seeds.extend((0..8).map(|i| (40.0 + i as f64 * w / 8.0, h - 2.0)));
    for d in &input.items {
        if d.loss_w > 20.0 {
            seeds.extend([0.2, 0.5, 0.8].map(|f| (d.x + d.w * f, d.y - 3.0)));
        }
    }
    let streams = streamline::evenly_spaced(&velocity, &seeds, Spacing::default(), solid);
    Cache {
        key: key(input),
        ambient,
        size: input.plate,
        source,
        smooth,
        mean,
        range,
        contours,
        peaks,
        diff: None,
        streams,
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

/// The Heat toggle of the Cabinet bar (View group).
pub(super) fn toggle(ui: &mut egui::Ui, state: &mut HeatState) -> egui::Response {
    state.poll(ui.ctx());
    let r = ui
        .add(egui::Button::new(format!("{}  Heat", icons::FLAME)).selected(state.on))
        .on_hover_text("Show how heat spreads over the plate (Shift+H)");
    if r.clicked() {
        state.on = !state.on;
    }
    r
}

/// The layer chips and simulate controls, in the strip under the bar while
/// Heat is on.
pub(super) fn strip(ui: &mut egui::Ui, state: &mut HeatState) {
    let chip = |ui: &mut egui::Ui, on: &mut bool, label: &str| {
        if ui.selectable_label(*on, label).clicked() {
            *on = !*on;
        }
    };
    chip(ui, &mut state.map, "Map");
    chip(ui, &mut state.contours, "Contours");
    chip(ui, &mut state.airflow, "Airflow");
    chip(ui, &mut state.badges, "Badges");
    simulate_controls(ui, state);
}

/// Keep as A, the Δ chip and Clear A.
fn compare_controls(ui: &mut egui::Ui, state: &mut HeatState) {
    let Some(shown) = state.solution.as_ref().map(|s| s.id) else {
        return;
    };
    match state.kept.as_ref().map(|a| a.id) {
        None => {
            if ui
                .button("Keep as A")
                .on_hover_text("Keep this result, change the cabinet, run again and compare")
                .clicked()
            {
                state.keep();
            }
        }
        Some(a) => {
            if a != shown
                && ui
                    .selectable_label(state.delta, "Δ B − A")
                    .on_hover_text("Show how the new result differs from the kept one")
                    .clicked()
            {
                state.delta = !state.delta;
                state.wipe = false;
                state.set_side(false);
            }
            if a != shown
                && ui
                    .selectable_label(state.wipe, "Wipe A | B")
                    .on_hover_text("Drag the split: A on the left, B on the right")
                    .clicked()
            {
                state.wipe = !state.wipe;
                state.delta = false;
                state.set_side(false);
            }
            if a != shown
                && ui
                    .selectable_label(state.side, "Side")
                    .on_hover_text("Two plates next to each other, same pan and zoom")
                    .clicked()
            {
                let on = !state.side;
                state.set_side(on);
                state.delta = false;
                state.wipe = false;
            }
            if ui
                .button("Clear A")
                .on_hover_text("Forget the kept result")
                .clicked()
            {
                state.clear_kept();
            }
        }
    }
}

/// Fidelity as the header and legend say it: a result that has not been
/// checked against benchmarks is a preview, never just "converged".
fn status_label(p: &novo_cfd::Progress, validated: bool) -> String {
    let state = if !validated {
        "not validated"
    } else if p.converged {
        "converged"
    } else {
        "not converged"
    };
    format!("{} · {state}", p.fidelity.label())
}

/// Total loss of the devices that have a figure, the number that have one,
/// and how many have none. The total is never negative zero (the sum of no
/// terms is -0.0 in Rust and would print as "-0 W").
fn totals(input: &Input) -> (f64, usize, usize) {
    let known = input.items.iter().filter(|i| i.loss_known);
    let (total, n) = known.fold((0.0_f64, 0), |(t, n), i| (t + i.loss_w, n + 1));
    (
        total + 0.0,
        n,
        input.items.iter().filter(|i| !i.loss_known).count(),
    )
}

/// Tells a first-time user that some loss figures are planning estimates, so
/// the temperatures follow from them and are not maker-verified. `None` when
/// every known loss is a maker or typed value.
fn estimate_note(input: &Input) -> Option<String> {
    let known = input.items.iter().filter(|i| i.loss_known).count();
    let estimated = input
        .items
        .iter()
        .filter(|i| i.loss_known && i.loss_estimated)
        .count();
    (estimated > 0).then(|| {
        format!(
            "{estimated} of {known} loss figures are planning estimates; \
             check them against maker data."
        )
    })
}

/// Simulate / Cancel, the fidelity badge and the last notice.
pub(super) fn simulate_controls(ui: &mut egui::Ui, state: &mut HeatState) {
    simulate_controls_with(ui, state, true);
}

/// [`simulate_controls`]; `chip` says whether to add the out of date line
/// (the 3D panel has its own sentence for it).
pub(super) fn simulate_controls_with(ui: &mut egui::Ui, state: &mut HeatState, chip: bool) {
    if state.running() {
        if ui
            .button("Cancel")
            .on_hover_text("Stop the simulation and keep the latest result")
            .clicked()
        {
            state.cancel();
        }
        let secs = state.run.as_ref().map_or(0.0, Runner::elapsed_s);
        let levels = state
            .run
            .as_ref()
            .map(|r| r.levels.as_slice())
            .unwrap_or(&[]);
        match &state.progress {
            Some(p) => {
                let frac = heat_progress::fraction(levels, p);
                ui.add(
                    egui::ProgressBar::new(frac)
                        .desired_width(150.0)
                        .text(egui::RichText::new(heat_progress::label(p, secs)).small()),
                );
                heat_progress::sparkline(ui, &state.history);
            }
            None => {
                ui.label(
                    egui::RichText::new(format!("starting · {secs:.0} s"))
                        .small()
                        .color(t::TEXT_MUTED),
                );
            }
        }
    } else {
        if ui
            .button("Simulate")
            .on_hover_text("Simulate the heat and airflow of this cabinet (Ctrl+K, heat)")
            .clicked()
        {
            state.request_run();
        }
        compare_controls(ui, state);
        if let Some(sol) = &state.solution {
            let p = &sol.snapshot.progress;
            let validated = sol.snapshot.accuracy.validated;
            ui.label(
                egui::RichText::new(status_label(p, validated))
                    .small()
                    .color(t::TEXT_MUTED),
            );
        }
    }
    super::heat_live::controls(ui, state, chip);
    if let Some(n) = &state.notice {
        ui.label(egui::RichText::new(n).small().color(t::WARNING));
    }
}

/// The devices of a layout as the Heat view reads them (shared by the
/// Cabinet and the 3D view, so a result is judged stale by one key).
pub(super) fn items_of(
    project: &novo_core::Project,
    layout: &novo_layout::state::CabinetLayout,
    tag_of: &dyn Fn(u64) -> String,
) -> Vec<Item> {
    use novo_core::{Device, DeviceId};
    layout
        .placements
        .iter()
        .map(|p| {
            let f = p.footprint();
            let loss = project
                .get::<Device>(DeviceId::from(p.id))
                .map_or(0.0, |d| {
                    let s = novo_layout::sizing_model::sizing_device(project, d);
                    s.power_loss * s.load_factor
                });
            Item {
                id: p.id,
                tag: tag_of(p.id),
                x: f.x,
                y: f.y,
                w: f.w,
                h: f.h,
                loss_w: loss,
                loss_known: project
                    .get::<Device>(DeviceId::from(p.id))
                    .is_some_and(|d| novo_thermal::bridge::device_loss_w(project, d).is_some()),
                loss_estimated: project
                    .get::<Device>(DeviceId::from(p.id))
                    .is_some_and(|d| novo_thermal::bridge::device_loss_estimated(project, d)),
            }
        })
        .collect()
}

/// Builds the scenario of the enclosure from the project and starts a run.
/// Says plainly when it cannot.
pub(super) fn start_run(
    state: &mut HeatState,
    project: &novo_core::Project,
    enclosure: novo_core::StructureId,
    input: &Input,
) {
    let options = novo_thermal::bridge::BridgeOptions {
        ambient_c: input.ambient,
        ..Default::default()
    };
    match novo_thermal::bridge::scenario_from_project(project, enclosure, &options) {
        Ok(mut bridge) => {
            bridge
                .scenario
                .vents
                .extend(state.settings.vents_for(&bridge.scenario.enclosure));
            let plate = bridge.scenario.enclosure.plate;
            let meta = Meta {
                scenario_key: super::heat_live::scenario_key(&bridge.scenario),
                layout_key: layout_key(input),
                ambient: input.ambient,
                plate: [plate.x, plate.y, plate.w, plate.h],
                messages: bridge.report.messages(),
            };
            state.start(bridge.scenario, meta);
        }
        Err(e) => state.fail(format!("Cannot simulate: {e}")),
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
        let to_screen = screen_fn(rect, viewport);
        let delta = state.delta_diff(cache);
        paint_map(&painter, cache, &to_screen, delta);
        if let Some(d) = state.side_diff(cache) {
            let off = cache.size.0 + SIDE_GAP_MM;
            let shifted = |x: f64, y: f64| to_screen(x + off, y);
            painter.add(plate_mesh(&d.a_plate, cache.size, &shifted, &|v| {
                rgb(heat::ramp(cache.range.t(v)), MAP_ALPHA)
            }));
        }
        if let Some(d) = state.wipe_diff(cache) {
            // A on the left of the split, drawn on the same ramp as B.
            let x = to_screen(f64::from(state.split) * cache.size.0, 0.0).x;
            let clip = painter.clip_rect();
            let left = Rect::from_min_max(
                clip.min,
                Pos2::new(x.clamp(clip.min.x, clip.max.x), clip.max.y),
            );
            let a = painter.with_clip_rect(left);
            a.add(plate_mesh(&d.a_plate, cache.size, &to_screen, &|v| {
                rgb(heat::ramp(cache.range.t(v)), MAP_ALPHA)
            }));
        }
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
    let delta = state.delta_diff(cache);
    if state.airflow {
        paint_airflow(&painter, cache, &to_screen);
    }
    if state.contours && delta.is_none() {
        paint_contours(&painter, cache, &to_screen);
    }
    paint_rims(&painter, cache, input, &to_screen);
    if state.badges {
        paint_badges(&painter, cache, input, &to_screen, viewport.zoom);
    }
    if delta.is_none() {
        paint_peaks(&painter, cache, &to_screen);
    }
    let stale = matches!(&cache.source, Source::Solved(sol) if state.live.out_of_date(sol, layout_key(input)));
    if let Some(d) = delta {
        let tag = |id: u64| {
            input
                .items
                .iter()
                .find(|i| i.id == id)
                .map_or_else(|| format!("#{id}"), |i| i.tag.clone())
        };
        heat_compare::legend(ui.ctx(), rect, d, &tag);
    } else {
        legend(ui.ctx(), rect, cache, input, stale);
    }
    if let Some(d) = state.side_diff(cache) {
        paint_side_a(&painter, cache, d, input, &to_screen);
    }
    if state.wipe_diff(cache).is_some() {
        let width = cache.size.0;
        let split = wipe_handle(ui, rect, &to_screen, viewport.zoom, width, state.split);
        state.split = split;
    }
}

/// The copy of the plate that shows A in the side by side compare: the
/// plate frame, the device rims in A's colours and the captions A and B.
fn paint_side_a(
    painter: &egui::Painter,
    cache: &Cache,
    d: &Diff,
    input: &Input,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
) {
    let (w, h) = cache.size;
    let off = w + SIDE_GAP_MM;
    let a = to_screen(off, 0.0);
    let b = to_screen(off + w, h);
    painter.rect_stroke(
        Rect::from_two_pos(a, b),
        0.0,
        Stroke::new(1.0, t::BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    for i in &input.items {
        let p0 = to_screen(i.x + off, i.y);
        let p1 = to_screen(i.x + i.w + off, i.y + i.h);
        let r = Rect::from_two_pos(p0, p1);
        let colour = match d.a_surface.iter().find(|(id, _)| *id == i.id) {
            Some((_, temp)) => rgb(heat::ramp(cache.range.t(*temp)), 1.0),
            None => t::TEXT_MUTED,
        };
        painter.rect_stroke(r, 2.0, Stroke::new(2.0, colour), egui::StrokeKind::Inside);
    }
    let font = FontId::proportional(t::FONT_SMALL);
    painter.text(
        a + Vec2::new(0.0, -6.0),
        Align2::LEFT_BOTTOM,
        "A (kept)",
        font.clone(),
        t::TEXT,
    );
    painter.text(
        to_screen(0.0, 0.0) + Vec2::new(0.0, -6.0),
        Align2::LEFT_BOTTOM,
        "B (new)",
        font,
        t::TEXT,
    );
}

/// The draggable split of the wipe: a line with the A and B labels. Returns
/// the new split, 0 to 1 of the plate width.
fn wipe_handle(
    ui: &egui::Ui,
    rect: Rect,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    zoom: f32,
    plate_w: f64,
    split: f32,
) -> f32 {
    let x = to_screen(f64::from(split) * plate_w, 0.0).x;
    let grab = Rect::from_min_max(
        Pos2::new(x - 6.0, rect.min.y),
        Pos2::new(x + 6.0, rect.max.y),
    );
    let response = ui
        .interact(grab, egui::Id::new("heat_wipe"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    let mut split = split;
    if response.dragged() && plate_w > 0.0 && zoom > 0.0 {
        split = (split + response.drag_delta().x / (zoom * plate_w as f32)).clamp(0.0, 1.0);
    }
    let x = to_screen(f64::from(split) * plate_w, 0.0).x;
    let painter = ui.painter_at(rect);
    painter.line_segment(
        [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
        Stroke::new(2.0, t::TEXT),
    );
    let font = FontId::proportional(t::FONT_SMALL);
    painter.text(
        Pos2::new(x - 8.0, rect.min.y + 12.0),
        Align2::RIGHT_CENTER,
        "A",
        font.clone(),
        t::TEXT,
    );
    painter.text(
        Pos2::new(x + 8.0, rect.min.y + 12.0),
        Align2::LEFT_CENTER,
        "B",
        font,
        t::TEXT,
    );
    split
}

fn paint_map(
    painter: &egui::Painter,
    cache: &Cache,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    delta: Option<&Diff>,
) {
    let field = delta.map_or(&cache.smooth, |d| &d.field);
    let shape = match delta {
        Some(d) => plate_mesh(field, cache.size, to_screen, &|v| {
            rgb(heat_compare::colour(v as f32, d.extent), MAP_ALPHA)
        }),
        None => plate_mesh(field, cache.size, to_screen, &|v| {
            rgb(heat::ramp(cache.range.t(v)), MAP_ALPHA)
        }),
    };
    painter.add(shape);
}

/// A mesh with a colour per vertex over the plate: the GPU blends between
/// them, so the field reads smooth whatever the cell size.
fn plate_mesh(
    field: &Scalar,
    size: (f64, f64),
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    colour: &dyn Fn(f64) -> Color32,
) -> Shape {
    let (w, h) = size;
    let (cols, rows) = (
        ((w / 5.0) as usize).clamp(2, 160),
        ((h / 5.0) as usize).clamp(2, 160),
    );
    let grid = field.resampled(cols, rows);
    let mut mesh = egui::Mesh::default();
    for r in 0..rows {
        for c in 0..cols {
            let (x, y) = grid.grid.centre(c, r);
            let color = colour(f64::from(grid.at(c, r)));
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
    Shape::mesh(mesh)
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
        let a = to_screen(i.x, i.y);
        let b = to_screen(i.x + i.w, i.y + i.h);
        if !i.loss_known {
            let r = Rect::from_two_pos(a, b);
            let corners = [
                r.left_top(),
                r.right_top(),
                r.right_bottom(),
                r.left_bottom(),
                r.left_top(),
            ];
            painter.extend(Shape::dashed_line(
                &corners,
                Stroke::new(1.5, t::TEXT_MUTED),
                6.0,
                4.0,
            ));
            continue;
        }
        let temp = device_temp(cache, i);
        let color = rgb(heat::ramp(cache.range.t(temp)), 1.0);
        painter.rect_stroke(
            Rect::from_two_pos(a, b),
            2.0,
            Stroke::new(2.0, color),
            egui::StrokeKind::Inside,
        );
    }
}

/// What a badge says about one device.
struct Reading {
    temp: f64,
    verdict: Verdict,
    /// Kelvin left to the tightest limit; negative = over.
    margin_k: Option<f64>,
}

/// The solver's reading of a device when there is one.
fn solved(cache: &Cache, id: u64) -> Option<&novo_thermal::DeviceResult> {
    match &cache.source {
        Source::Solved(sol) => sol.snapshot.devices.iter().find(|d| d.id == id),
        Source::Synthetic => None,
    }
}

/// The hottest air cell around the device, degrees C; a solver result gives
/// the housing surface temperature instead.
fn device_temp(cache: &Cache, i: &Item) -> f64 {
    if let Some(d) = solved(cache, i.id) {
        return d.surface_c;
    }
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

fn reading(cache: &Cache, i: &Item) -> Reading {
    let temp = device_temp(cache, i);
    match solved(cache, i.id) {
        Some(d) => Reading {
            temp,
            margin_k: d.margin_k,
            verdict: match d.status {
                Status::NoLimit => Verdict::Unknown,
                Status::Ok => Verdict::Ok,
                Status::Marginal => Verdict::Near,
                Status::Over => Verdict::Over,
            },
        },
        None => Reading {
            temp,
            margin_k: None,
            verdict: heat::verdict(cache.ambient, temp, None),
        },
    }
}

fn paint_badges(
    painter: &egui::Painter,
    cache: &Cache,
    input: &Input,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    zoom: f32,
) {
    let font = FontId::monospace(t::FONT_SMALL);
    let plate_left = to_screen(0.0, 0.0).x;
    let mut placed: Vec<Rect> = Vec::new();
    for i in input
        .items
        .iter()
        .filter(|i| i.loss_w > 0.0 || !i.loss_known)
    {
        if !i.loss_known {
            paint_no_data(painter, i, to_screen, zoom, &font);
            continue;
        }
        let Reading {
            temp,
            verdict,
            margin_k,
        } = reading(cache, i);
        // The badge names its device ("F1  46 °C") whenever the device is
        // wide enough; a badge without a name only where it is not.
        let reading_text = match margin_k {
            Some(m) => format!("{temp:.0} °C · {m:+.0} K"),
            None => format!("{temp:.0} °C"),
        };
        let measure = |text: &str| {
            painter
                .layout_no_wrap(text.to_owned(), font.clone(), t::TEXT)
                .size()
                .x
                + 30.0
        };
        let named = if i.tag.is_empty() {
            reading_text.clone()
        } else {
            format!("{}  {reading_text}", i.tag)
        };
        // The named badge may reach left over the free plate, but never over
        // another badge or past the plate edge; then it is the temperature
        // alone, and only where that fits inside the device.
        let br = to_screen(i.x + i.w, i.y + i.h);
        let rect_for = |width: f32| {
            Rect::from_min_size(br - Vec2::new(width + 6.0, 26.0), Vec2::new(width, 20.0))
        };
        let named_width = measure(&named);
        let named_rect = rect_for(named_width);
        let (text, width) = if (i.h as f32) * zoom >= 30.0
            && named_rect.left() >= plate_left
            && !placed.iter().any(|q| q.intersects(named_rect.expand(2.0)))
        {
            (named, named_width)
        } else {
            let w = measure(&reading_text);
            if (i.w as f32) * zoom < w + 8.0 || (i.h as f32) * zoom < 30.0 {
                continue;
            }
            (reading_text, w)
        };
        placed.push(rect_for(width));
        let dot = t::TEXT_MUTED;
        let dot = match verdict {
            Verdict::Ok => t::SUCCESS,
            Verdict::Near => t::WARNING,
            Verdict::Over => t::ERROR,
            Verdict::Unknown => dot,
        };
        let pill = rect_for(width);
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
            text,
            font.clone(),
            t::TEXT,
        );
    }
}

/// A device without loss data says so instead of showing a made-up temperature.
fn paint_no_data(
    painter: &egui::Painter,
    i: &Item,
    to_screen: &dyn Fn(f64, f64) -> Pos2,
    zoom: f32,
    font: &FontId,
) {
    if (i.w as f32) * zoom < 76.0 || (i.h as f32) * zoom < 30.0 {
        return;
    }
    let br = to_screen(i.x + i.w, i.y + i.h);
    let pill = Rect::from_min_size(br - Vec2::new(70.0, 26.0), Vec2::new(64.0, 20.0));
    painter.rect(
        pill,
        10.0,
        t::BG_2,
        Stroke::new(1.0, t::BORDER_STRONG),
        egui::StrokeKind::Inside,
    );
    painter.text(
        pill.center(),
        Align2::CENTER_CENTER,
        "no data",
        font.clone(),
        t::TEXT_MUTED,
    );
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

fn legend(ctx: &egui::Context, rect: Rect, cache: &Cache, input: &Input, stale: bool) {
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
                    egui::RichText::new(format!("°C · ambient {:.0}", cache.ambient))
                        .small()
                        .color(t::TEXT_MUTED),
                );
                ui.add_space(4.0);
                let small = |text: &str, color| egui::RichText::new(text).small().color(color);
                let (total_w, with_data, unknown) = totals(input);
                if with_data > 0 {
                    ui.label(small(&format!("Total loss {total_w:.0} W"), t::TEXT));
                }
                if let Some(note) = estimate_note(input) {
                    ui.label(small(&note, t::WARNING));
                }
                if unknown > 0 {
                    let (noun, verb) = if unknown == 1 {
                        ("device", "has")
                    } else {
                        ("devices", "have")
                    };
                    ui.label(small(
                        &format!("{unknown} {noun} {verb} no loss data (shown dashed)"),
                        t::WARNING,
                    ));
                }
                match &cache.source {
                    Source::Synthetic => {
                        ui.label(small(
                            "Preview model from the part losses, not a solver result. \
                             Press Simulate for a real run.",
                            t::TEXT_MUTED,
                        ));
                    }
                    Source::Solved(sol) => {
                        let p = &sol.snapshot.progress;
                        let validated = sol.snapshot.accuracy.validated;
                        ui.label(small(&status_label(p, validated), t::TEXT));
                        if stale {
                            ui.label(small(
                                "Out of date: the layout changed since this run.",
                                t::WARNING,
                            ));
                        }
                        if !p.converged {
                            ui.label(small("Not converged.", t::WARNING));
                        }
                        let a = &sol.snapshot.accuracy;
                        if !a.headline.is_empty() {
                            ui.label(small(&a.headline, t::TEXT_MUTED));
                        }
                        for n in sol.snapshot.notes.iter().take(2) {
                            ui.label(small(n, t::TEXT_MUTED));
                        }
                        for m in &sol.messages {
                            ui.label(small(m, t::WARNING));
                        }
                    }
                }
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
                    loss_known: true,
                    loss_estimated: false,
                },
                Item {
                    id: 2,
                    tag: "-K1".into(),
                    x: 350.0,
                    y: 100.0,
                    w: 45.0,
                    h: 90.0,
                    loss_w: 2.0,
                    loss_known: true,
                    loss_estimated: false,
                },
            ],
            solution: None,
            kept: None,
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
        assert_eq!(build(&b).ambient, 45.0);
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

    #[test]
    fn estimated_losses_get_a_note_and_maker_losses_do_not() {
        let mut i = input();
        assert_eq!(estimate_note(&i), None);
        i.items[0].loss_estimated = true;
        assert_eq!(
            estimate_note(&i).as_deref(),
            Some("1 of 2 loss figures are planning estimates; check them against maker data.")
        );
        // A device without a loss figure is not counted either way.
        i.items[0].loss_known = false;
        assert_eq!(estimate_note(&i), None);
    }

    /// A scenario matching `input()`: a 700 x 900 x 300 mm interior, the
    /// plate 600 x 800 at (50, 50).
    fn scenario() -> novo_thermal::Scenario {
        use novo_layout::Rect;
        use novo_layout::sizing::{Installation, Material};
        use novo_thermal::{Device as D, Enclosure, Params, Scenario};
        let dev = |id, x, y, w, h, loss_w| D {
            id,
            footprint: Rect { x, y, w, h },
            height_mm: Some(80.0),
            loss_w,
            rated_max_ambient_c: None,
            rated_max_surface_c: Some(70.0),
            heat_capacity_j_per_k: None,
        };
        Scenario {
            enclosure: Enclosure {
                width_mm: 704.0,
                height_mm: 904.0,
                depth_mm: 304.0,
                wall_mm: 2.0,
                material: Material::Steel,
                installation: Installation::Wall,
                plate: Rect {
                    x: 50.0,
                    y: 50.0,
                    w: 600.0,
                    h: 800.0,
                },
            },
            ambient_c: 35.0,
            devices: vec![
                dev(1, 150.0, 150.0, 150.0, 200.0, 80.0),
                dev(2, 400.0, 150.0, 45.0, 90.0, 2.0),
            ],
            obstacles: Vec::new(),
            vents: Vec::new(),
            fans: Vec::new(),
            params: Params::default(),
        }
    }

    fn meta(i: &Input) -> Meta {
        Meta {
            scenario_key: crate::app::heat_live::scenario_key(&scenario()),
            layout_key: layout_key(i),
            ambient: i.ambient,
            plate: [50.0, 50.0, 600.0, 800.0],
            messages: vec!["1 device has no loss data".into()],
        }
    }

    fn run_to_end(state: &mut HeatState) {
        let ctx = egui::Context::default();
        for _ in 0..600 {
            state.poll(&ctx);
            if !state.running() {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        panic!("the run did not finish");
    }

    #[test]
    fn a_run_replaces_the_synthetic_field_with_the_result() {
        let mut state = HeatState::new();
        let mut i = input();
        state.start(scenario(), meta(&i));
        assert!(state.running());
        run_to_end(&mut state);
        assert!(state.notice.is_none(), "{:?}", state.notice);
        i.solution = state.solution();
        let sol = i.solution.clone().expect("a result");
        assert!(sol.snapshot.is_consistent());
        let c = build(&i);
        assert!(matches!(c.source, Source::Solved(_)));
        assert!(!c.contours.is_empty());
        assert_eq!(c.size, (600.0, 800.0));
        // The badge reads the solver's surface temperature of the device.
        let want = sol.snapshot.devices.iter().find(|d| d.id == 1).unwrap();
        assert_eq!(device_temp(&c, &i.items[0]), want.surface_c);
        assert!(reading(&c, &i.items[0]).margin_k.is_some());
        // The drive is hotter than the relay in the solved picture too.
        assert!(device_temp(&c, &i.items[0]) > device_temp(&c, &i.items[1]));
    }

    #[test]
    fn a_result_is_out_of_date_when_the_layout_moves() {
        let mut i = input();
        let m = meta(&i);
        i.items[0].x += 20.0;
        assert_ne!(m.layout_key, layout_key(&i));
    }

    #[test]
    fn simulate_command_asks_for_a_run_once() {
        let mut s = HeatState::new();
        s.command("heat.simulate");
        assert!(s.on);
        assert!(s.take_request());
        assert!(!s.take_request());
    }

    #[test]
    fn a_second_start_is_ignored_while_running() {
        let mut state = HeatState::new();
        let i = input();
        state.start(scenario(), meta(&i));
        let runs = state.runs;
        state.start(scenario(), meta(&i));
        assert_eq!(state.runs, runs);
        run_to_end(&mut state);
    }

    #[test]
    fn devices_without_loss_data_are_counted_not_summed() {
        let mut i = input();
        i.items[1].loss_known = false;
        i.items[1].loss_w = 0.0;
        assert_eq!(totals(&i), (80.0, 1, 1));
        assert_eq!(totals(&input()), (82.0, 2, 0));
        // No figure anywhere: a positive zero, never "-0 W".
        for item in &mut i.items {
            item.loss_known = false;
            item.loss_w = 0.0;
        }
        let (total, with_data, _) = totals(&i);
        assert_eq!(with_data, 0);
        assert_eq!(format!("{total:.0}"), "0");
        // Knowing a loss changes the picture's cache key.
        assert_ne!(layout_key(&i), layout_key(&input()));
    }

    #[test]
    fn a_result_nobody_validated_is_never_just_converged() {
        let mut state = HeatState::new();
        let i = input();
        state.start(scenario(), meta(&i));
        run_to_end(&mut state);
        let sol = state.solution().unwrap();
        let p = &sol.snapshot.progress;
        assert!(status_label(p, false).ends_with("not validated"));
        assert!(status_label(p, true).ends_with("converged"));
    }

    #[test]
    fn a_cooler_second_run_shows_as_a_negative_difference() {
        let mut state = HeatState::new();
        let mut i = input();
        state.start(scenario(), meta(&i));
        run_to_end(&mut state);
        state.keep();
        assert!(state.kept().is_some());
        let mut cooler = scenario();
        cooler.devices[0].loss_w = 10.0;
        state.start(cooler, meta(&i));
        run_to_end(&mut state);
        i.solution = state.solution();
        i.kept = state.kept();
        let c = build(&i);
        let d = c.diff.as_ref().expect("a difference");
        let (lo, hi) = d.field.extent().unwrap();
        assert!(lo < -0.1 && hi < 0.5, "cooler everywhere: {lo} {hi}");
        assert_eq!(d.rows[0].id, 1, "the drive changed most");
        assert!(d.rows[0].delta() < 0.0);
        assert!(d.verdict.contains('→'), "{}", d.verdict);
        // Δ is only drawn when switched on.
        assert!(state.delta_diff(&c).is_none());
        state.delta = true;
        assert!(state.delta_diff(&c).is_some());
    }

    #[test]
    fn comparing_a_result_with_itself_has_no_difference() {
        let mut state = HeatState::new();
        let mut i = input();
        state.start(scenario(), meta(&i));
        run_to_end(&mut state);
        state.keep();
        i.solution = state.solution();
        i.kept = state.kept();
        assert!(build(&i).diff.is_none());
        state.clear_kept();
        assert!(state.kept().is_none() && !state.delta);
    }

    #[test]
    fn difference_colours_run_cool_to_hot() {
        let cold = heat_compare::colour(-5.0, 5.0);
        let hot = heat_compare::colour(5.0, 5.0);
        assert!(cold[2] > cold[0], "cooler is blue-ish {cold:?}");
        assert!(hot[0] > hot[2], "hotter is red-ish {hot:?}");
        assert_eq!(
            heat_compare::colour(f32::NAN, 5.0),
            heat_compare::colour(0.0, 5.0)
        );
    }

    #[test]
    fn wipe_and_delta_exclude_each_other() {
        let mut state = HeatState::new();
        let mut i = input();
        state.start(scenario(), meta(&i));
        run_to_end(&mut state);
        state.keep();
        let mut cooler = scenario();
        cooler.devices[0].loss_w = 10.0;
        state.start(cooler, meta(&i));
        run_to_end(&mut state);
        i.solution = state.solution();
        i.kept = state.kept();
        let c = build(&i);
        state.wipe = true;
        assert!(state.wipe_diff(&c).is_some());
        state.delta = true;
        assert!(state.wipe_diff(&c).is_none(), "Δ wins when both are set");
        // A is hotter than B at the drive: the two plates differ.
        let d = c.diff.as_ref().unwrap();
        assert!(d.a_plate.extent().unwrap().1 > c.smooth.extent().unwrap().1);
        state.clear_kept();
        assert!(!state.wipe && !state.delta);
    }

    #[test]
    fn side_excludes_the_other_modes_and_asks_for_a_fit() {
        let mut s = HeatState::new();
        assert!(!s.take_fit());
        s.set_side(true);
        assert!(s.side && s.take_fit() && !s.take_fit());
        assert_eq!(s.extent_mm(600.0), 1280.0);
        s.set_side(true);
        assert!(!s.take_fit(), "no change, no refit");
        s.clear_kept();
        assert!(!s.side && s.take_fit(), "leaving Side refits too");
        assert_eq!(s.extent_mm(600.0), 600.0);
    }
}
