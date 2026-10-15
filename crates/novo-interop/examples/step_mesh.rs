//! Builds the levels of detail of a STEP file and prints what came out:
//! `cargo run --release -p novo-interop --example step_mesh -- part.step [W H D] [--obj]`
//! `--obj` writes `part.step.obj` (most detailed level) to look at it.

use std::sync::atomic::AtomicBool;
use std::time::Instant;

use novo_interop::part3d::decimate::{edge_health, weld, winding_conflicts};
use novo_interop::part3d::{MeshOrigin, Quality, geometry_from_step};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(path) = args.first() else {
        eprintln!("usage: step_mesh FILE.step [WIDTH HEIGHT DEPTH] [--obj]");
        std::process::exit(2);
    };
    let nums: Vec<f64> = args.iter().skip(1).filter_map(|a| a.parse().ok()).collect();
    let size = (nums.len() == 3).then(|| [nums[0], nums[1], nums[2]]);
    let bytes = std::fs::read(path).expect("read file");
    let t = Instant::now();
    let g = match geometry_from_step(&bytes, size, Quality::Normal, &AtomicBool::new(false), None) {
        Ok(g) => g,
        Err(e) => {
            println!("{e}");
            return;
        }
    };
    println!("{} bytes in {:?}", bytes.len(), t.elapsed());
    println!("size {:?} mm", g.size_mm());
    for (i, l) in g.lods.iter().enumerate() {
        println!(
            "level {i}: {} triangles, {} vertices, {} feature edges, {} KB on the GPU",
            l.triangle_count(),
            l.positions.len(),
            l.edges.len(),
            l.gpu_bytes() / 1024
        );
    }
    let m = g.detail();
    let tris: Vec<[u32; 3]> = m
        .indices
        .chunks_exact(3)
        .map(|c| [c[0], c[1], c[2]])
        .collect();
    let pos: Vec<[f64; 3]> = m.positions.iter().map(|p| p.map(f64::from)).collect();
    let (_, welded) = weld(&pos, &tris, 1e-4);
    let (open, non_manifold) = edge_health(&welded);
    println!(
        "level 0 health: {open} open edges, {non_manifold} non-manifold edges, {} winding conflicts",
        winding_conflicts(&welded)
    );
    if let MeshOrigin::Step {
        stats,
        orientation,
        before_simplify,
    } = g.origin
    {
        println!("{stats:?}\n{orientation:?}, {before_simplify} triangles before simplifying");
    }
    if args.iter().any(|a| a == "--obj") {
        let mut o = String::new();
        for p in &m.positions {
            o.push_str(&format!("v {} {} {}\n", p[0], p[1], p[2]));
        }
        for t in m.indices.chunks_exact(3) {
            o.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
        }
        std::fs::write(format!("{path}.obj"), o).expect("write obj");
    }
}
