//! Checks (design §5): the predicate that ticks a step.
//!
//! Two families. **Events** say what the person did (a command ran, an
//! anchor was clicked or opened); the runner latches them from the moment
//! the step opens. **Facts and state** say how things are now: the tool in
//! hand, the page shown, the selection, and what the document holds. State
//! is read from the project, so it does not care how the person got there
//! and it survives undo and redo.

use std::collections::BTreeSet;

use novo_core::{DeviceTag, Netlist, PinRef, PotentialKind, Project, Value};
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
    /// The application raised a named signal for something that does not
    /// pass through a command: the place wheel opened, Tab or Enter took a
    /// proposal. The names are in the registry the lint compares against.
    Signal(String),
}

/// How things are now, supplied by the application on each update.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Facts {
    /// The tool in hand: `select`, `place`, `wire`, `cable` or `text`.
    pub tool: String,
    /// Name of the page shown.
    pub page: Option<String>,
    /// Title of the page shown; a `page` check matches the name or the title.
    pub page_title: Option<String>,
    /// Tags of the selected devices.
    pub selection: Vec<String>,
    /// View zoom, 1.0 at fit.
    pub zoom: f64,
    /// Ids of design checks that currently report something.
    pub problems: BTreeSet<String>,
    /// The wiring mode setting: `corners` or `lines`.
    pub wiring_mode: String,
    /// Whether the live view is on.
    pub live: bool,
    /// Tags of the devices whose coil is energized in the live view.
    pub live_coils: BTreeSet<String>,
    /// Pin designations (`-K1:A1`) that carry voltage in the live view.
    pub live_pins: BTreeSet<String>,
    /// Tags of the devices with a pole closed in the live view.
    pub live_closed: BTreeSet<String>,
}

/// A check, as written in a lesson file (`expect = { ... }`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Check {
    Command(String),
    Tool(String),
    Opened(String),
    Clicked(String),
    Signal(String),
    Page(String),
    Selected(String),
    SelectedCount(usize),
    ZoomAtLeast(f64),
    WiringMode(String),
    Live(bool),
    CoilOn(String),
    Lit(String),
    PoleClosed(String),
    Potential(PotentialCheck),
    PotentialKind(PotentialKindCheck),
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

/// The pin `wire` (written `tag:pin`) is on a net that carries the named
/// potential, such as `L1`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotentialCheck {
    pub wire: String,
    pub is: String,
}

/// Some wire (on this page, when given) is on a net whose potential is of
/// this kind: `line`, `neutral`, `protective_earth`, `pen`, `dc_positive`,
/// `dc_negative`, ...
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PotentialKindCheck {
    pub kind: PotentialKind,
    #[serde(default)]
    pub page: Option<String>,
}

/// Two pins, written `tag:pin` as the inspector shows them, are on one net.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireCheck {
    pub from: String,
    pub to: String,
}

/// Devices exist: with this tag, with a symbol of this key, or both. Give
/// `symbol` alone when the tag is assigned by the program (auto tags); then
/// `at_least` counts the devices.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceCheck {
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(default = "one")]
    pub at_least: usize,
}

fn one() -> usize {
    1
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
    /// Potential feeds: feed symbols and pins that define a potential.
    Feed,
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
            Check::Signal(id) => cx.events.contains(&Event::Signal(id.clone())),
            Check::Tool(tool) => cx.facts.tool == *tool,
            Check::Page(name) => {
                cx.facts.page.as_deref() == Some(name.as_str())
                    || cx.facts.page_title.as_deref() == Some(name.as_str())
            }
            Check::Selected(tag) => {
                cx.facts.selection.len() == 1 && tag_matches_text(&cx.facts.selection[0], tag)
            }
            Check::SelectedCount(n) => cx.facts.selection.len() == *n,
            Check::ZoomAtLeast(z) => cx.facts.zoom >= *z,
            Check::WiringMode(mode) => cx.facts.wiring_mode == *mode,
            Check::Live(on) => cx.facts.live == *on,
            Check::CoilOn(tag) => cx
                .facts
                .live_coils
                .iter()
                .any(|have| tag_matches_text(have, tag)),
            Check::PoleClosed(tag) => cx
                .facts
                .live_closed
                .iter()
                .any(|have| tag_matches_text(have, tag)),
            Check::PotentialKind(p) => potential_kind_holds(cx, p),
            Check::Lit(pin) => cx.facts.live_pins.iter().any(|have| same_pin(have, pin)),
            Check::Potential(p) => potential_holds(cx, p),
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
            Check::Command(_) | Check::Clicked(_) | Check::Opened(_) | Check::Signal(_) => true,
            Check::All(v) | Check::Any(v) => v.iter().any(Check::reads_events),
            Check::Not(c) => c.reads_events(),
            _ => false,
        }
    }

    /// Shape rules a lesson parse cannot express.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Check::Device(d) if d.tag.is_none() && d.symbol.is_none() => {
                Err("a device check needs a tag, a symbol or both".to_owned())
            }
            Check::Device(d) if d.at_least == 0 => Err("at_least starts at 1".to_owned()),
            Check::All(v) | Check::Any(v) => v.iter().try_for_each(Check::validate),
            Check::Not(c) => c.validate(),
            _ => Ok(()),
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

    /// Every signal name the check names, for the lint.
    pub fn signals<'a>(&'a self, out: &mut Vec<&'a str>) {
        match self {
            Check::Signal(id) => out.push(id),
            Check::All(v) | Check::Any(v) => v.iter().for_each(|c| c.signals(out)),
            Check::Not(c) => c.signals(out),
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

/// Whether two `tag:pin` texts name the same pin.
fn same_pin(have: &str, want: &str) -> bool {
    match (have.rsplit_once(':'), want.rsplit_once(':')) {
        (Some((ht, hp)), Some((wt, wp))) => hp == wp && tag_matches_text(ht, wt),
        _ => false,
    }
}

fn potential_kind_holds(cx: &Context<'_>, check: &PotentialKindCheck) -> bool {
    cx.project
        .wires()
        .filter(|w| {
            check
                .page
                .as_ref()
                .is_none_or(|name| page_matches(cx.project, w.page, name))
        })
        .filter_map(|w| cx.netlist.net_of_wire(w))
        .filter_map(|net| cx.netlist.net(net).potential())
        .filter_map(|id| cx.project.potential(id))
        .any(|p| p.kind == check.kind)
}

fn potential_holds(cx: &Context<'_>, check: &PotentialCheck) -> bool {
    pins_named(cx.project, &check.wire).into_iter().any(|pin| {
        cx.netlist
            .potential_of(pin)
            .and_then(|id| cx.project.potential(id))
            .is_some_and(|p| p.name == check.is)
    })
}

fn wire_joined(cx: &Context<'_>, from: &str, to: &str) -> bool {
    let a = pins_named(cx.project, from);
    let b = pins_named(cx.project, to);
    a.iter()
        .any(|&x| b.iter().any(|&y| x != y && cx.netlist.connected(x, y)))
}

fn device_present(project: &Project, check: &DeviceCheck) -> bool {
    project
        .devices()
        .filter(|device| {
            check
                .tag
                .as_ref()
                .is_none_or(|tag| tag_matches(&device.tag, tag))
                && check.symbol.as_ref().is_none_or(|key| {
                    project
                        .symbols_of_device(device.id)
                        .iter()
                        .any(|s| s.symbol.0 == *key)
                })
        })
        .count()
        >= check.at_least
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
        CountKind::Feed => {
            project
                .symbols()
                .filter(|s| on_page(s.page))
                .filter(|s| matches!(s.role, novo_core::SymbolRole::PotentialFeed { .. }))
                .count()
                + project
                    .symbols()
                    .filter(|s| on_page(s.page))
                    .map(|s| s.feeds.len())
                    .sum::<usize>()
        }
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

#[cfg(test)]
mod tests {
    use novo_core::Document;

    use super::*;

    fn check(text: &str) -> Check {
        #[derive(Deserialize)]
        struct Holder {
            expect: Check,
        }
        toml::from_str::<Holder>(&format!("expect = {text}"))
            .unwrap()
            .expect
    }

    fn starter() -> Document {
        let mut doc = Document::new(novo_core::Project::new("T"));
        novo_tests::motor_starter(&mut doc).unwrap();
        doc
    }

    fn holds(check: &Check, doc: &Document, facts: &Facts, events: &[Event]) -> bool {
        let netlist = doc.project().netlist();
        let events: BTreeSet<Event> = events.iter().cloned().collect();
        check.eval(&Context {
            project: doc.project(),
            netlist: &netlist,
            facts,
            events: &events,
        })
    }

    #[test]
    fn events_facts_and_signals() {
        let doc = starter();
        let facts = Facts {
            tool: "wire".into(),
            page: Some("Main circuit".into()),
            selection: vec!["=A1+CAB-K1".into()],
            zoom: 2.0,
            wiring_mode: "lines".into(),
            live: true,
            live_coils: ["=A1+CAB-K1".to_owned()].into(),
            live_pins: ["=A1+CAB-K1:A1".to_owned()].into(),
            ..Facts::default()
        };
        let yes = |text: &str, events: &[Event]| holds(&check(text), &doc, &facts, events);
        assert!(yes("{ tool = \"wire\" }", &[]));
        assert!(!yes("{ tool = \"cable\" }", &[]));
        assert!(yes("{ page = \"Main circuit\" }", &[]));
        assert!(yes("{ selected = \"-K1\" }", &[]));
        assert!(!yes("{ selected = \"-F1\" }", &[]));
        assert!(yes("{ selected_count = 1 }", &[]));
        assert!(yes("{ zoom_at_least = 1.5 }", &[]));
        assert!(yes("{ wiring_mode = \"lines\" }", &[]));
        assert!(yes("{ live = true }", &[]));
        assert!(yes("{ coil_on = \"-K1\" }", &[]));
        assert!(!yes("{ coil_on = \"-K2\" }", &[]));
        assert!(yes("{ lit = \"-K1:A1\" }", &[]));
        assert!(!yes("{ lit = \"-K1:A2\" }", &[]));
        assert!(!yes("{ signal = \"wheel_opened\" }", &[]));
        assert!(yes(
            "{ signal = \"wheel_opened\" }",
            &[Event::Signal("wheel_opened".into())]
        ));
        assert!(yes(
            "{ any = [ { command = \"x\" }, { clicked = \"y\" } ] }",
            &[Event::Clicked("y".into())]
        ));
        assert!(yes("{ not = { command = \"x\" } }", &[]));
        assert!(check("{ signal = \"s\" }").reads_events());
        assert!(!check("{ live = true }").reads_events());
    }

    #[test]
    fn state_reads_the_document() {
        let doc = starter();
        let facts = Facts::default();
        let yes = |text: &str| holds(&check(text), &doc, &facts, &[]);
        assert!(yes("{ wire = { from = \"-F1:2\", to = \"-K1:1\" } }"));
        assert!(!yes("{ wire = { from = \"-F1:1\", to = \"-K1:6\" } }"));
        assert!(yes("{ device = { tag = \"-K1\" } }"));
        assert!(yes("{ device = { tag = \"=A1+CAB-K1\" } }"));
        assert!(!yes("{ device = { tag = \"=A2-K1\" } }"));
        assert!(!yes("{ device = { tag = \"-K9\" } }"));
        assert!(yes("{ count = { kind = \"page\", at_least = 2 } }"));
        assert!(!yes("{ count = { kind = \"page\", at_least = 3 } }"));
        assert!(yes(
            "{ count = { kind = \"symbol\", page = \"Control circuit\", at_least = 5 } }"
        ));
        assert!(yes("{ cable = { name = \"-W1\", cores_at_least = 4 } }"));
        assert!(!yes("{ cable = { name = \"-W2\" } }"));
        assert!(yes(
            "{ field = { device = \"-K1\", name = \"description\", is_set = true } }"
        ));
        assert!(!yes(
            "{ field = { device = \"-K1\", name = \"nothing\", is_set = true } }"
        ));
        assert!(yes(
            "{ field = { device = \"-K1\", name = \"nothing\", is_set = false } }"
        ));
        assert!(yes("{ problem_free = \"wire.overlap\" }"));
        let mut facts = Facts::default();
        facts.problems.insert("wire.overlap".into());
        assert!(!holds(
            &check("{ problem_free = \"wire.overlap\" }"),
            &doc,
            &facts,
            &[]
        ));
    }

    #[test]
    fn a_potential_is_read_from_the_net() {
        let doc = starter();
        let net = doc.project().netlist();
        let named: Vec<_> = doc
            .project()
            .potentials()
            .filter(|p| p.name == "L1")
            .map(|p| p.id)
            .collect();
        assert_eq!(named.len(), 1);
        // Some pin of -F1 sits on a net that carries exactly L1.
        let on_l1 = pins_named(doc.project(), "-F1:2")
            .into_iter()
            .chain(pins_named(doc.project(), "-F1:1"))
            .any(|pin| net.potential_of(pin) == Some(named[0]));
        let facts = Facts::default();
        let by_check = ["-F1:1", "-F1:2"].iter().any(|pin| {
            holds(
                &check(&format!(
                    "{{ potential = {{ wire = \"{pin}\", is = \"L1\" }} }}"
                )),
                &doc,
                &facts,
                &[],
            )
        });
        assert_eq!(on_l1, by_check);
        assert!(!holds(
            &check("{ potential = { wire = \"-F1:1\", is = \"NOPE\" } }"),
            &doc,
            &facts,
            &[]
        ));
    }

    #[test]
    fn devices_by_symbol_alone_and_counted() {
        let doc = starter();
        let facts = Facts::default();
        let any_symbol = doc
            .project()
            .symbols_of_device(doc.project().devices().next().unwrap().id)
            .first()
            .map(|s| s.symbol.0.clone())
            .unwrap();
        let yes = |text: &str| holds(&check(text), &doc, &facts, &[]);
        assert!(yes(&format!(
            "{{ device = {{ symbol = \"{any_symbol}\" }} }}"
        )));
        assert!(!yes("{ device = { symbol = \"iec/none/nothing\" } }"));
        let many = doc.project().devices().count();
        assert!(yes("{ device = { tag = \"-K1\", at_least = 1 } }"));
        assert!(!holds(
            &check(&format!(
                "{{ device = {{ symbol = \"{any_symbol}\", at_least = {} }} }}",
                many + 1
            )),
            &doc,
            &facts,
            &[]
        ));
        assert!(check("{ device = { symbol = \"x\" } }").validate().is_ok());
        assert!(check("{ device = { at_least = 2 } }").validate().is_err());
        assert!(
            check("{ all = [ { device = { at_least = 1 } } ] }")
                .validate()
                .is_err()
        );
    }

    #[test]
    fn feeds_potential_kinds_and_closed_poles() {
        let doc = starter();
        let mut facts = Facts::default();
        facts.live_closed.insert("=A1+CAB-K1".into());
        let yes = |text: &str, facts: &Facts| holds(&check(text), &doc, facts, &[]);
        assert!(yes("{ pole_closed = \"-K1\" }", &facts));
        assert!(!yes("{ pole_closed = \"-F1\" }", &facts));
        // Feed symbols plus pins that define a potential.
        let feeds: usize = doc
            .project()
            .symbols()
            .map(|s| {
                usize::from(matches!(
                    s.role,
                    novo_core::SymbolRole::PotentialFeed { .. }
                )) + s.feeds.len()
            })
            .sum();
        assert!(yes(
            &format!("{{ count = {{ kind = \"feed\", at_least = {feeds} }} }}"),
            &facts
        ));
        assert!(!yes(
            &format!(
                "{{ count = {{ kind = \"feed\", at_least = {} }} }}",
                feeds + 1
            ),
            &facts
        ));
        assert!(!yes(
            "{ count = { kind = \"feed\", at_least = 9999 } }",
            &facts
        ));
        let any_kind = doc
            .project()
            .wires()
            .filter_map(|w| {
                doc.project()
                    .netlist()
                    .net_of_wire(w)
                    .and_then(|n| doc.project().netlist().net(n).potential())
            })
            .filter_map(|id| doc.project().potential(id))
            .map(|p| p.kind)
            .next();
        if let Some(kind) = any_kind {
            let name = serde_json::to_string(&kind).unwrap();
            assert!(yes(
                &format!("{{ potential_kind = {{ kind = {name} }} }}"),
                &facts
            ));
        }
        assert!(!yes(
            "{ potential_kind = { kind = \"dc_negative\", page = \"nowhere\" } }",
            &facts
        ));
    }

    #[test]
    fn typos_and_new_kinds_do_not_parse() {
        #[derive(Deserialize)]
        struct Holder {
            #[allow(dead_code)]
            expect: Check,
        }
        assert!(toml::from_str::<Holder>("expect = { comand = \"x\" }").is_err());
        assert!(
            toml::from_str::<Holder>(
                "expect = { wire = { from = \"a:1\", to = \"b:1\", via = 3 } }"
            )
            .is_err()
        );
    }

    #[test]
    fn tags_match_by_product_or_in_full() {
        let full = DeviceTag::parse("=A1+CAB-K1").unwrap();
        assert!(tag_matches(&full, "-K1"));
        assert!(tag_matches(&full, "=A1+CAB-K1"));
        assert!(!tag_matches(&full, "=A2-K1"));
        assert!(!tag_matches(&full, "-K2"));
        assert!(!tag_matches(&full, "not a tag"));
    }
}
