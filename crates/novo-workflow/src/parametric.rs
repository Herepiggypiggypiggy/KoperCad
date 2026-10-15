//! Parametric macros: typed parameters, calculated values, optional parts
//! and repeats, on top of the plain block parameters of `novo-core`.
//!
//! A block definition already carries named parameters with defaults and
//! fills `{name}` into texts and properties. This module adds, without any
//! change to the model, what a machine builder needs from a macro:
//!
//! - **Typed parameters** ([`ParamSpec`]): a number with a range and unit, a
//!   choice from a list, a yes/no, a text. A value outside the range or not in
//!   the list is refused with a message that names the parameter.
//! - **Calculated parameters**: a spec with `derive` is computed from the
//!   others with the expression language of [`crate::expr`], for example the
//!   breaker rating from the motor power. A calculated parameter is not asked.
//! - **Optional parts**: a template member with a `kc.if` property (an
//!   expression) is placed only when it is true. The overload relay exists
//!   only `when power > 0.75`; a reversing contactor only `when reversing`. Wires
//!   and texts that would dangle go with the part.
//! - **Repeat**: a `kc.repeat` property (`{"count": "motors", "pitch": [60, 0]}`)
//!   places the macro `count` times side by side, each with its own `index`.
//!
//! The specs live in the definition's `properties` (`kc.param.<name>`,
//! `kc.if`, `kc.repeat`), so they travel in a `.kcmacro` file and in a
//! project file unchanged, and an older KoperCAD simply ignores them.
//!
//! Expressions can read the project options as `opt.<name>`, so a macro can
//! adapt to the variant it is placed into.

use std::collections::{BTreeMap, BTreeSet};

use novo_core::{
    AnyId, BlockDefinition, BlockParameter, Endpoint, PageId, Point, Record, Result as ModelResult,
    SymbolRole, Tx, Value,
};
use serde::{Deserialize, Serialize};

use crate::expr::{Expr, ExprError, Val, Vars};
use crate::macro_place::{PlaceOptions, Placed, WireNumbers, place_macro};

/// Property key prefix of a parameter's spec.
pub const PARAM_PREFIX: &str = "kc.param.";
/// Property key of a template member's condition (`kc.when` is the
/// project-level condition of [`crate::options`]; a placed macro keeps no
/// `kc.if`, its choices were made when it was placed).
pub const WHEN_KEY: &str = "kc.if";
/// Property key of the repeat spec.
pub const REPEAT_KEY: &str = "kc.repeat";
/// Property on a filtered definition naming the macro it came from.
pub const BASE_KEY: &str = "kc.base";
/// The most copies one placement makes, so a typo cannot draw a thousand.
pub const MAX_REPEAT: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    /// Taken from the default's type.
    #[default]
    Auto,
    Number,
    Choice,
    Bool,
    Text,
}

/// What a parameter is and what it may be.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ParamSpec {
    /// Shown next to the field; the name when empty.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
    #[serde(default)]
    pub kind: ParamKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub unit: String,
    /// The allowed values of a choice, as they are written.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub choices: Vec<String>,
    /// Calculated from the other parameters instead of asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derive: Option<String>,
    /// Groups fields in the form.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub group: String,
}

/// The repeat spec of a macro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RepeatSpec {
    /// An expression for the number of copies (`motors`, `1`).
    pub count: String,
    /// Offset of each copy from the one before, in millimetres.
    pub pitch: [f64; 2],
}

/// A parameter of a definition with its spec.
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub default: Value,
    pub description: String,
    pub spec: ParamSpec,
}

impl Param {
    pub fn kind(&self) -> ParamKind {
        match self.spec.kind {
            ParamKind::Auto => match (&self.default, self.spec.choices.is_empty()) {
                (_, false) => ParamKind::Choice,
                (Value::Bool(_), _) => ParamKind::Bool,
                (Value::Int(_) | Value::Float(_), _) => ParamKind::Number,
                (Value::Text(_), _) => ParamKind::Text,
            },
            k => k,
        }
    }

    pub fn is_derived(&self) -> bool {
        self.spec.derive.is_some()
    }

    pub fn label(&self) -> &str {
        if self.spec.label.is_empty() {
            &self.name
        } else {
            &self.spec.label
        }
    }
}

/// Why a placement cannot be made as asked. Each names the parameter.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum Problem {
    #[error("'{0}' is not a parameter of this macro")]
    UnknownParameter(String),
    #[error("{name}: '{value}' is not a number")]
    NotANumber { name: String, value: String },
    #[error("{name}: {value} is below the minimum {min}")]
    BelowMin {
        name: String,
        value: String,
        min: f64,
    },
    #[error("{name}: {value} is above the maximum {max}")]
    AboveMax {
        name: String,
        value: String,
        max: f64,
    },
    #[error("{name}: '{value}' is not one of {choices}")]
    NotAChoice {
        name: String,
        value: String,
        choices: String,
    },
    #[error("{name}: '{value}' is not yes or no")]
    NotABool { name: String, value: String },
    #[error("{0} is calculated and cannot be set")]
    Derived(String),
    #[error("{name}: {error}")]
    Expression { name: String, error: ExprError },
    #[error("repeat count: {0}")]
    Repeat(String),
}

pub type Problems = Vec<Problem>;

fn spec_of(def: &BlockDefinition, name: &str) -> ParamSpec {
    match def.properties.get(&format!("{PARAM_PREFIX}{name}")) {
        Some(Value::Text(json)) => serde_json::from_str(json).unwrap_or_default(),
        _ => ParamSpec::default(),
    }
}

/// The definition's parameters with their specs, in definition order.
pub fn params(def: &BlockDefinition) -> Vec<Param> {
    def.parameters
        .iter()
        .map(|p| Param {
            name: p.name.clone(),
            default: p.default.clone(),
            description: p.description.clone(),
            spec: spec_of(def, &p.name),
        })
        .collect()
}

/// Declares a parameter (adding it when the definition lacks it) with its spec.
pub fn set_param(def: &mut BlockDefinition, name: &str, default: Value, spec: ParamSpec) {
    match def.parameters.iter_mut().find(|p| p.name == name) {
        Some(p) => p.default = default,
        None => def.parameters.push(BlockParameter {
            name: name.into(),
            default,
            description: String::new(),
        }),
    }
    let json = serde_json::to_string(&spec).expect("specs serialize");
    def.properties
        .insert(format!("{PARAM_PREFIX}{name}"), Value::Text(json));
}

/// Sets the condition of a template member (`None` clears it).
pub fn set_when(def: &mut BlockDefinition, member: AnyId, condition: Option<&str>) {
    for record in &mut def.members {
        if record.id() == member {
            if let Some(props) = record.properties_mut() {
                match condition {
                    Some(c) => props.insert(WHEN_KEY.into(), Value::Text(c.into())),
                    None => props.remove(WHEN_KEY),
                };
            }
        }
    }
}

pub fn set_repeat(def: &mut BlockDefinition, repeat: Option<&RepeatSpec>) {
    match repeat {
        Some(r) => def.properties.insert(
            REPEAT_KEY.into(),
            Value::Text(serde_json::to_string(r).expect("specs serialize")),
        ),
        None => def.properties.remove(REPEAT_KEY),
    };
}

pub fn repeat_of(def: &BlockDefinition) -> Option<RepeatSpec> {
    match def.properties.get(REPEAT_KEY) {
        Some(Value::Text(json)) => serde_json::from_str(json).ok(),
        _ => None,
    }
}

/// The macro a (possibly filtered) definition was made from.
pub fn base_name(def: &BlockDefinition) -> &str {
    match def.properties.get(BASE_KEY) {
        Some(Value::Text(b)) => b,
        _ => &def.name,
    }
}

fn vars_of(values: &[(String, Value)], options: &Vars) -> Vars {
    let mut vars = options.clone();
    for (name, value) in values {
        vars.insert(name.clone(), Val::from(value));
    }
    vars
}

/// Reads a given value as the parameter's kind and checks its range.
pub(crate) fn coerce(param: &Param, given: &Value) -> Result<Value, Problem> {
    let name = param.name.clone();
    let shown = given.to_string();
    match param.kind() {
        ParamKind::Number => {
            let n =
                match given {
                    Value::Int(i) => *i as f64,
                    Value::Float(f) => *f,
                    Value::Text(t) => t.trim().replace(',', ".").parse::<f64>().map_err(|_| {
                        Problem::NotANumber {
                            name: name.clone(),
                            value: shown.clone(),
                        }
                    })?,
                    Value::Bool(_) => {
                        return Err(Problem::NotANumber { name, value: shown });
                    }
                };
            if let Some(min) = param.spec.min {
                if n < min {
                    return Err(Problem::BelowMin {
                        name,
                        value: shown,
                        min,
                    });
                }
            }
            if let Some(max) = param.spec.max {
                if n > max {
                    return Err(Problem::AboveMax {
                        name,
                        value: shown,
                        max,
                    });
                }
            }
            Ok(Val::Num(n).to_value())
        }
        ParamKind::Choice => {
            let hit = param
                .spec
                .choices
                .iter()
                .find(|c| c.eq_ignore_ascii_case(shown.trim()));
            match hit {
                Some(c) => Ok(match &param.default {
                    Value::Int(_) | Value::Float(_) => c
                        .parse::<f64>()
                        .map(|n| Val::Num(n).to_value())
                        .unwrap_or_else(|_| Value::Text(c.clone())),
                    _ => Value::Text(c.clone()),
                }),
                None => Err(Problem::NotAChoice {
                    name,
                    value: shown,
                    choices: param.spec.choices.join(", "),
                }),
            }
        }
        ParamKind::Bool => match given {
            Value::Bool(b) => Ok(Value::Bool(*b)),
            other => match other.to_string().trim().to_lowercase().as_str() {
                "true" | "yes" | "ja" | "1" | "x" => Ok(Value::Bool(true)),
                "false" | "no" | "nee" | "0" | "" => Ok(Value::Bool(false)),
                _ => Err(Problem::NotABool { name, value: shown }),
            },
        },
        ParamKind::Text | ParamKind::Auto => Ok(Value::Text(shown)),
    }
}

/// Every parameter with the value a placement would use: asked values
/// checked, missing ones defaulted, calculated ones computed.
pub fn resolve(
    def: &BlockDefinition,
    given: &[(String, Value)],
    options: &Vars,
) -> Result<Vec<(String, Value)>, Problems> {
    let all = params(def);
    let mut problems = Problems::new();
    for (name, _) in given {
        match all.iter().find(|p| &p.name == name) {
            None => problems.push(Problem::UnknownParameter(name.clone())),
            Some(p) if p.is_derived() => problems.push(Problem::Derived(name.clone())),
            Some(_) => {}
        }
    }
    let mut values: Vec<(String, Value)> = Vec::new();
    for p in all.iter().filter(|p| !p.is_derived()) {
        let raw = given
            .iter()
            .rev()
            .find(|(n, _)| *n == p.name)
            .map_or(&p.default, |(_, v)| v);
        match coerce(p, raw) {
            Ok(v) => values.push((p.name.clone(), v)),
            Err(e) => problems.push(e),
        }
    }
    // Calculated ones, in definition order; a later one may read an earlier one.
    for p in all.iter().filter(|p| p.is_derived()) {
        let src = p.spec.derive.as_deref().unwrap_or_default();
        let result = Expr::parse(src).and_then(|e| e.eval(&vars_of(&values, options)));
        match result {
            Ok(v) => values.push((p.name.clone(), v.to_value())),
            Err(error) => problems.push(Problem::Expression {
                name: p.name.clone(),
                error,
            }),
        }
    }
    if problems.is_empty() {
        // Keep the definition's order, as `place_block` writes them.
        values.sort_by_key(|(n, _)| all.iter().position(|p| &p.name == n));
        Ok(values)
    } else {
        Err(problems)
    }
}

/// A placement ready to make.
#[derive(Debug, Clone)]
pub struct Instantiation {
    /// The definition to place: the original, or a copy without the optional
    /// parts that are off (named `<name>~<key>`, see [`BASE_KEY`]).
    pub definition: BlockDefinition,
    pub values: Vec<(String, Value)>,
    /// How many copies, and the offset between them.
    pub count: usize,
    pub pitch: Point,
    /// Template members left out, for the preview.
    pub left_out: usize,
}

/// Resolves the values and drops the optional parts that are off.
pub fn instantiate(
    def: &BlockDefinition,
    given: &[(String, Value)],
    options: &Vars,
) -> Result<Instantiation, Problems> {
    let values = resolve(def, given, options)?;
    let vars = vars_of(&values, options);
    let count = match repeat_of(def) {
        None => 1,
        Some(r) => {
            let n = Expr::parse(&r.count)
                .and_then(|e| e.eval(&vars))
                .and_then(|v| v.as_num())
                .map_err(|e| vec![Problem::Repeat(e.to_string())])?;
            if n.fract() != 0.0 || n < 1.0 {
                return Err(vec![Problem::Repeat(format!(
                    "{n} is not a whole number of at least 1"
                ))]);
            }
            if n as usize > MAX_REPEAT {
                return Err(vec![Problem::Repeat(format!(
                    "{n} copies is more than the limit of {MAX_REPEAT}"
                ))]);
            }
            n as usize
        }
    };
    let pitch = repeat_of(def).map_or(Point::new(0.0, 0.0), |r| Point::new(r.pitch[0], r.pitch[1]));
    let excluded = excluded_members(def, &vars)?;
    let left_out = excluded.len();
    let definition = {
        let mut d = def.clone();
        d.members.retain(|m| !excluded.contains(&m.id()));
        for m in &mut d.members {
            if let Some(props) = m.properties_mut() {
                props.remove(WHEN_KEY);
            }
        }
        if excluded.is_empty() {
            d
        } else {
            let mut d = d;
            d.properties
                .insert(BASE_KEY.into(), Value::Text(base_name(def).to_owned()));
            d.name = format!("{}~{:08x}", base_name(def), shape_key(&excluded));
            d
        }
    };
    Ok(Instantiation {
        definition,
        values,
        count,
        pitch,
        left_out,
    })
}

fn shape_key(excluded: &BTreeSet<AnyId>) -> u32 {
    // FNV-1a over the sorted ids: the same shape always names the same definition.
    let mut h: u32 = 0x811c_9dc5;
    for id in excluded {
        for b in format!("{id:?}").bytes() {
            h ^= u32::from(b);
            h = h.wrapping_mul(0x0100_0193);
        }
    }
    h
}

/// The template members a placement leaves out: those whose condition is
/// false, and what would dangle without them.
fn excluded_members(def: &BlockDefinition, vars: &Vars) -> Result<BTreeSet<AnyId>, Problems> {
    let mut out: BTreeSet<AnyId> = BTreeSet::new();
    let mut problems = Problems::new();
    for record in &def.members {
        let Some(Value::Text(cond)) = record.properties().get(WHEN_KEY) else {
            continue;
        };
        match Expr::parse(cond)
            .and_then(|e| e.eval(vars))
            .and_then(|v| v.as_bool())
        {
            Ok(true) => {}
            Ok(false) => {
                out.insert(record.id());
            }
            Err(error) => problems.push(Problem::Expression {
                name: format!("{WHEN_KEY} of {}", record.id()),
                error,
            }),
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    if out.is_empty() {
        return Ok(out);
    }
    // A device goes when its symbols go; its symbols go with it.
    loop {
        let before = out.len();
        for r in &def.members {
            match r {
                Record::Symbol(s) => {
                    if let SymbolRole::Device { device, .. } = &s.role {
                        if out.contains(&AnyId::from(*device)) {
                            out.insert(s.id.into());
                        }
                    }
                }
                Record::Wire(w) => {
                    if [w.from, w.to].iter().any(|e| gone(e, &out)) {
                        out.insert(w.id.into());
                    }
                }
                Record::Text(t) => {
                    if t.attached_to.is_some_and(|a| out.contains(&a)) {
                        out.insert(t.id.into());
                    }
                }
                _ => {}
            }
        }
        // A device whose every symbol is gone is gone too.
        for r in &def.members {
            if let Record::Device(d) = r {
                let symbols: Vec<AnyId> = def
                    .members
                    .iter()
                    .filter_map(|m| match m {
                        Record::Symbol(s)
                            if matches!(&s.role, SymbolRole::Device { device, .. } if *device == d.id) =>
                        {
                            Some(s.id.into())
                        }
                        _ => None,
                    })
                    .collect();
                if !symbols.is_empty() && symbols.iter().all(|s| out.contains(s)) {
                    out.insert(d.id.into());
                }
            }
        }
        // A junction left with no wire is gone.
        for r in &def.members {
            if let Record::Junction(j) = r {
                let mut touching = def.members.iter().filter_map(|m| match m {
                    Record::Wire(w) => Some(w),
                    _ => None,
                });
                let on_junction = |e: &Endpoint| matches!(e, Endpoint::Junction { junction } if *junction == j.id);
                let any = touching
                    .clone()
                    .any(|w| on_junction(&w.from) || on_junction(&w.to));
                let alive = touching.any(|w| {
                    (on_junction(&w.from) || on_junction(&w.to)) && !out.contains(&w.id.into())
                });
                if any && !alive {
                    out.insert(j.id.into());
                }
            }
        }
        if out.len() == before {
            break;
        }
    }
    Ok(out)
}

fn gone(end: &Endpoint, out: &BTreeSet<AnyId>) -> bool {
    match end {
        Endpoint::Pin { symbol, .. } => out.contains(&AnyId::from(*symbol)),
        Endpoint::Junction { junction } => out.contains(&AnyId::from(*junction)),
        Endpoint::Free { .. } => false,
    }
}

/// How to place a parametric macro.
#[derive(Debug, Clone, Default)]
pub struct ParametricOptions {
    pub values: Vec<(String, Value)>,
    pub options: Vars,
    pub wire_numbers: WireNumbers,
}

/// Places every copy; use inside one `Document::transact` for one undo step.
/// Each copy is a block instance with its own `index` (and `count`) when the
/// definition declares parameters of those names.
pub fn place_parametric(
    tx: &mut Tx<'_>,
    def: &BlockDefinition,
    page: PageId,
    origin: Point,
    opts: &ParametricOptions,
) -> ModelResult<Result<Vec<Placed>, Problems>> {
    let mut placed = Vec::new();
    let mut count = 1usize;
    let mut index = 1usize;
    while index <= count {
        let mut given = opts.values.clone();
        // `index` and `count` are available as parameters when declared.
        let all = params(def);
        let mut options = opts.options.clone();
        options.insert("index".into(), Val::Num(index as f64));
        let declared = |n: &str| all.iter().any(|p| p.name == n);
        if declared("index") && !given.iter().any(|(n, _)| n == "index") {
            given.push(("index".into(), Value::Int(index as i64)));
        }
        let inst = match instantiate(def, &given, &options) {
            Ok(i) => i,
            Err(p) => return Ok(Err(p)),
        };
        count = inst.count;
        let mut values = inst.values.clone();
        if declared("count") {
            if let Some(v) = values.iter_mut().find(|(n, _)| n == "count") {
                v.1 = Value::Int(count as i64);
            }
        }
        let at = Point::new(
            origin.x + inst.pitch.x * (index as f64 - 1.0),
            origin.y + inst.pitch.y * (index as f64 - 1.0),
        );
        let result = place_macro(
            tx,
            inst.definition,
            page,
            at,
            &PlaceOptions {
                values,
                wire_numbers: opts.wire_numbers,
            },
        )?;
        placed.push(result);
        index += 1;
    }
    Ok(Ok(placed))
}

/// What a parametric placement would take, without changing the project.
pub fn preview_parametric(
    project: &novo_core::Project,
    def: &BlockDefinition,
    page: PageId,
    origin: Point,
    opts: &ParametricOptions,
) -> ModelResult<Result<Vec<crate::macro_place::Report>, Problems>> {
    let mut copy = project.clone();
    let (result, _) = copy.transact("Preview parametric macro", |tx| {
        place_parametric(tx, def, page, origin, opts)
    })?;
    Ok(result.map(|v| v.into_iter().map(|p| p.report).collect()))
}

/// The parameter values a placed instance holds, for the inspector and the
/// spec-sheet export. Names in the definition's order.
pub fn values_of(
    project: &novo_core::Project,
    instance: novo_core::BlockInstanceId,
) -> Vec<(String, Value)> {
    project.block_values(instance)
}

/// A map of the asked (non-calculated) values by name.
pub fn asked_values(def: &BlockDefinition, values: &[(String, Value)]) -> BTreeMap<String, Value> {
    let derived: BTreeSet<String> = params(def)
        .into_iter()
        .filter(Param::is_derived)
        .map(|p| p.name)
        .collect();
    values
        .iter()
        .filter(|(n, _)| !derived.contains(n))
        .cloned()
        .collect()
}
