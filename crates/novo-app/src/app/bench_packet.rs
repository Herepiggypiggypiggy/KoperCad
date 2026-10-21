//! Bench packet: the printed bench output as one PDF in working order.
//!
//! Reports > Bench output > Bench packet (PDF), or as the first file of the
//! Workshop pack. One document with a cover (contents with page numbers,
//! what is exact and what is an estimate, which files to print at 100 %),
//! then the cabinet build order, the cut list, the ferrules, one tick sheet
//! per schematic page (devices to fit, wires, tests) and the terminal strip
//! plans. The parts are the existing reports; the cover and the circuit
//! sheets are `novo_reports::packet`.

use std::path::{Path, PathBuf};

use novo_reports::packet::{CoverLine, circuit_sheets, cover, cover_notes};
use novo_reports::{
    AssemblyChecklist, BenchCut, FerruleTable, Report, ReportOutput, TerminalDiagram,
    wire_cut_jobs_with,
};

use super::assembly::cabinet_sections;
use super::{Level, NovoApp};

/// File name the Workshop pack gives the packet.
pub(super) const PACKET_FILE: &str = "0 Bench packet.pdf";

/// What the numbers of each part are, as the cover prints them.
const DRAWING: &str = "Read from the drawing";
const ROUTED: &str =
    "Wires from the drawing; lengths routed along the ducts of the model, or ~ estimates";
const TABLE: &str = "Planning values you can edit";

/// Row height of the tick tables, mm.
const ROW_MM: f64 = 8.0;

/// The parts of a packet in order, each with its first section's place in
/// the whole.
struct Part {
    what: &'static str,
    print: &'static str,
    basis: &'static str,
    report: ReportOutput,
}

impl NovoApp {
    /// Writes the packet to `path`. `separate` are the files that go with it
    /// and are printed on their own (the 1:1 templates, the labels).
    pub fn export_bench_packet_to(
        &mut self,
        path: &Path,
        separate: &[CoverLine],
    ) -> Option<PathBuf> {
        let (bytes, what) = self.bench_packet_bytes(separate)?;
        let replaced = path.exists();
        match super::pdf::write_replacing(path, &bytes) {
            Ok(()) => {
                let verb = if replaced { "Replaced" } else { "Exported" };
                self.message_file(format!("{verb} {what}: {}", path.display()), path);
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(
                    Level::Error,
                    format!("Could not write {}: {e}", path.display()),
                );
                None
            }
        }
    }

    /// Reports > Bench output > Bench packet: next to the project.
    pub(super) fn export_bench_packet(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = dir.join(format!("{stem} bench packet.pdf"));
        let separate = [
            CoverLine::new(
                "Drilling templates, plate full size",
                "Workshop pack",
                "Print at 100 % (actual size)",
                "Hole positions from the cabinet layout; about 0.5 mm if the check square is 100 mm, else up to 0.2 % (not measured)",
            ),
            CoverLine::new(
                "Labels",
                "Bench output > Labels",
                "Print at 100 % on the label stock",
                "Texts from the drawing; sheet pitch from published data, never printed here",
            ),
        ];
        self.export_bench_packet_to(&path, &separate)
    }

    /// The packet's PDF and a short description of it, or `None` (with a
    /// message) when there is nothing to build or cut.
    fn bench_packet_bytes(&mut self, separate: &[CoverLine]) -> Option<(Vec<u8>, String)> {
        let inputs = self.bench_inputs()?;
        if !inputs.standard.is_valid() {
            self.message(
                Level::Warning,
                "The ferrule standard has a size twice or a value that is not a number above zero",
            );
            return None;
        }
        let name = self.machine_title().unwrap_or_default();
        let language = self.report_translator();
        let project = self.machine()?;
        let jobs = wire_cut_jobs_with(&project, &inputs.standard);

        let checklist = AssemblyChecklist.generate(&project);
        let build: Vec<_> = if checklist.sections.iter().all(|s| s.key == "none") {
            Vec::new()
        } else {
            // The circuit sheets list the wires, once and in circuit order.
            let revision = checklist.revision;
            let sections = cabinet_sections(&project, checklist)
                .into_iter()
                .filter(|s| !s.key.ends_with(":wire"))
                .collect();
            vec![ReportOutput {
                report: "assembly".into(),
                title: "Cabinet build".into(),
                revision,
                sections,
            }]
        };
        let mut parts: Vec<Part> = Vec::new();
        for report in build {
            parts.push(Part {
                what: "Cabinet build: prepare, drill, mount",
                print: "A4, any printer",
                basis: "Positions from the cabinet layout",
                report,
            });
        }
        if !jobs.is_empty() {
            parts.push(Part {
                what: "Cut list: what to pull, strip and cut",
                print: "A4, any printer",
                basis: ROUTED,
                report: BenchCut::build(&project, &jobs),
            });
            let ferrules = FerruleTable::build(&project, &jobs, &inputs.standard);
            if ferrules.section("ferrules").is_some() {
                parts.push(Part {
                    what: "Ferrules to crimp, with spares",
                    print: "A4, any printer",
                    basis: TABLE,
                    report: ferrules,
                });
            }
            let circuits = circuit_sheets(&project, &jobs);
            if !circuits.sections.is_empty() {
                parts.push(Part {
                    what: "Circuit sheets: devices, wires and tests per schematic page",
                    print: "A4, any printer",
                    basis: ROUTED,
                    report: circuits,
                });
            }
        }
        let strips = TerminalDiagram.generate(&project);
        if strips.sections.iter().any(|s| !s.rows.is_empty()) {
            parts.push(Part {
                what: "Terminal strip plans",
                print: "A4, any printer",
                basis: DRAWING,
                report: strips,
            });
        }
        if parts.is_empty() {
            self.message(
                Level::Warning,
                "Nothing to put in a bench packet yet: connect some wires or set a mounting plate in the Cabinet",
            );
            return None;
        }

        let make_cover = |pages: &[Option<usize>]| {
            let mut lines: Vec<CoverLine> = parts
                .iter()
                .zip(pages)
                .map(|(p, page)| {
                    CoverLine::new(
                        p.what,
                        &page.map_or("p. ?".to_owned(), |n| format!("p. {n}")),
                        p.print,
                        p.basis,
                    )
                })
                .collect();
            lines.extend(separate.iter().cloned());
            cover(&project, &name, &jobs, &lines, &cover_notes(&jobs))
        };
        let keys = |cover: &ReportOutput| {
            let mut all: Vec<String> = parts
                .iter()
                .flat_map(|p| p.report.sections.iter().map(|s| s.key.clone()))
                .filter(|k| k.contains(':'))
                .collect();
            all.extend(cover.sections.iter().map(|s| s.key.clone()).take(0));
            all
        };
        let options = |cover: &ReportOutput| novo_pdf::ReportPdfOptions {
            title: format!("{name}  Bench packet"),
            footer_right: format!("Revision {}", project.revision()),
            row_mm: keys(cover).into_iter().map(|k| (k, ROW_MM)).collect(),
            language: language.clone(),
            ..novo_pdf::ReportPdfOptions::default()
        };
        // Two passes: the cover's length does not depend on its page
        // numbers, so the outline of the first fixes them for the second.
        let mut pages: Vec<Option<usize>> = vec![None; parts.len()];
        let mut cover_out = make_cover(&pages);
        for _ in 0..2 {
            let all: Vec<&ReportOutput> = std::iter::once(&cover_out)
                .chain(parts.iter().map(|p| &p.report))
                .collect();
            let outline = novo_pdf::report_outline(&all, &options(&cover_out));
            let mut at = cover_out.sections.len();
            let mut next = Vec::new();
            for p in &parts {
                next.push(outline.get(at).map(|(_, page)| *page));
                at += p.report.sections.len();
            }
            if next == pages {
                break;
            }
            pages = next;
            cover_out = make_cover(&pages);
        }
        let all: Vec<&ReportOutput> = std::iter::once(&cover_out)
            .chain(parts.iter().map(|p| &p.report))
            .collect();
        let bytes = novo_pdf::export_report_pdf(&all, &options(&cover_out));
        let estimates = jobs
            .iter()
            .filter(|j| novo_reports::is_estimate(&j.basis))
            .count();
        let what = if estimates == 0 {
            "bench packet".to_owned()
        } else {
            format!("bench packet ({estimates} lengths are estimates)")
        };
        Some((bytes, what))
    }
}
