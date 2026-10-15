//! Device animation in the live view (WS-21): lamps glow, motors turn and
//! coils pull in, driven by the states novo-sim derives
//! (`novo_sim::device_states`). The animation values live in the workbench
//! next to the live view and are never stored in the project. Each value
//! eases towards its target, so a lamp fades and a motor spins up and down
//! instead of jumping.

use std::collections::{BTreeSet, HashMap};

use eframe::egui;
use novo_core::{PageId, Project, SymbolId, SymbolInstance};
use novo_sim::{
    DeviceStates, Direction, MotorRun, PoleState, delay_seconds, is_coil, is_lamp, is_motor,
    is_timer_contact, lamp_color,
};
use novo_symbols::{Graphic, Library};
use novo_theme::tokens;

/// Rate at which glow and pull-in ease, per second.
const EASE: f32 = 14.0;
/// Turns per second of a running motor at speed 0.
const TURNS: f32 = 1.2;

#[derive(Default)]
pub(crate) struct Anim {
    /// 0..1 glow of lamps and pull-in of coils.
    level: HashMap<SymbolId, f32>,
    /// Rotation angle (radians) and eased spin factor (-1..1, times speed).
    motors: HashMap<SymbolId, (f32, f32)>,
    /// Seconds since the first frame, for shake and pulsing.
    time: f32,
    /// Eased closing (0 open .. 1 closed) of each driven contact pole.
    blades: HashMap<(SymbolId, usize), f32>,
    /// Per timer coil: whether it was on, and for how many seconds it has
    /// been in that state.
    timers: HashMap<SymbolId, (bool, f32)>,
}

/// How far an open blade stands off its closed line, in radians.
const BLADE_OPEN: f32 = 0.5;

/// A ring arc of `fraction` of a turn, starting at the top.
fn arc(c: egui::Pos2, r: f32, fraction: f32, stroke: egui::Stroke) -> egui::Shape {
    let steps = ((fraction * 48.0).ceil() as usize).max(2);
    let points = (0..=steps)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2
                + fraction * std::f32::consts::TAU * i as f32 / steps as f32;
            c + egui::vec2(a.cos(), a.sin()) * r
        })
        .collect();
    egui::Shape::line(points, stroke)
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

impl Anim {
    fn ease(&mut self, id: SymbolId, target: f32, dt: f32) -> f32 {
        let v = self.level.entry(id).or_insert(0.0);
        *v += (target - *v) * (1.0 - (-EASE * dt).exp());
        if (*v - target).abs() < 0.005 {
            *v = target;
        }
        *v
    }

    /// Eases the blade of one pole towards open (0) or closed (1).
    fn blade(&mut self, key: (SymbolId, usize), target: f32, dt: f32) -> f32 {
        let v = self.blades.entry(key).or_insert(target);
        *v += (target - *v) * (1.0 - (-EASE * dt).exp());
        if (*v - target).abs() < 0.005 {
            *v = target;
        }
        *v
    }

    /// Seconds a timer coil has been in its present state, and whether it
    /// is on.
    fn timer_clock(&mut self, coil: SymbolId, on: bool, dt: f32) -> f32 {
        let (was, secs) = self.timers.entry(coil).or_insert((on, f32::INFINITY));
        if *was != on {
            *was = on;
            *secs = 0.0;
        }
        *secs += dt;
        *secs
    }

    /// Paints the device animation of `page` over the canvas and returns
    /// whether anything is still moving (the caller keeps repainting).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn paint(
        &mut self,
        ctx: &egui::Context,
        canvas: egui::Rect,
        zoom: f32,
        to_screen: &dyn Fn(egui::Vec2) -> egui::Pos2,
        project: &Project,
        library: Option<&Library>,
        page: PageId,
        states: &DeviceStates,
        energized: &BTreeSet<SymbolId>,
        contacts: (&BTreeSet<SymbolId>, &PoleState),
        reduced: bool,
        dt: f32,
    ) -> bool {
        // Reduced motion: every state switches at once. A huge step makes
        // each ease land on its target.
        let dt = if reduced { 1.0e3 } else { dt.min(0.1) };
        self.time = if reduced { 0.0 } else { self.time + dt };
        let painter = ctx
            .layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("live_anim"),
            ))
            .with_clip_rect(canvas);
        let mut moving = false;
        for symbol in project.symbols().filter(|s| s.page == page) {
            if is_lamp(symbol) {
                let lit = states.lamps.get(&symbol.id).copied().unwrap_or(false);
                let glow = self.ease(symbol.id, f32::from(u8::from(lit)), dt);
                moving |= glow != f32::from(u8::from(lit));
                let (Some((at, r)), true) = (body(library, symbol), glow > 0.0) else {
                    continue;
                };
                let tint = lamp_tint(lamp_color(project, symbol).as_deref());
                let (c, r) = (to_screen(at), r * zoom);
                for (scale, alpha) in [(2.6, 0.10), (1.9, 0.16), (1.3, 0.28)] {
                    painter.circle_filled(c, r * scale, tint.gamma_multiply(alpha * glow));
                }
                painter.circle_filled(c, r * 0.85, tint.gamma_multiply(0.9 * glow));
            } else if is_motor(symbol) {
                let run = states
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
                *spin += (target - *spin) * (1.0 - (-3.0 * dt).exp());
                if reduced {
                    *spin = target;
                }
                if (*spin - target).abs() < 0.01 {
                    *spin = target;
                }
                let turning = *spin != 0.0;
                if !reduced {
                    *angle += *spin * TURNS * std::f32::consts::TAU * dt;
                }
                let angle = *angle;
                moving |= !reduced && (turning || *spin != target);
                let humming = matches!(run, MotorRun::PhaseLoss);
                let faulty = matches!(run, MotorRun::Fault);
                if !turning && !humming && !faulty {
                    continue;
                }
                moving |= humming && !reduced;
                let Some((at, r)) = body(library, symbol) else {
                    continue;
                };
                let mut c = to_screen(at);
                let r = r * zoom;
                if humming {
                    c.x += (self.time * 60.0).sin() * r * 0.04;
                }
                painter.circle_filled(c, r * 0.92, tokens::BG_0.gamma_multiply(0.85));
                painter.circle_stroke(c, r, egui::Stroke::new(2.0, tint));
                if reduced && turning {
                    // A static arrow instead of spokes: clockwise or back.
                    let dir = if target > 0.0 { 1.0 } else { -1.0 };
                    let (a, b) = (
                        c + egui::vec2(-dir * r * 0.5, 0.0),
                        c + egui::vec2(dir * r * 0.5, 0.0),
                    );
                    painter.arrow(a, b - a, egui::Stroke::new(2.0, tint));
                } else if turning || humming {
                    let a = if humming { 0.0 } else { angle };
                    for k in 0..3 {
                        let t = a + k as f32 * std::f32::consts::TAU / 3.0;
                        let tip = c + egui::vec2(t.cos(), t.sin()) * r * 0.75;
                        painter.line_segment([c, tip], egui::Stroke::new(2.0, tint));
                    }
                    painter.circle_filled(c, r * 0.12, tint);
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
                let on = energized.contains(&symbol.id);
                let pull = self.ease(symbol.id, f32::from(u8::from(on)), dt);
                moving |= pull != f32::from(u8::from(on));
                let (Some(a), Some(b)) = (symbol.pin_position(0), symbol.pin_position(1)) else {
                    continue;
                };
                if let Some(device) = symbol.device() {
                    let timer = project
                        .symbols()
                        .filter(|t| t.device() == Some(device) && is_timer_contact(t))
                        .map(|t| t.symbol.0.ends_with("_delay_off"))
                        .next();
                    if let Some(off_delay) = timer {
                        let wait = delay_seconds(project, device) as f32;
                        let secs = self.timer_clock(symbol.id, on, dt);
                        let fraction = match (off_delay, on) {
                            // On-delay counting up while the coil is on.
                            (false, true) if wait > 0.0 => (secs / wait).min(1.0),
                            // Off-delay counting down after the coil dropped.
                            (true, false) if wait > 0.0 => 1.0 - (secs / wait).min(1.0),
                            _ => 1.0,
                        };
                        let active = match off_delay {
                            false => on && fraction < 1.0,
                            true => !on && fraction > 0.0 && secs.is_finite(),
                        };
                        if active {
                            moving = true;
                            let mid =
                                egui::vec2(((a.x + b.x) / 2.0) as f32, ((a.y + b.y) / 2.0) as f32);
                            let r = ((b.x - a.x).hypot(b.y - a.y) as f32 * 0.5 * zoom).max(10.0);
                            let c = to_screen(mid);
                            painter.circle_stroke(
                                c,
                                r * 1.25,
                                egui::Stroke::new(3.0, tokens::TEXT_MUTED.gamma_multiply(0.35)),
                            );
                            painter.add(arc(
                                c,
                                r * 1.25,
                                fraction,
                                egui::Stroke::new(3.0, tokens::WARNING),
                            ));
                        }
                    }
                }
                if pull > 0.0 {
                    let mid = egui::vec2(((a.x + b.x) / 2.0) as f32, ((a.y + b.y) / 2.0) as f32);
                    let c = to_screen(mid);
                    let r = ((b.x - a.x).hypot(b.y - a.y) as f32 * 0.5 * zoom).max(10.0);
                    painter.circle_stroke(
                        c,
                        r * (1.6 - 0.3 * pull),
                        egui::Stroke::new(2.0, tokens::SUCCESS.gamma_multiply(0.6 * pull)),
                    );
                }
            }
        }
        // Contacts: the moving blade swings from open to closed when the coil
        // pulls in, pivoting on the first pin of each pole.
        let (driven, poles) = contacts;
        for symbol in project
            .symbols()
            .filter(|s| s.page == page && driven.contains(&s.id))
        {
            for (g, group) in symbol.pass_through.iter().enumerate() {
                let (Some(&pa), Some(&pb)) = (group.first(), group.last()) else {
                    continue;
                };
                let (Some(a), Some(b)) = (symbol.pin_position(pa), symbol.pin_position(pb)) else {
                    continue;
                };
                let closed = poles.is_closed(symbol.id, g);
                let level = self.blade((symbol.id, g), f32::from(u8::from(closed)), dt);
                moving |= level != f32::from(u8::from(closed));
                let (pa, pb) = (
                    to_screen(egui::vec2(a.x as f32, a.y as f32)),
                    to_screen(egui::vec2(b.x as f32, b.y as f32)),
                );
                let swing = (1.0 - level) * BLADE_OPEN;
                let (sin, cos) = swing.sin_cos();
                let d = pb - pa;
                let tip = pa + egui::vec2(d.x * cos - d.y * sin, d.x * sin + d.y * cos);
                let tint = if closed {
                    tokens::SUCCESS
                } else {
                    tokens::TEXT_MUTED
                };
                painter.line_segment([pa, tip], egui::Stroke::new(2.5, tint));
                painter.circle_filled(pa, 2.5, tint);
            }
        }
        moving
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Pin, Placement, Point};

    fn lamp_at(x: f64, y: f64) -> SymbolInstance {
        let library = Library::builtin().unwrap();
        let def = library.get("iec/signalling/lamp").unwrap();
        let mut s = SymbolInstance {
            id: SymbolId(1),
            page: PageId(1),
            symbol: "iec/signalling/lamp".into(),
            placement: Placement::at(Point::new(x, y)),
            role: Default::default(),
            pins: def
                .connections
                .iter()
                .map(|c| Pin::new(c.id.clone(), c.at[0], c.at[1]))
                .collect(),
            bridges: Vec::new(),
            pass_through: Vec::new(),
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
    fn blades_ease_between_open_and_closed() {
        let mut anim = Anim::default();
        let key = (SymbolId(1), 0);
        assert_eq!(anim.blade(key, 0.0, 0.016), 0.0);
        let mid = anim.blade(key, 1.0, 0.016);
        assert!(mid > 0.0 && mid < 1.0);
        assert_eq!(anim.blade(key, 1.0, 1.0e3), 1.0);
    }

    #[test]
    fn timer_clock_restarts_when_the_coil_changes() {
        let mut anim = Anim::default();
        let coil = SymbolId(2);
        // First seen already on: no known start, so no progress to show.
        assert!(anim.timer_clock(coil, true, 0.5).is_infinite());
        anim.timer_clock(coil, false, 0.25);
        assert_eq!(anim.timer_clock(coil, false, 0.25), 0.5);
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
}
