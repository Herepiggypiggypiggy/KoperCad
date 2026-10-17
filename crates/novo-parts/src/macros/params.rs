//! Authoring a macro's parameters: the editable form of a
//! [`BlockParameter`] (name, type, default as typed text) and the rules a
//! parameter list must meet before it is written back to a macro file.
//!
//! A parameter is named in the macro's texts and device properties as
//! `{name}`; placing the macro fills it in from the value the person sets,
//! or from the default. The type is the type of the default value.

use std::collections::BTreeSet;

use novo_core::{BlockParameter, Value};

use super::{MacroFile, MacroLibrary, Result};

/// The type of a parameter's value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamKind {
    Text,
    Number,
    Integer,
    Switch,
}

impl ParamKind {
    pub const ALL: [ParamKind; 4] = [
        ParamKind::Text,
        ParamKind::Number,
        ParamKind::Integer,
        ParamKind::Switch,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ParamKind::Text => "Text",
            ParamKind::Number => "Number",
            ParamKind::Integer => "Whole number",
            ParamKind::Switch => "Yes/no",
        }
    }

    pub fn of(value: &Value) -> Self {
        match value {
            Value::Text(_) => ParamKind::Text,
            Value::Float(_) => ParamKind::Number,
            Value::Int(_) => ParamKind::Integer,
            Value::Bool(_) => ParamKind::Switch,
        }
    }
}

/// A parameter as the editor holds it: the default is the text the person
/// typed, checked against the type when the list is saved.
#[derive(Debug, Clone, PartialEq)]
pub struct ParamDraft {
    pub name: String,
    pub kind: ParamKind,
    pub default: String,
    pub description: String,
}

impl ParamDraft {
    /// A new, empty text parameter.
    pub fn new() -> Self {
        ParamDraft {
            name: String::new(),
            kind: ParamKind::Text,
            default: String::new(),
            description: String::new(),
        }
    }

    pub fn from_parameter(p: &BlockParameter) -> Self {
        ParamDraft {
            name: p.name.clone(),
            kind: ParamKind::of(&p.default),
            default: match &p.default {
                Value::Text(t) => t.clone(),
                Value::Float(f) => f.to_string(),
                Value::Int(i) => i.to_string(),
                Value::Bool(b) => if *b { "yes" } else { "no" }.into(),
            },
            description: p.description.clone(),
        }
    }

    /// Changes the type, converting the typed default when it can be and
    /// resetting it when it cannot.
    pub fn set_kind(&mut self, kind: ParamKind) {
        if self.kind == kind {
            return;
        }
        let old = self.default.trim().to_owned();
        self.default = match kind {
            ParamKind::Text => old,
            ParamKind::Number => old
                .parse::<f64>()
                .map(|v| v.to_string())
                .unwrap_or_else(|_| "0".into()),
            ParamKind::Integer => old
                .parse::<f64>()
                .map(|v| (v.round() as i64).to_string())
                .unwrap_or_else(|_| "0".into()),
            ParamKind::Switch => match old.to_lowercase().as_str() {
                "yes" | "true" | "1" => "yes".into(),
                _ => "no".into(),
            },
        };
        self.kind = kind;
    }

    /// The parameter, or what is wrong with it.
    pub fn to_parameter(&self) -> std::result::Result<BlockParameter, String> {
        let name = self.name.trim();
        if name.is_empty() {
            return Err("A parameter needs a name.".into());
        }
        if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!(
                "'{name}': use letters, digits and underscores only, so {{{name}}} works in texts."
            ));
        }
        let text = self.default.trim();
        let default = match self.kind {
            ParamKind::Text => Value::Text(self.default.clone()),
            ParamKind::Number => Value::Float(
                text.replace(',', ".")
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| format!("'{name}': the default '{text}' is not a number."))?,
            ),
            ParamKind::Integer => {
                Value::Int(text.parse::<i64>().map_err(|_| {
                    format!("'{name}': the default '{text}' is not a whole number.")
                })?)
            }
            ParamKind::Switch => match text.to_lowercase().as_str() {
                "yes" | "true" | "1" => Value::Bool(true),
                "no" | "false" | "0" | "" => Value::Bool(false),
                _ => return Err(format!("'{name}': the default must be yes or no.")),
            },
        };
        Ok(BlockParameter {
            name: name.to_owned(),
            default,
            description: self.description.trim().to_owned(),
        })
    }
}

impl Default for ParamDraft {
    fn default() -> Self {
        Self::new()
    }
}

/// The drafts of a parameter list.
pub fn param_drafts(parameters: &[BlockParameter]) -> Vec<ParamDraft> {
    parameters.iter().map(ParamDraft::from_parameter).collect()
}

/// Checks a list of drafts: every one valid, names unique. Returns the
/// parameters in the order given.
pub fn validate_parameters(
    drafts: &[ParamDraft],
) -> std::result::Result<Vec<BlockParameter>, String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for d in drafts {
        let p = d.to_parameter()?;
        if !seen.insert(p.name.clone()) {
            return Err(format!("The name '{}' is used twice.", p.name));
        }
        out.push(p);
    }
    Ok(out)
}

impl MacroFile {
    /// Replaces the parameter list (the definition is the one the macro
    /// carries; its members keep their `{name}` references).
    pub fn set_parameters(&mut self, parameters: Vec<BlockParameter>) {
        self.definition.parameters = parameters;
    }

    /// `{name}` references in the members' texts and properties that no
    /// parameter in `parameters` answers: they would stay literal text when
    /// placed.
    pub fn unknown_references(&self, parameters: &[BlockParameter]) -> Vec<String> {
        let json = serde_json::to_string(&self.definition.members).unwrap_or_default();
        let mut found = BTreeSet::new();
        let mut rest = json.as_str();
        while let Some(i) = rest.find('{') {
            let after = &rest[i + 1..];
            if let Some(j) = after.find('}') {
                let name = &after[..j];
                if !name.is_empty()
                    && name.chars().all(|c| c.is_alphanumeric() || c == '_')
                    && !parameters.iter().any(|p| p.name == name)
                {
                    found.insert(name.to_owned());
                }
            }
            rest = after;
        }
        found.into_iter().collect()
    }
}

impl MacroLibrary {
    /// Writes a new parameter list into the macro at `path`, bumping its
    /// revision like any replace, so projects holding an older copy are
    /// offered the update.
    pub fn set_parameters(&self, path: &str, parameters: Vec<BlockParameter>) -> Result<()> {
        let mut file = self.load(path)?;
        file.set_parameters(parameters);
        self.replace(path, &file)
    }
}
