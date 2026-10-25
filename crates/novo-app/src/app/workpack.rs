//! Work cards: Reports > Export > Export work cards (PDF) writes the build
//! order of every plated cabinet as half-page cards for the hand workshop:
//! a cover with the tool list, a tick-off sheet, then one card per rail,
//! duct and device (tools, hole positions from the nearest edges, a template
//! at true size where it fits) and per group of wires (the cut list with
//! tick boxes). The cards come from `novo_layout::workpack`, the PDF from
//! `novo_pdf::workpack`; the wires are the checklist's wire rows with the
//! cut list's lengths, grouped as the build order groups them.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use novo_core::{Device, DeviceId, Project};
use novo_layout::workpack::{Pack, WireCut, WireGroup, build};
use novo_reports::{AssemblyChecklist, Cell, CutList, Report, Section, is_estimate};

use super::assembly::Circuit;
use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp};

fn number(cell: Option<&Cell>) -> Option<f64> {
    match cell? {
        Cell::Number(v) => Some(*v),
        Cell::Int(v) => Some(*v as f64),
        Cell::Text(t) => t.trim().parse().ok(),
        Cell::Empty => None,
    }
}

fn at(section: &Section, row: &novo_reports::Row, key: &str) -> Option<Cell> {
    section.column(key).and_then(|i| row.cells.get(i)).cloned()
}

fn text_at(section: &Section, row: &novo_reports::Row, key: &str) -> String {
    at(section, row, key)
        .map(|c| c.to_string())
        .unwrap_or_default()
}

/// The cut lengths of the cut list by (number, from, to), each key's lengths
/// in list order so a number used twice gets its own length each time.
fn cut_lengths(project: &Project) -> BTreeMap<(String, String, String), VecDeque<f64>> {
    let mut out: BTreeMap<(String, String, String), VecDeque<f64>> = BTreeMap::new();
    for s in CutList::default()
        .generate(project)
        .sections
        .iter()
        .filter(|s| s.key.starts_with("group-"))
    {
        for row in &s.rows {
            let key = (
                text_at(s, row, "number"),
                text_at(s, row, "from"),
                text_at(s, row, "to"),
            );
            if let Some(cut) = number(at(s, row, "cut_mm").as_ref()) {
                out.entry(key).or_default().push_back(cut);
            }
        }
    }
    out
}

/// The work packs of every cabinet that has a mounting plate.
pub fn work_packs(project: &Project) -> Vec<Pack> {
    let checklist = AssemblyChecklist.generate(project);
    let mut cuts = cut_lengths(project);
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
        let features: Vec<_> = layout
            .placements
            .iter()
            .filter_map(|p| project.get::<Device>(DeviceId::from(p.id)))
            .map(|d| novo_layout::model::features(project, d))
            .collect();
        let mut by_circuit: BTreeMap<Circuit, Vec<WireCut>> = BTreeMap::new();
        if let Some(wire) = checklist
            .sections
            .iter()
            .find(|s| s.key == format!("{tag}:wire"))
        {
            for row in &wire.rows {
                let t = |key: &str| text_at(wire, row, key);
                let (number_text, from, to) = (t("number"), t("from"), t("to"));
                let length = number(at(wire, row, "length_mm").as_ref()).unwrap_or(0.0);
                let cut = cuts
                    .get_mut(&(number_text.clone(), from.clone(), to.clone()))
                    .and_then(VecDeque::pop_front)
                    .unwrap_or(length);
                let (colour, size) = (t("colour"), t("cross_section_mm2"));
                let kind = [
                    (!size.is_empty()).then(|| format!("{size} mm2")),
                    (!colour.is_empty()).then(|| colour.clone()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
                by_circuit
                    .entry(Circuit::of(&colour, &size))
                    .or_default()
                    .push(WireCut {
                        number: number_text,
                        from,
                        to,
                        kind,
                        cut_mm: cut,
                        estimated: is_estimate(&t("basis")),
                    });
            }
        }
        let groups: Vec<WireGroup> = by_circuit
            .into_iter()
            .map(|(circuit, wires)| WireGroup {
                title: circuit.name().to_owned(),
                wires,
            })
            .collect();
        out.push(build(&tag, &layout, &features, &name, &groups));
    }
    out
}

impl NovoApp {
    /// The work cards PDF of the open project; `None` without a cabinet that
    /// has something to build.
    pub(super) fn work_cards_pdf(&self) -> Option<Vec<u8>> {
        let name = self.machine_title().unwrap_or_default();
        let project = self.machine()?;
        novo_pdf::workpack::export_workpack_pdf(&work_packs(&project), &name)
    }

    /// Exports the work cards next to the project, or to the Documents folder
    /// for an untitled one.
    pub(super) fn export_workpack(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = replaceable_path(&dir.join(format!("{stem} work cards.pdf")));
        self.export_workpack_to(&path)
    }

    /// Exports to `path`. Without a plated cabinet with something to build it
    /// says so and writes nothing.
    pub fn export_workpack_to(&mut self, path: &Path) -> Option<PathBuf> {
        let Some(bytes) = self.work_cards_pdf() else {
            self.message(
                Level::Warning,
                "No cabinet has a mounting plate with something to build yet. Create a cabinet, set its plate and place devices on the Cabinet canvas.",
            );
            return None;
        };
        let replaced = path.exists();
        match write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(
                    format!(
                        "{verb} {}. Print at 100 % (actual size), then cut each sheet in half",
                        path.display()
                    ),
                    path,
                );
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not export the work cards: {e}"),
                );
                None
            }
        }
    }
}
