//! Problem triangles, census for the parts and catalogue area (lane T9).
//!
//! Counts the lines in the area's files that still say a problem in words or
//! in a warning or error colour: toasts at `Level::Error` / `Level::Warning`,
//! `colored_label` sentences, and text coloured with the palette's `error`,
//! `warning` or `warn_fg_color`. The ceilings in `census/parts.txt` only go
//! down; lower them when you migrate a site. The test module of a file (from a
//! `#[cfg(test)]` at the start of a line) is not counted.

use std::path::Path;

const FILES: &str = include_str!("census/parts.txt");

fn is_site(line: &str) -> bool {
    let code = line.trim_start();
    if code.starts_with("//") {
        return false;
    }
    code.contains("Level::Error")
        || code.contains("Level::Warning")
        || code.contains("colored_label(")
        || code.contains(".error")
        || code.contains(".warning")
        || code.contains("warn_fg_color")
}

fn count(path: &Path) -> usize {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    // A test module starts at a `#[cfg(test)]` in column 0; the indented
    // ones gate single functions of the real code.
    let body = text.split("\n#[cfg(test)]").next().unwrap_or("");
    body.lines().filter(|l| is_site(l)).count()
}

#[test]
fn the_parts_area_has_no_more_problem_sentences_than_its_ceiling() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/app");
    let mut failures = Vec::new();
    for line in FILES
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
    {
        let (file, ceiling) = line.split_once(' ').expect("`file ceiling`");
        let ceiling: usize = ceiling.trim().parse().expect("a number");
        let found = count(&src.join(file));
        if found > ceiling {
            failures.push(format!(
                "{file}: {found} sites, ceiling {ceiling}. Show the problem as a flag (parts_flags) instead."
            ));
        } else if found < ceiling {
            failures.push(format!(
                "{file}: {found} sites, ceiling {ceiling}. Lower the ceiling in tests/census/parts.txt."
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
