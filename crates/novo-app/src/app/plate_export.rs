//! Plate drawing export: the mounting plate (and door sheet) of the
//! enclosure shown in the Cabinet canvas, as one PDF with the drill table
//! (novo-layout `plate_documents`, novo-pdf `export_plate_pdf`). It goes
//! next to the project file as `<project> <enclosure> plate.pdf`, or into
//! `<Documents>\KoperCAD` for a project with no file yet, and follows the
//! same replace rules as the project PDF.

use std::path::PathBuf;

use novo_layout::model::plate_documents;
use novo_layout::plate_drawing::PlateDocument;

use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

impl NovoApp {
    /// Exports the plate drawing of the enclosure selected in the Cabinet
    /// canvas. Says why when there is nothing to export.
    fn plate_sheets(&mut self) -> Option<(String, String, Vec<PlateDocument>)> {
        let name = self.project_name()?;
        let location = match self.cabinet.location.clone() {
            Some(location) => location,
            None => {
                // Not in the Cabinet: take the only enclosure that has a plate,
                // as the command line does.
                let only = self.backend.document().and_then(|doc| {
                    let mut with_plate = doc
                        .project()
                        .structures()
                        .filter(|s| s.enclosure && s.plate.is_some());
                    let first = with_plate.next()?;
                    with_plate.next().is_none().then(|| first.tag.clone())
                });
                let Some(only) = only else {
                    self.message(
                        Level::Warning,
                        "Open the Cabinet and pick an enclosure first",
                    );
                    return None;
                };
                only
            }
        };
        let docs = {
            let doc = self.backend.document()?;
            let project = doc.project();
            project
                .structures()
                .find(|s| s.enclosure && s.tag == location)
                .and_then(|s| plate_documents(project, s.id))
        };
        let Some((docs, _table)) = docs else {
            self.message(
                Level::Warning,
                format!("{location} has no mounting plate yet; set its size in the Cabinet"),
            );
            return None;
        };
        Some((name, location.to_string(), docs))
    }

    /// Exports the plate drawing of the enclosure selected in the Cabinet
    /// canvas. Says why when there is nothing to export.
    pub(super) fn export_plate_pdf(&mut self) -> Option<PathBuf> {
        let (name, location, docs) = self.plate_sheets()?;
        let file_name = format!("{name} {location} plate.pdf").replace(['/', '\\', ':'], "-");
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let dir = match file.as_ref().and_then(|f| f.parent()) {
            Some(dir) => dir.to_owned(),
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                dir
            }
        };
        let path = replaceable_path(&dir.join(file_name));
        let replaced = path.exists();
        let bytes = novo_pdf::plate::export_plate_pdf(&docs, &name);
        match write_replacing(&path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message(Level::Info, format!("{verb} {}", path.display()));
                self.wb.last_export = Some(path.clone());
                Some(path)
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the plate drawing: {e}"),
                );
                None
            }
        }
    }

    /// The same plate drawing as DXF for a CNC or the panel builder's CAD:
    /// `<project> <enclosure> plate.dxf`, then `... plate 2.dxf` for the
    /// next sheet (the door). Returns the first file.
    pub(super) fn export_plate_dxf(&mut self) -> Option<PathBuf> {
        let (name, location, docs) = self.plate_sheets()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let dir = match file.as_ref().and_then(|f| f.parent()) {
            Some(dir) => dir.to_owned(),
            None => {
                let Some(dir) = projects_dir() else {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                };
                dir
            }
        };
        let mut first = None;
        for (n, doc) in docs.iter().enumerate() {
            let suffix = if n == 0 {
                String::new()
            } else {
                format!(" {}", n + 1)
            };
            let file_name =
                format!("{name} {location} plate{suffix}.dxf").replace(['/', '\\', ':'], "-");
            let path = dir.join(file_name);
            let text = novo_interop::dxf::write_dxf(&novo_interop::plate::plate_dxf(doc));
            match write_replacing(&path, text.as_bytes()) {
                Ok(()) => {
                    self.message(Level::Info, format!("Exported {}", path.display()));
                    self.wb.last_export = Some(path.clone());
                    first.get_or_insert(path);
                }
                Err(e) => {
                    self.message(Level::Error, format!("Could not export the plate DXF: {e}"));
                    return first;
                }
            }
        }
        first
    }
}
