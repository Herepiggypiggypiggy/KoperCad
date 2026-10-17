//! Photos on punch items. The pictures are files in `<project> field photos`
//! beside the project file, named by their content (novo-commissioning's
//! `photo_name`); the punch item keeps only the names, and a `.kcfield`
//! file carries the pictures with it.

use std::path::{Path, PathBuf};

use novo_commissioning::{FieldPackage, Photo, is_photo_name, photo_name};

use super::commissioning::now;
use super::pdf::write_replacing;
use super::{Level, NovoApp, projects_dir};

/// Pictures the dialog offers.
const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "bmp"];

impl NovoApp {
    /// Where this project's photos live: `<project> field photos` beside the
    /// project file, or in the Documents folder for a project with no file.
    pub(super) fn field_photos_dir(&self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let base = match file.as_deref().and_then(Path::parent) {
            Some(dir) => dir.to_owned(),
            None => projects_dir()?,
        };
        Some(base.join(format!("{name} field photos")))
    }

    /// Asks for picture files and attaches them to punch item `id`.
    pub(super) fn add_punch_photos(&mut self, id: u64) {
        let mut dialog = crate::dialogs::FileDialog::new()
            .set_title("Add photos to the punch item")
            .add_filter("Pictures", IMAGE_EXTENSIONS);
        if let Some(dir) = projects_dir() {
            dialog = dialog.set_directory(dir);
        }
        let Some(paths) = dialog.pick_files() else {
            return;
        };
        let added = self.attach_photo_files(id, &paths);
        if added > 0 {
            self.message(Level::Info, format!("Added {added} photo(s) to the item"));
        }
    }

    /// Copies each picture into the photos folder under its content name and
    /// records it on item `id`. Returns how many were new to the item.
    pub(super) fn attach_photo_files(&mut self, id: u64, paths: &[PathBuf]) -> usize {
        let Some(dir) = self.field_photos_dir() else {
            self.message(Level::Error, "Could not find the Documents folder");
            return 0;
        };
        self.load_commissioning();
        let mut added = 0;
        for path in paths {
            let bytes = match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(e) => {
                    self.message(
                        Level::Error,
                        format!("Could not read {}: {e}", path.display()),
                    );
                    continue;
                }
            };
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let name = photo_name(&bytes, ext);
            let target = dir.join(&name);
            if !target.exists()
                && let Err(e) = std::fs::create_dir_all(&dir)
                    .map_err(|e| e.to_string())
                    .and_then(|()| write_replacing(&target, &bytes).map_err(|e| e.to_string()))
            {
                self.message(Level::Error, format!("Could not keep the photo: {e}"));
                continue;
            }
            let Some(mode) = self.wb.commissioning.as_mut() else {
                return added;
            };
            let photo = Photo {
                name,
                at: now(),
                note: String::new(),
            };
            if mode.store.add_photo(id, photo).unwrap_or(false) {
                mode.dirty = true;
                added += 1;
            }
        }
        added
    }

    /// Packs the photos the punch items name into `package`.
    pub(super) fn pack_field_photos(&self, package: &mut FieldPackage) {
        let dir = self.field_photos_dir();
        package.attach_photos(|name| {
            if !is_photo_name(name) {
                return None;
            }
            std::fs::read(dir.as_ref()?.join(name)).ok()
        });
    }

    /// Writes the pictures of an imported package that are not here yet.
    pub(super) fn unpack_field_photos(&mut self, package: &FieldPackage) {
        let files = package.photo_files();
        if files.is_empty() {
            return;
        }
        let Some(dir) = self.field_photos_dir() else {
            return;
        };
        for (name, bytes) in files {
            let target = dir.join(&name);
            if target.exists() {
                continue;
            }
            if let Err(e) = std::fs::create_dir_all(&dir)
                .map_err(|e| e.to_string())
                .and_then(|()| write_replacing(&target, &bytes).map_err(|e| e.to_string()))
            {
                self.message(Level::Error, format!("Could not keep a photo: {e}"));
            }
        }
    }
}
