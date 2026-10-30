//! A panel as data (`*.panel.toml`): the add-on says what is on it, the program draws it with
//! its own widgets in the theme's own colours (`docs/design/visual-redesign-addons.md`
//! section 5). An add-on has no way to paint a pixel, set a colour or open a window.
//!
//! Add-on API 1 has `heading`, `label`, `separator`, `button`, `lamp` and `row`. API 2 (lane C3)
//! adds `rocker` (1 to 3 steps, optional spring return, labelled directions), `joystick`
//! (axes with detents), `selector` (also as a rotary), `keymap` and `table` (read-only tables),
//! `tabs` and `bar` (a meter). A layout that uses a kind of a newer API than its add-on declares
//! is refused at install, an unknown kind is refused at install, and a program that speaks an
//! older API refuses the whole add-on (`Manifest::parse`), so nobody ever sees half a panel.
//!
//! Every widget that can be operated sends ordinary button ids to the add-on's engine, the same
//! op as a `button`: a rocker `set-hoist` sends `set-hoist-up1`, `set-hoist-up2`,
//! `set-hoist-down1`, `set-hoist-off`; a selector `sel` with option `main` sends `sel-main`; a
//! joystick `stick` with axis `x` sends `stick-x-right2` or `stick-x-off`. [`PanelLayout::events`]
//! lists them all, and an event id may not collide with another widget's id.
//!
//! ```toml
//! [[widget]]
//! kind = "heading"
//! text = "Radio"
//!
//! [[widget]]
//! kind = "row"
//! widgets = [
//!   { kind = "button", id = "start", text = "Start" },
//!   { kind = "lamp", id = "main", text = "Main", state = "ok" },
//! ]
//!
//! [[widget]]
//! kind = "rocker"
//! id = "set-hoist"
//! text = "Hoist"
//! steps = 2
//! spring = true
//! ```

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::{AddonError, Result};
use crate::manifest::{API_VERSION, valid_id};

/// Most widgets in one panel, nested ones included.
pub const MAX_WIDGETS: usize = 200;
/// Deepest nesting of `row` and `tabs`.
pub const MAX_DEPTH: usize = 2;
/// Longest text on a widget.
pub const MAX_TEXT: usize = 200;
/// Largest layout file.
pub const MAX_BYTES: usize = 128 * 1024;
/// Longest id of a widget.
pub const MAX_ID: usize = 32;
/// Longest event id a widget makes from its id.
pub const MAX_EVENT_ID: usize = 64;
/// Most steps of a rocker, and detents per direction of a joystick axis.
pub const MAX_STEPS: u8 = 3;
/// Most options of a selector.
pub const MAX_OPTIONS: usize = 12;
/// Most tabs.
pub const MAX_TABS: usize = 6;
/// Most table columns.
pub const MAX_COLUMNS: usize = 8;
/// Most table and key map rows.
pub const MAX_ROWS: usize = 60;
/// Longest key name on a key map.
pub const MAX_KEY: usize = 16;

/// A meaning, not a colour: the program resolves it with its palette, so dark, light and High
/// contrast work without the add-on knowing. A lamp always carries text as well.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    #[default]
    Neutral,
    Accent,
    Ok,
    Warn,
    Error,
    Info,
}

/// One direction of a rocker or a joystick axis: a short name that becomes part of the event id
/// (`up`, `down`, `fwd`, `left`) and the words shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Direction {
    pub name: String,
    pub text: String,
}

impl Direction {
    fn new(name: &str, text: &str) -> Direction {
        Direction {
            name: name.into(),
            text: text.into(),
        }
    }
}

fn plus_default() -> Direction {
    Direction::new("up", "Up")
}
fn minus_default() -> Direction {
    Direction::new("down", "Down")
}
fn one() -> u8 {
    1
}
fn yes() -> bool {
    true
}

/// One axis of a joystick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Axis {
    /// Names the axis in the event id (`x`, `y`, `trolley`).
    pub name: String,
    pub plus: Direction,
    pub minus: Direction,
    /// Detents per direction, 1 to 3: the knob rests only on a detent or in the middle.
    #[serde(default = "one")]
    pub detents: u8,
}

/// One position of a selector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectorOption {
    pub name: String,
    pub text: String,
}

/// One row of a key map: the key and what it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyRow {
    pub key: String,
    pub text: String,
}

/// One tab of a `tabs` widget.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub id: String,
    pub text: String,
    pub widgets: Vec<Widget>,
}

/// One thing on a panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum Widget {
    Heading {
        text: String,
    },
    Label {
        text: String,
        /// Drawn in the weaker text colour.
        #[serde(default)]
        weak: bool,
    },
    Separator,
    /// A button. Pressing it sends `id` to the add-on's engine (C0b-2); with no engine it does
    /// nothing, and the program says so in the tooltip.
    Button {
        id: String,
        text: String,
        #[serde(default)]
        hint: String,
    },
    /// An indicator: a filled or hollow mark, its text, never colour alone.
    Lamp {
        id: String,
        text: String,
        #[serde(default)]
        state: Role,
        #[serde(default)]
        lit: bool,
    },
    /// Widgets side by side.
    Row {
        widgets: Vec<Widget>,
    },
    /// API 2. A rocker: `steps` zones per direction (1 to 3). With `spring` it returns to off
    /// when let go, otherwise it stays at the position clicked. Sends `<id>-<dir><step>` and
    /// `<id>-off`.
    Rocker {
        id: String,
        text: String,
        #[serde(default = "one")]
        steps: u8,
        #[serde(default)]
        spring: bool,
        #[serde(default = "plus_default")]
        plus: Direction,
        #[serde(default = "minus_default")]
        minus: Direction,
        #[serde(default)]
        hint: String,
    },
    /// API 2. A joystick of one or two axes with detents. Sends `<id>-<axis>-<dir><n>` and
    /// `<id>-<axis>-off`; with `spring` (the default) it returns to the middle when let go.
    Joystick {
        id: String,
        text: String,
        axes: Vec<Axis>,
        #[serde(default = "yes")]
        spring: bool,
        #[serde(default)]
        hint: String,
    },
    /// API 2. One of several positions; drawn as a row of choices, or as a rotary knob.
    /// Sends `<id>-<option>`.
    Selector {
        id: String,
        text: String,
        options: Vec<SelectorOption>,
        #[serde(default)]
        rotary: bool,
        #[serde(default)]
        hint: String,
    },
    /// API 2. A read-only key map (key caps and what they do).
    Keymap {
        #[serde(default)]
        text: String,
        rows: Vec<KeyRow>,
    },
    /// API 2. A read-only table of text.
    Table {
        #[serde(default)]
        text: String,
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    /// API 2. Tabs; only at the top of a panel (they do not nest in rows or in tabs).
    Tabs {
        tabs: Vec<Tab>,
    },
    /// API 2. A meter: a bar between `min` and `max` with its value in text. The engine may
    /// set the value (reply `bars`).
    Bar {
        id: String,
        text: String,
        min: f64,
        max: f64,
        #[serde(default)]
        value: f64,
        #[serde(default)]
        unit: String,
        #[serde(default)]
        state: Role,
    },
}

impl Widget {
    /// The lowest add-on API in which this kind exists.
    pub fn min_api(&self) -> u32 {
        match self {
            Widget::Heading { .. }
            | Widget::Label { .. }
            | Widget::Separator
            | Widget::Button { .. }
            | Widget::Lamp { .. }
            | Widget::Row { .. } => 1,
            _ => 2,
        }
    }

    /// The kind as written in the file.
    pub fn kind(&self) -> &'static str {
        match self {
            Widget::Heading { .. } => "heading",
            Widget::Label { .. } => "label",
            Widget::Separator => "separator",
            Widget::Button { .. } => "button",
            Widget::Lamp { .. } => "lamp",
            Widget::Row { .. } => "row",
            Widget::Rocker { .. } => "rocker",
            Widget::Joystick { .. } => "joystick",
            Widget::Selector { .. } => "selector",
            Widget::Keymap { .. } => "keymap",
            Widget::Table { .. } => "table",
            Widget::Tabs { .. } => "tabs",
            Widget::Bar { .. } => "bar",
        }
    }

    /// The event ids this widget itself can send (not those of widgets inside it).
    fn own_events(&self) -> Vec<String> {
        let mut out = Vec::new();
        match self {
            Widget::Button { id, .. } => out.push(id.clone()),
            Widget::Rocker {
                id,
                steps,
                plus,
                minus,
                ..
            } => {
                out.push(format!("{id}-off"));
                for d in [plus, minus] {
                    for n in 1..=*steps {
                        out.push(format!("{id}-{}{n}", d.name));
                    }
                }
            }
            Widget::Joystick { id, axes, .. } => {
                for a in axes {
                    out.push(format!("{id}-{}-off", a.name));
                    for d in [&a.plus, &a.minus] {
                        for n in 1..=a.detents {
                            out.push(format!("{id}-{}-{}{n}", a.name, d.name));
                        }
                    }
                }
            }
            Widget::Selector { id, options, .. } => {
                out.extend(options.iter().map(|o| format!("{id}-{}", o.name)));
            }
            _ => {}
        }
        out
    }

    /// The widgets directly inside this one.
    fn children(&self) -> Vec<&Widget> {
        match self {
            Widget::Row { widgets } => widgets.iter().collect(),
            Widget::Tabs { tabs } => tabs.iter().flat_map(|t| &t.widgets).collect(),
            _ => Vec::new(),
        }
    }
}

/// A parsed, validated panel layout.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelLayout {
    #[serde(default, rename = "widget")]
    pub widgets: Vec<Widget>,
}

/// Visits every widget, nested ones included, in order.
fn walk<'a>(ws: &'a [Widget], f: &mut impl FnMut(&'a Widget)) {
    for w in ws {
        f(w);
        for c in w.children() {
            walk(std::slice::from_ref(c), f);
        }
    }
}

impl PanelLayout {
    /// Parses a layout for this program's own API generation.
    pub fn parse(bytes: &[u8]) -> Result<PanelLayout> {
        PanelLayout::parse_for_api(bytes, API_VERSION)
    }

    /// Parses a layout of an add-on that declares `api_version = api`: a kind that came with a
    /// later API is refused, naming the kind and the API it needs.
    pub fn parse_for_api(bytes: &[u8], api: u32) -> Result<PanelLayout> {
        let bad = |m: String| Err(AddonError::Manifest(format!("panel layout: {m}")));
        if bytes.len() > MAX_BYTES {
            return bad("larger than 128 KiB".into());
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return bad("not UTF-8 text".into());
        };
        let layout: PanelLayout =
            toml::from_str(text).map_err(|e| AddonError::Manifest(format!("panel layout: {e}")))?;
        let mut state = Walk {
            api,
            count: 0,
            ids: BTreeSet::new(),
            events: BTreeSet::new(),
        };
        check(&layout.widgets, 0, &mut state)?;
        Ok(layout)
    }

    /// Every id of a button or lamp, in order (the ids an engine may answer for).
    pub fn ids(&self) -> Vec<&str> {
        let mut out = Vec::new();
        walk(&self.widgets, &mut |w| {
            if let Widget::Button { id, .. } | Widget::Lamp { id, .. } = w {
                out.push(id.as_str());
            }
        });
        out
    }

    /// Every id of a bar, in order (the ids an engine may set a value for).
    pub fn bar_ids(&self) -> Vec<&str> {
        let mut out = Vec::new();
        walk(&self.widgets, &mut |w| {
            if let Widget::Bar { id, .. } = w {
                out.push(id.as_str());
            }
        });
        out
    }

    /// Every id the panel can send to the engine, in order: buttons, and the positions of
    /// rockers, joysticks and selectors.
    pub fn events(&self) -> Vec<String> {
        let mut out = Vec::new();
        walk(&self.widgets, &mut |w| out.extend(w.own_events()));
        out
    }

    /// The lowest add-on API that can show this layout.
    pub fn min_api(&self) -> u32 {
        let mut api = 1;
        walk(&self.widgets, &mut |w| api = api.max(w.min_api()));
        api
    }
}

/// The detent a knob offset means: `offset` runs from -1 (full minus) to 1 (full plus) and the
/// result is a signed detent, `-detents..=detents`, 0 meaning the middle. A fifth at the middle
/// is a dead zone; each side beyond it is cut into `detents` equal zones.
pub fn detent_at(offset: f32, detents: u8) -> i8 {
    const DEAD: f32 = 0.2;
    let n = detents.clamp(1, MAX_STEPS) as f32;
    let o = if offset.is_finite() {
        offset.clamp(-1.0, 1.0)
    } else {
        0.0
    };
    if o.abs() < DEAD {
        return 0;
    }
    let frac = (o.abs() - DEAD) / (1.0 - DEAD);
    let step = ((frac * n).floor() + 1.0).min(n) as i8;
    if o < 0.0 { -step } else { step }
}

/// Where the knob sits at detent `k` (1 to `detents`) on the plus side, from 0 to 1: the middle
/// of its zone, so [`detent_at`] of this offset is `k` again. Use a negative `k` for the minus side.
pub fn detent_offset(k: i8, detents: u8) -> f32 {
    const DEAD: f32 = 0.2;
    let n = detents.clamp(1, MAX_STEPS) as f32;
    let mag = (k.unsigned_abs() as f32).clamp(0.0, n);
    if mag == 0.0 {
        return 0.0;
    }
    let off = DEAD + (mag - 0.5) / n * (1.0 - DEAD);
    if k < 0 { -off } else { off }
}

/// What a walk over the layout keeps.
struct Walk {
    api: u32,
    count: usize,
    ids: BTreeSet<String>,
    events: BTreeSet<String>,
}

fn text_ok(t: &str) -> bool {
    !t.is_empty()
        && t.chars().count() <= MAX_TEXT
        && !t.chars().any(|c| c.is_control() && c != '\n')
}

fn bad<T>(m: impl std::fmt::Display) -> Result<T> {
    Err(AddonError::Manifest(format!("panel layout: {m}")))
}

fn check(ws: &[Widget], depth: usize, st: &mut Walk) -> Result<()> {
    for w in ws {
        st.count += 1;
        if st.count > MAX_WIDGETS {
            return bad(format!("more than {MAX_WIDGETS} widgets"));
        }
        if w.min_api() > st.api {
            return bad(format!(
                "a {} needs add-on API {}, but this add-on declares API {}",
                w.kind(),
                w.min_api(),
                st.api
            ));
        }
        // Every widget with an id of its own: a good id, once in the panel.
        let own_id = match w {
            Widget::Button { id, .. }
            | Widget::Lamp { id, .. }
            | Widget::Rocker { id, .. }
            | Widget::Joystick { id, .. }
            | Widget::Selector { id, .. }
            | Widget::Bar { id, .. } => Some(id),
            _ => None,
        };
        if let Some(id) = own_id {
            if !valid_id(id, 1, MAX_ID) {
                return bad(format!("a {} has a bad id {id:?}", w.kind()));
            }
            if st.events.contains(id) || !st.ids.insert(id.clone()) {
                return bad(format!("the id {id:?} is used twice"));
            }
        }
        match w {
            Widget::Heading { text } | Widget::Label { text, .. } => {
                if !text_ok(text) {
                    return bad("a text is empty, too long or has control characters");
                }
            }
            Widget::Separator => {}
            Widget::Button { id, text, hint } => {
                if !text_ok(text) || hint.chars().count() > MAX_TEXT {
                    return bad(format!("button {id:?} has a bad text or hint"));
                }
            }
            Widget::Lamp { id, text, .. } => {
                if !text_ok(text) {
                    return bad(format!("lamp {id:?} has a bad text"));
                }
            }
            Widget::Row { widgets } => {
                if depth + 1 > MAX_DEPTH {
                    return bad("rows nest deeper than 2 levels");
                }
                if widgets.is_empty() {
                    return bad("a row is empty");
                }
                check(widgets, depth + 1, st)?;
            }
            Widget::Rocker {
                id,
                text,
                steps,
                plus,
                minus,
                hint,
                ..
            } => {
                if !text_ok(text) || hint.chars().count() > MAX_TEXT {
                    return bad(format!("rocker {id:?} has a bad text or hint"));
                }
                if !(1..=MAX_STEPS).contains(steps) {
                    return bad(format!("rocker {id:?} needs 1 to {MAX_STEPS} steps"));
                }
                direction_ok(id, plus, minus)?;
            }
            Widget::Joystick {
                id,
                text,
                axes,
                hint,
                ..
            } => {
                if !text_ok(text) || hint.chars().count() > MAX_TEXT {
                    return bad(format!("joystick {id:?} has a bad text or hint"));
                }
                if axes.is_empty() || axes.len() > 2 {
                    return bad(format!("joystick {id:?} needs one or two axes"));
                }
                let mut names = BTreeSet::new();
                for a in axes {
                    if !valid_id(&a.name, 1, 16) || !names.insert(a.name.as_str()) {
                        return bad(format!("joystick {id:?} has a bad or repeated axis name"));
                    }
                    if !(1..=MAX_STEPS).contains(&a.detents) {
                        return bad(format!("joystick {id:?} needs 1 to {MAX_STEPS} detents"));
                    }
                    direction_ok(id, &a.plus, &a.minus)?;
                }
            }
            Widget::Selector {
                id,
                text,
                options,
                hint,
                ..
            } => {
                if !text_ok(text) || hint.chars().count() > MAX_TEXT {
                    return bad(format!("selector {id:?} has a bad text or hint"));
                }
                if options.len() < 2 || options.len() > MAX_OPTIONS {
                    return bad(format!("selector {id:?} needs 2 to {MAX_OPTIONS} options"));
                }
                let mut names = BTreeSet::new();
                for o in options {
                    if !valid_id(&o.name, 1, 16) || !text_ok(&o.text) || !names.insert(&o.name) {
                        return bad(format!(
                            "selector {id:?} has a bad, empty or repeated option"
                        ));
                    }
                }
            }
            Widget::Keymap { text, rows } => {
                if text.chars().count() > MAX_TEXT || rows.is_empty() || rows.len() > MAX_ROWS {
                    return bad(format!("a keymap needs 1 to {MAX_ROWS} rows"));
                }
                for r in rows {
                    if r.key.is_empty()
                        || r.key.chars().count() > MAX_KEY
                        || r.key.chars().any(char::is_control)
                        || !text_ok(&r.text)
                    {
                        return bad("a keymap row has a bad key or text");
                    }
                }
            }
            Widget::Table {
                text,
                columns,
                rows,
            } => {
                if text.chars().count() > MAX_TEXT
                    || columns.is_empty()
                    || columns.len() > MAX_COLUMNS
                    || rows.len() > MAX_ROWS
                {
                    return bad(format!(
                        "a table needs 1 to {MAX_COLUMNS} columns and at most {MAX_ROWS} rows"
                    ));
                }
                if !columns.iter().all(|c| text_ok(c)) {
                    return bad("a table column heading is empty or too long");
                }
                for r in rows {
                    if r.len() != columns.len() {
                        return bad("a table row has another number of cells than columns");
                    }
                    // A cell may be empty; it may not be long or hold control characters.
                    if !r
                        .iter()
                        .all(|c| c.chars().count() <= MAX_TEXT && !c.chars().any(char::is_control))
                    {
                        return bad("a table cell is too long or has control characters");
                    }
                }
            }
            Widget::Tabs { tabs } => {
                if depth > 0 {
                    return bad("tabs are only allowed at the top of a panel");
                }
                if tabs.len() < 2 || tabs.len() > MAX_TABS {
                    return bad(format!("tabs needs 2 to {MAX_TABS} tabs"));
                }
                let mut names = BTreeSet::new();
                for t in tabs {
                    if !valid_id(&t.id, 1, 16) || !text_ok(&t.text) || !names.insert(&t.id) {
                        return bad("a tab has a bad, empty or repeated id or text");
                    }
                    if t.widgets.is_empty() {
                        return bad(format!("tab {:?} is empty", t.id));
                    }
                    // The contents of a tab are one level down, so a row fits in a tab but a
                    // row in a row in a tab does not.
                    check(&t.widgets, depth + 1, st)?;
                }
            }
            Widget::Bar {
                id,
                text,
                min,
                max,
                value,
                unit,
                ..
            } => {
                if !text_ok(text) || unit.chars().count() > 8 || unit.chars().any(char::is_control)
                {
                    return bad(format!("bar {id:?} has a bad text or unit"));
                }
                if !(min.is_finite() && max.is_finite() && value.is_finite() && min < max) {
                    return bad(format!(
                        "bar {id:?} needs finite numbers with min below max"
                    ));
                }
            }
        }
        // The ids this widget sends must be sane and must not meet another widget's.
        for e in w.own_events() {
            if e.len() > MAX_EVENT_ID {
                return bad(format!("the event id {e:?} is longer than {MAX_EVENT_ID}"));
            }
            let is_button = matches!(w, Widget::Button { .. });
            if (!is_button && st.ids.contains(&e)) || !st.events.insert(e.clone()) {
                return bad(format!("the event id {e:?} is used twice"));
            }
        }
    }
    Ok(())
}

fn direction_ok(id: &str, plus: &Direction, minus: &Direction) -> Result<()> {
    for d in [plus, minus] {
        if !valid_id(&d.name, 1, 12) || d.name.ends_with(|c: char| c.is_ascii_digit()) {
            return bad(format!(
                "{id:?} has a direction name {:?} that is not a lower-case word",
                d.name
            ));
        }
        if !text_ok(&d.text) {
            return bad(format!("{id:?} has an empty or too long direction text"));
        }
    }
    if plus.name == minus.name || plus.name == "off" || minus.name == "off" {
        return bad(format!(
            "{id:?} needs two different direction names, not off"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const API1: &str = r#"
[[widget]]
kind = "heading"
text = "Radio"
[[widget]]
kind = "row"
widgets = [
  { kind = "button", id = "start", text = "Start" },
  { kind = "lamp", id = "main", text = "Main", state = "ok", lit = true },
]
[[widget]]
kind = "separator"
"#;

    const NEW: &str = r#"
[[widget]]
kind = "tabs"
tabs = [
  { id = "drive", text = "Drive", widgets = [
    { kind = "rocker", id = "set-hoist", text = "Hoist", steps = 2, spring = true },
    { kind = "joystick", id = "stick", text = "Stick", axes = [
      { name = "x", plus = { name = "right", text = "Right" }, minus = { name = "left", text = "Left" }, detents = 2 },
    ] },
    { kind = "selector", id = "sel", text = "Select", rotary = true, options = [
      { name = "main", text = "Main" }, { name = "mast", text = "Mast" },
    ] },
    { kind = "bar", id = "speed", text = "Speed", min = 0.0, max = 50.0, value = 10.0, unit = "Hz" },
  ] },
  { id = "keys", text = "Keys", widgets = [
    { kind = "keymap", rows = [{ key = "Up", text = "Hoist up" }] },
    { kind = "table", columns = ["A", "B"], rows = [["1", "2"], ["", "4"]] },
  ] },
]
"#;

    fn refused(text: &str) -> String {
        PanelLayout::parse(text.as_bytes())
            .expect_err(text)
            .to_string()
    }

    #[test]
    fn parses_and_lists_ids() {
        let l = PanelLayout::parse(API1.as_bytes()).unwrap();
        assert_eq!(l.widgets.len(), 3);
        assert_eq!(l.ids(), vec!["start", "main"]);
        assert_eq!(l.min_api(), 1);
        assert_eq!(l.events(), vec!["start"]);
    }

    #[test]
    fn new_kinds_parse_and_send_the_ids_an_engine_expects() {
        let l = PanelLayout::parse(NEW.as_bytes()).unwrap();
        assert_eq!(l.min_api(), 2);
        assert_eq!(l.bar_ids(), vec!["speed"]);
        let e = l.events();
        for want in [
            "set-hoist-off",
            "set-hoist-up1",
            "set-hoist-up2",
            "set-hoist-down1",
            "set-hoist-down2",
            "stick-x-off",
            "stick-x-right1",
            "stick-x-right2",
            "stick-x-left2",
            "sel-main",
            "sel-mast",
        ] {
            assert!(e.iter().any(|x| x == want), "{want} in {e:?}");
        }
        assert_eq!(e.len(), 5 + 5 + 2, "{e:?}");
    }

    #[test]
    fn rocker_directions_are_labelled_and_steps_default_to_one() {
        let l = PanelLayout::parse(
            b"[[widget]]\nkind = \"rocker\"\nid = \"bridge\"\ntext = \"Bridge\"\nplus = { name = \"fwd\", text = \"Forward\" }\nminus = { name = \"back\", text = \"Back\" }\n",
        )
        .unwrap();
        assert_eq!(
            l.events(),
            vec!["bridge-off", "bridge-fwd1", "bridge-back1"]
        );
    }

    #[test]
    fn a_kind_of_a_newer_api_is_refused_for_an_older_add_on() {
        let msg = PanelLayout::parse_for_api(NEW.as_bytes(), 1)
            .unwrap_err()
            .to_string();
        assert!(msg.contains("tabs needs add-on API 2"), "{msg}");
        assert!(msg.contains("declares API 1"), "{msg}");
        assert!(PanelLayout::parse_for_api(API1.as_bytes(), 1).is_ok());
        assert!(PanelLayout::parse_for_api(NEW.as_bytes(), 2).is_ok());
    }

    #[test]
    fn detents_cut_the_travel_into_zones() {
        assert_eq!(detent_at(0.0, 2), 0);
        assert_eq!(detent_at(0.19, 2), 0);
        assert_eq!(detent_at(0.3, 2), 1);
        assert_eq!(detent_at(0.9, 2), 2);
        assert_eq!(detent_at(-0.9, 2), -2);
        assert_eq!(detent_at(-0.3, 3), -1);
        assert_eq!(detent_at(1.0, 3), 3);
        assert_eq!(detent_at(5.0, 1), 1);
        assert_eq!(detent_at(f32::NAN, 2), 0);
        assert_eq!(detent_at(0.9, 0), 1, "zero detents is read as one");
    }

    #[test]
    fn a_detent_offset_reads_back_as_its_detent() {
        for n in 1..=MAX_STEPS {
            assert_eq!(detent_offset(0, n), 0.0);
            for k in 1..=n as i8 {
                assert_eq!(detent_at(detent_offset(k, n), n), k);
                assert_eq!(detent_at(detent_offset(-k, n), n), -k);
            }
        }
    }

    #[test]
    fn hostile_layouts_are_refused() {
        for (why, text) in [
            (
                "unknown kind",
                "[[widget]]\nkind = \"webview\"\nurl = \"http://x\"\n",
            ),
            (
                "colour",
                "[[widget]]\nkind = \"label\"\ntext = \"a\"\ncolor = \"#f00\"\n",
            ),
            ("empty text", "[[widget]]\nkind = \"label\"\ntext = \"\"\n"),
            (
                "bad id",
                "[[widget]]\nkind = \"button\"\nid = \"../x\"\ntext = \"a\"\n",
            ),
            ("empty row", "[[widget]]\nkind = \"row\"\nwidgets = []\n"),
            (
                "deep",
                "[[widget]]\nkind = \"row\"\nwidgets = [{ kind = \"row\", widgets = [{ kind = \"row\", widgets = [{ kind = \"separator\" }] }] }]\n",
            ),
            (
                "role",
                "[[widget]]\nkind = \"lamp\"\nid = \"a\"\ntext = \"a\"\nstate = \"pink\"\n",
            ),
        ] {
            assert!(PanelLayout::parse(text.as_bytes()).is_err(), "{why}");
        }
        let many = "[[widget]]\nkind = \"separator\"\n".repeat(MAX_WIDGETS + 1);
        assert!(
            PanelLayout::parse(many.as_bytes())
                .unwrap_err()
                .to_string()
                .contains("200")
        );
    }

    #[test]
    fn hostile_new_widgets_are_refused() {
        let rocker = |extra: &str| {
            format!("[[widget]]\nkind = \"rocker\"\nid = \"r\"\ntext = \"R\"\n{extra}\n")
        };
        for (why, text) in [
            ("zero steps", rocker("steps = 0")),
            ("four steps", rocker("steps = 4")),
            ("huge steps", rocker("steps = 255")),
            ("negative steps", rocker("steps = -1")),
            (
                "same direction twice",
                rocker("plus = { name = \"up\", text = \"A\" }\nminus = { name = \"up\", text = \"B\" }"),
            ),
            (
                "direction named off",
                rocker("plus = { name = \"off\", text = \"A\" }"),
            ),
            (
                "direction ends in a digit",
                rocker("plus = { name = \"up1\", text = \"A\" }"),
            ),
            (
                "direction text empty",
                rocker("plus = { name = \"up\", text = \"\" }"),
            ),
            ("unknown field", rocker("colour = \"red\"")),
            (
                "oversized hint",
                rocker(&format!("hint = \"{}\"", "x".repeat(MAX_TEXT + 1))),
            ),
            (
                "oversized text",
                format!(
                    "[[widget]]\nkind = \"rocker\"\nid = \"r\"\ntext = \"{}\"\n",
                    "x".repeat(MAX_TEXT + 1)
                ),
            ),
            (
                "control characters",
                "[[widget]]\nkind = \"rocker\"\nid = \"r\"\ntext = \"a\\u0007b\"\n".into(),
            ),
            (
                "joystick without axes",
                "[[widget]]\nkind = \"joystick\"\nid = \"j\"\ntext = \"J\"\naxes = []\n".into(),
            ),
            (
                "joystick with three axes",
                format!(
                    "[[widget]]\nkind = \"joystick\"\nid = \"j\"\ntext = \"J\"\naxes = [{}]\n",
                    ["a", "b", "c"]
                        .map(|n| format!("{{ name = \"{n}\", plus = {{ name = \"p\", text = \"P\" }}, minus = {{ name = \"m\", text = \"M\" }} }}"))
                        .join(",")
                ),
            ),
            (
                "joystick axis twice",
                format!(
                    "[[widget]]\nkind = \"joystick\"\nid = \"j\"\ntext = \"J\"\naxes = [{0}, {0}]\n",
                    "{ name = \"x\", plus = { name = \"p\", text = \"P\" }, minus = { name = \"m\", text = \"M\" } }"
                ),
            ),
            (
                "joystick detents",
                "[[widget]]\nkind = \"joystick\"\nid = \"j\"\ntext = \"J\"\naxes = [{ name = \"x\", detents = 9, plus = { name = \"p\", text = \"P\" }, minus = { name = \"m\", text = \"M\" } }]\n".into(),
            ),
            (
                "selector with one option",
                "[[widget]]\nkind = \"selector\"\nid = \"s\"\ntext = \"S\"\noptions = [{ name = \"a\", text = \"A\" }]\n".into(),
            ),
            (
                "selector option twice",
                "[[widget]]\nkind = \"selector\"\nid = \"s\"\ntext = \"S\"\noptions = [{ name = \"a\", text = \"A\" }, { name = \"a\", text = \"B\" }]\n".into(),
            ),
            (
                "selector bad option name",
                "[[widget]]\nkind = \"selector\"\nid = \"s\"\ntext = \"S\"\noptions = [{ name = \"a\", text = \"A\" }, { name = \"../b\", text = \"B\" }]\n".into(),
            ),
            (
                "selector too many options",
                format!(
                    "[[widget]]\nkind = \"selector\"\nid = \"s\"\ntext = \"S\"\noptions = [{}]\n",
                    (0..=MAX_OPTIONS)
                        .map(|i| format!("{{ name = \"o{}\", text = \"O\" }}", (b'a' + i as u8) as char))
                        .collect::<Vec<_>>()
                        .join(",")
                ),
            ),
            (
                "keymap empty",
                "[[widget]]\nkind = \"keymap\"\nrows = []\n".into(),
            ),
            (
                "keymap long key",
                format!(
                    "[[widget]]\nkind = \"keymap\"\nrows = [{{ key = \"{}\", text = \"a\" }}]\n",
                    "k".repeat(MAX_KEY + 1)
                ),
            ),
            (
                "keymap too many rows",
                format!(
                    "[[widget]]\nkind = \"keymap\"\nrows = [{}]\n",
                    vec!["{ key = \"a\", text = \"b\" }"; MAX_ROWS + 1].join(",")
                ),
            ),
            (
                "table without columns",
                "[[widget]]\nkind = \"table\"\ncolumns = []\nrows = []\n".into(),
            ),
            (
                "table ragged row",
                "[[widget]]\nkind = \"table\"\ncolumns = [\"a\", \"b\"]\nrows = [[\"1\"]]\n".into(),
            ),
            (
                "table too many columns",
                format!(
                    "[[widget]]\nkind = \"table\"\ncolumns = [{}]\nrows = []\n",
                    ["\"c\""; MAX_COLUMNS + 1].join(",")
                ),
            ),
            (
                "table too many rows",
                format!(
                    "[[widget]]\nkind = \"table\"\ncolumns = [\"a\"]\nrows = [{}]\n",
                    vec!["[\"x\"]"; MAX_ROWS + 1].join(",")
                ),
            ),
            (
                "table huge cell",
                format!(
                    "[[widget]]\nkind = \"table\"\ncolumns = [\"a\"]\nrows = [[\"{}\"]]\n",
                    "x".repeat(MAX_TEXT + 1)
                ),
            ),
            (
                "tabs with one tab",
                "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"a\", text = \"A\", widgets = [{ kind = \"separator\" }] }]\n".into(),
            ),
            (
                "tab empty",
                "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"a\", text = \"A\", widgets = [] }, { id = \"b\", text = \"B\", widgets = [{ kind = \"separator\" }] }]\n".into(),
            ),
            (
                "tab id twice",
                "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"a\", text = \"A\", widgets = [{ kind = \"separator\" }] }, { id = \"a\", text = \"B\", widgets = [{ kind = \"separator\" }] }]\n".into(),
            ),
            (
                "tabs in tabs",
                "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"a\", text = \"A\", widgets = [{ kind = \"tabs\", tabs = [] }] }, { id = \"b\", text = \"B\", widgets = [{ kind = \"separator\" }] }]\n".into(),
            ),
            (
                "tabs in a row",
                "[[widget]]\nkind = \"row\"\nwidgets = [{ kind = \"tabs\", tabs = [] }]\n".into(),
            ),
            (
                "row in row in tab",
                "[[widget]]\nkind = \"tabs\"\ntabs = [{ id = \"a\", text = \"A\", widgets = [{ kind = \"row\", widgets = [{ kind = \"row\", widgets = [{ kind = \"separator\" }] }] }] }, { id = \"b\", text = \"B\", widgets = [{ kind = \"separator\" }] }]\n".into(),
            ),
            (
                "bar min above max",
                "[[widget]]\nkind = \"bar\"\nid = \"b\"\ntext = \"B\"\nmin = 5.0\nmax = 1.0\n".into(),
            ),
            (
                "bar not a number",
                "[[widget]]\nkind = \"bar\"\nid = \"b\"\ntext = \"B\"\nmin = 0.0\nmax = nan\n".into(),
            ),
            (
                "bar infinite",
                "[[widget]]\nkind = \"bar\"\nid = \"b\"\ntext = \"B\"\nmin = 0.0\nmax = 1.0\nvalue = inf\n".into(),
            ),
            (
                "duplicate id button and lamp",
                "[[widget]]\nkind = \"button\"\nid = \"a\"\ntext = \"A\"\n[[widget]]\nkind = \"lamp\"\nid = \"a\"\ntext = \"A\"\n".into(),
            ),
            (
                "duplicate id rockers",
                format!("{0}{0}", rocker("")),
            ),
            (
                "rocker event meets a button",
                format!(
                    "{}[[widget]]\nkind = \"button\"\nid = \"r-up1\"\ntext = \"X\"\n",
                    rocker("")
                ),
            ),
            (
                "rocker event meets a lamp",
                format!(
                    "{}[[widget]]\nkind = \"lamp\"\nid = \"r-up1\"\ntext = \"X\"\n",
                    rocker("")
                ),
            ),
        ] {
            assert!(
                PanelLayout::parse(text.as_bytes()).is_err(),
                "{why} should be refused:\n{text}"
            );
        }
    }

    #[test]
    fn huge_counts_and_sizes_are_refused() {
        // Widgets nested in tabs count too.
        let sep = "{ kind = \"separator\" }";
        let tab = |id: &str| {
            format!(
                "{{ id = \"{id}\", text = \"T\", widgets = [{}] }}",
                vec![sep; 120].join(",")
            )
        };
        let text = format!(
            "[[widget]]\nkind = \"tabs\"\ntabs = [{}, {}]\n",
            tab("a"),
            tab("b")
        );
        assert!(refused(&text).contains("200"));
        // A rocker with many events is still one widget but its events are bounded by steps.
        let big = "x".repeat(MAX_BYTES + 1);
        assert!(
            refused(&format!("[[widget]]\nkind = \"label\"\ntext = \"{big}\"\n")).contains("128")
        );
        // Not text at all.
        assert!(PanelLayout::parse(&[0xff, 0xfe, 0x00]).is_err());
        // A thousand rockers.
        let mut many = String::new();
        for i in 0..1000 {
            many.push_str(&format!(
                "[[widget]]\nkind = \"rocker\"\nid = \"r{}\"\ntext = \"R\"\n",
                (b'a' + (i % 26) as u8) as char
            ));
        }
        assert!(PanelLayout::parse(many.as_bytes()).is_err());
    }

    #[test]
    fn deep_inline_nesting_does_not_overflow_the_parser() {
        let depth = 500;
        let text = format!(
            "[[widget]]\nkind = \"row\"\nwidgets = [{}{}]\n",
            "{ kind = \"row\", widgets = [".repeat(depth),
            "]}".repeat(depth)
        );
        assert!(PanelLayout::parse(text.as_bytes()).is_err());
    }
}
