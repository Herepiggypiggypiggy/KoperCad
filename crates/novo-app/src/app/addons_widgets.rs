//! The widgets of add-on API 2 in a declared panel (lane C3): rocker, joystick, selector,
//! key map, table, tabs and bar. An add-on only says what is on the panel (`novo_addons::layout`);
//! this file draws it with the program's own painters (`controller_widgets`, the same ones the
//! Controller card uses) in the theme palette, and turns what the person does into the button
//! ids the add-on's engine already understands (`set-hoist-up2`, `stick-x-right1`, `sel-mast`).
//!
//! Where the user finds it: any installed add-on that declares a panel with these kinds, for
//! example Simulate > Radio panel of the Controllers add-on (File > Add-ons... to install it).

use eframe::egui::{self, Rect, Sense, Vec2};
use novo_addons::layout::{
    Axis, Direction, KeyRow, Role, SelectorOption, Tab, Widget, detent_at, detent_offset,
};
use novo_controller::panel::zone_at;

use super::addons::{PanelEnv, widget_ui};
use super::controller_widgets::{
    PAD, PadAxis, STACK_W, ZONE_H, key_grid, pad_travel, paint_knob, paint_pad, paint_zone,
    zone_cell,
};

/// Draws a widget of API 2. The ids to send to the engine go to `clicked`.
pub(super) fn new_widget_ui(
    ui: &mut egui::Ui,
    w: &Widget,
    env: &PanelEnv,
    clicked: &mut Vec<String>,
) {
    match w {
        Widget::Rocker {
            id,
            text,
            steps,
            spring,
            plus,
            minus,
            hint,
        } => rocker(
            ui,
            env,
            clicked,
            id,
            text,
            *steps,
            *spring,
            [minus, plus],
            hint,
        ),
        Widget::Joystick {
            id,
            text,
            axes,
            spring,
            hint,
        } => joystick(ui, env, clicked, id, text, axes, *spring, hint),
        Widget::Selector {
            id,
            text,
            options,
            rotary,
            hint,
        } => selector(ui, env, clicked, id, text, options, *rotary, hint),
        Widget::Keymap { text, rows } => keymap(ui, env, text, rows),
        Widget::Table {
            text,
            columns,
            rows,
        } => table(ui, env, text, columns, rows),
        Widget::Tabs { tabs } => tabs_ui(ui, env, clicked, tabs),
        Widget::Bar {
            id,
            text,
            min,
            max,
            value,
            unit,
            state,
        } => bar(ui, env, id, text, *min, *max, *value, unit, *state),
        _ => {}
    }
}

/// The egui memory key of one widget of one panel.
fn key(env: &PanelEnv, id: &str) -> egui::Id {
    egui::Id::new(("addon_widget", &env.panel, id))
}

fn title(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).strong());
}

/// What a widget that needs an engine says when there is none.
fn tip(env: &PanelEnv, hint: &str, fallback: &str) -> String {
    if !env.has_engine {
        "This add-on has no engine to answer this control".to_owned()
    } else if hint.is_empty() {
        fallback.to_owned()
    } else {
        hint.to_owned()
    }
}

#[allow(clippy::too_many_arguments)]
fn rocker(
    ui: &mut egui::Ui,
    env: &PanelEnv,
    clicked: &mut Vec<String>,
    id: &str,
    text: &str,
    steps: u8,
    spring: bool,
    dirs: [&Direction; 2],
    hint: &str,
) {
    title(ui, text);
    let mem = key(env, id);
    let was: String = ui.data(|d| d.get_temp(mem)).unwrap_or_default();
    // What the person points at this frame (spring) or clicks (latching).
    let mut pointed: Option<String> = None;
    let mut toggled: Option<String> = None;
    ui.add_enabled_ui(env.has_engine, |ui| {
        ui.horizontal_top(|ui| {
            for dir in dirs {
                let n = steps as usize;
                let header = 18.0;
                let size = Vec2::new(STACK_W, header + n as f32 * ZONE_H);
                let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
                let zones = Rect::from_min_max(
                    egui::pos2(rect.left(), rect.top() + header),
                    rect.right_bottom(),
                );
                let event = |zone: usize| format!("{id}-{}{}", dir.name, zone + 1);
                let zone_under = ui
                    .input(|i| i.pointer.interact_pos())
                    .map(|pos| zone_at((pos.y - zones.top()) / zones.height().max(1.0), n));
                if let Some(zone) = zone_under {
                    if response.is_pointer_button_down_on() {
                        pointed = Some(event(zone));
                    }
                    if response.clicked() {
                        toggled = Some(event(zone));
                    }
                }
                let painter = ui.painter();
                let p = crate::theme::ui_palette();
                painter.text(
                    egui::pos2(rect.left() + 2.0, rect.top() + header / 2.0),
                    egui::Align2::LEFT_CENTER,
                    &dir.text,
                    egui::FontId::proportional(12.0),
                    p.text_secondary,
                );
                let at_step = (0..n).find(|z| was == event(*z)).map_or(0, |z| z + 1);
                for z in 0..n {
                    let step = (z + 1) as u8;
                    let held = was == event(z);
                    // A deeper press keeps the lower contacts closed, as on a two-step switch.
                    let closed = !held && at_step > z + 1;
                    paint_zone(
                        painter,
                        zone_cell(zones, z),
                        step,
                        &step_word(step, steps),
                        held,
                        closed,
                        response.hovered(),
                    );
                }
                response.on_hover_text(tip(
                    env,
                    hint,
                    if spring {
                        "Hold. Press the top zone for step 1, slide down for step 2"
                    } else {
                        "Click to set, click again for off"
                    },
                ));
            }
        });
    });
    let now = if spring {
        pointed.unwrap_or_default()
    } else {
        match toggled {
            Some(t) if t == was => String::new(),
            Some(t) => t,
            None => was.clone(),
        }
    };
    if now != was {
        clicked.push(if now.is_empty() {
            format!("{id}-off")
        } else {
            now.clone()
        });
        ui.data_mut(|d| d.insert_temp(mem, now));
    }
}

/// A word for a step: slow and fast on a two-step button, the number otherwise.
fn step_word(step: u8, steps: u8) -> String {
    match (steps, step) {
        (2, 1) => "Step 1 (slow)".into(),
        (2, 2) => "Step 2 (fast)".into(),
        _ => format!("Step {step}"),
    }
}

#[allow(clippy::too_many_arguments)]
fn joystick(
    ui: &mut egui::Ui,
    env: &PanelEnv,
    clicked: &mut Vec<String>,
    id: &str,
    text: &str,
    axes: &[Axis],
    spring: bool,
    hint: &str,
) {
    title(ui, text);
    let Some(x) = axes.first() else {
        return;
    };
    let y = axes.get(1);
    let mem = key(env, id);
    let was: [i8; 2] = ui.data(|d| d.get_temp(mem)).unwrap_or([0, 0]);
    let mut now = was;
    ui.add_enabled_ui(env.has_engine, |ui| {
        let size = if y.is_some() {
            Vec2::splat(PAD)
        } else {
            Vec2::new(PAD, 44.0)
        };
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let usable = pad_travel(rect);
        if response.is_pointer_button_down_on()
            && let Some(pos) = ui.input(|i| i.pointer.interact_pos())
        {
            let vx = (pos.x - rect.center().x) / usable.x.max(1.0);
            let vy = -(pos.y - rect.center().y) / usable.y.max(1.0);
            now[0] = detent_at(vx, x.detents);
            if let Some(y) = y {
                now[1] = detent_at(vy, y.detents);
            }
        } else if spring {
            now = [0, 0];
        }
        let pad = |a: &Axis| PadAxis {
            dots: (1..=a.detents as i8)
                .flat_map(|k| [detent_offset(-k, a.detents), detent_offset(k, a.detents)])
                .collect(),
            minus: a.minus.text.clone(),
            plus: a.plus.text.clone(),
        };
        paint_pad(
            ui.painter(),
            rect,
            &pad(x),
            y.map(pad).as_ref(),
            (
                detent_offset(now[0], x.detents),
                y.map_or(0.0, |y| detent_offset(now[1], y.detents)),
            ),
        );
        response.on_hover_text(tip(
            env,
            hint,
            if spring {
                "Drag the knob to a detent. It springs back when you let go"
            } else {
                "Drag the knob to a detent. It stays where you leave it"
            },
        ));
    });
    for (i, axis) in axes.iter().enumerate() {
        if now[i] != was[i] {
            clicked.push(joystick_event(id, axis, now[i]));
        }
    }
    if now != was {
        ui.data_mut(|d| d.insert_temp(mem, now));
    }
}

/// The id sent when an axis moves to detent `k` (0 is the middle).
fn joystick_event(id: &str, axis: &Axis, k: i8) -> String {
    match k {
        0 => format!("{id}-{}-off", axis.name),
        k if k > 0 => format!("{id}-{}-{}{k}", axis.name, axis.plus.name),
        k => format!("{id}-{}-{}{}", axis.name, axis.minus.name, -k),
    }
}

#[allow(clippy::too_many_arguments)]
fn selector(
    ui: &mut egui::Ui,
    env: &PanelEnv,
    clicked: &mut Vec<String>,
    id: &str,
    text: &str,
    options: &[SelectorOption],
    rotary: bool,
    hint: &str,
) {
    title(ui, text);
    let mem = key(env, id);
    let was: usize = ui.data(|d| d.get_temp(mem)).unwrap_or(0);
    let was = was.min(options.len().saturating_sub(1));
    let mut now = was;
    ui.add_enabled_ui(env.has_engine, |ui| {
        ui.horizontal(|ui| {
            if rotary {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::click());
                paint_knob(ui.painter(), rect.center(), options.len(), was);
                if response.clicked() {
                    now = (was + 1) % options.len().max(1);
                }
                response.on_hover_text(tip(env, hint, "Click to turn to the next position"));
                ui.vertical(|ui| {
                    for (i, o) in options.iter().enumerate() {
                        if ui.selectable_label(was == i, &o.text).clicked() {
                            now = i;
                        }
                    }
                });
            } else {
                for (i, o) in options.iter().enumerate() {
                    if ui
                        .selectable_label(was == i, &o.text)
                        .on_hover_text(tip(env, hint, &o.text))
                        .clicked()
                    {
                        now = i;
                    }
                }
            }
        });
    });
    if now != was
        && let Some(o) = options.get(now)
    {
        clicked.push(format!("{id}-{}", o.name));
        ui.data_mut(|d| d.insert_temp(mem, now));
    }
}

fn keymap(ui: &mut egui::Ui, env: &PanelEnv, text: &str, rows: &[KeyRow]) {
    if !text.is_empty() {
        title(ui, text);
    }
    let rows: Vec<(String, String)> = rows
        .iter()
        .map(|r| (r.key.clone(), r.text.clone()))
        .collect();
    key_grid(ui, ("addon_keymap", &env.panel, text, rows.len()), &rows);
}

fn table(ui: &mut egui::Ui, env: &PanelEnv, text: &str, columns: &[String], rows: &[Vec<String>]) {
    if !text.is_empty() {
        title(ui, text);
    }
    egui::Grid::new(("addon_table", &env.panel, text, columns))
        .striped(true)
        .spacing([12.0, 2.0])
        .show(ui, |ui| {
            for c in columns {
                ui.label(egui::RichText::new(c).strong());
            }
            ui.end_row();
            for row in rows {
                for cell in row {
                    ui.label(cell);
                }
                ui.end_row();
            }
        });
}

fn tabs_ui(ui: &mut egui::Ui, env: &PanelEnv, clicked: &mut Vec<String>, tabs: &[Tab]) {
    let mem = key(env, tabs.first().map_or("tabs", |t| t.id.as_str()));
    let shown: String = ui.data(|d| d.get_temp(mem)).unwrap_or_default();
    let current = tabs.iter().position(|t| t.id == shown).unwrap_or_default();
    let mut pick = current;
    ui.horizontal_wrapped(|ui| {
        for (i, t) in tabs.iter().enumerate() {
            if ui.selectable_label(i == current, &t.text).clicked() {
                pick = i;
            }
        }
    });
    if pick != current {
        let id = tabs[pick].id.clone();
        ui.data_mut(|d| d.insert_temp(mem, id));
    }
    ui.separator();
    for w in &tabs[pick].widgets {
        widget_ui(ui, w, env, clicked);
    }
}

#[allow(clippy::too_many_arguments)]
fn bar(
    ui: &mut egui::Ui,
    env: &PanelEnv,
    id: &str,
    text: &str,
    min: f64,
    max: f64,
    value: f64,
    unit: &str,
    state: Role,
) {
    let p = crate::theme::ui_palette();
    let value = env.bars.get(id).copied().unwrap_or(value);
    let frac = ((value - min) / (max - min)).clamp(0.0, 1.0) as f32;
    let color = match state {
        Role::Neutral => p.text_secondary,
        Role::Accent => p.accent,
        Role::Ok => p.success,
        Role::Warn => p.warning,
        Role::Error => p.error,
        Role::Info => p.info,
    };
    // The number is always in the text: never the length of a bar alone.
    let words = if unit.is_empty() {
        format!("{text}: {value:.1}")
    } else {
        format!("{text}: {value:.1} {unit}")
    };
    ui.add(
        egui::ProgressBar::new(frac)
            .fill(color)
            .desired_width(240.0)
            .text(words),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn axis() -> Axis {
        Axis {
            name: "x".into(),
            plus: Direction {
                name: "right".into(),
                text: "Right".into(),
            },
            minus: Direction {
                name: "left".into(),
                text: "Left".into(),
            },
            detents: 2,
        }
    }

    #[test]
    fn joystick_events_name_axis_direction_and_detent() {
        let a = axis();
        assert_eq!(joystick_event("stick", &a, 0), "stick-x-off");
        assert_eq!(joystick_event("stick", &a, 2), "stick-x-right2");
        assert_eq!(joystick_event("stick", &a, -1), "stick-x-left1");
    }

    #[test]
    fn step_words() {
        assert_eq!(step_word(1, 2), "Step 1 (slow)");
        assert_eq!(step_word(2, 3), "Step 2");
    }

    // ------------------------------------------------------------------
    // Headless: draw a layout, point at it, see which ids it sends.

    use std::collections::{BTreeMap, BTreeSet};

    use novo_addons::layout::PanelLayout;

    fn env(has_engine: bool) -> PanelEnv {
        PanelEnv {
            has_engine,
            lamps: BTreeMap::new(),
            bars: BTreeMap::new(),
            panel: "addon.t.p".into(),
        }
    }

    fn pointer(pos: egui::Pos2, pressed: Option<bool>) -> Vec<egui::Event> {
        let mut events = vec![egui::Event::PointerMoved(pos)];
        if let Some(pressed) = pressed {
            events.push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            });
        }
        events
    }

    /// One frame; returns the ids sent and every text painted.
    fn frame(
        ctx: &egui::Context,
        layout: &PanelLayout,
        env: &PanelEnv,
        events: Vec<egui::Event>,
    ) -> (Vec<String>, String) {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(420.0, 700.0),
            )),
            events,
            ..Default::default()
        };
        let mut clicked = Vec::new();
        let mut out = ctx.run_ui(input, |ui| {
            for w in &layout.widgets {
                widget_ui(ui, w, env, &mut clicked);
            }
        });
        out.textures_delta.clear();
        let mut text = String::new();
        for clipped in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &clipped.shape {
                text.push_str(t.galley.text());
                text.push('\n');
            }
        }
        (clicked, text)
    }

    fn parse(text: &str) -> PanelLayout {
        PanelLayout::parse(text.as_bytes()).expect("layout parses")
    }

    /// Every id that pressing, holding and letting go anywhere on a grid sends.
    fn reach(layout: &PanelLayout, env: &PanelEnv) -> BTreeSet<String> {
        let ctx = egui::Context::default();
        let mut found = BTreeSet::new();
        frame(&ctx, layout, env, vec![]);
        for y in (0..300).step_by(7) {
            for x in (0..300).step_by(11) {
                let pos = egui::pos2(x as f32, y as f32);
                for events in [
                    pointer(pos, None),
                    pointer(pos, Some(true)),
                    pointer(pos, Some(false)),
                ] {
                    found.extend(frame(&ctx, layout, env, events).0);
                }
            }
        }
        found
    }

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|s| s.to_string()).collect()
    }

    const ROCKER: &str = "[[widget]]\nkind = \"rocker\"\nid = \"set-hoist\"\ntext = \"Hoist\"\nsteps = 2\nspring = true\n";

    #[test]
    fn a_spring_rocker_reaches_every_step_and_lets_go() {
        let found = reach(&parse(ROCKER), &env(true));
        assert_eq!(
            found,
            set(&[
                "set-hoist-up1",
                "set-hoist-up2",
                "set-hoist-down1",
                "set-hoist-down2",
                "set-hoist-off"
            ])
        );
    }

    #[test]
    fn a_latching_rocker_stays_and_a_second_click_sets_it_off() {
        let layout = parse(&ROCKER.replace("spring = true", "spring = false"));
        let found = reach(&layout, &env(true));
        assert!(found.contains("set-hoist-up2") && found.contains("set-hoist-down1"));
        // Let go does not send off by itself: click the same zone twice for that.
        let ctx = egui::Context::default();
        let e = env(true);
        frame(&ctx, &layout, &e, vec![]);
        // Find a point that sets a step, by sweeping, then click it again.
        let mut spot = None;
        'sweep: for y in (0..300).step_by(7) {
            for x in (0..300).step_by(11) {
                let pos = egui::pos2(x as f32, y as f32);
                frame(&ctx, &layout, &e, pointer(pos, None));
                frame(&ctx, &layout, &e, pointer(pos, Some(true)));
                let (sent, _) = frame(&ctx, &layout, &e, pointer(pos, Some(false)));
                if sent.iter().any(|s| s.starts_with("set-hoist-up")) {
                    spot = Some((pos, sent));
                    break 'sweep;
                }
            }
        }
        let (pos, first) = spot.expect("a zone was clicked");
        assert_eq!(first.len(), 1);
        frame(&ctx, &layout, &e, pointer(pos, Some(true)));
        let (again, _) = frame(&ctx, &layout, &e, pointer(pos, Some(false)));
        assert_eq!(again, vec!["set-hoist-off".to_owned()]);
    }

    #[test]
    fn labelled_directions_name_the_ids_and_the_zones() {
        let layout = parse(
            "[[widget]]\nkind = \"rocker\"\nid = \"set-bridge\"\ntext = \"Bridge\"\nsteps = 3\nspring = true\nplus = { name = \"fwd\", text = \"Forward\" }\nminus = { name = \"rev\", text = \"Reverse\" }\n",
        );
        let (_, text) = frame(&egui::Context::default(), &layout, &env(true), vec![]);
        assert!(
            text.contains("Forward") && text.contains("Reverse"),
            "{text}"
        );
        assert!(text.contains("Step 3"), "{text}");
        let found = reach(&layout, &env(true));
        assert!(found.contains("set-bridge-fwd3") && found.contains("set-bridge-rev1"));
    }

    #[test]
    fn controls_do_nothing_without_an_engine() {
        for text in [
            ROCKER,
            "[[widget]]\nkind = \"selector\"\nid = \"s\"\ntext = \"S\"\noptions = [{ name = \"a\", text = \"A\" }, { name = \"b\", text = \"B\" }]\n",
            "[[widget]]\nkind = \"joystick\"\nid = \"j\"\ntext = \"J\"\naxes = [{ name = \"x\", plus = { name = \"r\", text = \"R\" }, minus = { name = \"l\", text = \"L\" } }]\n",
        ] {
            assert!(reach(&parse(text), &env(false)).is_empty(), "{text}");
        }
    }

    #[test]
    fn a_selector_sends_its_option_and_a_rotary_turns_on() {
        for rotary in ["false", "true"] {
            let layout = parse(&format!(
                "[[widget]]\nkind = \"selector\"\nid = \"set-hoist-sel\"\ntext = \"Hoist select\"\nrotary = {rotary}\noptions = [{{ name = \"main\", text = \"Main hoist\" }}, {{ name = \"mast\", text = \"Mast hoist\" }}]\n"
            ));
            let found = reach(&layout, &env(true));
            assert!(
                found.contains("set-hoist-sel-mast"),
                "rotary {rotary}: {found:?}"
            );
        }
    }

    #[test]
    fn a_joystick_reaches_its_detents_and_springs_back() {
        let layout = parse(
            "[[widget]]\nkind = \"joystick\"\nid = \"stick\"\ntext = \"Stick\"\naxes = [{ name = \"x\", detents = 2, plus = { name = \"right\", text = \"Right\" }, minus = { name = \"left\", text = \"Left\" } }, { name = \"y\", plus = { name = \"up\", text = \"Up\" }, minus = { name = \"down\", text = \"Down\" } }]\n",
        );
        let found = reach(&layout, &env(true));
        for want in [
            "stick-x-right1",
            "stick-x-right2",
            "stick-x-left1",
            "stick-x-left2",
            "stick-y-up1",
            "stick-y-down1",
            "stick-x-off",
            "stick-y-off",
        ] {
            assert!(found.contains(want), "{want} not reached: {found:?}");
        }
        assert!(!found.contains("stick-y-up2"), "one detent on y");
    }

    #[test]
    fn tabs_show_one_tab_and_switch() {
        let layout = parse(
            "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"one\", text = \"First tab\", widgets = [{ kind = \"button\", id = \"a\", text = \"Button A\" }] }, { id = \"two\", text = \"Second tab\", widgets = [{ kind = \"button\", id = \"b\", text = \"Button B\" }] }]\n",
        );
        let ctx = egui::Context::default();
        let (_, text) = frame(&ctx, &layout, &env(true), vec![]);
        assert!(
            text.contains("Button A") && !text.contains("Button B"),
            "{text}"
        );
        assert!(text.contains("First tab") && text.contains("Second tab"));
        let found = reach(&layout, &env(true));
        // The sweep clicks the second tab header on its way and stays there, so only the second
        // tab's button is reached afterwards: switching worked.
        assert!(found.contains("b"), "{found:?}");
    }

    #[test]
    fn key_map_table_and_bar_draw_their_text() {
        let layout = parse(
            r#"
[[widget]]
kind = "keymap"
text = "Keyboard"
rows = [{ key = "Space", text = "Main latch" }]
[[widget]]
kind = "table"
text = "Contacts"
columns = ["Position", "Slow"]
rows = [["Step 1", "closed"]]
[[widget]]
kind = "bar"
id = "speed"
text = "Speed"
min = 0.0
max = 50.0
value = 10.0
unit = "Hz"
"#,
        );
        let ctx = egui::Context::default();
        let (_, text) = frame(&ctx, &layout, &env(true), vec![]);
        for want in [
            "Keyboard",
            "Space",
            "Main latch",
            "Contacts",
            "Position",
            "Step 1",
            "closed",
            "Speed: 10.0 Hz",
        ] {
            assert!(text.contains(want), "{want} in {text}");
        }
        // The engine sets the value; a value outside the range is clamped for the bar, and the
        // number stays in the text.
        let mut e = env(true);
        e.bars.insert("speed".into(), 12.5);
        let (_, text) = frame(&ctx, &layout, &e, vec![]);
        assert!(text.contains("Speed: 12.5 Hz"), "{text}");
        e.bars.insert("speed".into(), 1e9);
        let (_, text) = frame(&ctx, &layout, &e, vec![]);
        assert!(text.contains("Speed: 1000000000.0 Hz"), "{text}");
    }
}
