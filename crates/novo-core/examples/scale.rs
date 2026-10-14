//! Timings for the model at 10k symbols:
//! `cargo run --release -p novo-core --example scale`.

use std::time::{Duration, Instant};

use novo_core::*;

#[path = "../tests/common/scale_project.rs"]
mod scale_project;

fn time<R>(label: &str, f: impl FnOnce() -> R) -> R {
    let start = Instant::now();
    let result = f();
    println!("{label:<28} {:>10.2?}", start.elapsed());
    result
}

fn main() {
    let mut doc = Document::default();
    time("build 10k symbols (1 tx)", || {
        scale_project::build(&mut doc)
    });
    let symbol = doc.project().symbols().nth(5000).unwrap().id;

    let netlist = time("netlist build", || doc.project().netlist());
    println!("  {} nets", netlist.nets().len());

    // A single small edit, as the editor does on every drop.
    let mut total = Duration::ZERO;
    let rounds = 100;
    for i in 0..rounds {
        let start = Instant::now();
        doc.transact("Move", |tx| {
            tx.move_symbol(symbol, Placement::at(Point::new(f64::from(i), 0.0)))
        })
        .unwrap();
        total += start.elapsed();
    }
    println!(
        "{:<28} {:>10.2?}",
        "transact: move one symbol",
        total / rounds
    );
    time("undo one edit", || doc.undo().unwrap());

    let json = time("to_json", || doc.project().to_json());
    println!("  {:.1} MB", json.len() as f64 / 1e6);
    time("from_json", || Project::from_json(&json).unwrap());
}
