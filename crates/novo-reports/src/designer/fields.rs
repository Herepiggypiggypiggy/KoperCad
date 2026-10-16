//! `{field}` placeholders in template text.

use std::collections::BTreeMap;

use novo_core::{Project, Properties, Value};

/// Values for the placeholders of a template, by name.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Fields(BTreeMap<String, String>);

fn prop(props: &Properties, key: &str) -> String {
    match props.get(key) {
        Some(Value::Text(t)) => t.clone(),
        Some(Value::Int(i)) => i.to_string(),
        _ => String::new(),
    }
}

impl Fields {
    pub fn new() -> Self {
        Fields::default()
    }

    /// The project's fields: `project.name`, `project.description`,
    /// `revision`, `revision_date`, `drawn_by`, `checked_by` (the project
    /// properties the canvas title block reads). The page fields
    /// (`page.name`, `page.title`, `page.structure`) are known and empty
    /// until [`Fields::with_page`] sets them.
    pub fn from_project(project: &Project) -> Self {
        let info = project.info();
        let mut f = Fields::new();
        f.set("project.name", &info.name);
        f.set("project.description", &info.description);
        for key in ["revision", "revision_date", "drawn_by", "checked_by"] {
            f.set(key, &prop(&info.properties, key));
        }
        f.with_page("", "", "")
    }

    /// Sets the page fields.
    pub fn with_page(mut self, name: &str, title: &str, structure: &str) -> Self {
        self.set("page.name", name);
        self.set("page.title", title);
        self.set("page.structure", structure);
        self
    }

    /// Example values for the designer's preview without a project.
    pub fn sample() -> Self {
        let mut f = Fields::new();
        f.set("project.name", "Sample project");
        f.set("project.description", "Packaging line 3");
        f.set("revision", "B");
        f.set("revision_date", "2026-10-16");
        f.set("drawn_by", "A. Drafter");
        f.set("checked_by", "C. Checker");
        f.with_page("12", "Motor starter", "=A1+CAB")
    }

    pub fn set(&mut self, name: &str, value: &str) {
        self.0.insert(name.to_owned(), value.to_owned());
    }

    pub fn get(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.0.keys().map(String::as_str)
    }
}

/// `text` with every `{name}` replaced. `{{` and `}}` print a brace. An
/// unknown name stays as typed (`{name}`) and is listed in the second
/// value, so a typo is visible on the page and in the problem list.
pub fn expand(text: &str, fields: &Fields) -> (String, Vec<String>) {
    let mut out = String::with_capacity(text.len());
    let mut unknown = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
        } else if tail.starts_with('{') {
            match tail.find('}') {
                Some(end) => {
                    let name = tail[1..end].trim();
                    match fields.get(name) {
                        Some(v) => out.push_str(v),
                        None => {
                            out.push_str(&tail[..=end]);
                            if !unknown.iter().any(|u| u == name) {
                                unknown.push(name.to_owned());
                            }
                        }
                    }
                    rest = &tail[end + 1..];
                }
                None => {
                    out.push_str(tail);
                    rest = "";
                }
            }
        } else {
            out.push('}');
            rest = &tail[1..];
        }
    }
    out.push_str(rest);
    (out, unknown)
}
