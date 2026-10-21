//! The machine as the current choice of options (docs/design/variants.md).
//!
//! The working project always keeps every option's pages, devices and wires,
//! so switching an option back loses nothing. Everything that *describes the
//! machine* (the Reports workspace and the generated pages, the cut list and
//! the other wire production files, the bench pack, the workshop pack, the
//! labels and the PDF) reads it through [`NovoApp::machine`], which is the
//! project with what the current choice leaves out removed and the choice
//! named in the project name (so the title block and every report header say
//! which machine this is). A project without options is read as it is.
//!
//! The copy is made once per project revision, and only when an output asks.
//! The status bar shows the choice and how much it leaves out
//! ([`NovoApp::machine_info`]); `option <name> [value]` switches it from the
//! keyboard and `tie <option> [value]` ties the selection to it.

use std::cell::{Cell, RefCell};
use std::ops::Deref;
use std::rc::Rc;

use novo_core::{AnyId, Project, SymbolRole, Value};
use novo_workflow::options::{self, Difference, OptionDef};
use novo_workflow::parametric::ParamKind;

use super::reports::Reports;
use super::{Level, NovoApp};

/// What the current choice leaves out of the working project.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Off {
    pub pages: usize,
    pub devices: usize,
    pub wires: usize,
    /// Conditions that could not be evaluated; their objects stay in.
    pub errors: usize,
}

impl Off {
    pub(crate) fn nothing(&self) -> bool {
        self.pages + self.devices + self.wires == 0
    }
}

/// The choice in words, for the status bar and the Reports header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MachineInfo {
    pub summary: String,
    pub off: Off,
}

impl MachineInfo {
    /// "Left out of the documents: 1 page, 3 devices, 4 wires" or "Nothing is left out".
    pub(crate) fn left_out(&self) -> String {
        if self.off.nothing() {
            return "Nothing is left out of the documents".to_owned();
        }
        let mut parts = Vec::new();
        for (n, one, many) in [
            (self.off.pages, "page", "pages"),
            (self.off.devices, "device", "devices"),
            (self.off.wires, "wire", "wires"),
        ] {
            if n > 0 {
                parts.push(format!("{n} {}", if n == 1 { one } else { many }));
            }
        }
        format!("Left out of the documents: {}", parts.join(", "))
    }
}

struct Cache {
    /// Project revision, next id and name the cache was made for.
    key: (u64, u64, String),
    has_options: bool,
    info: MachineInfo,
    variant: Option<Rc<Project>>,
    generation: u64,
}

/// The machine view's state, owned by the workbench.
#[derive(Default)]
pub(crate) struct Machine {
    cache: RefCell<Option<Cache>>,
    generations: Cell<u64>,
    /// The live reports of the machine copy, used while the choice leaves
    /// something out or names the project.
    pub(super) reports: Reports,
}

/// The project as the current choice: the working project itself, or the
/// copy made for the choice. Derefs to a [`Project`].
pub(crate) enum MachineProject<'a> {
    Working(&'a Project),
    Choice(Rc<Project>),
}

impl Deref for MachineProject<'_> {
    type Target = Project;
    fn deref(&self) -> &Project {
        match self {
            MachineProject::Working(p) => p,
            MachineProject::Choice(p) => p,
        }
    }
}

impl NovoApp {
    /// Brings the cache up to the working project. False without a project.
    fn machine_refresh(&self) -> bool {
        let Some(doc) = self.backend.document() else {
            return false;
        };
        let project = doc.project();
        let key = (
            project.revision(),
            project.next_id(),
            project.info().name.clone(),
        );
        let mut cache = self.wb.machine.cache.borrow_mut();
        if cache.as_ref().is_some_and(|c| c.key == key) {
            return true;
        }
        let defs = options::options(project);
        let made = if defs.is_empty() {
            Cache {
                key,
                has_options: false,
                info: MachineInfo {
                    summary: String::new(),
                    off: Off::default(),
                },
                variant: None,
                generation: 0,
            }
        } else {
            let assignment = options::current(project);
            let evaluation = options::evaluate(project, &assignment);
            let count = |f: fn(&AnyId) -> bool| evaluation.inactive.iter().filter(|i| f(i)).count();
            Cache {
                key,
                has_options: true,
                info: MachineInfo {
                    summary: options::summary(project, &assignment),
                    off: Off {
                        pages: count(|i| matches!(i, AnyId::Page(_))),
                        devices: count(|i| matches!(i, AnyId::Device(_))),
                        wires: count(|i| matches!(i, AnyId::Wire(_))),
                        errors: evaluation.errors.len(),
                    },
                },
                variant: None,
                generation: 0,
            }
        };
        *cache = Some(made);
        true
    }

    /// The current choice, when the project has options.
    pub(crate) fn machine_info(&self) -> Option<MachineInfo> {
        if !self.machine_refresh() {
            return None;
        }
        let cache = self.wb.machine.cache.borrow();
        let c = cache.as_ref()?;
        c.has_options.then(|| c.info.clone())
    }

    /// The copy of the project for the current choice, made on first ask for
    /// each revision. `None` when the project has no options (outputs read the
    /// working project) or the copy could not be made.
    pub(crate) fn machine_variant(&self) -> Option<Rc<Project>> {
        if !self.machine_refresh() {
            return None;
        }
        let project = self.backend.document()?.project();
        let mut cache = self.wb.machine.cache.borrow_mut();
        let c = cache.as_mut()?;
        if !c.has_options {
            return None;
        }
        if c.variant.is_none() {
            let made = options::machine_project(project, &options::current(project)).ok()?;
            let generation = self.wb.machine.generations.get() + 1;
            self.wb.machine.generations.set(generation);
            c.generation = generation;
            c.variant = Some(Rc::new(made));
        }
        c.variant.clone()
    }

    /// Changes whenever the copy is made again; 0 for the working project.
    /// Caches keyed on a project revision add this, since a copy has its own
    /// revision numbers.
    pub(crate) fn machine_generation(&self) -> u64 {
        let _ = self.machine_variant();
        self.wb
            .machine
            .cache
            .borrow()
            .as_ref()
            .map_or(0, |c| c.generation)
    }

    /// The project outputs read: the current choice of the machine.
    pub(crate) fn machine(&self) -> Option<MachineProject<'_>> {
        let doc = self.backend.document()?;
        Some(match self.machine_variant() {
            Some(copy) => MachineProject::Choice(copy),
            None => MachineProject::Working(doc.project()),
        })
    }

    /// The live reports and the project they read, for the Reports
    /// workspace and the generated pages on the canvas.
    pub(crate) fn machine_reports(&mut self) -> Option<(&mut Reports, MachineProject<'_>)> {
        let copy = self.machine_variant();
        let doc = self.backend.document()?;
        Some(match copy {
            Some(copy) => (&mut self.wb.machine.reports, MachineProject::Choice(copy)),
            None => (&mut self.wb.reports, MachineProject::Working(doc.project())),
        })
    }

    /// The project's name with the choice after it (`Press line [Conveyor]`),
    /// for the headings of the documents.
    pub(crate) fn machine_title(&self) -> Option<String> {
        let name = self.project_name()?;
        Some(match self.machine_info() {
            Some(info) if !info.summary.is_empty() => format!("{name} [{}]", info.summary),
            _ => name,
        })
    }

    // ----- switching ------------------------------------------------------

    /// `option <name> [value]` (Ctrl+K): sets an option and says what that
    /// adds to or takes from the documents. Without an argument the Project
    /// options window opens.
    pub(super) fn option_command(&mut self, argument: Option<&str>) {
        let Some(argument) = argument.map(str::trim).filter(|a| !a.is_empty()) else {
            self.open_options();
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let defs = options::options(project);
        if defs.is_empty() {
            self.message(
                Level::Info,
                "This project has no options yet: add one in Project options (Ctrl+K options)",
            );
            return;
        }
        let before = options::current(project);
        let Some((def, rest)) = find_option(&defs, argument) else {
            let names: Vec<_> = defs.iter().map(|d| d.id.as_str()).collect();
            self.message(
                Level::Warning,
                format!(
                    "No option '{argument}'. This project has: {}",
                    names.join(", ")
                ),
            );
            return;
        };
        let current = before.get(&def.id).cloned().unwrap_or(def.default.clone());
        let Some(value) = parse_value(def, rest, &current) else {
            self.message(
                Level::Warning,
                format!("{}: {}", def.label(), value_help(def)),
            );
            return;
        };
        let (id, label) = (def.id.clone(), def.label().to_owned());
        if !self.edit_project("Project option", |tx| options::set_option(tx, &id, value)) {
            return;
        }
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let after = options::current(project);
        let difference = options::diff(project, &before, &after);
        let now = match after.get(&id) {
            Some(Value::Bool(true)) => "on".to_owned(),
            Some(Value::Bool(false)) => "off".to_owned(),
            Some(other) => other.to_string(),
            None => String::new(),
        };
        self.message(
            Level::Info,
            format!("{label} {now}: {}", difference_words(&difference)),
        );
    }

    /// What `tie` and `untie` act on: the devices and wires in the
    /// selection (a device through any of its symbols), else the page on
    /// screen.
    pub(crate) fn tie_targets(&self) -> Vec<AnyId> {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return Vec::new();
        };
        let mut out: Vec<AnyId> = Vec::new();
        for id in self.selection() {
            let target = match id {
                AnyId::Symbol(s) => match project.symbol(s).map(|s| &s.role) {
                    Some(SymbolRole::Device { device, .. }) => AnyId::Device(*device),
                    _ => continue,
                },
                AnyId::Wire(_) => id,
                _ => continue,
            };
            if !out.contains(&target) {
                out.push(target);
            }
        }
        if out.is_empty()
            && let Some(page) = self.current_page
        {
            out.push(AnyId::Page(page));
        }
        out
    }

    /// `tie <option> [value]`: the selection (or the page) appears only while
    /// the option has that value. A yes/no option means "on" when no value is
    /// given.
    pub(super) fn tie_command(&mut self, argument: Option<&str>) {
        let Some(argument) = argument.map(str::trim).filter(|a| !a.is_empty()) else {
            self.open_options();
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let defs = options::options(project);
        let Some((def, rest)) = find_option(&defs, argument) else {
            let names: Vec<_> = defs.iter().map(|d| d.id.as_str()).collect();
            self.message(
                Level::Warning,
                format!(
                    "No option '{argument}'. Add it in Project options first. This project has: {}",
                    if names.is_empty() {
                        "none".to_owned()
                    } else {
                        names.join(", ")
                    }
                ),
            );
            return;
        };
        let wanted = if rest.is_empty() && def.spec.kind == ParamKind::Bool {
            Some(Value::Bool(true))
        } else {
            parse_value(def, rest, &Value::Bool(true))
        };
        let Some(wanted) = wanted else {
            self.message(
                Level::Warning,
                format!("{}: {}", def.label(), value_help(def)),
            );
            return;
        };
        let def = def.clone();
        self.tie_selection(&def, &wanted);
    }

    /// Ties the selection (or the page) to `def` having `wanted`.
    pub(crate) fn tie_selection(&mut self, def: &OptionDef, wanted: &Value) -> bool {
        let targets = self.tie_targets();
        if targets.is_empty() {
            self.message(
                Level::Info,
                "Select devices or wires, or open a page, to tie",
            );
            return false;
        }
        let words = self.targets_words(&targets);
        if !self.edit_project("Tie to option", |tx| {
            options::tie_to(tx, &targets, def, wanted)
        }) {
            return false;
        }
        let when = options::condition_for(def, wanted)
            .ok()
            .and_then(|_| {
                let project = self.backend.document()?.project();
                options::describe_condition(project, *targets.first()?)
            })
            .unwrap_or_default();
        self.message(Level::Info, format!("{words} now appear only with: {when}"));
        true
    }

    /// `untie`: the selection (or the page) always appears again.
    pub(super) fn untie_command(&mut self) {
        let targets = self.tie_targets();
        let words = self.targets_words(&targets);
        if targets.is_empty() {
            return;
        }
        if self.edit_project("Untie from options", |tx| {
            for id in &targets {
                options::set_condition(tx, *id, None)?;
            }
            Ok(())
        }) {
            self.message(Level::Info, format!("{words} always appear"));
        }
    }

    /// "-K5, 2 wires" / "page 3".
    pub(crate) fn targets_words(&self, targets: &[AnyId]) -> String {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return String::new();
        };
        let mut devices: Vec<String> = Vec::new();
        let mut wires = 0usize;
        let mut pages: Vec<String> = Vec::new();
        for id in targets {
            match id {
                AnyId::Device(d) => {
                    devices.extend(project.device(*d).map(|d| d.tag.to_string()));
                }
                AnyId::Wire(_) => wires += 1,
                AnyId::Page(p) => {
                    pages.extend(project.page(*p).map(|p| format!("page {}", p.name)));
                }
                _ => {}
            }
        }
        let mut parts = devices;
        if wires > 0 {
            parts.push(format!("{wires} wire{}", if wires == 1 { "" } else { "s" }));
        }
        parts.extend(pages);
        parts.join(", ")
    }
}

/// The option an argument starts with (its id or its label, in any case) and
/// the rest of the argument.
fn find_option<'a>(defs: &'a [OptionDef], argument: &'a str) -> Option<(&'a OptionDef, &'a str)> {
    let lower = argument.to_lowercase();
    let mut best: Option<(&OptionDef, usize)> = None;
    for def in defs {
        for name in [def.id.as_str(), def.label()] {
            let n = name.to_lowercase();
            let fits = lower == n || lower.strip_prefix(&n).is_some_and(|r| r.starts_with(' '));
            if fits && best.is_none_or(|(_, len)| n.len() > len) {
                best = Some((def, n.len()));
            }
        }
    }
    let (def, len) = best?;
    // `to_lowercase` can change the length of exotic letters; fall back to
    // no remainder rather than slice inside a character.
    let rest = argument.get(len..).unwrap_or("").trim();
    Some((def, rest))
}

/// The value the user typed for an option. A yes/no option with nothing
/// typed flips.
fn parse_value(def: &OptionDef, text: &str, current: &Value) -> Option<Value> {
    let t = text.trim().to_lowercase();
    match def.spec.kind {
        ParamKind::Bool => match t.as_str() {
            "" => Some(Value::Bool(!matches!(current, Value::Bool(true)))),
            "on" | "yes" | "true" | "with" | "1" => Some(Value::Bool(true)),
            "off" | "no" | "false" | "without" | "0" => Some(Value::Bool(false)),
            _ => None,
        },
        ParamKind::Choice => def
            .spec
            .choices
            .iter()
            .find(|c| c.to_lowercase() == t)
            .cloned()
            .map(Value::Text),
        _ => t.parse::<f64>().ok().map(Value::Float),
    }
}

fn value_help(def: &OptionDef) -> String {
    match def.spec.kind {
        ParamKind::Bool => "say on or off".to_owned(),
        ParamKind::Choice => format!("one of {}", def.spec.choices.join(", ")),
        _ => "type a number".to_owned(),
    }
}

/// What a switch did to the documents, in one sentence.
pub(crate) fn difference_words(d: &Difference) -> String {
    let count =
        |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let side = |pages: usize, devices: usize| -> Option<String> {
        let mut parts = Vec::new();
        if pages > 0 {
            parts.push(count(pages, "page", "pages"));
        }
        if devices > 0 {
            parts.push(count(devices, "device", "devices"));
        }
        (!parts.is_empty()).then(|| parts.join(" and "))
    };
    let added = side(d.pages_added.len(), d.devices_added.len());
    let removed = side(d.pages_removed.len(), d.devices_removed.len());
    match (added, removed) {
        (Some(a), Some(r)) => format!("adds {a}, takes away {r}"),
        (Some(a), None) => format!("adds {a}"),
        (None, Some(r)) => format!("takes away {r}"),
        (None, None) if d.bom_changes.is_empty() => "no page or device changes".to_owned(),
        (None, None) => "the parts list changes".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Vec<OptionDef> {
        vec![
            OptionDef::yes_no("conveyor", "Conveyor", false),
            OptionDef::yes_no("conveyor_2", "Second conveyor", false),
            OptionDef::choice("supply", "Supply", &["400V", "480V"], "400V"),
            OptionDef::number("motors", "Motors", 1.0, 6.0, 1.0),
        ]
    }

    #[test]
    fn an_option_is_found_by_id_or_label_and_the_longest_name_wins() {
        let d = defs();
        let (o, rest) = find_option(&d, "conveyor on").unwrap();
        assert_eq!((o.id.as_str(), rest), ("conveyor", "on"));
        let (o, rest) = find_option(&d, "Second conveyor off").unwrap();
        assert_eq!((o.id.as_str(), rest), ("conveyor_2", "off"));
        let (o, rest) = find_option(&d, "conveyor_2").unwrap();
        assert_eq!((o.id.as_str(), rest), ("conveyor_2", ""));
        assert!(find_option(&d, "conveyors on").is_none());
        assert!(find_option(&d, "nothing").is_none());
    }

    #[test]
    fn values_are_read_by_the_options_kind() {
        let d = defs();
        let off = Value::Bool(false);
        assert_eq!(parse_value(&d[0], "", &off), Some(Value::Bool(true)));
        assert_eq!(
            parse_value(&d[0], "", &Value::Bool(true)),
            Some(off.clone())
        );
        assert_eq!(parse_value(&d[0], "With", &off), Some(Value::Bool(true)));
        assert_eq!(parse_value(&d[0], "maybe", &off), None);
        assert_eq!(
            parse_value(&d[2], "480v", &off),
            Some(Value::Text("480V".into()))
        );
        assert_eq!(parse_value(&d[2], "230V", &off), None);
        assert_eq!(parse_value(&d[3], "3", &off), Some(Value::Float(3.0)));
        assert_eq!(parse_value(&d[3], "three", &off), None);
    }

    #[test]
    fn the_difference_is_put_in_words() {
        let mut d = Difference::default();
        assert_eq!(difference_words(&d), "no page or device changes");
        d.pages_added = vec!["2".into()];
        d.devices_added = vec!["-M1".into(), "-K2".into()];
        assert_eq!(difference_words(&d), "adds 1 page and 2 devices");
        d.devices_removed = vec!["-K9".into()];
        assert_eq!(
            difference_words(&d),
            "adds 1 page and 2 devices, takes away 1 device"
        );
    }

    #[test]
    fn what_is_left_out_is_counted_in_words() {
        let mut info = MachineInfo {
            summary: "Conveyor".into(),
            off: Off::default(),
        };
        assert_eq!(info.left_out(), "Nothing is left out of the documents");
        info.off = Off {
            pages: 1,
            devices: 3,
            wires: 0,
            errors: 0,
        };
        assert_eq!(
            info.left_out(),
            "Left out of the documents: 1 page, 3 devices"
        );
    }
}
