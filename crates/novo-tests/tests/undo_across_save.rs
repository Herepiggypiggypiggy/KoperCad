//! Project data safety: undo, redo and save in any order reach the file
//! exactly, for the kinds of edit listed below.
//!
//! `data_safety.rs` pins single steps. Here a seeded random run mixes edits
//! (place, move, remove a symbol, a wire or a device, add and delete pages,
//! junctions; removing a device cascades through its symbols and wires), undo, redo, saves and
//! autosaves. After every save the file equals the drawing; at the end a
//! crash (the session dropped, nothing closed) leaves the last save in the
//! file and the drawing as it stood in the recovery file. Undoing the whole
//! history and saving gives back the project the run started from (the id
//! counter stays where the edits left it).
//!
//! The basic mix is the list above. The full mix adds cables (proposals or a
//! hand-made cable with a core), parts (add and assign), issued revisions,
//! wire numbering runs, Autopilot plans and the generated pages. The same
//! runs also go over a 600-page project. Not covered: adding devices,
//! connecting pins, and property or wire data edits.

#[cfg(unix)]
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

use novo_core::*;
use novo_storage::{ProjectFile, Session};
use novo_tests::{BigSpec, big_project, motor_starter};

/// A small deterministic generator (xorshift), so a failure names its seed.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }

    fn pick<T: Copy>(&mut self, items: &[T]) -> Option<T> {
        (!items.is_empty()).then(|| items[self.below(items.len())])
    }
}

fn point(rng: &mut Rng) -> Point {
    Point::new(rng.below(300) as f64 * 4.0, rng.below(200) as f64 * 4.0)
}

/// The JSON without the id counter: ids are never handed out twice, so after
/// undoing everything the counter stays where the edits left it (on purpose,
/// so a redo or a pasted copy never collides with an id a file already used).
fn without_counter(json: &str) -> String {
    json.lines()
        .filter(|line| !line.trim_start().starts_with("\"next_id\":"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Which edits a run draws from.
#[derive(Clone, Copy, PartialEq)]
enum Mix {
    /// Symbols, wires, devices, pages and junctions.
    Basic,
    /// Also cables, parts, revisions, wire numbering, Autopilot and generated
    /// pages: the commands that touch many kinds of object in one step.
    Full,
}

/// The project a run starts from.
#[derive(Clone, Copy)]
enum Setup {
    Starter,
    /// 300 feeders: 600 pages.
    Big600,
}

fn start_document(setup: Setup) -> Document {
    let mut doc = Document::default();
    match setup {
        Setup::Starter => {
            motor_starter(&mut doc).unwrap();
        }
        Setup::Big600 => {
            big_project(&mut doc, BigSpec::LARGE).unwrap();
            assert!(doc.project().pages().count() >= 600);
        }
    }
    doc.clear_history();
    doc
}

/// How often each heavy edit changed the drawing, over all runs.
const HEAVY: [&str; 6] = [
    "cables",
    "parts",
    "revision",
    "numbering",
    "autopilot",
    "generated pages",
];
static APPLIED: [AtomicUsize; 6] = [const { AtomicUsize::new(0) }; 6];

/// One edit from the heavy kinds. `None` when it changed nothing.
fn heavy_edit(doc: &mut Document, rng: &mut Rng) -> Option<ChangeSet> {
    let kind = rng.below(HEAVY.len());
    let before = doc.project().revision();
    let result = match kind {
        0 => {
            // The proposals when there are any; otherwise a cable by hand
            // that carries a wire.
            let wires: Vec<WireId> = doc.project().wires().map(|w| w.id).collect();
            let wire = rng.pick(&wires);
            let tag = format!("=A1-W{}", 10 + rng.below(80));
            doc.transact("Add cable", |tx| {
                let proposals = novo_cables::propose::propose_cables(tx.project());
                if !proposals.is_empty() {
                    return novo_cables::propose::accept_proposals(tx, &proposals, None)
                        .map(|_| ());
                }
                let tag = DeviceTag::parse(&tag).expect("a valid tag");
                let cable = tx.add_cable(tag, vec![CableCore::new("1"), CableCore::new("2")])?;
                tx.assign_core(cable, 0, wire)
            })
        }
        1 => {
            let devices: Vec<DeviceId> = doc.project().devices().map(|d| d.id).collect();
            let device = rng.pick(&devices)?;
            let order = format!("PX{}", rng.below(50));
            doc.transact("Assign part", |tx| {
                let part = tx.add_part("Siemens", order)?;
                tx.assign_part(device, part)
            })
        }
        2 => {
            let index = format!("R{}", rng.below(6));
            doc.transact("Issue revision", |tx| {
                tx.issue_revision(index, "2026-10-21", "TV", "Random run")
                    .map(|_| ())
            })
        }
        3 => doc.transact("Number wires", |tx| {
            let plan = novo_numbering::plan(
                tx.project(),
                &novo_numbering::Scheme::default(),
                novo_numbering::Mode::Fill,
                &novo_numbering::Scope::Project,
            );
            plan.apply(tx).map(|_| ())
        }),
        4 => {
            let plan = novo_reports::autopilot::plan_for(
                doc.project(),
                &novo_checks::calc::tables::TableSet::builtin(),
            );
            doc.transact("Autopilot", |tx| plan.apply(tx, None).map(|_| ()))
        }
        _ => {
            let specs = novo_reports::default_generated_pages(doc.project());
            doc.transact("Generated pages", |tx| {
                novo_reports::add_generated_pages(tx, &specs)
            })
        }
    };
    let (_, changes) = result.ok()?;
    if doc.project().revision() == before {
        return None;
    }
    APPLIED[kind].fetch_add(1, Ordering::Relaxed);
    Some(changes)
}

/// One random edit. An edit the model refuses (the last page, a locked page)
/// changes nothing and is not an error here.
fn edit(doc: &mut Document, rng: &mut Rng, mix: Mix) -> Option<ChangeSet> {
    if mix == Mix::Full && rng.below(100) < 40 {
        return heavy_edit(doc, rng);
    }
    let project = doc.project();
    let pages: Vec<PageId> = project.pages().map(|p| p.id).collect();
    let symbols: Vec<SymbolId> = project.symbols().map(|s| s.id).collect();
    let devices: Vec<DeviceId> = project.devices().map(|d| d.id).collect();
    let wires: Vec<WireId> = project.wires().map(|w| w.id).collect();
    let page = rng.pick(&pages)?;
    let at = point(rng);
    let (label, result) = match rng.below(9) {
        0 | 1 => (
            "Place",
            doc.transact("Place", |tx| {
                tx.place_symbol(
                    page,
                    "iec/contacts/no",
                    Placement::at(at),
                    vec![Pin::new("1", 0.0, 0.0), Pin::new("2", 0.0, 8.0)],
                    SymbolRole::Unassigned,
                )
                .map(|_| ())
            }),
        ),
        2 | 3 => {
            let symbol = rng.pick(&symbols)?;
            (
                "Move",
                doc.transact("Move", |tx| tx.move_symbol(symbol, Placement::at(at))),
            )
        }
        4 => {
            let symbol = rng.pick(&symbols)?;
            (
                "Remove symbol",
                doc.transact("Remove symbol", |tx| tx.remove(symbol)),
            )
        }
        5 => {
            // A device takes its symbols and what hangs on them with it.
            let device = rng.pick(&devices)?;
            (
                "Remove device",
                doc.transact("Remove device", |tx| tx.remove(device)),
            )
        }
        6 => {
            let wire = rng.pick(&wires)?;
            (
                "Remove wire",
                doc.transact("Remove wire", |tx| tx.remove(wire)),
            )
        }
        7 => {
            let name = format!("P{}", rng.below(1000));
            if rng.below(3) == 0 && pages.len() > 1 {
                (
                    "Delete page",
                    doc.transact("Delete page", |tx| tx.delete_page(page)),
                )
            } else {
                (
                    "Add page",
                    doc.transact("Add page", |tx| tx.add_page(name).map(|_| ())),
                )
            }
        }
        _ => (
            "Add junction",
            doc.transact("Add junction", |tx| tx.add_junction(page, at).map(|_| ())),
        ),
    };
    let _ = label;
    result.ok().map(|(_, changes)| changes)
}

/// The file as the next start would read it while the session still holds the
/// lock: a copy of the bytes (Unix only; a locked file cannot be copied on
/// Windows, where the end of the run checks the file after closing it).
#[cfg(unix)]
fn file_json(path: &Path) -> String {
    let dir = tempfile::tempdir().unwrap();
    let copy = dir.path().join("copy.novo");
    std::fs::copy(path, &copy).unwrap();
    ProjectFile::read(&copy).unwrap().to_json()
}

fn run(seed: u64, steps: usize, setup: Setup, mix: Mix) -> usize {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("run.novo");
    let mut doc = start_document(setup);
    let mut session = Session::create(&path, doc.project()).unwrap();
    let mut saved = doc.project().to_json();
    let mut rng = Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let (mut edits, mut undos, mut redos, mut saves) = (0, 0, 0, 0);

    for _step in 0..steps {
        let what = rng.below(100);
        if what < 55 {
            if let Some(changes) = edit(&mut doc, &mut rng, mix) {
                session.record(&changes);
                edits += 1;
            }
        } else if what < 70 {
            if let Some(changes) = doc.undo().unwrap() {
                session.record(&changes);
                undos += 1;
            }
        } else if what < 78 {
            if let Some(changes) = doc.redo().unwrap() {
                session.record(&changes);
                redos += 1;
            }
        } else if what < 90 {
            session.save(doc.project()).unwrap();
            saved = doc.project().to_json();
            saves += 1;
            #[cfg(unix)]
            assert_eq!(
                file_json(&path),
                saved,
                "seed {seed}, step {_step}: the file after a save"
            );
        } else {
            session.autosave(doc.project()).unwrap();
        }
    }
    assert!(
        edits > 10 && undos > 3 && saves > 3,
        "seed {seed}: {edits} edits {undos} undos {redos} redos {saves} saves"
    );

    // A crash: the last autosave of the drawing as it stands, then nothing
    // is closed. The file holds the last save; the recovery file the drawing.
    session.autosave(doc.project()).unwrap();
    let now = doc.project().to_json();
    drop(session);
    let opened = Session::open(&path).unwrap();
    assert_eq!(
        opened.project.to_json(),
        saved,
        "seed {seed}: the file after the crash"
    );
    match opened.recovered {
        Some(recovered) => assert_eq!(
            recovered.project.to_json(),
            now,
            "seed {seed}: the recovered drawing"
        ),
        None => assert_eq!(now, saved, "seed {seed}: unsaved work was not offered back"),
    }
    // The drawing accepted back and saved: the file reads back the same.
    let mut session = opened.session;
    session.save(&opened.project).unwrap();
    session.close().unwrap();
    assert_eq!(
        ProjectFile::read(&path).unwrap().to_json(),
        saved,
        "seed {seed}"
    );
    redos
}

/// Undo to the bottom of the history and save: the file is the starting
/// project; redo to the top and save: the file is the final drawing.
fn undo_all_then_redo_all(seed: u64, steps: usize, mix: Mix) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("history.novo");
    let mut doc = start_document(Setup::Starter);
    let start = doc.project().to_json();
    let mut session = Session::create(&path, doc.project()).unwrap();
    let mut rng = Rng(seed.wrapping_mul(0xD1B5_4A32_D192_ED03) | 1);

    for step in 0..steps {
        if let Some(changes) = edit(&mut doc, &mut rng, mix) {
            session.record(&changes);
        }
        // Saves in between, so the undo below crosses many of them.
        if step % 17 == 16 {
            session.save(doc.project()).unwrap();
        }
    }
    let top = doc.project().to_json();
    let depth = doc.undo_depth();
    assert!(depth > 10, "seed {seed}: only {depth} undo steps");

    while let Some(changes) = doc.undo().unwrap() {
        session.record(&changes);
    }
    assert_eq!(
        without_counter(&doc.project().to_json()),
        without_counter(&start),
        "seed {seed}: undone to the start"
    );
    session.save(doc.project()).unwrap();
    session.close().unwrap();
    assert_eq!(
        without_counter(&ProjectFile::read(&path).unwrap().to_json()),
        without_counter(&start),
        "seed {seed}: the file after undoing everything and saving"
    );

    let mut session = Session::open(&path).unwrap().session;
    while let Some(changes) = doc.redo().unwrap() {
        session.record(&changes);
    }
    assert_eq!(
        doc.project().to_json(),
        top,
        "seed {seed}: redone to the top"
    );
    session.save(doc.project()).unwrap();
    session.close().unwrap();
    assert_eq!(
        ProjectFile::read(&path).unwrap().to_json(),
        top,
        "seed {seed}: the file after redoing everything and saving"
    );
}

#[test]
fn random_runs_of_edits_undo_redo_and_saves_reach_the_file_exactly() {
    let redos: usize = (1..=10)
        .map(|seed| run(seed, 160, Setup::Starter, Mix::Basic))
        .sum();
    assert!(redos > 0, "no redo happened across the seeds");
}

#[test]
fn undoing_everything_across_many_saves_and_redoing_it_again_is_exact() {
    for seed in 1..=10 {
        undo_all_then_redo_all(seed, 120, Mix::Basic);
    }
}

#[test]
fn runs_with_cables_parts_revisions_numbering_autopilot_and_generated_pages_reach_the_file_exactly()
{
    for seed in 1..=10 {
        run(seed, 160, Setup::Starter, Mix::Full);
    }
    for seed in 1..=10 {
        undo_all_then_redo_all(seed, 120, Mix::Full);
    }
    for (kind, applied) in HEAVY.iter().zip(&APPLIED) {
        assert!(
            applied.load(Ordering::Relaxed) > 0,
            "no run ever applied {kind}: the mix does not reach it"
        );
    }
}

#[test]
fn a_600_page_project_reaches_the_file_exactly_through_edits_undo_redo_and_saves() {
    for seed in 1..=2 {
        run(seed, 60, Setup::Big600, Mix::Full);
    }
}
