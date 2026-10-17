//! Place all per surface (PANEL-B2): which kinds of device belong on which
//! surface. Pilot lights, push buttons and meters go on the door, the rest
//! stays on the plate.
//!
//! The rules are text in the structure property [`SURFACE_RULES_KEY`], one
//! line per surface: `door: H S P`. A device goes to the surface whose code
//! is the longest prefix of the device's letter code, like the kind rules in
//! `rules.rs`. Blank lines and `#` comments are ignored. A line that is not
//! understood, or names a surface the enclosure does not have, is a sentence
//! and its devices stay on the plate; nothing is dropped silently.

use crate::plan::{Candidate, Plan, place_all};
use crate::rules::Spec;
use crate::surfaces::{MAIN_PLATE, Surfaces};

/// Structure property that carries the rules.
pub const SURFACE_RULES_KEY: &str = "surface_rules";
/// Rule name used when a device is sent to another surface.
pub const RULE_SURFACE: &str = "place.surface";

#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceRule {
    pub surface: String,
    pub codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SurfaceRules {
    pub rules: Vec<SurfaceRule>,
}

impl SurfaceRules {
    /// Reads the rule text; unreadable lines come back as sentences.
    pub fn parse(text: &str) -> (SurfaceRules, Vec<String>) {
        let (mut rules, mut problems) = (Vec::new(), Vec::new());
        for (n, raw) in text.lines().enumerate() {
            let line = raw.split('#').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }
            let parsed = line.split_once(':').and_then(|(surface, codes)| {
                let surface = surface.trim().to_ascii_lowercase();
                let codes: Vec<String> = codes
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .filter(|c| !c.is_empty())
                    .map(|c| c.to_ascii_uppercase())
                    .collect();
                (!surface.is_empty() && !codes.is_empty()).then_some(SurfaceRule { surface, codes })
            });
            match parsed {
                Some(rule) => rules.push(rule),
                None => problems.push(format!(
                    "Surface rules, line {}: should read 'door: H S P' (surface, colon, letter codes).",
                    n + 1
                )),
            }
        }
        (SurfaceRules { rules }, problems)
    }

    /// The surface for a letter code: the longest matching code wins, the
    /// first rule on a tie. `None` means the plate.
    pub fn surface_for(&self, kind: &str) -> Option<&str> {
        let kind = kind.to_ascii_uppercase();
        let mut best: Option<(&str, usize)> = None;
        for rule in &self.rules {
            for code in &rule.codes {
                if kind.starts_with(code.as_str()) && best.is_none_or(|(_, len)| code.len() > len) {
                    best = Some((rule.surface.as_str(), code.len()));
                }
            }
        }
        best.map(|(s, _)| s)
    }
}

/// Devices sorted by surface.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Split {
    /// Stay on the plate.
    pub plate: Vec<Spec>,
    /// Sent to another surface, by surface name, in the order a device for
    /// that surface is first met.
    pub others: Vec<(String, Vec<Spec>)>,
    pub problems: Vec<String>,
}

/// Sorts `specs` by the rules. A rule naming a surface that does not exist
/// (or the main plate itself, which needs no rule) leaves its devices on the
/// plate and adds a sentence for the unknown ones.
pub fn split(specs: &[Spec], rules: &SurfaceRules, surfaces: &Surfaces) -> Split {
    let mut out = Split::default();
    let mut reported: Vec<String> = Vec::new();
    for spec in specs {
        match rules.surface_for(&spec.kind) {
            None => out.plate.push(spec.clone()),
            Some(name) if name == MAIN_PLATE => out.plate.push(spec.clone()),
            Some(name) => {
                if surfaces.find(Some(name)).is_none() {
                    if !reported.iter().any(|r| r == name) {
                        reported.push(name.to_string());
                        out.problems.push(format!(
                            "Surface rule '{name}' names a surface this enclosure does not have; its devices stay on the plate."
                        ));
                    }
                    out.plate.push(spec.clone());
                    continue;
                }
                match out.others.iter_mut().find(|(s, _)| s == name) {
                    Some((_, list)) => list.push(spec.clone()),
                    None => out.others.push((name.to_string(), vec![spec.clone()])),
                }
            }
        }
    }
    out
}

/// A plan for the devices of one surface other than the plate. Panel
/// devices on a door sit in cut-outs, not on rails, so the plan has no rails
/// (`Proposed::rail` is 0); rows are filled left to right inside the
/// surface's usable area.
pub fn plan_surface(surfaces: &Surfaces, name: &str, specs: &[Spec]) -> Option<Plan> {
    let surface = surfaces.find(Some(name))?;
    let candidates: Vec<Candidate> = specs
        .iter()
        .map(|s| Candidate {
            id: s.id,
            size: s.size,
            sort_key: s.sort_key.clone(),
        })
        .collect();
    let mut plan = place_all(&surface.plate(), &candidates, 1);
    plan.rails.clear();
    for p in &mut plan.placements {
        p.rail = 0;
    }
    Some(plan)
}
