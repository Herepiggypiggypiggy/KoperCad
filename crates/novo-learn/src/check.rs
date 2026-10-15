//! Checks (design §5): the predicate that ticks a step.
//!
//! Two families. **Events** say what the person did (a command ran, an
//! anchor was clicked or opened); the runner latches them from the moment
//! the step opens. **Facts and state** say how things are now: the tool in
//! hand, the page shown, the selection, and what the document holds. State
//! is read from the project, so it does not care how the person got there
//! and it survives undo and redo.

use std::collections::BTreeSet;

use novo_core::{DeviceTag, Netlist, PinRef, Project, Value};
use serde::Deserialize;

/// Something the person did, reported by the application.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Event {
    /// `run_command(id)` ran: a key, the palette, the wheel or a menu.
    Command(String),
    /// A control with this anchor id was clicked.
    Clicked(String),
    /// A panel, drawer or menu with this anchor id became visible.
    Opened(String),
}

/// How things are now, supplied by the application on each update.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Facts {
    /// The tool in hand: `select`, `place`, `wire`, `cable` or `text`.
    pub tool: String,
    /// Name or title of the page shown.
    pub page: Option<String>,
    /// Tags of the selected devices.
    pub selection: Vec<String>,
    /// View zoom, 1.0 at fit.
    pub zoom: f64,
    /// Ids of design checks that currently report something.
    pub problems: BTreeSet<String>,
}

/// A check, as written in a lesson file (`expect = { ... }`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    Command(String),
    Tool(String),
    Opened(String),
    Clicked(String),
    Page(String),
    Selected(String),
    SelectedCount(usize),
    ZoomAtLeast(f64),
    Wire(WireCheck),
    Device(DeviceCheck),
    Count(CountCheck),
    Field(FieldCheck),
    Cable(CableCheck),
    ProblemFree(String),
    All(Vec<Check>),
    Any(Vec<Check>),
    Not(Box<Check>),
}

/// Two pins, written `tag:pin` as the inspector shows them, are on one net.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireCheck {
    pub from: String,
    pub to: String,
}

/// A device with this tag exists, optionally with a symbol of this key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCheck {
    pub tag: String,
    #[serde(default)]
    pub symbol: Option<String>,
}

/// Which kind of thing a count check counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CountKind {
    Symbol,
    Wire,
    Device,
    Page,
    Cable,
    Text,
    Junction,
}

/// At least this many of a kind exist, optionally on one page or of one
/// symbol key.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountCheck {
    pub kind: CountKind,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    pub at_least: usize,
}

/// A property of a device is set, or equals a value. `description` and
/// `part` (a part is assigned) are built in; any other name is a property.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldCheck {
    pub device: String,
    pub name: String,
    #[serde(default)]
    pub is_set: Option<bool>,
    #[serde(default)]
    pub equals: Option<String>,
}

/// A cable with this tag exists, optionally with this many cores.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CableCheck {
    pub name: String,
    #[serde(default)]
    pub cores_at_least: usize,
}

/// What a check reads.
pub struct Context<'a> {
    pub project: &'a Project,
    pub netlist: &'a Netlist,
    pub facts: &'a Facts,
    pub events: &'a BTreeSet<Event>,
}

impl Check {
    /// Whether the check holds now.
    pub fn eval(&self, cx: &Context<'_>) -> bool {
        match self {
            Check::Command(id) => cx.events.contains(&Event::Command(id.clone())),
            Check::Clicked(id) => cx.events.contains(&Event::Clicked(id.clone())),
            Check::Opened(id) => cx.events.contains(&Event::Opened(id.clone())),
            Check::Tool(tool) => cx.facts.tool == *tool,
            Check::Page(name) => cx.facts.page.as_deref() == Some(name.as_str()),
            Check::Selected(tag) => {
                cx.facts.selection.len() == 1 && tag_matches_text(&cx.facts.selection[0], tag)
            }
            Check::SelectedCount(n) => cx.facts.selection.len() == *n,
            Check::ZoomAtLeast(z) => cx.facts.zoom >= *z,
            Check::Wire(w) => wire_joined(cx, &w.from, &w.to),
            Check::Device(d) => device_present(cx.project, d),
            Check::Count(c) => count(cx.project, c) >= c.at_least,
            Check::Field(f) => field_holds(cx.project, f),
            Check::Cable(c) => cx.project.cables().any(|cable| {
                tag_matches(&cable.tag, &c.name) && cable.cores.len() >= c.cores_at_least
            }),
            Check::ProblemFree(id) => !cx.facts.problems.contains(id),
            Check::All(all) => all.iter().all(|c| c.eval(cx)),
            Check::Any(any) => any.iter().any(|c| c.eval(cx)),
            Check::Not(c) => !c.eval(cx),
        }
    }

    /// Whether the check can only become true through something the person
    /// does (it reads latched events), so it is false when a step opens.
    pub fn reads_events(&self) -> bool {
        match self {
            Check::Command(_) | Check::Clicked(_) | Check::Opened(_) => true,
            Check::All(v) | Check::Any(v) => v.iter().any(Check::reads_events),
            Check::Not(c) => c.reads_events(),
            _ => false,
        }
    }

    /// Every command id the check names, for the lint.
    pub fn commands<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Check::Command(id) => out.push(id),
            Check::All(v) | Check::Any(v) => v.iter().for_each(|c| c.commands(out)),
            Check::Not(c) => c.commands(out),
            _ => {}
        }
    }

    /// Every anchor id the check names, for the lint.
    pub fn anchors<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Check::Opened(id) | Check::Clicked(id) => out.push(id),
            Check::All(v) | Check::Any(v) => v.iter().for_each(|c| c.anchors(out)),
            Check::Not(c) => c.anchors(out),
            _ => {}
        }
    }
}

/// Whether a lesson's tag text names a device tag. A tag written with only a
/// product level (`-K1`) matches the same product in any function or
/// location (`=A1+CAB1-K1`); a fuller tag must match exactly.
pub fn tag_matches(tag: &DeviceTag, text: &str) -> bool {
    let Ok(wanted) = DeviceTag::parse(text) else {
        return false;
    };
    if wanted.function.is_empty() && wanted.location.is_empty() {
        tag.product == wanted.product
    } else {
        *tag == wanted
    }
}

fn tag_matches_text(have: &str, text: &str) -> bool {
    DeviceTag::parse(have).is_ok_and(|tag| tag_matches(&tag, text))
}

/// Every pin whose designation (`-K1:13`, `-X1:3`) is `text`.
pub fn pins_named(project: &Project, text: &str) -> Vec<PinRef> {
    let Some((tag, pin)) = text.rsplit_once(':') else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for symbol in project.symbols() {
        for index in 0..symbol.pins.len() {
            let Ok(index) = u16::try_from(index) else {
                continue;
            };
            let pin_ref = PinRef::new(symbol.id, index);
            let Some(designation) = project.pin_designation(pin_ref) else {
                continue;
            };
            if let Some((have_tag, have_pin)) = designation.rsplit_once(':')
                && have_pin == pin
                && tag_matches_text(have_tag, tag)
            {
                found.push(pin_ref);
            }
        }
    }
    found
}

fn wire_joined(cx: &Context<'_>, from: &str, to: &str) -> bool {
    let a = pins_named(cx.project, from);
    let b = pins_named(cx.project, to);
    a.iter()
        .any(|&x| b.iter().any(|&y| x != y && cx.netlist.connected(x, y)))
}

fn device_present(project: &Project, check: &DeviceCheck) -> bool {
    project.devices().any(|device| {
        tag_matches(&device.tag, &check.tag)
            && check.symbol.as_ref().is_none_or(|key| {
                project
                    .symbols_of_device(device.id)
                    .iter()
                    .any(|s| s.symbol.0 == *key)
            })
    })
}

/// Whether a page is named by a lesson's text: by its name or its title.
pub fn page_matches(project: &Project, page: novo_core::PageId, text: &str) -> bool {
    project
        .page(page)
        .is_some_and(|p| p.name == text || p.title == text)
}

fn count(project: &Project, check: &CountCheck) -> usize {
    let on_page = |page| {
        check
            .page
            .as_ref()
            .is_none_or(|name| page_matches(project, page, name))
    };
    match check.kind {
        CountKind::Symbol => project
            .symbols()
            .filter(|s| on_page(s.page))
            .filter(|s| check.symbol.as_ref().is_none_or(|k| s.symbol.0 == *k))
            .count(),
        CountKind::Wire => project.wires().filter(|w| on_page(w.page)).count(),
        CountKind::Text => project.texts().filter(|t| on_page(t.page)).count(),
        CountKind::Junction => project.junctions().filter(|j| on_page(j.page)).count(),
        CountKind::Page => project.pages().count(),
        CountKind::Device => project.devices().count(),
        CountKind::Cable => project.cables().count(),
    }
}

fn field_holds(project: &Project, check: &FieldCheck) -> bool {
    let Some(device) = project
        .devices()
        .find(|d| tag_matches(&d.tag, &check.device))
    else {
        return false;
    };
    let text = match check.name.as_str() {
        "description" => Some(device.description.clone()).filter(|d| !d.is_empty()),
        "part" => device.parts.first().map(|_| String::from("part")),
        name => device.properties.get(name).map(|v| match v {
            Value::Bool(b) => b.to_string(),
            Value::Int(i) => i.to_string(),
            Value::Float(f) => f.to_string(),
            Value::Text(t) => t.clone(),
        }),
    };
    let is_set = text.is_some();
    check.is_set.is_none_or(|want| want == is_set)
        && check
            .equals
            .as_ref()
            .is_none_or(|want| text.as_deref() == Some(want.as_str()))
}
