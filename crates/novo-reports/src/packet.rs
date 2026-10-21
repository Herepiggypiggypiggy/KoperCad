//! Bench packet: the pieces of printed bench output as one document in the
//! order a workshop uses them.
//!
//! 1. a cover that says what is in the packet, on which page, what is read
//!    from the drawing (exact) and what is a planning value (an estimate),
//!    and which files must be printed at 100 % on their own;
//! 2. [`circuit_sheets`]: one sheet per schematic page (one circuit to build
//!    at a time) with the devices to fit, every wire with its ends, size,
//!    colour, cut length and ferrules, and the tests to tick off;
//! 3. the existing bench tables (cut list, ferrules, build order, terminal
//!    strips) which the app places around them.
//!
//! Only the project and the wire jobs are read; every table is a function of
//! them.

use std::collections::BTreeMap;

use novo_core::{AnyId, Device, DeviceId, PageId, Project, SymbolRole, Wire, natural_cmp};

use crate::bench::{TICK_BOX, is_estimate};
use crate::table::{Cell, Column, ReportOutput, Section};
use crate::wirecut::WireCutJob;

/// Report id of the circuit sheets.
pub const CIRCUITS_ID: &str = "circuits";
/// Report id of the cover.
pub const COVER_ID: &str = "packet-cover";

/// The checks printed at the end of every circuit sheet.
const TESTS: [&str; 4] = [
    "Every clamp tightened and tugged",
    "Continuity of each wire above, end to end",
    "Wire numbers and ferrules compared with the schematic page",
    "No bare conductor outside the clamp",
];

/// Section key of a page's circuit sheet.
pub fn circuit_key(page: PageId) -> String {
    format!("circuit:{}", page.raw())
}

/// The page a job belongs to: the page of its first drawn wire.
fn page_of(project: &Project, job: &WireCutJob) -> Option<PageId> {
    job.wires
        .iter()
        .find_map(|w| project.get::<Wire>(*w))
        .map(|w| w.page)
}

/// A cut length as printed: estimates carry `~`.
fn length_text(job: &WireCutJob) -> String {
    let mm = crate::bench::number_text(job.cut_mm);
    if is_estimate(&job.basis) {
        format!("~{mm}")
    } else {
        mm
    }
}

/// The ferrules of both ends: one name when both ends take the same.
fn ferrule_text(job: &WireCutJob) -> String {
    match (&job.end_a.ferrule, &job.end_b.ferrule) {
        (Some(a), Some(b)) if a == b => a.clone(),
        (Some(a), Some(b)) => format!("{a} / {b}"),
        (Some(a), None) | (None, Some(a)) => format!("{a} / none"),
        (None, None) => "none".to_owned(),
    }
}

/// The part of a device as one line.
fn part_text(project: &Project, device: &Device) -> String {
    let parts: Vec<String> = device
        .parts
        .iter()
        .filter_map(|p| project.part(*p))
        .map(|p| {
            format!("{} {}", p.manufacturer, p.order_number)
                .trim()
                .to_owned()
        })
        .collect();
    if parts.is_empty() {
        "no part assigned".to_owned()
    } else {
        parts.join(" + ")
    }
}

/// The page a device is drawn on first (in page order), if it is drawn.
fn first_page_of(project: &Project, order: &[PageId], device: DeviceId) -> Option<PageId> {
    let on: Vec<PageId> = project
        .symbols_of_device(device)
        .iter()
        .map(|s| s.page)
        .collect();
    order.iter().copied().find(|p| on.contains(p))
}

/// One sheet per schematic page that has wires or devices to build, in page
/// order: first the devices to fit, then the wires, then the tests. A
/// device drawn on several pages is fitted on the first of them. The sheet
/// of a page without anything to build is left out.
pub fn circuit_sheets(project: &Project, jobs: &[WireCutJob]) -> ReportOutput {
    let pages = project.pages_in_order();
    let order: Vec<PageId> = pages.iter().map(|p| p.id).collect();
    let mut wires: BTreeMap<PageId, Vec<&WireCutJob>> = BTreeMap::new();
    for job in jobs {
        if let Some(page) = page_of(project, job) {
            wires.entry(page).or_default().push(job);
        }
    }
    let mut fit: BTreeMap<PageId, Vec<&Device>> = BTreeMap::new();
    for device in project.devices() {
        let drawn = project
            .symbols_of_device(device.id)
            .iter()
            .any(|s| matches!(s.role, SymbolRole::Device { .. }));
        if !drawn {
            continue;
        }
        if let Some(page) = first_page_of(project, &order, device.id) {
            fit.entry(page).or_default().push(device);
        }
    }
    let mut sections = Vec::new();
    for page in &pages {
        let list = wires.get(&page.id).map(Vec::as_slice).unwrap_or_default();
        let devices = fit.get(&page.id).map(Vec::as_slice).unwrap_or_default();
        if list.is_empty() && devices.is_empty() {
            continue;
        }
        let name = if page.title.is_empty() {
            format!("Page {}", page.name)
        } else {
            format!("Page {} {}", page.name, page.title)
        };
        let estimates = list.iter().filter(|j| is_estimate(&j.basis)).count();
        let mut title = format!(
            "{name} · {}, {}",
            if devices.len() == 1 {
                "1 device to fit".to_owned()
            } else {
                format!("{} devices to fit", devices.len())
            },
            crate::count_text(list.len(), "wires")
        );
        if estimates > 0 {
            title.push_str(&format!(" ({estimates} lengths are estimates ~)"));
        }
        let mut section = Section::new(
            &circuit_key(page.id),
            &title,
            vec![
                Column::new("done", "", 5.0),
                Column::new("what", "Step", 8.0),
                Column::new("item", "Tag / wire", 18.0),
                Column::new("from", "From / part", 38.0),
                Column::new("to", "To", 34.0),
                Column::new("mm2", "mm²", 7.0).right(),
                Column::new("colour", "Colour", 10.0),
                Column::new("cut", "Cut mm", 10.0).right(),
                Column::new("ferrule", "Ferrule", 16.0),
            ],
        );
        let mut devices: Vec<&&Device> = devices.iter().collect();
        devices.sort_by(|a, b| natural_cmp(&a.tag.to_string(), &b.tag.to_string()));
        for d in devices {
            section.push(
                vec![
                    Cell::text(TICK_BOX),
                    Cell::text("Fit"),
                    Cell::text(d.tag.to_string()),
                    Cell::text(part_text(project, d)),
                    Cell::text(d.description.clone()),
                ],
                vec![AnyId::from(d.id)],
            );
        }
        let mut list: Vec<&&WireCutJob> = list.iter().collect();
        list.sort_by(|a, b| natural_cmp(&a.number, &b.number));
        for j in list {
            section.push(
                vec![
                    Cell::text(TICK_BOX),
                    Cell::text("Wire"),
                    Cell::text(if j.number.is_empty() { "-" } else { &j.number }),
                    Cell::text(j.end_a.designation.clone()),
                    Cell::text(j.end_b.designation.clone()),
                    j.cross_section_mm2
                        .map_or_else(|| Cell::text("not set"), Cell::from),
                    Cell::text(if j.colour.is_empty() {
                        "not set"
                    } else {
                        &j.colour
                    }),
                    Cell::text(length_text(j)),
                    Cell::text(ferrule_text(j)),
                ],
                j.wires.iter().map(|w| AnyId::from(*w)).collect(),
            );
        }
        for test in TESTS {
            section.push(
                vec![
                    Cell::text(TICK_BOX),
                    Cell::text("Test"),
                    Cell::Empty,
                    Cell::text(test),
                ],
                Vec::new(),
            );
        }
        section.push(
            vec![
                Cell::Empty,
                Cell::text("Signed"),
                Cell::Empty,
                Cell::text("Name and date:"),
            ],
            Vec::new(),
        );
        sections.push(section);
    }
    ReportOutput {
        report: CIRCUITS_ID.into(),
        title: "Circuit sheets".into(),
        revision: project.revision(),
        sections,
    }
}

/// One line of the cover's contents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverLine {
    /// What it is (`Circuit sheets`).
    pub what: String,
    /// Where it is: a page of this packet (`p. 4`) or a file name.
    pub where_: String,
    /// How to print it.
    pub print: String,
    /// What the numbers on it are: read from the drawing, or planning values.
    pub basis: String,
}

impl CoverLine {
    pub fn new(what: &str, where_: &str, print: &str, basis: &str) -> CoverLine {
        CoverLine {
            what: what.to_owned(),
            where_: where_.to_owned(),
            print: print.to_owned(),
            basis: basis.to_owned(),
        }
    }
}

/// How the wire lengths were found, counted by basis, so the cover never
/// says more than the data does.
fn length_note(jobs: &[WireCutJob]) -> String {
    let count = |f: &dyn Fn(&str) -> bool| jobs.iter().filter(|j| f(&j.basis)).count();
    let routed = count(&|b| b == "routed");
    let flat = count(&|b| b == "routed, no depth");
    let cable = count(&|b| b == "cable length");
    let stated = count(&|b| b == "to entry + stated outside");
    let estimates = jobs.iter().filter(|j| is_estimate(&j.basis)).count();
    let mut parts: Vec<String> = Vec::new();
    if routed > 0 {
        parts.push(format!("{routed} routed along the ducts of the model"));
    }
    if flat > 0 {
        parts.push(format!(
            "{flat} routed without duct depth (flat: check the height)"
        ));
    }
    if cable > 0 {
        parts.push(format!("{cable} from a stated cable length"));
    }
    if stated > 0 {
        parts.push(format!(
            "{stated} with the outside part stated by the designer"
        ));
    }
    if estimates > 0 {
        parts.push(format!(
            "{estimates} {} (~, from the schematic distance: measure on the plate before you cut)",
            if estimates == 1 {
                "estimate"
            } else {
                "estimates"
            }
        ));
    }
    format!(
        "Wire lengths, {} in all: {}. Routed is as good as the cabinet layout; wiring done differently from the drawing needs your own margin.",
        jobs.len(),
        parts.join("; ")
    )
}

/// The honest notes of the packet, printed on the cover. `jobs` are the wire
/// jobs the packet was made from.
pub fn cover_notes(jobs: &[WireCutJob]) -> Vec<String> {
    let mut notes = vec![
        "Sheets marked \"print at 100 %\" are true size: print them at 100 % or actual size, never fit to page, and check the 100 mm ruler on each with a rule before you drill or cut.".to_owned(),
        "Hole and rail positions come from the cabinet layout; accuracy on paper is about 0.5 mm when the check square measures 100 mm and up to 0.2 % otherwise, and was not measured on a real printer.".to_owned(),
        "Ferrule colours and lengths are a default table you can edit (Reports > Bench output). Makers differ above 16 mm²; it is not a claim of compliance with a standard.".to_owned(),
        "The test lines are a workshop routine, not an inspection record: use your own checks for a handover or a standard.".to_owned(),
    ];
    if !jobs.is_empty() {
        notes.insert(0, length_note(jobs));
    }
    let no_size = jobs
        .iter()
        .filter(|j| j.cross_section_mm2.is_none())
        .count();
    if no_size > 0 {
        notes.insert(
            0,
            format!(
                "{no_size} of {} wires to cut have no cross-section: no ferrule is listed for them and they sit in one group. Set the size (Reports > Bench output > Cut list, or Autopilot), then print again.",
                jobs.len()
            ),
        );
    }
    notes
}

/// The cover: the facts of the packet, the contents with where to find each
/// part, and what to read before printing and building.
pub fn cover(
    project: &Project,
    title: &str,
    jobs: &[WireCutJob],
    contents: &[CoverLine],
    notes: &[String],
) -> ReportOutput {
    let mut facts = Section::new(
        "facts",
        &format!("{title} · bench packet"),
        vec![
            Column::new("item", "", 34.0),
            Column::new("value", "", 120.0),
        ],
    );
    let fact = |s: &mut Section, a: &str, b: String| {
        s.push(vec![Cell::text(a), Cell::text(b)], Vec::new());
    };
    // The issued revision (A, B, ...), not the edit counter of the model: a
    // workshop reads "Revision 3" as a drawing revision.
    fact(
        &mut facts,
        "Drawing revision",
        match project.latest_revision() {
            Some(r) => r.index.to_string(),
            None => "none issued yet".to_owned(),
        },
    );
    let on_cable = jobs.iter().filter(|j| j.cable.is_some()).count();
    let set = crate::WireSet::of(project);
    // One line each: the cover rows are one or two lines tall in the PDF.
    fact(
        &mut facts,
        "Wires to cut",
        format!(
            "{} ({} between pins + {})",
            jobs.len(),
            set.connections,
            crate::count_text(set.leads(), "supply and PE leads")
        ),
    );
    if on_cable > 0 {
        fact(
            &mut facts,
            "Cable cores",
            format!("{on_cable} of the wires to cut are cores of a cable"),
        );
    }
    fact(
        &mut facts,
        "Wires as drawn",
        format!("{} pieces on the sheets (supply feeds included)", set.drawn),
    );
    let drawn = project
        .devices()
        .filter(|d| {
            project
                .symbols_of_device(d.id)
                .iter()
                .any(|s| matches!(s.role, SymbolRole::Device { .. }))
        })
        .count();
    fact(&mut facts, "Devices to fit", drawn.to_string());
    let mut sheet_count = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for job in jobs {
        if let Some(p) = page_of(project, job)
            && seen.insert(p)
        {
            sheet_count += 1;
        }
    }
    fact(&mut facts, "Circuits with wires", sheet_count.to_string());

    let mut index = Section::new(
        "contents",
        "Contents",
        vec![
            Column::new("what", "What", 44.0),
            Column::new("where", "Where", 40.0),
            Column::new("print", "How to print", 52.0),
            Column::new("basis", "What the numbers are", 60.0),
        ],
    )
    .flowing();
    for line in contents {
        index.push(
            vec![
                Cell::text(line.what.clone()),
                Cell::text(line.where_.clone()),
                Cell::text(line.print.clone()),
                Cell::text(line.basis.clone()),
            ],
            Vec::new(),
        );
    }

    let mut read = Section::new(
        "read-first",
        "Read before you print",
        vec![Column::new("note", "Note", 196.0)],
    )
    .flowing();
    for note in notes {
        read.push(vec![Cell::text(note.clone())], Vec::new());
    }
    ReportOutput {
        report: COVER_ID.into(),
        title: "Bench packet".into(),
        revision: project.revision(),
        sections: vec![facts, index, read],
    }
}
