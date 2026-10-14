//! Times the design checks on the Gate 0 stress page.
//!
//! `cargo run --release -p novo-checks --example checks_scale`

use std::time::Instant;

use novo_checks::{Summary, check, check_with};
use novo_core::{Document, Project};
use novo_tests::stress_page;

fn main() {
    // stress_page makes about a third of its objects symbols.
    let symbols = 10_000;
    let mut doc = Document::default();
    stress_page(&mut doc, symbols * 3).expect("stress page");
    println!(
        "{} symbols, {} wires",
        doc.project().symbols().count(),
        doc.project().wires().count()
    );
    println!("-- no device has a part (10k info problems):");
    measure(doc.project());

    doc.transact("Assign parts", |tx| {
        let part = tx.add_part("Acme", "NO-1")?;
        let devices: Vec<_> = tx.project().devices().map(|d| d.id).collect();
        for device in devices {
            tx.assign_part(device, part)?;
        }
        Ok(())
    })
    .expect("parts");
    println!("-- every device has a part:");
    measure(doc.project());
}

fn measure(project: &Project) {
    let runs = 10;
    let mut netlist_ms = Vec::new();
    let mut check_ms = Vec::new();
    let mut with_ms = Vec::new();
    let mut problems = Vec::new();
    for _ in 0..runs {
        let start = Instant::now();
        let netlist = project.netlist();
        netlist_ms.push(start.elapsed().as_secs_f64() * 1e3);
        let start = Instant::now();
        problems = check_with(project, &netlist);
        with_ms.push(start.elapsed().as_secs_f64() * 1e3);
        let start = Instant::now();
        let all = check(project);
        check_ms.push(start.elapsed().as_secs_f64() * 1e3);
        assert_eq!(all, problems);
    }
    let median = |v: &mut Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    println!(
        "netlist build:            {:7.2} ms",
        median(&mut netlist_ms)
    );
    println!("check_with (netlist kept): {:7.2} ms", median(&mut with_ms));
    println!(
        "check (builds netlist):    {:7.2} ms",
        median(&mut check_ms)
    );
    let summary = Summary::of(&problems);
    println!(
        "{} problems: {} ({} infos)",
        problems.len(),
        summary.pill_text(),
        summary.infos
    );
}
