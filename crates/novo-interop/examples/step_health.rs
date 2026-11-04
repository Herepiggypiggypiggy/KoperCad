//! Prints the model health report of a STEP file:
//! `cargo run --release -p novo-interop --example step_health -- part.step`
use std::sync::atomic::AtomicBool;

use novo_interop::part3d::health::check_step;

fn main() {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: step_health FILE.step");
        std::process::exit(2);
    };
    let bytes = std::fs::read(&path).expect("read file");
    let h = check_step(&bytes, &AtomicBool::new(false));
    print!("{}", h.to_text());
    for s in &h.appearance.solids {
        println!(
            "solid {:?} colour {:?} faces {}",
            s.name,
            s.rgb.map(novo_interop::part3d::health::hex),
            s.faces
        );
    }
}
