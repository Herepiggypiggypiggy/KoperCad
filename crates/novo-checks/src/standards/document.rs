//! The compliance document: the report arranged for a person who did not run
//! the check, a customer or an inspector.
//!
//! Per requirement it states the clause pointer, what the rule asks in our
//! own words, a verdict (pass, fail, not checkable), the evidence found in
//! the project, where to fix it, and what the verdict rests on: nothing but
//! the drawing, or planning values (see [`provenance`]). It never says the
//! project complies: it says what was checked, and what was not.
//!
//! The model is presentation-neutral ([`ComplianceDocument`]); [`to_html`]
//! renders a self-contained page that prints on A4, and the app maps the same
//! model to a PDF table report.

use novo_core::Project;

use super::provenance::{self, BasisItem, BasisKind};
use super::{ComplianceReport, NOTICE, RuleResult, RuleStatus, StandardSet, page_name};

/// How a requirement came out, in the words a reader expects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Pass,
    Fail,
    /// Some items passed, others lacked the data to be judged.
    Partial,
    /// Data is missing: the program cannot say.
    NotCheckable,
    /// Every finding is waived by the engineer, with a reason.
    Waived,
    NotApplicable,
}

impl Verdict {
    pub fn label(self) -> &'static str {
        match self {
            Verdict::Pass => "Pass",
            Verdict::Fail => "Fail",
            Verdict::Partial => "Partly checkable",
            Verdict::NotCheckable => "Not checkable",
            Verdict::Waived => "Waived",
            Verdict::NotApplicable => "Not applicable",
        }
    }

    fn of(status: RuleStatus) -> Verdict {
        match status {
            RuleStatus::Pass => Verdict::Pass,
            RuleStatus::Fail => Verdict::Fail,
            RuleStatus::Partial => Verdict::Partial,
            RuleStatus::NotEvaluated => Verdict::NotCheckable,
            RuleStatus::NotApplicable => Verdict::NotApplicable,
            RuleStatus::Waived => Verdict::Waived,
        }
    }

    fn class(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Partial | Verdict::NotCheckable => "unknown",
            Verdict::Waived => "waived",
            Verdict::NotApplicable => "na",
        }
    }
}

/// A finding with where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub text: String,
    /// Page name, when the finding is on a page.
    pub page: Option<String>,
    /// For a waived finding: the engineer's reason.
    pub reason: Option<String>,
}

/// One requirement of a standard.
#[derive(Debug, Clone, PartialEq)]
pub struct Requirement {
    pub id: &'static str,
    pub clause: &'static str,
    pub title: &'static str,
    pub requirement: &'static str,
    pub verdict: Verdict,
    /// How many things the rule evaluated.
    pub checked: usize,
    /// What passed, one line per thing (may be fewer than `checked`).
    pub evidence: Vec<String>,
    pub findings: Vec<Finding>,
    /// What could not be judged, and why.
    pub not_checkable: Vec<String>,
    /// What the rule reads: shown when data is missing.
    pub needs: &'static str,
    /// Where and how to fix a finding, or to supply missing data.
    pub fix: &'static str,
    /// Planning values or models the verdict rests on. Empty when the rule
    /// is structural.
    pub basis: Vec<&'static BasisItem>,
}

impl Requirement {
    /// Whether a pass or fail here depends on a value that was not checked
    /// against the licensed standard.
    pub fn rests_on_planning_values(&self) -> bool {
        self.basis.iter().any(|b| b.kind == BasisKind::Planning)
    }
}

/// One standard's section.
#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub set: StandardSet,
    pub requirements: Vec<Requirement>,
    /// Project overrides of this set's built-in values (the engineer's own).
    pub overrides: Vec<String>,
}

impl Section {
    pub fn count(&self, v: Verdict) -> usize {
        self.requirements.iter().filter(|r| r.verdict == v).count()
    }
}

/// The document.
#[derive(Debug, Clone, PartialEq)]
pub struct ComplianceDocument {
    pub project_name: String,
    pub notice: &'static str,
    pub sections: Vec<Section>,
    /// Planning values and models used by the selected sets' rules.
    pub basis: Vec<&'static BasisItem>,
    /// What the check does not cover at all.
    pub not_covered: Vec<&'static str>,
}

/// What no rule here looks at. Printed so a reader does not assume it was.
pub const NOT_COVERED: &[&str] = &[
    "Conductor current-carrying capacity under IEC 60204-1 and the IEC 60364-5-52 tables: those tables feed the cable calculations only, and every value is a planning value (not loaded from a licensed copy).",
    "Anything not drawn: the physical build, enclosure ingress protection, clearances, creepage, marking on the machine, the technical file, risk assessment, verification tests and measurements.",
    "Selection and rating of parts beyond the properties stored on devices and parts.",
    "Whether a device property stated by the user is true.",
];

/// Where and how to fix a rule's findings or missing data.
pub fn fix_hint(rule_id: &str) -> &'static str {
    match rule_id {
        "iec60204.supply-disconnect" | "nfpa79.supply-disconnect" => {
            "Place a disconnecting device (switch-disconnector, or a breaker marked isolating) between the supply feed and the loads. Mark a breaker suitable for isolation with `isolating = true` on the device or its part."
        }
        "iec60204.disconnect-lockable" | "nfpa79.disconnect-lockable" => {
            "Select the disconnecting device and set `lockable = true` in the Inspector, or choose a part that can be locked in OFF."
        }
        "iec60204.pe-connection" | "nfpa79.pe-connection" => {
            "Wire the motor's PE connection point to a protective-earth potential on the schematic page named in the finding."
        }
        "iec60204.pe-size" | "nfpa79.ground-size" | "ul508a.ground-size" => {
            "Select the protective or grounding wire and set its cross-section in the Inspector (or the cable core), at least the size the finding names."
        }
        "iec60204.pe-colour" | "nfpa79.pe-colour" => {
            "Select the wire and set its colour to green-yellow; use that colour on protective earth only."
        }
        "iec60204.neutral-colour" | "nfpa79.neutral-colour" => {
            "Select the neutral wire and set its colour to blue."
        }
        "iec60204.conductor-colour" | "nfpa79.conductor-colour" => {
            "Select the wires named in the finding and set the colour for their circuit type in the Inspector."
        }
        "iec60204.control-voltage" | "nfpa79.control-voltage" => {
            "Change the potential voltage of the control circuit, or feed it through a control transformer; the Potentials view shows each net's voltage."
        }
        "iec60204.control-supply" => {
            "Feed the control circuit from a control transformer with its own potential instead of the power potentials."
        }
        "iec60204.control-protection" | "nfpa79.control-protection" => {
            "Add a fuse or circuit breaker between the control supply and the control devices."
        }
        "iec60204.short-circuit-protection" | "nfpa79.short-circuit-protection" => {
            "Add a fuse or circuit breaker upstream of the load named in the finding."
        }
        "iec60204.motor-overload" | "nfpa79.motor-overload" => {
            "Add an overload relay or motor protection breaker, or set `thermal_protection = true` on a motor with a built-in thermal sensor."
        }
        "iec60204.estop-present" | "nfpa79.estop-present" => {
            "Add an emergency stop device, or state `machine.estop_required = false` in Project settings when your risk assessment says none is needed."
        }
        "iec60204.estop-contact" | "nfpa79.estop-contact" => {
            "Use a normally closed, positively opening contact for the emergency stop; set the device's contact property accordingly."
        }
        "iec60204.estop-colour" | "nfpa79.estop-colour" => {
            "Set the emergency stop actuator colour to red (`actuator_colour`) on the device or its part."
        }
        "iec60204.stop-category" | "nfpa79.stop-category" => {
            "State the stop category (0, 1 or 2) in `stop_category` on the stop device."
        }
        "iec60204.pushbutton-colour" | "nfpa79.pushbutton-colour" => {
            "Set `actuator_colour` on the pushbutton to a colour the standard allows for its function (`function`)."
        }
        "iec60204.indicator-colour" | "nfpa79.indicator-colour" => {
            "Set the lamp colour on the indicator to one that fits its function."
        }
        "iec60204.nameplate" | "nfpa79.nameplate" | "ul508a.nameplate" => {
            "Fill in the machine data in Project settings: rated voltage, phases, frequency and full-load current."
        }
        "nfpa79.conductor-ampacity" | "ul508a.conductor-ampacity" => {
            "Increase the wire cross-section, or reduce the protective device rating (`rated_current_a`), as the finding says."
        }
        "nfpa79.motor-branch-size" | "ul508a.motor-branch-size" => {
            "Change the branch protective device rating, or enter the motor's `flc_a` on the motor device or its part."
        }
        "ul508a.overload-setting" => {
            "Enter the overload relay setting and the motor nameplate current on the devices."
        }
        "nfpa79.sccr" | "ul508a.sccr-adequate" => {
            "Raise the panel rating: use components with a higher `sccr_ka`, or add a current-limiting device (`current_limiting`, `let_through_ka`) upstream."
        }
        "ul508a.sccr-components" => {
            "Enter `sccr_ka` on each power component's device or part, from its data sheet or marking."
        }
        "ul508a.sccr-marking" => {
            "State the panel's marked rating in Project settings (`panel.sccr_ka`) and the available fault current (`supply.short_circuit_ka`)."
        }
        _ => {
            "Open the page named in the finding, select the item and correct it in the Inspector, then run the check again."
        }
    }
}

fn requirement(project: &Project, r: &RuleResult) -> Requirement {
    Requirement {
        id: r.rule.id,
        clause: r.rule.clause,
        title: r.rule.title,
        requirement: r.rule.requirement,
        verdict: Verdict::of(r.status),
        checked: r.checked,
        evidence: r.evidence.clone(),
        findings: r
            .open
            .iter()
            .map(|p| Finding {
                text: p.message.clone(),
                page: page_name(project, p.page),
                reason: None,
            })
            .chain(r.waived.iter().map(|w| Finding {
                text: w.problem.message.clone(),
                page: page_name(project, w.problem.page),
                reason: Some(w.reason.clone()),
            }))
            .collect(),
        not_checkable: r.unknown.iter().map(|u| u.why.clone()).collect(),
        needs: r.rule.inputs,
        fix: fix_hint(r.rule.id),
        basis: provenance::basis_of(r.rule.id),
    }
}

/// Arranges a compliance report as a document.
pub fn build(project: &Project, report: &ComplianceReport) -> ComplianceDocument {
    let sections: Vec<Section> = report
        .sets
        .iter()
        .map(|set| Section {
            set: *set,
            requirements: report.of(*set).map(|r| requirement(project, r)).collect(),
            overrides: provenance::overrides(project, *set),
        })
        .collect();
    let mut basis: Vec<&'static BasisItem> = Vec::new();
    for section in &sections {
        for r in &section.requirements {
            for b in &r.basis {
                if !basis.contains(b) {
                    basis.push(b);
                }
            }
        }
    }
    ComplianceDocument {
        project_name: report.project_name.clone(),
        notice: NOTICE,
        sections,
        basis,
        not_covered: NOT_COVERED.to_vec(),
    }
}

fn esc(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const STYLE: &str = "body{font:11pt/1.4 'Segoe UI',Arial,sans-serif;color:#111;margin:18mm 16mm;max-width:190mm}h1{font-size:20pt;margin:0 0 4pt}h2{font-size:14pt;margin:22pt 0 4pt;border-bottom:1.5pt solid #111;padding-bottom:2pt}h3{font-size:11pt;margin:0}.notice{border:1pt solid #111;padding:6pt 8pt;margin:10pt 0;font-size:10pt}table.sum{border-collapse:collapse;margin:6pt 0}table.sum td,table.sum th{border:.5pt solid #777;padding:2pt 8pt;text-align:left}.req{border:.5pt solid #777;margin:8pt 0;padding:6pt 8pt;break-inside:avoid}.head{display:flex;justify-content:space-between;gap:8pt}.v{font-weight:700;white-space:nowrap}.pass .v{color:#0a6b2d}.fail .v{color:#b00020}.unknown .v{color:#8a5a00}.waived .v,.na .v{color:#555}.clause{color:#555;font-size:9.5pt}.req p{margin:3pt 0}.lbl{font-weight:700}.tag{display:inline-block;border:.5pt solid #8a5a00;color:#8a5a00;padding:0 4pt;font-size:9pt;margin-left:4pt}ul{margin:2pt 0 2pt 16pt;padding:0}.sign td{height:14mm;border:.5pt solid #777;padding:2pt 8pt;width:25%}@media print{body{margin:0}}";

/// The document as one self-contained HTML page that prints on A4. `date`
/// is printed under the title; pass an empty string to leave it out (output
/// is otherwise deterministic).
pub fn to_html(doc: &ComplianceDocument, date: &str) -> String {
    let mut h = String::new();
    h.push_str(&format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>Standards compliance report: {}</title><style>@page{{size:A4;margin:14mm}}{STYLE}</style></head><body>",
        esc(&doc.project_name)
    ));
    h.push_str(&format!(
        "<h1>Standards compliance report</h1><p><b>{}</b>{}</p>",
        esc(&doc.project_name),
        if date.is_empty() {
            String::new()
        } else {
            format!(" &middot; {}", esc(date))
        }
    ));
    h.push_str(&format!("<div class=\"notice\">{}</div>", esc(doc.notice)));
    h.push_str("<h2>Summary</h2><table class=\"sum\"><tr><th>Standard</th><th>Pass</th><th>Fail</th><th>Partly</th><th>Not checkable</th><th>Waived</th><th>n/a</th></tr>");
    for s in &doc.sections {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(s.set.short()),
            s.count(Verdict::Pass),
            s.count(Verdict::Fail),
            s.count(Verdict::Partial),
            s.count(Verdict::NotCheckable),
            s.count(Verdict::Waived),
            s.count(Verdict::NotApplicable)
        ));
    }
    h.push_str("</table>");
    let planning = doc
        .sections
        .iter()
        .flat_map(|s| &s.requirements)
        .filter(|r| {
            r.rests_on_planning_values()
                && matches!(r.verdict, Verdict::Pass | Verdict::Fail | Verdict::Partial)
        })
        .count();
    if planning > 0 {
        h.push_str(&format!(
            "<p>{planning} verdicts rest on planning values that were not checked against the licensed standard (marked <span class=\"tag\">planning value</span>). Confirm them before relying on those verdicts; see the basis table at the end.</p>"
        ));
    }
    for s in &doc.sections {
        h.push_str(&format!(
            "<h2>{}: {}</h2><p class=\"clause\">Edition referred to: {}</p>",
            esc(s.set.short()),
            esc(s.set.title()),
            esc(s.set.edition())
        ));
        if !s.overrides.is_empty() {
            h.push_str(&format!(
                "<p>Project values replace built-in ones: {}. These are the engineer's own entries.</p>",
                esc(&s.overrides.join(", "))
            ));
        }
        for r in &s.requirements {
            let planning = r.rests_on_planning_values()
                && matches!(r.verdict, Verdict::Pass | Verdict::Fail | Verdict::Partial);
            h.push_str(&format!(
                "<div class=\"req {}\"><div class=\"head\"><h3>{} {}{}</h3><span class=\"v\">{}</span></div><p class=\"clause\">Clause {} &middot; rule {}</p><p>{}</p>",
                r.verdict.class(),
                esc(s.set.short()),
                esc(r.title),
                if planning { " <span class=\"tag\">planning value</span>" } else { "" },
                r.verdict.label(),
                esc(r.clause),
                esc(r.id),
                esc(r.requirement)
            ));
            if r.verdict == Verdict::NotApplicable {
                h.push_str("<p>Nothing in the project falls under this requirement.</p>");
            }
            if !r.evidence.is_empty() || (r.checked > 0 && r.findings.is_empty()) {
                h.push_str(&format!(
                    "<p><span class=\"lbl\">Evidence</span> ({} checked)</p>",
                    r.checked
                ));
                if !r.evidence.is_empty() {
                    h.push_str("<ul>");
                    for e in r.evidence.iter().take(12) {
                        h.push_str(&format!("<li>{}</li>", esc(e)));
                    }
                    if r.evidence.len() > 12 {
                        h.push_str(&format!("<li>and {} more</li>", r.evidence.len() - 12));
                    }
                    h.push_str("</ul>");
                }
            }
            if !r.findings.is_empty() {
                h.push_str("<p><span class=\"lbl\">Findings</span></p><ul>");
                for f in &r.findings {
                    h.push_str(&format!(
                        "<li>{}{}{}</li>",
                        esc(&f.text),
                        f.page
                            .as_ref()
                            .map(|p| format!(" (page {})", esc(p)))
                            .unwrap_or_default(),
                        f.reason
                            .as_ref()
                            .map(|x| format!(" <b>Waived</b>, reason: {}", esc(x)))
                            .unwrap_or_default()
                    ));
                }
                h.push_str("</ul>");
            }
            if !r.not_checkable.is_empty() {
                h.push_str(&format!(
                    "<p><span class=\"lbl\">Not checkable</span>, data missing. Needs: {}</p><ul>",
                    esc(r.needs)
                ));
                for u in &r.not_checkable {
                    h.push_str(&format!("<li>{}</li>", esc(u)));
                }
                h.push_str("</ul>");
            }
            if matches!(
                r.verdict,
                Verdict::Fail | Verdict::Partial | Verdict::NotCheckable
            ) {
                h.push_str(&format!(
                    "<p><span class=\"lbl\">Where to fix</span> {}</p>",
                    esc(r.fix)
                ));
            }
            if !r.basis.is_empty() {
                h.push_str(&format!(
                    "<p class=\"clause\">Rests on: {}</p>",
                    esc(&r
                        .basis
                        .iter()
                        .map(|b| b.kind.label().to_owned() + " (" + b.id + ")")
                        .collect::<Vec<_>>()
                        .join(", "))
                ));
            }
            h.push_str("</div>");
        }
    }
    h.push_str("<h2>Basis of the numbers</h2><p>These values and methods were not checked against the licensed text of the standard. A verdict that uses one is marked above. To record that you have confirmed a value, set the project value (<code>standards.&lt;set&gt;.&lt;name&gt;</code>); the report then lists it as yours.</p><table class=\"sum\"><tr><th>Item</th><th>Kind</th><th>What</th><th>How it was checked</th><th>Used by</th></tr>");
    for b in &doc.basis {
        h.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
            esc(b.id),
            b.kind.label(),
            esc(b.what),
            esc(b.checked),
            esc(&b.used_by.join(", "))
        ));
    }
    h.push_str("</table><h2>Not covered</h2><ul>");
    for n in &doc.not_covered {
        h.push_str(&format!("<li>{}</li>", esc(n)));
    }
    h.push_str("</ul><h2>Review</h2><table class=\"sign\"><tr><td>Reviewed by</td><td></td><td>Date</td><td></td></tr><tr><td>Signature</td><td></td><td>Role</td><td></td></tr></table></body></html>");
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_rule_has_a_specific_fix() {
        let generic = fix_hint("no.such.rule");
        for r in super::super::RULES {
            assert_ne!(
                fix_hint(r.id),
                generic,
                "{} has only the generic hint",
                r.id
            );
        }
    }

    #[test]
    fn escaping() {
        assert_eq!(esc("<a & \"b\">"), "&lt;a &amp; &quot;b&quot;&gt;");
    }
}
