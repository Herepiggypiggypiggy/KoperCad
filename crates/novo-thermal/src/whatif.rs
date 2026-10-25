//! Cooling what-if: one question, "what do I do about the heat?", answered by
//! trying the fixes that exist against each other in the same model.
//!
//! The person states the conditions (the hottest ambient and how much of the
//! entered loss is on: the duty). The engine runs the cabinet under them, then
//! tries, each on the same grid so that before and after are fair:
//!
//! 1. **Layout**: the cooler layout of [`crate::placement`] (devices move
//!    along their rails);
//! 2. **Openings**: the best passive pair and the best powered pair of the
//!    vent and fan sweep ([`crate::sweep`]);
//! 3. **Layout and openings**: the sweep on the moved layout, when neither
//!    alone is enough;
//! 4. **Bigger enclosure**: the outside size grown, which adds air and wall
//!    surface (a model run only; changing the enclosure is a design decision
//!    that is made by hand).
//!
//! The fixes are ranked by what the person cares about: devices still over
//! their limit first; among fixes that leave none over, the easiest one first
//! (effort), then the larger margin; among fixes that leave some over, the
//! tightest margin (in half-kelvin steps so that the wobble of the solver
//! does not decide), then the effort it takes. Everything
//! here is the 2D model with planning values; nothing is a measurement, and the
//! result says so ([`MODEL_NOTE`]).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use crate::model::*;
use crate::placement::{self, Move, Row};
use crate::setup::Error;
use crate::sweep::{self, Candidate, FINE_CELL_MM, Reading, SIGNIFICANT_K};

/// The one statement of what the numbers are.
pub const MODEL_NOTE: &str = "Model estimate: the 2D heat model with planning values and the losses you entered, on a grid that is the same for before and after. Not validated against measurements (the enclosure model is not validated), and the 2D slice does not see the depth of the cabinet. Check the fix you pick with Simulate in 3D.";

/// Fewest kelvin of margin that makes a device "fine"; tries that reach it tie
/// and the easier fix wins.
const COMFORT_K: f64 = 2.0;
/// A cabinet with at least this margin everywhere has nothing to fix, K.
const NOTHING_TO_FIX_K: f64 = 5.0;
/// A fix that wins less than this, K, is not worth listing.
const MIN_GAIN_K: f64 = 0.3;

/// What the person says about the day the cabinet has to survive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    /// Air temperature outside, degrees C.
    pub ambient_c: f64,
    /// Share of the entered losses that is on, 0.1 to 1: 1 is the losses as
    /// entered (every device at its stated load).
    pub duty: f64,
}

impl Conditions {
    /// The cabinet as entered, at its own ambient.
    pub fn as_entered(scn: &Scenario) -> Conditions {
        Conditions {
            ambient_c: scn.ambient_c,
            duty: 1.0,
        }
    }

    /// The scenario under these conditions.
    pub fn apply(&self, scn: &Scenario) -> Scenario {
        let duty = self.duty.clamp(0.1, 1.0);
        let mut out = scn.clone();
        out.ambient_c = self.ambient_c;
        for d in &mut out.devices {
            d.loss_w *= duty;
        }
        for o in &mut out.obstacles {
            o.loss_w *= duty;
        }
        out
    }
}

/// The rails of the cabinet, for the layout search.
#[derive(Debug, Clone, Copy)]
pub struct Rails<'a> {
    pub rows: &'a [Row],
    /// Per scenario device the row it stands on.
    pub row_of: &'a [Option<usize>],
}

/// What to try and how hard.
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub layout: placement::Settings,
    pub sweep: sweep::Settings,
    /// Outside width and height are multiplied by this for the bigger
    /// enclosure (rounded up to 100 mm); 1 leaves the option out.
    pub enclosure_growth: f64,
    /// Fixes listed.
    pub show: usize,
    /// Go on even when no device is short of margin.
    pub always: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            layout: placement::Settings::default(),
            sweep: sweep::Settings::default(),
            enclosure_growth: 1.25,
            show: 5,
            always: false,
        }
    }
}

impl Settings {
    /// A small ladder for tests and a first look.
    pub fn quick() -> Settings {
        Settings {
            sweep: sweep::Settings::quick(),
            ..Settings::default()
        }
    }
}

/// Stages, in the order they run.
pub const STAGES: [&str; 5] = [
    "The cabinet as it is",
    "Cooler layout",
    "Vents and fans",
    "Layout with vents and fans",
    "Bigger enclosure",
];

/// Progress and cancel, shared with the thread that shows the run.
#[derive(Debug, Default)]
pub struct Control {
    pub cancel: AtomicBool,
    /// Index into [`STAGES`] being worked on.
    pub stage: AtomicUsize,
    pub(crate) sweep: sweep::Control,
    pub(crate) layout: placement::Control,
}

impl Control {
    /// Ends the run with what has been found so far.
    pub fn stop(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.sweep.cancel.store(true, Ordering::Relaxed);
        self.layout.cancel.store(true, Ordering::Relaxed);
    }

    /// Tries run so far, over all stages.
    pub fn evaluated(&self) -> usize {
        self.sweep.evaluated.load(Ordering::Relaxed) + self.layout.evaluated.load(Ordering::Relaxed)
    }

    pub fn stage_name(&self) -> &'static str {
        STAGES[self.stage.load(Ordering::Relaxed).min(STAGES.len() - 1)]
    }

    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn enter(&self, stage: usize) {
        self.stage.store(stage, Ordering::Relaxed);
    }
}

/// The outside size of an enclosure, mm.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Size {
    pub width_mm: f64,
    pub height_mm: f64,
    pub depth_mm: f64,
}

/// One thing to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Fix {
    /// Move devices along their rails.
    Layout { moves: Vec<Move> },
    /// Add a pair of vents or fans.
    Openings { candidate: Candidate },
    /// Both: the pair is the best one for the moved layout.
    LayoutAndOpenings {
        moves: Vec<Move>,
        candidate: Candidate,
    },
    /// A larger enclosure.
    BiggerEnclosure { from: Size, to: Size },
}

impl Fix {
    /// Short name for lists.
    pub fn title(&self) -> String {
        match self {
            Fix::Layout { moves } => format!(
                "Move {} device{} to cooler places",
                moves.len(),
                if moves.len() == 1 { "" } else { "s" }
            ),
            Fix::Openings { candidate } => format!("Add: {}", candidate.pairing.label()),
            Fix::LayoutAndOpenings { moves, candidate } => format!(
                "Move {} and add: {}",
                moves.len(),
                candidate.pairing.label().to_lowercase()
            ),
            Fix::BiggerEnclosure { to, .. } => format!(
                "Bigger enclosure, {:.0} x {:.0} mm",
                to.width_mm, to.height_mm
            ),
        }
    }

    /// What it takes, 1 (a drag) to 5 (a different enclosure), to break ties.
    pub fn effort(&self) -> u8 {
        match self {
            Fix::Layout { .. } => 1,
            Fix::Openings { candidate } => {
                if candidate.pairing.fans() == 0 {
                    2
                } else {
                    3
                }
            }
            Fix::LayoutAndOpenings { candidate, .. } => {
                if candidate.pairing.fans() == 0 {
                    3
                } else {
                    4
                }
            }
            Fix::BiggerEnclosure { .. } => 5,
        }
    }

    /// The effort in words.
    pub fn effort_word(&self) -> &'static str {
        match self.effort() {
            1 => "Easy: drag devices",
            2 => "Cut-outs in the cabinet, no power",
            3 => "Cut-outs and fans to wire",
            4 => "Moves, cut-outs and fans",
            _ => "A different enclosure",
        }
    }

    /// The moves of the fix, if it has any.
    pub fn moves(&self) -> &[Move] {
        match self {
            Fix::Layout { moves } | Fix::LayoutAndOpenings { moves, .. } => moves,
            _ => &[],
        }
    }

    /// The pair of openings of the fix, if it has one.
    pub fn candidate(&self) -> Option<&Candidate> {
        match self {
            Fix::Openings { candidate } | Fix::LayoutAndOpenings { candidate, .. } => {
                Some(candidate)
            }
            _ => None,
        }
    }

    /// Whether KoperCAD can carry the fix out on the project; a different
    /// enclosure is a decision made by hand.
    pub fn applicable(&self) -> bool {
        !matches!(self, Fix::BiggerEnclosure { .. })
    }
}

/// A fix with what the model says it wins.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub fix: Fix,
    pub reading: Reading,
    /// Tightest margin gained over the baseline (or hottest air lost when no
    /// device has a limit), K.
    pub gain_k: f64,
    /// The gain is at least [`SIGNIFICANT_K`].
    pub clear: bool,
    /// No device is over its limit with this fix.
    pub solves: bool,
    /// 1 is the best.
    pub rank: usize,
}

/// The finished what-if.
#[derive(Debug, Clone, PartialEq)]
pub struct WhatIf {
    pub conditions: Conditions,
    /// The cabinet as it is under the conditions.
    pub baseline: Reading,
    /// Scenario device ids in the order of the readings.
    pub device_ids: Vec<u64>,
    /// Best first.
    pub choices: Vec<Choice>,
    /// Layouts run through the model in all.
    pub tried: usize,
    /// Stopped early: the list is what was found so far.
    pub partial: bool,
    pub notes: Vec<String>,
}

impl WhatIf {
    /// Devices of the baseline that are over their limit.
    pub fn over_before(&self) -> usize {
        self.baseline.over
    }
}

/// Better first. Fixes that clear every limit rank by effort first (the
/// easiest one that clears them), then by margin; the rest rank by devices
/// over, the tightest margin (half-kelvin steps) and then effort.
fn order(a: &Choice, b: &Choice) -> std::cmp::Ordering {
    let step = |r: &Reading| match r.worst_margin_k {
        Some(m) => -((m.min(COMFORT_K) * 2.0).round() as i64),
        None => (r.max_air_c * 2.0).round() as i64,
    };
    let over = a.reading.over.cmp(&b.reading.over);
    if over.is_eq() && a.reading.over == 0 {
        return a
            .fix
            .effort()
            .cmp(&b.fix.effort())
            .then(step(&a.reading).cmp(&step(&b.reading)))
            .then(b.gain_k.total_cmp(&a.gain_k))
            .then(a.reading.max_air_c.total_cmp(&b.reading.max_air_c));
    }
    over.then(step(&a.reading).cmp(&step(&b.reading)))
        .then(a.fix.effort().cmp(&b.fix.effort()))
        .then(b.gain_k.total_cmp(&a.gain_k))
        .then(a.reading.max_air_c.total_cmp(&b.reading.max_air_c))
}

fn rounded_up(v: f64) -> f64 {
    (v / 100.0).ceil() * 100.0
}

/// Runs the what-if. `control` can stop it (the best so far is returned) and
/// shows progress. `rails` is `None` when the cabinet has no devices on
/// rails: the layout fixes are then left out.
pub fn run(
    scn: &Scenario,
    conditions: &Conditions,
    rails: Option<Rails<'_>>,
    settings: &Settings,
    control: &Control,
) -> Result<WhatIf, Error> {
    let base = conditions.apply(scn);
    let device_ids: Vec<u64> = base.devices.iter().map(|d| d.id).collect();
    control.enter(0);
    let (baseline, _, base_state) = sweep::run(&base, FINE_CELL_MM, None)?;
    control.sweep.evaluated.fetch_add(1, Ordering::Relaxed);
    let mut notes = vec![MODEL_NOTE.to_owned()];
    if conditions.duty < 0.999 {
        notes.push(format!(
            "Duty {:.0} %: every loss is multiplied by {:.2} before the run. A cabinet that only runs at part load can be fine where the same cabinet at full load is not.",
            100.0 * conditions.duty,
            conditions.duty
        ));
    }
    if baseline.unsettled {
        notes.push(
            "The model did not settle fully for the cabinet as it is: differences under about 1 K mean nothing.".into(),
        );
    }
    let mut out = WhatIf {
        conditions: *conditions,
        baseline: baseline.clone(),
        device_ids,
        choices: Vec::new(),
        tried: 1,
        partial: false,
        notes,
    };
    let fine = baseline
        .worst_margin_k
        .is_some_and(|m| m >= NOTHING_TO_FIX_K);
    if fine && !settings.always {
        out.notes.push(format!(
            "Every device is at least {NOTHING_TO_FIX_K:.0} K inside its limit under these conditions, so there is nothing to fix."
        ));
        return Ok(out);
    }

    let mut choices: Vec<Choice> = Vec::new();
    let add = |fix: Fix, reading: Reading, choices: &mut Vec<Choice>| {
        let gain_k = reading.gain_over(&baseline);
        choices.push(Choice {
            solves: reading.over == 0 && baseline.over > 0,
            clear: gain_k >= SIGNIFICANT_K,
            gain_k,
            rank: 0,
            fix,
            reading,
        });
    };

    // 1. The cooler layout.
    let mut moved: Option<(Vec<Move>, Scenario)> = None;
    if let Some(r) = rails
        && !control.cancelled()
    {
        control.enter(1);
        let s =
            placement::suggest_with(&base, r.rows, r.row_of, &settings.layout, &control.layout)?;
        out.tried += s.evaluated;
        if !s.moves.is_empty() {
            let (reading, _, _) = sweep::run(&s.scenario, FINE_CELL_MM, Some(&base_state))?;
            out.tried += 1;
            add(
                Fix::Layout {
                    moves: s.moves.clone(),
                },
                reading,
                &mut choices,
            );
            moved = Some((s.moves, s.scenario));
        } else {
            out.notes
                .push("No device has a cooler place on its rails that is worth the move.".into());
        }
    } else if rails.is_none() {
        out.notes.push(
            "No device stands on a rail, so no layout fix was tried. Place the devices first."
                .into(),
        );
    }

    // 2. Vents and fans: the best passive and the best powered pair.
    let mut sw = settings.sweep.clone();
    sw.always = true;
    if !control.cancelled() {
        control.enter(2);
        let s = sweep::sweep_with(&base, &sw, &control.sweep)?;
        out.tried += s.tried;
        out.partial |= s.partial;
        let mut passive = false;
        let mut powered = false;
        for o in &s.best {
            let seen = if o.candidate.pairing.fans() == 0 {
                &mut passive
            } else {
                &mut powered
            };
            if *seen {
                continue;
            }
            *seen = true;
            add(
                Fix::Openings {
                    candidate: o.candidate.clone(),
                },
                o.reading.clone(),
                &mut choices,
            );
        }
    }

    // 3. Layout and openings, when neither alone clears every limit.
    let alone_solves = choices.iter().any(|c| c.reading.over == 0);
    if let Some((moves, layout)) = moved
        && !alone_solves
        && baseline.over > 0
        && !control.cancelled()
    {
        control.enter(3);
        let s = sweep::sweep_with(&layout, &sw, &control.sweep)?;
        out.tried += s.tried;
        out.partial |= s.partial;
        if let Some(o) = s.best.first() {
            add(
                Fix::LayoutAndOpenings {
                    moves,
                    candidate: o.candidate.clone(),
                },
                o.reading.clone(),
                &mut choices,
            );
        }
    }

    // 4. A bigger enclosure.
    if settings.enclosure_growth > 1.0 && !control.cancelled() {
        control.enter(4);
        let e = &base.enclosure;
        let from = Size {
            width_mm: e.width_mm,
            height_mm: e.height_mm,
            depth_mm: e.depth_mm,
        };
        let to = Size {
            width_mm: rounded_up(e.width_mm * settings.enclosure_growth),
            height_mm: rounded_up(e.height_mm * settings.enclosure_growth),
            depth_mm: e.depth_mm,
        };
        let mut bigger = base.clone();
        bigger.enclosure.width_mm = to.width_mm;
        bigger.enclosure.height_mm = to.height_mm;
        let (reading, _, _) = sweep::run(&bigger, FINE_CELL_MM, Some(&base_state))?;
        out.tried += 1;
        add(Fix::BiggerEnclosure { from, to }, reading, &mut choices);
    }

    out.partial |= control.cancelled();
    choices.retain(|c| c.solves || c.gain_k >= MIN_GAIN_K);
    choices.sort_by(order);
    choices.truncate(settings.show);
    for (i, c) in choices.iter_mut().enumerate() {
        c.rank = i + 1;
    }
    if choices.is_empty() {
        out.notes.push(
            "None of the fixes that were tried wins anything worth listing. The heat has to be lowered (a more efficient device, a heat sink) or taken out by a cooling unit: see Cooling selection."
                .into(),
        );
    } else if !choices.iter().any(|c| c.solves) && baseline.over > 0 {
        out.notes.push(
            "No single fix clears every limit. The first one gains the most; the rest of the heat needs a cooling unit or less loss (see Cooling selection)."
                .into(),
        );
    }
    out.choices = choices;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn choice(fix: Fix, over: usize, margin: f64, gain: f64) -> Choice {
        Choice {
            fix,
            reading: Reading {
                worst_margin_k: Some(margin),
                over,
                max_air_c: 40.0,
                mean_air_c: 35.0,
                margins_k: vec![Some(margin)],
                surface_c: vec![50.0],
                unsettled: false,
            },
            gain_k: gain,
            clear: true,
            solves: over == 0,
            rank: 0,
        }
    }

    fn pair(fans: bool) -> Candidate {
        use crate::sweep::{Pairing, Piece};
        let piece = |placement, flow| Piece {
            opening: Opening {
                placement,
                width_mm: 150.0,
                height_mm: 150.0,
            },
            fan_flow_m3h: flow,
        };
        Candidate {
            pairing: if fans {
                Pairing::FanPair
            } else {
                Pairing::VentPair
            },
            inlet: piece(Placement::Left { y_mm: 400.0 }, fans.then_some(250.0)),
            outlet: piece(Placement::Top { x_mm: 300.0 }, fans.then_some(250.0)),
        }
    }

    #[test]
    fn among_fixes_that_clear_every_limit_the_easier_one_ranks_first() {
        let layout = choice(Fix::Layout { moves: Vec::new() }, 0, 1.2, 9.0);
        let fans = choice(
            Fix::Openings {
                candidate: pair(true),
            },
            0,
            3.0,
            12.0,
        );
        let mut v = [fans.clone(), layout.clone()];
        v.sort_by(order);
        assert!(
            matches!(v[0].fix, Fix::Layout { .. }),
            "a layout at +1.2 K beats fans at +3 K"
        );
        // Same effort: the larger margin first.
        let vents_a = choice(
            Fix::Openings {
                candidate: pair(false),
            },
            0,
            0.5,
            5.0,
        );
        let vents_b = choice(
            Fix::Openings {
                candidate: pair(false),
            },
            0,
            4.0,
            8.0,
        );
        let mut v = [vents_a, vents_b];
        v.sort_by(order);
        assert_eq!(v[0].reading.worst_margin_k, Some(4.0));
    }

    #[test]
    fn a_fix_that_leaves_fewer_devices_over_always_ranks_ahead() {
        let easy = choice(Fix::Layout { moves: Vec::new() }, 2, -5.0, 3.0);
        let hard = choice(
            Fix::Openings {
                candidate: pair(true),
            },
            1,
            -9.0,
            7.0,
        );
        let clears = choice(
            Fix::BiggerEnclosure {
                from: Size {
                    width_mm: 1.0,
                    height_mm: 1.0,
                    depth_mm: 1.0,
                },
                to: Size {
                    width_mm: 2.0,
                    height_mm: 2.0,
                    depth_mm: 1.0,
                },
            },
            0,
            0.1,
            9.0,
        );
        let mut v = [easy, hard, clears];
        v.sort_by(order);
        let overs: Vec<usize> = v.iter().map(|c| c.reading.over).collect();
        assert_eq!(overs, vec![0, 1, 2]);
    }
}
