//! Timing diagrams: digital channels (a contactor, a breaker, a lamp) and
//! analog traces (a current) on one time axis, the way a logic analyzer and a
//! scope would show a run.
//!
//! Two sources feed it: the scenario runner ([`crate::scenario`]) for faults
//! and protection, and [`record_logic`] for the control logic of the drawing
//! itself (coils, timers, lamps, motors stepped through a script of button
//! presses). The diagram is data; [`TimingDiagram::to_svg`] draws it for
//! export and [`TimingDiagram::to_vcd`] writes the digital part for a waveform
//! viewer. The app's window draws the same data with egui.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;

use novo_core::{Project, SymbolId};

use crate::trace::Trace;
use crate::{Clock, MotorRun, PoleState, device_states, is_coil, settle_timed};

/// State of a digital channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// Off, open, released.
    Low,
    /// On, closed, energized.
    High,
    /// Something to look at: a fault, a device that tripped but still
    /// carries current, a motor that hums on two phases.
    Alarm,
}

/// One row of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct DigitalChannel {
    pub name: String,
    /// Rows with the same group are drawn together under one heading.
    pub group: String,
    /// `(time, level from then on)`, in time order, no two equal levels in a
    /// row.
    pub edges: Vec<(f64, Level)>,
}

impl DigitalChannel {
    pub fn new(name: impl Into<String>, group: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            group: group.into(),
            edges: Vec::new(),
        }
    }

    /// The channel has `level` from `t` on. A repeat of the current level is
    /// ignored; an earlier time than the last edge is placed in order.
    pub fn set(&mut self, t: f64, level: Level) {
        let at = self.edges.partition_point(|e| e.0 <= t);
        if at > 0 && self.edges[at - 1].1 == level {
            return;
        }
        if at < self.edges.len() && self.edges[at].1 == level && self.edges[at].0 == t {
            return;
        }
        self.edges.insert(at, (t, level));
    }

    pub fn level_at(&self, t: f64) -> Level {
        let at = self.edges.partition_point(|e| e.0 <= t);
        if at == 0 {
            Level::Low
        } else {
            self.edges[at - 1].1
        }
    }

    /// Constant stretches `(from, to, level)` covering `[0, end]`.
    pub fn segments(&self, end: f64) -> Vec<(f64, f64, Level)> {
        let mut out = Vec::new();
        let mut from = 0.0;
        let mut level = Level::Low;
        for &(t, l) in &self.edges {
            if t > from {
                out.push((from, t.min(end), level));
                from = t;
            }
            level = l;
            if from >= end {
                break;
            }
        }
        if from < end {
            out.push((from, end, level));
        }
        out
    }

    /// Times when the channel goes to `level`.
    pub fn times_to(&self, level: Level) -> Vec<f64> {
        self.edges
            .iter()
            .filter(|e| e.1 == level)
            .map(|e| e.0)
            .collect()
    }
}

/// Digital channels and analog traces on one axis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimingDiagram {
    pub duration: f64,
    pub channels: Vec<DigitalChannel>,
    pub analog: Vec<Trace>,
    /// Things the user should know (the recording was cut short).
    pub warnings: Vec<String>,
}

impl TimingDiagram {
    pub fn new(duration: f64) -> Self {
        Self {
            duration,
            ..Self::default()
        }
    }

    pub fn channel(&self, name: &str) -> Option<&DigitalChannel> {
        self.channels.iter().find(|c| c.name == name)
    }

    /// Value Change Dump of the digital channels, 1 µs resolution; a level is
    /// a 2 bit vector (0 low, 1 high, 2 alarm).
    pub fn to_vcd(&self) -> String {
        let mut out = String::from("$timescale 1 us $end\n$scope module kopercad $end\n");
        let id = |i: usize| char::from(b'!' + (i % 90) as u8).to_string() + &"!".repeat(i / 90);
        for (i, c) in self.channels.iter().enumerate() {
            let name: String = c
                .name
                .chars()
                .map(|ch| if ch.is_whitespace() { '_' } else { ch })
                .collect();
            let _ = writeln!(out, "$var wire 2 {} {} $end", id(i), name);
        }
        out.push_str("$upscope $end\n$enddefinitions $end\n");
        let mut times: BTreeSet<i64> = BTreeSet::new();
        times.insert(0);
        for c in &self.channels {
            times.extend(c.edges.iter().map(|e| (e.0 * 1e6).round() as i64));
        }
        for t in times {
            let _ = writeln!(out, "#{t}");
            for (i, c) in self.channels.iter().enumerate() {
                let here = c.level_at(t as f64 / 1e6 + 1e-9);
                let before = c.level_at(t as f64 / 1e6 - 1e-9);
                if t == 0 || here != before {
                    let v = match here {
                        Level::Low => "b00",
                        Level::High => "b01",
                        Level::Alarm => "b10",
                    };
                    let _ = writeln!(out, "{v} {}", id(i));
                }
            }
        }
        out
    }

    /// A drawing of the window `[t0, t1]` (the whole run when `None`):
    /// digital rows first, grouped, then one row per analog trace scaled to
    /// its own range. `width` is the SVG width in pixels.
    pub fn to_svg(&self, width: f64, window: Option<(f64, f64)>) -> String {
        let (t0, t1) = window.unwrap_or((0.0, self.duration));
        let t1 = if t1 > t0 { t1 } else { t0 + 1.0 };
        let (label_w, row_h, gap, head_h, axis_h) = (150.0, 22.0, 8.0, 18.0, 28.0);
        let plot_w = (width - label_w - 12.0).max(50.0);
        let x = |t: f64| label_w + (t - t0) / (t1 - t0) * plot_w;
        let mut groups: Vec<(&str, Vec<&DigitalChannel>)> = Vec::new();
        for c in &self.channels {
            match groups.iter_mut().find(|g| g.0 == c.group) {
                Some(g) => g.1.push(c),
                None => groups.push((&c.group, vec![c])),
            }
        }
        let analog_h = 56.0;
        let digital_rows: usize = groups.iter().map(|g| g.1.len()).sum();
        let height = axis_h
            + groups.len() as f64 * head_h
            + digital_rows as f64 * (row_h + gap)
            + self.analog.len() as f64 * (analog_h + gap)
            + 8.0;
        let mut s = String::new();
        let _ = write!(
            s,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" \
             font-family=\"sans-serif\" font-size=\"11\"><rect width=\"100%\" height=\"100%\" fill=\"#14171c\"/>"
        );
        // Time axis with ticks at a round step.
        let step = nice_step((t1 - t0) / 8.0);
        let mut tick = (t0 / step).ceil() * step;
        while tick <= t1 + step * 1e-9 {
            let px = x(tick);
            let _ = write!(
                s,
                "<line x1=\"{px:.1}\" y1=\"{}\" x2=\"{px:.1}\" y2=\"{height}\" stroke=\"#2a303a\"/>\
                 <text x=\"{px:.1}\" y=\"14\" fill=\"#9aa4b2\" text-anchor=\"middle\">{}</text>",
                axis_h - 6.0,
                format_time(tick)
            );
            tick += step;
        }
        let mut y = axis_h;
        for (group, rows) in &groups {
            let _ = write!(
                s,
                "<text x=\"6\" y=\"{:.1}\" fill=\"#6b7685\" font-weight=\"bold\">{}</text>",
                y + 12.0,
                xml(group)
            );
            y += head_h;
            for c in rows {
                let _ = write!(
                    s,
                    "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#d6dbe3\" text-anchor=\"end\">{}</text>",
                    label_w - 8.0,
                    y + row_h * 0.7,
                    xml(&c.name)
                );
                let (hi, lo) = (y + 3.0, y + row_h - 3.0);
                let mut d = String::new();
                let mut prev: Option<f64> = None;
                for (a, b, level) in c.segments(self.duration) {
                    if b <= t0 || a >= t1 {
                        continue;
                    }
                    let (a, b) = (a.max(t0), b.min(t1));
                    let level_y = if level == Level::Low { lo } else { hi };
                    let _ = match prev {
                        None => write!(d, "M{:.1},{level_y:.1}", x(a)),
                        Some(py) if py != level_y => {
                            write!(d, "L{:.1},{py:.1}L{:.1},{level_y:.1}", x(a), x(a))
                        }
                        _ => Ok(()),
                    };
                    let _ = write!(d, "L{:.1},{level_y:.1}", x(b));
                    prev = Some(level_y);
                    if level == Level::Alarm {
                        let _ = write!(
                            s,
                            "<rect x=\"{:.1}\" y=\"{hi:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"#e5484d\" opacity=\"0.25\"/>",
                            x(a),
                            (x(b) - x(a)).max(1.0),
                            lo - hi
                        );
                    }
                }
                let color = if c.edges.iter().any(|e| e.1 == Level::Alarm) {
                    "#ff6369"
                } else {
                    "#4cc38a"
                };
                let _ = write!(
                    s,
                    "<path d=\"{d}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"1.6\"/>"
                );
                y += row_h + gap;
            }
        }
        for tr in &self.analog {
            let sub = decimate_window(tr, t0, t1, (plot_w as usize).max(16) * 2);
            let (lo_v, hi_v) = (sub.min().unwrap_or(0.0), sub.max().unwrap_or(1.0));
            let (lo_v, hi_v) = if hi_v - lo_v < 1e-12 {
                (lo_v - 1.0, hi_v + 1.0)
            } else {
                (lo_v, hi_v)
            };
            let _ = write!(
                s,
                "<text x=\"{:.1}\" y=\"{:.1}\" fill=\"#d6dbe3\" text-anchor=\"end\">{}</text>\
                 <text x=\"{:.1}\" y=\"{:.1}\" fill=\"#6b7685\" text-anchor=\"end\">{:.4} {}</text>",
                label_w - 8.0,
                y + 14.0,
                xml(&tr.name),
                label_w - 8.0,
                y + 28.0,
                hi_v,
                xml(&tr.unit)
            );
            let mut d = String::new();
            for (i, (t, v)) in sub.t.iter().zip(&sub.v).enumerate() {
                let py = y + analog_h - (v - lo_v) / (hi_v - lo_v) * analog_h;
                let _ = write!(d, "{}{:.1},{py:.1}", if i == 0 { 'M' } else { 'L' }, x(*t));
            }
            let _ = write!(
                s,
                "<rect x=\"{label_w}\" y=\"{y:.1}\" width=\"{plot_w:.1}\" height=\"{analog_h}\" fill=\"none\" stroke=\"#2a303a\"/>\
                 <path d=\"{d}\" fill=\"none\" stroke=\"#6cb6ff\" stroke-width=\"1.2\"/>"
            );
            y += analog_h + gap;
        }
        s.push_str("</svg>\n");
        s
    }
}

fn decimate_window(tr: &Trace, t0: f64, t1: f64, max: usize) -> Trace {
    let mut w = Trace::new(tr.name.clone(), tr.unit.clone());
    if let Some(v) = tr.value_at(t0) {
        w.push(t0, v);
    }
    for (&t, &v) in tr.t.iter().zip(&tr.v) {
        if t > t0 && t < t1 {
            w.push(t, v);
        }
    }
    if let Some(v) = tr.value_at(t1) {
        w.push(t1, v);
    }
    w.decimate(max)
}

fn nice_step(raw: f64) -> f64 {
    let raw = raw.max(1e-9);
    let mag = 10f64.powf(raw.log10().floor());
    let f = raw / mag;
    mag * if f <= 1.0 {
        1.0
    } else if f <= 2.0 {
        2.0
    } else if f <= 5.0 {
        5.0
    } else {
        10.0
    }
}

/// `1.5 s`, `20 ms`, `300 µs`.
pub fn format_time(t: f64) -> String {
    let a = t.abs();
    if a == 0.0 {
        "0".into()
    } else if a >= 1.0 {
        format!("{} s", trim(t))
    } else if a >= 1e-3 {
        format!("{} ms", trim(t * 1e3))
    } else {
        format!("{} µs", trim(t * 1e6))
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// A hand-set contact (a push button, a selector) changing at a time.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Stimulus {
    pub t: f64,
    pub symbol: SymbolId,
    pub closed: bool,
}

/// Steps the control logic of the drawing through `stimuli` for `duration`
/// seconds and records coils, hand-set inputs, lamps and motors as channels.
/// Time advances to the next stimulus, the next timer expiry, or `max_step`,
/// whichever comes first, so a 5 s on-delay switches at exactly 5 s.
pub fn record_logic(
    project: &Project,
    stimuli: &[Stimulus],
    duration: f64,
    max_step: f64,
) -> TimingDiagram {
    let netlist = project.netlist();
    let mut script: Vec<Stimulus> = stimuli.to_vec();
    script.sort_by(|a, b| a.t.total_cmp(&b.t));
    let name_of = |symbol: SymbolId| -> String {
        project
            .symbol(symbol)
            .and_then(|s| s.device())
            .and_then(|d| project.device(d))
            .map_or_else(|| format!("symbol {}", symbol.0), |d| d.tag.to_string())
    };
    let mut diagram = TimingDiagram::new(duration);
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut put = |diagram: &mut TimingDiagram, key: String, name: String, group: &str, t, l| {
        let i = *index.entry(key).or_insert_with(|| {
            diagram.channels.push(DigitalChannel::new(name, group));
            diagram.channels.len() - 1
        });
        diagram.channels[i].set(t, l);
    };

    let mut hand = PoleState::all_open();
    let mut clock = Clock::new();
    let mut held: BTreeSet<SymbolId> = BTreeSet::new();
    let mut next = 0;
    let mut t = 0.0_f64;
    let mut finished = false;
    for _ in 0..200_000 {
        while let Some(s) = script.get(next) {
            if s.t > t + 1e-12 {
                break;
            }
            hand.set_all(project, s.symbol, s.closed);
            put(
                &mut diagram,
                format!("in{}", s.symbol.0),
                name_of(s.symbol),
                "Inputs",
                t,
                if s.closed { Level::High } else { Level::Low },
            );
            next += 1;
        }
        let settled = settle_timed(
            project,
            &netlist,
            &hand,
            &BTreeSet::new(),
            &held,
            &mut clock,
        );
        held = settled.energized.clone();
        for symbol in project.symbols().filter(|s| is_coil(s)) {
            let on = settled.energized.contains(&symbol.id);
            put(
                &mut diagram,
                format!("coil{}", symbol.id.0),
                name_of(symbol.id),
                "Coils",
                t,
                if on { Level::High } else { Level::Low },
            );
        }
        let states = device_states(project, &netlist, &settled);
        for (&id, &on) in &states.lamps {
            put(
                &mut diagram,
                format!("lamp{}", id.0),
                name_of(id),
                "Lamps",
                t,
                if on { Level::High } else { Level::Low },
            );
        }
        for (&id, &run) in &states.motors {
            let level = match run {
                MotorRun::Stopped => Level::Low,
                MotorRun::Running { .. } => Level::High,
                MotorRun::PhaseLoss | MotorRun::Fault => Level::Alarm,
            };
            put(
                &mut diagram,
                format!("motor{}", id.0),
                name_of(id),
                "Motors",
                t,
                level,
            );
        }
        if t >= duration - 1e-12 {
            finished = true;
            break;
        }
        let mut step = max_step.min(duration - t);
        if let Some(s) = script.get(next) {
            step = step.min((s.t - t).max(0.0));
        }
        if settled.timing && settled.remaining > 0.0 {
            step = step.min(settled.remaining);
        }
        // A zero step would repeat the same instant: nudge to the next
        // stimulus or expiry boundary.
        let step = if step <= 0.0 { 1e-9 } else { step };
        clock.advance(step);
        t += step;
    }
    if !finished {
        diagram.warnings.push(format!(
            "The recording stopped at {} before the end: too many steps (a very long run with short timers?)",
            format_time(t)
        ));
    }
    diagram
}
