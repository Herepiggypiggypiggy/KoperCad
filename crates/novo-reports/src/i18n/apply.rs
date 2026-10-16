//! Putting a language onto report data, templates and laid-out pages.

use crate::designer::{Element, Fields, Rendered, Template, render};
use crate::page::PageLayout;
use crate::table::{ReportOutput, Section};

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
    if FIXED_SECTION_TITLES.contains(&s.title.as_str()) {
        out.title = tr.tr(&s.title).into_owned();
    }
    for c in &mut out.columns {
        c.header = tr.tr(&c.header).into_owned();
    }
    out
}

/// A report with its title, fixed section titles and column headers in the
/// language. Cell values are project data and stay as they are; keys,
/// ids and row sources are untouched.
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
/// titles, column headers.
pub fn strings_of_output(output: &ReportOutput) -> Vec<String> {
    let mut out = Vec::new();
    push(&mut out, &output.title);
    for s in &output.sections {
        if FIXED_SECTION_TITLES.contains(&s.title.as_str()) {
            push(&mut out, &s.title);
        }
        for c in &s.columns {
            push(&mut out, &c.header);
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
