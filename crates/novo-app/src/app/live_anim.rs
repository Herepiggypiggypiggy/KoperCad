//! Device animation in the live view (WS-21): lamps glow, motors turn and
//! coils pull in, driven by the states novo-sim derives
//! (`novo_sim::device_states`). The animation values live in the workbench
//! next to the live view and are never stored in the project. Each value
//! eases towards its target, so a lamp fades and a motor spins up and down
//! instead of jumping.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use eframe::egui;
use novo_core::{PageId, Project, SymbolId, SymbolInstance};
use novo_sim::{
    DeviceStates, Direction, MotorRun, PoleState, TimerKind, TimerProgress, is_coil, is_lamp,
    is_motor, lamp_color,
};
use novo_symbols::{Graphic, Library};
use novo_theme::tokens;

/// Rate at which glow and pull-in ease, per second.
const EASE: f32 = 14.0;
/// Width of the lines the canvas draws symbols with, in millimetres.
const SYMBOL_STROKE_MM: f32 = 0.35;
/// Turns per second of a running motor at speed 0.
const TURNS: f32 = 1.2;

#[derive(Default)]
pub(crate) struct Anim {
    /// 0..1 glow of lamps and pull-in of coils.
    level: HashMap<SymbolId, f32>,
    /// Rotation angle (radians) and eased spin factor (-1..1, times speed).
    motors: HashMap<SymbolId, (f32, f32)>,
    /// 0..1 travel of a contact blade from its drawn rest position.
    blades: HashMap<SymbolId, f32>,
    /// Seconds since the first frame, for shake and pulsing.
    time: f32,
}

/// Page position of the circle in a symbol's artwork (lamp bulb, motor M),
/// else the middle of its pins, with the radius in millimetres.
fn body(library: Option<&Library>, symbol: &SymbolInstance) -> Option<(egui::Vec2, f32)> {
    let circle = library.and_then(|l| l.get(&symbol.symbol.0)).and_then(|s| {
        s.graphics.iter().find_map(|g| match g {
            Graphic::Circle { center, r, .. } => Some((center[0], center[1], *r)),
            _ => None,
        })
    });
    if let Some((x, y, r)) = circle {
        let at = symbol.placement.apply(novo_core::Point::new(x, y));
        return Some((egui::vec2(at.x as f32, at.y as f32), r as f32));
    }
    let pins: Vec<_> = (0..symbol.pins.len())
        .filter_map(|i| symbol.pin_position(i as u16))
        .collect();
    if pins.is_empty() {
        return None;
    }
    let n = pins.len() as f64;
    let x = pins.iter().map(|p| p.x).sum::<f64>() / n;
    let y = pins.iter().map(|p| p.y).sum::<f64>() / n;
    Some((egui::vec2(x as f32, y as f32), 4.0))
}

fn lamp_tint(name: Option<&str>) -> egui::Color32 {
    match name {
        Some("red" | "rood") => egui::Color32::from_rgb(235, 70, 60),
        Some("green" | "groen") => egui::Color32::from_rgb(70, 200, 100),
        Some("blue" | "blauw") => egui::Color32::from_rgb(80, 140, 240),
        Some("white" | "wit") => egui::Color32::from_rgb(240, 240, 235),
        _ => egui::Color32::from_rgb(250, 190, 60),
    }
}

/// Everything one frame of the animation reads.
pub(super) struct Frame<'a> {
    pub ctx: &'a egui::Context,
    pub canvas: egui::Rect,
    pub zoom: f32,
    pub to_screen: &'a dyn Fn(egui::Vec2) -> egui::Pos2,
    pub project: &'a Project,
    pub library: Option<&'a Library>,
    pub page: PageId,
    pub states: &'a DeviceStates,
    pub energized: &'a BTreeSet<SymbolId>,
    pub poles: &'a PoleState,
    pub timers: &'a BTreeMap<SymbolId, TimerProgress>,
    /// Switch states without easing and show a direction arrow instead of
    /// spinning (Preferences, Simulation).
    pub reduced: bool,
    pub dt: f32,
}

/// The moving blade of a single-pole contact, in symbol coordinates.
struct Blade {
    pivot: [f64; 2],
    /// Where the artwork draws the blade at rest.
    rest: [f64; 2],
    /// Where it swings to when the contact operates.
    operated: [f64; 2],
    /// The contact is closed at rest (normally closed).
    closed_at_rest: bool,
}

/// Finds the blade in the artwork of a make or break contact: the slanted
/// line with one end on the pole axis (the pivot) and one off it (the tip).
/// A make contact swings its tip onto the axis, a break contact to the
/// other side.
fn blade(library: Option<&Library>, symbol: &SymbolInstance) -> Option<Blade> {
    if symbol.pass_through.len() != 1 || symbol.pass_through[0].len() != 2 {
        return None;
    }
    let name = symbol.symbol.0.rsplit('/').next().unwrap_or("");
    let base = name.split("_delay_").next().unwrap_or(name);
    let normally_closed = base == "nc" || base.ends_with("_nc");
    if !(normally_closed || base == "no" || base.ends_with("_no")) {
        return None;
    }
    let def = library?.get(&symbol.symbol.0)?;
    let [a, b] = [symbol.pass_through[0][0], symbol.pass_through[0][1]];
    let pa = symbol.pins.get(usize::from(a))?.offset;
    let pb = symbol.pins.get(usize::from(b))?.offset;
    let axis = pa.x;
    if (pb.x - axis).abs() > 0.01 {
        return None;
    }
    let on_axis = |p: [f64; 2]| (p[0] - axis).abs() < 0.01;
    let (pivot, rest) = def.graphics.iter().find_map(|g| match g {
        Graphic::Line { from, to, .. } if (from[1] - to[1]).abs() > 0.5 => {
            match (on_axis(*from), on_axis(*to)) {
                (true, false) => Some((*from, *to)),
                (false, true) => Some((*to, *from)),
                _ => None,
            }
        }
        _ => None,
    })?;
    // The end of the stub from the first pin: where a make contact closes.
    let stub = def.graphics.iter().find_map(|g| match g {
        Graphic::Line { from, to, .. } if *from == [pa.x, pa.y] && on_axis(*to) => Some(*to),
        Graphic::Line { from, to, .. } if *to == [pa.x, pa.y] && on_axis(*from) => Some(*from),
        _ => None,
    })?;
    let operated = if normally_closed {
        [2.0 * axis - rest[0], rest[1]]
    } else {
        stub
    };
    Some(Blade {
        pivot,
        rest,
        operated,
        closed_at_rest: normally_closed,
    })
}

impl Anim {
    fn ease(&mut self, id: SymbolId, target: f32, dt: f32) -> f32 {
        let v = self.level.entry(id).or_insert(0.0);
        *v += (target - *v) * (1.0 - (-EASE * dt).exp());
        if (*v - target).abs() < 0.005 {
            *v = target;
        }
        *v
    }

    /// Paints the device animation of `f.page` over the canvas and returns
    /// whether anything is still moving (the caller keeps repainting).
    pub(super) fn paint(&mut self, f: &Frame<'_>) -> bool {
        let dt = f.dt.min(0.1);
        self.time += dt;
        // Reduced motion: levels jump to their target at once.
        let ease_dt = if f.reduced { 10.0 } else { dt };
        let to_screen = f.to_screen;
        let painter = f
            .ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("live_anim"),
            ))
            .with_clip_rect(f.canvas);
        let palette = novo_theme::CANVAS_DARK;
        let sheet = novo_theme::canvas::to_color32(palette.sheet);
        let ink = novo_theme::canvas::to_color32(palette.symbol);
        let mut moving = false;
        for symbol in f.project.symbols().filter(|s| s.page == f.page) {
            if is_lamp(symbol) {
                let lit = f.states.lamps.get(&symbol.id).copied().unwrap_or(false);
                let target = f32::from(u8::from(lit));
                let glow = self.ease(symbol.id, target, ease_dt);
                moving |= glow != target;
                let (Some((at, r)), true) = (body(f.library, symbol), glow > 0.0) else {
                    continue;
                };
                let tint = lamp_tint(lamp_color(f.project, symbol).as_deref());
                let (c, r) = (to_screen(at), r * f.zoom);
                for (scale, alpha) in [(2.6, 0.10), (1.9, 0.16), (1.3, 0.28)] {
                    painter.circle_filled(c, r * scale, tint.gamma_multiply(alpha * glow));
                }
                painter.circle_filled(c, r * 0.85, tint.gamma_multiply(0.9 * glow));
            } else if is_motor(symbol) {
                let run = f
                    .states
                    .motors
                    .get(&symbol.id)
                    .copied()
                    .unwrap_or(MotorRun::Stopped);
                let (target, tint) = match run {
                    MotorRun::Running { direction, speed } => {
                        let sign = if direction == Direction::Forward {
                            1.0
                        } else {
                            -1.0
                        };
                        (sign * (1.0 + f32::from(speed)), tokens::SUCCESS)
                    }
                    MotorRun::PhaseLoss => (0.0, tokens::WARNING),
                    MotorRun::Fault => (0.0, tokens::ERROR),
                    MotorRun::Stopped => (0.0, tokens::TEXT_MUTED),
                };
                let (angle, spin) = self.motors.entry(symbol.id).or_insert((0.0, 0.0));
                if f.reduced {
                    // No spinning: the arrow below says which way it runs.
                    *spin = target;
                    *angle = 0.0;
                } else {
                    *spin += (target - *spin) * (1.0 - (-3.0 * dt).exp());
                    if (*spin - target).abs() < 0.01 {
                        *spin = target;
                    }
                    *angle += *spin * TURNS * std::f32::consts::TAU * dt;
                }
                let angle = *angle;
                let spin = *spin;
                let turning = spin != 0.0;
                moving |= !f.reduced && (turning || spin != target);
                let humming = matches!(run, MotorRun::PhaseLoss);
                let faulty = matches!(run, MotorRun::Fault);
                if !turning && !humming && !faulty {
                    continue;
                }
                moving |= humming && !f.reduced;
                let Some((at, r)) = body(f.library, symbol) else {
                    continue;
                };
                let mut c = to_screen(at);
                let r = r * f.zoom;
                if humming && !f.reduced {
                    c.x += (self.time * 60.0).sin() * r * 0.04;
                }
                painter.circle_filled(c, r * 0.92, tokens::BG_0.gamma_multiply(0.85));
                painter.circle_stroke(c, r, egui::Stroke::new(2.0, tint));
                if turning || humming {
                    for k in 0..3 {
                        let t = angle + k as f32 * std::f32::consts::TAU / 3.0;
                        let tip = c + egui::vec2(t.cos(), t.sin()) * r * 0.75;
                        painter.line_segment([c, tip], egui::Stroke::new(2.0, tint));
                    }
                    painter.circle_filled(c, r * 0.12, tint);
                    if f.reduced && turning {
                        // A chevron above the hub: right for forward, left for reverse.
                        let side = spin.signum();
                        let tip = c + egui::vec2(side * r * 0.45, -r * 0.5);
                        painter.add(egui::Shape::convex_polygon(
                            vec![
                                tip,
                                tip + egui::vec2(-side * r * 0.3, -r * 0.2),
                                tip + egui::vec2(-side * r * 0.3, r * 0.2),
                            ],
                            tint,
                            egui::Stroke::NONE,
                        ));
                    }
                } else {
                    painter.text(
                        c,
                        egui::Align2::CENTER_CENTER,
                        "!",
                        egui::FontId::proportional(r),
                        tint,
                    );
                }
            } else if is_coil(symbol) {
                let on = f.energized.contains(&symbol.id);
                let target = f32::from(u8::from(on));
                let pull = self.ease(symbol.id, target, ease_dt);
                moving |= pull != target;
                let (Some(a), Some(b)) = (symbol.pin_position(0), symbol.pin_position(1)) else {
                    continue;
                };
                let mid = egui::vec2(((a.x + b.x) / 2.0) as f32, ((a.y + b.y) / 2.0) as f32);
                let c = to_screen(mid);
                let r = ((b.x - a.x).hypot(b.y - a.y) as f32 * 0.5 * f.zoom).max(10.0);
                if pull > 0.0 {
                    painter.circle_stroke(
                        c,
                        r * (1.6 - 0.3 * pull),
                        egui::Stroke::new(2.0, tokens::SUCCESS.gamma_multiply(0.6 * pull)),
                    );
                }
                // A timer counting: a ring that fills (on-delay) or drains
                // (off-delay) with the time left.
                if let Some(t) = f.timers.get(&symbol.id) {
                    let share = match t.kind {
                        TimerKind::OnDelay => t.elapsed,
                        TimerKind::OffDelay => 1.0 - t.elapsed,
                    } as f32;
                    let ring = r * 1.9;
                    painter.circle_stroke(c, ring, egui::Stroke::new(1.5, tokens::BORDER_STRONG));
                    let steps = 48;
                    let used = ((share * steps as f32).ceil() as usize).min(steps);
                    let points: Vec<egui::Pos2> = (0..=used)
                        .map(|i| {
                            let t = -std::f32::consts::FRAC_PI_2
                                + (i as f32 / steps as f32) * std::f32::consts::TAU;
                            c + egui::vec2(t.cos(), t.sin()) * ring
                        })
                        .collect();
                    if points.len() > 1 {
                        painter.add(egui::Shape::line(
                            points,
                            egui::Stroke::new(3.0, tokens::WARNING),
                        ));
                    }
                    painter.text(
                        c + egui::vec2(0.0, -ring - 8.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{:.1} s", t.remaining),
                        egui::FontId::proportional(tokens::FONT_SMALL),
                        tokens::WARNING,
                    );
                    moving = true;
                }
            } else if let Some(blade) = blade(f.library, symbol) {
                // Operated when the pole differs from its drawn rest state.
                let closed = f.poles.is_closed(symbol.id, 0);
                let target = f32::from(u8::from(closed != blade.closed_at_rest));
                let k = self.blade_ease(symbol.id, target, ease_dt);
                moving |= k != target;
                if k == 0.0 {
                    continue;
                }
                let place = |p: [f64; 2]| {
                    let at = symbol.placement.apply(novo_core::Point::new(p[0], p[1]));
                    to_screen(egui::vec2(at.x as f32, at.y as f32))
                };
                let pivot = place(blade.pivot);
                // The blade has the width of the artwork's own lines.
                let width = (SYMBOL_STROKE_MM * f.zoom).max(1.0);
                // Wipe the drawn blade, then draw it where it is now.
                painter.line_segment(
                    [pivot, place(blade.rest)],
                    egui::Stroke::new((width * 1.5).max(width + 2.0), sheet),
                );
                let mix = |a: f64, b: f64| a + (b - a) * f64::from(k);
                let tip = place([
                    mix(blade.rest[0], blade.operated[0]),
                    mix(blade.rest[1], blade.operated[1]),
                ]);
                painter.line_segment([pivot, tip], egui::Stroke::new(width, ink));
            }
        }
        moving
    }

    fn blade_ease(&mut self, id: SymbolId, target: f32, dt: f32) -> f32 {
        let v = self.blades.entry(id).or_insert(0.0);
        *v += (target - *v) * (1.0 - (-EASE * dt).exp());
        if (*v - target).abs() < 0.01 {
            *v = target;
        }
        *v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Pin, Placement, Point};

    fn lamp_at(x: f64, y: f64) -> SymbolInstance {
        placed("iec/signalling/lamp", x, y)
    }

    fn placed(key: &str, x: f64, y: f64) -> SymbolInstance {
        let library = Library::builtin().unwrap();
        let def = library.get(key).unwrap();
        let mut s = SymbolInstance {
            id: SymbolId(1),
            page: PageId(1),
            symbol: key.into(),
            placement: Placement::at(Point::new(x, y)),
            role: Default::default(),
            pins: def
                .connections
                .iter()
                .map(|c| Pin::new(c.id.clone(), c.at[0], c.at[1]))
                .collect(),
            bridges: Vec::new(),
            pass_through: def
                .pass_through()
                .paths()
                .iter()
                .map(|p| p.iter().map(|&i| i as u16).collect())
                .collect(),
            feeds: Vec::new(),
            properties: Default::default(),
        };
        s.placement = Placement::at(Point::new(x, y));
        s
    }

    #[test]
    fn the_lamp_bulb_is_found_in_the_artwork() {
        let library = Library::builtin().unwrap();
        let (at, r) = body(Some(&library), &lamp_at(100.0, 50.0)).unwrap();
        // The bulb circle sits 8 mm below the symbol origin, radius 4.
        assert_eq!((at.x, at.y, r), (100.0, 58.0, 4.0));
    }

    #[test]
    fn levels_ease_towards_their_target_and_settle() {
        let mut anim = Anim::default();
        let id = SymbolId(1);
        let first = anim.ease(id, 1.0, 0.016);
        assert!(first > 0.0 && first < 1.0);
        let mut v = first;
        for _ in 0..200 {
            v = anim.ease(id, 1.0, 0.016);
        }
        assert_eq!(v, 1.0);
        assert_eq!(anim.ease(id, 0.0, 10.0), 0.0);
    }

    #[test]
    fn a_make_contact_swings_its_blade_onto_the_axis() {
        let library = Library::builtin().unwrap();
        let b = blade(Some(&library), &placed("iec/contacts/no", 0.0, 0.0)).unwrap();
        assert_eq!(b.pivot, [0.0, 11.0]);
        assert_eq!(b.rest, [-4.0, 4.0]);
        assert_eq!(b.operated, [0.0, 5.0]);
        assert!(!b.closed_at_rest);
    }

    #[test]
    fn a_break_contact_swings_its_blade_to_the_other_side() {
        let library = Library::builtin().unwrap();
        let b = blade(Some(&library), &placed("iec/contacts/nc", 0.0, 0.0)).unwrap();
        assert_eq!(b.rest, [4.0, 4.0]);
        assert_eq!(b.operated, [-4.0, 4.0]);
        assert!(b.closed_at_rest);
        // Changeovers and non-contacts have no single blade.
        assert!(blade(Some(&library), &placed("iec/contacts/changeover", 0.0, 0.0)).is_none());
        assert!(blade(Some(&library), &lamp_at(0.0, 0.0)).is_none());
    }
}
