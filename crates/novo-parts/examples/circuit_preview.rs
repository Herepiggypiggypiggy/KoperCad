//! Draws a library circuit file as an SVG page, for review while writing it:
//! the schematic, with pin names, and below it the mounting view.
//!
//! `cargo run -p novo-parts --example circuit_preview -- circuits/3-hoists/x.toml out.svg`

use std::fmt::Write as _;

use novo_parts::{MountKind, build_circuit, thumbnail};
use novo_symbols::{Library, SvgOptions, to_svg};

fn main() {
    let mut args = std::env::args().skip(1);
    let path = args.next().expect("circuit file");
    let out = args.next().unwrap_or_else(|| "circuit.svg".into());
    let text = std::fs::read_to_string(&path).expect("read circuit");
    let file = build_circuit(&text, &path).unwrap_or_else(|e| panic!("{e}"));
    let lib = Library::builtin().expect("symbols");
    let thumb = thumbnail(&file.definition);
    let (min, max) = thumb.bounds.expect("a circuit has content");
    let (w, h) = (max.x - min.x + 20.0, max.y - min.y + 20.0);
    let mut body = String::new();
    let opts = SvgOptions {
        scale: 1.0,
        show_connections: false,
        show_anchors: false,
        ..SvgOptions::default()
    };
    for s in &thumb.symbols {
        let Some(sym) = lib.get(&s.key) else { continue };
        let svg = to_svg(sym, &opts);
        let vb = svg
            .split("viewBox=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap();
        let v: Vec<f64> = vb.split(' ').map(|x| x.parse().unwrap()).collect();
        let inner = svg
            .split_once('>')
            .unwrap()
            .1
            .trim_end()
            .trim_end_matches("</svg>");
        let turn = match s.placement.rotation {
            novo_core::Rotation::R0 => 0,
            novo_core::Rotation::R90 => 90,
            novo_core::Rotation::R180 => 180,
            novo_core::Rotation::R270 => 270,
        };
        let _ = write!(
            body,
            "<g transform=\"translate({} {}) rotate({}) scale({} 1)\"><svg x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" viewBox=\"{}\" overflow=\"visible\">{}",
            s.placement.origin.x,
            s.placement.origin.y,
            -turn,
            if s.placement.mirrored { -1 } else { 1 },
            v[0],
            v[1],
            v[2],
            v[3],
            vb,
            inner
        );
        for i in 0..s.pins.len() {
            let name = &sym.connections[i].id;
            let _ = write!(
                body,
                "<text x=\"{}\" y=\"{}\" font-size=\"2.2\" fill=\"#c33\" stroke=\"none\">{}</text>",
                sym.connections[i].at[0] + 0.8,
                sym.connections[i].at[1] + 2.2,
                name
            );
        }
        body.push_str("</svg></g>");
    }
    for wire in &thumb.wires {
        let pts: Vec<String> = wire.iter().map(|p| format!("{},{}", p.x, p.y)).collect();
        let _ = writeln!(
            body,
            "<polyline points=\"{}\" fill=\"none\" stroke=\"#06c\" stroke-width=\"0.3\"/>",
            pts.join(" ")
        );
    }
    for j in &thumb.junctions {
        let _ = writeln!(
            body,
            "<circle cx=\"{}\" cy=\"{}\" r=\"0.8\" fill=\"#06c\"/>",
            j.x, j.y
        );
    }
    let mut page = String::new();
    let _ = write!(
        page,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"{} {} {} {}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#fff\"/><g stroke=\"#222\" stroke-width=\"0.35\" fill=\"none\" color=\"#222\">{}</g>",
        min.x - 10.0,
        min.y - 10.0,
        w,
        h,
        w * 4.0,
        h * 4.0,
        min.x - 10.0,
        min.y - 10.0,
        w,
        h,
        body
    );
    if let Some(m) = file.mounting() {
        let _ = write!(
            page,
            "<g transform=\"translate({} {})\">",
            min.x,
            max.y + 20.0
        );
        let _ = write!(
            page,
            "<rect width=\"{}\" height=\"{}\" fill=\"#eee\" stroke=\"#999\"/>",
            m.plate.width / 4.0,
            m.plate.height / 4.0
        );
        for r in &m.rails {
            let _ = write!(
                page,
                "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#555\" stroke-width=\"1\"/>",
                r.x / 4.0,
                r.y / 4.0,
                (r.x + r.length) / 4.0,
                r.y / 4.0
            );
        }
        for i in m.panel().chain(m.door()) {
            if let Some(r) = i.rect() {
                let _ = write!(
                    page,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"#cde\" stroke=\"#369\"/><text x=\"{}\" y=\"{}\" font-size=\"3\" fill=\"#000\">{}</text>",
                    r[0] / 4.0,
                    r[1] / 4.0,
                    (r[2] - r[0]) / 4.0,
                    (r[3] - r[1]) / 4.0,
                    r[0] / 4.0 + 0.5,
                    r[1] / 4.0 + 3.5,
                    i.tag
                );
            }
        }
        let _ = write!(page, "</g>");
        let _ = MountKind::Field;
    }
    page.push_str("</svg>");
    std::fs::write(&out, page).expect("write");
    println!("wrote {out}");
}
