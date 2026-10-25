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
use novo_layout::workpack::{
    OpeningJob, OpeningWall, Pack, WireCut, WireGroup, add_opening_cards, add_wiring_opening_cards,
    build,
};
use novo_reports::{AssemblyChecklist, Cell, CutList, Report, Section, is_estimate};

use novo_thermal::bridge::{BridgeOptions, scenario_from_project};

use super::assembly::Circuit;
use super::heat_openings_store;
use super::heat_settings::{Kind, Wall};
use super::pdf::write_replacing;
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
fn cut_lengths(project: &Project) -> BTreeMap<(String, String, String), VecDeque<(f64, f64)>> {
    let mut out: BTreeMap<(String, String, String), VecDeque<(f64, f64)>> = BTreeMap::new();
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
                // The cut list's "Ends mm" is both ends together.
                let ends = number(at(s, row, "allowance_mm").as_ref()).unwrap_or(0.0) / 2.0;
                out.entry(key).or_default().push_back((cut, ends));
            }
        }
    }
    out
}

/// The strip lengths of the wire-cut jobs by wire number, the same jobs and
/// ferrule standard as the bench output and the cut list (the part's or the
/// wire's stated length first, else the typical one for the size).
fn strip_lengths(project: &Project) -> BTreeMap<String, VecDeque<novo_reports::WireCutJob>> {
    let mut out: BTreeMap<String, VecDeque<novo_reports::WireCutJob>> = BTreeMap::new();
    let standard = novo_reports::FerruleStandard::of(project);
    for job in novo_reports::wire_cut_jobs_with(project, &standard) {
        out.entry(job.number.clone()).or_default().push_back(job);
    }
    out
}

/// The strip length at the `from` and `to` end of a card row and where it
/// came from, taken from the job with that number whose ends are those
/// two (in either order); consumed, so a number used twice gets its own.
fn strip_of(
    jobs: &mut BTreeMap<String, VecDeque<novo_reports::WireCutJob>>,
    number: &str,
    from: &str,
    to: &str,
) -> (Option<f64>, Option<f64>, &'static str) {
    use novo_reports::StripBasis;
    let Some(list) = jobs.get_mut(number) else {
        return (None, None, "");
    };
    let at = list.iter().position(|j| {
        (j.end_a.designation == from && j.end_b.designation == to)
            || (j.end_a.designation == to && j.end_b.designation == from)
    });
    let Some(job) = at.and_then(|i| list.remove(i)) else {
        return (None, None, "");
    };
    let (f, t) = if job.end_a.designation == from {
        (&job.end_a, &job.end_b)
    } else {
        (&job.end_b, &job.end_a)
    };
    // The ends can differ in basis (one end a part's length): say the less certain.
    let basis = if f.strip_mm.is_none() && t.strip_mm.is_none() {
        ""
    } else if [f.strip_basis, t.strip_basis].contains(&StripBasis::Typical) {
        "typical"
    } else if [f.strip_basis, t.strip_basis].contains(&StripBasis::Part) {
        "part"
    } else {
        "stated"
    };
    (f.strip_mm, t.strip_mm, basis)
}

/// The work packs of every cabinet that has a mounting plate.
pub fn work_packs(project: &Project) -> Vec<Pack> {
    let checklist = AssemblyChecklist.generate(project);
    let mut cuts = cut_lengths(project);
    let mut jobs = strip_lengths(project);
    let rules = novo_routing::project_rules::WireRules::of(project);
    let total_cut: usize = cuts.values().map(VecDeque::len).sum();
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
        let mut missing: Vec<String> = Vec::new();
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
                let found = cuts
                    .get_mut(&(number_text.clone(), from.clone(), to.clone()))
                    .and_then(VecDeque::pop_front);
                if found.is_none() {
                    missing.push(format!(
                        "Wire {number_text} ({from} to {to}) is not in the cut list: its length {} mm has no end allowances, cut it longer",
                        novo_layout::drill::mm(length)
                    ));
                }
                let cut = found.map_or(length, |(cut, _)| cut);
                let end_mm = found.map(|(_, end)| end);
                let (colour, size) = (t("colour"), t("cross_section_mm2"));
                let fitting = fitting_of(end_mm, rules.ferrule_mm, rules.lug_mm);
                let strips = strip_of(&mut jobs, &number_text, &from, &to);
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
                        estimated: found.is_none() || is_estimate(&t("basis")),
                        end_mm,
                        fitting,
                        strip_from_mm: strips.0,
                        strip_to_mm: strips.1,
                        strip_basis: strips.2,
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
        let mut pack = build(&tag, &layout, &features, &name, &groups);
        pack.problems.extend(missing);
        add_wiring_opening_cards(&mut pack, &layout);
        let stored = heat_openings_store::stored_text(project, enclosure.id);
        let vents = heat_openings_store::parse(&stored);
        if !vents.is_empty() {
            // The openings as the Heat panel and the solver cut them: moved and
            // trimmed to lie inside their wall, in the interior's millimetres.
            match scenario_from_project(project, enclosure.id, &BridgeOptions::default()) {
                Ok(built) => {
                    let e = built.scenario.enclosure;
                    let jobs: Vec<OpeningJob> = vents
                        .iter()
                        .map(|v| v.fitted(&e))
                        .enumerate()
                        .map(|(i, v)| OpeningJob {
                            name: format!("Opening {}", i + 1),
                            what: match v.kind {
                                Kind::Grille => "grille",
                                Kind::Fan => "fan",
                                Kind::FilterFan => "filter fan",
                            }
                            .to_owned(),
                            wall: match v.wall {
                                Wall::Door => OpeningWall::Door,
                                Wall::Top => OpeningWall::Top,
                                Wall::Bottom => OpeningWall::Bottom,
                                Wall::Left => OpeningWall::Left,
                                Wall::Right => OpeningWall::Right,
                            },
                            along: v.u_mm,
                            down: (v.wall == Wall::Door).then_some(v.v_mm),
                            width: v.width_mm,
                            height: v.height_mm,
                            size: (e.interior_width_mm(), e.interior_height_mm()),
                            depth: e.interior_depth_mm(),
                        })
                        .collect();
                    add_opening_cards(&mut pack, &jobs);
                }
                Err(_) => pack.problems.push(format!(
                    "{} grille or fan openings of the Heat panel are not on cards: the Heat view cannot build this cabinet (interior size or plate missing)",
                    vents.len()
                )),
            }
        }
        out.push(pack);
    }
    // Cut-list wires with no end in any plated cabinet (supply feed leads,
    // field wires) are on no card: say which, on the cover of the cabinet
    // their device is in, else of the first.
    let mut left: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for ((number, from, to), lengths) in &cuts {
        for cut in lengths {
            let at = out
                .iter()
                .position(|p| {
                    from.contains(&format!("{}-", p.cabinet))
                        || to.contains(&format!("{}-", p.cabinet))
                })
                .unwrap_or(0);
            left.entry(at).or_default().push(format!(
                "{number} ({from} to {to}) cut {} mm",
                novo_layout::drill::mm(cut.0)
            ));
        }
    }
    for (at, list) in left {
        if let Some(pack) = out.get_mut(at) {
            pack.problems.push(format!(
                "{} of {total_cut} cut-list wires are not on this cabinet's cards: they have no end inside a plated cabinet (supply and field wires). They are on the cut list: {}",
                list.len(),
                list.join("; ")
            ));
        }
    }
    out
}

/// `<project> work card <mark>.pdf`; with several cabinets the tag too, as two
/// cabinets can both have a rail R1.
pub(super) fn step_file_name(stem: &str, tag: &str, mark: &str, several: bool) -> String {
    use super::workshop::clean;
    if several {
        format!("{stem} work card {} {}.pdf", clean(tag), clean(mark))
    } else {
        format!("{stem} work card {}.pdf", clean(mark))
    }
}

/// Which fitting an end allowance is, from the cut list's own number: the
/// wire rules' lug allowance or their ferrule allowance. The cut list picks
/// by the largest size of a mixed chain, so the word follows what it used
/// instead of parsing the size again; equal allowances name neither.
pub(super) fn fitting_of(end_mm: Option<f64>, ferrule_mm: f64, lug_mm: f64) -> &'static str {
    const EPS: f64 = 1e-6;
    match end_mm {
        Some(e) if (lug_mm - ferrule_mm).abs() > EPS && (e - lug_mm).abs() < EPS => "lug",
        Some(e) if (lug_mm - ferrule_mm).abs() > EPS && (e - ferrule_mm).abs() < EPS => "ferrule",
        _ => "",
    }
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
    #[cfg(test)]
    pub(super) fn export_workpack(&mut self) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let path = super::pdf::replaceable_path(&dir.join(format!("{stem} work cards.pdf")));
        self.export_workpack_to(&path)
    }

    /// The Work cards commands (menu, Bench panel, Build order strip): the
    /// person picks where the PDF goes. The dialog starts next to the project
    /// (Documents for an untitled one) with the usual file name. Cancelling
    /// writes nothing.
    pub(super) fn ask_workpack(&mut self) -> Option<PathBuf> {
        if self.work_cards_pdf().is_none() {
            // Says what is missing, without a dialog for nothing.
            return self.export_workpack_to(Path::new(""));
        }
        let (dir, stem) = self.export_base()?;
        let path = Self::pick_pdf(
            "Save the work cards",
            &dir,
            &format!("{stem} work cards.pdf"),
        )?;
        self.export_workpack_to(&path)
    }

    /// The same for one Build order step's card.
    pub(super) fn ask_workpack_step(
        &mut self,
        id: novo_core::StructureId,
        mark: &str,
    ) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let (bytes, tag, several) = self.workpack_step_card(id, mark)?;
        let file = step_file_name(&stem, &tag, mark, several);
        let path = Self::pick_pdf("Save the work card", &dir, &file)?;
        self.write_step_card(&path, &bytes)
    }

    /// A PDF save dialog starting in `dir`; the name always ends in `.pdf`.
    fn pick_pdf(title: &str, dir: &Path, file: &str) -> Option<PathBuf> {
        let path = crate::dialogs::FileDialog::new()
            .set_title(title)
            .set_directory(dir)
            .set_file_name(file)
            .add_filter("PDF", &["pdf"])
            .save_file()?;
        Some(if path.extension().is_some() {
            path
        } else {
            path.with_extension("pdf")
        })
    }

    /// The card of the Build order step `mark` of enclosure `id` as a one-page
    /// PDF next to the project, `<project> work card <mark>.pdf`.
    #[cfg(test)]
    pub(super) fn export_workpack_step(
        &mut self,
        id: novo_core::StructureId,
        mark: &str,
    ) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let (bytes, tag, several) = self.workpack_step_card(id, mark)?;
        let file = step_file_name(&stem, &tag, mark, several);
        let path = super::pdf::replaceable_path(&dir.join(file));
        self.write_step_card(&path, &bytes)
    }

    /// The one-page PDF of step `mark`, the enclosure's tag and whether the
    /// project has several enclosures; says so when there is no such card.
    fn workpack_step_card(
        &mut self,
        id: novo_core::StructureId,
        mark: &str,
    ) -> Option<(Vec<u8>, String, bool)> {
        let name = self.machine_title().unwrap_or_default();
        let (bytes, several) = {
            let project = self.machine()?;
            let tag = project.structures().find(|s| s.id == id)?.tag.to_string();
            let packs = work_packs(&project);
            let several = packs.len() > 1;
            let bytes = packs
                .into_iter()
                .find(|p| p.cabinet == tag)
                .and_then(|pack| {
                    let index = pack.cards.iter().position(|c| c.mark == mark)?;
                    novo_pdf::workpack::export_card_pdf(&pack, index, &name)
                });
            (bytes.map(|b| (b, tag)), several)
        };
        let Some((bytes, tag)) = bytes else {
            self.message(
                Level::Warning,
                format!("No work card for {mark}: print the whole pack instead"),
            );
            return None;
        };
        Some((bytes, tag, several))
    }

    fn write_step_card(&mut self, path: &Path, bytes: &[u8]) -> Option<PathBuf> {
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.message_file(
                    format!("Exported {}. Print at 100 % (actual size)", path.display()),
                    path,
                );
                self.wb.last_export = Some(path.to_owned());
                Some(path.to_owned())
            }
            Err(e) => {
                self.message(Level::Error, format!("Could not export the work card: {e}"));
                None
            }
        }
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
