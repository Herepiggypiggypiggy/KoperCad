//! Assembly print pack: Reports > Assembly writes the shop-floor PDF of every
//! cabinet next to the project: the build order (`novo_layout::assembly`:
//! rails, ducts, devices, wires, labels), then the workshop sheet
//! (`novo_reports::AssemblyChecklist`: prepare with the cut list, drill table,
//! mount, wire, label, check), then the wire cutting list.
//!
//! Like the page PDF, a saved project's file goes next to it and takes its
//! name (`Plant 2.novo` gives `Plant 2 assembly.pdf`); a second export
//! replaces the first only while it is still exactly what we wrote.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use novo_core::{Device, DeviceId, Project};
use novo_layout::assembly::{WireJob, steps};
use novo_pdf::{ReportPdfOptions, export_report_pdf};
use novo_reports::{AssemblyChecklist, Cell, Column, CutList, Report, ReportOutput, Section};

use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

/// Row height in mm: room for a tick or a pencilled note.
const ROW_MM: f64 = 8.0;

/// The build order of every plated cabinet as one table each, keyed
/// `{tag}:order`. The wire steps come from the checklist's own wire tables,
/// so the order and the checklist never disagree about a wire.
pub fn order_sections(project: &Project, checklist: &ReportOutput) -> Vec<Section> {
    let mut out = Vec::new();
    for enclosure in project.structures().filter(|s| s.enclosure) {
        let Some(layout) = novo_layout::model::load(project, enclosure.id) else {
            continue;
        };
        let tag = enclosure.tag.to_string();
        let name = |id: u64| {
            project
                .get::<Device>(DeviceId::from(id))
                .map(|d| d.tag.to_string())
                .unwrap_or_else(|| format!("device {id}"))
        };
        let jobs: Vec<WireJob> = checklist
            .sections
            .iter()
            .find(|s| s.key == format!("{tag}:wire"))
            .map(|wire| {
                let at = |row: &novo_reports::Row, key: &str| {
                    wire.column(key)
                        .and_then(|i| row.cells.get(i))
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                };
                wire.rows
                    .iter()
                    .map(|row| WireJob {
                        name: at(row, "number"),
                        text: format!(
                            "Run {} to {}, cut {} mm",
                            at(row, "from"),
                            at(row, "to"),
                            at(row, "length_mm")
                        ),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mut section = Section::new(
            &format!("{tag}:order"),
            &format!("{tag}  Build order"),
            vec![
                Column::new("step", "Step", 8.0).right(),
                Column::new("phase", "Phase", 12.0),
                Column::new("mark", "Mark", 16.0),
                Column::new("do", "Do", 80.0),
                Column::new("after", "After", 14.0),
                Column::new("done", "Done", 7.0),
            ],
        );
        for step in steps(&layout, &name, &jobs) {
            section.push(
                vec![
                    Cell::Int(step.number as i64),
                    Cell::text(step.phase.name()),
                    Cell::text(step.mark),
                    Cell::text(step.text),
                    Cell::text(
                        step.after
                            .iter()
                            .map(|n| n.to_string())
                            .collect::<Vec<_>>()
                            .join(", "),
                    ),
                    Cell::text(novo_reports::ASSEMBLY_BOX),
                ],
                Vec::new(),
            );
        }
        out.push(section);
    }
    out
}

/// The whole pack as one output: per cabinet its build order then its
/// checklist tables, then the wire cutting list.
pub fn print_pack(project: &Project, checklist: ReportOutput) -> ReportOutput {
    let mut order = order_sections(project, &checklist);
    let mut sections = Vec::new();
    let mut rest = checklist.sections.into_iter().peekable();
    while let Some(first) = rest.next() {
        let tag = first.key.split(':').next().unwrap_or_default().to_owned();
        if let Some(i) = order.iter().position(|o| o.key == format!("{tag}:order")) {
            sections.push(order.remove(i));
        }
        sections.push(first);
        while rest
            .peek()
            .is_some_and(|s| s.key.split(':').next() == Some(tag.as_str()))
        {
            sections.extend(rest.next());
        }
    }
    sections.extend(CutList::default().generate(project).sections);
    ReportOutput {
        report: "assembly-pack".into(),
        title: "Assembly print pack".into(),
        revision: checklist.revision,
        sections,
    }
}

impl NovoApp {
    /// Exports the checklist next to the project, or to the Documents
    /// folder for an untitled one.
    pub(super) fn export_assembly(&mut self) -> Option<PathBuf> {
        let name = self.project_name()?;
        let file = self.backend.file().filter(|_| !self.backend.is_untitled());
        let dir = match &file {
            Some(file) => file.parent()?.to_owned(),
            None => match projects_dir() {
                Some(dir) => dir,
                None => {
                    self.message(Level::Error, "Could not find the Documents folder");
                    return None;
                }
            },
        };
        let stem = match &file {
            Some(file) => file
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => Path::new(&super::project_file_name(&name))
                .with_extension("")
                .to_string_lossy()
                .into_owned(),
        };
        let path = replaceable_path(&dir.join(format!("{stem} assembly.pdf")));
        self.export_assembly_to(&path)
    }

    /// Exports to `path`. Without a cabinet that has a mounting plate there
    /// is nothing to build, so it says so and writes nothing.
    pub fn export_assembly_to(&mut self, path: &Path) -> Option<PathBuf> {
        let name = self.project_name().unwrap_or_default();
        let project = self.backend.document()?.project();
        let output = AssemblyChecklist.generate(project);
        if output.sections.iter().all(|s| s.key == "none") {
            self.message(
                Level::Warning,
                "No cabinet has a mounting plate yet. Create a cabinet and set its plate on the Cabinet canvas.",
            );
            return None;
        }
        let pack = print_pack(project, output);
        let options = ReportPdfOptions {
            title: format!("{name}  Assembly print pack"),
            row_mm: pack
                .sections
                .iter()
                .filter(|s| s.key.contains(':'))
                .map(|s| (s.key.clone(), ROW_MM))
                .collect::<BTreeMap<_, _>>(),
            language: self.report_translator(),
            ..ReportPdfOptions::default()
        };
        let bytes = export_report_pdf(&[&pack], &options);
        let replaced = path.exists();
        match write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {}", path.display()), &path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the assembly checklist: {e}"),
                );
                None
            }
        }
    }
}
