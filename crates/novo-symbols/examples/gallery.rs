//! Writes an HTML page showing every built-in symbol, for visual review.
//!
//! `cargo run -p novo-symbols --example gallery -- gallery.html`

use std::fmt::Write as _;

use novo_symbols::{Library, SvgOptions, to_svg};

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "symbol-gallery.html".into());
    let lib = Library::builtin().expect("built-in library loads");
    let options = SvgOptions::default();

    let mut html = String::from(
        "<!doctype html><meta charset=\"utf-8\"><title>Symbol gallery</title>\n\
         <style>body{font-family:sans-serif;margin:16px}h2{margin-top:32px}\
         .grid{display:flex;flex-wrap:wrap;gap:12px}\
         figure{margin:0;padding:8px;border:1px solid #ccc;border-radius:6px;min-width:180px}\
         figure svg{display:block;margin:auto}\
         figcaption{font-size:12px;text-align:center}code{font-size:11px;color:#555}</style>\n",
    );
    let mut category = String::new();
    for symbol in lib.iter() {
        if symbol.category != category {
            if !category.is_empty() {
                html.push_str("</div>\n");
            }
            category = symbol.category.clone();
            let _ = writeln!(html, "<h2>{category}</h2><div class=\"grid\">");
        }
        let _ = writeln!(
            html,
            "<figure>{}<figcaption>{}<br><code>{}</code></figcaption></figure>",
            to_svg(symbol, &options),
            symbol.name,
            symbol.id
        );
    }
    html.push_str("</div>\n");
    std::fs::write(&out, html).expect("write gallery");
    println!("{} symbols written to {out}", lib.len());
}
