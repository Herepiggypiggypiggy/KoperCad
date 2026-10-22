//! Builds the text stress matrix, finds every text collision and writes the
//! report and the project file.
//!
//!   cargo run -p novo-tests --example text_conflicts -- OUT_DIR
//!
//! Writes `OUT_DIR/text-stress-matrix.novo`, `OUT_DIR/conflicts.json` and
//! `OUT_DIR/summary.md`.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use novo_canvas::model::SymbolArt;
use novo_core::Document;
use novo_editor::art::LibraryArt;
use novo_storage::Session;
use novo_tests::text_conflicts::{Hit, collect, conflicts, group};
use novo_tests::text_stress::matrix;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| ".".into()));
    std::fs::create_dir_all(&out).unwrap();
    let mut doc = Document::default();
    let pages = matrix(&mut doc).expect("matrix builds");
    let catalog = novo_editor::catalog::Catalog::builtin();
    let art = LibraryArt::new(catalog.library(), novo_canvas::Stroke::default());
    let art_ref: &dyn SymbolArt = &art;
    let art_labels = |id: &str| -> Vec<String> {
        catalog
            .get(id)
            .map(|s| {
                novo_editor::art::labels(s)
                    .into_iter()
                    .map(|l| l.text)
                    .collect()
            })
            .unwrap_or_default()
    };
    let project = doc.project();
    let xref = novo_xref::XrefIndex::build(project, catalog.library());
    let mut all = Vec::new();
    let mut coverage: BTreeMap<&str, usize> = BTreeMap::new();
    let mut per_page: BTreeMap<String, usize> = BTreeMap::new();
    let mut json = String::from("[\n");
    for &page in &pages {
        let (ps, boxes, lines) = collect(project, page, art_ref, &art_labels, Some(&xref));
        let sheet = ps.scene().sheet().map(|s| (s.rect.min, s.rect.max));
        for b in &boxes {
            *coverage.entry(b.role.name()).or_default() += 1;
        }
        let found = conflicts(page, &boxes, &lines, sheet);
        let name = project
            .page(page)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        per_page.insert(name.clone(), found.len());
        for c in &found {
            let other = match &c.hit {
                Hit::Text(o) => format!("text '{}' ({})", o.text, o.role.name()),
                Hit::Line(l) => format!(
                    "{} ({:.1},{:.1})-({:.1},{:.1})",
                    l.kind.name(),
                    l.a.x,
                    l.a.y,
                    l.b.x,
                    l.b.y
                ),
                Hit::OutsideSheet => "outside the sheet".into(),
            };
            let _ = writeln!(
                json,
                "{{\"page\":{:?},\"cause\":{:?},\"text\":{:?},\"role\":{:?},\"at\":[{:.1},{:.1}],\"other\":{:?},\"depth\":{:.2}}},",
                name,
                c.cause(),
                c.text.text,
                c.text.role.name(),
                c.text.label.pos.x,
                c.text.label.pos.y,
                other,
                c.depth
            );
        }
        all.extend(found);
    }
    json.push_str("null]\n");
    std::fs::write(out.join("conflicts.json"), json).unwrap();

    let groups = group(&all);
    let mut sorted: Vec<_> = groups.iter().collect();
    sorted.sort_by_key(|b| std::cmp::Reverse(b.1.0));
    let mut md = String::new();
    let _ = writeln!(
        md,
        "# Text stress matrix: {} pages, {} conflicts\n",
        pages.len(),
        all.len()
    );
    let _ = writeln!(
        md,
        "| Cause (text / other) | Count | Worst depth mm |\n|---|---|---|"
    );
    for (cause, (n, d)) in sorted {
        let _ = writeln!(md, "| {cause} | {n} | {d:.1} |");
    }
    let _ = writeln!(
        md,
        "\n## Texts examined\n\n| Kind of text | Count |\n|---|---|"
    );
    for (k, n) in &coverage {
        let _ = writeln!(md, "| {k} | {n} |");
    }
    let _ = writeln!(md, "\n## By page\n\n| Page | Conflicts |\n|---|---|");
    for (p, n) in &per_page {
        let _ = writeln!(md, "| {p} | {n} |");
    }
    std::fs::write(out.join("summary.md"), &md).unwrap();
    println!("{md}");

    // Symbol sweep: which symbol kinds run their own text into their own drawing, per orientation.
    let mut by_symbol: BTreeMap<String, BTreeMap<String, usize>> = BTreeMap::new();
    for c in &all {
        let own = matches!(&c.hit, Hit::Line(l) if c.text.owner.is_some() && c.text.owner == l.owner)
            || matches!(&c.hit, Hit::Text(o) if c.text.owner.is_some() && c.text.owner == o.owner);
        let name = project
            .page(c.page)
            .map(|p| p.name.clone())
            .unwrap_or_default();
        if !own || !name.starts_with("symbols ") {
            continue;
        }
        let orient = name.split(' ').nth(1).unwrap_or("").to_string();
        if let Some(novo_core::AnyId::Symbol(s)) = c.text.owner
            && let Some(inst) = project.symbol(s)
        {
            *by_symbol
                .entry(inst.symbol.0.clone())
                .or_default()
                .entry(orient)
                .or_default() += 1;
        }
    }
    let orients: Vec<String> = novo_tests::text_stress::ORIENTATIONS
        .iter()
        .map(|o| novo_tests::text_stress::orientation_name(*o))
        .collect();
    let mut sym_md = String::from(
        "# Own-text conflicts per symbol and orientation (symbol sweep pages)\n\nCell = number of the symbol's own texts that run into its own drawing or into each other.\n\n| Symbol |",
    );
    for o in &orients {
        let _ = write!(sym_md, " {o} |");
    }
    sym_md.push_str("\n|---|");
    for _ in &orients {
        sym_md.push_str("---|");
    }
    sym_md.push('\n');
    for (sym, row) in &by_symbol {
        let _ = write!(sym_md, "| {sym} |");
        for o in &orients {
            let _ = write!(
                sym_md,
                " {} |",
                row.get(o).map(|n| n.to_string()).unwrap_or_default()
            );
        }
        sym_md.push('\n');
    }
    std::fs::write(out.join("symbols-by-orientation.md"), sym_md).unwrap();

    let path = out.join("text-stress-matrix.novo");
    let _ = std::fs::remove_file(&path);
    Session::create(&path, doc.project()).expect("project file");

    // A crowded cabinet for the Cabinet and 3D label pictures (qa scenario 62).
    let mut cab = Document::default();
    novo_tests::big_project(
        &mut cab,
        novo_tests::BigSpec {
            feeders: 8,
            per_cabinet: 8,
            uniform: false,
            seed: 7,
        },
    )
    .expect("cabinet project");
    let path = out.join("text-stress-cabinet.novo");
    let _ = std::fs::remove_file(&path);
    Session::create(&path, cab.project()).expect("cabinet project file");
}
