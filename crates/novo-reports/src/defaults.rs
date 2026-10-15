//! Which generated pages a project gets (DESIGN.md §8).
//!
//! The contents list opens the project as section `0`. The other reports
//! follow the schematic pages in their own numbered sections, ten apart and
//! above every existing page number (`10`, `20`, ... for a project of
//! pages 1 to 9), so a schematic page never moves when a section grows,
//! and the schematic pages can grow a little before a name clashes (a
//! clash is still handled: the sheet takes a `'`, see [`crate::sheet_name`]).
//!
//! A kind is proposed only when its report has rows (a project without
//! cables gets no cable page) and the project has no page of that kind
//! yet, so calling this again on open adds what has come into use and
//! never duplicates or renumbers a page.

use novo_core::{PageKind, Project, Result, Tx, natural_cmp};

use crate::TableOfContents;

/// One generated page to add: its report, its section (the page name) and
/// its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedPageSpec {
    /// The report id: the page's [`PageKind::Generated`] generator.
    pub generator: String,
    /// The section number, used as the page name (`20`); its sheets are
    /// `20a`, `20b`, ...
    pub name: String,
    pub title: String,
}

/// The generated pages the project should have and does not yet, in the
/// order they print: the contents list, bill of materials, device list,
/// wire list, terminal diagrams, cable diagrams.
pub fn default_generated_pages(project: &Project) -> Vec<GeneratedPageSpec> {
    default_generated_pages_with(project, &|_| true)
}

/// Like [`default_generated_pages`], proposing only the kinds `enabled`
/// accepts (by report id: `toc`, `bom`, `devices`, `wires`, `terminals`,
/// `cables`). This is the hook for a project rule that switches kinds off.
pub fn default_generated_pages_with(
    project: &Project,
    enabled: &dyn Fn(&str) -> bool,
) -> Vec<GeneratedPageSpec> {
    let has_kind = |id: &str| {
        project
            .pages()
            .any(|p| matches!(&p.kind, PageKind::Generated { generator } if generator == id))
    };
    let taken = |name: &str| project.pages().any(|p| p.name == name);
    // Above every page number in use, generated sections included.
    let highest = project
        .pages()
        .filter_map(|p| leading_number(&p.name))
        .max()
        .unwrap_or(0);
    let mut next = (highest / 10 + 1) * 10;
    let mut specs = Vec::new();
    for report in crate::standard_reports() {
        let id = report.id();
        if !enabled(id) || has_kind(id) {
            continue;
        }
        let name = if id == TableOfContents::ID && !taken("0") {
            "0".to_owned()
        } else {
            if report.generate(project).row_count() == 0 {
                continue;
            }
            let name = next.to_string();
            next += 10;
            name
        };
        specs.push(GeneratedPageSpec {
            generator: id.to_owned(),
            name,
            title: report.title(),
        });
    }
    specs.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    specs
}

/// Adds the pages `specs` describe (one transaction, so one undo step).
pub fn add_generated_pages(tx: &mut Tx<'_>, specs: &[GeneratedPageSpec]) -> Result<()> {
    for spec in specs {
        let page = tx.add_page(spec.name.clone())?;
        tx.update::<novo_core::Page>(page, |p| {
            p.title = spec.title.clone();
            p.kind = PageKind::Generated {
                generator: spec.generator.clone(),
            };
        })?;
    }
    Ok(())
}

/// The number a page name starts with (`12` for `12`, `12a` or `12.3`).
fn leading_number(name: &str) -> Option<u64> {
    let digits: String = name.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leading_numbers() {
        assert_eq!(leading_number("12"), Some(12));
        assert_eq!(leading_number("20b'"), Some(20));
        assert_eq!(leading_number("3.1"), Some(3));
        assert_eq!(leading_number("A"), None);
    }
}
