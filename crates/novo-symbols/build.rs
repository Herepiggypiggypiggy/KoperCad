//! Embeds every file under `symbols/` so the built-in library ships inside
//! the binary and works offline with no install step.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::{env, fs};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("directory entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "toml") {
            out.push(path);
        }
    }
}

fn main() {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("symbols");
    println!("cargo:rerun-if-changed={}", root.display());

    let mut files = Vec::new();
    collect(&root, &mut files);

    // One table per pack (the first path segment), in file order.
    let mut packs: Vec<(String, Vec<(String, String)>)> = Vec::new();
    for file in &files {
        let rel = file.strip_prefix(&root).unwrap().with_extension("");
        let id: Vec<_> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        let pack = id[0].to_string();
        let entry = (id.join("/"), file.to_string_lossy().into_owned());
        match packs.iter_mut().find(|(name, _)| *name == pack) {
            Some((_, list)) => list.push(entry),
            None => packs.push((pack, vec![entry])),
        }
    }

    let mut code = String::from("pub(crate) static BUILTIN: &[(&str, &[(&str, &str)])] = &[\n");
    for (pack, list) in &packs {
        writeln!(code, "    ({pack:?}, &[").unwrap();
        for (id, abs) in list {
            writeln!(code, "        ({id:?}, include_str!({abs:?})),").unwrap();
        }
        code.push_str("    ]),\n");
    }
    code.push_str("];\n");

    let out = Path::new(&env::var("OUT_DIR").unwrap()).join("builtin.rs");
    fs::write(out, code).unwrap();
}
