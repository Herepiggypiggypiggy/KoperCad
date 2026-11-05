//! The Wire-it list as a PDF: Reports > Export > Export the Wire-it list
//! (PDF), or Print list in the Wire-it strip of the 3D view. Every wire in
//! the bench order on landscape A4 with a box to tick; wires already run in
//! the project are printed crossed. The order is `novo_reports::wireit`, the
//! page is `novo_pdf::wireit`.

use std::path::{Path, PathBuf};

use super::NovoApp;
use super::flags_wiring::{JobRetry, job};
use super::pdf::write_replacing;

impl NovoApp {
    /// The list as PDF bytes, or a warning when the project has no wire to run.
    fn wire_it_pdf_or_warn(&mut self) -> Option<Vec<u8>> {
        let name = self.project_name()?;
        let bytes = {
            let project = self.backend.document()?.project();
            let plan = novo_reports::wire_it_plan(project);
            let done: Vec<bool> = plan
                .steps
                .iter()
                .map(|s| novo_reports::wire_it_done(project, s))
                .collect();
            novo_pdf::wireit::export_wireit_pdf(&plan, &done, &name)
        };
        if bytes.is_none() {
            self.wiring_job_flag(
                job::WIRE_IT,
                false,
                "No wires to run yet",
                "Draw wires between device pins in the schematic.",
                JobRetry::WireIt,
            );
        }
        bytes
    }

    /// Asks where to save the list and writes it. Cancelling writes nothing.
    pub(super) fn ask_wire_it(&mut self) -> Option<PathBuf> {
        let bytes = self.wire_it_pdf_or_warn()?;
        let (dir, stem) = self.export_base()?;
        let path = Self::pick_pdf(
            "Save the Wire-it list",
            &dir,
            &format!("{stem} wire-it list.pdf"),
        )?;
        self.write_wire_it(&path, &bytes)
    }

    /// The list next to the project, `<project> wire-it list.pdf`.
    #[cfg(test)]
    pub(super) fn export_wire_it(&mut self) -> Option<PathBuf> {
        let bytes = self.wire_it_pdf_or_warn()?;
        let (dir, stem) = self.export_base()?;
        let path = super::pdf::replaceable_path(&dir.join(format!("{stem} wire-it list.pdf")));
        self.write_wire_it(&path, &bytes)
    }

    fn write_wire_it(&mut self, path: &Path, bytes: &[u8]) -> Option<PathBuf> {
        if self.refuse_project_file(path) {
            return None;
        }
        let verb = if path.exists() {
            "Replaced"
        } else {
            "Exported"
        };
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.job_clear(job::WIRE_IT);
                self.message_file(format!("{verb} {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.wiring_job_flag(
                    job::WIRE_IT,
                    true,
                    "Wire-it list not exported",
                    &format!("{}: {e}", path.display()),
                    JobRetry::WireIt,
                );
                None
            }
        }
    }
}
