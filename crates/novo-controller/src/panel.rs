//! The panel plan: which widget draws which control (lane C3).
//!
//! A profile says what the controls are; the plan says how the host draws them, the way the
//! real device looks: a pendant or radio is a column of two-step push-button pairs with a
//! mushroom and a main/start, a joystick box is a pad with detents, a selector is a rotary.
//! The plan is pure data derived from the [`Profile`], so the same profile always draws the same
//! panel, the host needs no code per controller, and a profile made by the import wizard gets
//! the right widgets with nothing added. The application only draws a plan; the geometry that
//! decides which position a pointer means lives here, where it is tested.

use crate::profile::{ControlDef, ControlKind, ControllerKind, Profile, split_group_step};

/// How the panel is arranged, from the kind of device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Template {
    /// Radio transmitter or wired pendant: push-button pairs in a column.
    Pendant,
    /// Joystick box: pads first.
    Joystick,
    /// Plain list of controls (PLC test panel, custom).
    List,
}

impl Template {
    pub fn of(kind: ControllerKind) -> Template {
        match kind {
            ControllerKind::Radio | ControllerKind::Pendant => Template::Pendant,
            ControllerKind::Joystick => Template::Joystick,
            ControllerKind::PlcPanel | ControllerKind::Custom => Template::List,
        }
    }
}

/// One direction of a motion or axis: the positions that go that way, nearest rest first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Direction {
    /// `up`, `fwd`, `left`... or the position itself when it has no step number.
    pub name: String,
    /// `(step, position)`, step 1 first.
    pub steps: Vec<(u8, String)>,
}

/// What a plain button does, for its look and place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonRole {
    Plain,
    Stop,
    Horn,
    Deadman,
}

/// One axis of a joystick pad.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickAxis {
    pub control: String,
    pub label: String,
    /// Detents from the negative end (left, down) through rest to the positive end (right, up),
    /// as positions. The rest position is in the middle.
    pub detents: Vec<String>,
    /// Index of the rest detent in `detents`.
    pub rest: usize,
    /// Springs back to rest when released.
    pub spring: bool,
}

impl StickAxis {
    /// The detent a pointer at `value` (-1 at the negative end, +1 at the positive end) means.
    /// A pad with `n` detents each side puts the boundaries half way between them.
    pub fn detent_at(&self, value: f32) -> &str {
        let value = if value.is_finite() {
            value.clamp(-1.0, 1.0)
        } else {
            0.0
        };
        let below = self.rest;
        let above = self.detents.len() - 1 - self.rest;
        let steps = if value < 0.0 { below } else { above } as f32;
        let n = (value.abs() * steps).round() as usize;
        let index = if value < 0.0 {
            self.rest - n.min(below)
        } else {
            self.rest + n.min(above)
        };
        &self.detents[index]
    }

    /// Where a detent sits on the axis, -1 to +1.
    pub fn offset_of(&self, position: &str) -> f32 {
        let Some(i) = self.detents.iter().position(|d| d == position) else {
            return 0.0;
        };
        if i < self.rest {
            -((self.rest - i) as f32) / self.rest as f32
        } else if i > self.rest {
            (i - self.rest) as f32 / (self.detents.len() - 1 - self.rest) as f32
        } else {
            0.0
        }
    }
}

/// A widget of the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Widget {
    /// The latching mushroom with its twist reset.
    EStop { control: String, label: String },
    /// Main/start, a light or a key switch: click toggles.
    Latch {
        control: String,
        label: String,
        key: bool,
    },
    /// A push button held while the pointer is down.
    Button {
        control: String,
        label: String,
        position: String,
        role: ButtonRole,
    },
    /// A motion with two-step (or three-step) push buttons, one per direction.
    Motion {
        control: String,
        label: String,
        spring: bool,
        directions: Vec<Direction>,
    },
    /// A joystick pad of one or two axes with detents.
    Stick {
        name: String,
        label: String,
        /// Left-right first when there are two.
        axes: Vec<StickAxis>,
    },
    /// A rotary: stays where it is.
    Selector {
        control: String,
        label: String,
        positions: Vec<String>,
    },
}

/// A named group of widgets on the panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub title: &'static str,
    pub widgets: Vec<Widget>,
}

/// Everything the host needs to draw a profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelPlan {
    pub template: Template,
    pub sections: Vec<Section>,
}

impl PanelPlan {
    pub fn widgets(&self) -> impl Iterator<Item = &Widget> {
        self.sections.iter().flat_map(|s| s.widgets.iter())
    }

    /// The control ids the plan draws; every control of the profile is drawn exactly once.
    pub fn controls(&self) -> Vec<&str> {
        let mut out = Vec::new();
        for w in self.widgets() {
            match w {
                Widget::EStop { control, .. }
                | Widget::Latch { control, .. }
                | Widget::Button { control, .. }
                | Widget::Motion { control, .. }
                | Widget::Selector { control, .. } => out.push(control.as_str()),
                Widget::Stick { axes, .. } => out.extend(axes.iter().map(|a| a.control.as_str())),
            }
        }
        out
    }
}

/// Which of `n` stacked step zones a pointer means. `frac` runs from 0 at the top edge of the
/// first zone to 1 at the bottom edge of the last; a drag past either end stays in the end
/// zone, so sliding down a two-step button goes from step 1 to step 2 and no further.
pub fn zone_at(frac: f32, n: usize) -> usize {
    if n == 0 {
        return 0;
    }
    let frac = if frac.is_finite() { frac } else { 0.0 };
    ((frac.clamp(0.0, 1.0) * n as f32) as usize).min(n - 1)
}

/// The positions of a rocker or axis grouped by direction: `up1` and `up2` are one direction,
/// `down1` and `down2` the next. A detent without a step number is a direction of its own.
pub fn direction_groups(def: &ControlDef) -> Vec<Direction> {
    let rest = def.rest_position();
    let mut groups: Vec<Direction> = Vec::new();
    for position in def.position_list().into_iter().filter(|p| *p != rest) {
        let (name, step) = match split_group_step(&position) {
            Some((g, n)) => (g.to_owned(), n),
            None => (position.clone(), 1),
        };
        match groups.iter_mut().find(|g| g.name == name) {
            Some(g) => g.steps.push((step, position)),
            None => groups.push(Direction {
                name,
                steps: vec![(step, position)],
            }),
        }
    }
    for g in &mut groups {
        g.steps.sort();
    }
    groups
}

/// Direction names that sit on the negative side of an axis (left, down, back).
fn negative_name(name: &str) -> bool {
    matches!(
        name,
        "left"
            | "west"
            | "down"
            | "lower"
            | "lowering"
            | "back"
            | "backward"
            | "rev"
            | "reverse"
            | "south"
            | "lower1"
    )
}

/// The axis of a control: negative detents, rest, positive detents. The first direction is the
/// positive one unless its name says it goes left, down or back.
pub fn stick_axis(def: &ControlDef) -> StickAxis {
    let mut dirs = direction_groups(def);
    if dirs.len() == 2 && negative_name(&dirs[0].name) && !negative_name(&dirs[1].name) {
        dirs.swap(0, 1);
    }
    let mut detents: Vec<String> = Vec::new();
    if let Some(negative) = dirs.get(1) {
        detents.extend(negative.steps.iter().rev().map(|(_, p)| p.clone()));
    }
    let rest = detents.len();
    detents.push(def.rest_position());
    if let Some(positive) = dirs.first() {
        detents.extend(positive.steps.iter().map(|(_, p)| p.clone()));
    }
    StickAxis {
        control: def.id.clone(),
        label: label_of(def),
        detents,
        rest,
        spring: def.is_spring(),
    }
}

/// Whether an axis goes up and down (hoist, raise, lower) rather than sideways.
fn is_vertical(axis: &StickAxis) -> bool {
    axis.detents.iter().any(|d| {
        let name = split_group_step(d).map_or(d.as_str(), |(g, _)| g);
        matches!(
            name,
            "up" | "down" | "raise" | "lower" | "hoist" | "lowering"
        )
    })
}

fn label_of(def: &ControlDef) -> String {
    if def.label.is_empty() {
        def.id.clone()
    } else {
        def.label.clone()
    }
}

fn button_role(def: &ControlDef) -> ButtonRole {
    let hay = format!("{} {}", def.id, def.label).to_lowercase();
    if def.kind == ControlKind::Deadman {
        ButtonRole::Deadman
    } else if hay.contains("horn") {
        ButtonRole::Horn
    } else if hay.contains("stop") {
        ButtonRole::Stop
    } else {
        ButtonRole::Plain
    }
}

/// Plan the panel of a profile.
pub fn plan(profile: &Profile) -> PanelPlan {
    let template = Template::of(profile.controller.kind);
    let joystick = profile.controller.kind == ControllerKind::Joystick;
    let mut safety = Vec::new();
    let mut motions = Vec::new();
    let mut selectors = Vec::new();
    let mut buttons = Vec::new();
    // Axes in the order of the profile, grouped by stick name.
    let mut sticks: Vec<(String, Vec<StickAxis>)> = Vec::new();
    let mut unnamed_axes = 0usize;

    for def in &profile.controls {
        let label = label_of(def);
        let control = def.id.clone();
        let as_axis = def.kind == ControlKind::Axis
            || (joystick && matches!(def.kind, ControlKind::Rocker2 | ControlKind::Rocker3));
        if as_axis {
            let name = match &def.stick {
                Some(s) => s.clone(),
                None => {
                    // Two axes to a pad, in profile order.
                    let n = unnamed_axes / 2;
                    unnamed_axes += 1;
                    format!("stick{}", n + 1)
                }
            };
            let axis = stick_axis(def);
            match sticks.iter_mut().find(|(n, _)| *n == name) {
                Some((_, axes)) => axes.push(axis),
                None => sticks.push((name, vec![axis])),
            }
            continue;
        }
        match def.kind {
            ControlKind::EStop => safety.push(Widget::EStop { control, label }),
            ControlKind::Latch => safety.push(Widget::Latch {
                control,
                label,
                key: false,
            }),
            ControlKind::Key => safety.push(Widget::Latch {
                control,
                label,
                key: true,
            }),
            ControlKind::Rocker2 | ControlKind::Rocker3 => motions.push(Widget::Motion {
                spring: def.is_spring(),
                directions: direction_groups(def),
                control,
                label,
            }),
            ControlKind::Selector => selectors.push(Widget::Selector {
                positions: def.position_list(),
                control,
                label,
            }),
            ControlKind::Button | ControlKind::Deadman => {
                let role = button_role(def);
                let widget = Widget::Button {
                    position: def.first_active_position().unwrap_or_default(),
                    role,
                    control,
                    label,
                };
                if matches!(role, ButtonRole::Stop | ButtonRole::Deadman) {
                    safety.push(widget);
                } else {
                    buttons.push(widget);
                }
            }
            ControlKind::Axis => {}
        }
    }

    let stick_widgets: Vec<Widget> = sticks
        .into_iter()
        .map(|(name, mut axes)| {
            axes.truncate(2);
            // Up and down belong on the vertical axis: a pad of hoist and bridge puts the hoist
            // up-down whatever order the profile lists them in.
            if axes.len() == 2 && is_vertical(&axes[0]) && !is_vertical(&axes[1]) {
                axes.swap(0, 1);
            }
            let label = axes
                .iter()
                .map(|a| a.label.as_str())
                .collect::<Vec<_>>()
                .join(" / ");
            Widget::Stick { name, label, axes }
        })
        .collect();

    let mut sections = Vec::new();
    let mut push = |title: &'static str, widgets: Vec<Widget>| {
        if !widgets.is_empty() {
            sections.push(Section { title, widgets });
        }
    };
    match template {
        Template::Joystick => {
            push("Joysticks", stick_widgets);
            push("Motions", motions);
            push("Selectors", selectors);
            push("Buttons", buttons);
            push("Safety and power", safety);
        }
        Template::Pendant => {
            push("Safety and power", safety);
            push("Motions", motions);
            push("Joysticks", stick_widgets);
            push("Selectors", selectors);
            push("Buttons", buttons);
        }
        Template::List => {
            let mut all = safety;
            all.extend(stick_widgets);
            all.extend(motions);
            all.extend(selectors);
            all.extend(buttons);
            push("Controls", all);
        }
    }
    PanelPlan { template, sections }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(text: &str) -> Profile {
        Profile::load(text).expect("profile loads")
    }

    const PENDANT: &str = r#"
[controller]
id = "p"
name = "Pendant"
kind = "pendant"

[[control]]
id = "estop"
kind = "estop"
[[control]]
id = "start"
kind = "latch"
[[control]]
id = "horn"
label = "Horn"
kind = "button"
[[control]]
id = "stop"
kind = "button"
[[control]]
id = "hoist"
kind = "rocker2"
positions = ["off", "up1", "up2", "down1", "down2"]
[[control]]
id = "bridge"
kind = "rocker2"
positions = ["off", "fwd1", "fwd2", "rev1", "rev2"]
[[control]]
id = "mode"
kind = "selector"
positions = ["fast", "slow"]
"#;

    #[test]
    fn pendant_is_safety_first_then_motions() {
        let plan = plan(&load(PENDANT));
        assert_eq!(plan.template, Template::Pendant);
        let titles: Vec<_> = plan.sections.iter().map(|s| s.title).collect();
        assert_eq!(
            titles,
            ["Safety and power", "Motions", "Selectors", "Buttons"]
        );
        assert_eq!(
            plan.controls(),
            ["estop", "start", "stop", "hoist", "bridge", "mode", "horn"]
        );
        let Widget::Motion { directions, .. } = &plan.sections[1].widgets[0] else {
            panic!("hoist is a motion");
        };
        assert_eq!(directions[0].name, "up");
        assert_eq!(directions[0].steps[1], (2, "up2".to_owned()));
        assert_eq!(directions[1].name, "down");
    }

    #[test]
    fn every_control_is_drawn_exactly_once() {
        for text in [PENDANT, JOYSTICK] {
            let profile = load(text);
            let mut drawn: Vec<String> = plan(&profile)
                .controls()
                .into_iter()
                .map(str::to_owned)
                .collect();
            let mut all: Vec<String> = profile.controls.iter().map(|c| c.id.clone()).collect();
            drawn.sort();
            all.sort();
            assert_eq!(drawn, all);
        }
    }

    const JOYSTICK: &str = r#"
[controller]
id = "j"
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
[[control]]
id = "estop"
kind = "estop"
"#;

    #[test]
    fn two_axes_make_one_pad_with_the_negative_side_on_the_left_and_down() {
        let plan = plan(&load(JOYSTICK));
        assert_eq!(plan.template, Template::Joystick);
        let Widget::Stick { axes, .. } = &plan.sections[0].widgets[0] else {
            panic!("first widget is the pad");
        };
        assert_eq!(axes.len(), 2);
        // Left-right: left (named first) goes to the negative end.
        assert_eq!(
            axes[0].detents,
            ["left2", "left1", "center", "right1", "right2"]
        );
        assert_eq!(axes[0].rest, 2);
        // Up-down: up is positive, down negative.
        assert_eq!(axes[1].detents, ["down2", "down1", "center", "up1", "up2"]);
    }

    #[test]
    fn a_pointer_picks_the_nearest_detent() {
        let plan = plan(&load(JOYSTICK));
        let Widget::Stick { axes, .. } = &plan.sections[0].widgets[0] else {
            panic!();
        };
        let x = &axes[0];
        assert_eq!(x.detent_at(0.0), "center");
        assert_eq!(x.detent_at(0.2), "center");
        assert_eq!(x.detent_at(0.3), "right1");
        assert_eq!(x.detent_at(0.8), "right2");
        assert_eq!(x.detent_at(5.0), "right2");
        assert_eq!(x.detent_at(-0.6), "left1");
        assert_eq!(x.detent_at(-1.0), "left2");
        assert_eq!(x.detent_at(f32::NAN), "center");
        for position in &x.detents {
            assert_eq!(x.detent_at(x.offset_of(position)), position);
        }
    }

    #[test]
    fn an_axis_with_one_step_each_way_is_three_detents() {
        let profile = load(
            r#"
[controller]
id = "j"
name = "J"
kind = "joystick"
[[control]]
id = "a"
kind = "rocker2"
positions = ["off", "up1", "down1"]
"#,
        );
        let plan = plan(&profile);
        let Widget::Stick { axes, .. } = &plan.sections[0].widgets[0] else {
            panic!();
        };
        assert_eq!(axes[0].detents, ["down1", "off", "up1"]);
        assert_eq!(axes[0].detent_at(0.6), "up1");
        assert_eq!(axes[0].detent_at(-0.6), "down1");
    }

    #[test]
    fn the_hoist_goes_up_and_down_even_when_the_profile_lists_it_first() {
        let mut profile = load(JOYSTICK);
        profile.controls.swap(0, 1);
        let plan = plan(&profile);
        let Widget::Stick { axes, .. } = &plan.sections[0].widgets[0] else {
            panic!();
        };
        assert_eq!(axes[0].control, "bridge", "left-right first");
        assert_eq!(axes[1].control, "hoist", "up-down second");
    }

    #[test]
    fn named_sticks_pair_axes_across_the_profile() {
        let mut profile = load(JOYSTICK);
        profile.controls[0].stick = Some("main".into());
        profile.controls[1].stick = Some("main".into());
        let plan = plan(&profile);
        assert_eq!(plan.widgets().count(), 2);
    }

    #[test]
    fn sliding_down_a_two_step_button_goes_from_step_one_to_two() {
        assert_eq!(zone_at(-0.5, 2), 0);
        assert_eq!(zone_at(0.0, 2), 0);
        assert_eq!(zone_at(0.49, 2), 0);
        assert_eq!(zone_at(0.5, 2), 1);
        assert_eq!(zone_at(3.0, 2), 1);
        assert_eq!(zone_at(f32::NAN, 3), 0);
        assert_eq!(zone_at(0.7, 3), 2);
        assert_eq!(zone_at(0.5, 0), 0);
    }

    #[test]
    fn horn_stop_and_deadman_get_their_roles() {
        let plan = plan(&load(PENDANT));
        let roles: Vec<_> = plan
            .widgets()
            .filter_map(|w| match w {
                Widget::Button { control, role, .. } => Some((control.as_str(), *role)),
                _ => None,
            })
            .collect();
        assert_eq!(
            roles,
            [("stop", ButtonRole::Stop), ("horn", ButtonRole::Horn)]
        );
    }
}
