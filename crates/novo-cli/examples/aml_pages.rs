//! Developer tool: imports an AutomationML file into an empty project and
//! writes every page as an SVG, so the imported drawing can be looked at.
//!
//! `cargo run -p novo-cli --example aml_pages -- file.aml out_dir`

use std::fmt::Write as _;

use novo_core::{Document, Project};
use novo_editor::art::LibraryArt;
use novo_interop::aml::{plan_import, read_aml};
use novo_interop::dxf::EntityKind;

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(file), Some(out)) = (args.next(), args.next()) else {
        eprintln!("usage: aml_pages <file.aml> <out_dir>");
        std::process::exit(2);
    };
    let plant = read_aml(&std::fs::read_to_string(file).unwrap()).unwrap();
    let mut doc = Document::new(Project::new("aml"));
    let plan = plan_import(doc.project(), &plant);
    eprintln!("{}", plan.summary());
    plan.apply(&mut doc).unwrap();
    let project = doc.project();
    std::fs::create_dir_all(&out).unwrap();
    let library = novo_py::library().unwrap();
    let style = novo_pdf::print_style();
    let art = LibraryArt::new(library, style.symbol);
    for (n, page) in project.pages().enumerate() {
        let (w, h) = page.format.size_mm();
        let drawing = novo_interop::bridge::page_drawing(project, page.id, &art, style).unwrap();
        let mut s = format!(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 {w} {h}' width='{}' height='{}'><rect width='{w}' height='{h}' fill='white'/>",
            w * 4.0,
            h * 4.0
        );
        let y = |v: f64| h - v;
        for e in &drawing.entities {
            let col = match e.layer.as_str() {
                "Wires" => "#1a5fd0",
                "Texts" => "#444",
                "Frame" => "#bbb",
                _ => "#000",
            };
            let stroke = format!("stroke='{col}' stroke-width='0.25' fill='none'");
            match &e.kind {
                EntityKind::Line { a, b } => {
                    let _ = write!(
                        s,
                        "<line x1='{}' y1='{}' x2='{}' y2='{}' {stroke}/>",
                        a.x,
                        y(a.y),
                        b.x,
                        y(b.y)
                    );
                }
                EntityKind::Polyline { points, closed } => {
                    let pts: Vec<String> = points
                        .iter()
                        .map(|p| format!("{},{}", p.x, y(p.y)))
                        .collect();
                    let tag = if *closed { "polygon" } else { "polyline" };
                    let _ = write!(s, "<{tag} points='{}' {stroke}/>", pts.join(" "));
                }
                EntityKind::Circle { center, radius } => {
                    let _ = write!(
                        s,
                        "<circle cx='{}' cy='{}' r='{radius}' {stroke}/>",
                        center.x,
                        y(center.y)
                    );
                }
                EntityKind::Arc {
                    center,
                    radius,
                    start_deg,
                    end_deg,
                } => {
                    let pt = |d: f64| {
                        (
                            center.x + radius * d.to_radians().cos(),
                            y(center.y + radius * d.to_radians().sin()),
                        )
                    };
                    let (x0, y0) = pt(*start_deg);
                    let (x1, y1) = pt(*end_deg);
                    let large = i32::from((end_deg - start_deg).rem_euclid(360.0) > 180.0);
                    let _ = write!(
                        s,
                        "<path d='M{x0},{y0} A{radius},{radius} 0 {large} 0 {x1},{y1}' {stroke}/>"
                    );
                }
                EntityKind::Text {
                    pos, height, text, ..
                } => {
                    let t = text.replace('&', "&amp;").replace('<', "&lt;");
                    let _ = write!(
                        s,
                        "<text x='{}' y='{}' font-size='{}' fill='{col}' font-family='sans-serif'>{t}</text>",
                        pos.x,
                        y(pos.y),
                        height.max(1.5)
                    );
                }
            }
        }
        s.push_str("</svg>");
        std::fs::write(format!("{out}/page{:02}.svg", n + 1), s).unwrap();
    }
}
