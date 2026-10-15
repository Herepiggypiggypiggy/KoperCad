//! Advice that is tried before it is given: each candidate change is run
//! through the model and reported with what it wins.
//!
//! Candidates: move the worst device to a cooler spot of the plate, add an
//! intake and an exhaust fan sized with the sizing helper's `3.1 W/K` rule,
//! add a bottom and a top vent for chimney flow. The runs use a coarser grid
//! than the main result so that advice stays affordable; before and after are
//! both taken on that grid so the comparison is fair.

use novo_layout::Rect;
use novo_layout::sizing::FAN_FACTOR;

use crate::model::*;
use crate::report;
use crate::result::*;
use crate::setup::Error;
use crate::thermal::State;

/// Cell size of the advice runs, mm.
const ADVICE_CELL_MM: f64 = 30.0;
/// Smallest margin gain worth suggesting, K.
const MIN_GAIN_K: f64 = 0.3;
/// Fan flows a maker typically lists, m3/h.
const FAN_SIZES: [f64; 7] = [50.0, 100.0, 150.0, 250.0, 400.0, 600.0, 800.0];

struct Score {
    /// Worst margin to a limit, K; `None` when no device has one.
    worst_margin: Option<f64>,
    max_air: f64,
    /// Margin per device in scenario order.
    margins: Vec<Option<f64>>,
}

fn score(scn: &Scenario, init: Option<&State>) -> Result<(Score, State), Error> {
    let mut coarse = scn.clone();
    coarse.params.cell_mm = coarse.params.cell_mm.max(ADVICE_CELL_MM);
    let (s, st) = crate::run_steady_from(&coarse, init)?;
    let r = report::build(&coarse, &s, &st, 1.0, None);
    let margins: Vec<Option<f64>> = r.devices.iter().map(|d| d.margin_k).collect();
    let worst = margins
        .iter()
        .flatten()
        .copied()
        .fold(None, |m: Option<f64>, v| Some(m.map_or(v, |m| m.min(v))));
    Ok((
        Score {
            worst_margin: worst,
            max_air: r.balance.max_air_c,
            margins,
        },
        st,
    ))
}

fn gain(before: &Score, after: &Score) -> f64 {
    match (before.worst_margin, after.worst_margin) {
        (Some(b), Some(a)) => a - b,
        // No limits known: judge by the hottest air.
        _ => before.max_air - after.max_air,
    }
}

enum Kind {
    Move { index: usize, rect: Rect },
    Fan { flow: f64 },
    Vent { area_cm2: f64 },
    Reduce { index: usize, loss_w: f64 },
}

struct Candidate {
    kind: Kind,
    scn: Scenario,
}

pub fn recommend(scn: &Scenario, res: &ThermalResult) -> Result<Vec<Recommendation>, Error> {
    let trouble: Vec<&DeviceResult> = res
        .devices
        .iter()
        .filter(|d| matches!(d.status, Status::Over | Status::Marginal))
        .collect();
    if trouble.is_empty() {
        return Ok(Vec::new());
    }
    let (before, base) = score(scn, None)?;
    let mut cands: Vec<Candidate> = Vec::new();

    // 1. Move the worst devices; 4. cut the loss of one that heats itself.
    let mut worst: Vec<&DeviceResult> = trouble.clone();
    worst.sort_by(|a, b| {
        a.margin_k
            .unwrap_or(f64::MAX)
            .total_cmp(&b.margin_k.unwrap_or(f64::MAX))
    });
    for d in worst.iter().take(2) {
        let Some(index) = scn.devices.iter().position(|x| x.id == d.id) else {
            continue;
        };
        for rect in candidate_spots(scn, res, index) {
            let mut moved = scn.clone();
            moved.devices[index].footprint = rect;
            cands.push(Candidate {
                kind: Kind::Move { index, rect },
                scn: moved,
            });
        }
        if let Some(limit) = d.rated_max_surface_c
            && d.surface_c > limit
            && d.surface_c > d.air_c
            && d.loss_w > 0.0
        {
            let keep = ((limit - 2.0 - d.air_c) / (d.surface_c - d.air_c)).clamp(0.0, 1.0);
            let loss_w = (d.loss_w * keep * 0.95).floor().max(0.0);
            let mut cut = scn.clone();
            cut.devices[index].loss_w = loss_w;
            cands.push(Candidate {
                kind: Kind::Reduce { index, loss_w },
                scn: cut,
            });
        }
    }

    // 2. Fan pair, from the flow the sizing rule asks for upwards.
    let total: f64 = scn.devices.iter().map(|d| d.loss_w).sum();
    let limit = trouble
        .iter()
        .filter_map(|d| d.rated_max_ambient_c)
        .fold(f64::INFINITY, f64::min);
    let allowed = if limit.is_finite() {
        (limit - scn.ambient_c - 2.0).max(5.0)
    } else {
        10.0
    };
    let need = FAN_FACTOR * total / allowed;
    let (w, h) = (
        scn.enclosure.interior_width_mm(),
        scn.enclosure.interior_height_mm(),
    );
    for flow in FAN_SIZES.iter().copied().filter(|f| *f >= need).take(3) {
        let size = match flow as u32 {
            0..=100 => 92.0,
            101..=250 => 120.0,
            251..=400 => 150.0,
            _ => 200.0,
        };
        let fan = |placement, mode| Fan {
            opening: Opening {
                placement,
                width_mm: size,
                height_mm: size,
            },
            mode,
            free_flow_m3h: flow,
            max_pressure_pa: 60.0,
        };
        let mut with_fans = scn.clone();
        with_fans.fans.push(fan(
            Placement::Front {
                x_mm: 0.3 * w,
                y_mm: 0.85 * h,
            },
            FanMode::Intake,
        ));
        with_fans
            .fans
            .push(fan(Placement::Top { x_mm: 0.7 * w }, FanMode::Exhaust));
        cands.push(Candidate {
            kind: Kind::Fan { flow },
            scn: with_fans,
        });
    }

    // 3. Vent pair.
    let vent_w = (0.3 * w).min(300.0);
    let vent_h = 100.0f64.min(scn.enclosure.interior_depth_mm() * 0.5);
    let vent = |placement| Vent {
        opening: Opening {
            placement,
            width_mm: vent_w,
            height_mm: vent_h,
        },
        free_area_fraction: 0.6,
    };
    let mut with_vents = scn.clone();
    with_vents
        .vents
        .push(vent(Placement::Bottom { x_mm: 0.5 * w }));
    with_vents
        .vents
        .push(vent(Placement::Top { x_mm: 0.5 * w }));
    cands.push(Candidate {
        kind: Kind::Vent {
            area_cm2: vent_w * vent_h / 100.0,
        },
        scn: with_vents,
    });

    let scores = crate::par_map(&cands, |c| score(&c.scn, Some(&base)).map(|s| s.0));
    let mut done: Vec<(&Candidate, Score)> = Vec::new();
    for (c, s) in cands.iter().zip(scores) {
        done.push((c, s?));
    }

    let mut found: Vec<(f64, Recommendation)> = Vec::new();
    let rec = |advice: Advice, text: String, after: &Score| Recommendation {
        advice,
        text,
        worst_margin_before_k: before.worst_margin,
        worst_margin_after_k: after.worst_margin,
        max_air_before_c: before.max_air,
        max_air_after_c: after.max_air,
        rank: 0,
    };

    // Best move per device.
    for d in worst.iter().take(2) {
        let Some(index) = scn.devices.iter().position(|x| x.id == d.id) else {
            continue;
        };
        let best = done
            .iter()
            .filter_map(|(c, s)| match c.kind {
                Kind::Move { index: i, rect } if i == index => Some((rect, s)),
                _ => None,
            })
            .max_by(|a, b| {
                move_gain(&before, a.1, index).total_cmp(&move_gain(&before, b.1, index))
            });
        if let Some((rect, after)) = best {
            let (x, y) = (rect.x.round() as i64, rect.y.round() as i64);
            // A move is judged by the moved device's own margin.
            let own = |s: &Score| s.margins.get(index).copied().flatten();
            let text = match (own(&before), own(after)) {
                (Some(b), Some(a)) => format!(
                    "Move device {} to about ({x}, {y}) mm: its margin to the limit goes from {b:+.0} K to {a:+.0} K (model estimate).",
                    d.id
                ),
                _ => format!(
                    "Move device {} to about ({x}, {y}) mm (model estimate).",
                    d.id
                ),
            };
            if move_gain(&before, after, index) >= MIN_GAIN_K {
                found.push((
                    move_gain(&before, after, index),
                    rec(
                        Advice::MoveDevice {
                            id: d.id,
                            to_x_mm: x,
                            to_y_mm: y,
                        },
                        text,
                        after,
                    ),
                ));
            }
        }
    }

    // Smallest fan that clears every limit, else the best one tried.
    let fans: Vec<(f64, &Score)> = done
        .iter()
        .filter_map(|(c, s)| match c.kind {
            Kind::Fan { flow } => Some((flow, s)),
            _ => None,
        })
        .collect();
    let pick = fans
        .iter()
        .find(|(_, s)| s.worst_margin.is_some_and(|m| m >= 0.0))
        .or_else(|| {
            fans.iter()
                .max_by(|a, b| gain(&before, a.1).total_cmp(&gain(&before, b.1)))
        });
    if let Some(&(flow, after)) = pick
        && gain(&before, after) >= MIN_GAIN_K
    {
        found.push((
            gain(&before, after),
            rec(
                Advice::AddFanPair {
                    flow_m3h: flow as i64,
                },
                format!(
                    "Add a filter fan low in the door and an exhaust fan high in the roof, about {flow:.0} m3/h each. {}",
                    effect_text(&before, after)
                ),
                after,
            ),
        ));
    }

    for (c, after) in &done {
        match c.kind {
            Kind::Vent { area_cm2 } if gain(&before, after) >= MIN_GAIN_K => found.push((
                gain(&before, after),
                rec(
                Advice::AddVentPair {
                    area_cm2: area_cm2.round() as i64,
                },
                format!(
                    "Add a vent low and a vent high, about {area_cm2:.0} cm2 each, so warm air can leave by itself. {}",
                    effect_text(&before, after)
                ),
                after,
                ),
            )),
            Kind::Reduce { index, loss_w } if gain(&before, after) >= MIN_GAIN_K => {
                let id = scn.devices[index].id;
                let was = scn.devices[index].loss_w;
                found.push((
                    gain(&before, after),
                    rec(
                        Advice::ReduceLoss {
                            id,
                            max_loss_w: loss_w as i64,
                        },
                        format!(
                            "Device {id} heats its own housing too much ({was:.0} W): keep its loss near {loss_w:.0} W, or use a larger device or one with a heat sink. {}",
                            effect_text(&before, after)
                        ),
                        after,
                    ),
                ));
            }
            _ => {}
        }
    }

    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    Ok(found
        .into_iter()
        .enumerate()
        .map(|(i, (_, mut r))| {
            r.rank = i + 1;
            r
        })
        .collect())
}

/// Margin a move wins for the moved device itself, falling back to the
/// overall gain when it has no limit.
fn move_gain(before: &Score, after: &Score, index: usize) -> f64 {
    match (
        before.margins.get(index).copied().flatten(),
        after.margins.get(index).copied().flatten(),
    ) {
        (Some(b), Some(a)) => a - b,
        _ => gain(before, after),
    }
}

fn effect_text(before: &Score, after: &Score) -> String {
    match (before.worst_margin, after.worst_margin) {
        (Some(b), Some(a)) => format!(
            "The tightest margin goes from {b:+.0} K to {a:+.0} K; the hottest air from {:.0} to {:.0} C (model estimate).",
            before.max_air, after.max_air
        ),
        _ => format!(
            "The hottest air goes from {:.0} to {:.0} C (model estimate).",
            before.max_air, after.max_air
        ),
    }
}

/// Free spots on the plate where the device fits, coolest first, spaced
/// apart, at most three.
fn candidate_spots(scn: &Scenario, res: &ThermalResult, index: usize) -> Vec<Rect> {
    const STEP: f64 = 30.0;
    const GAP: f64 = 15.0;
    let dev = &scn.devices[index];
    let plate = scn.enclosure.plate;
    let (w, h) = (dev.footprint.w, dev.footprint.h);
    let blockers: Vec<Rect> = scn
        .devices
        .iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, d)| d.footprint)
        .chain(
            scn.obstacles
                .iter()
                .filter(|o| o.blockage() >= 0.9)
                .map(|o| o.rect),
        )
        .collect();
    let grow = |r: &Rect| Rect {
        x: r.x - GAP,
        y: r.y - GAP,
        w: r.w + 2.0 * GAP,
        h: r.h + 2.0 * GAP,
    };
    let mut spots: Vec<(f64, Rect)> = Vec::new();
    let mut y = plate.y;
    while y + h <= plate.bottom() + 1e-9 {
        let mut x = plate.x;
        while x + w <= plate.right() + 1e-9 {
            let rect = Rect { x, y, w, h };
            let far = (x - dev.footprint.x).hypot(y - dev.footprint.y) >= 100.0;
            if far && !blockers.iter().any(|b| grow(b).overlaps(&rect)) {
                let t = res.air_temp.sample(x + w / 2.0, y + h / 2.0);
                spots.push((t, rect));
            }
            x += STEP;
        }
        y += STEP;
    }
    spots.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<Rect> = Vec::new();
    for (_, r) in spots {
        if out.iter().all(|o| (o.x - r.x).hypot(o.y - r.y) >= 150.0) {
            out.push(r);
        }
        if out.len() >= 3 {
            break;
        }
    }
    out
}
