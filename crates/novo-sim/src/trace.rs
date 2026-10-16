//! Time series: the common shape of everything the simulation plots.
//!
//! A [`Trace`] is one signal over time (a current in A, a voltage in V, a
//! temperature in K); a [`TraceSet`] is the signals of one run on a shared
//! time axis. Samples are kept in time order and may be uneven: a solver that
//! takes small steps around a switching event and large ones elsewhere just
//! pushes what it computed. Plot code never draws every sample, it asks for
//! [`Trace::decimate`], which keeps each bucket's minimum and maximum so a
//! one millisecond spike is still there when a ten second run is shown in a
//! thousand pixels.

use serde::{Deserialize, Serialize};

/// One signal over time.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Trace {
    pub name: String,
    /// Unit symbol for axis labels: `"A"`, `"V"`, `"K"`.
    pub unit: String,
    /// Seconds, strictly increasing.
    pub t: Vec<f64>,
    pub v: Vec<f64>,
}

impl Trace {
    pub fn new(name: impl Into<String>, unit: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            unit: unit.into(),
            t: Vec::new(),
            v: Vec::new(),
        }
    }

    /// Samples `f` every `dt` seconds from 0 to `duration` inclusive.
    pub fn sampled(
        name: impl Into<String>,
        unit: impl Into<String>,
        duration: f64,
        dt: f64,
        f: impl Fn(f64) -> f64,
    ) -> Self {
        let mut trace = Self::new(name, unit);
        if dt <= 0.0 || duration < 0.0 {
            return trace;
        }
        let n = (duration / dt).round() as usize;
        for k in 0..=n {
            let t = (k as f64 * dt).min(duration);
            trace.push(t, f(t));
        }
        trace
    }

    /// Appends a sample. A sample at or before the last time replaces the
    /// last value when the times are equal (the later value of a step) and is
    /// ignored when it would go back in time.
    pub fn push(&mut self, t: f64, v: f64) {
        match self.t.last() {
            Some(&last) if t < last => {}
            Some(&last) if t == last => *self.v.last_mut().unwrap() = v,
            _ => {
                self.t.push(t);
                self.v.push(v);
            }
        }
    }

    /// A sample-and-hold step: the value jumps to `v` at `t`, so the line
    /// stays at the old value until then.
    pub fn push_step(&mut self, t: f64, v: f64) {
        if let Some(&old) = self.v.last()
            && old != v
            && self.t.last().is_some_and(|&l| l < t)
        {
            self.push(t, old);
        }
        self.push(t, v);
    }

    pub fn len(&self) -> usize {
        self.t.len()
    }

    pub fn is_empty(&self) -> bool {
        self.t.is_empty()
    }

    /// First and last time, `None` when empty.
    pub fn span(&self) -> Option<(f64, f64)> {
        Some((*self.t.first()?, *self.t.last()?))
    }

    /// Linear interpolation; clamped to the end values outside the span.
    pub fn value_at(&self, t: f64) -> Option<f64> {
        let (&first, &last) = (self.t.first()?, self.t.last()?);
        if t <= first {
            return self.v.first().copied();
        }
        if t >= last {
            return self.v.last().copied();
        }
        let hi = self.t.partition_point(|&x| x <= t);
        let (t0, t1) = (self.t[hi - 1], self.t[hi]);
        let f = (t - t0) / (t1 - t0);
        Some(self.v[hi - 1] + f * (self.v[hi] - self.v[hi - 1]))
    }

    pub fn max(&self) -> Option<f64> {
        self.v.iter().copied().reduce(f64::max)
    }

    pub fn min(&self) -> Option<f64> {
        self.v.iter().copied().reduce(f64::min)
    }

    /// Largest absolute value (the peak of an AC signal).
    pub fn peak(&self) -> Option<f64> {
        self.v.iter().copied().map(f64::abs).reduce(f64::max)
    }

    /// Time-weighted mean over `[t0, t1]` (trapezoids on the interpolated
    /// signal), `None` for an empty window or trace.
    pub fn mean_between(&self, t0: f64, t1: f64) -> Option<f64> {
        self.integral(t0, t1, |v| v).map(|i| i / (t1 - t0))
    }

    /// Root mean square over `[t0, t1]`.
    pub fn rms_between(&self, t0: f64, t1: f64) -> Option<f64> {
        self.integral(t0, t1, |v| v * v)
            .map(|i| (i / (t1 - t0)).sqrt())
    }

    /// Integral of `v(t)` over the window, e.g. the let-through energy
    /// `∫i²dt` in A²s when the trace is a current.
    pub fn i2t_between(&self, t0: f64, t1: f64) -> Option<f64> {
        self.integral(t0, t1, |v| v * v)
    }

    fn integral(&self, t0: f64, t1: f64, f: impl Fn(f64) -> f64) -> Option<f64> {
        if self.t.len() < 2 || t1 <= t0 || t1 < self.t[0] || t0 > *self.t.last()? {
            return None;
        }
        let mut points = vec![(t0, self.value_at(t0)?)];
        points.extend(
            self.t
                .iter()
                .zip(&self.v)
                .filter(|&(&t, _)| t > t0 && t < t1)
                .map(|(&t, &v)| (t, v)),
        );
        points.push((t1, self.value_at(t1)?));
        let mut sum = 0.0;
        for pair in points.windows(2) {
            let ((ta, va), (tb, vb)) = (pair[0], pair[1]);
            // Squares are not linear between samples: a three point Simpson
            // rule is exact for the quadratic that v² is on a linear span.
            let vm = 0.5 * (va + vb);
            sum += (tb - ta) / 6.0 * (f(va) + 4.0 * f(vm) + f(vb));
        }
        Some(sum)
    }

    /// Times at which the signal crosses `level`, rising (`true`) or
    /// falling (`false`) as the second element says.
    pub fn crossings(&self, level: f64) -> Vec<(f64, bool)> {
        let mut out = Vec::new();
        for i in 1..self.t.len() {
            let (a, b) = (self.v[i - 1] - level, self.v[i] - level);
            if (a < 0.0 && b >= 0.0) || (a > 0.0 && b <= 0.0) {
                let f = if b == a { 0.0 } else { -a / (b - a) };
                out.push((self.t[i - 1] + f * (self.t[i] - self.t[i - 1]), b > a));
            }
        }
        out
    }

    /// At most about `max_points` samples, keeping each bucket's minimum and
    /// maximum in time order so peaks and dips survive.
    pub fn decimate(&self, max_points: usize) -> Trace {
        let max_points = max_points.max(4);
        if self.len() <= max_points {
            return self.clone();
        }
        let buckets = max_points / 2;
        let per = self.len().div_ceil(buckets);
        let mut out = Trace::new(self.name.clone(), self.unit.clone());
        out.push(self.t[0], self.v[0]);
        for chunk in (1..self.len() - 1).step_by(per) {
            let end = (chunk + per).min(self.len() - 1);
            let (mut lo, mut hi) = (chunk, chunk);
            for i in chunk..end {
                if self.v[i] < self.v[lo] {
                    lo = i;
                }
                if self.v[i] > self.v[hi] {
                    hi = i;
                }
            }
            let (a, b) = if lo < hi { (lo, hi) } else { (hi, lo) };
            out.push(self.t[a], self.v[a]);
            if b != a {
                out.push(self.t[b], self.v[b]);
            }
        }
        let last = self.len() - 1;
        out.push(self.t[last], self.v[last]);
        out
    }
}

/// The signals of one run.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TraceSet {
    pub traces: Vec<Trace>,
}

impl TraceSet {
    pub fn get(&self, name: &str) -> Option<&Trace> {
        self.traces.iter().find(|t| t.name == name)
    }

    pub fn add(&mut self, trace: Trace) {
        self.traces.push(trace);
    }

    /// Latest time of any trace.
    pub fn end(&self) -> f64 {
        self.traces
            .iter()
            .filter_map(|t| t.span().map(|s| s.1))
            .fold(0.0, f64::max)
    }

    /// Comma separated values on the union of all sample times, one column per
    /// trace (linear interpolation where a trace has no sample), for a
    /// spreadsheet or a scope tool.
    pub fn to_csv(&self) -> String {
        let mut times: Vec<f64> = self.traces.iter().flat_map(|t| t.t.clone()).collect();
        times.sort_by(f64::total_cmp);
        times.dedup();
        let mut out = String::from("t_s");
        for t in &self.traces {
            out.push(',');
            out.push_str(&format!("{} [{}]", t.name.replace(',', ";"), t.unit));
        }
        out.push('\n');
        for time in times {
            out.push_str(&format!("{time:.9}"));
            for t in &self.traces {
                out.push_str(&format!(",{:.9e}", t.value_at(time).unwrap_or(f64::NAN)));
            }
            out.push('\n');
        }
        out
    }
}
