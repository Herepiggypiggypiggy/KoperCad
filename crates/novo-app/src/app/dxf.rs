//! DXF export in the app (novo-interop, WS-24). One page goes to
//! `<project> - <page>.dxf` next to the project file; all pages go to a
//! folder `<project> DXF` beside it. A project with no file yet uses
//! `<Documents>\KoperCAD`. Millimetres, no dialog: the file opens upright in
//! any CAD program.
//!
//! Only a DXF KoperCAD wrote (its leading comment) is ever replaced: a
//! user's own file of the same name stays, and the export takes `<name> 2.dxf`.

use std::path::{Path, PathBuf};

use novo_core::{Command, PageId, Value};
use novo_interop::underlay as bg;

use super::{Level, NovoApp, projects_dir};

/// Which pages the File menu asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DxfScope {
    CurrentPage,
    AllPages,
}

impl NovoApp {
    /// Exports to the default place (see the module notes); returns the
    /// files written.
    pub(super) fn export_dxf(&mut self, scope: DxfScope) -> Vec<PathBuf> {
        let Some(name) = self.project_name() else {
            return Vec::new();
        };
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let (dir, stem) = match &file {
            Some(file) => (
                file.parent().map(Path::to_path_buf).unwrap_or_default(),
                file.file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| name.clone()),
            ),
            None => match projects_dir() {
                Some(dir) => (dir, name.clone()),
                None => {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return Vec::new();
                }
            },
        };
        self.export_dxf_to(scope, &dir, &stem)
    }

    /// Exports into `dir` (a folder `<stem> DXF` inside it for all pages).
    pub(super) fn export_dxf_to(
        &mut self,
        scope: DxfScope,
        dir: &Path,
        stem: &str,
    ) -> Vec<PathBuf> {
        let pages: Vec<PageId> = match scope {
            DxfScope::CurrentPage => self.current_page.into_iter().collect(),
            DxfScope::AllPages => self
                .backend
                .document()
                .map(|d| d.project().pages_in_order().iter().map(|p| p.id).collect())
                .unwrap_or_default(),
        };
        if pages.is_empty() {
            self.message(Level::Info, "There is no page to export");
            return Vec::new();
        }
        let target = match scope {
            DxfScope::CurrentPage => dir.to_path_buf(),
            DxfScope::AllPages => dir.join(format!("{} DXF", safe(stem))),
        };
        let style = crate::theme::model_style();
        let mut jobs = Vec::new();
        {
            let Some(doc) = self.backend.document() else {
                return Vec::new();
            };
            let art = self
                .library()
                .map(|lib| novo_editor::art::LibraryArt::new(lib, style.symbol));
            for id in pages {
                let Some(page) = doc.project().page(id) else {
                    continue;
                };
                let text = match &art {
                    Some(art) => novo_interop::bridge::page_dxf(doc.project(), id, art, style),
                    None => novo_interop::bridge::page_dxf(
                        doc.project(),
                        id,
                        &novo_canvas::model::PlaceholderArt,
                        style,
                    ),
                };
                if let Some(text) = text {
                    let file = novo_interop::bridge::page_file_name(stem, &page.name);
                    jobs.push((file, text));
                }
            }
        }
        let mut written = Vec::new();
        let mut replaced = false;
        for (file, text) in jobs {
            let path = replaceable_path(&target.join(file));
            replaced |= path.exists();
            if let Err(e) = write_replacing(&path, text.as_bytes()) {
                self.message(
                    Level::Error,
                    format!("Could not export {}: {e}", path.display()),
                );
                return written;
            }
            written.push(path);
        }
        let verb = if replaced { "Replaced" } else { "Exported" };
        let shown = match (scope, written.as_slice()) {
            (DxfScope::CurrentPage, [one]) => one.display().to_string(),
            _ => format!("{} pages to {}", written.len(), target.display()),
        };
        match written.last() {
            Some(last) => self.message_file(format!("{verb} {shown}"), last),
            None => self.message(Level::Info, format!("{verb} {shown}")),
        }
        if let Some(last) = written.last() {
            self.wb.last_export = Some(last.clone());
        }
        written
    }
}

impl NovoApp {
    /// Asks for a DXF and shows it under the current page.
    pub(super) fn import_dxf_background(&mut self) {
        let mut dialog = rfd::FileDialog::new()
            .set_title("Import DXF as background")
            .add_filter("DXF drawing", &["dxf"]);
        if let Some(dir) = projects_dir().filter(|dir| dir.is_dir()) {
            dialog = dialog.set_directory(dir);
        }
        if let Some(path) = dialog.pick_file() {
            self.import_dxf_background_from(&path);
        }
    }

    /// Stores the DXF at `path` in the current page's properties (one undo
    /// step). It is reference geometry: no devices or wires come from it.
    pub fn import_dxf_background_from(&mut self, path: &Path) -> bool {
        let Some(page) = self.current_page else {
            self.message(Level::Info, "Open a page first");
            return false;
        };
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let import = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| bg::import_underlay(&bytes, &name).map_err(|e| e.to_string()));
        let import = match import {
            Ok(import) => import,
            Err(e) => {
                self.message(Level::Error, format!("Could not import {name}: {e}"));
                return false;
            }
        };
        let summary = format!(
            "{}. Placed at the page corner; move and scale come later",
            import.summary(&name)
        );
        let commands = import
            .properties
            .into_iter()
            .map(|(key, value)| Command::SetProperty {
                target: page.into(),
                key: key.to_string(),
                value: Some(value),
            })
            .collect();
        self.run_background(Command::Batch {
            label: "Import DXF background".into(),
            commands,
        });
        self.message(Level::Info, summary);
        true
    }

    pub(super) fn page_properties(&self) -> Option<&novo_core::Properties> {
        let page = self.current_page?;
        Some(&self.backend.document()?.project().page(page)?.properties)
    }

    pub(super) fn has_background(&self) -> bool {
        self.page_properties()
            .is_some_and(|p| bg::stored_text(p).is_some())
    }

    pub(super) fn background_shown(&self) -> bool {
        self.page_properties().is_some_and(bg::is_visible)
    }

    pub(super) fn toggle_background(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let show = !self.background_shown();
        self.run_background(Command::SetProperty {
            target: page.into(),
            key: bg::KEY_VISIBLE.into(),
            value: Some(Value::Bool(show)),
        });
    }

    pub(super) fn remove_background(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let keys = [
            bg::KEY_DXF,
            bg::KEY_HEIGHT,
            bg::KEY_NAME,
            bg::KEY_DX,
            bg::KEY_DY,
            bg::KEY_SCALE,
            bg::KEY_VISIBLE,
        ];
        self.run_background(Command::Batch {
            label: "Remove DXF background".into(),
            commands: keys
                .into_iter()
                .map(|key| Command::SetProperty {
                    target: page.into(),
                    key: key.into(),
                    value: None,
                })
                .collect(),
        });
    }

    pub(super) fn run_background(&mut self, command: Command) {
        if let Some(Err(e)) = self.execute(command) {
            self.message(
                Level::Error,
                format!("Could not change the background: {e}"),
            );
        }
    }

    /// Hands the shown page's background to the canvas before it draws.
    /// The drawing is parsed only when the page or the project changed.
    pub(super) fn update_underlay(&mut self) {
        let Some(page) = self.current_page else {
            return;
        };
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let revision = project.revision();
        let seen = self.wb.underlay_seen;
        let key = match seen {
            Some((p, r, key)) if p == page && r == revision => key,
            _ => project
                .page(page)
                .and_then(|p| bg::cache_key(&p.properties)),
        };
        self.wb.underlay_seen = Some((page, revision, key));
        let props = project.page(page).map(|p| &p.properties);
        if let Some(view) = self.canvas.schematic() {
            view.set_underlay(key, || props.and_then(bg::underlay_of));
        }
    }
}

/// `stem` without characters Windows does not allow in a folder name.
fn safe(stem: &str) -> String {
    let name = novo_interop::bridge::page_file_name(stem, "");
    name.trim_end_matches(".dxf").to_string()
}

/// `path` when it is free or ours to replace, else the first free or ours
/// of `<stem> 2.dxf`, `<stem> 3.dxf`...
fn replaceable_path(path: &Path) -> PathBuf {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut n = 1;
    loop {
        let candidate = if n == 1 {
            path.to_owned()
        } else {
            path.with_file_name(format!("{stem} {n}.dxf"))
        };
        let ours = std::fs::read(&candidate).is_ok_and(|b| novo_interop::dxf::is_written_by_us(&b));
        if !candidate.exists() || ours {
            return candidate;
        }
        n += 1;
    }
}

/// Writes `bytes` to a temporary file beside `path`, then renames it over
/// `path`; the temporary file is removed when anything fails.
fn write_replacing(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let mut name = path.file_name().unwrap_or_default().to_owned();
    name.push(format!(".{}.part", std::process::id()));
    let temp = path.with_file_name(name);
    let written = std::fs::write(&temp, bytes).and_then(|()| std::fs::rename(&temp, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_our_own_dxf_files_are_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Plant - 1.dxf");
        assert_eq!(replaceable_path(&path), path, "free");

        let ours = novo_interop::dxf::write_dxf(&novo_interop::dxf::Drawing::default());
        std::fs::write(&path, ours).unwrap();
        assert_eq!(replaceable_path(&path), path, "ours");

        std::fs::write(&path, "0\r\nSECTION\r\n").unwrap();
        assert_eq!(
            replaceable_path(&path),
            dir.path().join("Plant - 1 2.dxf"),
            "the user's own file stays"
        );
    }

    #[test]
    fn folder_names_are_safe() {
        assert_eq!(safe("A/B"), "A_B");
    }
}
