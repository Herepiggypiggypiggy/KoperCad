//! Device descriptions from part data, by an editable recipe per device kind.
//!
//! A shop writes the same kind of device the same way every time: pole count
//! and kind, rating, contact configuration, coil voltage. A [`Recipe`] says
//! how, as a list of segments with `{field}` placeholders:
//!
//! ```text
//! [{poles}-pole ]{kind}      3-pole contactor
//! {current}                  9 A
//! {contacts}                 3M/1m/1v
//! [Coil: {coil}]             Coil: 48 V, 50 Hz
//! ```
//!
//! Square brackets mark a group that is dropped when a field inside it is
//! unknown, and a segment with an unknown field outside brackets is dropped
//! whole. Unknown stays unknown: nothing is guessed, and [`Description`]
//! lists the fields the part data could not give, so the caller can say what
//! the maker's file lacks.
//!
//! The recipes are plain data in the user's profile ([`recipes_path`]); the
//! built-in ones ([`RecipeSet::english`], [`RecipeSet::dutch`]) are generic.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use novo_core::{Command, Device, DeviceId, Part, Project, Properties, Record, Value};
use serde::{Deserialize, Serialize};

use crate::catalog::CatalogPart;
use crate::fit::{Role, parse_supply, part_role};

/// Stable `data` keys the generator reads, besides the ones in
/// [`crate::eng`] (`poles`, `rated_current_a`, `ac3_current_a`,
/// `rated_power_kw`, `coil_voltage_v`).
pub mod keys {
    /// Make (NO) contacts in the main path.
    pub const MAIN_NO: &str = "main_contacts_no";
    /// Auxiliary make contacts.
    pub const AUX_NO: &str = "aux_contacts_no";
    /// Auxiliary break contacts.
    pub const AUX_NC: &str = "aux_contacts_nc";
    /// Coil frequency in Hz (`50`, or text `50/60`).
    pub const COIL_HZ: &str = "coil_frequency_hz";
    /// Text: `AC`, `DC`, `AC/DC`.
    pub const COIL_KIND: &str = "coil_current_type";
    /// Rated operating voltage in V.
    pub const VOLTAGE_V: &str = "rated_voltage_v";
}

/// The fields a template can name.
pub const FIELDS: &[(&str, &str)] = &[
    ("kind", "the kind's word from the recipe"),
    ("poles", "number of poles"),
    ("current", "rated current, e.g. 9 A"),
    ("voltage", "rated voltage, e.g. 400 V"),
    ("power", "rated power, e.g. 4 kW"),
    ("contacts", "contact configuration, e.g. 3M/1m/1v"),
    ("switching", "the part's own contact text, e.g. 1 NO + 1 NC"),
    ("coil", "coil voltage and frequency, e.g. 48 V, 50 Hz"),
    ("coil_v", "coil voltage alone"),
    ("coil_hz", "coil frequency alone"),
    ("range", "setting range of a relay, e.g. 1.6-2.5 A"),
    ("type", "the maker's type number"),
    ("maker", "the maker"),
];

/// How one kind of device is described.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    /// The word `{kind}` stands for: `contactor`.
    pub kind: String,
    /// Segments in order; see the module documentation.
    pub segments: Vec<String>,
    /// Between segments.
    #[serde(default = "default_separator")]
    pub separator: String,
    /// The letters in `{contacts}`: main, make, break.
    #[serde(default)]
    pub marks: ContactMarks,
    /// `9 A` (true) or `9A` (false).
    #[serde(default)]
    pub unit_space: bool,
}

fn default_separator() -> String {
    "; ".into()
}

/// The letters `{contacts}` writes after the counts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContactMarks {
    pub main: String,
    pub make: String,
    pub brk: String,
}

impl Default for ContactMarks {
    fn default() -> Self {
        ContactMarks {
            main: "M".into(),
            make: "m".into(),
            brk: "b".into(),
        }
    }
}

/// A template problem found by [`check_template`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateError {
    /// `{name}` is not a field.
    UnknownField(String),
    /// A `[` without `]`, a `]` without `[`, or a `{` without `}`.
    Unbalanced,
    /// Groups inside groups are not supported.
    NestedGroup,
}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateError::UnknownField(name) => write!(f, "unknown field {{{name}}}"),
            TemplateError::Unbalanced => f.write_str("brackets do not match"),
            TemplateError::NestedGroup => f.write_str("a [ ] group cannot hold another group"),
        }
    }
}

#[derive(Debug)]
enum Piece {
    Text(String),
    Field(String),
    Group(Vec<Piece>),
}

fn parse_template(text: &str) -> Result<Vec<Piece>, TemplateError> {
    let mut top: Vec<Piece> = Vec::new();
    let mut group: Option<Vec<Piece>> = None;
    let mut literal = String::new();
    let mut chars = text.chars();
    fn flush(literal: &mut String, into: &mut Vec<Piece>) {
        if !literal.is_empty() {
            into.push(Piece::Text(std::mem::take(literal)));
        }
    }
    while let Some(c) = chars.next() {
        match c {
            '[' => {
                if group.is_some() {
                    return Err(TemplateError::NestedGroup);
                }
                flush(&mut literal, &mut top);
                group = Some(Vec::new());
            }
            ']' => {
                let Some(mut inner) = group.take() else {
                    return Err(TemplateError::Unbalanced);
                };
                flush(&mut literal, &mut inner);
                top.push(Piece::Group(inner));
            }
            '{' => {
                let mut name = String::new();
                loop {
                    match chars.next() {
                        Some('}') => break,
                        Some(ch) => name.push(ch),
                        None => return Err(TemplateError::Unbalanced),
                    }
                }
                if !FIELDS.iter().any(|(f, _)| *f == name) {
                    return Err(TemplateError::UnknownField(name));
                }
                let target = group.as_mut().unwrap_or(&mut top);
                flush(&mut literal, target);
                target.push(Piece::Field(name));
            }
            c => literal.push(c),
        }
    }
    if group.is_some() {
        return Err(TemplateError::Unbalanced);
    }
    flush(&mut literal, &mut top);
    Ok(top)
}

/// Whether a template can be used, for the recipe editor.
pub fn check_template(text: &str) -> Result<(), TemplateError> {
    parse_template(text).map(|_| ())
}

/// What part data says, in the form the templates read.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facts {
    pub maker: String,
    pub type_number: String,
    pub poles: Option<u32>,
    pub current_a: Option<f64>,
    pub voltage_v: Option<f64>,
    pub power_kw: Option<f64>,
    pub main_no: Option<u32>,
    pub aux_no: Option<u32>,
    pub aux_nc: Option<u32>,
    pub coil_v: Option<f64>,
    /// Text because makers write `50/60`.
    pub coil_hz: Option<String>,
    /// `AC`, `DC`, `AC/DC`.
    pub coil_type: Option<String>,
    pub setting_a: Option<(f64, f64)>,
    /// The part's own contact text, `1 NO + 1 NC` (push buttons, auxiliary blocks).
    pub switching: Option<String>,
}

/// A number a description can state: finite, positive and not absurd.
/// Anything else (`NaN`, `-400`, `1e300`) counts as unknown.
fn number(props: &Properties, key: &str) -> Option<f64> {
    let v = match props.get(key)? {
        Value::Float(v) => *v,
        Value::Int(v) => *v as f64,
        Value::Text(t) => t.trim().parse().ok()?,
        _ => return None,
    };
    (v.is_finite() && v > 0.0 && v < 1e9).then_some(v)
}

fn count(props: &Properties, key: &str) -> Option<u32> {
    number(props, key)
        .filter(|v| *v >= 0.0 && v.fract() == 0.0)
        .map(|v| v as u32)
}

fn text(props: &Properties, key: &str) -> Option<String> {
    match props.get(key)? {
        Value::Text(t) if !t.trim().is_empty() => Some(t.trim().to_owned()),
        Value::Int(v) => Some(v.to_string()),
        Value::Float(v) => Some(trim_number(*v)),
        _ => None,
    }
}

impl Facts {
    /// Facts from technical data and the identity of a part.
    pub fn from_data(maker: &str, type_number: &str, data: &Properties) -> Self {
        use crate::schema::eng;
        let min = number(data, eng::SETTING_MIN_A);
        let max = number(data, eng::SETTING_MAX_A);
        // Maker tables often write the coil as one text, `24 V AC/DC`, and
        // the auxiliary contacts as `1 NO + 1 NC`: read both spellings.
        let coil_text = text(data, "coil_voltage");
        let supply = coil_text.as_deref().and_then(parse_supply);
        let (aux_no_t, aux_nc_t) = text(data, "auxiliary_contacts")
            .map(|t| contact_counts(&t))
            .unzip();
        Facts {
            maker: maker.to_owned(),
            type_number: type_number.to_owned(),
            poles: count(data, eng::POLES),
            current_a: number(data, eng::AC3_CURRENT_A)
                .or_else(|| number(data, eng::RATED_CURRENT_A)),
            voltage_v: number(data, keys::VOLTAGE_V)
                .or_else(|| text(data, "rated_voltage").and_then(|t| first_number(&t))),
            power_kw: number(data, eng::RATED_POWER_KW),
            main_no: count(data, keys::MAIN_NO),
            aux_no: count(data, keys::AUX_NO).or(aux_no_t),
            aux_nc: count(data, keys::AUX_NC).or(aux_nc_t),
            coil_v: number(data, eng::COIL_VOLTAGE_V).or(supply.map(|s| s.volts)),
            coil_hz: text(data, keys::COIL_HZ).or_else(|| coil_text.as_deref().and_then(hertz_in)),
            coil_type: text(data, keys::COIL_KIND).or_else(|| {
                supply.map(|s| {
                    match (s.ac, s.dc) {
                        (true, true) => "AC/DC",
                        (true, false) => "AC",
                        _ => "DC",
                    }
                    .to_owned()
                })
            }),
            setting_a: min.zip(max),
            switching: text(data, "contacts").or_else(|| text(data, "auxiliary_contacts")),
        }
    }

    pub fn of_catalog(part: &CatalogPart) -> Self {
        Self::from_data(&part.manufacturer, &part.type_number, &part.data)
    }

    pub fn of_project(part: &Part) -> Self {
        Self::from_data(&part.manufacturer, &part.type_number, &part.properties)
    }
}

/// The first number in a text: `400 V` gives 400.
fn first_number(text: &str) -> Option<f64> {
    let digits: String = text
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    digits.parse().ok()
}

/// The number written before `Hz`: `230 V AC 50/60 Hz` gives `50/60`.
fn hertz_in(text: &str) -> Option<String> {
    let lower = text.to_lowercase();
    let before = lower.split("hz").next().filter(|_| lower.contains("hz"))?;
    let token: String = before
        .trim_end()
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit() || *c == '/' || *c == '.')
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    (!token.is_empty()).then_some(token)
}

/// Make and break counts of `1 NO + 1 NC`. A kind that is not named is zero:
/// the text states the whole configuration.
fn contact_counts(text: &str) -> (u32, u32) {
    let mut no = 0;
    let mut nc = 0;
    for part in text.split(['+', ',', '/']) {
        let upper = part.to_uppercase();
        let n = first_number(&upper).map_or(1, |v| v as u32);
        if upper.contains("NO") {
            no += n;
        } else if upper.contains("NC") {
            nc += n;
        }
    }
    (no, nc)
}

/// `9`, `1.6`, `0.25`: no trailing zeros.
fn trim_number(v: f64) -> String {
    let s = format!("{v:.3}");
    s.trim_end_matches('0').trim_end_matches('.').to_owned()
}

impl Recipe {
    fn unit(&self, value: f64, unit: &str) -> String {
        let sep = if self.unit_space { " " } else { "" };
        format!("{}{sep}{unit}", trim_number(value))
    }

    fn field(&self, name: &str, facts: &Facts) -> Option<String> {
        match name {
            "kind" => Some(self.kind.clone()).filter(|k| !k.is_empty()),
            "poles" => facts.poles.map(|p| p.to_string()),
            "current" => facts.current_a.map(|v| self.unit(v, "A")),
            "voltage" => facts.voltage_v.map(|v| self.unit(v, "V")),
            "power" => facts.power_kw.map(|v| self.unit(v, "kW")),
            "contacts" => {
                // Only a 3-pole device is known to have three make contacts
                // in its main path; a 4-pole one is often 2 make + 2 break,
                // so it needs `main_contacts_no`.
                let main = facts.main_no.or(facts.poles.filter(|&p| p == 3))?;
                let m = &self.marks;
                let mut out = format!("{main}{}", m.main);
                for (n, mark) in [(facts.aux_no, &m.make), (facts.aux_nc, &m.brk)] {
                    // Not stated is not zero: leave the whole configuration
                    // out rather than write a false one.
                    out.push_str(&format!("/{}{mark}", n?));
                }
                Some(out)
            }
            "coil_v" => facts.coil_v.map(|v| self.unit(v, "V")),
            "coil_hz" => facts.coil_hz.as_ref().map(|h| {
                let sep = if self.unit_space { " " } else { "" };
                format!("{h}{sep}Hz")
            }),
            "coil" => {
                let volts = self.field("coil_v", facts)?;
                let kind = facts.coil_type.as_deref().filter(|k| !k.is_empty());
                let hz = self.field("coil_hz", facts);
                let mut out = volts;
                if let Some(kind) = kind {
                    out.push(' ');
                    out.push_str(kind);
                }
                if let Some(hz) = hz {
                    out.push_str(", ");
                    out.push_str(&hz);
                }
                Some(out)
            }
            "range" => facts.setting_a.map(|(lo, hi)| {
                let sep = if self.unit_space { " " } else { "" };
                format!("{}-{}{sep}A", trim_number(lo), trim_number(hi))
            }),
            "switching" => facts.switching.clone(),
            "type" => Some(facts.type_number.clone()).filter(|t| !t.is_empty()),
            "maker" => Some(facts.maker.clone()).filter(|t| !t.is_empty()),
            _ => None,
        }
    }

    fn render(&self, pieces: &[Piece], facts: &Facts, missing: &mut Vec<String>) -> Option<String> {
        let mut out = String::new();
        let mut ok = true;
        for piece in pieces {
            match piece {
                Piece::Text(t) => out.push_str(t),
                Piece::Field(name) => match self.field(name, facts) {
                    Some(v) => out.push_str(&v),
                    None => {
                        if !missing.contains(name) {
                            missing.push(name.clone());
                        }
                        ok = false;
                    }
                },
                Piece::Group(inner) => {
                    // A group with an unknown field vanishes; its fields are
                    // still reported so the caller sees what was lacking.
                    if let Some(s) = self.render(inner, facts, missing) {
                        out.push_str(&s);
                    }
                }
            }
        }
        ok.then_some(out)
    }

    /// The description for `facts`. Segments that cannot be written because
    /// a field is unknown are left out and their fields listed.
    pub fn describe(&self, facts: &Facts) -> Description {
        let mut missing = Vec::new();
        let mut parts = Vec::new();
        for segment in &self.segments {
            let Ok(pieces) = parse_template(segment) else {
                continue;
            };
            if let Some(s) = self.render(&pieces, facts, &mut missing) {
                let s = s.trim();
                if !s.is_empty() {
                    parts.push(s.to_owned());
                }
            }
        }
        Description {
            text: parts.join(&self.separator),
            missing,
        }
    }

    /// Problems in this recipe's templates, segment number first.
    pub fn problems(&self) -> Vec<(usize, TemplateError)> {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(i, s)| check_template(s).err().map(|e| (i, e)))
            .collect()
    }
}

/// The generated text and what the part data could not give.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Description {
    pub text: String,
    /// Field names the recipe wanted and the part does not state.
    pub missing: Vec<String>,
}

/// The kinds that have a recipe, by stable id.
pub fn recipe_id(role: Role) -> &'static str {
    match role {
        Role::Contactor => "contactor",
        Role::Relay => "relay",
        Role::Mcb => "mcb",
        Role::Mpcb => "mpcb",
        Role::Overload => "overload",
        Role::FuseHolder => "fuse_holder",
        Role::PushNo | Role::PushNc | Role::PushBoth => "push_button",
        Role::Motor => "motor",
        Role::Drive => "drive",
        Role::SoftStarter => "soft_starter",
        Role::PowerSupply => "power_supply",
        Role::PlcCpu => "plc_cpu",
        Role::PlcIo => "plc_io",
        Role::NetSwitch => "net_switch",
        Role::NetPort => "net_port",
        Role::Fan => "fan",
        Role::Vent => "vent",
        Role::Enclosure => "enclosure",
        Role::Disconnector => "disconnector",
        Role::Lamp => "lamp",
    }
}

/// Every recipe id with the role that owns it, for the editor's list.
pub const ALL_ROLES: [Role; 20] = [
    Role::Contactor,
    Role::Relay,
    Role::Mcb,
    Role::Mpcb,
    Role::Overload,
    Role::FuseHolder,
    Role::PushNo,
    Role::Motor,
    Role::Drive,
    Role::SoftStarter,
    Role::PowerSupply,
    Role::PlcCpu,
    Role::PlcIo,
    Role::NetSwitch,
    Role::NetPort,
    Role::Fan,
    Role::Vent,
    Role::Enclosure,
    Role::Disconnector,
    Role::Lamp,
];

/// Recipes by kind id. A kind with no entry is not described: the part's
/// own description stays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeSet {
    pub format: u32,
    pub recipes: BTreeMap<String, Recipe>,
}

/// File format version of the recipes file.
pub const RECIPES_FORMAT: u32 = 1;

impl Default for RecipeSet {
    fn default() -> Self {
        Self::english()
    }
}

fn recipe(kind: &str, segments: &[&str]) -> Recipe {
    Recipe {
        kind: kind.into(),
        segments: segments.iter().map(|s| (*s).to_owned()).collect(),
        separator: default_separator(),
        marks: ContactMarks::default(),
        unit_space: true,
    }
}

impl RecipeSet {
    /// Generic English recipes.
    pub fn english() -> Self {
        let mut r = BTreeMap::new();
        let mut put = |id: &str, kind: &str, segs: &[&str]| {
            r.insert(id.to_owned(), recipe(kind, segs));
        };
        put(
            "contactor",
            "contactor",
            &[
                "[{poles}-pole ]{kind}",
                "{current}",
                "{contacts}",
                "{voltage}",
                "{power}",
                "Coil: {coil}",
            ],
        );
        put(
            "relay",
            "relay",
            &["[{poles}-pole ]{kind}", "{current}", "Coil: {coil}"],
        );
        put(
            "mcb",
            "circuit breaker",
            &["[{poles}-pole ]{kind}", "{current}", "{voltage}"],
        );
        put(
            "mpcb",
            "motor protection breaker",
            &["[{poles}-pole ]{kind}", "{range}", "{voltage}"],
        );
        put("overload", "overload relay", &["{kind}", "{range}"]);
        put(
            "fuse_holder",
            "fuse holder",
            &["[{poles}-pole ]{kind}", "{current}", "{voltage}"],
        );
        put("push_button", "push button", &["{kind}", "{switching}"]);
        put("motor", "motor", &["{kind}", "{power}", "{voltage}"]);
        put(
            "drive",
            "frequency converter",
            &["{kind}", "{power}", "{current}", "{voltage}"],
        );
        put(
            "soft_starter",
            "soft starter",
            &["{kind}", "{power}", "{current}", "{voltage}"],
        );
        put(
            "power_supply",
            "power supply",
            &["{kind}", "{voltage}", "{current}"],
        );
        put(
            "disconnector",
            "main switch",
            &["[{poles}-pole ]{kind}", "{current}", "{voltage}"],
        );
        put("lamp", "pilot lamp", &["{kind}", "{voltage}"]);
        put("fan", "fan", &["{kind}", "{voltage}"]);
        RecipeSet {
            format: RECIPES_FORMAT,
            recipes: r,
        }
    }

    /// The same recipes in Dutch, with the contact marks `M/m/v`
    /// (hoofd/maak/verbreek): `3-polige magneetschakelaar; 9A; 3M/1m/1v`.
    pub fn dutch() -> Self {
        let mut set = Self::english();
        let words = [
            ("contactor", "magneetschakelaar"),
            ("relay", "relais"),
            ("mcb", "installatieautomaat"),
            ("mpcb", "motorbeveiligingsschakelaar"),
            ("overload", "thermisch relais"),
            ("fuse_holder", "zekeringhouder"),
            ("push_button", "drukknop"),
            ("motor", "motor"),
            ("drive", "frequentieregelaar"),
            ("soft_starter", "softstarter"),
            ("power_supply", "voeding"),
            ("disconnector", "hoofdschakelaar"),
            ("lamp", "signaallamp"),
            ("fan", "ventilator"),
        ];
        for (id, word) in words {
            if let Some(r) = set.recipes.get_mut(id) {
                r.kind = word.into();
                r.unit_space = false;
                r.marks = ContactMarks {
                    main: "M".into(),
                    make: "m".into(),
                    brk: "v".into(),
                };
                for s in &mut r.segments {
                    *s = s
                        .replace("-pole ", "-polige ")
                        .replace("Coil:", "Spoelspanning:");
                }
            }
        }
        set
    }

    pub fn get(&self, role: Role) -> Option<&Recipe> {
        self.recipes.get(recipe_id(role))
    }

    /// The description for a catalog part, or `None` when its kind is
    /// unknown or has no recipe, or the recipe writes nothing.
    pub fn describe_catalog(&self, part: &CatalogPart) -> Option<Description> {
        let role = part_role(part)?;
        self.describe_with(role, &Facts::of_catalog(part))
    }

    /// The description for a project part (its kind from the text it holds).
    pub fn describe_project_part(&self, part: &Part) -> Option<Description> {
        let as_catalog = CatalogPart::from_model(part);
        let role = part_role(&as_catalog)?;
        self.describe_with(role, &Facts::of_project(part))
    }

    pub fn describe_with(&self, role: Role, facts: &Facts) -> Option<Description> {
        let d = self.get(role)?.describe(facts);
        (!d.text.is_empty()).then_some(d)
    }

    /// Reads the user's recipes, falling back to the built-in English ones.
    /// Kinds missing from the file keep their built-in recipe.
    pub fn load(path: &Path) -> Self {
        Self::load_checked(path).0
    }

    /// Like [`RecipeSet::load`], and says why the file was not used when it
    /// exists but cannot be read (saving then overwrites it).
    pub fn load_checked(path: &Path) -> (Self, Option<String>) {
        let mut set = Self::english();
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (set, None),
            Err(e) => return (set, Some(format!("could not read {}: {e}", path.display()))),
        };
        match serde_json::from_slice::<RecipeSet>(&bytes) {
            Ok(user) => {
                set.recipes.extend(user.recipes);
                (set, None)
            }
            Err(e) => (
                set,
                Some(format!(
                    "{} is not a valid recipe file ({e})",
                    path.display()
                )),
            ),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, json)?;
        std::fs::rename(&tmp, path)
    }
}

/// Where the user's recipes live, beside the user catalog.
pub fn recipes_path() -> Option<PathBuf> {
    crate::catalog::user_catalog_path().map(|p| p.with_file_name("description-recipes.json"))
}

/// Command that writes generated descriptions into the project parts whose
/// description is empty (or all of them with `overwrite`). One undo step.
pub fn describe_parts(project: &Project, set: &RecipeSet, overwrite: bool) -> Option<Command> {
    let mut commands = Vec::new();
    for part in project.parts() {
        if !overwrite && !part.description.trim().is_empty() {
            continue;
        }
        let Some(d) = set.describe_project_part(part) else {
            continue;
        };
        if d.text != part.description {
            let mut next = part.clone();
            next.description = d.text;
            commands.push(Command::Replace {
                record: Record::Part(next),
            });
        }
    }
    batch("Describe parts from recipes", commands)
}

/// Command that writes generated descriptions into `devices` whose
/// description is empty (or into all with `overwrite`), from the device's
/// first part with a recipe. One undo step.
pub fn describe_devices(
    project: &Project,
    devices: &[DeviceId],
    set: &RecipeSet,
    overwrite: bool,
) -> Option<Command> {
    let mut commands = Vec::new();
    for &id in devices {
        let Some(device) = project.device(id) else {
            continue;
        };
        if !overwrite && !device.description.trim().is_empty() {
            continue;
        }
        let Some(text) = device_description(project, device, set) else {
            continue;
        };
        if text != device.description {
            let mut next = device.clone();
            next.description = text;
            commands.push(Command::Replace {
                record: Record::Device(next),
            });
        }
    }
    batch("Describe devices from parts", commands)
}

/// What the recipe makes of a device's first described part.
pub fn device_description(project: &Project, device: &Device, set: &RecipeSet) -> Option<String> {
    device
        .parts
        .iter()
        .filter_map(|&p| project.part(p))
        .find_map(|p| set.describe_project_part(p))
        .map(|d| d.text)
}

fn batch(label: &str, commands: Vec<Command>) -> Option<Command> {
    match commands.len() {
        0 => None,
        _ => Some(Command::Batch {
            label: format!("{label} ({})", commands.len()),
            commands,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contactor() -> CatalogPart {
        let mut p = CatalogPart::new("Acme", "C-09");
        p.function = "contactor".into();
        p.type_number = "AC-09".into();
        p.data.insert("poles".into(), Value::Int(3));
        p.data.insert("ac3_current_a".into(), Value::Float(9.0));
        p.data.insert("rated_power_kw".into(), Value::Float(4.0));
        p.data.insert("rated_voltage_v".into(), Value::Int(400));
        p.data.insert("aux_contacts_no".into(), Value::Int(1));
        p.data.insert("aux_contacts_nc".into(), Value::Int(1));
        p.data.insert("coil_voltage_v".into(), Value::Int(48));
        p.data.insert("coil_frequency_hz".into(), Value::Int(50));
        p.data
            .insert("coil_current_type".into(), Value::Text("AC".into()));
        p
    }

    #[test]
    fn english_contactor_follows_the_recipe() {
        let d = RecipeSet::english().describe_catalog(&contactor()).unwrap();
        assert_eq!(
            d.text,
            "3-pole contactor; 9 A; 3M/1m/1b; 400 V; 4 kW; Coil: 48 V AC, 50 Hz"
        );
        assert!(d.missing.is_empty());
    }

    #[test]
    fn dutch_preset_writes_the_shop_shape() {
        let d = RecipeSet::dutch().describe_catalog(&contactor()).unwrap();
        assert_eq!(
            d.text,
            "3-polige magneetschakelaar; 9A; 3M/1m/1v; 400V; 4kW; Spoelspanning: 48V AC, 50Hz"
        );
    }

    #[test]
    fn unknown_fields_are_left_out_and_reported() {
        let mut p = contactor();
        p.data.remove("poles");
        p.data.remove("coil_voltage_v");
        p.data.remove("aux_contacts_nc");
        let d = RecipeSet::english().describe_catalog(&p).unwrap();
        // No poles: the group drops, the kind stays. No coil: segment drops.
        // Aux break contacts unstated: no contact configuration at all.
        assert_eq!(d.text, "contactor; 9 A; 400 V; 4 kW");
        assert!(d.missing.contains(&"poles".to_string()));
        assert!(d.missing.contains(&"contacts".to_string()));
        assert!(d.missing.contains(&"coil".to_string()));
    }

    #[test]
    fn a_part_of_unknown_kind_is_not_described() {
        let p = CatalogPart::new("Acme", "X-1");
        assert!(RecipeSet::english().describe_catalog(&p).is_none());
    }

    #[test]
    fn templates_are_checked() {
        assert!(check_template("[{poles}-pole ]{kind}").is_ok());
        assert_eq!(
            check_template("{nope}"),
            Err(TemplateError::UnknownField("nope".into()))
        );
        assert_eq!(check_template("[{kind}"), Err(TemplateError::Unbalanced));
        assert_eq!(check_template("{kind}]"), Err(TemplateError::Unbalanced));
        assert_eq!(
            check_template("[[{kind}]]"),
            Err(TemplateError::NestedGroup)
        );
    }

    #[test]
    fn user_recipe_overrides_one_kind_and_survives_a_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.json");
        let mut set = RecipeSet::english();
        set.recipes.get_mut("contactor").unwrap().segments = vec!["{kind} {type}".into()];
        set.save(&path).unwrap();
        let back = RecipeSet::load(&path);
        let d = back.describe_catalog(&contactor()).unwrap();
        assert_eq!(d.text, "contactor AC-09");
        // Other kinds keep the built-ins.
        assert!(back.get(Role::Motor).is_some());
        // A missing or broken file gives the built-ins.
        assert_eq!(
            RecipeSet::load(&dir.path().join("none.json")),
            RecipeSet::english()
        );
    }

    #[test]
    fn maker_table_spellings_are_read() {
        // The starter catalog writes the coil and the contacts as text.
        let mut p = CatalogPart::new("Acme", "C-2");
        p.function = "contactor".into();
        p.data.insert("poles".into(), Value::Int(3));
        p.data.insert("rated_current_a".into(), Value::Float(9.0));
        p.data
            .insert("rated_voltage".into(), Value::Text("400 V".into()));
        p.data.insert(
            "coil_voltage".into(),
            Value::Text("230 V AC 50/60 Hz".into()),
        );
        p.data.insert(
            "auxiliary_contacts".into(),
            Value::Text("1 NO + 1 NC".into()),
        );
        let d = RecipeSet::english().describe_catalog(&p).unwrap();
        assert_eq!(
            d.text,
            "3-pole contactor; 9 A; 3M/1m/1b; 400 V; Coil: 230 V AC, 50/60 Hz"
        );
        let mut q = p.clone();
        q.data
            .insert("auxiliary_contacts".into(), Value::Text("1 NO".into()));
        q.data
            .insert("coil_voltage".into(), Value::Text("24 V AC/DC".into()));
        let d = RecipeSet::english().describe_catalog(&q).unwrap();
        assert!(d.text.contains("3M/1m/0b") && d.text.ends_with("Coil: 24 V AC/DC"));
    }

    #[test]
    fn four_poles_do_not_imply_four_make_contacts() {
        let mut p = contactor();
        p.data.insert("poles".into(), Value::Int(4));
        let d = RecipeSet::english().describe_catalog(&p).unwrap();
        assert!(!d.text.contains("M/"));
        assert!(d.missing.contains(&"contacts".to_string()));
        p.data.insert("main_contacts_no".into(), Value::Int(2));
        let d = RecipeSet::english().describe_catalog(&p).unwrap();
        assert!(d.text.contains("2M/1m/1b"));
    }

    #[test]
    fn absurd_numbers_are_unknown() {
        let mut p = contactor();
        p.data
            .insert("ac3_current_a".into(), Value::Text("NaN".into()));
        p.data.insert("rated_voltage_v".into(), Value::Int(-400));
        p.data.insert("rated_power_kw".into(), Value::Float(1e300));
        let d = RecipeSet::english().describe_catalog(&p).unwrap();
        assert!(!d.text.contains("NaN") && !d.text.contains("-400") && !d.text.contains("e300"));
        assert!(d.missing.contains(&"voltage".to_string()));
    }

    #[test]
    fn a_broken_recipe_file_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.json");
        std::fs::write(&path, "{ not json").unwrap();
        let (set, warning) = RecipeSet::load_checked(&path);
        assert_eq!(set, RecipeSet::english());
        assert!(warning.unwrap().contains("not a valid recipe file"));
        assert!(
            RecipeSet::load_checked(&dir.path().join("none.json"))
                .1
                .is_none()
        );
    }

    #[test]
    fn numbers_print_without_trailing_zeros() {
        assert_eq!(trim_number(9.0), "9");
        assert_eq!(trim_number(1.6), "1.6");
        assert_eq!(trim_number(0.25), "0.25");
    }
}
