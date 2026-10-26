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
    CardKind, OpeningJob, OpeningWall, Pack, PrintRecord, WireCut, WireGroup, add_opening_cards,
    add_outside_wire_cards, add_wiring_opening_cards, build,
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
/// Each entry: cut length, each end's allowance, and the cut list group's title (`1.5 mm2 black`).
type CutLengths = BTreeMap<(String, String, String), VecDeque<(f64, f64, String, String)>>;

fn cut_lengths(project: &Project) -> CutLengths {
    let mut out = CutLengths::new();
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
                out.entry(key).or_default().push_back((
                    cut,
                    ends,
                    s.title.clone(),
                    text_at(s, row, "cable"),
                ));
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

/// Which of several wires a row is, by wire number: a number runs on through
/// a terminal, so the same number on several rows is several wires, `(1, 2)`
/// and `(2, 2)`; a number used once is `None`.
pub(super) fn parts_of(numbers: &[String]) -> Vec<Option<(usize, usize)>> {
    let mut uses: BTreeMap<&str, usize> = BTreeMap::new();
    for n in numbers.iter().filter(|n| !n.trim().is_empty()) {
        *uses.entry(n).or_default() += 1;
    }
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    numbers
        .iter()
        .map(|n| {
            // An unnumbered wire is not "1 of N" of the other unnumbered ones.
            if n.trim().is_empty() {
                return None;
            }
            let nth = seen.entry(n).or_default();
            *nth += 1;
            (uses[n.as_str()] > 1).then_some((*nth, uses[n.as_str()]))
        })
        .collect()
}

/// Cover lines for outside wires whose ends name no cabinet. With several
/// cabinets they are filed under the first one's pack, which would otherwise
/// look like its own wiring, and the other packs never list them: so the first
/// says what they are and every other pack points to it. `unplaced` maps the
/// pack index they were filed under to their numbers; the result maps a pack
/// index to its lines. One cabinet gets none.
pub(super) fn unplaced_notes(
    unplaced: &BTreeMap<usize, Vec<String>>,
    cabinets: &[String],
) -> BTreeMap<usize, Vec<String>> {
    let mut out: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    if cabinets.len() < 2 {
        return out;
    }
    for (&at, numbers) in unplaced {
        let mut numbers = numbers.clone();
        numbers.sort();
        numbers.dedup();
        let list = numbers.join(", ");
        out.entry(at).or_default().push(format!(
            "Wires {list} name no cabinet in their ends, so they are listed on this pack only because it is the first. They belong to no one cabinet: whoever builds the other cabinets cuts them from this list."
        ));
        for i in (0..cabinets.len()).filter(|i| *i != at) {
            out.entry(i).or_default().push(format!(
                "Wires {list} name no cabinet in their ends: they are listed on the pack of {}, not here.",
                cabinets[at]
            ));
        }
    }
    out
}

/// A core of a cable is cut from the cable, which is bought and run once: say
/// so beside its size, so the bench does not look for hookup wire to cut.
const CORE_MARK: &str = " core ";

fn core_of(kind: String, cable: &str) -> String {
    if cable.is_empty() {
        kind
    } else {
        format!(
            "{kind}{CORE_MARK}{}",
            cable.split_whitespace().next().unwrap_or(cable)
        )
    }
}

/// The cover line for wires that are cores of a cable (one entry per core).
fn cores_note(mut cables: Vec<String>) -> Option<String> {
    let count = cables.len();
    cables.sort();
    cables.dedup();
    (count > 0).then(|| {
        format!(
            "{count} wires are cores of {}: the cable is bought and run once, not cut from hookup wire. Strip the sheath and fit the core ends; the length on these rows is the cable's length plus the ends.",
            cables.join(", ")
        )
    })
}

/// The work packs of every cabinet that has a mounting plate.
pub fn work_packs(project: &Project) -> Vec<Pack> {
    let checklist = AssemblyChecklist.generate(project);
    let mut cuts = cut_lengths(project);
    let mut jobs = strip_lengths(project);
    let label_stock = novo_reports::LabelSettings::of(project).stock.name();
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
        let mut cores: Vec<String> = Vec::new();
        let mut by_circuit: BTreeMap<Circuit, Vec<WireCut>> = BTreeMap::new();
        if let Some(wire) = checklist
            .sections
            .iter()
            .find(|s| s.key == format!("{tag}:wire"))
        {
            let numbers: Vec<String> = wire
                .rows
                .iter()
                .map(|r| text_at(wire, r, "number"))
                .collect();
            let mut parts = parts_of(&numbers).into_iter();
            for row in &wire.rows {
                let t = |key: &str| text_at(wire, row, key);
                let (number_text, from, to) = (t("number"), t("from"), t("to"));
                let part = parts.next().flatten();
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
                let cut = found.as_ref().map_or(length, |(cut, ..)| *cut);
                let end_mm = found.as_ref().map(|(_, end, ..)| *end);
                let cable = found.as_ref().map_or("", |f| f.3.as_str()).to_owned();
                let (colour, size) = (t("colour"), t("cross_section_mm2"));
                let fitting = fitting_of(end_mm, rules.ferrule_mm, rules.lug_mm);
                let strips = strip_of(&mut jobs, &number_text, &from, &to);
                let mut kind = [
                    (!size.is_empty()).then(|| format!("{size} mm2")),
                    (!colour.is_empty()).then(|| colour.clone()),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(" ");
                if size.is_empty() {
                    // A blank cell reads as a printing error at the bench; a
                    // colour alone hides that the size is missing.
                    kind = if kind.is_empty() {
                        "size not set".to_owned()
                    } else {
                        format!("size not set, {kind}")
                    };
                }
                kind = core_of(kind, &cable);
                cores.extend(cable.split_whitespace().next().map(str::to_owned));
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
                        part,
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
        pack.problems.extend(cores_note(cores));
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
        // The label card names the stock the labels are printed on, which is a
        // setting of the project (the default is a common A4 sheet).
        for card in pack.cards.iter_mut().filter(|c| c.kind == CardKind::Labels) {
            card.tools = vec![format!(
                "Labels printed on {label_stock} (Bench output > Labels tab sets the stock)"
            )];
            card.notes.push(
                "The label text is the device tag. Print at 100 %, no page scaling.".to_owned(),
            );
        }
        out.push(pack);
    }
    // Cut-list wires with no end in any plated cabinet (supply feed leads,
    // field wires) are on no card: say which, on the cover of the cabinet
    // their device is in, else of the first.
    let mut left: BTreeMap<usize, Vec<(String, WireCut)>> = BTreeMap::new();
    // Wires no cabinet tag names: filed under the first cabinet, and told so.
    let mut unplaced: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for ((number, from, to), lengths) in &cuts {
        for (cut, end, group, cable) in lengths {
            let own = out.iter().position(|p| {
                from.contains(&format!("{}-", p.cabinet)) || to.contains(&format!("{}-", p.cabinet))
            });
            let at = own.unwrap_or(0);
            if own.is_none() {
                unplaced.entry(at).or_default().push(number.clone());
            }
            let fitting = fitting_of(Some(*end), rules.ferrule_mm, rules.lug_mm);
            let strips = strip_of(&mut jobs, number, from, to);
            left.entry(at).or_default().push((
                group.clone(),
                WireCut {
                    number: number.clone(),
                    from: from.clone(),
                    to: to.clone(),
                    // The cut list's own words for no size, as the cabinet cards say it.
                    kind: core_of(
                        if group == "Size unknown" {
                            "size not set".to_owned()
                        } else {
                            group.clone()
                        },
                        cable,
                    ),
                    cut_mm: *cut,
                    // Outside the plate the duct model does not reach: cut long.
                    estimated: true,
                    end_mm: Some(*end),
                    fitting,
                    part: None,
                    strip_from_mm: strips.0,
                    strip_to_mm: strips.1,
                    strip_basis: strips.2,
                },
            ));
        }
    }
    let cabinets: Vec<String> = out.iter().map(|p| p.cabinet.clone()).collect();
    let notes = unplaced_notes(&unplaced, &cabinets);
    for (at, lines) in notes {
        if let Some(pack) = out.get_mut(at) {
            pack.problems.extend(lines);
        }
    }
    for (at, list) in left {
        if let Some(pack) = out.get_mut(at) {
            pack.problems.push(format!(
                "{} of {total_cut} cut-list wires have no end inside a plated cabinet (supply and field wires): they are on the \"Wires outside the plate\" cards at the end, not numbered steps. {}",
                list.len(),
                list.iter()
                    .map(|(_, w)| format!(
                        "{} ({} to {}) cut ~{} mm",
                        w.number,
                        w.from,
                        w.to,
                        novo_layout::drill::mm(w.cut_mm)
                    ))
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
            let mut wires: Vec<WireCut> = list.into_iter().map(|(_, w)| w).collect();
            let numbers: Vec<String> = wires.iter().map(|w| w.number.clone()).collect();
            for (w, part) in wires.iter_mut().zip(parts_of(&numbers)) {
                w.part = part;
            }
            let cables: Vec<String> = wires
                .iter()
                .filter_map(|w| w.kind.split(CORE_MARK).nth(1).map(str::to_owned))
                .collect();
            pack.problems.extend(cores_note(cables));
            add_outside_wire_cards(pack, wires);
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

/// The last work cards print of this session: what it was made from, the
/// project revision and file it was made for.
#[derive(Debug, Clone)]
pub(super) struct PrintState {
    pub(super) record: PrintRecord,
    pub(super) revision: u64,
    pub(super) file: Option<PathBuf>,
}

/// A card's (cabinet, mark) and its fingerprint.
type CardEntry = ((String, String), u64);

/// The stamp for a print of `packs` today, measured against `previous`.
fn stamp_for(packs: &[Pack], previous: Option<&PrintRecord>) -> novo_pdf::workpack::Stamp {
    novo_pdf::workpack::Stamp {
        printed: super::revisions::today(),
        since: previous.map(|r| r.date.clone()),
        changes: previous.map(|r| r.changes(packs)).unwrap_or_default(),
    }
}

impl NovoApp {
    /// The PDF with the revision, print date and CHANGED marks of the cards
    /// that differ from the last print of this session, and the record of
    /// this print for `remember_print`.
    pub(super) fn work_cards_stamped(&self) -> Option<(Vec<u8>, PrintRecord)> {
        let name = self.machine_title().unwrap_or_default();
        let project = self.machine()?;
        let packs = work_packs(&project);
        let stamp = stamp_for(&packs, self.print_record_for_project());
        let bytes = novo_pdf::workpack::export_workpack_pdf_stamped(&packs, &name, Some(&stamp))?;
        Some((bytes, PrintRecord::of(&packs, &stamp.printed)))
    }

    /// The last print's record, when it was made for the open project file.
    fn print_record_for_project(&self) -> Option<&PrintRecord> {
        let st = self.wb.workpack_print.as_ref()?;
        (st.file == self.project_path).then_some(&st.record)
    }

    pub(super) fn remember_print(&mut self, record: PrintRecord) {
        let revision = self
            .backend
            .document()
            .map_or(0, |doc| doc.project().revision());
        self.wb.workpack_print = Some(PrintState {
            record,
            revision,
            file: self.project_path.clone(),
        });
        self.wb.workpack_stale = None;
    }

    /// A sentence when the cards printed earlier no longer match the layout:
    /// how many cards changed. `None` before any print, while nothing
    /// differs, and when the cards cannot be built. Compared once per project
    /// revision, and only called while a panel that shows it is open.
    pub(super) fn workpack_stale_note(&mut self) -> Option<String> {
        let st = self.wb.workpack_print.as_ref()?;
        if st.file != self.project_path {
            return None;
        }
        let revision = self.backend.document()?.project().revision();
        if revision == st.revision {
            return None;
        }
        if let Some((r, note)) = &self.wb.workpack_stale
            && *r == revision
        {
            return note.clone();
        }
        let record = st.record.clone();
        let changes = {
            let project = self.machine()?;
            record.changes(&work_packs(&project))
        };
        let note = (!changes.is_empty()).then(|| {
            let n = changes.changed.len();
            format!(
                "Work cards out of date: {n} card{} changed since the print of {}{}. Print them again; the changed cards are marked CHANGED.",
                if n == 1 { "" } else { "s" },
                record.date,
                if changes.removed > 0 {
                    format!(", {} removed", changes.removed)
                } else {
                    String::new()
                }
            )
        });
        self.wb.workpack_stale = Some((revision, note.clone()));
        note
    }

    /// Whether `path` is the open project file: a card must never replace it.
    fn is_project_file(&self, path: &Path) -> bool {
        self.project_path
            .as_deref()
            .is_some_and(|p| crate::backend::same_file(p, path))
    }

    /// Says why a card file cannot go to `path`, when it would replace the
    /// open project.
    fn refuse_project_file(&mut self, path: &Path) -> bool {
        if !self.is_project_file(path) {
            return false;
        }
        self.message(
            Level::Error,
            format!(
                "{} is the open project file, so the work cards were not written. Choose another name for the PDF.",
                path.display()
            ),
        );
        true
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
        // Built once, before the dialog; without a plate it says what is
        // missing and opens no dialog for nothing.
        let (bytes, record) = self.work_cards_or_warn()?;
        let (dir, stem) = self.export_base()?;
        let path = Self::pick_pdf(
            "Save the work cards",
            &dir,
            &format!("{stem} work cards.pdf"),
        )?;
        self.write_workpack(&path, &bytes, record)
    }

    /// The same for one Build order step's card.
    pub(super) fn ask_workpack_step(
        &mut self,
        id: novo_core::StructureId,
        mark: &str,
    ) -> Option<PathBuf> {
        let (dir, stem) = self.export_base()?;
        let (bytes, tag, several, entry) = self.workpack_step_card(id, mark)?;
        let file = step_file_name(&stem, &tag, mark, several);
        let path = Self::pick_pdf("Save the work card", &dir, &file)?;
        let written = self.write_step_card(&path, &bytes);
        if written.is_some() {
            self.remember_card(entry);
        }
        written
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
        let (bytes, tag, several, entry) = self.workpack_step_card(id, mark)?;
        let file = step_file_name(&stem, &tag, mark, several);
        let path = super::pdf::replaceable_path(&dir.join(file));
        let written = self.write_step_card(&path, &bytes);
        if written.is_some() {
            self.remember_card(entry);
        }
        written
    }

    /// The one-page PDF of step `mark`, the enclosure's tag and whether the
    /// project has several enclosures; says so when there is no such card.
    fn workpack_step_card(
        &mut self,
        id: novo_core::StructureId,
        mark: &str,
    ) -> Option<(Vec<u8>, String, bool, CardEntry)> {
        let name = self.machine_title().unwrap_or_default();
        let (bytes, several) = {
            let project = self.machine()?;
            let tag = project.structures().find(|s| s.id == id)?.tag.to_string();
            let packs = work_packs(&project);
            let several = packs.len() > 1;
            let stamp = stamp_for(&packs, self.print_record_for_project());
            let bytes = packs
                .into_iter()
                .find(|p| p.cabinet == tag)
                .and_then(|pack| {
                    let index = pack.cards.iter().position(|c| c.mark == mark)?;
                    let entry = (
                        (tag.clone(), mark.to_owned()),
                        pack.cards[index].fingerprint(),
                    );
                    novo_pdf::workpack::export_card_pdf_stamped(&pack, index, &name, Some(&stamp))
                        .map(|b| (b, entry))
                });
            (bytes.map(|(b, entry)| (b, tag, entry)), several)
        };
        let Some((bytes, tag, entry)) = bytes else {
            self.message(
                Level::Warning,
                format!("No work card for {mark}: print the whole pack instead"),
            );
            return None;
        };
        Some((bytes, tag, several, entry))
    }

    /// A card printed on its own counts as printed: the pack is out of date
    /// only for the cards not yet printed again.
    fn remember_card(&mut self, ((cabinet, mark), fingerprint): CardEntry) {
        let file = self.project_path.clone();
        if let Some(st) = self.wb.workpack_print.as_mut()
            && st.file == file
        {
            st.record.cards.insert((cabinet, mark), fingerprint);
            self.wb.workpack_stale = None;
        }
    }

    pub(super) fn write_step_card(&mut self, path: &Path, bytes: &[u8]) -> Option<PathBuf> {
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
                self.message_file(
                    format!("{verb} {}. Print at 100 % (actual size)", path.display()),
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
        let (bytes, record) = self.work_cards_or_warn()?;
        self.write_workpack(path, &bytes, record)
    }

    /// The work cards PDF and the record of the print, or the warning that
    /// there is nothing to build yet.
    fn work_cards_or_warn(&mut self) -> Option<(Vec<u8>, PrintRecord)> {
        let bytes = self.work_cards_stamped();
        if bytes.is_none() {
            self.message(
                Level::Warning,
                "No cabinet has a mounting plate with something to build yet. Create a cabinet, set its plate and place devices on the Cabinet canvas.",
            );
        }
        bytes
    }

    fn write_workpack(
        &mut self,
        path: &Path,
        bytes: &[u8],
        record: PrintRecord,
    ) -> Option<PathBuf> {
        if self.refuse_project_file(path) {
            return None;
        }
        let replaced = path.exists();
        match write_replacing(path, bytes) {
            Ok(()) => {
                self.remember_print(record);
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
