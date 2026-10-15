//! Device animation in the live view (WS-21): lamps glow, motors turn and
//! coils pull in, driven by the states novo-sim derives
//! (`novo_sim::device_states`). The animation values live in the workbench
//! next to the live view and are never stored in the project. Each value
//! eases towards its target, so a lamp fades and a motor spins up and down
//! instead of jumping.

use std::collections::{BTreeSet, HashMap};

use eframe::egui;
use novo_core::{PageId, Project, SymbolId, SymbolInstance};
use novo_sim::{DeviceStates, Direction, MotorRun, is_coil, is_lamp, is_motor, lamp_color};
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
        dt: f32,
    ) -> bool {
        let dt = dt.min(0.1);
        self.time += dt;
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
                if (*spin - target).abs() < 0.01 {
                    *spin = target;
                }
                *angle += *spin * TURNS * std::f32::consts::TAU * dt;
                let angle = *angle;
                let turning = *spin != 0.0;
                moving |= turning || *spin != target;
                let humming = matches!(run, MotorRun::PhaseLoss);
                let faulty = matches!(run, MotorRun::Fault);
                if !turning && !humming && !faulty {
                    continue;
                }
                moving |= humming;
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
                if turning || humming {
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
