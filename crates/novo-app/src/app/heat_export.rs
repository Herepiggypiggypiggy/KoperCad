//! Export of the heat and airflow result for the user's own ParaView:
//! `<project> heat.vtr` (a rectilinear grid, one value per solver cell), or
//! for a heat-up `<project> heat-up NNNN.vtr` per frame plus a
//! `<project> heat-up.pvd` that ParaView plays as an animation. Next to the
//! project file, or in `<Documents>\KoperCAD` for an unsaved project.
//!
//! Commands: `heat.export_vtk` writes the files; `heat.open_paraview` writes
//! them and starts a ParaView found in the user's own folders (no admin
//! needed), or says where to point KoperCAD at it. Nothing of ParaView is
//! bundled. Writer and frame rules: `novo_heatview::vtk`.

use std::path::{Path, PathBuf};

use novo_cfd::CfdSnapshot;
use novo_heatview::vtk::{self, VtkOptions};

use super::pdf::write_replacing;
use super::{Level, NovoApp, projects_dir};

/// Environment variable that names the ParaView program to start.
pub(super) const PARAVIEW_ENV: &str = "KOPERCAD_PARAVIEW";

/// What was written: the file ParaView opens first, and the script beside it.
#[derive(Debug, PartialEq)]
pub(super) struct Written {
    pub open: PathBuf,
    pub script: PathBuf,
    pub files: usize,
}

fn safe(name: &str) -> String {
    name.replace(['/', '\\', ':', '*', '?', '"', '<', '>', '|'], "-")
}

/// Writes one result, or the frames of a heat-up, into `dir`.
pub(super) fn write_files(
    dir: &Path,
    stem: &str,
    title: &str,
    run_date: &str,
    frames: &[(f64, &CfdSnapshot)],
    steady: &CfdSnapshot,
) -> Result<Written, String> {
    let stem = safe(stem);
    let opts = |time_s| VtkOptions {
        run_date: run_date.to_owned(),
        title: title.to_owned(),
        time_s,
        ..VtkOptions::default()
    };
    let put = |path: &Path, text: &str| {
        write_replacing(path, text.as_bytes())
            .map_err(|e| format!("could not write {}: {e}", path.display()))
    };
    let (open, files) = if frames.len() > 1 {
        let mut listed = Vec::new();
        for (n, (t, snap)) in frames.iter().enumerate() {
            let name = format!("{stem} heat-up {n:04}.vtr");
            let text = vtk::write_rectilinear(snap, &opts(Some(*t))).map_err(|e| e.to_string())?;
            put(&dir.join(&name), &text)?;
            listed.push((*t, name));
        }
        let pvd = dir.join(format!("{stem} heat-up.pvd"));
        put(&pvd, &vtk::write_pvd(&listed))?;
        (pvd, listed.len() + 1)
    } else {
        let path = dir.join(format!("{stem} heat.vtr"));
        let text = vtk::write_rectilinear(steady, &opts(frames.first().map(|f| f.0)))
            .map_err(|e| e.to_string())?;
        put(&path, &text)?;
        (path, 1)
    };
    let script = open.with_extension("py");
    put(&script, &vtk::paraview_script(&open.display().to_string()))?;
    Ok(Written {
        open,
        script,
        files: files + 1,
    })
}

/// The first ParaView program that exists, from the override variable, the
/// user's own folders, then `Program Files`.
pub(super) fn find_paraview() -> Option<PathBuf> {
    let var = |k: &str| std::env::var(k).ok();
    let list = |dir: &str| -> Vec<String> {
        std::fs::read_dir(dir)
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.file_name().to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    };
    vtk::paraview_candidates(
        var(PARAVIEW_ENV).as_deref(),
        var("LOCALAPPDATA").as_deref(),
        var("USERPROFILE").as_deref(),
        &list,
    )
    .into_iter()
    .find(|p| p.is_file())
}

impl NovoApp {
    /// The files go next to the project, or into Documents when unsaved.
    fn heat_export_dir(&mut self) -> Option<PathBuf> {
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        match file.as_ref().and_then(|f| f.parent()) {
            Some(dir) => Some(dir.to_owned()),
            None => {
                let dir = projects_dir();
                if dir.is_none() {
                    self.message(Level::Error, "Could not find the Documents folder");
                }
                dir
            }
        }
    }

    /// `heat.export_vtk`. Says why when there is no result to export.
    pub(super) fn export_heat_vtk(&mut self) -> Option<Written> {
        let heat = &self.cabinet.heat;
        let Some(shown) = heat.solution() else {
            self.message(
                Level::Warning,
                "Simulate the heat first, then export the result",
            );
            return None;
        };
        let frames: Vec<(f64, std::sync::Arc<novo_cfd::CfdSnapshot>)> = heat
            .time
            .frames
            .iter()
            .map(|f| (f.time_s, f.solution.snapshot.clone()))
            .collect();
        let dir = self.heat_export_dir()?;
        let stem = self
            .backend
            .file()
            .filter(|_| !self.backend.is_untitled())
            .and_then(|f| f.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "KoperCAD".to_owned());
        let refs: Vec<(f64, &CfdSnapshot)> = frames.iter().map(|(t, s)| (*t, &**s)).collect();
        let date = super::revisions::today();
        match write_files(&dir, &stem, &stem, &date, &refs, &shown.snapshot) {
            Ok(w) => {
                self.message_file(
                    format!(
                        "Exported the heat result for ParaView (a model, not validated): {}",
                        w.open.display()
                    ),
                    &w.open,
                );
                self.wb.last_export = Some(w.open.clone());
                Some(w)
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the heat result: {e}"),
                );
                None
            }
        }
    }

    /// `heat.open_paraview`: export, then start a ParaView found on this PC.
    pub(super) fn open_heat_in_paraview(&mut self) {
        let Some(w) = self.export_heat_vtk() else {
            return;
        };
        let Some(exe) = find_paraview() else {
            self.message(
                Level::Warning,
                format!(
                    "ParaView was not found. The file is written; open {} in ParaView, or set {PARAVIEW_ENV} to its paraview.exe",
                    w.open.display()
                ),
            );
            return;
        };
        match std::process::Command::new(&exe).arg(&w.open).spawn() {
            Ok(_) => self.message(Level::Info, format!("Opening in {}", exe.display())),
            Err(e) => self.message(
                Level::Error,
                format!("Could not start {}: {e}", exe.display()),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::heat_3d_tests::snapshot;

    #[test]
    fn a_steady_result_is_one_vtr_with_a_script_beside_it() {
        let dir = tempfile::tempdir().unwrap();
        let s = snapshot();
        let w = write_files(dir.path(), "Crane: K1", "K1", "2026-11-04", &[], &s).unwrap();
        assert_eq!(w.open, dir.path().join("Crane- K1 heat.vtr"));
        assert!(w.script.is_file());
        let text = std::fs::read_to_string(&w.open).unwrap();
        assert!(text.contains("<VTKFile type=\"RectilinearGrid\""));
        assert!(text.contains("NOT validated"));
        let py = std::fs::read_to_string(&w.script).unwrap();
        assert!(py.contains("heat.vtr"));
    }

    #[test]
    fn a_heat_up_is_a_frame_per_file_and_a_pvd_with_the_times() {
        let dir = tempfile::tempdir().unwrap();
        let s = snapshot();
        let w = write_files(
            dir.path(),
            "P",
            "P",
            "2026-11-04",
            &[(0.0, &s), (300.0, &s), (600.0, &s)],
            &s,
        )
        .unwrap();
        assert_eq!(w.open, dir.path().join("P heat-up.pvd"));
        let pvd = std::fs::read_to_string(&w.open).unwrap();
        assert!(pvd.contains("timestep=\"300\"") && pvd.contains("P heat-up 0002.vtr"));
        for n in 0..3 {
            assert!(dir.path().join(format!("P heat-up {n:04}.vtr")).is_file());
        }
        let last = std::fs::read_to_string(dir.path().join("P heat-up 0002.vtr")).unwrap();
        assert!(last.contains("Name=\"TimeSeconds\" NumberOfTuples=\"1\" format=\"ascii\">600<"));
    }

    #[test]
    fn export_without_a_result_says_so_and_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut d = crate::app::gate1_tests::Drive::new(&dir.path().join("untitled"));
        assert!(d.app.export_heat_vtk().is_none());
        assert!(crate::app::app_runs("heat.export_vtk"));
        assert!(crate::app::app_runs("heat.open_paraview"));
    }
}
