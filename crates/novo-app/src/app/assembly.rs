//! Assembly print pack: Reports > Assembly writes the shop-floor PDF of every
//! cabinet next to the project: the build order (`novo_layout::assembly`:
//! rails, ducts, devices, wires, labels), then the workshop sheet
//! (`novo_reports::AssemblyChecklist`: prepare with the cut list, drill table,
//! mount, wire, label, check), then the wire cutting list.
//!
//! Like the page PDF, a saved project's file goes next to it and takes its
//! name (`Plant 2.novo` gives `Plant 2 assembly.pdf`); a second export
//! replaces the first only while it is still exactly what we wrote.

use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};

use novo_core::{Device, DeviceId, Project};
use novo_layout::assembly::{Phase, WireJob, steps};
use novo_pdf::{ReportPdfOptions, export_report_pdf};
use novo_reports::{AssemblyChecklist, Cell, Column, CutList, Report, ReportOutput, Section};

use super::pdf::{replaceable_path, write_replacing};
use super::{Level, NovoApp, projects_dir};

/// Row height in mm: room for a tick or a pencilled note.
const ROW_MM: f64 = 8.0;

/// Which circuit a wire belongs to, for working through one at a time. A
/// model of the usual practice, not a reading of the schematic: green-yellow
/// is protective earth, 2.5 mm² and up is power (mains, motor), the rest is
/// control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Circuit {
    Power,
    Control,
    Earth,
}

impl Circuit {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Circuit::Power => "Power",
            Circuit::Control => "Control",
            Circuit::Earth => "PE",
        }
    }

    pub(crate) fn of(colour: &str, cross_section_mm2: &str) -> Circuit {
        if novo_checks::standards::Colour::parse(colour)
            == Some(novo_checks::standards::Colour::GreenYellow)
        {
            Circuit::Earth
        } else if cross_section_mm2
            .trim()
            .parse::<f64>()
            .is_ok_and(|mm2| mm2 >= 2.5)
        {
            Circuit::Power
        } else {
            Circuit::Control
        }
    }
}

/// The circuits of the wire jobs by wire number, each name's circuits in job
/// order, so a number used twice still gets its own circuit each time.
fn circuits_by_mark(jobs: &[(Circuit, WireJob)]) -> BTreeMap<String, VecDeque<Circuit>> {
    let mut out: BTreeMap<String, VecDeque<Circuit>> = BTreeMap::new();
    for (circuit, job) in jobs {
        out.entry(job.name.clone()).or_default().push_back(*circuit);
    }
    out
}

/// The circuit of the wire step marked `mark`.
fn circuit_of(by_mark: &mut BTreeMap<String, VecDeque<Circuit>>, mark: &str) -> &'static str {
    by_mark
        .get_mut(mark)
        .and_then(VecDeque::pop_front)
        .map_or("", Circuit::name)
}

/// Step numbers as a short list: runs of consecutive numbers become
/// `5-12`, so a wire that waits on every device does not print them all.
fn number_ranges(numbers: &[usize]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < numbers.len() {
        let mut j = i;
        while j + 1 < numbers.len() && numbers[j + 1] == numbers[j] + 1 {
            j += 1;
        }
        out.push(if j > i {
            format!("{}-{}", numbers[i], numbers[j])
        } else {
            numbers[i].to_string()
        });
        i = j + 1;
    }
    out.join(", ")
}

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
        let mut jobs: Vec<(Circuit, WireJob)> = checklist
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
                    .map(|row| {
                        (
                            Circuit::of(&at(row, "colour"), &at(row, "cross_section_mm2")),
                            WireJob {
                                name: at(row, "number"),
                                text: format!(
                                    "Run {} to {}, cut {} mm",
                                    at(row, "from"),
                                    at(row, "to"),
                                    at(row, "length_mm")
                                ),
                            },
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        // One circuit at a time: power, then control, then earth. The sort is
        // stable, so inside a circuit the wire list's order stays.
        jobs.sort_by_key(|(circuit, _)| *circuit);
        let mut circuits = circuits_by_mark(&jobs);
        let jobs: Vec<WireJob> = jobs.into_iter().map(|(_, job)| job).collect();
        let mut section = Section::new(
            &format!("{tag}:order"),
            &format!("{tag}  Build order"),
            vec![
                Column::new("step", "Step", 8.0).right(),
                Column::new("phase", "Phase", 12.0),
                Column::new("mark", "Mark", 16.0),
                Column::new("circuit", "Group (colour, size)", 18.0),
                Column::new("do", "Do", 64.0),
                Column::new("after", "After", 16.0),
                Column::new("done", "Done", 7.0),
            ],
        );
        for step in steps(&layout, &name, &jobs) {
            section.push(
                vec![
                    Cell::Int(step.number as i64),
                    Cell::text(step.phase.name()),
                    Cell::text(step.mark.clone()),
                    Cell::text(if step.phase == Phase::Wires {
                        circuit_of(&mut circuits, &step.mark)
                    } else {
                        ""
                    }),
                    Cell::text(step.text),
                    Cell::text(number_ranges(&step.after)),
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
                self.message_file(format!("{verb} {}", path.display()), path);
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

#[cfg(test)]
mod circuit_tests {
    use super::{Circuit, circuit_of, circuits_by_mark, number_ranges};
    use novo_layout::Plate;
    use novo_layout::assembly::{Phase, WireJob, steps};
    use novo_layout::state::CabinetLayout;

    #[test]
    fn the_group_sits_beside_the_right_wire_number() {
        let job = |name: &str| WireJob {
            name: name.into(),
            text: format!("Run {name}"),
        };
        // Wire list order mixes the groups; "7" is used for two wires.
        let mut jobs = vec![
            (Circuit::Earth, job("PE1")),
            (Circuit::Control, job("7")),
            (Circuit::Power, job("L1")),
            (Circuit::Power, job("7")),
            (Circuit::Control, job("3")),
        ];
        jobs.sort_by_key(|(circuit, _)| *circuit);
        let mut by_mark = circuits_by_mark(&jobs);
        let wires: Vec<WireJob> = jobs.iter().map(|(_, j)| j.clone()).collect();
        let layout = CabinetLayout::new(Plate {
            width: 400.0,
            height: 300.0,
            margin: 10.0,
        });
        let got: Vec<(String, &str)> = steps(&layout, &|id| format!("d{id}"), &wires)
            .into_iter()
            .filter(|s| s.phase == Phase::Wires)
            .map(|s| {
                let group = circuit_of(&mut by_mark, &s.mark);
                (s.mark, group)
            })
            .collect();
        let want = [
            ("L1", "Power"),
            ("7", "Power"),
            ("7", "Control"),
            ("3", "Control"),
            ("PE1", "PE"),
        ];
        assert_eq!(
            got,
            want.map(|(m, g)| (m.to_owned(), g)).to_vec(),
            "power first, control next, PE last, each beside its own wire"
        );
    }

    #[test]
    fn wires_sort_into_power_control_and_earth() {
        assert_eq!(Circuit::of("black", "2.5"), Circuit::Power);
        assert_eq!(Circuit::of("BK", "10"), Circuit::Power);
        assert_eq!(Circuit::of("blue", "0.75"), Circuit::Control);
        assert_eq!(Circuit::of("", ""), Circuit::Control);
        // Earth wins over size: a 10 mm² earth lead is still the PE circuit.
        assert_eq!(Circuit::of("GNYE", "10"), Circuit::Earth);
        assert_eq!(Circuit::of("green-yellow", "1.5"), Circuit::Earth);
        assert!(Circuit::Power < Circuit::Control && Circuit::Control < Circuit::Earth);
    }

    #[test]
    fn long_dependency_lists_print_as_ranges() {
        assert_eq!(number_ranges(&[]), "");
        assert_eq!(number_ranges(&[4]), "4");
        assert_eq!(number_ranges(&[5, 6, 7, 8, 9, 10, 11, 12]), "5-12");
        assert_eq!(number_ranges(&[1, 2, 4, 6, 7]), "1-2, 4, 6-7");
    }
}
