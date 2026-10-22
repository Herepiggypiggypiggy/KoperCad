//! Compares Place all with and without the heat-aware rule on a project file,
//! using the 2D thermal model (planning values, not validated).
//!
//! ```text
//! cargo run --release -p novo-tests --example heat_place_all -- file.novo [--detail]
//! cargo run --release -p novo-tests --example heat_place_all -- --synthetic
//! cargo run --release -p novo-tests --example heat_place_all -- --save qa/fixtures/heat-place-all.novo
//! ```
//!
//! For every enclosure with a plate: take every device off the plate, delete
//! its rails and ducts, run Place all by the rules (once with `heat` off,
//! once on) and print devices over their limit, the tightest margin and the
//! hottest device air for each.

use novo_core::{Command, Document, Rail as ModelRail, RailKind, Structure, StructureId};
use novo_layout::model::{apply_plan, load, unplace_device};
use novo_layout::rules::{RuleSet, model::specs, place_all_explained};
use novo_thermal::bridge::{BridgeOptions, scenario_from_project};
use novo_thermal::{Options, simulate_steady_with};

struct Row {
    label: &'static str,
    placed: usize,
    unplaced: usize,
    over: usize,
    near: usize,
    mean_margin: f64,
    tightest: f64,
    hottest_air: f64,
    air_top: f64,
    converged: bool,
    /// Tag, loss W, margin K of every device over its limit.
    worst: Vec<(String, f64, f64)>,
}

fn run(doc: &Document, cab: StructureId, label: &'static str) -> Row {
    let bridge =
        scenario_from_project(doc.project(), cab, &BridgeOptions::default()).expect("scenario");
    let r = simulate_steady_with(
        &bridge.scenario,
        Options {
            bands: false,
            advice: false,
        },
    )
    .expect("solves");
    let margins: Vec<f64> = r.devices.iter().filter_map(|d| d.margin_k).collect();
    let mut worst: Vec<(String, f64, f64)> = r
        .devices
        .iter()
        .filter(|d| d.margin_k.is_some_and(|m| m < 0.0))
        .map(|d| {
            let tag = doc
                .project()
                .get::<novo_core::Device>(novo_core::DeviceId::from(d.id))
                .map_or_else(String::new, |x| x.tag.to_string());
            (tag, d.loss_w, d.margin_k.unwrap_or(0.0))
        })
        .collect();
    if std::env::args().any(|a| a == "--detail") {
        let mut all: Vec<_> = r.devices.iter().collect();
        all.sort_by(|a, b| b.loss_w.total_cmp(&a.loss_w));
        for d in all
            .iter()
            .filter(|d| d.loss_w >= 5.0 || d.rated_max_ambient_c.is_some_and(|l| l <= 45.0))
        {
            let tag = doc
                .project()
                .get::<novo_core::Device>(novo_core::DeviceId::from(d.id))
                .map_or_else(String::new, |x| x.tag.to_string());
            eprintln!(
                "      {tag:<14} x{:>5.0} y{:>5.0} {:>4.0}x{:<4.0} {:>5.1} W limit {:?} air {:.1} margin {:?}",
                d.footprint.x,
                d.footprint.y,
                d.footprint.w,
                d.footprint.h,
                d.loss_w,
                d.rated_max_ambient_c,
                d.air_c,
                d.margin_k.map(|m| (m * 10.0).round() / 10.0)
            );
        }
        eprintln!("      cabinet plate: {} rails", 0);
    }
    worst.sort_by(|a, b| a.2.total_cmp(&b.2));
    Row {
        worst,
        label,
        placed: r.devices.len(),
        unplaced: bridge.report.unplaced.len(),
        over: margins.iter().filter(|m| **m < 0.0).count(),
        near: margins.iter().filter(|m| (0.0..5.0).contains(*m)).count(),
        mean_margin: margins.iter().sum::<f64>() / margins.len().max(1) as f64,
        tightest: margins.iter().copied().fold(f64::INFINITY, f64::min),
        hottest_air: r.devices.iter().map(|d| d.air_c).fold(f64::MIN, f64::max),
        air_top: r.air_at_top_c,
        converged: r.converged,
    }
}

fn clear(doc: &mut Document, cab: StructureId) {
    let layout = load(doc.project(), cab).expect("plate");
    let rails: Vec<_> = doc
        .project()
        .all::<ModelRail>()
        .filter(|r| r.structure == cab)
        .map(|r| r.id)
        .collect();
    doc.transact("clear", |tx| {
        for p in &layout.placements {
            unplace_device(tx, p.id)?;
        }
        for rail in &rails {
            tx.execute(Command::DeleteRail { rail: *rail })?;
        }
        Ok(())
    })
    .expect("clear");
}

fn place(doc: &mut Document, cab: StructureId, rules: &RuleSet) {
    let layout = load(doc.project(), cab).expect("plate");
    let s = specs(doc.project(), cab);
    let started = std::time::Instant::now();
    let (plan, why) = place_all_explained(&layout.plate, &s, rules, 1, 0.0);
    eprintln!(
        "    place all took {:.2} s",
        started.elapsed().as_secs_f64()
    );
    if let Some(h) = &why.heat {
        eprintln!("    {}", h.message());
        eprintln!("    {h:?}");
    }
    doc.transact("place", |tx| apply_plan(tx, cab, &plan))
        .expect("apply");
}

fn main() {
    // `--save out.novo` writes the made-up cabinet (devices not yet placed).
    let args: Vec<String> = std::env::args().collect();
    if let Some(at) = args.iter().position(|a| a == "--save") {
        let out = args.get(at + 1).expect("a path after --save");
        let doc = novo_tests::heat_cabinet::heat_cabinet();
        novo_storage::ProjectFile::create(std::path::Path::new(out), doc.project()).expect("save");
        return;
    }
    let path = std::env::args()
        .nth(1)
        .expect("path of a .novo file, or --synthetic");
    let mut doc = if path == "--synthetic" {
        novo_tests::heat_cabinet::heat_cabinet()
    } else {
        let file = novo_storage::ProjectFile::open(&path).expect("open");
        Document::new(file.load().expect("load"))
    };
    let cabs: Vec<StructureId> = doc
        .project()
        .all::<Structure>()
        .filter(|s| s.enclosure && s.plate.is_some())
        .map(|s| s.id)
        .collect();
    let _ = RailKind::DinRail;
    println!(
        "| cabinet | run | placed | unplaced | over limit | within 5 K | tightest margin K | mean margin K | hottest device air C | air at top C | converged |"
    );
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for cab in cabs {
        let name = doc.project().get::<Structure>(cab).unwrap().tag.to_string();
        for (label, heat) in [("rules only", false), ("heat-aware", true)] {
            clear(&mut doc, cab);
            let mut rules = RuleSet::standard();
            set_heat(&mut rules, heat);
            place(&mut doc, cab, &rules);
            let row = run(&doc, cab, label);
            println!(
                "| {name} | {} | {} | {} | {} | {} | {:+.1} | {:+.1} | {:.1} | {:.1} | {} |",
                row.label,
                row.placed,
                row.unplaced,
                row.over,
                row.near,
                row.tightest,
                row.mean_margin,
                row.hottest_air,
                row.air_top,
                row.converged
            );
            for (tag, loss, margin) in &row.worst {
                eprintln!("    over: {tag} {loss:.0} W margin {margin:+.1} K");
            }
        }
    }
}

fn set_heat(rules: &mut RuleSet, on: bool) {
    if let Ok(zone) = std::env::var("HOT_ZONE") {
        rules.hot = Some(novo_layout::rules::HotRule {
            min_loss_w: 25.0,
            zone: novo_layout::rules::Zone::parse(&zone).expect("zone"),
        });
    }
    rules.heat = on.then(novo_layout::heat::HeatPlace::default);
}
