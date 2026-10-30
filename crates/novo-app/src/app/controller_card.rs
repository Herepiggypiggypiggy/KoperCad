//! The Controller card of the live view (lane C2, minimal panel; lane C3 builds the full one).
//!
//! Where the user finds it: Simulate > Controller panel (command `sim.controller`), or the
//! "Controller..." button on the Live potentials card. The card shows the selected controller
//! the way its profile describes it: buttons to hold, rockers with a zone per step, selectors,
//! the emergency stop with its reset, then a monitor of the signals, the bindings to the drawing
//! and the checks. It simulates wiring and logic; it says so on the card.

use eframe::egui;
use novo_controller::profile::split_group_step;
use novo_controller::{ControlDef, ControlKind, Profile, Severity, SignalKey, SignalKind, Target};
use novo_core::Project;

use super::controller_live::{
    ControllerLive, HoldBy, Resolved, Source, hand_devices, receiver_outputs, receivers,
};
use super::{Level, NovoApp};

/// What a press on the card asks the application to do.
enum CardAction {
    OpenWizard,
    Close,
}

/// The keys of the keyboard map (see [`key_map`]).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct KeyBinding {
    pub(super) key: egui::Key,
    pub(super) control: String,
    pub(super) action: KeyAction,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum KeyAction {
    /// Hold the control at a position while the key is down: `(step, position)` for each step of
    /// one direction; Shift picks step 2, Alt step 3.
    Hold(Vec<(u8, String)>),
    Toggle,
    Press,
    Reset,
}

/// The default keyboard map of a profile, in the order of its controls:
/// rockers and axes take arrow and letter pairs (one key per direction, Shift for step 2),
/// the first latch is Space, the first emergency stop is X (reset R), other buttons 1 to 9.
pub(super) fn key_map(profile: &Profile) -> Vec<KeyBinding> {
    use egui::Key::*;
    let pairs = [
        (ArrowUp, ArrowDown),
        (ArrowRight, ArrowLeft),
        (W, S),
        (D, A),
        (I, K),
        (L, J),
    ];
    let digits = [Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9];
    let (mut next_pair, mut next_digit) = (0, 0);
    let (mut latch, mut estop, mut deadman) = (false, false, false);
    let mut out = Vec::new();
    for def in &profile.controls {
        match def.kind {
            ControlKind::Rocker2 | ControlKind::Rocker3 | ControlKind::Axis => {
                let Some(pair) = pairs.get(next_pair) else {
                    continue;
                };
                next_pair += 1;
                for (key, group) in [pair.0, pair.1].into_iter().zip(direction_groups(def)) {
                    out.push(KeyBinding {
                        key,
                        control: def.id.clone(),
                        action: KeyAction::Hold(group),
                    });
                }
            }
            ControlKind::Latch if !latch => {
                latch = true;
                out.push(KeyBinding {
                    key: Space,
                    control: def.id.clone(),
                    action: KeyAction::Toggle,
                });
            }
            ControlKind::EStop if !estop => {
                estop = true;
                out.push(KeyBinding {
                    key: X,
                    control: def.id.clone(),
                    action: KeyAction::Press,
                });
                out.push(KeyBinding {
                    key: R,
                    control: def.id.clone(),
                    action: KeyAction::Reset,
                });
            }
            ControlKind::Deadman if !deadman => {
                deadman = true;
                out.push(KeyBinding {
                    key: V,
                    control: def.id.clone(),
                    action: KeyAction::Hold(vec![(1, "held".to_owned())]),
                });
            }
            ControlKind::Button => {
                if let (Some(key), Some(position)) =
                    (digits.get(next_digit), def.first_active_position())
                {
                    next_digit += 1;
                    out.push(KeyBinding {
                        key: *key,
                        control: def.id.clone(),
                        action: KeyAction::Hold(vec![(1, position)]),
                    });
                }
            }
            _ => {}
        }
    }
    out
}

/// The positions of a rocker or axis grouped by direction, each as `(step, position)`.
fn direction_groups(def: &ControlDef) -> Vec<Vec<(u8, String)>> {
    novo_controller::panel::direction_groups(def)
        .into_iter()
        .map(|d| d.steps)
        .collect()
}

/// A position as the card prints it: `up2` is "up 2".
pub(super) fn position_label(position: &str) -> String {
    match split_group_step(position) {
        Some((g, n)) => format!("{g} {n}"),
        None => position.to_owned(),
    }
}

/// The key a hint shows: arrows as arrows.
pub(super) fn key_name(key: egui::Key) -> String {
    match key {
        egui::Key::ArrowUp => "↑".into(),
        egui::Key::ArrowDown => "↓".into(),
        egui::Key::ArrowLeft => "←".into(),
        egui::Key::ArrowRight => "→".into(),
        egui::Key::Space => "Space".into(),
        other => other.name().to_owned(),
    }
}

impl NovoApp {
    /// Simulate > Controller panel: the live view on, the card open or closed.
    pub(super) fn toggle_controller_panel(&mut self) {
        if !self.wb.live.on {
            self.toggle_live();
        }
        self.wb.live.ctl.open ^= true;
    }

    /// Keys of the selected controller while the keyboard is switched on.
    fn controller_keys(&mut self, ctx: &egui::Context) {
        let ctl = &mut self.wb.live.ctl;
        let Some(id) = ctl.selected.clone() else {
            return;
        };
        let Some(profile) = ctl
            .set
            .instance(&id)
            .map(|i| i.controller.profile().clone())
        else {
            return;
        };
        let typing = ctx.egui_wants_keyboard_input();
        for binding in key_map(&profile) {
            let (down, pressed, mods) = ctx.input(|i| {
                (
                    i.key_down(binding.key),
                    i.key_pressed(binding.key),
                    i.modifiers,
                )
            });
            let active = ctl.keys && !typing;
            match &binding.action {
                KeyAction::Hold(steps) => {
                    let want = steps
                        .iter()
                        .rev()
                        .find(|(step, _)| {
                            let wish = if mods.alt {
                                3
                            } else if mods.shift {
                                2
                            } else {
                                1
                            };
                            *step <= wish
                        })
                        .or(steps.first())
                        .map(|(_, p)| p.as_str());
                    ctl.hold(
                        &id,
                        &binding.control,
                        HoldBy::Key,
                        if active && down { want } else { None },
                    );
                }
                KeyAction::Toggle | KeyAction::Press | KeyAction::Reset if active && pressed => {
                    let action = match binding.action {
                        KeyAction::Reset => novo_controller::Action::Reset {
                            control: binding.control.clone(),
                        },
                        _ => novo_controller::Action::Press {
                            control: binding.control.clone(),
                        },
                    };
                    ctl.act(&id, action);
                }
                _ => {}
            }
            if active && (down || pressed) {
                // The arrows and letters belong to the controller now, not to the canvas.
                ctx.input_mut(|i| {
                    i.consume_key(egui::Modifiers::NONE, binding.key);
                    i.consume_key(egui::Modifiers::SHIFT, binding.key);
                    i.consume_key(egui::Modifiers::ALT, binding.key);
                });
            }
        }
    }

    /// The Controller card, drawn over the live view while it is open.
    pub(super) fn controller_card(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.live.on || !self.wb.live.ctl.open {
            return;
        }
        self.controller_keys(ctx);
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let ctl = &mut self.wb.live.ctl;
        ctl.sync(project);
        ctl.apply_holds();
        let mut actions = Vec::new();
        let card = super::panel::Panel::new(
            "controller_card",
            "Controller",
            canvas,
            super::panel::Side::Right,
            300.0,
        )
        .top(10.0)
        .fixed_open()
        .fit()
        .dockable()
        .scrolls()
        .show(ctx, |ui| draw_card(ui, ctl, project, &mut actions));
        if let Some(card) = card {
            self.anchor("live.controller", card.response.rect, "Controller card");
        }
        // The held controls change signals every frame they are held.
        if ctx.input(|i| i.pointer.any_down()) || self.wb.live.ctl.wants_frames() {
            ctx.request_repaint();
        }
        for action in actions {
            match action {
                CardAction::OpenWizard => self.open_controller_import(),
                CardAction::Close => self.wb.live.ctl.open = false,
            }
        }
        self.controllers_save();
    }

    /// Writes the controllers to the project when their structure changed: one undo step.
    pub(super) fn controllers_save(&mut self) {
        let Some(text) = self.wb.live.ctl.pending_write() else {
            return;
        };
        let text = match text {
            Ok(t) => t,
            Err(e) => {
                self.message(Level::Error, format!("Controllers could not be saved: {e}"));
                self.wb.live.ctl.dirty = false;
                return;
            }
        };
        let Some(doc) = self.backend.document_mut() else {
            return;
        };
        let result = doc.transact("Controllers", |tx| {
            tx.set_info(|i| {
                i.properties.remove(super::controller_live::LEGACY_PROPERTY);
            })?;
            tx.set_addon_block(
                super::controller_live::ADDON,
                super::controller_live::BLOCK,
                novo_core::AddonBlock {
                    format: super::controller_live::BLOCK_FORMAT,
                    text: text.clone(),
                },
            )
        });
        match result {
            Ok(((), changes)) => {
                self.wb.live.ctl.written(&text);
                self.changed_outside_canvas(&changes);
            }
            Err(e) => self.message(Level::Error, format!("Controllers could not be saved: {e}")),
        }
    }
}

fn draw_card(
    ui: &mut egui::Ui,
    ctl: &mut ControllerLive,
    project: &Project,
    actions: &mut Vec<CardAction>,
) {
    let palette = crate::theme::ui_palette();
    ui.horizontal(|ui| {
        if ui.small_button("✕ Close").clicked() {
            actions.push(CardAction::Close);
        }
        ui.weak("Simulation of wiring and logic");
    });
    if let Some(why) = ctl.damaged.clone() {
        ui.colored_label(palette.error, &why);
        ui.weak("It is kept in the project until you discard it.");
        if ui.small_button("Discard the stored controllers").clicked() {
            ctl.discard_damaged();
        }
    }
    for note in ctl.notes.clone() {
        ui.add(egui::Label::new(egui::RichText::new(note).color(palette.warning)).wrap());
    }
    instance_row(ui, ctl, actions);
    let Some(id) = ctl.selected.clone() else {
        ui.add_space(4.0);
        ui.add(
            egui::Label::new(
                "A virtual radio or pendant presses the contacts of a receiver drawn in the \
                 schematic. Add a controller, place a receiver (type rx6 in the command field) \
                 and bind its pins.",
            )
            .wrap(),
        );
        return;
    };
    let Some((profile, generic)) = ctl.set.instance(&id).map(|i| {
        (
            i.controller.profile().clone(),
            i.controller.profile().controller.generic,
        )
    }) else {
        return;
    };
    if generic {
        ui.weak("Generic controller, not a specific product");
    }
    // The effect of what is held, without scrolling to the signal list.
    if let Some(inst) = ctl.set.instance(&id) {
        let signals = inst.controller.signals();
        let high = signals.iter().filter(|s| s.is_high()).count();
        ui.weak(format!("Signals ({high} of {} high)", signals.len()));
    }
    ui.separator();
    let map = key_map(&profile);
    let plan = novo_controller::panel::plan(&profile);
    super::controller_widgets::draw_panel(ui, ctl, &id, &profile, &plan, &map);
    if let Some(why) = ctl.refused.clone() {
        ui.colored_label(palette.warning, why);
    }
    ui.horizontal(|ui| {
        ui.checkbox(&mut ctl.keys, "Keyboard").on_hover_text(
            "Arrow keys and letters hold the rockers (Shift for step 2), Space is the main \
             latch, X presses the emergency stop and R resets it. The canvas does not see \
             these keys while this is on",
        );
        ui.checkbox(&mut ctl.sticky, "Sticky").on_hover_text(
            "A click keeps a button or the stick held until you click it again, so several \
             motions run at once",
        );
        if ui
            .small_button("Reset")
            .on_hover_text("Every control back to rest")
            .clicked()
        {
            ctl.reset_state();
        }
    });
    egui::CollapsingHeader::new("Key map")
        .id_salt(("controller_key_map", &id))
        .show(ui, |ui| {
            super::controller_widgets::key_map_table(ui, &profile, &map);
        });
    effects_row(ui, ctl, &id);
    ui.weak("Scroll for the signals, bindings and checks");
    ui.separator();
    signals_block(ui, ctl, &id, project);
    ui.separator();
    bindings_block(ui, ctl, &id, project);
    let findings = ctl.findings(project);
    if !findings.is_empty() {
        ui.separator();
        ui.label(egui::RichText::new("Checks").strong());
        for f in &findings {
            let color = match f.severity {
                Severity::Error => palette.error,
                Severity::Warning => palette.warning,
            };
            ui.add(egui::Label::new(egui::RichText::new(&f.text).color(color)).wrap());
        }
    }
    ui.separator();
    ui.horizontal(|ui| {
        if ui
            .small_button("Import profile…")
            .on_hover_text("File > Import... > Controller from I/O table")
            .clicked()
        {
            actions.push(CardAction::OpenWizard);
        }
        if ui
            .small_button("Reload profiles")
            .on_hover_text("Read the profile folder again")
            .clicked()
        {
            ctl.reload_library();
        }
    });
}

fn instance_row(ui: &mut egui::Ui, ctl: &mut ControllerLive, _actions: &mut Vec<CardAction>) {
    ui.horizontal(|ui| {
        let current = ctl
            .selected
            .as_ref()
            .and_then(|s| ctl.set.instance(s))
            .map(|i| i.label.clone())
            .unwrap_or_else(|| "No controller".to_owned());
        let ids: Vec<(String, String)> = ctl
            .set
            .instances()
            .iter()
            .map(|i| (i.id.clone(), i.label.clone()))
            .collect();
        egui::ComboBox::from_id_salt("controller_instance")
            .selected_text(current)
            .width(150.0)
            .show_ui(ui, |ui| {
                for (id, label) in &ids {
                    if ui
                        .selectable_label(ctl.selected.as_deref() == Some(id), label)
                        .clicked()
                    {
                        ctl.selected = Some(id.clone());
                    }
                }
            });
        let mut add: Option<novo_controller::Profile> = None;
        ui.menu_button("Add", |ui| {
            let mut last_source: Option<bool> = None;
            for e in ctl.profiles() {
                let folder = matches!(e.source, Source::Folder(_));
                if last_source.is_some_and(|l| l != folder) {
                    ui.separator();
                }
                last_source = Some(folder);
                if ui.button(&e.profile.controller.name).clicked() {
                    add = Some(e.profile.clone());
                    ui.close();
                }
            }
            for p in ctl.library_problems() {
                ui.weak(p);
            }
        });
        if let Some(profile) = add
            && let Err(e) = ctl.add_instance(&profile)
        {
            ctl.refused = Some(e);
        }
        if let Some(id) = ctl.selected.clone()
            && ui
                .small_button("Remove")
                .on_hover_text("Remove this controller and its bindings")
                .clicked()
        {
            ctl.remove_instance(&id);
        }
    });
}

/// Press, toggle or set a position from a click, keeping what the engine refused.
pub(super) fn press_outcome(
    ctl: &mut ControllerLive,
    id: &str,
    control: &str,
    position: Option<&str>,
) {
    let action = match position {
        Some(p) => novo_controller::Action::SetPosition {
            control: control.to_owned(),
            position: p.to_owned(),
        },
        None => novo_controller::Action::Press {
            control: control.to_owned(),
        },
    };
    ctl.act(id, action);
}

fn effects_row(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str) {
    let Some(inst) = ctl.set.instance(id) else {
        return;
    };
    let (mut link, mut battery, mut power) = (
        inst.controller.link_ok(),
        inst.controller.battery_low(),
        inst.controller.powered(),
    );
    let is_radio =
        inst.controller.profile().controller.kind == novo_controller::ControllerKind::Radio;
    let (before_link, before_battery, before_power) = (link, battery, power);
    ui.horizontal_wrapped(|ui| {
        ui.checkbox(&mut power, "Powered");
        if is_radio {
            ui.checkbox(&mut link, "Link")
                .on_hover_text("Off: the radio link is lost and every signal goes low");
            ui.checkbox(&mut battery, "Battery low");
        }
    });
    use novo_controller::Action;
    if link != before_link {
        ctl.act(id, Action::Link { ok: link });
    }
    if battery != before_battery {
        ctl.act(id, Action::BatteryLow { low: battery });
    }
    if power != before_power {
        ctl.act(id, Action::Power { on: power });
    }
}

fn signals_block(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str, project: &Project) {
    let palette = crate::theme::ui_palette();
    let Some(inst) = ctl.set.instance(id) else {
        return;
    };
    let signals = inst.controller.signals();
    let high = signals.iter().filter(|s| s.is_high()).count();
    egui::CollapsingHeader::new(format!("Signals ({high} of {} high)", signals.len()))
        .id_salt(("controller_signals", id))
        .default_open(true)
        .show(ui, |ui| {
            let mut rows = Vec::new();
            for s in &signals {
                let key = SignalKey::new(id, &s.name);
                let targets: Vec<String> =
                    ctl.table.targets_of(&key).map(|t| t.to_string()).collect();
                rows.push((s.clone(), targets));
            }
            let _ = project;
            egui::ScrollArea::vertical()
                .max_height(150.0)
                .id_salt(("controller_signal_scroll", id))
                .show(ui, |ui| {
                    for (s, targets) in rows {
                        ui.horizontal(|ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                            let color = if s.is_high() {
                                palette.success
                            } else {
                                palette.border_control
                            };
                            ui.painter().circle_filled(rect.center(), 4.5, color);
                            ui.label(&s.name);
                            if s.kind == SignalKind::Analog {
                                ui.weak(format!("{:.2}", s.level));
                            }
                            if targets.is_empty() {
                                ui.weak("not bound");
                            } else {
                                ui.weak(format!("→ {}", targets.join(", ")));
                            }
                        });
                    }
                });
        });
}

fn bindings_block(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str, project: &Project) {
    let palette = crate::theme::ui_palette();
    egui::CollapsingHeader::new("Bindings")
        .id_salt(("controller_bindings", id))
        .default_open(true)
        .show(ui, |ui| {
            let rx = receivers(project);
            if rx.is_empty() {
                ui.add(
                    egui::Label::new(
                        "No receiver in the drawing. Place one (rx6, rx12) or bind to a pin, a \
                         hand device or a PLC input below.",
                    )
                    .wrap(),
                );
            }
            for tag in rx {
                ui.horizontal(|ui| {
                    ui.label(&tag);
                    if ui
                        .small_button("Bind by pin names")
                        .on_hover_text("Binds each signal to the receiver pin that has the signal's terminal name")
                        .clicked()
                    {
                        let (bound, missed) = ctl.auto_bind(project, id, &tag);
                        ctl.refused = Some(if missed.is_empty() {
                            format!("{bound} signals bound to {tag}")
                        } else {
                            format!(
                                "{bound} signals bound to {tag}; no contact for pin {}",
                                missed.join(", ")
                            )
                        });
                    }
                });
            }
            let signals: Vec<String> = ctl
                .set
                .instance(id)
                .map(|i| i.controller.profile().signals.iter().map(|s| s.name.clone()).collect())
                .unwrap_or_default();
            let pins = receiver_outputs(project);
            let hands = hand_devices(project);
            let mut changes: Vec<(SignalKey, Option<(Target, bool)>)> = Vec::new();
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .id_salt(("controller_bind_scroll", id))
                .show(ui, |ui| {
                    for name in &signals {
                        let key = SignalKey::new(id, name);
                        let current: Option<(Target, bool)> = ctl
                            .table
                            .bindings
                            .iter()
                            .find(|b| b.key == key)
                            .map(|b| (b.target.clone(), b.invert));
                        ui.horizontal(|ui| {
                            ui.label(name);
                            let text = current
                                .as_ref()
                                .map_or("choose…".to_owned(), |(t, inv)| {
                                    format!("{}{}", if *inv { "not " } else { "" }, short(t))
                                });
                            let state = current.as_ref().map(|(t, _)| ctl.resolution(project, t));
                            let mut chosen: Option<(Target, bool)> = None;
                            let mut unbind = false;
                            let color = match &state {
                                Some(Resolved::Broken(_)) => palette.error,
                                Some(_) => palette.text_primary,
                                None => palette.text_tertiary,
                            };
                            egui::ComboBox::from_id_salt(("controller_target", id, name))
                                .selected_text(egui::RichText::new(text).color(color))
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    if current.is_some() && ui.selectable_label(false, "unbind").clicked() {
                                        unbind = true;
                                    }
                                    if !pins.is_empty() {
                                        ui.weak("Receiver contacts");
                                    }
                                    for p in &pins {
                                        if ui.selectable_label(false, p).clicked() {
                                            chosen = Some((Target::Pin(p.clone()), false));
                                        }
                                    }
                                    if !hands.is_empty() {
                                        ui.weak("Hand devices (pressed while high)");
                                    }
                                    for h in &hands {
                                        if ui.selectable_label(false, h).clicked() {
                                            chosen = Some((Target::Symbol(h.clone()), false));
                                        }
                                        if ui
                                            .selectable_label(false, format!("{h} (inverted)"))
                                            .on_hover_text("Pressed while the signal is low, as an E-stop chain needs")
                                            .clicked()
                                        {
                                            chosen = Some((Target::Symbol(h.clone()), true));
                                        }
                                    }
                                });
                            if let Some(Resolved::Broken(why)) = &state {
                                ui.colored_label(palette.error, "!").on_hover_text(why);
                            }
                            if unbind {
                                changes.push((key.clone(), None));
                            }
                            if let Some(c) = chosen {
                                changes.push((key.clone(), Some(c)));
                            }
                        });
                    }
                });
            for (key, change) in changes {
                ctl.unbind(&key);
                if let Some((target, invert)) = change {
                    ctl.bind(key, target, invert);
                }
            }
            plc_row(ui, ctl, id);
        });
}

fn short(target: &Target) -> String {
    match target {
        Target::Pin(p) => p.clone(),
        Target::Symbol(s) => format!("{s} (hand)"),
        Target::PlcAddress(a) => a.clone(),
        Target::ModulePath(m) => m.clone(),
    }
}

/// A line to bind a signal to a PLC input bit or a module path by typing it.
fn plc_row(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str) {
    let signals: Vec<String> = ctl
        .set
        .instance(id)
        .map(|i| {
            i.controller
                .profile()
                .signals
                .iter()
                .map(|s| s.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let state_id = egui::Id::new(("controller_plc_row", id));
    let (mut signal, mut text): (usize, String) =
        ui.data(|d| d.get_temp(state_id)).unwrap_or_default();
    let mut bind = false;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt(("controller_plc_signal", id))
            .selected_text(signals.get(signal).cloned().unwrap_or_default())
            .width(90.0)
            .show_ui(ui, |ui| {
                for (i, s) in signals.iter().enumerate() {
                    ui.selectable_value(&mut signal, i, s);
                }
            });
        ui.add(
            egui::TextEdit::singleline(&mut text)
                .hint_text("%I0.1 or crane/hoist")
                .desired_width(90.0),
        );
        bind = ui.small_button("Bind").clicked();
    });
    if bind
        && let Some(name) = signals.get(signal)
        && !text.trim().is_empty()
    {
        let t = text.trim();
        let target = if t.starts_with('%') || novo_sim::program::Address::parse(t).is_some() {
            Target::PlcAddress(t.to_owned())
        } else {
            Target::ModulePath(t.to_owned())
        };
        ctl.bind(SignalKey::new(id, name), target, false);
        text.clear();
    }
    ui.data_mut(|d| d.insert_temp(state_id, (signal, text)));
}
