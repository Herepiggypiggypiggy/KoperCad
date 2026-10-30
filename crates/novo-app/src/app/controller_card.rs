//! The Controller card of the live view (lane C2, minimal panel; lane C3 builds the full one).
//!
//! Where the user finds it: Simulate > Controller panel (command `sim.controller`), or the
//! "Controller..." button on the Live potentials card. The card shows the selected controller
//! the way its profile describes it: buttons to hold, rockers with a zone per step, selectors,
//! the emergency stop with its reset, then a monitor of the signals, the bindings to the drawing
//! and the checks. It simulates wiring and logic; it says so on the card.

use eframe::egui;
use novo_controller::profile::split_group_step;
use novo_controller::{
    ControlDef, ControlKind, MachinePath, ModuleEntry, PortKind, Profile, Severity, SignalKey,
    SignalKind, Target,
};
use novo_core::Project;

use super::controller_live::{
    ControllerLive, HoldBy, PRESET_COLOURS, Resolved, Source, hand_devices, receiver_outputs,
    receivers,
};
use super::{Level, NovoApp};

/// What a press on the card asks the application to do.
enum CardAction {
    OpenWizard,
    Close,
    Pad(super::controller_pad::PadAction),
    /// Where the Gamepad section is drawn, for the lesson anchor.
    PadAnchor(egui::Rect),
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
        if !self.wb.live.on || !(self.wb.live.ctl.open || !self.wb.live.ctl.popped.is_empty()) {
            self.wb.live.ctl.pad_card_closed();
            return;
        }
        self.controller_keys(ctx);
        let Some(doc) = self.backend.document() else {
            return;
        };
        let project = doc.project();
        let ctl = &mut self.wb.live.ctl;
        ctl.sync(project);
        // The pad has the focus rules of a hand: a window in the background lets go.
        let focused = ctx.input(|i| i.focused) || crate::control::requested_port().is_some();
        ctl.pad_frame(focused);
        ctl.apply_holds();
        let mut actions = Vec::new();
        let card = if ctl.open {
            super::panel::Panel::new(
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
            .show(ctx, |ui| draw_card(ui, ctl, project, &mut actions))
        } else {
            None
        };
        popped_panels(ctx, canvas, ctl, project);
        if let Some(card) = card {
            self.anchor("live.controller", card.response.rect, "Controller card");
        }
        // The held controls change signals every frame they are held.
        if ctx.input(|i| i.pointer.any_down()) || self.wb.live.ctl.wants_frames() {
            ctx.request_repaint();
        }
        {
            let ctl = &mut self.wb.live.ctl;
            super::controller_pad::editor(ctx, ctl);
            super::controller_pad::wizard(ctx, ctl);
            ctl.pad_commit(ctx.input(|i| i.pointer.any_down()));
            if std::mem::take(&mut ctl.pad.seen) || ctl.pad.wants_frames() {
                ctx.request_repaint_after(std::time::Duration::from_millis(33));
            }
        }
        for action in actions {
            match action {
                CardAction::OpenWizard => self.open_controller_import(),
                CardAction::Close => self.wb.live.ctl.open = false,
                CardAction::Pad(super::controller_pad::PadAction::OpenEditor) => {
                    self.wb.live.ctl.pad.editor = true;
                }
                CardAction::Pad(super::controller_pad::PadAction::OpenWizard) => {
                    self.wb.live.ctl.pad_open_wizard();
                }
                CardAction::PadAnchor(rect) => {
                    self.anchor("live.controller.gamepad", rect, "Gamepad section");
                }
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
    let popped = ctl.popped.contains(&id);
    let map = key_map(&profile);
    if popped {
        ui.add(
            egui::Label::new("This controller has a panel of its own. Dock it to operate it here.")
                .wrap(),
        );
    } else {
        instance_controls(ui, ctl, &id, &profile);
    }
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
    if !popped {
        let pad = egui::CollapsingHeader::new("Gamepad")
            .id_salt(("controller_pad", &id))
            .show(ui, |ui| {
                let mut pad_actions = Vec::new();
                super::controller_pad::section(ui, ctl, &id, &profile, &mut pad_actions);
                actions.extend(pad_actions.into_iter().map(CardAction::Pad));
            });
        actions.push(CardAction::PadAnchor(pad.header_response.rect));
        effects_row(ui, ctl, &id);
    }
    ui.weak("Scroll for the signals, bindings and checks");
    ui.separator();
    signals_block(ui, ctl, &id, project);
    ui.separator();
    bindings_block(ui, ctl, &id, project);
    machine_block(ui, ctl, project);
    ownership_lines(ui, ctl, &id);
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

/// The instance tabs, the Add menu and the settings of the selected instance.
fn instance_row(ui: &mut egui::Ui, ctl: &mut ControllerLive, _actions: &mut Vec<CardAction>) {
    let palette = crate::theme::ui_palette();
    let ids: Vec<(String, String)> = ctl
        .set
        .instances()
        .iter()
        .map(|i| (i.id.clone(), i.label.clone()))
        .collect();
    ui.horizontal_wrapped(|ui| {
        for (id, label) in &ids {
            let colour = ctl.colour_of(id);
            let active = ctl.set.is_active(id);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                instance_dot(ui, colour, active);
                let popped = ctl.popped.contains(id);
                // A long name is cut so the tab and the Pop out mark fit the card; the hover has it all.
                let mut text = tab_text(label);
                if popped {
                    text.push_str(" ⧉");
                }
                let mut response = ui
                    .selectable_label(ctl.selected.as_deref() == Some(id), text)
                    .on_hover_text(label.as_str());
                if popped {
                    response = response.on_hover_text("In a panel of its own");
                }
                if response.clicked() {
                    ctl.selected = Some(id.clone());
                }
            });
        }
        let mut add: Option<novo_controller::Profile> = None;
        ui.menu_button("+ Add", |ui| {
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
    });
    let Some(id) = ctl.selected.clone() else {
        return;
    };
    if ctl.set.instance(&id).is_none() {
        return;
    }
    ui.horizontal(|ui| {
        let popped = ctl.popped.contains(&id);
        if ui
            .small_button(if popped { "Dock" } else { "Pop out" })
            .on_hover_text(if popped {
                "Show this controller in the card again"
            } else {
                "Give this controller a panel of its own, so several can be operated at once"
            })
            .clicked()
        {
            ctl.set_popped(&id, !popped);
        }
        if ui
            .small_button("Duplicate")
            .on_hover_text("A second controller of the same kind, with no bindings yet")
            .clicked()
            && let Err(e) = ctl.duplicate_instance(&id)
        {
            ctl.refused = Some(e);
        }
        if ui
            .small_button("Remove")
            .on_hover_text("Remove this controller and its bindings")
            .clicked()
        {
            ctl.remove_instance(&id);
        }
    });
    let Some(inst) = ctl.set.instance(&id) else {
        return;
    };
    let (label, group) = (inst.label.clone(), inst.target_group.clone());
    egui::CollapsingHeader::new("Name, colour and group")
        .id_salt(("controller_settings", &id))
        .show(ui, |ui| {
            let state_id = egui::Id::new(("controller_rename", &id));
            let mut name: String = ui.data(|d| d.get_temp(state_id)).unwrap_or(label);
            let edit =
                ui.add(egui::TextEdit::singleline(&mut name).desired_width(ui.available_width()));
            if edit.lost_focus() {
                ctl.rename_instance(&id, &name);
                ui.data_mut(|d| d.remove_temp::<String>(state_id));
            } else {
                ui.data_mut(|d| d.insert_temp(state_id, name));
            }
            ui.horizontal(|ui| {
                for (name, rgb) in PRESET_COLOURS {
                    let (rect, response) =
                        ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
                    let chosen = ctl.colour_of(&id) == rgb;
                    ui.painter().rect_filled(
                        rect.shrink(2.0),
                        3.0,
                        egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
                    );
                    if chosen {
                        ui.painter().rect_stroke(
                            rect,
                            3.0,
                            egui::Stroke::new(1.5, palette.text_primary),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if response.on_hover_text(name).clicked() {
                        let hex = format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]);
                        ctl.set_colour(&id, Some(&hex));
                    }
                }
            });
            let group_id = egui::Id::new(("controller_group", &id));
            let mut text: String = ui
                .data(|d| d.get_temp(group_id))
                .unwrap_or(group.unwrap_or_default());
            let edit = ui.add(
                egui::TextEdit::singleline(&mut text)
                    .hint_text("drives: crane 1, crab, hoist A")
                    .desired_width(ui.available_width()),
            );
            if edit.lost_focus() {
                ctl.set_target_group(&id, &text);
                ui.data_mut(|d| d.remove_temp::<String>(group_id));
            } else {
                ui.data_mut(|d| d.insert_temp(group_id, text));
            }
        });
}

/// Control rows of one instance, the same in the card and in a panel of its own.
fn instance_controls(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str, profile: &Profile) {
    let map = key_map(profile);
    let plan = novo_controller::panel::plan(profile);
    super::controller_widgets::draw_panel(ui, ctl, id, profile, &plan, &map);
}

/// Who else drives the targets of this instance, and who has them right now.
fn ownership_lines(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str) {
    let palette = crate::theme::ui_palette();
    let rows = ctl.shared_targets(id);
    if rows.is_empty() {
        return;
    }
    ui.separator();
    ui.label(egui::RichText::new("Shared targets").strong());
    for o in rows {
        let others: Vec<String> = o
            .controllers
            .iter()
            .filter(|c| *c != id)
            .map(|c| {
                ctl.set
                    .instance(c)
                    .map_or_else(|| c.clone(), |i| i.label.clone())
            })
            .collect();
        let now = if o.driving.is_empty() {
            "nobody is pressing".to_owned()
        } else {
            let who: Vec<String> = o
                .driving
                .iter()
                .map(|k| {
                    ctl.set
                        .instance(&k.controller)
                        .map_or_else(|| k.controller.clone(), |i| i.label.clone())
                })
                .collect();
            format!("{} driving", who.join(" and "))
        };
        let (color, note) = if o.contested {
            (palette.error, " (both pressing)")
        } else if o.conflict {
            (palette.warning, " (not marked shared)")
        } else {
            (palette.text_secondary, "")
        };
        ui.add(
            egui::Label::new(
                egui::RichText::new(format!(
                    "{} also driven by {}: {now}{note}",
                    short(&o.target),
                    others.join(", ")
                ))
                .color(color),
            )
            .wrap(),
        );
    }
}

/// One floating panel for each popped-out instance: its controls, who owns its shared targets and
/// its own findings. Closing the window docks the instance back into the card.
fn popped_panels(
    ctx: &egui::Context,
    canvas: egui::Rect,
    ctl: &mut ControllerLive,
    project: &Project,
) {
    let popped: Vec<String> = ctl
        .popped
        .iter()
        .filter(|id| ctl.set.instance(id).is_some())
        .cloned()
        .collect();
    ctl.popped.retain(|id| popped.contains(id));
    let findings = if popped.is_empty() {
        Vec::new()
    } else {
        ctl.findings(project)
    };
    for (n, id) in popped.iter().enumerate() {
        let Some(inst) = ctl.set.instance(id) else {
            continue;
        };
        let profile = inst.controller.profile().clone();
        let title = match &inst.target_group {
            Some(g) => format!("{} · {g}", inst.label),
            None => inst.label.clone(),
        };
        let mut open = true;
        let step = 28.0 * n as f32;
        let window_id = egui::Id::new(("controller_popout", id));
        super::panel::dialog(ctx, title, [290.0, 0.0])
            .id(super::panel::keep_id(ctx, window_id))
            .open(&mut open)
            .default_pos(super::panel::right_of_left_card(
                ctx,
                // Right of the Live potentials card, which sits at the top left of the canvas.
                canvas.left_top() + egui::vec2(310.0 + step, 70.0 + step),
            ))
            .show(ctx, |ui| {
                let palette = crate::theme::ui_palette();
                ui.set_max_width(280.0);
                ui.horizontal(|ui| {
                    instance_dot(ui, ctl.colour_of(id), ctl.set.is_active(id));
                    let mine = ctl.selected.as_deref() == Some(id.as_str()) && ctl.keys;
                    if ui
                        .selectable_label(mine, "Keyboard here")
                        .on_hover_text("The keyboard drives this controller")
                        .clicked()
                    {
                        ctl.selected = Some(id.clone());
                        ctl.keys = !mine;
                    }
                });
                instance_controls(ui, ctl, id, &profile);
                if let Some(why) = ctl.refused.clone() {
                    ui.colored_label(palette.warning, why);
                }
                effects_row(ui, ctl, id);
                ownership_lines(ui, ctl, id);
                for f in findings
                    .iter()
                    .filter(|f| f.key.as_ref().is_some_and(|k| k.controller == *id))
                {
                    let color = match f.severity {
                        Severity::Error => palette.error,
                        Severity::Warning => palette.warning,
                    };
                    ui.add(egui::Label::new(egui::RichText::new(&f.text).color(color)).wrap());
                }
            });
        if !open {
            ctl.set_popped(id, false);
        }
    }
}

/// The most of a name a tab shows.
const TAB_CHARS: usize = 16;

pub(super) fn tab_text(label: &str) -> String {
    if label.chars().count() <= TAB_CHARS {
        label.to_owned()
    } else {
        let cut: String = label.chars().take(TAB_CHARS - 1).collect();
        format!("{}…", cut.trim_end())
    }
}

/// A dot in the colour of the instance, ringed while a control is held away from rest.
fn instance_dot(ui: &mut egui::Ui, rgb: [u8; 3], active: bool) {
    let palette = crate::theme::ui_palette();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
    ui.painter().circle_filled(
        rect.center(),
        4.5,
        egui::Color32::from_rgb(rgb[0], rgb[1], rgb[2]),
    );
    if active {
        ui.painter()
            .circle_stroke(rect.center(), 6.5, egui::Stroke::new(1.5, palette.success));
    }
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
            let modules = ctl.catalog(project).modules.clone();
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
                                    machine_picker(ui, &modules, &mut chosen);
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
            plc_row(ui, ctl, id, project);
        });
}

/// The machine part of the target picker: one fold per module with its ports. A chain port also
/// comes inverted, for a signal that is high while the E-stop is pressed.
fn machine_picker(ui: &mut egui::Ui, modules: &[ModuleEntry], chosen: &mut Option<(Target, bool)>) {
    if modules.is_empty() {
        return;
    }
    ui.separator();
    ui.weak("Machine ports (drive Simulate > Hoist run or Drive run)");
    for m in modules {
        ui.collapsing(&m.label, |ui| {
            for p in &m.ports {
                let Ok(path) = MachinePath::new(&m.path, &p.name) else {
                    continue;
                };
                let target = Target::machine(&path);
                let kind = match p.kind {
                    PortKind::Command => "",
                    PortKind::Chain => " (chain: low stops)",
                    PortKind::Enable => " (hold to move)",
                };
                if ui
                    .selectable_label(false, format!("{}{kind}", p.label))
                    .on_hover_text(path.to_string())
                    .clicked()
                {
                    *chosen = Some((target.clone(), false));
                }
                if p.kind != PortKind::Command
                    && ui
                        .selectable_label(false, format!("{} (inverted)", p.label))
                        .on_hover_text("For a signal that is high while the stop is pressed")
                        .clicked()
                {
                    *chosen = Some((target, true));
                }
            }
        });
    }
}

/// Which module each controller signal drives, and what the machine ports are good for.
fn machine_block(ui: &mut egui::Ui, ctl: &mut ControllerLive, project: &Project) {
    let drivers = ctl.machine_drivers();
    let palette = crate::theme::ui_palette();
    egui::CollapsingHeader::new("Machine modules")
        .id_salt("controller_machine_modules")
        .default_open(!drivers.is_empty())
        .show(ui, |ui| {
            if drivers.is_empty() {
                ui.add(
                    egui::Label::new(
                        "No signal is bound to a machine port. Pick one in the binding list \
                         (hoist up, down, fast; travel forward, reverse, fast; permit for an \
                         E-stop chain; dead-man).",
                    )
                    .wrap(),
                );
            }
            let modules = ctl.catalog(project).modules.clone();
            for (id, list) in &drivers {
                let path = modules
                    .iter()
                    .find(|m| m.path.rsplit('/').next() == Some(id.as_str()))
                    .map_or(id.clone(), |m| m.path.clone());
                ui.label(egui::RichText::new(path).strong());
                for (port, key, high) in list {
                    ui.horizontal(|ui| {
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                        let color = if *high {
                            palette.success
                        } else {
                            palette.border_control
                        };
                        ui.painter().circle_filled(rect.center(), 4.5, color);
                        ui.label(format!("{key} \u{2192} {}", port.name()));
                    });
                }
            }
            ui.add(
                egui::Label::new(
                    egui::RichText::new(
                        "The machine reads these ports while Simulate > Hoist run or Drive run \
                         is running with Live on. A bound port replaces the drawn input for \
                         that function, so a wiring fault on it is not simulated; bind a \
                         receiver pin instead to test the wiring. This simulates wiring and \
                         logic. It is not a safety function and gives no PL, SIL or \
                         compliance claim.",
                    )
                    .weak(),
                )
                .wrap(),
            );
        });
}

fn short(target: &Target) -> String {
    match target {
        Target::Pin(p) => p.clone(),
        Target::Symbol(s) => format!("{s} (hand)"),
        Target::PlcAddress(a) => a.clone(),
        Target::ModulePath(m) => format!("{m} (machine)"),
    }
}

/// A line to bind a signal to a PLC input bit or a module path by typing it.
fn plc_row(ui: &mut egui::Ui, ctl: &mut ControllerLive, id: &str, project: &Project) {
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
                .hint_text("%I0.1 or hoist/=A-M1@up")
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
            // A machine path is stored in the spelling of the machine model when it resolves.
            MachinePath::parse(t)
                .ok()
                .and_then(|p| ctl.catalog(project).canonical(&p))
                .map_or_else(|| Target::ModulePath(t.to_owned()), |p| Target::machine(&p))
        };
        ctl.bind(SignalKey::new(id, name), target, false);
        text.clear();
    }
    ui.data_mut(|d| d.insert_temp(state_id, (signal, text)));
}
