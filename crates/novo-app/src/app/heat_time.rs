//! The heat-up over time: a transient run publishes frames, the time slider
//! picks the one the Heat view shows. Frames are kept as ready `Solution`s,
//! so showing one is a clone of an `Arc`; the Heat view needs to know
//! nothing about time.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::Instant;

use eframe::egui;
use novo_cfd::{
    CfdError, Control, Fidelity, StubSolver, TransientFrame, TransientSolver, TransientSpec,
    TransientSummary,
};
use novo_thermal::Scenario;

use super::heat::Meta;
use super::heat_run::Solution;

/// Frames a run publishes (61 with the first and last).
const FRAMES: f64 = 60.0;
/// A play-through of the whole timeline takes this long, s.
const PLAY_S: f64 = 10.0;

/// The transient solver the app uses. One place to change when the cabinet
/// solver gets a transient path; until then the 2D engine's, extruded.
pub(super) fn default_solver() -> Arc<dyn TransientSolver> {
    Arc::new(StubSolver)
}

/// A cold start at full load for `minutes`, with `FRAMES` + 1 frames.
pub(super) fn spec_for_minutes(minutes: f64) -> TransientSpec {
    let duration_s = minutes.clamp(1.0, 24.0 * 60.0) * 60.0;
    TransientSpec {
        duration_s,
        frame_interval_s: duration_s / FRAMES,
        ..TransientSpec::default()
    }
}

pub(super) enum Event {
    Frame(TransientFrame),
    Finished(Result<TransientSummary, CfdError>),
    /// The worker ended without reporting (it panicked).
    Lost,
}

/// A transient run in flight.
struct Run {
    control: Control,
    rx: Receiver<Event>,
    started: Instant,
}

impl Run {
    fn start(solver: Arc<dyn TransientSolver>, scenario: Scenario, spec: TransientSpec) -> Run {
        let control = Control::new();
        let (tx, rx) = channel();
        let worker = control.clone();
        std::thread::spawn(move || {
            let mut publish = |f: &TransientFrame| {
                let _ = tx.send(Event::Frame(f.clone()));
            };
            let result =
                solver.solve_transient(&scenario, Fidelity::Coarse, &spec, &worker, &mut publish);
            let _ = tx.send(Event::Finished(result));
        });
        Run {
            control,
            rx,
            started: Instant::now(),
        }
    }
}

/// One moment of the run.
pub(super) struct Frame {
    pub time_s: f64,
    pub load: f64,
    pub solution: Arc<Solution>,
}

/// The frames of the last heat-up and the slider over them.
pub(super) struct TimeLine {
    pub frames: Vec<Frame>,
    pub summary: Option<TransientSummary>,
    /// The frame shown.
    pub at: usize,
    pub playing: bool,
    /// Length of the next run, minutes.
    pub minutes: f32,
    run: Option<Run>,
    pending: Option<Meta>,
    expected: usize,
    /// The newest frame is shown as it arrives, until the user moves.
    follow: bool,
    /// The shown frame changed and the Heat view must take it.
    dirty: bool,
    last_step: f64,
}

impl TimeLine {
    pub(super) fn new() -> TimeLine {
        TimeLine {
            frames: Vec::new(),
            summary: None,
            at: 0,
            playing: false,
            minutes: 60.0,
            run: None,
            pending: None,
            expected: 0,
            follow: true,
            dirty: false,
            last_step: 0.0,
        }
    }

    pub(super) fn running(&self) -> bool {
        self.run.is_some()
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub(super) fn elapsed_s(&self) -> f64 {
        self.run
            .as_ref()
            .map_or(0.0, |r| r.started.elapsed().as_secs_f64())
    }

    /// Frames received of the frames expected, 0 to 1.
    pub(super) fn progress(&self) -> f32 {
        if self.expected == 0 {
            0.0
        } else {
            (self.frames.len() as f32 / self.expected as f32).min(1.0)
        }
    }

    pub(super) fn start(
        &mut self,
        solver: Arc<dyn TransientSolver>,
        scenario: Scenario,
        spec: TransientSpec,
        meta: Meta,
    ) {
        self.clear();
        self.expected = spec.frame_count();
        self.pending = Some(meta);
        self.run = Some(Run::start(solver, scenario, spec));
    }

    pub(super) fn cancel(&self) {
        if let Some(r) = &self.run {
            r.control.cancel();
        }
    }

    /// Forgets the frames (a steady run replaces them, or the user clears).
    pub(super) fn clear(&mut self) {
        self.cancel();
        self.run = None;
        self.pending = None;
        self.frames.clear();
        self.summary = None;
        self.at = 0;
        self.playing = false;
        self.follow = true;
        self.dirty = false;
    }

    /// What arrived since the last call. A worker that died without a word
    /// counts as a failed run.
    pub(super) fn poll(&mut self) -> Vec<Event> {
        let Some(run) = &self.run else {
            return Vec::new();
        };
        let mut out = Vec::new();
        loop {
            match run.rx.try_recv() {
                Ok(e) => out.push(e),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !matches!(out.last(), Some(Event::Finished(_))) {
                        out.push(Event::Lost);
                    }
                    break;
                }
            }
        }
        out
    }

    /// What a frame needs to become a `Solution`: the moment the run started.
    pub(super) fn meta(&self) -> Option<&Meta> {
        self.pending.as_ref()
    }

    pub(super) fn push(&mut self, time_s: f64, load: f64, solution: Arc<Solution>) {
        self.frames.push(Frame {
            time_s,
            load,
            solution,
        });
        if self.follow {
            self.at = self.frames.len() - 1;
            self.dirty = true;
        }
    }

    pub(super) fn finish(&mut self, summary: Option<TransientSummary>) {
        self.run = None;
        self.pending = None;
        self.summary = summary;
    }

    /// The user moved the slider.
    pub(super) fn seek(&mut self, at: usize) {
        let at = at.min(self.frames.len().saturating_sub(1));
        if at != self.at {
            self.at = at;
            self.dirty = true;
        }
        self.follow = false;
        self.playing = false;
    }

    pub(super) fn play(&mut self, now: f64) {
        if self.frames.len() < 2 {
            return;
        }
        if self.at + 1 >= self.frames.len() {
            self.at = 0;
            self.dirty = true;
        }
        self.follow = false;
        self.playing = true;
        self.last_step = now;
    }

    /// Steps the playback; a play-through takes `PLAY_S` seconds.
    pub(super) fn tick(&mut self, now: f64) {
        if !self.playing || self.frames.len() < 2 {
            return;
        }
        let per_frame = PLAY_S / self.frames.len() as f64;
        let steps = ((now - self.last_step) / per_frame).floor() as usize;
        if steps == 0 {
            return;
        }
        self.last_step += steps as f64 * per_frame;
        let last = self.frames.len() - 1;
        let to = (self.at + steps).min(last);
        if to != self.at {
            self.at = to;
            self.dirty = true;
        }
        if to == last {
            self.playing = false;
        }
    }

    /// The solution to show, once per change.
    pub(super) fn take_selection(&mut self) -> Option<Arc<Solution>> {
        if !std::mem::take(&mut self.dirty) {
            return None;
        }
        self.frames.get(self.at).map(|f| f.solution.clone())
    }

    /// "23 min", "1 h 05 min" or "45 s".
    pub(super) fn time_label(time_s: f64) -> String {
        let s = time_s.round() as u64;
        match s {
            0..=89 => format!("{s} s"),
            90..=3599 => format!("{} min", (time_s / 60.0).round() as u64),
            _ => format!("{} h {:02} min", s / 3600, (s % 3600) / 60),
        }
    }
}

/// The curves of the summary as points in a `w` by `h` box, mean air and
/// hottest air on one scale, plus the x of the shown time.
pub(super) struct Plot {
    pub mean: Vec<(f32, f32)>,
    pub max: Vec<(f32, f32)>,
    pub cursor_x: f32,
}

pub(super) fn plot(s: &TransientSummary, shown_s: f64, w: f32, h: f32) -> Option<Plot> {
    let n = s.times_s.len();
    if n < 2 || s.mean_air_c.len() != n || s.max_air_c.len() != n {
        return None;
    }
    let (t0, t1) = (s.times_s[0], s.times_s[n - 1]);
    let all = s.mean_air_c.iter().chain(&s.max_air_c).copied();
    let lo = all.clone().fold(f64::MAX, f64::min);
    let hi = all.fold(f64::MIN, f64::max);
    if t1.is_nan() || t0.is_nan() || t1 <= t0 || !lo.is_finite() || !hi.is_finite() {
        return None;
    }
    let span = (hi - lo).max(0.5);
    let x = |t: f64| (((t - t0) / (t1 - t0)) as f32).clamp(0.0, 1.0) * w;
    let curve = |v: &[f64]| -> Vec<(f32, f32)> {
        s.times_s
            .iter()
            .zip(v)
            .map(|(t, v)| (x(*t), h * (1.0 - ((v - lo) / span) as f32)))
            .collect()
    };
    Some(Plot {
        mean: curve(&s.mean_air_c),
        max: curve(&s.max_air_c),
        cursor_x: x(shown_s),
    })
}

/// Play, the slider, the shown time and the curve of the air temperature.
pub(super) fn strip(ui: &mut egui::Ui, tl: &mut TimeLine) {
    use novo_theme::tokens as t;
    if tl.frames.is_empty() {
        return;
    }
    let last = tl.frames.len() - 1;
    let now = ui.input(|i| i.time);
    let label = if tl.playing { "Pause" } else { "Play" };
    if ui
        .button(label)
        .on_hover_text("Play the heat-up from the start")
        .clicked()
    {
        if tl.playing {
            tl.playing = false;
        } else {
            tl.play(now);
        }
    }
    let mut at = tl.at;
    let total = tl.frames[last].time_s;
    let slider = ui.add(
        egui::Slider::new(&mut at, 0..=last)
            .show_value(false)
            .text(""),
    );
    if slider.changed() {
        tl.seek(at);
    }
    slider.on_hover_text("Drag to see the cabinet at another time of the heat-up");
    let frame = &tl.frames[tl.at];
    ui.label(
        egui::RichText::new(format!(
            "{} of {} · load {:.0} %",
            TimeLine::time_label(frame.time_s),
            TimeLine::time_label(total),
            frame.load * 100.0
        ))
        .small()
        .color(t::TEXT_MUTED),
    );
    if let Some(s) = &tl.summary {
        let size = egui::vec2(110.0, 18.0);
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        if let Some(p) = plot(s, frame.time_s, size.x, size.y) {
            let pts = |v: &[(f32, f32)]| -> Vec<egui::Pos2> {
                v.iter()
                    .map(|(x, y)| rect.min + egui::vec2(*x, *y))
                    .collect()
            };
            let painter = ui.painter();
            painter.add(egui::Shape::line(
                pts(&p.max),
                egui::Stroke::new(1.2, t::WARNING),
            ));
            painter.add(egui::Shape::line(
                pts(&p.mean),
                egui::Stroke::new(1.2, t::TEXT_MUTED),
            ));
            let x = rect.min.x + p.cursor_x;
            painter.line_segment(
                [egui::pos2(x, rect.min.y), egui::pos2(x, rect.max.y)],
                egui::Stroke::new(1.0, t::TEXT),
            );
        }
        let tc = s
            .time_constant_s
            .map(|c| format!(" Time constant {}.", TimeLine::time_label(c)))
            .unwrap_or_default();
        resp.on_hover_text(format!(
            "Hottest air (orange) and mean air (grey) over the heat-up.{tc}"
        ));
    }
}
