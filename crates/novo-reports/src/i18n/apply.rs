//! Putting a language onto report data, templates and laid-out pages.

use crate::designer::{Element, Fields, Rendered, Template, render};
use crate::page::PageLayout;
use crate::table::{Cell, ReportOutput, Section};

use super::translator::Translator;

/// Section titles that are fixed text of the program, not names from the
/// project (a strip's tag, a cable's number). Only these are translated
/// and counted as missing.
pub const FIXED_SECTION_TITLES: &[&str] = &[
    "Check before cutting",
    "Checks",
    "Devices without parts",
    "Duct fill",
    "Ferrule standard used",
    "Ferrules",
    "Ferrules to crimp",
    "Overview",
    "Panels",
    "Punch list",
    "Sign-off",
    "Summary",
    "What to pull",
    "Wire ends without a ferrule",
    "Wire stock",
    "Wires to cut",
];

/// Section titles that are fixed text with project numbers filled in, as
/// `{name}` patterns. Translated through the same pattern entries.
pub const FIXED_SECTION_TITLE_PATTERNS: &[&str] = &[
    "What to pull · {estimated} of {total} lengths are estimates (~): measure on the plate",
    "{n} connections between device pins (supply and PE leads: see Wire production)",
    "Summary · {n} wires between device pins (supply and PE leads: see Wire production)",
    "Wires to cut · {n} (device pin to device pin, plus supply and PE leads)",
    "Wire labels · {n} (one per wire end: two for each wire-list connection)",
];

fn matches_pattern(pattern: &str, text: &str) -> bool {
    super::translator::fills(pattern, text)
}

/// Whether a section title is fixed program text: one of
/// [`FIXED_SECTION_TITLES`] or a filled-in [`FIXED_SECTION_TITLE_PATTERNS`].
pub fn is_fixed_title(title: &str) -> bool {
    FIXED_SECTION_TITLES.contains(&title)
        || FIXED_SECTION_TITLE_PATTERNS
            .iter()
            .any(|pattern| matches_pattern(pattern, title))
}

/// Cell texts that are fixed words of the program, not project data: the
/// status words, check names and notes the reports write themselves. A cell
/// is translated only when all of it is one of these (or fills a
/// [`FIXED_CELL_PATTERNS`] entry), so a tag or a part name is never touched.
pub const FIXED_CELL_TEXTS: &[&str] = &[
    "(no part)",
    "door, no gap",
    "estimate",
    "free air",
    "routed",
    "routed, no depth",
    "Built by (name)",
    "Cut DIN rail",
    "Cut Wiring duct",
    "Door",
    "Floor",
    "Left",
    "Plate2",
    "Plate3",
    "Plate4",
    "Right",
    "Roof",
    "No enclosure has a mounting plate yet. Create a cabinet and set its plate on the Cabinet canvas.",
    "Blocking punch items",
    "Cross-section not set: give the wire a size",
    "Devices are inside the plate margin",
    "Devices keep their clearances",
    "Done %",
    "Ducts are clear of rails and devices",
    "Every device of the cabinet is on the plate",
    "Every drill pattern could be read",
    "Every terminal is tightened",
    "Ferrule switched off on the wire (none)",
    "Labels are fixed and readable",
    "Mounting plate",
    "No device overlaps another",
    "No holes or cut-outs to make",
    "Not applicable",
    "Open punch items",
    "Part sizes are known",
    "Rails are long enough",
    "Ready for sign-off",
    "Reviewed by",
    "Total",
    "Wire lengths are measured, not estimated",
    "Wiring compared with the schematic",
    "Yes",
    "free",
    "horizontal",
    "no",
    "no wires in this cabinet",
    "ok",
    "overfull",
    "plate",
    "unsized",
    "vertical",
    "yes",
];

/// Fixed cell texts with project numbers or names filled in, as `{name}`
/// patterns.
pub const FIXED_CELL_PATTERNS: &[&str] = &[
    "{mark} at {offset}",
    "Cut Wiring duct {w} x {h}",
    "Cut-out dia {d}",
    "Cut-out {w} x {h}",
    "Fit drill dia {d}: {count} holes",
    "Hole dia {d}",
    "Mounting plate {w} x {h} mm, margin {m} mm",
    "guessed size: {list}",
    "not placed: {list}",
    "unreadable: {list}",
    "{length} mm horizontal",
    "{length} mm vertical",
    "{mark} DIN rail",
    "{mark} Wiring duct",
    "{mark} Wiring duct {w} x {h}",
    "{n} outside",
    "{n} overlapping pairs",
    "{n} overlaps",
    "{n} pairs too close",
    "{n} rails overflow",
    "{n} wires use an estimated length",
];

/// Whether a cell text is fixed program text: one of [`FIXED_CELL_TEXTS`]
/// or a filled-in [`FIXED_CELL_PATTERNS`] entry.
pub fn is_fixed_cell(text: &str) -> bool {
    FIXED_CELL_TEXTS.contains(&text)
        || FIXED_CELL_PATTERNS
            .iter()
            .any(|pattern| matches_pattern(pattern, text))
}

/// A text that is worth translating: it has a letter outside `{fields}`.
pub fn is_translatable(text: &str) -> bool {
    let mut depth = 0;
    for c in text.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            c if depth == 0 && c.is_alphabetic() => return true,
            _ => {}
        }
    }
    false
}

fn section(tr: &Translator, s: &Section) -> Section {
    let mut out = s.clone();
    if is_fixed_title(&s.title) {
        out.title = tr.tr(&s.title).into_owned();
    }
    for c in &mut out.columns {
        c.header = tr.tr(&c.header).into_owned();
    }
    for row in &mut out.rows {
        for cell in &mut row.cells {
            if let Cell::Text(text) = cell {
                if text == "No" {
                    // The answer, not the column header `No` (a number).
                    *text = tr.translate("No (answer)").unwrap_or_else(|| text.clone());
                } else if is_fixed_cell(text) {
                    *text = tr.tr(text).into_owned();
                }
            }
        }
    }
    out.decimal = tr.decimal();
    out
}

/// A report with its title, fixed section titles and column headers in the
/// language, fixed words in cells ([`FIXED_CELL_TEXTS`]) translated and
/// measured values set to the language's decimal separator. Other cell
/// values are project data and stay as they are; keys, ids and row
/// sources are untouched.
pub fn translate_output(output: &ReportOutput, tr: &Translator) -> ReportOutput {
    if tr.is_english() {
        return output.clone();
    }
    ReportOutput {
        title: tr.tr(&output.title).into_owned(),
        sections: output.sections.iter().map(|s| section(tr, s)).collect(),
        ..output.clone()
    }
}

/// A template with its texts, title block captions and values, table
/// headings and column headers in the language. Name, geometry and field
/// placeholders are untouched, so the translated template renders with the
/// same fields.
pub fn translate_template(template: &Template, tr: &Translator) -> Template {
    if tr.is_english() {
        return template.clone();
    }
    let mut out = template.clone();
    if let Some(block) = &mut out.title_block {
        for row in &mut block.rows {
            row.caption = tr.tr_keyed(row.key.as_deref(), &row.caption);
            row.value = tr.tr(&row.value).into_owned();
        }
    }
    for e in &mut out.elements {
        match e {
            Element::Text { text, key, .. } => *text = tr.tr_keyed(key.as_deref(), text),
            Element::Table(t) => {
                t.spec.heading = tr.tr(&t.spec.heading).into_owned();
                for c in &mut t.spec.columns {
                    if let Some(h) = &mut c.header {
                        *h = tr.tr(h).into_owned();
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A laid-out page with the fixed words the pager printed (`page 2 / 5`,
/// `(continued)`) and any other text with an entry in the language. Data
/// cells without an entry are left alone.
pub fn translate_layout(layout: &PageLayout, tr: &Translator) -> PageLayout {
    if tr.is_english() {
        return layout.clone();
    }
    let mut out = layout.clone();
    for t in &mut out.texts {
        if let Some(head) = t.text.strip_suffix(" (continued)") {
            let head = tr.tr(head);
            let tail = tr.tr("(continued)");
            t.text = format!("{head} {tail}");
        } else if let Some(n) = t.text.strip_prefix("page ").filter(|n| {
            n.split(" / ")
                .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
        }) {
            let mut it = n.split(" / ");
            let (page, pages) = (it.next().unwrap_or(""), it.next().unwrap_or(""));
            t.text = tr
                .tr("page {page} / {pages}")
                .replace("{page}", page)
                .replace("{pages}", pages);
        }
    }
    out
}

/// Renders a template in a language: the template, the report data and the
/// fixed pager words are all translated. English renders exactly as
/// [`render`].
pub fn render_in(
    template: &Template,
    data: &[ReportOutput],
    fields: &Fields,
    tr: &Translator,
) -> Rendered {
    if tr.is_english() {
        return render(template, data, fields);
    }
    let data: Vec<ReportOutput> = data.iter().map(|o| translate_output(o, tr)).collect();
    let mut rendered = render(&translate_template(template, tr), &data, fields);
    rendered.pages = rendered
        .pages
        .iter()
        .map(|p| translate_layout(p, tr))
        .collect();
    rendered
}

fn push(out: &mut Vec<String>, text: &str) {
    if is_translatable(text) && !out.iter().any(|o| o == text) {
        out.push(text.to_owned());
    }
}

/// The texts of a report that need a translation: title, fixed section
/// titles, column headers and fixed words in cells.
pub fn strings_of_output(output: &ReportOutput) -> Vec<String> {
    let mut out = Vec::new();
    push(&mut out, &output.title);
    for s in &output.sections {
        if is_fixed_title(&s.title) {
            push(&mut out, &s.title);
        }
        for c in &s.columns {
            push(&mut out, &c.header);
        }
        for cell in s.rows.iter().flat_map(|r| &r.cells) {
            if let Cell::Text(text) = cell {
                if text == "No" {
                    push(&mut out, "No (answer)");
                } else if is_fixed_cell(text) {
                    push(&mut out, text);
                }
            }
        }
    }
    out
}

/// The texts of a template that need a translation: free texts, title block
/// captions and values, table headings and column headers it sets itself.
pub fn strings_of_template(template: &Template) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(b) = &template.title_block {
        for r in &b.rows {
            push(&mut out, &r.caption);
            push(&mut out, &r.value);
        }
    }
    for e in &template.elements {
        match e {
            Element::Text { text, .. } => push(&mut out, text),
            Element::Table(t) => {
                push(&mut out, &t.spec.heading);
                for h in t.spec.columns.iter().filter_map(|c| c.header.as_deref()) {
                    push(&mut out, h);
                }
            }
            _ => {}
        }
    }
    out
}

/// The keys in `strings` that `tr` has no translation for, in order. English
/// is the source and misses nothing.
pub fn missing<'a>(tr: &Translator, strings: impl IntoIterator<Item = &'a String>) -> Vec<String> {
    strings
        .into_iter()
        .filter(|s| !tr.knows(s))
        .cloned()
        .collect()
}
