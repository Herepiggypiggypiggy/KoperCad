//! The person's own symbol folder (Preferences > Symbols > My symbol
//! folder): its valid `*.toml` symbols are added to the editor's catalog
//! beside the built-in pack of the project's standard, and drawn from it.
//! Per user and read only; a bad file is listed, the rest still load.

use std::path::PathBuf;

use novo_symbols::{Library, LoadError, Pack};

use super::NovoApp;

/// What was last loaded from the folder.
#[derive(Debug, Default)]
pub(super) struct CustomSymbols {
    /// The pack and folder the catalog was built for; `None`: build again.
    pub key: Option<(Pack, Option<PathBuf>)>,
    /// Symbols added from the folder.
    pub loaded: usize,
    /// One line per file or symbol that was left out.
    pub problems: Vec<String>,
}

impl CustomSymbols {
    /// "3 symbols loaded" for the Preferences row; `None` without a folder.
    pub(super) fn summary(&self) -> Option<String> {
        let (_, folder) = self.key.as_ref()?;
        folder.as_ref()?;
        Some(match self.loaded {
            1 => "1 symbol loaded".to_owned(),
            n => format!("{n} symbols loaded"),
        })
    }
}

/// The library of `pack` with the folder's symbols added, how many were
/// added, and the problems met.
pub(super) fn build_library(
    pack: Pack,
    folder: Option<&std::path::Path>,
) -> (Library, usize, Vec<String>) {
    let base = Library::builtin_pack(pack).expect("the built-in symbol library is valid");
    let Some(folder) = folder else {
        return (base, 0, Vec::new());
    };
    let before = base.len();
    let mut problems = Vec::new();
    let extra = match Library::load_dir_lenient(folder) {
        Ok((extra, bad)) => {
            problems.extend(bad.iter().map(describe));
            extra
        }
        Err(e) => {
            problems.push(describe(&e));
            Library::default()
        }
    };
    let (library, left_out) = base.layered(&extra);
    problems.extend(left_out.iter().map(describe));
    let loaded = library.len() - before;
    (library, loaded, problems)
}

fn describe(e: &LoadError) -> String {
    e.to_string()
}

impl NovoApp {
    /// Keeps the catalog in step with the symbol folder. Returns true while
    /// a folder is (or just was) in use, so the plain pack sync stays out of
    /// the way. Rebuilds when the folder, its reload button or the project's
    /// standard changed.
    pub(super) fn sync_symbol_folder(&mut self, pack: Pack) -> bool {
        let folder = self.settings.symbol_folder.clone();
        let used_before = self
            .wb
            .custom_symbols
            .key
            .as_ref()
            .is_some_and(|k| k.1.is_some());
        if folder.is_none() && !used_before {
            return false;
        }
        let want = (pack, folder.clone());
        if self.wb.custom_symbols.key.as_ref() == Some(&want) {
            return true;
        }
        if self.canvas.schematic().is_none() {
            return true;
        }
        let (library, loaded, problems) = build_library(pack, folder.as_deref());
        let Some(view) = self.canvas.schematic() else {
            return true;
        };
        let mut catalog = novo_editor::Catalog::new(library);
        for id in view.editor.catalog.recent().iter().rev() {
            let id = catalog.in_pack(id);
            if catalog.get(&id).is_some() {
                catalog.used(&id);
            }
        }
        let favourites: Vec<String> = view
            .editor
            .catalog
            .favourites()
            .iter()
            .map(|id| catalog.in_pack(id))
            .filter(|id| catalog.get(id).is_some())
            .collect();
        catalog.set_favourites(favourites);
        view.editor.catalog = catalog;
        view.refresh_art();
        self.wb.custom_symbols = CustomSymbols {
            key: Some(want),
            loaded,
            problems,
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PUMP: &str = r#"
format = 1
id = "my/pump"
name = "My pump"
description = "A pump."
standard = "IEC 60617"
category = "my"
function = "motor.pump"
keywords = ["pump"]
aliases = ["mypump"]

connections = [
  { id = "1", at = [0, 0], dir = "up" },
  { id = "2", at = [0, 16], dir = "down" },
]

anchors = [
  { role = "tag", at = [-6, 8], align = "right" },
]

graphics = [
  { kind = "line", from = [0, 0], to = [0, 5] },
  { kind = "circle", center = [0, 8], r = 3 },
  { kind = "line", from = [0, 11], to = [0, 16] },
]

[behavior]
kind = "motor"
phases = 0
ports = [["1", "2"]]
"#;

    #[test]
    fn a_folder_adds_valid_symbols_and_lists_the_rest() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("my")).unwrap();
        std::fs::write(dir.path().join("my/pump.toml"), PUMP).unwrap();
        std::fs::write(dir.path().join("broken.toml"), "not a symbol").unwrap();
        let (lib, loaded, problems) = build_library(Pack::Iec, Some(dir.path()));
        assert_eq!(loaded, 1, "{problems:?}");
        assert!(lib.get("my/pump").is_some());
        assert!(lib.by_alias("mypump").is_some());
        assert!(!problems.is_empty(), "the broken file is listed");
        assert!(lib.get("iec/contacts/no").is_some(), "built-ins stay");
    }

    #[test]
    fn no_folder_gives_the_plain_pack() {
        let (lib, loaded, problems) = build_library(Pack::Iec, None);
        assert_eq!(loaded, 0);
        assert!(problems.is_empty());
        assert_eq!(lib.len(), Library::builtin().unwrap().len());
    }

    #[test]
    fn a_missing_folder_is_a_problem_not_a_crash() {
        let (lib, loaded, problems) =
            build_library(Pack::Iec, Some(std::path::Path::new("/no/such/folder")));
        assert_eq!(loaded, 0);
        assert_eq!(problems.len(), 1);
        assert!(lib.get("iec/contacts/no").is_some());
    }

    #[test]
    fn choosing_a_folder_puts_its_symbols_in_the_insert_catalog_and_clearing_takes_them_out() {
        use crate::app::gate1_tests::Drive;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib/my")).unwrap();
        std::fs::write(dir.path().join("lib/my/pump.toml"), PUMP).unwrap();
        let mut d = Drive::new(&dir.path().join("untitled"));
        let ctx = d.ctx.clone();
        d.app.run(&ctx, crate::app::Action::NewProject);
        d.frame();
        d.frame();
        let has = |d: &mut Drive| d.app.library().is_some_and(|l| l.get("my/pump").is_some());
        assert!(!has(&mut d));
        d.app.settings.symbol_folder = Some(dir.path().join("lib"));
        d.frame();
        d.frame();
        assert!(has(&mut d));
        assert_eq!(d.app.wb.custom_symbols.loaded, 1);
        d.app.settings.symbol_folder = None;
        d.frame();
        d.frame();
        assert!(!has(&mut d), "cleared: back to the plain pack");
        assert!(d.app.library().unwrap().get("iec/contacts/no").is_some());
    }
}
