//! Hostile input: project files and JSON that are cut short, bit-flipped,
//! zeroed or stuffed with odd text never panic or hang. They open (possibly
//! salvaged), or they are refused with an error.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use novo_core::Project;
use novo_storage::{ProjectFile, Session, salvage};

const GOLDEN: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.novo"
);
const GOLDEN_JSON: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/golden/v0.1-motor-starter.json"
);

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n.max(1) as u64) as usize
    }
}

/// Opens the file every way the app does. A panic is the only failure.
fn poke(path: &Path, what: &str) {
    let r = catch_unwind(AssertUnwindSafe(|| {
        let _ = ProjectFile::read(path);
        let _ = ProjectFile::read_only(path).and_then(|f| f.load());
        let _ = salvage(path);
        let dir = path.parent().unwrap().join("rec");
        let _ = Session::open_in(path, Some(&dir));
    }));
    assert!(r.is_ok(), "panic while opening: {what}");
}

fn copy_to(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn truncated_and_corrupted_project_files_never_panic() {
    let golden = std::fs::read(GOLDEN).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let mut rng = Rng(0x5eed_1234_abcd);

    // Cut short at many lengths, including the empty file.
    let cuts = golden.len().min(200);
    for i in 0..=cuts {
        let len = golden.len() * i / cuts.max(1);
        let p = copy_to(dir.path(), "cut.novo", &golden[..len]);
        poke(&p, &format!("truncated to {len}"));
    }
    // Flipped bytes, in the header page and anywhere.
    for round in 0..150 {
        let mut b = golden.clone();
        let n = 1 + rng.below(12);
        for _ in 0..n {
            let at = if round % 3 == 0 {
                rng.below(200)
            } else {
                rng.below(b.len())
            };
            b[at] ^= 1 << rng.below(8);
        }
        let p = copy_to(dir.path(), "flip.novo", &b);
        poke(&p, &format!("bit flips round {round}"));
    }
    // Zeroed and 0xFF ranges.
    for round in 0..60 {
        let mut b = golden.clone();
        let at = rng.below(b.len());
        let len = (1 + rng.below(4096)).min(b.len() - at);
        let fill = if round % 2 == 0 { 0 } else { 0xFF };
        b[at..at + len].fill(fill);
        let p = copy_to(dir.path(), "zero.novo", &b);
        poke(&p, &format!("range fill round {round}"));
    }
    // Not a database at all.
    for (name, bytes) in [
        ("empty.novo", Vec::new()),
        ("text.novo", b"hello world, not a project".to_vec()),
        ("sqlite-header.novo", b"SQLite format 3\0".to_vec()),
        ("zeros.novo", vec![0u8; 100_000]),
        ("json.novo", GOLDEN_JSON.as_bytes().to_vec()),
    ] {
        let p = copy_to(dir.path(), name, &bytes);
        poke(&p, name);
    }
    // A folder or a missing file where a project should be.
    poke(&dir.path().join("missing.novo"), "missing");
    poke(dir.path(), "a folder");
}

#[test]
fn mutated_project_json_never_panics() {
    let json = std::fs::read_to_string(GOLDEN_JSON).unwrap();
    let mut rng = Rng(0xfeed_beef_1);
    let tokens: [&str; 14] = [
        "null",
        "-1",
        "1e999",
        "-1e999",
        "NaN",
        "18446744073709551616",
        "[]",
        "{}",
        "\"\"",
        "\"\u{202e}\u{0}\u{1f600}\"",
        "0",
        "true",
        "9007199254740993",
        "-0.0",
    ];
    // Truncation at every 1/300th.
    for i in 0..300 {
        let cut = json.len() * i / 300;
        let mut c = cut;
        while !json.is_char_boundary(c) {
            c -= 1;
        }
        let r = catch_unwind(|| Project::from_json(&json[..c]));
        assert!(r.is_ok(), "panic on json cut at {c}");
    }
    // Replace a random value token with something awkward.
    let bytes: Vec<usize> = json
        .match_indices(|c: char| c.is_ascii_digit() || c == '"')
        .map(|(i, _)| i)
        .collect();
    for round in 0..400 {
        let at = bytes[rng.below(bytes.len())];
        let end = json[at..]
            .find(|c: char| matches!(c, ',' | '}' | ']' | '\n'))
            .map(|n| at + n)
            .unwrap_or(json.len());
        let tok = tokens[rng.below(tokens.len())];
        let mutated = format!("{}{}{}", &json[..at], tok, &json[end..]);
        let r = catch_unwind(|| {
            if let Ok(p) = Project::from_json(&mutated) {
                // Whatever loads must also save and read back.
                let again = p.to_json();
                let _ = Project::from_json(&again);
            }
        });
        assert!(
            r.is_ok(),
            "panic on json mutation round {round}: {tok} at {at}"
        );
    }
}

#[test]
fn a_project_that_loads_always_round_trips() {
    let json = std::fs::read_to_string(GOLDEN_JSON).unwrap();
    let p = Project::from_json(&json).unwrap();
    let a = p.to_json();
    let b = Project::from_json(&a).unwrap().to_json();
    assert_eq!(a, b);
}
