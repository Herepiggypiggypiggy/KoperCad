//! Writes the sample project of the PLC panel lesson (`assets/lessons/plc-panel`):
//!
//! ```text
//! cargo run -p novo-app --example plc_panel_lesson -- assets/lessons
//! ```
//!
//! `plc-panel/project.json` is a 800 x 1000 mm plate with a PLC, its 24 V supply, a drive, relays,
//! breakers and three terminal strips, nothing placed. `plc-panel/finished.json` is the same panel
//! after `Place all`. The sizes are typical published sizes of the part families, not checked
//! against a data sheet, and the part numbers are illustrative.

use std::path::PathBuf;

use novo_core::{DeviceTag, Document, MountingPlate, Project, Result, Structure, Tx, Value};
use novo_layout::model::{apply_plan, load};
use novo_layout::rules::{RuleSet, place_all_explained};

/// `(tag, maker, type, width, height, depth, power loss W)`.
type PartSpec = (&'static str, &'static str, &'static str, f64, f64, f64, f64);

const PARTS: &[PartSpec] = &[
    (
        "-Q1",
        "Siemens",
        "5SL6 330-7 (3-pole 30 A)",
        54.0,
        90.0,
        70.0,
        9.0,
    ),
    (
        "-F1",
        "Siemens",
        "5SL6 106-7 (1-pole 6 A)",
        18.0,
        90.0,
        70.0,
        1.5,
    ),
    (
        "-F2",
        "Siemens",
        "5SL6 106-7 (1-pole 6 A)",
        18.0,
        90.0,
        70.0,
        1.5,
    ),
    (
        "-F3",
        "Siemens",
        "5SL6 210-7 (2-pole 10 A)",
        36.0,
        90.0,
        70.0,
        2.5,
    ),
    (
        "-G1",
        "Siemens",
        "6EP1334-2BA20 (24 V 10 A)",
        70.0,
        125.0,
        120.0,
        18.0,
    ),
    (
        "-A1",
        "Siemens",
        "S7-1200 CPU 1214C",
        110.0,
        100.0,
        75.0,
        14.0,
    ),
    (
        "-A2",
        "Siemens",
        "SM 1223 (16 DI / 16 DO)",
        45.0,
        100.0,
        75.0,
        4.0,
    ),
    ("-A3", "Siemens", "SM 1231 (8 AI)", 45.0, 100.0, 75.0, 2.0),
    (
        "-U1",
        "Siemens",
        "V20 drive 2.2 kW",
        90.0,
        196.0,
        160.0,
        60.0,
    ),
    (
        "-K1",
        "Finder",
        "Relay 4-pole on 27 mm base",
        27.0,
        80.0,
        70.0,
        1.0,
    ),
    (
        "-K2",
        "Finder",
        "Relay 4-pole on 27 mm base",
        27.0,
        80.0,
        70.0,
        1.0,
    ),
    (
        "-K3",
        "Siemens",
        "3RT2026 contactor 11 kW",
        45.0,
        85.0,
        90.0,
        4.0,
    ),
    (
        "-X1",
        "Phoenix Contact",
        "UT 4 strip, 24 blocks (field power)",
        140.0,
        50.0,
        45.0,
        0.0,
    ),
    (
        "-X2",
        "Phoenix Contact",
        "UT 2.5 strip, 32 blocks (inputs)",
        170.0,
        50.0,
        45.0,
        0.0,
    ),
    (
        "-X3",
        "Phoenix Contact",
        "UT 2.5 strip, 32 blocks (outputs)",
        170.0,
        50.0,
        45.0,
        0.0,
    ),
];

fn tag(text: &str) -> DeviceTag {
    DeviceTag::parse(text).expect("valid tag")
}

fn base(doc: &mut Document) -> Result<()> {
    doc.transact("PLC panel lesson project", |tx: &mut Tx<'_>| {
        tx.set_info(|info| {
            info.name = "PLC panel (lesson)".into();
            info.description = "A PLC with its supply, a drive, relays and terminal strips on an \
                800 x 1000 mm plate. The sizes are typical published sizes, not checked against \
                data sheets."
                .into();
        })?;
        let cab = tx.add_structure(tag("+CAB"), "Control cabinet", true)?;
        tx.set_plate(
            cab,
            Some(MountingPlate {
                width: 800.0,
                height: 1000.0,
                margin: 10.0,
            }),
        )?;
        for (name, maker, kind, w, h, d, loss) in PARTS {
            let device = tx.add_device(tag(&format!("=A1+CAB{name}")), *kind)?;
            let part = tx.add_part(*maker, *kind)?;
            tx.assign_part(device, part)?;
            for (key, value) in [
                ("width_mm", *w),
                ("height_mm", *h),
                ("depth_mm", *d),
                ("power_loss_w", *loss),
            ] {
                tx.set_property(part, key, Some(Value::Float(value)))?;
            }
        }
        Ok(())
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

fn place(doc: &mut Document) -> Result<()> {
    let id = cab(doc.project());
    let layout = load(doc.project(), id).expect("plate");
    let specs = novo_layout::rules::model::specs(doc.project(), id);
    let (plan, _) = place_all_explained(&layout.plate, &specs, &RuleSet::standard(), 1, 0.0);
    if std::env::var_os("SHOW_PLAN").is_some() {
        println!(
            "{} rails, {} ducts, {} placed, {} unplaced",
            plan.rails.len(),
            plan.ducts.len(),
            plan.placements.len(),
            plan.unplaced.len()
        );
        for p in &plan.placements {
            println!("device {} at ({}, {}) rail {}", p.id, p.x, p.y, p.rail);
        }
    }
    doc.transact("Place all", |tx| apply_plan(tx, id, &plan))?;
    Ok(())
}

fn write(project: &Project, root: &std::path::Path, file: &str) {
    let path = root.join(file);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("create folder");
    std::fs::write(&path, project.to_json()).expect("write file");
    println!("{} ({} bytes)", path.display(), project.to_json().len());
}

fn main() -> Result<()> {
    let root = PathBuf::from(std::env::args().nth(1).expect("folder to write into"));
    let mut doc = Document::new(Project::new("PLC panel (lesson)"));
    base(&mut doc)?;
    write(doc.project(), &root, "plc-panel/project.json");
    place(&mut doc)?;
    write(doc.project(), &root, "plc-panel/finished.json");
    Ok(())
}
