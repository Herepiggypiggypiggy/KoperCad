//! Project options and variants: one project, many machines.
//!
//! A machine builder sells options: with or without a conveyor, 400 V or
//! 480 V, one motor or three. This module keeps the **options** of a project
//! (a yes/no, a choice, a number or a text, each with a default), the
//! **conditions** that tie pages, devices, cables and terminals to them, and
//! named **variants** (a saved set of option values: "Standard", "Export US").
//!
//! - Options and variants live in the project info, as JSON in two properties
//!   (`kc.options`, `kc.variants`); the current values are `kc.option.<id>`.
//!   Nothing in the model changes, and a project without options behaves as
//!   before.
//! - A **condition** is an expression over `opt.<id>` in the object's `kc.when`
//!   property ([`crate::expr`]). A page whose condition is false takes
//!   everything on it with it; a device takes its symbols and the wires on them.
//! - [`configuration`] shows what a set of option values includes: the active
//!   and inactive pages and devices, and the bill of materials of what stays.
//!   [`diff`] compares two sets, so "what does the conveyor add?" has an exact
//!   answer: these pages, these devices, these order numbers and quantities.
//! - [`variant_project`] makes the project as that variant: a copy with the
//!   inactive objects removed, ready for PDF, reports and exports, without
//!   touching the working project.
//! - [`lint`] finds conditions that cannot be evaluated and options nothing
//!   uses.
//!
//! The working project keeps everything, so switching the option back is
//! instant and loses nothing.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{AnyId, Endpoint, ModelError, PageId, Project, Result, SymbolRole, Tx, Value};
use serde::{Deserialize, Serialize};

use crate::expr::{Expr, ExprError, Val, Vars};
use crate::parametric::{Param, ParamKind, ParamSpec, Problem, coerce};

/// Project-info property holding the option definitions (JSON).
pub const OPTIONS_KEY: &str = "kc.options";
/// Project-info property holding the variants (JSON).
pub const VARIANTS_KEY: &str = "kc.variants";
/// Prefix of the current option values in the project info.
pub const VALUE_PREFIX: &str = "kc.option.";
/// Property key of an object's condition.
pub const CONDITION_KEY: &str = "kc.when";

/// One option of a project.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OptionDef {
    /// Lowercase letters, digits and `_`; used as `opt.<id>` in conditions.
    pub id: String,
    pub spec: ParamSpec,
    pub default: Value,
    #[serde(default)]
    pub description: String,
}

impl OptionDef {
    pub fn yes_no(id: &str, label: &str, default: bool) -> OptionDef {
        OptionDef {
            id: id.into(),
            spec: ParamSpec {
                label: label.into(),
                kind: ParamKind::Bool,
                ..ParamSpec::default()
            },
            default: Value::Bool(default),
            description: String::new(),
        }
    }

    pub fn choice(id: &str, label: &str, choices: &[&str], default: &str) -> OptionDef {
        OptionDef {
            id: id.into(),
            spec: ParamSpec {
                label: label.into(),
                kind: ParamKind::Choice,
                choices: choices.iter().map(|c| c.to_string()).collect(),
                ..ParamSpec::default()
            },
            default: Value::Text(default.into()),
            description: String::new(),
        }
    }

    pub fn number(id: &str, label: &str, min: f64, max: f64, default: f64) -> OptionDef {
        OptionDef {
            id: id.into(),
            spec: ParamSpec {
                label: label.into(),
                kind: ParamKind::Number,
                min: Some(min),
                max: Some(max),
                ..ParamSpec::default()
            },
            default: crate::expr::Val::Num(default).to_value(),
            description: String::new(),
        }
    }

    pub fn label(&self) -> &str {
        if self.spec.label.is_empty() {
            &self.id
        } else {
            &self.spec.label
        }
    }

    fn param(&self) -> Param {
        Param {
            name: self.id.clone(),
            default: self.default.clone(),
            description: self.description.clone(),
            spec: self.spec.clone(),
        }
    }

    /// Checks a value and returns it in the option's own type.
    pub fn check(&self, value: &Value) -> std::result::Result<Value, Problem> {
        coerce(&self.param(), value)
    }
}

/// A saved set of option values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variant {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Option id to value. Options not listed take their default.
    pub values: BTreeMap<String, Value>,
}

/// A full set of option values: every option has one.
pub type Assignment = BTreeMap<String, Value>;

fn read_json<T: for<'de> Deserialize<'de>>(project: &Project, key: &str) -> Vec<T> {
    match project.info().properties.get(key) {
        Some(Value::Text(json)) => serde_json::from_str(json).unwrap_or_default(),
        _ => Vec::new(),
    }
}

/// The options defined in the project.
pub fn options(project: &Project) -> Vec<OptionDef> {
    read_json(project, OPTIONS_KEY)
}

/// The saved variants.
pub fn variants(project: &Project) -> Vec<Variant> {
    read_json(project, VARIANTS_KEY)
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && !id.starts_with(|c: char| c.is_ascii_digit())
}

fn bad(msg: impl Into<String>) -> ModelError {
    ModelError::Format(msg.into())
}

/// Adds or replaces an option (by id). The current value is kept when it is
/// still allowed, else the default is taken.
pub fn define_option(tx: &mut Tx<'_>, def: OptionDef) -> Result<()> {
    if !valid_id(&def.id) {
        return Err(bad(format!(
            "'{}' is not a usable option id (lowercase letters, digits and _)",
            def.id
        )));
    }
    def.check(&def.default)
        .map_err(|e| bad(format!("default of {}: {e}", def.id)))?;
    let mut all = options(tx.project());
    match all.iter_mut().find(|o| o.id == def.id) {
        Some(slot) => *slot = def.clone(),
        None => all.push(def.clone()),
    }
    let json = serde_json::to_string(&all).expect("options serialize");
    let current = tx
        .project()
        .info()
        .properties
        .get(&format!("{VALUE_PREFIX}{}", def.id))
        .cloned();
    tx.set_info(|i| {
        i.properties.insert(OPTIONS_KEY.into(), Value::Text(json));
        if let Some(c) = &current
            && def.check(c).is_err()
        {
            i.properties.remove(&format!("{VALUE_PREFIX}{}", def.id));
        }
    })
}

/// Removes an option, its current value, and its value in every variant.
pub fn remove_option(tx: &mut Tx<'_>, id: &str) -> Result<()> {
    let mut all = options(tx.project());
    all.retain(|o| o.id != id);
    let mut vs = variants(tx.project());
    for v in &mut vs {
        v.values.remove(id);
    }
    let (o, v) = (
        serde_json::to_string(&all).unwrap(),
        serde_json::to_string(&vs).unwrap(),
    );
    tx.set_info(|i| {
        i.properties.insert(OPTIONS_KEY.into(), Value::Text(o));
        i.properties.insert(VARIANTS_KEY.into(), Value::Text(v));
        i.properties.remove(&format!("{VALUE_PREFIX}{id}"));
    })
}

/// The current value of every option: the chosen one, else the default.
pub fn current(project: &Project) -> Assignment {
    options(project)
        .into_iter()
        .map(|o| {
            let chosen = project
                .info()
                .properties
                .get(&format!("{VALUE_PREFIX}{}", o.id))
                .and_then(|v| o.check(v).ok());
            let value = chosen.unwrap_or_else(|| o.default.clone());
            (o.id, value)
        })
        .collect()
}

/// An assignment with the options' defaults under the given values.
pub fn complete(project: &Project, given: &BTreeMap<String, Value>) -> Result<Assignment> {
    let defs = options(project);
    for key in given.keys() {
        if !defs.iter().any(|o| &o.id == key) {
            return Err(bad(format!("'{key}' is not an option of this project")));
        }
    }
    defs.into_iter()
        .map(|o| {
            let value = match given.get(&o.id) {
                Some(v) => o.check(v).map_err(|e| bad(e.to_string()))?,
                None => o.default.clone(),
            };
            Ok((o.id, value))
        })
        .collect()
}

/// The names `opt.<id>` for conditions and macro expressions.
pub fn vars(assignment: &Assignment) -> Vars {
    assignment
        .iter()
        .map(|(k, v)| (format!("opt.{k}"), Val::from(v)))
        .collect()
}

/// Sets the current value of an option.
pub fn set_option(tx: &mut Tx<'_>, id: &str, value: Value) -> Result<()> {
    let def = options(tx.project())
        .into_iter()
        .find(|o| o.id == id)
        .ok_or_else(|| bad(format!("'{id}' is not an option of this project")))?;
    let value = def.check(&value).map_err(|e| bad(e.to_string()))?;
    tx.set_info(|i| {
        i.properties.insert(format!("{VALUE_PREFIX}{id}"), value);
    })
}

/// Saves the current values as a variant (replacing one of the same name).
pub fn save_variant(tx: &mut Tx<'_>, name: &str, description: &str) -> Result<()> {
    if name.trim().is_empty() {
        return Err(bad("a variant needs a name"));
    }
    let values = current(tx.project());
    let mut all = variants(tx.project());
    let v = Variant {
        name: name.trim().into(),
        description: description.into(),
        values,
    };
    match all
        .iter_mut()
        .find(|x| x.name.eq_ignore_ascii_case(&v.name))
    {
        Some(slot) => *slot = v,
        None => all.push(v),
    }
    let json = serde_json::to_string(&all).unwrap();
    tx.set_info(|i| {
        i.properties.insert(VARIANTS_KEY.into(), Value::Text(json));
    })
}

pub fn delete_variant(tx: &mut Tx<'_>, name: &str) -> Result<()> {
    let mut all = variants(tx.project());
    let before = all.len();
    all.retain(|v| !v.name.eq_ignore_ascii_case(name));
    if all.len() == before {
        return Err(bad(format!("no variant named '{name}'")));
    }
    let json = serde_json::to_string(&all).unwrap();
    tx.set_info(|i| {
        i.properties.insert(VARIANTS_KEY.into(), Value::Text(json));
    })
}

/// The values of a saved variant, completed with defaults.
pub fn variant_assignment(project: &Project, name: &str) -> Result<Assignment> {
    let v = variants(project)
        .into_iter()
        .find(|v| v.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| bad(format!("no variant named '{name}'")))?;
    complete(project, &v.values)
}

/// Makes a saved variant the current one.
pub fn apply_variant(tx: &mut Tx<'_>, name: &str) -> Result<()> {
    let assignment = variant_assignment(tx.project(), name)?;
    tx.set_info(|i| {
        for (k, v) in assignment {
            i.properties.insert(format!("{VALUE_PREFIX}{k}"), v);
        }
    })
}

/// Sets (`Some`) or clears (`None`) an object's condition. The expression is
/// parsed first, so a typo is refused here rather than found at export.
pub fn set_condition(tx: &mut Tx<'_>, id: impl Into<AnyId>, condition: Option<&str>) -> Result<()> {
    if let Some(c) = condition {
        Expr::parse(c).map_err(|e| bad(format!("condition: {e}")))?;
    }
    tx.set_property(id, CONDITION_KEY, condition.map(|c| Value::Text(c.into())))
}

/// The condition of an object, if it has one.
pub fn condition_of(project: &Project, id: impl Into<AnyId>) -> Option<String> {
    match project.record(id.into())?.properties().get(CONDITION_KEY) {
        Some(Value::Text(t)) => Some(t.clone()),
        _ => None,
    }
}

/// The condition that holds when option `def` has the value `wanted`:
/// `opt.conveyor`, `!opt.conveyor`, `opt.supply == '480V'` or, for a number,
/// `opt.motors == 3`. Refused for a value with a quote in it.
pub fn condition_for(def: &OptionDef, wanted: &Value) -> Result<String> {
    let wanted = def.check(wanted).map_err(|e| bad(e.to_string()))?;
    Ok(match wanted {
        Value::Bool(true) => format!("opt.{}", def.id),
        Value::Bool(false) => format!("!opt.{}", def.id),
        Value::Text(t) => {
            if t.contains('\'') {
                return Err(bad(
                    "a choice with a quote in it cannot be used in a condition",
                ));
            }
            format!("opt.{} == '{t}'", def.id)
        }
        other => format!("opt.{} == {other}", def.id),
    })
}

/// Ties objects to an option: each appears only while `def` has the value
/// `wanted`. Replaces any condition they had (the one condition is the whole
/// rule; a combined rule is typed in the Project options window).
pub fn tie_to(tx: &mut Tx<'_>, ids: &[AnyId], def: &OptionDef, wanted: &Value) -> Result<()> {
    let condition = condition_for(def, wanted)?;
    for id in ids {
        set_condition(tx, *id, Some(&condition))?;
    }
    Ok(())
}

/// Says an object's condition in words: "Conveyor", "not Conveyor",
/// "Supply 480V", or the expression itself when it is none of those. `None`
/// for an object with no condition.
pub fn describe_condition(project: &Project, id: impl Into<AnyId>) -> Option<String> {
    let condition = condition_of(project, id)?;
    for def in &options(project) {
        for wanted in wanted_values(def) {
            if condition_for(def, &wanted).is_ok_and(|c| c == condition) {
                return Some(match wanted {
                    Value::Bool(true) => def.label().to_owned(),
                    Value::Bool(false) => format!("not {}", def.label()),
                    other => format!("{} {other}", def.label()),
                });
            }
        }
    }
    Some(condition)
}

/// The values worth trying when reading a condition back: both answers of a
/// yes/no option and every choice.
fn wanted_values(def: &OptionDef) -> Vec<Value> {
    match def.spec.kind {
        ParamKind::Bool => vec![Value::Bool(true), Value::Bool(false)],
        ParamKind::Choice => def.spec.choices.iter().cloned().map(Value::Text).collect(),
        _ => Vec::new(),
    }
}

/// The current choice in a few words for a status bar or a title block:
/// "Conveyor, no Fan, Supply 480V". Yes/no options read as the option's
/// name or "no <name>"; the rest as "<name> <value>".
pub fn summary(project: &Project, assignment: &Assignment) -> String {
    options(project)
        .iter()
        .map(|def| {
            let value = assignment.get(&def.id).unwrap_or(&def.default);
            match value {
                Value::Bool(true) => def.label().to_owned(),
                Value::Bool(false) => format!("no {}", def.label()),
                other => format!("{} {other}", def.label()),
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// [`variant_project`] for an output: the same copy, with the project's name
/// carrying the choice (`Press line [Conveyor, no Fan]`), so the title block
/// and every report header say which machine the document is. A project
/// without options comes back unchanged.
pub fn machine_project(project: &Project, assignment: &Assignment) -> Result<Project> {
    let mut copy = variant_project(project, assignment)?;
    let label = summary(project, assignment);
    if !label.is_empty() {
        let name = format!("{} [{label}]", project.info().name);
        copy.transact("Name the variant", |tx| tx.set_info(|i| i.name = name))?;
    }
    Ok(copy)
}

/// Which objects a set of values leaves out, and what could not be decided.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluation {
    pub inactive: BTreeSet<AnyId>,
    /// Conditions that failed to evaluate: the object stays active and is
    /// listed here, never silently dropped.
    pub errors: Vec<(AnyId, ExprError)>,
}

impl Evaluation {
    pub fn is_active(&self, id: impl Into<AnyId>) -> bool {
        !self.inactive.contains(&id.into())
    }
}

/// Decides, for every object, whether the assignment includes it.
pub fn evaluate(project: &Project, assignment: &Assignment) -> Evaluation {
    let vars = vars(assignment);
    let mut out = Evaluation::default();
    // Borrowed walks over the tables, in the order `Project::records` gives
    // (cloning every record twice per pass cost 30 ms at 23,000 records).
    let mut judge = |id: AnyId, properties: &novo_core::Properties| {
        let Some(Value::Text(cond)) = properties.get(CONDITION_KEY) else {
            return;
        };
        match Expr::parse(cond)
            .and_then(|e| e.eval(&vars))
            .and_then(|v| v.as_bool())
        {
            Ok(true) => {}
            Ok(false) => {
                out.inactive.insert(id);
            }
            Err(e) => out.errors.push((id, e)),
        }
    };
    macro_rules! judge_all {
        ($($ty:ty),*) => {$(
            for e in project.all::<$ty>() {
                judge(e.id.into(), &e.properties);
            }
        )*};
    }
    {
        use novo_core::*;
        judge_all!(
            Part,
            Structure,
            Potential,
            Page,
            Device,
            Terminal,
            SymbolInstance,
            Wire,
            Cable,
            Text,
            PotentialSystem,
            Dismissal,
            BlockDefinition,
            BlockInstance,
            Revision,
            Rail,
            CabinetPlacement
        );
    }
    if out.inactive.is_empty() {
        // Nothing is switched off, so nothing spreads.
        return out;
    }
    // Spread: a page takes what is on it, a device its symbols, a symbol the
    // wires and texts on it, until nothing changes.
    loop {
        let before = out.inactive.len();
        let off = out.inactive.clone();
        for s in project.symbols() {
            let page_off = off.contains(&AnyId::from(s.page));
            let device_off = matches!(&s.role, SymbolRole::Device { device, .. } if off.contains(&AnyId::from(*device)));
            if page_off || device_off {
                out.inactive.insert(s.id.into());
            }
        }
        for j in project.junctions() {
            if off.contains(&AnyId::from(j.page)) {
                out.inactive.insert(j.id.into());
            }
        }
        for w in project.wires() {
            let end_off = |e: &Endpoint| match e {
                Endpoint::Pin { symbol, .. } => off.contains(&AnyId::from(*symbol)),
                Endpoint::Junction { junction } => off.contains(&AnyId::from(*junction)),
                Endpoint::Free { .. } => false,
            };
            if off.contains(&AnyId::from(w.page)) || end_off(&w.from) || end_off(&w.to) {
                out.inactive.insert(w.id.into());
            }
        }
        for t in project.texts() {
            if off.contains(&AnyId::from(t.page)) || t.attached_to.is_some_and(|a| off.contains(&a))
            {
                out.inactive.insert(t.id.into());
            }
        }
        for t in project.all::<novo_core::Terminal>() {
            if off.contains(&AnyId::from(t.strip)) {
                out.inactive.insert(t.id.into());
            }
        }
        for c in project.all::<novo_core::CabinetPlacement>() {
            if off.contains(&AnyId::from(c.device)) {
                out.inactive.insert(c.id.into());
            }
        }
        // A device drawn only on inactive symbols is inactive too, and a
        // cable all of whose wired cores are inactive.
        for d in project.devices() {
            let symbols = project.symbols_of_device(d.id);
            if !symbols.is_empty()
                && symbols
                    .iter()
                    .all(|s| out.inactive.contains(&AnyId::from(s.id)))
            {
                out.inactive.insert(d.id.into());
            }
        }
        for c in project.cables() {
            let wires: Vec<_> = c.cores.iter().filter_map(|k| k.wire).collect();
            if !wires.is_empty()
                && wires
                    .iter()
                    .all(|w| out.inactive.contains(&AnyId::from(*w)))
            {
                out.inactive.insert(c.id.into());
            }
        }
        if out.inactive.len() == before {
            break;
        }
    }
    out
}

/// One row of the page list in a configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageRow {
    pub id: PageId,
    pub name: String,
    pub title: String,
    pub active: bool,
}

/// One line of the bill of materials of a configuration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct BomLine {
    pub manufacturer: String,
    pub order_number: String,
    pub description: String,
    pub quantity: u32,
}

/// What a set of option values includes.
#[derive(Debug, Clone, PartialEq)]
pub struct Configuration {
    pub assignment: Assignment,
    pub pages: Vec<PageRow>,
    /// Active devices as tags; inactive ones in `devices_off`.
    pub devices: Vec<String>,
    pub devices_off: Vec<String>,
    /// Active devices, cables and terminals with no part assigned.
    pub unassigned: Vec<String>,
    pub bom: Vec<BomLine>,
    pub errors: Vec<(AnyId, ExprError)>,
}

/// Evaluates the project under `assignment`.
pub fn configuration(project: &Project, assignment: &Assignment) -> Configuration {
    let eval = evaluate(project, assignment);
    let pages = project
        .pages_in_order()
        .into_iter()
        .map(|p| PageRow {
            id: p.id,
            name: p.name.clone(),
            title: p.title.clone(),
            active: eval.is_active(p.id),
        })
        .collect();
    let mut devices = Vec::new();
    let mut devices_off = Vec::new();
    let mut unassigned = Vec::new();
    let mut counts: BTreeMap<(String, String), (String, u32)> = BTreeMap::new();
    let mut add_part = |project: &Project, part: novo_core::PartId| {
        if let Some(p) = project.part(part) {
            let e = counts
                .entry((p.manufacturer.clone(), p.order_number.clone()))
                .or_insert_with(|| (p.description.clone(), 0));
            e.1 += 1;
        }
    };
    for d in project.devices() {
        let tag = d.tag.to_string();
        if !eval.is_active(d.id) {
            devices_off.push(tag);
            continue;
        }
        if d.parts.is_empty() && d.kind != novo_core::DeviceKind::TerminalStrip {
            unassigned.push(tag.clone());
        }
        for &part in &d.parts {
            add_part(project, part);
        }
        devices.push(tag);
    }
    for t in project.terminals() {
        if eval.is_active(t.id)
            && let Some(p) = t.part
        {
            add_part(project, p);
        }
    }
    for c in project.cables() {
        if eval.is_active(c.id) {
            match c.part {
                Some(p) => add_part(project, p),
                None => unassigned.push(c.tag.to_string()),
            }
        }
    }
    devices.sort();
    devices_off.sort();
    unassigned.sort();
    let bom = counts
        .into_iter()
        .map(
            |((manufacturer, order_number), (description, quantity))| BomLine {
                manufacturer,
                order_number,
                description,
                quantity,
            },
        )
        .collect();
    Configuration {
        assignment: assignment.clone(),
        pages,
        devices,
        devices_off,
        unassigned,
        bom,
        errors: eval.errors,
    }
}

/// What changes from one set of option values to another.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Difference {
    pub options_changed: Vec<(String, Value, Value)>,
    pub pages_added: Vec<String>,
    pub pages_removed: Vec<String>,
    pub devices_added: Vec<String>,
    pub devices_removed: Vec<String>,
    /// Order number with its quantity before and after.
    pub bom_changes: Vec<(BomKey, u32, u32)>,
}

/// Manufacturer and order number.
pub type BomKey = (String, String);

impl Difference {
    pub fn is_empty(&self) -> bool {
        self.pages_added.is_empty()
            && self.pages_removed.is_empty()
            && self.devices_added.is_empty()
            && self.devices_removed.is_empty()
            && self.bom_changes.is_empty()
    }
}

/// Compares two sets of option values: what `after` adds to and takes from
/// `before`.
pub fn diff(project: &Project, before: &Assignment, after: &Assignment) -> Difference {
    let a = configuration(project, before);
    let b = configuration(project, after);
    let mut d = Difference::default();
    for (k, v) in after {
        match before.get(k) {
            Some(old) if old == v => {}
            old => d.options_changed.push((
                k.clone(),
                old.cloned().unwrap_or(Value::Bool(false)),
                v.clone(),
            )),
        }
    }
    let label = |p: &PageRow| {
        if p.title.is_empty() {
            p.name.clone()
        } else {
            format!("{} {}", p.name, p.title)
        }
    };
    let active = |c: &Configuration| -> BTreeSet<String> {
        c.pages.iter().filter(|p| p.active).map(label).collect()
    };
    let (pa, pb) = (active(&a), active(&b));
    d.pages_added = pb.difference(&pa).cloned().collect();
    d.pages_removed = pa.difference(&pb).cloned().collect();
    let (da, db): (BTreeSet<_>, BTreeSet<_>) = (
        a.devices.iter().cloned().collect(),
        b.devices.iter().cloned().collect(),
    );
    d.devices_added = db.difference(&da).cloned().collect();
    d.devices_removed = da.difference(&db).cloned().collect();
    let qa: BTreeMap<BomKey, u32> = a
        .bom
        .iter()
        .map(|l| ((l.manufacturer.clone(), l.order_number.clone()), l.quantity))
        .collect();
    let qb: BTreeMap<BomKey, u32> = b
        .bom
        .iter()
        .map(|l| ((l.manufacturer.clone(), l.order_number.clone()), l.quantity))
        .collect();
    let keys: BTreeSet<&BomKey> = qa.keys().chain(qb.keys()).collect();
    for k in keys {
        let (x, y) = (
            qa.get(k).copied().unwrap_or(0),
            qb.get(k).copied().unwrap_or(0),
        );
        if x != y {
            d.bom_changes.push((k.clone(), x, y));
        }
    }
    d
}

/// The project as one variant: a copy with every inactive object removed.
/// The working project is untouched; export this copy to get the documents
/// of that machine.
pub fn variant_project(project: &Project, assignment: &Assignment) -> Result<Project> {
    let eval = evaluate(project, assignment);
    let mut copy = project.clone();
    copy.transact("Make variant", |tx| {
        // Pages first (they take their contents), then devices and cables.
        let mut order: Vec<AnyId> = eval.inactive.iter().copied().collect();
        order.sort_by_key(|id| match id {
            AnyId::Page(_) => 0,
            AnyId::Device(_) => 1,
            AnyId::Cable(_) => 2,
            _ => 3,
        });
        for id in order {
            // Already gone with its page or device.
            if tx.project().contains(id) {
                tx.remove(id)?;
            }
        }
        Ok(())
    })?;
    Ok(copy)
}

/// A finding of [`lint`].
#[derive(Debug, Clone, PartialEq)]
pub enum Finding {
    /// The condition cannot be evaluated (typo, unknown option, wrong type).
    BadCondition { object: AnyId, error: ExprError },
    /// No condition and no macro reads this option.
    UnusedOption(String),
    /// Two options (or a variant) disagree about a name: a variant sets an
    /// option that does not exist.
    UnknownInVariant { variant: String, option: String },
}

/// Checks the conditions of the project against its options.
pub fn lint(project: &Project) -> Vec<Finding> {
    let defs = options(project);
    let known = vars(&complete(project, &BTreeMap::new()).unwrap_or_default());
    let mut findings = Vec::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    for record in project.records() {
        let Some(Value::Text(cond)) = record.properties().get(CONDITION_KEY) else {
            continue;
        };
        match Expr::parse(cond) {
            Ok(e) => {
                used.extend(
                    e.names()
                        .into_iter()
                        .filter_map(|n| n.strip_prefix("opt.").map(String::from)),
                );
                if let Err(error) = e.eval(&known).and_then(|v| v.as_bool()) {
                    findings.push(Finding::BadCondition {
                        object: record.id(),
                        error,
                    });
                }
            }
            Err(error) => findings.push(Finding::BadCondition {
                object: record.id(),
                error,
            }),
        }
    }
    for def in project.block_definitions() {
        for p in def.properties.values() {
            if let Value::Text(t) = p {
                for o in &defs {
                    if t.contains(&format!("opt.{}", o.id)) {
                        used.insert(o.id.clone());
                    }
                }
            }
        }
        for m in &def.members {
            if let Some(Value::Text(t)) = m.properties().get(crate::parametric::WHEN_KEY) {
                for o in &defs {
                    if t.contains(&format!("opt.{}", o.id)) {
                        used.insert(o.id.clone());
                    }
                }
            }
        }
    }
    for o in &defs {
        if !used.contains(&o.id) {
            findings.push(Finding::UnusedOption(o.id.clone()));
        }
    }
    for v in variants(project) {
        for k in v.values.keys() {
            if !defs.iter().any(|o| &o.id == k) {
                findings.push(Finding::UnknownInVariant {
                    variant: v.name.clone(),
                    option: k.clone(),
                });
            }
        }
    }
    findings
}

/// Every combination of the yes/no and choice options, up to `limit`, for
/// checking each variant of a product line. Number and text options keep
/// their current value.
pub fn combinations(project: &Project, limit: usize) -> Vec<Assignment> {
    let defs = options(project);
    let base = current(project);
    let mut all = vec![base];
    for def in &defs {
        let values: Vec<Value> = match def.spec.kind {
            ParamKind::Bool => vec![Value::Bool(false), Value::Bool(true)],
            ParamKind::Choice => def
                .spec
                .choices
                .iter()
                .filter_map(|c| def.check(&Value::Text(c.clone())).ok())
                .collect(),
            _ => continue,
        };
        let mut next = Vec::new();
        for a in &all {
            for v in &values {
                let mut b = a.clone();
                b.insert(def.id.clone(), v.clone());
                next.push(b);
                if next.len() >= limit {
                    return next;
                }
            }
        }
        all = next;
    }
    all.truncate(limit);
    all
}
