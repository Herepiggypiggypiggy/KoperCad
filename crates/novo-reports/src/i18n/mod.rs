//! Report and PDF languages (DOC-2): the same report in another language.
//!
//! The program's menus stay English; a project picks the language of what
//! it prints: reports, generated pages, designer templates and the PDF.
//! Command ids and file formats never change with the language.
//!
//! ```
//! use novo_reports::i18n::{ProjectTable, Translator};
//!
//! let nl = Translator::new("nl", &ProjectTable::default());
//! assert_eq!(nl.tr("Device list"), "Apparatenlijst");
//! assert_eq!(nl.tr("Line 3"), "Regel 3");          // pattern entry
//! assert_eq!(nl.tr("Unknown text"), "Unknown text"); // falls back to English
//! ```
//!
//! # Pieces
//!
//! - [`Catalog`]: one language's translation file, English text to text.
//!   The shipped Dutch file is `nl.json`; [`builtin`] lists the shipped
//!   files. Keys may hold `{name}` placeholders ([`Catalog::problems`]
//!   rejects a translation that changes them).
//! - [`ProjectTable`]: the project's own entries and whole languages,
//!   stored in the project property [`TABLE_KEY`]; the chosen language is
//!   [`LANGUAGE_KEY`]. Project entries win over the shipped file.
//! - [`Translator`]: looks a text up (project, then shipped file, then the
//!   English text itself, so nothing prints blank).
//! - [`translate_output`], [`translate_template`], [`translate_layout`] and
//!   [`render_in`] put a language on report data, designer templates and
//!   laid-out pages; [`strings_of_output`], [`strings_of_template`] and
//!   [`missing`] list the texts a language has no entry for.
//!
//! Cell values are project data (tags, descriptions, part names) and are
//! not translated.

mod apply;
mod catalog;
mod project;
mod translator;

pub use apply::{
    FIXED_SECTION_TITLE_PATTERNS, FIXED_SECTION_TITLES, is_fixed_title, is_translatable, missing,
    render_in, strings_of_output, strings_of_template, translate_layout, translate_output,
    translate_template,
};
pub use catalog::{
    Catalog, CatalogProblem, EXTENSION, LanguageFileError, builtin, check_entry, placeholders,
};
pub use project::{LANGUAGE_KEY, ProjectTable, TABLE_KEY, language_of};
pub use translator::{ENGLISH, Translator};

use novo_core::Project;

/// A language the picker offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
    /// Shipped with the program (not only in this project).
    pub shipped: bool,
}

/// English, the shipped languages, then the project's own, by code.
pub fn languages(table: &ProjectTable) -> Vec<LanguageInfo> {
    let mut out = vec![LanguageInfo {
        code: ENGLISH.into(),
        name: "English".into(),
        shipped: true,
    }];
    for c in builtin() {
        out.push(LanguageInfo {
            code: c.code,
            name: c.name,
            shipped: true,
        });
    }
    for code in table.codes() {
        if !out.iter().any(|l| l.code == code) {
            let name = table
                .language(code)
                .map(|c| c.name.clone())
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| code.to_owned());
            out.push(LanguageInfo {
                code: code.to_owned(),
                name,
                shipped: false,
            });
        }
    }
    out
}

/// The translator for the project's chosen language.
pub fn translator_of(project: &Project) -> Translator {
    Translator::new(&language_of(project), &ProjectTable::of_project(project))
}

/// Where the user finds the language settings.
pub const ENTRY_COMMAND: &str = "project.languages";
