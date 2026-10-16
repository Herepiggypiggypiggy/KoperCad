//! Writes the sample projects of the cabinet lessons (`assets/lessons/cabinet-layout`
//! and `wire-routing`) as the diffable JSON form:
//!
//! ```text
//! cargo run -p novo-app --example panel_lessons -- assets/lessons
//! ```
//!
//! Three states are the Gate 1 motor starter with a cabinet `+CAB`:
//! `cabinet-layout/project.json` has a 600 x 800 mm plate and nothing placed,
//! `cabinet-layout/finished.json` has everything placed on rails by `Place all`,
//! and `wire-routing/project.json` adds one duct down the right-hand end that joins the five ducts Place all lays. The parts
//! are illustrative catalogue entries, not a verified parts list.

use std::path::PathBuf;

use novo_core::{
    Command, Device, DeviceId, DeviceTag, Document, MountingPlate, Project, Result, Structure, Tx,
    Value,
};
use novo_layout::model::{apply_plan, load};
use novo_layout::rules::{RuleSet, place_all_explained};

/// `(device tag, width, height, depth, power loss W, extra part keys)`.
type PartSpec = (
    &'static str,
    f64,
    f64,
    f64,
    f64,
    &'static [(&'static str, &'static str)],
);

const PARTS: &[PartSpec] = &[
    ("-F1", 54.0, 90.0, 68.0, 6.0, &[]),
    ("-K1", 45.0, 85.0, 90.0, 3.0, &[]),
    ("-F2", 45.0, 75.0, 90.0, 4.0, &[]),
    ("-F3", 18.0, 90.0, 70.0, 1.5, &[]),
    ("-X1", 45.0, 50.0, 45.0, 0.0, &[]),
    (
        "-S0",
        30.0,
        30.0,
        50.0,
        0.0,
        &[("mounting", "door_cutout"), ("cutout", "round 22")],
    ),
    (
        "-S1",
        30.0,
        30.0,
        50.0,
        0.0,
        &[("mounting", "door_cutout"), ("cutout", "round 22")],
    ),
];

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("valid tag")
}

fn device_named(tx: &Tx<'_>, product: &str) -> DeviceId {
    tx.project()
        .devices()
        .find(|d| d.tag.to_string().ends_with(product))
        .map(|d| d.id)
        .unwrap_or_else(|| panic!("no device {product}"))
}

fn set(tx: &mut Tx<'_>, part: novo_core::PartId, key: &str, value: Value) -> Result<()> {
    tx.set_property(part, key, Some(value))
}

/// The starter, its cabinet, sized parts and a transformer on the plate.
fn base(doc: &mut Document) -> Result<()> {
    doc.transact("Cabinet lesson project", |tx| {
        novo_tests::motor_starter_in(tx)?;
        tx.set_info(|info| {
            info.name = "Motor starter cabinet (lesson)".into();
            info.description = "Direct-on-line starter in a 600 x 800 mm cabinet. The parts \
                are illustrative catalogue entries, not a verified parts list."
                .into();
        })?;
        let cab = tx.add_structure(tag("+CAB"), "Control cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 600.0,
                height: 800.0,
                margin: 10.0,
            }),
        )?;
        // A control transformer that screws to the plate: it makes heat, and
        // it gives the drill table a four-hole pattern.
        let t1 = tx.add_device(tag("=A1+CAB-T1"), "Control transformer 400 V / 24 V")?;
        let part = tx.add_part("Siemens", "4AV3101-2EB")?;
        tx.assign_part(t1, part)?;
        for (product, w, h, d, loss, extra) in PARTS {
            let id = device_named(tx, product);
            let part = tx
                .project()
                .get::<Device>(id)
                .and_then(|d| d.parts.first().copied());
            let part = match part {
                Some(p) => p,
                None => {
                    // The strip carries no part in the fixture; the cabinet needs a size.
                    let p = tx.add_part("Phoenix Contact", "3044076 (strip, 4 blocks)")?;
                    tx.assign_part(id, p)?;
                    p
                }
            };
            set(tx, part, "width_mm", Value::Float(*w))?;
            set(tx, part, "height_mm", Value::Float(*h))?;
            set(tx, part, "depth_mm", Value::Float(*d))?;
            set(tx, part, "power_loss_w", Value::Float(*loss))?;
            for (key, value) in *extra {
                set(tx, part, key, Value::Text((*value).into()))?;
            }
        }
        for (key, value) in [
            ("width_mm", 90.0),
            ("height_mm", 110.0),
            ("depth_mm", 85.0),
            ("power_loss_w", 20.0),
        ] {
            set(tx, part, key, Value::Float(value))?;
        }
        set(tx, part, "mounting", Value::Text("plate".into()))?;
        set(
            tx,
            part,
            "hole_pattern",
            Value::Text("10,10,6; 80,10,6; 10,100,6; 80,100,6".into()),
        )?;
        // Numbered wires, so the wire list and the labels have something to print.
        let wires: Vec<novo_core::WireId> = tx.project().wires().map(|w| w.id).collect();
        for (i, wire) in wires.into_iter().enumerate() {
            tx.execute(Command::SetWireNumber {
                wire,
                number: Some(format!("{}", i + 1)),
            })?;
        }
        Ok(())
    })?;
    Ok(())
}

fn generated_pages(doc: &mut Document) -> Result<()> {
    let specs = novo_reports::default_generated_pages(doc.project());
    doc.transact("Generated pages", |tx| {
        novo_reports::add_generated_pages(tx, &specs)
    })?;
    Ok(())
}

fn cab(project: &Project) -> novo_core::StructureId {
    project
        .structures()
        .find(|s: &&Structure| s.enclosure)
        .expect("cabinet")
        .id
}

/// What the app's Place all does: the standard rules, below what is placed.
fn place(doc: &mut Document) -> Result<()> {
    let id = cab(doc.project());
    let layout = load(doc.project(), id).expect("plate");
    let specs = novo_layout::rules::model::specs(doc.project(), id);
    let (plan, why) = place_all_explained(&layout.plate, &specs, &RuleSet::standard(), 1, 0.0);
    if std::env::var_os("SHOW_WIRES").is_some() {
        println!(
            "plan: {} rails, {} ducts, {} placed, {} unplaced, separation relaxed: {}",
            plan.rails.len(),
            plan.ducts.len(),
            plan.placements.len(),
            plan.unplaced.len(),
            why.separation_relaxed
        );
        for r in &plan.rails {
            println!("rail {} at ({}, {}) len {}", r.id, r.x, r.y, r.length);
        }
        for d in &plan.ducts {
            println!(
                "duct ({}, {}) len {} vertical {} {:?}",
                d.x, d.y, d.length, d.vertical, d.class
            );
        }
        for p in &plan.placements {
            println!("device {} at ({}, {}) rail {}", p.id, p.x, p.y, p.rail);
        }
    }
    doc.transact("Place all", |tx| apply_plan(tx, id, &plan))?;
    Ok(())
}

#[allow(dead_code)]
fn device_named_in(project: &Project, product: &str) -> DeviceId {
    project
        .devices()
        .find(|d| d.tag.to_string().ends_with(product))
        .map(|d| d.id)
        .unwrap_or_else(|| panic!("no device {product}"))
}

fn write(project: &Project, root: &std::path::Path, file: &str) {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create folder");
    std::fs::write(&path, project.to_json()).expect("write file");
    println!("{} ({} bytes)", path.display(), project.to_json().len());
}

/// Prints the wire list (set `SHOW_WIRES=1`), to see which lengths are routed.
fn show_wires(project: &Project, label: &str) {
    if std::env::var_os("SHOW_WIRES").is_none() {
        return;
    }
    use novo_reports::Report;
    let out = novo_reports::WireList.generate(project);
    println!("--- wire list, {label}");
    print!(
        "{}",
        novo_reports::to_csv(&out, &novo_reports::CsvOptions::default())
    );
    let id = cab(project);
    if let Some((_, table)) = novo_layout::model::plate_documents(project, id) {
        println!(
            "drill summary {:?}, {} cut-outs, problems {:?}",
            table.summary(),
            table.cutouts.len(),
            table.problems
        );
    }
    if let Some(layout) = load(project, id) {
        let r = layout.report();
        println!(
            "layout problems: {} collisions, {} outside, {} too close, {} rail overflows",
            r.collisions.len(),
            r.outside.len(),
            r.too_close.len(),
            r.rail_overflows.len()
        );
    }
}

fn main() -> Result<()> {
    let root = PathBuf::from(std::env::args().nth(1).expect("folder to write into"));
    let mut doc = Document::new(Project::new("Motor starter cabinet (lesson)"));
    base(&mut doc)?;
    generated_pages(&mut doc)?;
    show_wires(doc.project(), "nothing placed");
    write(doc.project(), &root, "cabinet-layout/project.json");
    place(&mut doc)?;
    show_wires(doc.project(), "after Place all");
    write(doc.project(), &root, "cabinet-layout/finished.json");
    // Place all now adds the joining duct itself (#612), so no extra step is needed.
    write(doc.project(), &root, "wire-routing/project.json");
    Ok(())
}
