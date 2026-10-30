//! The widgets of the Controller card (lane C3): the host draws a pendant, radio or joystick box
//! from the panel plan of the profile (`novo_controller::panel`), so a controller made by the
//! import wizard looks like the device with no code per controller.
//!
//! Where the user finds it: Simulate > Controller panel (command `sim.controller`), then add a
//! controller. Two-step push buttons stand as a stack per direction: press the upper zone for
//! step 1 (slow), slide down or press the lower zone for step 2 (fast). The joystick is a pad
//! with a dot per detent: drag the knob to a detent, let go and it springs back. The mushroom
//! latches, its Reset twists it free. **Sticky** keeps what you click held, so several
//! motions can run together.

use eframe::egui::{self, Align2, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use novo_controller::panel::{
    ButtonRole, Direction, PanelPlan, StickAxis, Template, Widget, zone_at,
};
use novo_controller::{ControlKind, Profile, step_contact};

use super::controller_card::{KeyAction, KeyBinding, key_name};
use super::controller_live::{ControllerLive, HoldBy};

/// Width of one direction stack.
const STACK_W: f32 = 112.0;
/// Height of one step zone.
const ZONE_H: f32 = 30.0;
/// Side of a joystick pad.
const PAD: f32 = 150.0;
/// Radius of the emergency stop mushroom.
const MUSHROOM: f32 = 26.0;

/// What a drawing pass needs about the controller it draws.
struct Frame<'a> {
    id: &'a str,
    profile: &'a Profile,
    map: &'a [KeyBinding],
    sticky: bool,
}

/// Draws the plan of the selected controller.
pub(super) fn draw_panel(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    id: &str,
    profile: &Profile,
    plan: &PanelPlan,
    map: &[KeyBinding],
) {
    let frame = Frame {
        id,
        profile,
        map,
        sticky: ctl.sticky,
    };
    let p = crate::theme::ui_palette();
    for section in &plan.sections {
        if plan.template != Template::List || plan.sections.len() > 1 {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(section.title)
                    .small()
                    .color(p.text_secondary),
            );
        }
        for widget in &section.widgets {
            ui.add_space(2.0);
            draw_widget(ui, ctl, &frame, widget);
        }
    }
}

fn hint(map: &[KeyBinding], control: &str) -> String {
    let keys: Vec<String> = map
        .iter()
        .filter(|b| b.control == control)
        .map(|b| key_name(b.key))
        .collect();
    keys.join(" ")
}

fn title(ui: &mut egui::Ui, frame: &Frame, control: &str, label: &str) {
    let keys = hint(frame.map, control);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(label).strong());
        if !keys.is_empty() {
            ui.weak(format!("[{keys}]"));
        }
    });
}

fn at_of(ctl: &ControllerLive, id: &str, control: &str) -> String {
    ctl.set
        .instance(id)
        .and_then(|i| i.controller.effective_position(control))
        .unwrap_or_default()
}

fn draw_widget(ui: &mut egui::Ui, ctl: &mut ControllerLive, frame: &Frame, widget: &Widget) {
    match widget {
        Widget::EStop { control, label } => estop(ui, ctl, frame, control, label),
        Widget::Latch {
            control,
            label,
            key,
        } => latch(ui, ctl, frame, control, label, *key),
        Widget::Button {
            control,
            label,
            position,
            role,
        } => button(ui, ctl, frame, control, label, position, *role),
        Widget::Motion {
            control,
            label,
            spring,
            directions,
        } => motion(ui, ctl, frame, control, label, *spring, directions),
        Widget::Stick { label, axes, .. } => stick(ui, ctl, frame, label, axes),
        Widget::Selector {
            control,
            label,
            positions,
        } => selector(ui, ctl, frame, control, label, positions),
    }
}

/// A word for a step: slow and fast on a two-step button.
fn step_word(step: u8, steps: usize) -> String {
    match (steps, step) {
        (2, 1) => "slow".to_owned(),
        (2, 2) => "fast".to_owned(),
        _ => format!("step {step}"),
    }
}

fn arrow(name: &str) -> &'static str {
    match name {
        "up" | "fwd" | "forward" | "north" | "raise" => "↑",
        "down" | "rev" | "reverse" | "back" | "backward" | "south" | "lower" => "↓",
        "left" | "west" => "←",
        "right" | "east" => "→",
        _ => "•",
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// A motion: one stack of step zones per direction.
fn motion(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    control: &str,
    label: &str,
    spring: bool,
    directions: &[Direction],
) {
    title(ui, frame, control, label);
    let at = at_of(ctl, frame.id, control);
    ui.horizontal_top(|ui| {
        for dir in directions {
            stack(ui, ctl, frame, control, dir, spring, &at);
        }
    });
}

fn stack(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    control: &str,
    dir: &Direction,
    spring: bool,
    at: &str,
) {
    let p = crate::theme::ui_palette();
    let n = dir.steps.len();
    let header = 18.0;
    let size = Vec2::new(STACK_W, header + n as f32 * ZONE_H);
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let zones = Rect::from_min_max(
        Pos2::new(rect.left(), rect.top() + header),
        rect.right_bottom(),
    );
    let includes = frame.profile.controller.step2_includes_step1;
    let at_step = dir
        .steps
        .iter()
        .find(|(_, position)| position == at)
        .map(|(step, _)| *step)
        .unwrap_or(0);

    // Which zone the pointer means while it is down on the stack.
    let down = response.is_pointer_button_down_on();
    let pointed = ui.input(|i| i.pointer.interact_pos()).map(|pos| {
        let frac = (pos.y - zones.top()) / zones.height().max(1.0);
        &dir.steps[zone_at(frac, n)].1
    });
    if spring && !frame.sticky {
        match (down, pointed) {
            (true, Some(position)) => {
                ctl.hold(frame.id, control, HoldBy::Mouse, Some(position));
            }
            _ if dir.steps.iter().any(|(_, position)| position == at) => {
                ctl.hold(frame.id, control, HoldBy::Mouse, None);
            }
            _ => {}
        }
    } else if (response.clicked() || response.drag_started())
        && let Some(position) = pointed
    {
        {
            let rest = frame
                .profile
                .control(control)
                .map(|c| c.rest_position())
                .unwrap_or_default();
            let target = if position == at {
                rest.clone()
            } else {
                position.clone()
            };
            if spring {
                // Sticky: the hold stays until the same zone is clicked again.
                let held = (target != rest).then_some(target.as_str());
                ctl.hold(frame.id, control, HoldBy::Mouse, held);
            } else {
                ctl.act(
                    frame.id,
                    novo_controller::Action::SetPosition {
                        control: control.to_owned(),
                        position: target,
                    },
                );
            }
        }
    }

    let painter = ui.painter();
    painter.text(
        Pos2::new(rect.left() + 2.0, rect.top() + header / 2.0),
        Align2::LEFT_CENTER,
        format!("{} {}", arrow(&dir.name), capitalise(&dir.name)),
        FontId::proportional(12.0),
        p.text_secondary,
    );
    for (i, (step, position)) in dir.steps.iter().enumerate() {
        let cell = Rect::from_min_size(
            Pos2::new(zones.left(), zones.top() + i as f32 * ZONE_H),
            Vec2::new(zones.width(), ZONE_H),
        )
        .shrink2(Vec2::new(0.0, 1.0));
        let held = at == position;
        // A closed contact below the held step: a real two-step button keeps step 1 closed.
        let closed = !held && at_step > 0 && step_contact(at_step, *step, includes);
        let fill = if held {
            p.accent
        } else if closed {
            p.accent_wash
        } else if response.hovered() {
            p.hover
        } else {
            p.control
        };
        let ink = if held { p.on_accent } else { p.text_primary };
        painter.rect_filled(cell, 5.0, fill);
        painter.rect_stroke(
            cell,
            5.0,
            Stroke::new(1.0, p.border_control),
            StrokeKind::Inside,
        );
        // Second and further steps get inner ticks: the deeper the press the more ticks.
        for t in 0..*step {
            let x = cell.left() + 8.0 + t as f32 * 5.0;
            painter.line_segment(
                [
                    Pos2::new(x, cell.center().y - 5.0),
                    Pos2::new(x, cell.center().y + 5.0),
                ],
                Stroke::new(1.5, ink),
            );
        }
        painter.text(
            Pos2::new(
                cell.left() + 8.0 + *step as f32 * 5.0 + 6.0,
                cell.center().y,
            ),
            Align2::LEFT_CENTER,
            step_word(*step, n),
            FontId::proportional(12.0),
            ink,
        );
    }
    response.on_hover_text(if spring && !frame.sticky {
        "Hold. Press the top zone for step 1, slide down for step 2"
    } else if spring {
        "Sticky: click to hold, click again to let go"
    } else {
        "Click to set, click again for rest"
    });
}

/// A joystick pad of one or two axes with a dot per detent.
fn stick(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    label: &str,
    axes: &[StickAxis],
) {
    let p = crate::theme::ui_palette();
    let Some(x_axis) = axes.first() else {
        return;
    };
    let y_axis = axes.get(1);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new(label).strong());
        let keys: Vec<String> = axes
            .iter()
            .map(|a| hint(frame.map, &a.control))
            .filter(|k| !k.is_empty())
            .collect();
        if !keys.is_empty() {
            ui.weak(format!("[{}]", keys.join(" ")));
        }
    });
    // One axis stands alone as a slider-like strip, left-right.
    let size = if y_axis.is_some() {
        Vec2::splat(PAD)
    } else {
        Vec2::new(PAD, 44.0)
    };
    let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let margin = 16.0;
    let usable = Vec2::new(rect.width() / 2.0 - margin, rect.height() / 2.0 - margin);

    let spring = x_axis.spring && !frame.sticky;
    let down = response.is_pointer_button_down_on();
    if down && let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
        let vx = (pos.x - rect.center().x) / usable.x.max(1.0);
        let vy = -(pos.y - rect.center().y) / usable.y.max(1.0);
        for (axis, value) in [(Some(x_axis), vx), (y_axis, vy)] {
            let Some(axis) = axis else { continue };
            let detent = axis.detent_at(value);
            if axis.spring {
                ctl.hold(frame.id, &axis.control, HoldBy::Mouse, Some(detent));
            } else if at_of(ctl, frame.id, &axis.control) != detent {
                ctl.act(
                    frame.id,
                    novo_controller::Action::SetPosition {
                        control: axis.control.clone(),
                        position: detent.to_owned(),
                    },
                );
            }
        }
    } else if spring {
        for axis in axes {
            ctl.hold(frame.id, &axis.control, HoldBy::Mouse, None);
        }
    }

    let painter = ui.painter();
    painter.rect_filled(rect, 8.0, p.sunken);
    painter.rect_stroke(
        rect,
        8.0,
        Stroke::new(1.0, p.border_control),
        StrokeKind::Inside,
    );
    let centre = rect.center();
    let line = Stroke::new(1.0, p.border_subtle);
    painter.line_segment(
        [
            Pos2::new(rect.left() + 6.0, centre.y),
            Pos2::new(rect.right() - 6.0, centre.y),
        ],
        line,
    );
    if y_axis.is_some() {
        painter.line_segment(
            [
                Pos2::new(centre.x, rect.top() + 6.0),
                Pos2::new(centre.x, rect.bottom() - 6.0),
            ],
            line,
        );
    }
    // A dot for each detent on each axis line, so the person sees where the stops are.
    let dot = |pos: Pos2| painter.circle_filled(pos, 2.5, p.text_tertiary);
    for detent in &x_axis.detents {
        dot(Pos2::new(
            centre.x + x_axis.offset_of(detent) * usable.x,
            centre.y,
        ));
    }
    if let Some(y_axis) = y_axis {
        for detent in &y_axis.detents {
            dot(Pos2::new(
                centre.x,
                centre.y - y_axis.offset_of(detent) * usable.y,
            ));
        }
    }
    // The ends are named, so a pad of "left/right, up/down" reads without a legend.
    let end_name = |axis: &StickAxis, last: bool| {
        let name = if last {
            axis.detents.last()
        } else {
            axis.detents.first()
        };
        name.map(|n| super::controller_card::position_label(n))
            .unwrap_or_default()
    };
    let small = FontId::proportional(10.0);
    painter.text(
        Pos2::new(rect.left() + 4.0, rect.top() + 3.0),
        Align2::LEFT_TOP,
        format!("← {}", end_name(x_axis, false)),
        small.clone(),
        p.text_secondary,
    );
    painter.text(
        Pos2::new(rect.right() - 4.0, rect.top() + 3.0),
        Align2::RIGHT_TOP,
        format!("{} →", end_name(x_axis, true)),
        small.clone(),
        p.text_secondary,
    );
    if let Some(y_axis) = y_axis {
        painter.text(
            Pos2::new(rect.left() + 4.0, rect.bottom() - 3.0),
            Align2::LEFT_BOTTOM,
            format!("↓ {}", end_name(y_axis, false)),
            small.clone(),
            p.text_secondary,
        );
        painter.text(
            Pos2::new(rect.right() - 4.0, rect.bottom() - 3.0),
            Align2::RIGHT_BOTTOM,
            format!("↑ {}", end_name(y_axis, true)),
            small,
            p.text_secondary,
        );
    }
    let ax = x_axis.offset_of(&at_of(ctl, frame.id, &x_axis.control));
    let ay = y_axis
        .map(|a| a.offset_of(&at_of(ctl, frame.id, &a.control)))
        .unwrap_or(0.0);
    let knob = Pos2::new(centre.x + ax * usable.x, centre.y - ay * usable.y);
    let moved = ax != 0.0 || ay != 0.0;
    painter.circle_filled(knob, 11.0, if moved { p.accent } else { p.control });
    painter.circle_stroke(knob, 11.0, Stroke::new(1.5, p.border_strong));
    response.on_hover_text(if spring {
        "Drag the knob to a detent. It springs back when you let go"
    } else {
        "Drag the knob to a detent. It stays where you leave it"
    });
}

/// A rotary that stays where it is.
fn selector(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    control: &str,
    label: &str,
    positions: &[String],
) {
    let p = crate::theme::ui_palette();
    title(ui, frame, control, label);
    let at = at_of(ctl, frame.id, control);
    let n = positions.len().max(1);
    let index = positions.iter().position(|q| *q == at).unwrap_or(0);
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
        let painter = ui.painter();
        let centre = rect.center();
        painter.circle_filled(centre, 22.0, p.control);
        painter.circle_stroke(centre, 22.0, Stroke::new(1.5, p.border_control));
        // Positions spread over an arc of 270 degrees, the first at lower left.
        let angle = |i: usize| {
            let t = if n > 1 {
                i as f32 / (n - 1) as f32
            } else {
                0.5
            };
            (-225.0f32 + t * 270.0).to_radians()
        };
        for i in 0..n {
            let a = angle(i);
            let tick = centre + Vec2::new(a.cos(), -a.sin()) * 18.0;
            painter.circle_filled(tick, 1.8, p.text_tertiary);
        }
        let a = angle(index);
        painter.line_segment(
            [centre, centre + Vec2::new(a.cos(), -a.sin()) * 18.0],
            Stroke::new(3.0, p.accent),
        );
        if response.clicked() {
            let next = &positions[(index + 1) % n];
            super::controller_card::press_outcome(ctl, frame.id, control, Some(next));
        }
        response.on_hover_text("Click to turn to the next position");
        ui.vertical(|ui| {
            for q in positions {
                if ui
                    .selectable_label(at == *q, super::controller_card::position_label(q))
                    .clicked()
                {
                    super::controller_card::press_outcome(ctl, frame.id, control, Some(q));
                }
            }
        });
    });
}

/// The latching mushroom, with its twist reset.
fn estop(ui: &mut egui::Ui, ctl: &mut ControllerLive, frame: &Frame, control: &str, label: &str) {
    let p = crate::theme::ui_palette();
    let at = at_of(ctl, frame.id, control);
    let rest = frame
        .profile
        .control(control)
        .map(|c| c.rest_position())
        .unwrap_or_default();
    let pressed = at != rest;
    ui.horizontal(|ui| {
        let size = Vec2::splat(MUSHROOM * 2.0 + 6.0);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let painter = ui.painter();
        let centre = rect.center();
        // A yellow collar and a red cap; pressed it sinks and darkens.
        painter.circle_filled(centre, MUSHROOM + 3.0, p.warning);
        let cap = if pressed {
            p.error.gamma_multiply(0.6)
        } else {
            p.error
        };
        painter.circle_filled(
            centre
                + if pressed {
                    Vec2::new(0.0, 2.0)
                } else {
                    Vec2::ZERO
                },
            MUSHROOM - 3.0,
            cap,
        );
        painter.circle_stroke(centre, MUSHROOM - 3.0, Stroke::new(1.5, p.border_strong));
        if response.clicked() && !pressed {
            super::controller_card::press_outcome(ctl, frame.id, control, None);
        }
        response.on_hover_text(if pressed {
            "Pressed and latched. Twist to release"
        } else {
            "Press the emergency stop"
        });
        ui.vertical(|ui| {
            let keys = hint(frame.map, control);
            ui.label(egui::RichText::new(label).strong());
            if !keys.is_empty() {
                ui.weak(format!("[{keys}]"));
            }
            if pressed {
                ui.label(egui::RichText::new("Pressed").color(p.error));
                if ui
                    .small_button("Reset")
                    .on_hover_text("Twist to release")
                    .clicked()
                {
                    ctl.act(
                        frame.id,
                        novo_controller::Action::Reset {
                            control: control.to_owned(),
                        },
                    );
                }
            }
        });
    });
}

/// Main/start, a light or a key switch.
fn latch(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    control: &str,
    label: &str,
    key: bool,
) {
    let p = crate::theme::ui_palette();
    let at = at_of(ctl, frame.id, control);
    let rest = frame
        .profile
        .control(control)
        .map(|c| c.rest_position())
        .unwrap_or_default();
    let on = at != rest;
    ui.horizontal(|ui| {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(54.0, 26.0), Sense::click());
        let painter = ui.painter();
        painter.rect_filled(rect, 13.0, if on { p.success } else { p.control });
        painter.rect_stroke(
            rect,
            13.0,
            Stroke::new(1.0, p.border_control),
            StrokeKind::Inside,
        );
        let knob_x = if on {
            rect.right() - 13.0
        } else {
            rect.left() + 13.0
        };
        painter.circle_filled(
            Pos2::new(knob_x, rect.center().y),
            9.0,
            if on { p.on_accent } else { p.text_secondary },
        );
        if response.clicked() {
            super::controller_card::press_outcome(ctl, frame.id, control, None);
        }
        response.on_hover_text(if key {
            "Key switch: click to turn"
        } else {
            "Click to switch"
        });
        ui.label(egui::RichText::new(label).strong());
        ui.weak(if on { "on" } else { "off" });
        let keys = hint(frame.map, control);
        if !keys.is_empty() {
            ui.weak(format!("[{keys}]"));
        }
    });
}

/// A push button held while the pointer is down: stop, horn, dead-man.
fn button(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    frame: &Frame,
    control: &str,
    label: &str,
    position: &str,
    role: ButtonRole,
) {
    let p = crate::theme::ui_palette();
    let at = at_of(ctl, frame.id, control);
    let held = at == position;
    let icon = match role {
        ButtonRole::Horn => "♪ ",
        ButtonRole::Stop => "■ ",
        ButtonRole::Deadman => "✋ ",
        ButtonRole::Plain => "",
    };
    ui.horizontal(|ui| {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(104.0, 28.0), Sense::click_and_drag());
        let fill = match (held, role) {
            (true, ButtonRole::Stop) => p.error,
            (true, _) => p.accent,
            _ if response.hovered() => p.hover,
            _ => p.control,
        };
        let painter = ui.painter();
        painter.rect_filled(rect, 14.0, fill);
        painter.rect_stroke(
            rect,
            14.0,
            Stroke::new(1.0, p.border_control),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            format!("{icon}{label}"),
            FontId::proportional(12.0),
            if held { p.on_accent } else { p.text_primary },
        );
        let down = response.is_pointer_button_down_on();
        if frame.sticky {
            if response.clicked() {
                ctl.hold(
                    frame.id,
                    control,
                    HoldBy::Mouse,
                    (!held).then_some(position),
                );
            }
        } else {
            ctl.hold(frame.id, control, HoldBy::Mouse, down.then_some(position));
        }
        response.on_hover_text(if role == ButtonRole::Deadman {
            "Hold the enabling bar while you move"
        } else {
            "Hold"
        });
        let keys = hint(frame.map, control);
        if !keys.is_empty() {
            ui.weak(format!("[{keys}]"));
        }
    });
}

/// The rows of the key map table: the keys, what they do. Shown under the Keyboard switch.
pub(super) fn key_rows(profile: &Profile, map: &[KeyBinding]) -> Vec<(String, String)> {
    let label = |control: &str| {
        profile
            .control(control)
            .map(|c| {
                if c.label.is_empty() {
                    c.id.clone()
                } else {
                    c.label.clone()
                }
            })
            .unwrap_or_else(|| control.to_owned())
    };
    map.iter()
        .map(|b| {
            let what = match &b.action {
                KeyAction::Hold(steps) => {
                    let name = steps
                        .first()
                        .map(|(_, p)| {
                            novo_controller::profile::split_group_step(p)
                                .map_or(p.as_str(), |(g, _)| g)
                        })
                        .unwrap_or_default()
                        .to_owned();
                    let kind = profile.control(&b.control).map(|c| c.kind);
                    let more = if steps.len() > 1 {
                        "; with Shift step 2"
                    } else {
                        ""
                    };
                    if kind == Some(ControlKind::Button) || kind == Some(ControlKind::Deadman) {
                        format!("Hold {}", label(&b.control))
                    } else {
                        format!("{} {name}{more}", label(&b.control))
                    }
                }
                KeyAction::Toggle => format!("Switch {}", label(&b.control)),
                KeyAction::Press => format!("Press {}", label(&b.control)),
                KeyAction::Reset => format!("Reset {}", label(&b.control)),
            };
            (key_name(b.key), what)
        })
        .collect()
}

/// The key map as a small table.
pub(super) fn key_map_table(ui: &mut egui::Ui, profile: &Profile, map: &[KeyBinding]) {
    let rows = key_rows(profile, map);
    if rows.is_empty() {
        ui.weak("This controller has no keys.");
        return;
    }
    egui::Grid::new("controller_key_map")
        .num_columns(2)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for (key, what) in rows {
                ui.label(egui::RichText::new(key).monospace().strong());
                ui.label(what);
                ui.end_row();
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::controller_card::key_map;

    const PENDANT: &str = r#"
[controller]
id = "pendant"
name = "Pendant"
kind = "pendant"

[[control]]
id = "estop"
kind = "estop"
[[control]]
id = "start"
kind = "latch"
[[control]]
id = "hoist"
label = "Hoist"
kind = "rocker2"
positions = ["off", "up1", "up2", "down1", "down2"]
[[control]]
id = "mode"
kind = "selector"
positions = ["fast", "slow"]

[[signal]]
name = "UP_SLOW"
control = "hoist"
group = "up"
step = 1
ungated = true
"#;

    const STICK: &str = r#"
[controller]
id = "stick"
name = "Joystick box"
kind = "joystick"

[[control]]
id = "bridge"
label = "Bridge"
kind = "axis"
positions = ["center", "left1", "left2", "right1", "right2"]
[[control]]
id = "hoist"
label = "Hoist"
kind = "axis"
positions = ["center", "up1", "up2", "down1", "down2"]

[[signal]]
name = "UP"
control = "hoist"
when = ["up1", "up2"]
"#;

    struct Rig {
        ctx: egui::Context,
        ctl: ControllerLive,
        id: String,
        profile: Profile,
        end: Pos2,
    }

    impl Rig {
        fn new(text: &str) -> Rig {
            let profile = Profile::load(text).expect("profile loads");
            let mut ctl = ControllerLive::default();
            let id = ctl.add_instance(&profile).expect("instance");
            Rig {
                ctx: egui::Context::default(),
                ctl,
                id,
                profile,
                end: Pos2::ZERO,
            }
        }

        /// One frame with these pointer events; returns where the last widget ends.
        fn frame(&mut self, events: Vec<egui::Event>) {
            let plan = novo_controller::panel::plan(&self.profile);
            let map = key_map(&self.profile);
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 900.0))),
                events,
                ..Default::default()
            };
            let (ctl, id, profile) = (&mut self.ctl, self.id.clone(), &self.profile);
            let mut end = Pos2::ZERO;
            let mut out = self.ctx.run_ui(input, |ui| {
                draw_panel(ui, ctl, &id, profile, &plan, &map);
                end = ui.min_rect().max;
            });
            out.textures_delta.clear();
            self.end = end;
            self.ctl.apply_holds();
        }

        fn at(&self, control: &str) -> String {
            self.ctl
                .set
                .instance(&self.id)
                .and_then(|i| i.controller.position(control).map(str::to_owned))
                .unwrap()
        }
    }

    fn press(pos: Pos2) -> Vec<egui::Event> {
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: Default::default(),
            },
        ]
    }

    fn release(pos: Pos2) -> Vec<egui::Event> {
        vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        }]
    }

    #[test]
    fn a_pendant_draws_every_control() {
        let mut rig = Rig::new(PENDANT);
        rig.frame(vec![]);
        rig.frame(vec![]);
        assert!(
            rig.end.y > 150.0,
            "the panel has some height: {:?}",
            rig.end
        );
    }

    #[test]
    fn pressing_a_two_step_stack_holds_step_one_then_slides_to_step_two() {
        let mut rig = Rig::new(PENDANT);
        rig.frame(vec![]);
        rig.frame(vec![]);
        // The selector is the last widget; the hoist stacks are above it, so find them by
        // scanning a column of pointer positions on the left stack for the one that holds up1.
        let mut found = None;
        let mut y = 20.0;
        while y < rig.end.y {
            let pos = Pos2::new(40.0, y);
            rig.frame(press(pos));
            let at = rig.at("hoist");
            rig.frame(release(pos));
            rig.frame(vec![]);
            assert_eq!(rig.at("hoist"), "off", "letting go springs back");
            if at == "up1" && found.is_none() {
                found = Some(y);
            }
            y += 4.0;
        }
        let top = found.expect("some point on the upper zone holds step 1");
        // The first hit may be in the header; 18 + 1.5 zones below it is inside the lower zone.
        let pos = Pos2::new(40.0, top);
        rig.frame(press(pos));
        assert_eq!(rig.at("hoist"), "up1");
        rig.frame(vec![egui::Event::PointerMoved(Pos2::new(
            40.0,
            top + 18.0 + ZONE_H * 1.5,
        ))]);
        assert_eq!(rig.at("hoist"), "up2", "sliding down is step 2");
        rig.frame(release(Pos2::new(40.0, top + 18.0 + ZONE_H * 1.5)));
        rig.frame(vec![]);
        assert_eq!(rig.at("hoist"), "off");
    }

    #[test]
    fn sticky_keeps_the_press_until_it_is_clicked_again() {
        let mut rig = Rig::new(PENDANT);
        rig.ctl.sticky = true;
        rig.frame(vec![]);
        rig.frame(vec![]);
        let mut hit = None;
        let mut y = 20.0;
        while y < rig.end.y && hit.is_none() {
            let pos = Pos2::new(40.0, y);
            rig.frame(press(pos));
            rig.frame(release(pos));
            rig.frame(vec![]);
            if rig.at("hoist") == "up1" {
                hit = Some(pos);
            }
            y += 4.0;
        }
        let pos = hit.expect("a click holds step 1");
        rig.frame(vec![]);
        assert_eq!(rig.at("hoist"), "up1", "still held after the pointer left");
        rig.frame(press(pos));
        rig.frame(release(pos));
        rig.frame(vec![]);
        assert_eq!(rig.at("hoist"), "off", "a second click lets go");
    }

    #[test]
    fn dragging_the_joystick_knob_selects_detents_and_it_springs_back() {
        let mut rig = Rig::new(STICK);
        rig.frame(vec![]);
        rig.frame(vec![]);
        // The pad is the last widget: PAD square ending at `end`.
        let pad = Rect::from_min_max(rig.end - Vec2::splat(PAD), rig.end);
        let c = pad.center();
        let reach = PAD / 2.0 - 16.0;
        rig.frame(press(Pos2::new(c.x + reach, c.y - reach)));
        assert_eq!(rig.at("bridge"), "right2");
        assert_eq!(rig.at("hoist"), "up2");
        rig.frame(vec![egui::Event::PointerMoved(Pos2::new(
            c.x - reach * 0.5,
            c.y,
        ))]);
        assert_eq!(rig.at("bridge"), "left1");
        assert_eq!(rig.at("hoist"), "center");
        rig.frame(release(Pos2::new(c.x - reach * 0.5, c.y)));
        rig.frame(vec![]);
        assert_eq!(rig.at("bridge"), "center", "springs back");
    }

    #[test]
    fn the_key_map_table_names_each_key() {
        let profile = Profile::load(PENDANT).unwrap();
        let rows = key_rows(&profile, &key_map(&profile));
        assert!(
            rows.iter()
                .any(|(k, w)| k == "Space" && w.contains("Switch"))
        );
        assert!(rows.iter().any(|(k, w)| k == "↑" && w.contains("Hoist up")));
        assert!(
            rows.iter()
                .any(|(_, w)| w.contains("Hoist up") && w.contains("Shift")),
            "{rows:?}"
        );
        assert!(rows.iter().any(|(k, w)| k == "X" && w.contains("Press")));
    }
}
