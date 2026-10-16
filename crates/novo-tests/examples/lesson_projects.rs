//! Writes the sample projects of the productivity lessons.
//!
//! ```
//! cargo run -p novo-tests --example lesson_projects
//! ```
//!
//! Each lesson folder under `assets/lessons/` gets `project.json` (the start
//! state) and `finished.json` (the same drawing when the lesson is done), in
//! the diffable form `kopercad-cli json` writes. The drawings are the Gate 1
//! motor starter (`novo_tests::motor_starter`) with a few edits, so they
//! cannot drift from what the tests draw. Parts are illustrative catalogue
//! entries, not a verified parts list. Run it again when the project JSON
//! format moves.

use std::path::{Path, PathBuf};

use novo_core::{
    AnyId, Device, DeviceTag, Document, Endpoint, Pin, Placement, Point, Project, Result,
    SymbolRole, Tx, WireId,
};
use novo_tests::motor_starter::{MotorStarter, motor_starter};

type Build = fn(&mut Document, &MotorStarter) -> Result<()>;

fn main() {
    let root = lessons_dir();
    let lessons: [(&str, &str, Build, Option<Build>); 8] = [
        (
            "design-checks",
            "Design checks",
            checks_start,
            Some(checks_end),
        ),
        ("blocks-and-macros", "Macros", macro_start, Some(macro_end)),
        (
            "parts-library",
            "Parts library",
            parts_start,
            Some(parts_end),
        ),
        (
            "spreadsheets",
            "Spreadsheets",
            sheets_start,
            Some(sheets_end),
        ),
        (
            "revisions-and-compare",
            "Revisions",
            revisions_start,
            Some(revisions_end),
        ),
        ("dxf-and-backgrounds", "DXF", plain_start, None),
        ("commissioning", "Commissioning", plain_start, None),
        ("time-savers", "Settings", plain_start, Some(settings_end)),
    ];
    for (id, name, start, end) in lessons {
        let files = [("project.json", Some(start)), ("finished.json", end)];
        for (file, build) in files {
            let Some(build) = build else { continue };
            let mut doc = Document::default();
            let starter = if id == "blocks-and-macros" {
                None
            } else {
                Some(motor_starter(&mut doc).expect("the starter builds"))
            };
            let built = match &starter {
                Some(s) => build(&mut doc, s),
                None => macro_build(&mut doc, file == "finished.json"),
            };
            built.expect("the lesson project builds");
            doc.transact("Lesson info", |tx| {
                tx.set_info(|i| {
                    i.name = format!("Lesson: {name}");
                    i.description = format!(
                        "Sample project for the lesson {id}. The parts are illustrative \
                         catalogue entries, not a verified parts list."
                    );
                })
            })
            .expect("info");
            let path = root.join(id).join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, doc.project().to_json()).unwrap();
            let again = Project::from_json(&std::fs::read_to_string(&path).unwrap());
            assert!(again.is_ok(), "{} does not load back", path.display());
            println!("{}", path.display());
        }
    }
}

fn lessons_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons")
}

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("tag")
}

fn wire_between(project: &Project, a: novo_core::SymbolId, b: novo_core::SymbolId) -> WireId {
    project
        .wires()
        .find(|w| {
            let sym = |e: &Endpoint| match e {
                Endpoint::Pin { symbol, .. } => Some(*symbol),
                _ => None,
            };
            let ends = (sym(&w.from), sym(&w.to));
            ends == (Some(a), Some(b)) || ends == (Some(b), Some(a))
        })
        .map(|w| w.id)
        .expect("the wire exists")
}

fn drop_part(tx: &mut Tx<'_>, short: &str, project: &Project) -> Result<()> {
    let id = [
        tag(&format!("=A1+CAB-{short}")),
        tag(&format!("=A1+FLD-{short}")),
    ]
    .into_iter()
    .flat_map(|t| project.devices_by_tag(&t))
    .next()
    .expect("device");
    tx.update::<Device>(id, |d| d.parts.clear())
}

fn add_part(tx: &mut Tx<'_>, device: novo_core::DeviceId, maker: &str, order: &str) -> Result<()> {
    let part = tx.add_part(maker, order)?;
    tx.assign_part(device, part)
}

// ----- design-checks -----------------------------------------------------

/// A wire between -F3 and -S0 is missing and -S1 has no part: two problems
/// to find with F8, fix, and one to waive.
fn checks_start(doc: &mut Document, s: &MotorStarter) -> Result<()> {
    doc.transact("Lesson start", |tx| {
        let wire = wire_between(tx.project(), s.f3_sym, s.s0_sym);
        tx.remove(wire)?;
        let p = tx.project().clone();
        drop_part(tx, "S1", &p)
    })?;
    Ok(())
}

fn checks_end(doc: &mut Document, s: &MotorStarter) -> Result<()> {
    checks_start(doc, s)?;
    doc.transact("Lesson end", |tx| {
        tx.connect(Endpoint::pin(s.f3_sym, 1), Endpoint::pin(s.s0_sym, 0))?;
        add_part(tx, s.s1, "Siemens", "3SU1100-0AB40-1BA0")
    })?;
    Ok(())
}

// ----- blocks-and-macros -------------------------------------------------

/// A page "Motor 1" with a breaker, a contactor and an overload relay wired
/// in a chain, and an empty page "Motor 2" to place the macro on. Built
/// without the Gate 1 starter: a macro cannot hold feeds or terminals.
fn macro_build(doc: &mut Document, finished: bool) -> Result<()> {
    let pins3 = |_: ()| -> Vec<Pin> {
        [
            ("1", 0.0, 0.0),
            ("2", 0.0, 16.0),
            ("3", 8.0, 0.0),
            ("4", 8.0, 16.0),
            ("5", 16.0, 0.0),
            ("6", 16.0, 16.0),
        ]
        .map(|(n, x, y)| Pin::new(n, x, y))
        .to_vec()
    };
    let ((second, items), _) = doc.transact("Lesson start", |tx| {
        let first = tx.add_page("1")?;
        let second = tx.add_page("2")?;
        tx.update::<novo_core::Page>(first, |p| p.title = "Motor 1".into())?;
        tx.update::<novo_core::Page>(second, |p| p.title = "Motor 2".into())?;
        let mut syms = Vec::new();
        let mut devs = Vec::new();
        for (i, (text, desc, key, main)) in [
            (
                "=A1+CAB-Q1",
                "Motor protection breaker",
                "iec/protection/fuse_3p",
                true,
            ),
            (
                "=A1+CAB-K1",
                "Motor contactor",
                "iec/contactors/main_3p",
                false,
            ),
            (
                "=A1+CAB-F1",
                "Overload relay",
                "iec/protection/overload_relay",
                true,
            ),
        ]
        .into_iter()
        .enumerate()
        {
            let d = tx.add_device(tag(text), desc)?;
            let sym = tx.place_symbol(
                first,
                key,
                Placement::at(Point::new(40.0, 40.0 + 32.0 * i as f64)),
                pins3(()),
                SymbolRole::Device { device: d, main },
            )?;
            devs.push(d);
            syms.push(sym);
        }
        let three = vec![vec![0, 1], vec![2, 3], vec![4, 5]];
        for s in &syms {
            tx.update::<novo_core::SymbolInstance>(*s, |x| x.pass_through = three.clone())?;
        }
        for pair in syms.windows(2) {
            for pole in 0..3u16 {
                tx.connect(
                    Endpoint::pin(pair[0], pole * 2 + 1),
                    Endpoint::pin(pair[1], pole * 2),
                )?;
            }
        }
        // The wires between the picked symbols come along by themselves.
        let items: Vec<AnyId> = syms.iter().map(|s| AnyId::Symbol(*s)).collect();
        Ok((second, items))
    })?;
    if finished {
        doc.transact("Lesson end", |tx| {
            let def = tx.define_block("dol-starter", &items, Point::new(40.0, 40.0), Vec::new())?;
            tx.place_block(def, second, Point::new(40.0, 40.0), Vec::new())?;
            Ok(())
        })?;
    }
    Ok(())
}

fn macro_start(_: &mut Document, _: &MotorStarter) -> Result<()> {
    Ok(())
}

fn macro_end(_: &mut Document, _: &MotorStarter) -> Result<()> {
    Ok(())
}

// ----- parts-library -----------------------------------------------------

/// The contactor and the overload relay have no part yet.
fn parts_start(doc: &mut Document, _: &MotorStarter) -> Result<()> {
    doc.transact("Lesson start", |tx| {
        let p = tx.project().clone();
        drop_part(tx, "K1", &p)?;
        drop_part(tx, "F2", &p)
    })?;
    Ok(())
}

fn parts_end(doc: &mut Document, s: &MotorStarter) -> Result<()> {
    parts_start(doc, s)?;
    doc.transact("Lesson end", |tx| {
        add_part(tx, s.k1, "Siemens", "3RT2016-1AP01")?;
        add_part(tx, s.f2, "Siemens", "3RU2116-1FB0")
    })?;
    Ok(())
}

// ----- spreadsheets ------------------------------------------------------

const GENERIC: [(&str, &str); 7] = [
    ("F1", "Fuse"),
    ("K1", "Contactor"),
    ("F2", "Overload"),
    ("M1", "Motor"),
    ("F3", "Breaker"),
    ("S0", "Button"),
    ("S1", "Button"),
];

/// Generic descriptions, the kind a first draft has before the sheet.
fn sheets_start(doc: &mut Document, _: &MotorStarter) -> Result<()> {
    doc.transact("Lesson start", |tx| {
        for (short, text) in GENERIC {
            let id = device_in(tx.project(), short);
            tx.update::<Device>(id, |d| d.description = text.into())?;
        }
        Ok(())
    })?;
    Ok(())
}

fn sheets_end(doc: &mut Document, s: &MotorStarter) -> Result<()> {
    sheets_start(doc, s)?;
    let better = [
        ("F1", "Pump 1 main fuses"),
        ("K1", "Pump 1 contactor"),
        ("F2", "Pump 1 overload relay"),
        ("M1", "Pump 1 motor 2.2 kW"),
        ("F3", "Control circuit breaker"),
        ("S0", "Pump 1 stop"),
        ("S1", "Pump 1 start"),
    ];
    doc.transact("Lesson end", |tx| {
        for (short, text) in better {
            let id = device_in(tx.project(), short);
            tx.update::<Device>(id, |d| d.description = text.into())?;
        }
        Ok(())
    })?;
    Ok(())
}

fn device_in(project: &Project, short: &str) -> novo_core::DeviceId {
    [
        tag(&format!("=A1+CAB-{short}")),
        tag(&format!("=A1+FLD-{short}")),
    ]
    .into_iter()
    .flat_map(|t| project.devices_by_tag(&t))
    .next()
    .unwrap_or_else(|| panic!("no device {short}"))
}

// ----- revisions-and-compare ----------------------------------------------

/// No revision yet: the lesson issues A itself, because a lesson project
/// carries no kept copy of an earlier revision to compare with.
fn revisions_start(_: &mut Document, _: &MotorStarter) -> Result<()> {
    Ok(())
}

/// Revision A was issued, the description of -M1 changed and the control
/// breaker was renamed, then revision B was issued.
fn revisions_end(doc: &mut Document, s: &MotorStarter) -> Result<()> {
    doc.transact("Lesson A", |tx| {
        tx.issue_revision("A", "2026-10-01", "TV", "First issue")?;
        Ok(())
    })?;
    doc.transact("Lesson end", |tx| {
        tx.update::<Device>(s.m1, |d| d.description = "Pump motor 3 kW".into())?;
        tx.update::<Device>(s.f3, |d| {
            d.description = "Control circuit breaker 2 A".into()
        })?;
        tx.issue_revision("B", "2026-10-15", "TV", "Larger pump motor")?;
        Ok(())
    })?;
    Ok(())
}

// ----- plain and settings -------------------------------------------------

fn plain_start(_: &mut Document, _: &MotorStarter) -> Result<()> {
    Ok(())
}

/// The drawing standard is NFPA after the lesson.
fn settings_end(doc: &mut Document, _: &MotorStarter) -> Result<()> {
    doc.transact("Lesson end", |tx| {
        tx.set_info(|i| i.rules.standard = "nfpa".into())
    })?;
    Ok(())
}
