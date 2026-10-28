//! Writing a proposal as profile text (`*.controller.toml`) and as a binding table.

use std::fmt::Write;

use super::propose::{Proposal, ProposedProfile};

/// A TOML basic string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn list(items: &[String]) -> String {
    let parts: Vec<String> = items.iter().map(|s| quote(s)).collect();
    format!("[{}]", parts.join(", "))
}

impl ProposedProfile {
    /// The profile in the controller profile format (`docs/design/controller-profile.md`). The bindings are not part of it: see [`Proposal::bindings_csv`].
    pub fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("# Made by the KoperCAD controller import wizard from an I/O table.\n");
        out.push_str("# A guess from the words of the table: check it before relying on it.\n\n");
        out.push_str("[controller]\n");
        let _ = writeln!(out, "id = {}", quote(&self.id));
        let _ = writeln!(out, "name = {}", quote(&self.name));
        let _ = writeln!(out, "kind = {}", quote(&self.kind));
        if let Some(s) = self.supply {
            let volts = if s.volts.fract() == 0.0 {
                format!("{}", s.volts as i64)
            } else {
                format!("{}", s.volts)
            };
            let _ = writeln!(
                out,
                "supply = {{ kind = {}, volts = {volts} }}",
                quote(if s.ac { "ac" } else { "dc" })
            );
        }
        if let Some(c) = &self.common {
            let _ = writeln!(out, "common = {}", quote(c));
        }
        if self.generic {
            out.push_str("generic = true\n");
        }
        if !self.step2_includes_step1 {
            out.push_str("step2_includes_step1 = false\n");
        }
        if !self.gate.is_empty() {
            let _ = writeln!(out, "gate = {}", list(&self.gate));
        }
        for c in &self.controls {
            out.push_str("\n[[control]]\n");
            let _ = writeln!(out, "id = {}", quote(&c.id));
            let _ = writeln!(out, "label = {}", quote(&c.label));
            let _ = writeln!(out, "kind = {}", quote(&c.kind));
            if !c.positions.is_empty() {
                let _ = writeln!(out, "positions = {}", list(&c.positions));
            }
            if c.needs_link {
                out.push_str("needs_link = true\n");
            }
            if !c.requires.is_empty() {
                let _ = writeln!(out, "requires = {}", list(&c.requires));
            }
            if !c.drops_when.is_empty() {
                let _ = writeln!(out, "drops_when = {}", list(&c.drops_when));
            }
        }
        for s in &self.signals {
            out.push_str("\n[[signal]]\n");
            let _ = writeln!(out, "name = {}", quote(&s.name));
            let _ = writeln!(out, "control = {}", quote(&s.control));
            if let (Some(g), Some(n)) = (&s.group, s.step) {
                let _ = writeln!(out, "group = {}", quote(g));
                let _ = writeln!(out, "step = {n}");
            } else {
                let _ = writeln!(out, "when = {}", list(&s.when));
            }
            if s.ungated {
                out.push_str("ungated = true\n");
            }
            if !s.pin.is_empty() {
                let _ = writeln!(out, "pin = {}", quote(&s.pin));
            }
        }
        out
    }
}

impl Proposal {
    /// The binding table as semicolon separated text: signal, pin, target.
    pub fn bindings_csv(&self) -> String {
        let mut out = String::from("signal;pin;target\n");
        for b in &self.bindings {
            let _ = writeln!(out, "{};{};{}", b.signal, b.pin, b.target);
        }
        out
    }
}
