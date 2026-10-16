//! "Show me how": plays a step's demonstration (`novo_learn::Demo`) over the
//! real screen. A pointer glides to the control or the spot, key caps show
//! the person's own keys, typed text appears letter by letter, a click
//! ripples, a faint copy of the part lands and a wire is dragged.
//!
//! It never touches the drawing and never takes input; it repeats a few
//! times and stops when the step changes. Words come from `ui.toml` and the
//! lesson; this file only turns a [`Frame`] into pictures.

use eframe::egui::{self, Color32, Pos2, Stroke};
use novo_learn::demo::{Frame, KeyCap};
use novo_learn::lesson::PlaceArgs;
use novo_learn::{Demo, Target};
use novo_symbols::{Graphic, Library, Pack};
use novo_theme::tokens;

use super::NovoApp;

/// How many times the demonstration plays before it stops by itself.
const PLAYS: f64 = 3.0;

/// A demonstration that is playing.
pub(crate) struct DemoRun {
    pub(super) demo: Demo,
    /// The lesson and step it belongs to.
    key: (String, String),
    /// The animation time it began, set on the first frame it is drawn.
    pub(super) started: Option<f64>,
    /// Played out: only the start and end markers and the wire between
    /// them stay, until the step changes or the person stops it.
    settled: bool,
}

impl DemoRun {
    /// The moment `now` falls on, in seconds from the start of a play; the
    /// last picture, held, when motion is reduced.
    fn time(&self, now: f64, calm: bool) -> Option<f32> {
        if calm {
            return Some(self.demo.duration());
        }
        let period = f64::from(self.demo.duration() + novo_learn::demo::END_HOLD_SECS);
        let elapsed = now - self.started?;
        (elapsed < period * PLAYS).then(|| (elapsed % period) as f32)
    }
}

/// What a frame looks like on screen.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Scene {
    pub cursor: Option<Pos2>,
    pub ripple: Option<(Pos2, f32)>,
    /// Rings a click makes, and whether it is the right button.
    pub rings: u8,
    pub right: bool,
    /// Numbered start and end markers: where, which number, end or start.
    pub markers: Vec<(Pos2, u8, bool)>,
    pub keys: Vec<(String, bool)>,
    pub typed: Option<String>,
    pub ghost: Option<(Pos2, PlaceArgs, f32)>,
    pub drag: Option<(Pos2, Pos2)>,
}

impl NovoApp {
    /// Starts the demonstration of the current step. False when the step
    /// has nothing to demonstrate.
    pub(super) fn learn_start_demo(&mut self) -> bool {
        let Some(run) = self.learn.run.as_ref() else {
            return false;
        };
        let Some(step) = run.step() else {
            return false;
        };
        let demo = Demo::for_step(step);
        if demo.is_empty() {
            return false;
        }
        let key = (run.lesson().id.clone(), step.id.clone());
        self.learn.demo = Some(DemoRun {
            demo,
            key,
            started: None,
            settled: false,
        });
        true
    }

    /// The lines of the how-to list for the card, with the one being done.
    pub(super) fn learn_how_lines(&self) -> Vec<(String, bool)> {
        let Some(run) = self.learn.demo.as_ref() else {
            return Vec::new();
        };
        let active = self.learn.demo_caption;
        let lang = self.learn.lang;
        run.demo
            .captions()
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let arg = match c.kind {
                    "demo_press" | "demo_hold" => self.demo_key_text(c.arg(lang)),
                    _ => c.arg(lang).to_owned(),
                };
                let text = self.learn.say_with(c.kind, &[("arg", &arg)]);
                (format!("{}. {text}", i + 1), active == Some(i))
            })
            .collect()
    }

    /// The person's own key for a command.
    fn demo_key_text(&self, command: &str) -> String {
        self.wb
            .keymap
            .shortcut_text(command)
            .unwrap_or_else(|| "(no key)".to_owned())
    }

    /// Plays the demonstration on top of the canvas.
    pub(super) fn learn_demo_overlay(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let current = self
            .learn
            .run
            .as_ref()
            .and_then(|r| r.step().map(|s| (r.lesson().id.clone(), s.id.clone())));
        let stale = match (&self.learn.demo, &current) {
            (Some(run), Some(now)) => &run.key != now,
            _ => true,
        };
        if stale || !self.learn.card_on() {
            self.learn.demo = None;
            self.learn.demo_caption = None;
            return;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.learn_stop_demo();
            return;
        }
        let calm = self.settings.reduce_motion;
        let now = ctx.input(|i| i.time);
        let Some(run) = self.learn.demo.as_mut() else {
            return;
        };
        run.started.get_or_insert(now);
        let t = match run.time(now, calm) {
            Some(t) => t,
            None if !run.demo.wire_ends().is_empty() => {
                run.settled = true;
                run.demo.duration()
            }
            None => {
                self.learn_stop_demo();
                return;
            }
        };
        let mut frame = run.demo.frame(t);
        if run.settled {
            frame.cursor = None;
            frame.keys.clear();
            frame.typed = None;
            frame.ripple = None;
            frame.ghost = None;
            frame.caption = None;
        }
        let settled = run.settled;
        self.learn.demo_caption = frame.caption;
        let start = self.learn.card_pos.unwrap_or(canvas.center());
        let scene = self.demo_scene(&frame, canvas, start);
        self.paint_demo(ctx, canvas, &scene);
        if !calm && !settled {
            ctx.request_repaint();
        }
    }

    /// Ends the demonstration: the pointer, the markers and the wire go.
    pub(super) fn learn_stop_demo(&mut self) {
        self.learn.demo = None;
        self.learn.demo_caption = None;
    }

    /// Where everything of a frame goes on screen.
    pub(super) fn demo_scene(&self, frame: &Frame, canvas: egui::Rect, start: Pos2) -> Scene {
        let pos = |target: &Target| self.target_rect(target, canvas).map(|r| r.center());
        let mut scene = Scene::default();
        if let Some(c) = &frame.cursor {
            let to = pos(&c.to);
            let from = c.from.as_ref().and_then(pos).unwrap_or(start);
            scene.cursor = to.map(|to| from.lerp(to, c.k));
        }
        scene.ripple = frame.ripple.zip(scene.cursor).map(|(k, at)| (at, k));
        scene.rings = frame.ripples;
        scene.right = frame.right;
        scene.markers = frame
            .markers
            .iter()
            .filter_map(|m| pos(&m.target).map(|p| (p, m.number, m.end)))
            .collect();
        scene.keys = frame
            .keys
            .iter()
            .map(|k| (self.key_cap_text(k), k.down))
            .collect();
        scene.typed = frame.typed.clone();
        if let Some((place, alpha)) = &frame.ghost {
            let target = Target {
                at: Some(novo_learn::lesson::At {
                    page: Some(place.page.clone()),
                    x: Some(place.x),
                    y: Some(place.y),
                    ..Default::default()
                }),
                ..Target::default()
            };
            if let Some(p) = pos(&target) {
                scene.ghost = Some((p, place.clone(), *alpha));
            }
        }
        if let Some((from, to, k)) = &frame.drag
            && let (Some(a), Some(b)) = (pos(from), pos(to))
        {
            scene.drag = Some((a, a.lerp(b, *k)));
        }
        scene
    }

    fn key_cap_text(&self, cap: &KeyCap) -> String {
        match (&cap.command, &cap.literal) {
            (Some(command), _) => self.demo_key_text(command),
            (None, Some(name)) => name.clone(),
            (None, None) => String::new(),
        }
    }

    fn paint_demo(&self, ctx: &egui::Context, canvas: egui::Rect, scene: &Scene) {
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("lesson_demo"),
        ));
        if let Some((at, place, alpha)) = &scene.ghost {
            self.paint_ghost(&painter, canvas, *at, place, *alpha);
        }
        if let Some((a, b)) = scene.drag {
            // The wire to draw, as a dashed ghost with a dark edge so it
            // shows on any page colour.
            let shadow = Stroke::new(5.0, Color32::from_black_alpha(140));
            let stroke = Stroke::new(2.5, tokens::ACCENT_HOVER);
            painter.extend(egui::Shape::dashed_line(&[a, b], shadow, 9.0, 6.0));
            painter.extend(egui::Shape::dashed_line(&[a, b], stroke, 9.0, 6.0));
        }
        let start = self.learn.say("demo_start");
        let end = self.learn.say("demo_end");
        for (at, number, is_end) in &scene.markers {
            let label = if *is_end { &end } else { &start };
            paint_marker(&painter, canvas, *at, *number, label, *is_end);
        }
        if let Some((at, k)) = scene.ripple {
            let color = tokens::ACCENT_HOVER.gamma_multiply((1.0 - k).clamp(0.0, 1.0));
            for ring in 0..scene.rings.max(1) {
                let lag = (k - 0.35 * f32::from(ring)).clamp(0.0, 1.0);
                if lag > 0.0 {
                    let r = 6.0 + 22.0 * lag;
                    painter.circle_stroke(at, r, Stroke::new(2.0, color));
                }
            }
            paint_mouse(&painter, at + egui::vec2(16.0, 16.0), scene.right, color);
        }
        if let Some(at) = scene.cursor {
            paint_arrow(&painter, at);
        }
        if !scene.keys.is_empty() || scene.typed.is_some() {
            let keys = scene.keys.clone();
            let typed = scene.typed.clone();
            egui::Area::new(egui::Id::new("lesson_demo_keys"))
                .order(egui::Order::Tooltip)
                .interactable(false)
                .pivot(egui::Align2::CENTER_BOTTOM)
                .fixed_pos(egui::pos2(canvas.center().x, canvas.bottom() - 90.0))
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.horizontal(|ui| {
                            for (text, down) in &keys {
                                key_cap(ui, text, *down);
                            }
                            if let Some(typed) = &typed {
                                let blink = (ctx.input(|i| i.time) * 2.0) as i64 % 2 == 0;
                                let caret = if blink { "▏" } else { " " };
                                ui.label(
                                    egui::RichText::new(format!("{typed}{caret}"))
                                        .monospace()
                                        .size(20.0)
                                        .color(tokens::ACCENT_HOVER),
                                );
                            }
                        });
                    });
                });
        }
    }

    /// A faint copy of the part that lands.
    fn paint_ghost(
        &self,
        painter: &egui::Painter,
        canvas: egui::Rect,
        at: Pos2,
        place: &PlaceArgs,
        alpha: f32,
    ) {
        // Pixels to the millimetre at this zoom.
        let scale = self
            .canvas
            .schematic_ref()
            .map(|v| v.canvas.viewport.zoom)
            .unwrap_or(1.0)
            .max(0.01);
        let ink = tokens::ACCENT_HOVER.gamma_multiply(0.75 * alpha);
        let stroke = Stroke::new(1.5, ink);
        let turn = |p: [f64; 2]| -> [f64; 2] {
            let (mut x, mut y) = (p[0], p[1]);
            for _ in 0..place.turns % 4 {
                (x, y) = (-y, x);
            }
            [x, y]
        };
        let map = |p: [f64; 2]| {
            let q = turn(p);
            Pos2::new(at.x + q[0] as f32 * scale, at.y + q[1] as f32 * scale)
        };
        let library = Library::shared(Pack::Iec);
        let Some(symbol) = library.get(&place.symbol) else {
            painter.circle_stroke(at, 14.0, stroke);
            return;
        };
        let clip = painter.with_clip_rect(canvas);
        for g in &symbol.graphics {
            match g {
                Graphic::Line { from, to, .. } => {
                    clip.line_segment([map(*from), map(*to)], stroke);
                }
                Graphic::Polyline { points, closed, .. } => {
                    clip.add(egui::Shape::Path(egui::epaint::PathShape {
                        points: points.iter().map(|p| map(*p)).collect(),
                        closed: *closed,
                        fill: Color32::TRANSPARENT,
                        stroke: stroke.into(),
                    }));
                }
                Graphic::Rect { at, size, .. } => {
                    let c = [
                        *at,
                        [at[0] + size[0], at[1]],
                        [at[0] + size[0], at[1] + size[1]],
                        [at[0], at[1] + size[1]],
                    ];
                    clip.add(egui::Shape::closed_line(
                        c.iter().map(|p| map(*p)).collect(),
                        stroke,
                    ));
                }
                Graphic::Circle { center, r, .. } => {
                    clip.circle_stroke(map(*center), *r as f32 * scale, stroke);
                }
                Graphic::Ellipse { center, rx, ry, .. } => {
                    clip.circle_stroke(map(*center), (*rx).max(*ry) as f32 * scale, stroke);
                }
                Graphic::Arc {
                    center,
                    r,
                    start,
                    end,
                    ..
                } => {
                    let pts: Vec<Pos2> = (0..=16)
                        .map(|i| {
                            let a = (start + (end - start) * f64::from(i) / 16.0).to_radians();
                            map([center[0] + r * a.cos(), center[1] + r * a.sin()])
                        })
                        .collect();
                    clip.add(egui::Shape::line(pts, stroke));
                }
                Graphic::Text { .. } => {}
            }
        }
    }
}

/// A key on a keyboard, pushed in while it is down.
fn key_cap(ui: &mut egui::Ui, text: &str, down: bool) {
    let fill = if down {
        tokens::ACCENT_SELECTION
    } else {
        ui.visuals().widgets.inactive.bg_fill
    };
    let lift = if down { 2.0 } else { 0.0 };
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.5, tokens::ACCENT_HOVER))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .outer_margin(egui::Margin {
            top: lift as i8,
            ..egui::Margin::ZERO
        })
        .show(ui, |ui| {
            ui.label(egui::RichText::new(text).monospace().size(20.0).strong());
        });
}

/// A mouse pointer with a soft shadow.
fn paint_arrow(painter: &egui::Painter, at: Pos2) {
    let head = |offset: egui::Vec2| -> Vec<Pos2> {
        [(0.0, 0.0), (0.0, 17.0), (11.0, 12.0)]
            .iter()
            .map(|(x, y)| at + offset + egui::vec2(*x, *y))
            .collect()
    };
    let tail = |offset: egui::Vec2| {
        [
            at + offset + egui::vec2(4.5, 13.0),
            at + offset + egui::vec2(8.5, 20.0),
        ]
    };
    let shadow = egui::vec2(1.5, 2.0);
    let soft = Color32::from_black_alpha(90);
    painter.add(egui::Shape::convex_polygon(
        head(shadow),
        soft,
        Stroke::NONE,
    ));
    painter.line_segment(tail(shadow), Stroke::new(3.5, soft));
    painter.line_segment(tail(egui::Vec2::ZERO), Stroke::new(4.0, Color32::BLACK));
    painter.line_segment(tail(egui::Vec2::ZERO), Stroke::new(2.0, Color32::WHITE));
    painter.add(egui::Shape::convex_polygon(
        head(egui::Vec2::ZERO),
        Color32::WHITE,
        Stroke::new(1.2, Color32::BLACK),
    ));
}

/// A ring round the point (so the pin itself stays in view), a numbered
/// badge beside it and a word: where a wire starts (1) or ends (2).
fn paint_marker(
    painter: &egui::Painter,
    canvas: egui::Rect,
    at: Pos2,
    number: u8,
    label: &str,
    end: bool,
) {
    let colour = if end {
        tokens::SUCCESS
    } else {
        tokens::ACCENT_HOVER
    };
    painter.circle_stroke(at, 11.0, Stroke::new(4.5, Color32::from_black_alpha(140)));
    painter.circle_stroke(at, 11.0, Stroke::new(2.5, colour));
    // The badge sits up and to the left; at the page edge it flips inwards.
    let flip = |v: f32, lo: f32, hi: f32| {
        if v < lo + 40.0 {
            1.0
        } else if v > hi - 40.0 {
            -1.0
        } else {
            1.0
        }
    };
    let dx = flip(at.x, canvas.left(), canvas.right());
    let badge = at + egui::vec2(-16.0 * dx, -16.0);
    painter.circle_filled(badge, 10.0, colour);
    painter.circle_stroke(badge, 10.0, Stroke::new(1.5, Color32::BLACK));
    painter.text(
        badge,
        egui::Align2::CENTER_CENTER,
        number.to_string(),
        egui::FontId::proportional(14.0),
        Color32::BLACK,
    );
    let (anchor, x) = if dx > 0.0 {
        (egui::Align2::RIGHT_CENTER, badge.x - 14.0)
    } else {
        (egui::Align2::LEFT_CENTER, badge.x + 14.0)
    };
    let text_pos = Pos2::new(x, badge.y);
    painter.text(
        text_pos + egui::vec2(1.0, 1.0),
        anchor,
        label,
        egui::FontId::proportional(14.0),
        Color32::BLACK,
    );
    painter.text(
        text_pos,
        anchor,
        label,
        egui::FontId::proportional(14.0),
        colour,
    );
}

/// A little mouse with the button that is pressed filled in.
fn paint_mouse(painter: &egui::Painter, at: Pos2, right: bool, fade: Color32) {
    let body = egui::Rect::from_center_size(at, egui::vec2(14.0, 20.0));
    painter.rect_filled(body, 6.0, Color32::from_black_alpha(160));
    let half = if right {
        egui::Rect::from_min_max(egui::pos2(at.x, body.top()), egui::pos2(body.right(), at.y))
    } else {
        egui::Rect::from_min_max(body.left_top(), egui::pos2(at.x, at.y))
    };
    painter.rect_filled(half, 3.0, fade.to_opaque());
    painter.rect_stroke(
        body,
        6.0,
        Stroke::new(1.5, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
    painter.line_segment(
        [egui::pos2(at.x, body.top()), egui::pos2(at.x, at.y)],
        Stroke::new(1.0, Color32::WHITE),
    );
}
