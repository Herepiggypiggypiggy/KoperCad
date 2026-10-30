//! The gamepad of the Controller card (lane C5): a real USB gamepad or joystick holds the same
//! controls as the mouse and the keyboard, through a map per controller.
//!
//! Where the user finds it: Simulate > Controller panel, the "Gamepad" section of the card (device
//! and state, a live monitor of the sticks and buttons, "Edit map...", "Calibrate...", and the
//! switch that lets the pad drive the controller). The switch is off until asked, so plugging a
//! pad in never moves anything; with it off the section shows what the pad would do (the test).
//!
//! This file is the host side: the pad state of the live view ([`PadHost`]), the per-frame step
//! that feeds the pure mapper of `novo_controller::gamepad`, and the three views (section, map
//! editor, calibration wizard). The rules (deadzone, detents, dead-man, E-stop, device loss) are
//! in the crate and tested there.
//!
//! The pad only acts while the Controller card is open, the window has the focus and the
//! controller is the selected one; losing any of these lets go of every control at once.
//! It simulates wiring and logic: no claim about a real transmitter or its radio.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use eframe::egui;
use novo_controller::gamepad::{
    AXES, AxisMap, BUTTONS, ButtonAction, ButtonMap, CalPhase, Calibrator, DeadmanMap, GamepadMap,
    InputState, LinkLoss, PadMapper, PadOutput, detent_counts, is_axis_name, step_edges,
};
use novo_controller::{Action, ControlKind, Profile, Severity};

use super::controller_live::{ControllerLive, HoldBy};
use super::input_devices::InputService;

/// How many lines the event log keeps.
const LOG_LINES: usize = 6;

/// What the next pad movement is waiting to fill in (the "listen" buttons of the editor).
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Listen {
    Axis(usize),
    Button(usize),
    Deadman,
}

/// The calibration wizard window and its engine.
pub(super) struct Wizard {
    cal: Calibrator,
    pairs: Vec<(String, String)>,
    inputs: Vec<String>,
    message: Option<String>,
}

impl Wizard {
    #[cfg(test)]
    pub(super) fn phase(&self) -> CalPhase {
        self.cal.phase()
    }
}

/// Everything the live view keeps about the gamepad.
#[derive(Default)]
pub(crate) struct PadHost {
    service: Option<InputService>,
    /// The maps by controller instance, saved in the project.
    pub(super) maps: BTreeMap<String, GamepadMap>,
    mappers: BTreeMap<String, PadMapper>,
    /// The pad drives the selected controller. Off: the pad is only watched (test).
    pub(super) drive: bool,
    /// What the mapper asked for last frame, with the instance it was worked out for.
    pub(super) last: Option<(String, PadOutput)>,
    /// The chosen device's state last frame, as read.
    pub(super) input: InputState,
    pub(super) device_name: String,
    was_connected: Option<bool>,
    log: VecDeque<String>,
    /// Pad holds set in the controller, to let go of them.
    holding: BTreeSet<(String, String)>,
    /// The instance the mapper ran for last frame.
    active: Option<String>,
    pub(super) editor: bool,
    pub(super) wizard: Option<Wizard>,
    pub(super) listen: Option<(Listen, InputState)>,
    /// A map was edited; the project gets it when no pointer button is down.
    pub(super) edit_pending: bool,
    /// The section was drawn this frame (keeps the monitor live).
    pub(super) seen: bool,
}

impl PadHost {
    /// Replace the maps (the project was read).
    pub(super) fn set_maps(&mut self, maps: BTreeMap<String, GamepadMap>) {
        self.maps = maps;
        self.mappers.clear();
    }

    /// A controller instance was removed.
    pub(super) fn forget(&mut self, id: &str) {
        self.maps.remove(id);
        self.mappers.remove(id);
        self.holding.retain(|(i, _)| i != id);
    }

    /// Start reading devices (the first time the section is opened or the pad is switched on).
    pub(super) fn ensure_service(&mut self) {
        if self.service.is_none() {
            let mut svc = InputService::platform();
            svc.poll();
            self.service = Some(svc);
        }
    }

    fn note(&mut self, text: String) {
        if self.log.len() >= LOG_LINES {
            self.log.pop_front();
        }
        self.log.push_back(text);
    }

    /// Whether anything about the pad needs frames without other input.
    pub(super) fn wants_frames(&self) -> bool {
        self.service.is_some() && (self.seen || self.drive || self.editor || self.wizard.is_some())
    }

    /// Test hook: the service in use.
    #[cfg(test)]
    pub(super) fn set_service(&mut self, service: InputService) {
        self.service = Some(service);
    }
}

fn describe(action: &Action) -> String {
    match action {
        Action::Press { control } => format!("pressed {control}"),
        Action::Reset { control } => format!("reset {control}"),
        Action::SetPosition { control, position } => format!("{control} set to {position}"),
        Action::Release { control } => format!("released {control}"),
        Action::Link { ok } => format!("radio link {}", if *ok { "up" } else { "dropped" }),
        Action::BatteryLow { low } => format!("battery low {low}"),
        Action::Power { on } => format!("power {}", if *on { "on" } else { "off" }),
    }
}

impl ControllerLive {
    /// One frame of the pad: read the device, work out what the map asks of the selected
    /// controller and, when the pad drives it, apply that. `focused` is false while the window
    /// is in the background: the pad then counts as hands off (not as unplugged).
    pub(super) fn pad_frame(&mut self, focused: bool) {
        let Some(svc) = self.pad.service.as_mut() else {
            self.pad_release_all();
            return;
        };
        svc.poll();
        let raw = svc.state();
        self.pad.device_name = svc.chosen().map(|d| d.name.clone()).unwrap_or_default();
        self.pad.input = raw.clone();
        match (self.pad.was_connected, raw.connected) {
            (Some(false), true) => self.pad.note("pad connected".to_owned()),
            (Some(true), false) => self.pad.note("pad disconnected".to_owned()),
            _ => {}
        }
        self.pad.was_connected = Some(raw.connected);

        if self.pad.wizard.is_some() {
            self.pad_wizard_feed(&raw);
        }
        self.pad_listen(&raw);

        // While calibrating, or with the window in the background, the controller sees a pad
        // at rest and with the dead-man open.
        let hands_off = self.pad.wizard.is_some() || !focused;
        let input = if hands_off && raw.connected {
            InputState::connected()
        } else {
            raw
        };

        let target = self.selected.clone();
        if hands_off && let Some(id) = &target {
            self.pad.mappers.entry(id.clone()).or_default().hands_off();
        }
        if self.pad.active != target {
            self.pad_release_all();
            if let Some(old) = self.pad.active.take() {
                self.pad.mappers.remove(&old);
            }
            self.pad.active = target.clone();
        }
        let Some(id) = target else {
            self.pad.last = None;
            return;
        };
        let Some(map) = self.pad.maps.get(&id).cloned() else {
            self.pad.last = None;
            self.pad_release_all();
            return;
        };
        let Some(inst) = self.set.instance(&id) else {
            self.pad.last = None;
            return;
        };
        let out =
            self.pad
                .mappers
                .entry(id.clone())
                .or_default()
                .update(&map, &inst.controller, &input);
        for a in out.actions.iter() {
            let line = format!(
                "{}{}",
                if self.pad.drive { "" } else { "would have: " },
                describe(a)
            );
            self.pad.note(line);
        }
        if let Some(ok) = out.link {
            let line = format!(
                "{}radio link {}",
                if self.pad.drive { "" } else { "would have: " },
                if ok { "up" } else { "dropped" }
            );
            self.pad.note(line);
        }
        if self.pad.drive {
            for a in &out.actions {
                self.act(&id, a.clone());
            }
            if let Some(ok) = out.link {
                self.act(&id, Action::Link { ok });
            }
            for (control, position) in &out.holds {
                self.hold(&id, control, HoldBy::Pad, position.as_deref());
                let key = (id.clone(), control.clone());
                if position.is_some() {
                    self.pad.holding.insert(key);
                } else {
                    self.pad.holding.remove(&key);
                }
            }
        } else {
            self.pad_release_all();
        }
        self.pad.last = Some((id, out));
    }

    /// The card is closed or the live view is off: the pad lets go of everything it holds.
    pub(super) fn pad_card_closed(&mut self) {
        if self.pad.holding.is_empty() && self.pad.active.is_none() {
            return;
        }
        self.pad_stop();
        self.apply_holds();
    }

    /// Let go of everything the pad holds.
    pub(super) fn pad_release_all(&mut self) {
        let held: Vec<(String, String)> = self.pad.holding.iter().cloned().collect();
        for (instance, control) in held {
            self.hold(&instance, &control, HoldBy::Pad, None);
        }
        self.pad.holding.clear();
    }

    /// The pad stops acting (card closed, drive switched off): everything released and the
    /// mapper starts afresh the next time, so a deflected stick must return to neutral first.
    pub(super) fn pad_stop(&mut self) {
        self.pad_release_all();
        self.pad.mappers.clear();
        self.pad.last = None;
        self.pad.active = None;
    }

    /// Switch driving on or off.
    pub(super) fn pad_set_drive(&mut self, on: bool) {
        if self.pad.drive == on {
            return;
        }
        self.pad.drive = on;
        if on {
            self.pad.ensure_service();
        }
        self.pad_stop();
        self.pad.note(
            if on {
                "the pad now drives the controller"
            } else {
                "the pad is only watched"
            }
            .to_owned(),
        );
    }

    /// Fill the entry the person is listening for from the first thing the pad does.
    pub(super) fn pad_listen(&mut self, raw: &InputState) {
        let Some((what, base)) = self.pad.listen.clone() else {
            return;
        };
        if !raw.connected {
            return;
        }
        // An axis that moved far from where it was when listening began, or a button that
        // went down (a trigger counts as a button when it passes half travel).
        let axis = AXES
            .iter()
            .filter(|a| !a.ends_with("_trigger"))
            .find(|a| (raw.axis(a) - base.axis(a)).abs() > 0.6)
            .map(|a| a.to_string());
        let button = BUTTONS
            .iter()
            .find(|b| raw.button(b) && !base.button(b))
            .map(|b| b.to_string())
            .or_else(|| {
                ["left_trigger", "right_trigger"]
                    .into_iter()
                    .find(|t| raw.is_down(t) && !base.is_down(t))
                    .map(str::to_owned)
            });
        let Some(id) = self.selected.clone() else {
            return;
        };
        let map = self.pad.maps.entry(id).or_default();
        let done = match what {
            Listen::Axis(i) => axis.and_then(|a| map.axes.get_mut(i).map(|m| m.input = a)),
            Listen::Button(i) => button.and_then(|b| map.buttons.get_mut(i).map(|m| m.input = b)),
            Listen::Deadman => button.map(|b| {
                map.deadman
                    .get_or_insert(DeadmanMap {
                        input: String::new(),
                        control: None,
                    })
                    .input = b;
            }),
        };
        if done.is_some() {
            self.pad.listen = None;
            self.pad.edit_pending = true;
        }
    }

    fn pad_wizard_feed(&mut self, raw: &InputState) {
        let Some(w) = self.pad.wizard.as_mut() else {
            return;
        };
        w.cal.feed(raw);
        if w.cal.phase() == CalPhase::Centre && w.cal.centre_ready() {
            w.cal.start_range();
        }
    }

    /// Hand the edited maps to the project once no pointer button is down (a slider drag is one
    /// undo step, not one per frame).
    pub(super) fn pad_commit(&mut self, pointer_down: bool) {
        if self.pad.edit_pending && !pointer_down {
            self.pad.edit_pending = false;
            self.changed();
        }
    }

    /// Opens the calibration wizard for the axes of the selected map (the four sticks when it
    /// has none).
    pub(super) fn pad_open_wizard(&mut self) {
        self.pad.ensure_service();
        let id = self.selected.clone().unwrap_or_default();
        let mut inputs: Vec<String> = Vec::new();
        if let Some(map) = self.pad.maps.get(&id) {
            for a in &map.axes {
                if is_axis_name(&a.input) && !inputs.contains(&a.input) {
                    inputs.push(a.input.clone());
                }
            }
        }
        if inputs.is_empty() {
            inputs = ["left_x", "left_y", "right_x", "right_y"]
                .map(String::from)
                .to_vec();
        }
        let mut pairs = Vec::new();
        for (x, y) in [("left_x", "left_y"), ("right_x", "right_y")] {
            if inputs.iter().any(|i| i == x) && inputs.iter().any(|i| i == y) {
                pairs.push((x.to_owned(), y.to_owned()));
            }
        }
        let mut cal = Calibrator::new(&inputs);
        for (x, y) in &pairs {
            cal = cal.with_pair(x, y);
        }
        self.pad.wizard = Some(Wizard {
            cal,
            pairs,
            inputs,
            message: None,
        });
        // Calibrating is hands off for the controller: everything held is let go.
        self.pad_release_all();
    }

    /// Writes a map suggested for the selected controller's profile.
    pub(super) fn pad_suggest(&mut self, profile: &Profile) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let mut map = GamepadMap::suggest(profile);
        map.device = self.pad.device_name.clone();
        // Keep the calibration already measured for an input.
        if let Some(old) = self.pad.maps.get(&id) {
            for a in &mut map.axes {
                if let Some(o) = old.axes.iter().find(|o| o.input == a.input) {
                    (a.centre, a.low, a.high) = (o.centre, o.low, o.high);
                }
            }
        }
        self.pad.maps.insert(id, map);
        self.pad.mappers.clear();
        self.pad.edit_pending = true;
    }
}

// ---- the section in the card ----

/// A horizontal bar for a value from -1 to +1 (or 0 to 1 when `signed` is false), with ticks.
fn bar(ui: &mut egui::Ui, value: f64, signed: bool, ticks: &[f64], width: f32) -> egui::Response {
    let palette = crate::theme::ui_palette();
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, 10.0), egui::Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, 3.0, palette.sunken);
    let x_of = |v: f64| {
        let t = if signed { (v + 1.0) / 2.0 } else { v };
        rect.left() + rect.width() * t.clamp(0.0, 1.0) as f32
    };
    let base = x_of(0.0);
    let at = x_of(value);
    let fill = egui::Rect::from_x_y_ranges(base.min(at)..=base.max(at), rect.y_range());
    painter.rect_filled(fill, 3.0, palette.accent);
    for t in ticks {
        for sign in if signed { vec![-1.0, 1.0] } else { vec![1.0] } {
            let x = x_of(t * sign);
            painter.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(1.0, palette.text_tertiary),
            );
        }
    }
    if signed {
        painter.line_segment(
            [
                egui::pos2(base, rect.top() - 1.0),
                egui::pos2(base, rect.bottom() + 1.0),
            ],
            egui::Stroke::new(1.0, palette.text_secondary),
        );
    }
    painter.rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0, palette.border_control),
        egui::StrokeKind::Inside,
    );
    response
}

/// What the section asks the card to do.
pub(super) enum PadAction {
    OpenEditor,
    OpenWizard,
}

/// The Gamepad section of the Controller card.
pub(super) fn section(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    id: &str,
    profile: &Profile,
    actions: &mut Vec<PadAction>,
) {
    let palette = crate::theme::ui_palette();
    ctl.pad.ensure_service();
    ctl.pad.seen = true;
    let Some(svc) = ctl.pad.service.as_mut() else {
        return;
    };
    if let Some(why) = svc.unavailable() {
        ui.add(egui::Label::new(egui::RichText::new(why).color(palette.warning)).wrap());
        ui.weak("Mouse, touch and keyboard drive the controller everywhere.");
        return;
    }
    // Device choice and state.
    let devices: Vec<(String, String, bool)> = svc
        .devices()
        .iter()
        .map(|d| (d.id.clone(), d.name.clone(), d.state.connected))
        .collect();
    let chosen = svc.chosen().map(|d| d.id.clone());
    let source = svc.label().to_owned();
    let mut pick: Option<String> = None;
    if devices.is_empty() {
        ui.horizontal(|ui| {
            ui.colored_label(palette.text_tertiary, "○");
            ui.label("No gamepad found");
        });
        ui.add(
            egui::Label::new(
                egui::RichText::new(
                    "Plug in a USB gamepad or joystick; it is picked up at once, no driver \
                     or administrator rights needed.",
                )
                .color(palette.text_secondary),
            )
            .wrap(),
        );
    } else {
        let current = devices
            .iter()
            .find(|(i, _, _)| Some(i) == chosen.as_ref())
            .map(|(_, n, _)| n.clone())
            .unwrap_or_else(|| "Choose a device".to_owned());
        egui::ComboBox::from_id_salt(("pad_device", id))
            .selected_text(current)
            .width(200.0)
            .show_ui(ui, |ui| {
                for (dev, name, on) in &devices {
                    let label = if *on {
                        name.clone()
                    } else {
                        format!("{name} (not connected)")
                    };
                    if ui
                        .selectable_label(chosen.as_deref() == Some(dev.as_str()), label)
                        .clicked()
                    {
                        pick = Some(dev.clone());
                    }
                }
            });
        let connected = ctl.pad.input.connected;
        ui.horizontal(|ui| {
            ui.colored_label(
                if connected {
                    palette.success
                } else {
                    palette.error
                },
                if connected { "●" } else { "○" },
            );
            ui.label(if connected {
                "Connected"
            } else {
                "Not connected: every control is let go"
            });
            ui.weak(format!("via {source}"));
        });
    }
    if let Some(dev) = pick
        && let Some(svc) = ctl.pad.service.as_mut()
    {
        svc.choose(&dev);
        ctl.pad_stop();
    }

    // The switch.
    let mut drive = ctl.pad.drive;
    ui.checkbox(&mut drive, "Drive this controller with the pad")
        .on_hover_text(
            "Off: the pad is only watched and the lines below show what it would do. On: the \
             sticks and buttons hold the same controls as the mouse and the keyboard, through \
             the map. The pad stops when this card is closed or the window loses the focus",
        );
    if drive != ctl.pad.drive {
        ctl.pad_set_drive(drive);
    }

    ui.horizontal_wrapped(|ui| {
        if ui
            .small_button("Edit map…")
            .on_hover_text("Which axis or button drives which control, deadzone, invert, dead-man")
            .clicked()
        {
            actions.push(PadAction::OpenEditor);
        }
        if ui
            .small_button("Calibrate…")
            .on_hover_text("Move the sticks to all corners to measure centre and range")
            .clicked()
        {
            actions.push(PadAction::OpenWizard);
        }
    });

    // Monitor: the sticks and triggers as the device reports them.
    let input = ctl.pad.input.clone();
    let map = ctl.pad.maps.get(id).cloned();
    let ticks_of = |axis: &str| -> Vec<f64> {
        let Some(m) = map.as_ref() else {
            return Vec::new();
        };
        let Some(a) = m.axes.iter().find(|a| a.input == axis) else {
            return Vec::new();
        };
        let Some(def) = profile.control(&a.control) else {
            return Vec::new();
        };
        if matches!(
            def.kind,
            ControlKind::Rocker2 | ControlKind::Rocker3 | ControlKind::Axis
        ) {
            let (neg, pos) = detent_counts(def);
            step_edges(a.deadzone, neg.max(pos))
        } else {
            Vec::new()
        }
    };
    if ctl.pad.input.connected {
        egui::Grid::new(("pad_monitor", id))
            .num_columns(3)
            .spacing([6.0, 2.0])
            .show(ui, |ui| {
                for axis in AXES.iter().filter(|a| !a.starts_with("dpad")) {
                    let trigger = axis.ends_with("_trigger");
                    ui.weak(*axis);
                    let v = input.axis(axis);
                    bar(ui, v, !trigger, &ticks_of(axis), 110.0);
                    ui.weak(format!("{v:+.2}"));
                    ui.end_row();
                }
            });
        let pressed: Vec<&str> = BUTTONS
            .iter()
            .copied()
            .filter(|b| input.button(b))
            .collect();
        ui.horizontal_wrapped(|ui| {
            ui.weak("Buttons down:");
            if pressed.is_empty() {
                ui.weak("none");
            }
            for b in pressed {
                ui.label(egui::RichText::new(b).strong());
            }
        });
    }

    // Gate and what the pad asks for.
    match (&map, ctl.pad.last.as_ref()) {
        (None, _) => {
            ui.add(
                egui::Label::new(
                    egui::RichText::new("This controller has no pad map yet.")
                        .color(palette.warning),
                )
                .wrap(),
            );
        }
        (Some(m), Some((_, out))) => {
            if m.deadman.is_some() {
                ui.horizontal(|ui| {
                    ui.colored_label(
                        if out.gate_open {
                            palette.success
                        } else {
                            palette.warning
                        },
                        if out.gate_open { "●" } else { "○" },
                    );
                    ui.label(if out.gate_open {
                        "Dead-man held"
                    } else {
                        "Dead-man open: nothing moves"
                    });
                });
            }
            let held: Vec<String> = out
                .holds
                .iter()
                .filter_map(|(c, p)| p.as_ref().map(|p| format!("{c} → {p}")))
                .collect();
            if !held.is_empty() {
                ui.add(
                    egui::Label::new(format!(
                        "{} {}",
                        if ctl.pad.drive {
                            "Holding:"
                        } else {
                            "Test, would hold:"
                        },
                        held.join(", ")
                    ))
                    .wrap(),
                );
            }
            for r in out.axes.iter().filter(|r| r.waiting_for_neutral) {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!(
                            "{} waits until {} is back at rest",
                            r.control, r.input
                        ))
                        .color(palette.warning),
                    )
                    .wrap(),
                );
            }
        }
        _ => {}
    }
    if !ctl.pad.log.is_empty() {
        for line in ctl.pad.log.iter().rev() {
            ui.weak(line);
        }
    }

    ui.add(
        egui::Label::new(
            egui::RichText::new(
                "Simulation of wiring and logic. The pad holds the controls of this profile; \
                 it says nothing about any real transmitter.",
            )
            .color(palette.text_tertiary),
        )
        .wrap(),
    );
}

// ---- the map editor ----

fn combo(
    ui: &mut egui::Ui,
    salt: impl std::hash::Hash + std::fmt::Debug,
    value: &mut String,
    options: &[String],
) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(if value.is_empty() {
            "choose…".to_owned()
        } else {
            value.clone()
        })
        .width(96.0)
        .show_ui(ui, |ui| {
            for o in options {
                ui.selectable_value(value, o.clone(), o);
            }
        });
}

fn num(ui: &mut egui::Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, tip: &str) {
    ui.add(
        egui::DragValue::new(value)
            .range(range)
            .speed(0.005)
            .fixed_decimals(2),
    )
    .on_hover_text(tip);
}

/// The map editor window.
pub(super) fn editor(ctx: &egui::Context, ctl: &mut ControllerLive) {
    if !ctl.pad.editor {
        return;
    }
    let Some(id) = ctl.selected.clone() else {
        ctl.pad.editor = false;
        return;
    };
    let Some(profile) = ctl
        .set
        .instance(&id)
        .map(|i| i.controller.profile().clone())
    else {
        ctl.pad.editor = false;
        return;
    };
    let palette = crate::theme::ui_palette();
    let before = ctl.pad.maps.get(&id).cloned().unwrap_or_default();
    let mut map = before.clone();
    let input = ctl.pad.input.clone();
    let listening = ctl.pad.listen.as_ref().map(|(l, _)| l.clone());
    let mut new_listen: Option<Option<Listen>> = None;
    let mut suggest = false;
    let mut open = true;

    let axis_controls: Vec<String> = profile
        .controls
        .iter()
        .filter(|c| {
            matches!(
                c.kind,
                ControlKind::Rocker2
                    | ControlKind::Rocker3
                    | ControlKind::Axis
                    | ControlKind::Selector
            )
        })
        .map(|c| c.id.clone())
        .collect();
    let all_controls: Vec<String> = profile.controls.iter().map(|c| c.id.clone()).collect();
    let deadman_controls: Vec<String> = profile
        .controls
        .iter()
        .filter(|c| c.kind == ControlKind::Deadman)
        .map(|c| c.id.clone())
        .collect();
    let axis_names: Vec<String> = AXES.iter().map(|s| s.to_string()).collect();
    let button_names: Vec<String> = BUTTONS
        .iter()
        .map(|s| s.to_string())
        .chain(["left_trigger".to_owned(), "right_trigger".to_owned()])
        .collect();

    super::panel::dialog(ctx, "Gamepad map", [560.0, 520.0])
        .open(super::panel::esc(ctx, &mut open))
        .show(ctx, |ui| {
            ui.add(
                egui::Label::new(
                    "Which axis or button of the pad drives which control of this controller. \
                     A stick position picks the step of a rocker or axis: the deadzone is rest, \
                     the rest of the throw is cut into one band per step.",
                )
                .wrap(),
            );
            ui.horizontal_wrapped(|ui| {
                if ui
                    .button("Suggest a map")
                    .on_hover_text("Replace this map with the usual layout for this profile")
                    .clicked()
                {
                    suggest = true;
                }
                ui.label("When the pad is lost:");
                egui::ComboBox::from_id_salt("pad_on_disconnect")
                    .selected_text(map.on_disconnect.label())
                    .show_ui(ui, |ui| {
                        for l in LinkLoss::ALL {
                            ui.selectable_value(&mut map.on_disconnect, l, l.label());
                        }
                    });
            });
            ui.checkbox(
                &mut map.neutral_required,
                "A deflected stick must return to rest first (after connecting or the dead-man)",
            );

            ui.separator();
            ui.label(egui::RichText::new("Dead-man").strong());
            ui.horizontal_wrapped(|ui| {
                let mut on = map.deadman.is_some();
                if ui
                    .checkbox(&mut on, "Hold a button for motion")
                    .on_hover_text(
                        "Nothing that moves acts while this button (or trigger) is not held. \
                         The E-stop button is never held back",
                    )
                    .changed()
                {
                    map.deadman = on.then(|| DeadmanMap {
                        input: "left_trigger".to_owned(),
                        control: deadman_controls.first().cloned(),
                    });
                }
                if let Some(d) = map.deadman.as_mut() {
                    combo(ui, "pad_deadman_input", &mut d.input, &button_names);
                    let l = listening == Some(Listen::Deadman);
                    if ui
                        .small_button(if l { "press it…" } else { "listen" })
                        .on_hover_text("Press the button or trigger on the pad")
                        .clicked()
                    {
                        new_listen = Some((!l).then_some(Listen::Deadman));
                    }
                    ui.label("holds");
                    let mut c = d.control.clone().unwrap_or_default();
                    let mut options = vec![String::new()];
                    options.extend(deadman_controls.iter().cloned());
                    egui::ComboBox::from_id_salt("pad_deadman_control")
                        .selected_text(if c.is_empty() {
                            "no control"
                        } else {
                            c.as_str()
                        })
                        .width(96.0)
                        .show_ui(ui, |ui| {
                            for o in &options {
                                ui.selectable_value(
                                    &mut c,
                                    o.clone(),
                                    if o.is_empty() {
                                        "no control"
                                    } else {
                                        o.as_str()
                                    },
                                );
                            }
                        });
                    d.control = (!c.is_empty()).then_some(c);
                }
            });

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Axes").strong());
                if ui.small_button("Add axis").clicked() {
                    map.axes.push(AxisMap::new(
                        "left_y",
                        axis_controls.first().map_or("", String::as_str),
                    ));
                }
            });
            let mut remove: Option<usize> = None;
            for (i, a) in map.axes.iter_mut().enumerate() {
                ui.push_id(("pad_axis", i), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        combo(ui, "input", &mut a.input, &axis_names);
                        let l = listening == Some(Listen::Axis(i));
                        if ui
                            .small_button(if l { "move it…" } else { "listen" })
                            .on_hover_text("Move the stick on the pad")
                            .clicked()
                        {
                            new_listen = Some((!l).then_some(Listen::Axis(i)));
                        }
                        ui.label("→");
                        combo(ui, "control", &mut a.control, &axis_controls);
                        if ui.small_button("✕").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("deadzone");
                        num(
                            ui,
                            &mut a.deadzone,
                            0.0..=0.9,
                            "Share of the throw that counts as rest",
                        );
                        ui.label("hysteresis");
                        num(
                            ui,
                            &mut a.hysteresis,
                            0.0..=0.5,
                            "How far a value must fall back before a step is left",
                        );
                        ui.checkbox(&mut a.invert, "invert");
                        ui.checkbox(&mut a.gated, "dead-man")
                            .on_hover_text("Held back while the dead-man is not held");
                    });
                    ui.horizontal_wrapped(|ui| {
                        let calibrated = (a.centre, a.low, a.high) != (0.0, -1.0, 1.0);
                        ui.weak(if calibrated {
                            format!(
                                "calibrated: centre {:+.2}, low {:+.2}, high {:+.2}",
                                a.centre, a.low, a.high
                            )
                        } else {
                            "not calibrated (use Calibrate…)".to_owned()
                        });
                        let v = a.value(input.axis(&a.input));
                        let ticks = profile
                            .control(&a.control)
                            .filter(|d| d.kind != ControlKind::Selector)
                            .map(|d| {
                                let (n, p) = detent_counts(d);
                                step_edges(a.deadzone, n.max(p))
                            })
                            .unwrap_or_default();
                        bar(ui, v, true, &ticks, 120.0);
                    });
                });
                ui.add_space(2.0);
            }
            if let Some(i) = remove {
                map.axes.remove(i);
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Buttons").strong());
                if ui.small_button("Add button").clicked() {
                    map.buttons.push(ButtonMap::new(
                        "south",
                        all_controls.first().map_or("", String::as_str),
                        ButtonAction::Hold,
                    ));
                }
            });
            let mut remove: Option<usize> = None;
            for (i, b) in map.buttons.iter_mut().enumerate() {
                ui.push_id(("pad_button", i), |ui| {
                    ui.horizontal_wrapped(|ui| {
                        combo(ui, "input", &mut b.input, &button_names);
                        let l = listening == Some(Listen::Button(i));
                        if ui
                            .small_button(if l { "press it…" } else { "listen" })
                            .on_hover_text("Press the button on the pad")
                            .clicked()
                        {
                            new_listen = Some((!l).then_some(Listen::Button(i)));
                        }
                        ui.label("→");
                        combo(ui, "control", &mut b.control, &all_controls);
                        egui::ComboBox::from_id_salt("action")
                            .selected_text(b.action.label())
                            .width(110.0)
                            .show_ui(ui, |ui| {
                                for a in ButtonAction::ALL {
                                    ui.selectable_value(&mut b.action, a, a.label());
                                }
                            });
                        if matches!(b.action, ButtonAction::Hold | ButtonAction::Set) {
                            let positions = profile
                                .control(&b.control)
                                .map(|c| c.position_list())
                                .unwrap_or_default();
                            let mut p = b.position.clone().unwrap_or_default();
                            egui::ComboBox::from_id_salt("position")
                                .selected_text(if p.is_empty() { "first" } else { p.as_str() })
                                .width(70.0)
                                .show_ui(ui, |ui| {
                                    if b.action == ButtonAction::Hold {
                                        ui.selectable_value(&mut p, String::new(), "first");
                                    }
                                    for q in &positions {
                                        ui.selectable_value(&mut p, q.clone(), q);
                                    }
                                });
                            b.position = (!p.is_empty()).then_some(p);
                        }
                        if b.action == ButtonAction::Hold {
                            ui.checkbox(&mut b.gated, "dead-man");
                        }
                        let down = input.is_down(&b.input);
                        ui.colored_label(
                            if down {
                                palette.success
                            } else {
                                palette.text_tertiary
                            },
                            if down { "●" } else { "○" },
                        );
                        if ui.small_button("✕").on_hover_text("Remove").clicked() {
                            remove = Some(i);
                        }
                    });
                });
            }
            if let Some(i) = remove {
                map.buttons.remove(i);
            }

            ui.separator();
            let issues = map.validate(&profile);
            if issues.is_empty() {
                ui.colored_label(palette.success, "No problems found in this map");
            }
            for issue in &issues {
                let colour = match issue.severity {
                    Severity::Error => palette.error,
                    Severity::Warning => palette.warning,
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(format!("{}: {}", issue.path, issue.message))
                            .color(colour),
                    )
                    .wrap(),
                );
            }
            ui.add_space(4.0);
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "The map is saved with the project. A pad only holds controls; the \
                         controller keeps its own interlocks, E-stop latch and link rules.",
                    )
                    .color(palette.text_tertiary),
                )
                .wrap(),
            );
        });

    if !open {
        ctl.pad.editor = false;
        ctl.pad.listen = None;
    }
    if let Some(l) = new_listen {
        ctl.pad.listen = l.map(|l| (l, input.clone()));
    }
    if suggest {
        ctl.pad_suggest(&profile);
    } else if map != before {
        ctl.pad.maps.insert(id, map);
        ctl.pad.edit_pending = true;
    }
}

// ---- the calibration wizard ----

/// The calibration window.
pub(super) fn wizard(ctx: &egui::Context, ctl: &mut ControllerLive) {
    let Some(w) = ctl.pad.wizard.as_ref() else {
        return;
    };
    let palette = crate::theme::ui_palette();
    let raw = ctl.pad.input.clone();
    let mut open = true;
    let mut finish = false;
    let mut restart = false;
    let mut cancel = false;
    let progress = w.cal.progress();
    let corners: Vec<(String, String, [bool; 4])> = w
        .cal
        .corners()
        .into_iter()
        .map(|(x, y, c)| (x.to_owned(), y.to_owned(), c))
        .collect();
    let phase = w.cal.phase();
    let centre_fraction = w.cal.centre_fraction();
    let complete = w.cal.complete();
    let can_finish = w.cal.finish().is_ok();
    let message = w.message.clone();
    let connected = raw.connected;

    super::panel::dialog(ctx, "Calibrate gamepad", [420.0, 0.0])
        .open(super::panel::esc(ctx, &mut open))
        .show(ctx, |ui| {
            ui.add(
                egui::Label::new(
                    "The pad does not drive the controller while you calibrate. Calibration \
                     measures where each stick rests and how far it really goes, so the \
                     deadzone and the steps are right for this pad.",
                )
                .wrap(),
            );
            if !connected {
                ui.colored_label(
                    palette.warning,
                    "No pad connected. Plug one in to continue.",
                );
            }
            ui.separator();
            match phase {
                CalPhase::Centre => {
                    ui.label(egui::RichText::new("1. Let go of the sticks and triggers").strong());
                    ui.add(egui::ProgressBar::new(centre_fraction as f32).show_percentage());
                }
                CalPhase::Range => {
                    ui.label(
                        egui::RichText::new("2. Move each stick slowly to all corners").strong(),
                    );
                    ui.add(
                        egui::Label::new(
                            "Draw a few full circles with every stick. Press a trigger all the \
                             way if it is listed. The corners turn green as you reach them.",
                        )
                        .wrap(),
                    );
                    ui.horizontal(|ui| {
                        for (x, y, c) in &corners {
                            stick_square(ui, &raw, x, y, c);
                        }
                    });
                    egui::Grid::new("pad_cal_axes")
                        .num_columns(4)
                        .spacing([8.0, 2.0])
                        .show(ui, |ui| {
                            for p in &progress {
                                ui.label(&p.input);
                                ui.weak(format!("{:+.2} … {:+.2}", p.min, p.max));
                                let side = |ok: bool| {
                                    if ok {
                                        palette.success
                                    } else {
                                        palette.text_tertiary
                                    }
                                };
                                ui.horizontal(|ui| {
                                    if !p.one_sided {
                                        ui.colored_label(side(p.low_reached), "low");
                                    }
                                    ui.colored_label(side(p.high_reached), "high");
                                });
                                ui.colored_label(
                                    if p.done() {
                                        palette.success
                                    } else {
                                        palette.text_tertiary
                                    },
                                    if p.done() { "✓" } else { "…" },
                                );
                                ui.end_row();
                            }
                        });
                    if !complete {
                        ui.weak("Not every corner or end has been reached yet");
                    }
                }
            }
            if let Some(m) = &message {
                ui.colored_label(palette.warning, m);
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(can_finish, egui::Button::new("Finish and save"))
                    .on_hover_text("Needs every stick moved to both ends")
                    .clicked()
                {
                    finish = true;
                }
                if ui.button("Start over").clicked() {
                    restart = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });

    if !open || cancel {
        ctl.pad.wizard = None;
        return;
    }
    if restart {
        let (inputs, pairs) = {
            let w = ctl.pad.wizard.as_ref().expect("wizard is open");
            (w.inputs.clone(), w.pairs.clone())
        };
        let mut cal = Calibrator::new(&inputs);
        for (x, y) in &pairs {
            cal = cal.with_pair(x, y);
        }
        if let Some(w) = ctl.pad.wizard.as_mut() {
            w.cal = cal;
            w.message = None;
        }
    }
    if finish {
        ctl.pad_wizard_finish();
    }
}

impl ControllerLive {
    /// Take the calibration into the map of the selected controller and close the wizard. When
    /// an axis was not moved far enough the wizard stays open and says which.
    pub(super) fn pad_wizard_finish(&mut self) -> bool {
        let Some(result) = self.pad.wizard.as_ref().map(|w| w.cal.finish()) else {
            return false;
        };
        match result {
            Ok(cal) => {
                if let Some(id) = self.selected.clone() {
                    let name = self.pad.device_name.clone();
                    let map = self.pad.maps.entry(id).or_default();
                    map.apply_calibration(&cal);
                    if map.device.is_empty() {
                        map.device = name;
                    }
                    self.pad.edit_pending = true;
                    self.pad.mappers.clear();
                    self.pad.note("calibration saved".to_owned());
                }
                self.pad.wizard = None;
                true
            }
            Err(e) => {
                if let Some(w) = self.pad.wizard.as_mut() {
                    w.message = Some(match e {
                        novo_controller::gamepad::CalError::CentreNotReady => {
                            "Let go of the sticks first".to_owned()
                        }
                        novo_controller::gamepad::CalError::Incomplete(missing) => format!(
                            "Not finished: {}",
                            missing
                                .iter()
                                .map(|(a, side)| format!("{a} {side}"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    });
                }
                false
            }
        }
    }
}

/// A small square showing where a stick is, with its four corners lit once visited.
fn stick_square(ui: &mut egui::Ui, raw: &InputState, x: &str, y: &str, corners: &[bool; 4]) {
    let palette = crate::theme::ui_palette();
    let size = 92.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(size, size + 14.0), egui::Sense::hover());
    let square = egui::Rect::from_min_size(rect.min, egui::vec2(size, size));
    let painter = ui.painter();
    painter.rect_filled(square, 4.0, palette.sunken);
    painter.rect_stroke(
        square,
        4.0,
        egui::Stroke::new(1.0, palette.border_control),
        egui::StrokeKind::Inside,
    );
    let to = |vx: f64, vy: f64| {
        egui::pos2(
            square.center().x + (vx as f32) * (size / 2.0 - 8.0),
            square.center().y - (vy as f32) * (size / 2.0 - 8.0),
        )
    };
    // Counter-clockwise from the upper right, as the calibrator reports them.
    for (c, (sx, sy)) in corners
        .iter()
        .zip([(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)])
    {
        painter.circle_filled(
            to(sx, sy),
            5.0,
            if *c { palette.success } else { palette.control },
        );
    }
    painter.circle_filled(to(raw.axis(x), raw.axis(y)), 4.0, palette.accent);
    painter.text(
        egui::pos2(rect.center().x, rect.bottom()),
        egui::Align2::CENTER_BOTTOM,
        format!("{x}/{y}"),
        egui::FontId::proportional(11.0),
        palette.text_secondary,
    );
    response.on_hover_text("The dot is the stick; the circles are its four corners");
}
