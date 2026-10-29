//! Hostile and damaged input sweep (pre-work for real use).
//!
//! Every reader gets real files cut short, with bytes flipped, bytes removed,
//! chunks repeated and noise appended. Nothing may panic or run for long: a
//! damaged file is refused in words or read as far as it is whole. Every
//! output (reports, CSV, workbook, PDF, labels, checks) also runs over every
//! fixture project in the repository.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// What one input may take before it counts as a hang. The whole workspace runs in parallel
/// in CI, where a debug build of one output over a big fixture can pass 20 s on a loaded
/// runner (circuits PRs #1726 and #1739 failed here and passed on re-run). A real hang or a
/// quadratic blow-up takes minutes, so the limit is loose.
const SLOW: Duration = Duration::from_secs(120);

/// Small deterministic generator, so a failure repeats.
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

/// The damaged copies of `seed` (deterministic): cut at spread-out points,
/// bit flips, deleted ranges, repeated ranges, noise.
fn mutants(seed: &[u8], count: usize) -> Vec<(String, Vec<u8>)> {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ seed.len() as u64);
    let mut out = vec![
        ("empty".to_string(), Vec::new()),
        (
            "one byte".to_string(),
            vec![seed.first().copied().unwrap_or(0)],
        ),
        ("nul bytes".to_string(), vec![0u8; 4096]),
        ("0xff bytes".to_string(), vec![0xffu8; 4096]),
    ];
    let n = seed.len();
    if n > 2 {
        for i in 1..=12 {
            let cut = n * i / 13;
            out.push((format!("cut at {cut}"), seed[..cut].to_vec()));
        }
        out.push(("cut last byte".to_string(), seed[..n - 1].to_vec()));
        out.push(("first half doubled".into(), {
            let mut v = seed[..n / 2].to_vec();
            v.extend_from_slice(&seed[..n / 2]);
            v
        }));
        out.push(("garbage appended".into(), {
            let mut v = seed.to_vec();
            v.extend((0..512).map(|_| rng.next() as u8));
            v
        }));
    }
    for k in 0..count {
        if n < 4 {
            break;
        }
        let mut v = seed.to_vec();
        match k % 4 {
            0 => {
                for _ in 0..1 + rng.below(8) {
                    let i = rng.below(v.len());
                    v[i] ^= 1 << rng.below(8);
                }
                out.push((format!("bit flips #{k}"), v));
            }
            1 => {
                let a = rng.below(v.len());
                let b = (a + 1 + rng.below(64)).min(v.len());
                v.drain(a..b);
                out.push((format!("deleted {a}..{b} #{k}"), v));
            }
            2 => {
                let a = rng.below(v.len());
                let b = (a + 1 + rng.below(256)).min(v.len());
                let chunk = v[a..b].to_vec();
                for _ in 0..1 + rng.below(30) {
                    let at = rng.below(v.len());
                    for (j, c) in chunk.iter().enumerate() {
                        v.insert(at + j, *c);
                    }
                }
                out.push((format!("repeated {a}..{b} #{k}"), v));
            }
            _ => {
                let i = rng.below(v.len());
                v[i] = [0, b'\n', b'"', b'<', b'>', b';', 0xff, 0x80][rng.below(8)];
                out.push((format!("byte swap #{k}"), v));
            }
        }
    }
    out
}

/// Run `f` on every mutant; fail on panic or when one takes too long.
fn sweep(what: &str, seed: &[u8], count: usize, f: impl Fn(&[u8])) {
    let mut bad = Vec::new();
    for (label, data) in mutants(seed, count) {
        let started = Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| f(&data)));
        let took = started.elapsed();
        if result.is_err() {
            bad.push(format!("{what}: PANIC on `{label}` ({} bytes)", data.len()));
        } else if took > SLOW {
            bad.push(format!("{what}: SLOW {took:?} on `{label}`"));
        }
    }
    assert!(bad.is_empty(), "\n{}", bad.join("\n"));
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn files(dir: &str, ext: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(repo().join(dir))
        .map(|d| {
            d.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == ext))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn xml_files(dir: &str) -> Vec<PathBuf> {
    fn walk(p: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(d) = std::fs::read_dir(p) {
            for e in d.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, out);
                } else if p.extension().is_some_and(|x| x == "xml") {
                    out.push(p);
                }
            }
        }
    }
    let mut v = Vec::new();
    walk(&repo().join(dir), &mut v);
    v.sort();
    v
}

#[test]
fn damaged_project_files_are_refused_not_crashed() {
    let dir = tempfile::tempdir().unwrap();
    for (i, seed) in files("qa/fixtures", "novo").iter().take(8).enumerate() {
        let bytes = std::fs::read(seed).unwrap();
        let name = seed.file_name().unwrap().to_string_lossy().to_string();
        sweep(&format!("project {name}"), &bytes, 12, |data| {
            let p = dir.path().join(format!("m{i}.novo"));
            std::fs::write(&p, data).unwrap();
            if let Ok(project) = novo_storage::ProjectFile::read(&p) {
                // A file that reads must also be usable everywhere.
                let _ = novo_reports::generate_all(&project);
                let _ = novo_checks::check(&project);
            }
        });
    }
}

#[test]
fn damaged_aml_is_refused_or_read_whole() {
    let seed =
        std::fs::read(repo().join("crates/novo-interop/tests/fixtures/aml/hoist-feeder.aml"))
            .unwrap();
    sweep("aml", &seed, 400, |data| {
        let text = String::from_utf8_lossy(data);
        if let Ok(plant) = novo_interop::aml::read_aml(&text) {
            let mut doc = novo_core::Document::default();
            let plan = novo_interop::aml::plan_import(doc.project(), &plant);
            let _ = plan.summary();
            let _ = plan.apply(&mut doc);
            // Importing the same plant twice must not duplicate or fail loudly.
            let plan2 = novo_interop::aml::plan_import(doc.project(), &plant);
            let _ = plan2.apply(&mut doc);
            let _ = novo_checks::check(doc.project());
        }
    });
}

#[test]
fn damaged_pxf_is_refused_or_read_whole() {
    for f in files("crates/novo-interop/tests/fixtures/pxf", "epj") {
        let seed = std::fs::read(&f).unwrap();
        sweep(&format!("pxf {f:?}"), &seed, 150, |data| {
            let text = String::from_utf8_lossy(data);
            let _ = novo_interop::eplan::pxf::read_pxf_str(&text);
        });
    }
}

const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nBLOCKS\n0\nBLOCK\n2\nB\n10\n0\n20\n0\n0\nLINE\n8\n0\n10\n0\n20\n0\n11\n5\n21\n5\n0\nENDBLK\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\nWALL\n10\n0\n20\n0\n11\n100\n21\n0\n0\nCIRCLE\n8\nWALL\n10\n50\n20\n50\n40\n10\n0\nARC\n8\nWALL\n10\n0\n20\n0\n40\n5\n50\n0\n51\n90\n0\nTEXT\n8\nT\n10\n1\n20\n1\n40\n2.5\n1\nHello \u{e9}\n0\nLWPOLYLINE\n8\nP\n90\n3\n70\n1\n10\n0\n20\n0\n10\n10\n20\n0\n10\n10\n20\n10\n0\nINSERT\n8\n0\n2\nB\n10\n20\n20\n20\n0\nENDSEC\n0\nEOF\n";

#[test]
fn damaged_dxf_is_refused_or_read_whole() {
    sweep("dxf", DXF.as_bytes(), 600, |data| {
        let _ = novo_interop::dxf::read_dxf(data);
    });
}

#[test]
fn damaged_bmecat_is_refused_or_read_whole() {
    for f in xml_files("crates/novo-parts/tests/golden") {
        let seed = std::fs::read(&f).unwrap();
        sweep(&format!("bmecat {f:?}"), &seed, 200, |data| {
            let text = String::from_utf8_lossy(data);
            let _ = novo_parts::import_bmecat(&text, "ACME");
        });
    }
}

const PARTS_CSV: &str = "Manufacturer;Order number;Description;Price;Width;Weight\nABB;1SBL;Contactor, 3-pole \"AF09\";12,50;45;0,3\nABB;1SBL;Duplicate row;13,10;45;0,3\nPhoenix;3044076;Terminal \u{e9}\u{e8};0.42;5,2;\n;;;;;\nSiemens;3RT2015;=1+1;1e400;-5;NaN\n";

#[test]
fn damaged_part_lists_are_refused_or_read_whole() {
    sweep("parts csv", PARTS_CSV.as_bytes(), 600, |data| {
        let _ = novo_parts::TableFile::read("p.csv", data);
        let text = String::from_utf8_lossy(data);
        let _ = novo_parts::parse_delimited(&text);
        let _ = novo_parts::import_catalog(&text, "ACME");
    });
    for f in files("qa/fixtures/parts", "xlsx") {
        let seed = std::fs::read(&f).unwrap();
        sweep(&format!("parts xlsx {f:?}"), &seed, 150, |data| {
            let _ = novo_parts::TableFile::read("p.xlsx", data);
        });
    }
    for f in files("qa/fixtures/parts", "csv") {
        let seed = std::fs::read(&f).unwrap();
        sweep(&format!("parts file {f:?}"), &seed, 150, |data| {
            let _ = novo_parts::TableFile::read("p.csv", data);
            let _ = novo_parts::TableFile::read("p.txt", data);
            let _ = novo_parts::TableFile::read("p.tsv", data);
        });
    }
}

#[test]
fn damaged_edit_sheets_are_refused_or_read_whole() {
    // A real export, read back damaged.
    let project =
        novo_storage::ProjectFile::read(repo().join("qa/fixtures/distribution-board.novo"))
            .expect("fixture");
    let book = novo_interop::sheets::export_project(&project);
    let xlsx = novo_interop::sheets::xlsx_bytes(&book).unwrap();
    sweep("sheets xlsx", &xlsx, 200, |data| {
        if let Ok(b) = novo_interop::sheets::read_xlsx(data) {
            let _ = novo_interop::sheets::plan_import(&project, &b);
        }
    });
    let csv = novo_interop::sheets::to_csv(
        book.table(novo_interop::sheets::TableKind::Devices)
            .expect("devices table"),
        &novo_interop::sheets::CsvOptions::default(),
    );
    sweep("sheets csv", csv.as_bytes(), 300, |data| {
        let text = novo_interop::sheets::decode_csv(data);
        if let Ok(b) = novo_interop::sheets::read_csv(&text, None) {
            let _ = novo_interop::sheets::plan_import(&project, &b);
        }
    });
}

#[test]
fn every_output_runs_over_every_fixture_project() {
    let dir = tempfile::tempdir().unwrap();
    let mut seen = 0;
    for f in files("qa/fixtures", "novo") {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let project = match novo_storage::ProjectFile::read(&f) {
            Ok(p) => p,
            Err(e) => {
                // Deliberately damaged fixtures are refused in words.
                assert!(!e.to_string().is_empty(), "{name}");
                continue;
            }
        };
        seen += 1;
        let started = Instant::now();
        let r = catch_unwind(AssertUnwindSafe(|| {
            let reports = novo_reports::generate_all(&project);
            let refs: Vec<&novo_reports::ReportOutput> = reports.iter().collect();
            let _ = novo_reports::xlsx_bytes(&refs).expect("xlsx");
            for r in &refs {
                let _ = novo_reports::to_csv(r, &novo_reports::CsvOptions::default());
            }
            let _ = novo_checks::check(&project);
            let _ = novo_interop::sheets::export_project(&project);
            let _ = novo_pdf::export_pdf(&project, &novo_pdf::PdfOptions::default());
            let _ = novo_interop::aml::export_project(
                &project,
                &novo_interop::aml::AmlOptions::default(),
            );
        }));
        assert!(r.is_ok(), "output panicked on fixture {name}");
        assert!(
            started.elapsed() < SLOW,
            "outputs on fixture {name} took {:?}",
            started.elapsed()
        );
        let _ = dir.path();
    }
    assert!(seen > 10, "fixtures were not found ({seen})");
}

/// Change one JSON leaf (or drop a key) in a hostile way.
fn hurt(value: &mut serde_json::Value, rng: &mut Rng) {
    use serde_json::{Value, json};
    let leaves = count_leaves(value);
    if leaves == 0 {
        return;
    }
    let mut target = rng.below(leaves);
    let drop_key = rng.below(6) == 0;
    visit(value, &mut target, &mut |leaf, remove| {
        if drop_key {
            *remove = true;
            return;
        }
        let long = "x".repeat(70_000);
        let options: Vec<Value> = match leaf {
            Value::Number(_) => vec![
                json!(0),
                json!(-1),
                json!(-1.5e300),
                json!(1.0e300),
                json!(4294967296u64),
                json!(9223372036854775807i64),
                json!(0.000001),
                json!("7"),
                Value::Null,
            ],
            Value::String(_) => vec![
                json!(""),
                json!(" "),
                json!("\u{0}"),
                json!("=cmd|' /C calc'!A0"),
                json!("é\u{200b}\u{202e}日本語🙂"),
                json!("a\nb\r\n\"c\";d,e"),
                json!(long),
                json!(12),
                Value::Null,
            ],
            Value::Bool(b) => vec![json!(!*b), json!("true"), Value::Null],
            _ => vec![Value::Null],
        };
        *leaf = options[rng.below(options.len())].clone();
    });
}

fn count_leaves(v: &serde_json::Value) -> usize {
    match v {
        serde_json::Value::Array(a) => a.iter().map(count_leaves).sum(),
        serde_json::Value::Object(o) => o.values().map(count_leaves).sum(),
        _ => 1,
    }
}

fn visit(
    v: &mut serde_json::Value,
    target: &mut usize,
    f: &mut dyn FnMut(&mut serde_json::Value, &mut bool),
) -> bool {
    match v {
        serde_json::Value::Array(a) => {
            let mut hit = None;
            for (i, x) in a.iter_mut().enumerate() {
                if matches!(
                    x,
                    serde_json::Value::Array(_) | serde_json::Value::Object(_)
                ) {
                    if visit(x, target, f) {
                        return true;
                    }
                } else if *target == 0 {
                    let mut remove = false;
                    f(x, &mut remove);
                    if remove {
                        hit = Some(i);
                    }
                    *target = usize::MAX;
                    if let Some(i) = hit {
                        a.remove(i);
                    }
                    return true;
                } else {
                    *target -= 1;
                }
            }
            false
        }
        serde_json::Value::Object(o) => {
            let keys: Vec<String> = o.keys().cloned().collect();
            for k in keys {
                let x = o.get_mut(&k).unwrap();
                if matches!(
                    x,
                    serde_json::Value::Array(_) | serde_json::Value::Object(_)
                ) {
                    if visit(x, target, f) {
                        return true;
                    }
                } else if *target == 0 {
                    let mut remove = false;
                    f(x, &mut remove);
                    *target = usize::MAX;
                    if remove {
                        o.remove(&k);
                    }
                    return true;
                } else {
                    *target -= 1;
                }
            }
            false
        }
        _ => false,
    }
}

/// Everything the program does with a project that reads.
fn use_everywhere(project: &novo_core::Project) {
    let reports = novo_reports::generate_all(project);
    let refs: Vec<&novo_reports::ReportOutput> = reports.iter().collect();
    let _ = novo_reports::xlsx_bytes(&refs);
    for r in &refs {
        let _ = novo_reports::to_csv(r, &novo_reports::CsvOptions::default());
    }
    let _ = novo_checks::check(project);
    let _ = novo_interop::sheets::export_project(project);
    let _ = novo_pdf::export_pdf(project, &novo_pdf::PdfOptions::default());
    let _ = novo_interop::aml::export_project(project, &novo_interop::aml::AmlOptions::default());
}

#[test]
fn hostile_values_inside_a_valid_project_file_never_crash_an_output() {
    let dir = tempfile::tempdir().unwrap();
    let rounds: usize = std::env::var("HOSTILE_ROUNDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25);
    let mut failures = Vec::new();
    let mut read_ok = 0;
    let mut refused = 0;
    for (n, seed) in files("qa/fixtures", "novo").iter().enumerate() {
        let name = seed.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("damaged") {
            continue;
        }
        let salt: u64 = std::env::var("HOSTILE_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let mut rng =
            Rng(0xD1B5_4A32_D192_ED03 ^ (n as u64 * 7919) ^ salt.wrapping_mul(0x9E37_79B9));
        for round in 0..rounds {
            let copy = dir.path().join("m.novo");
            std::fs::copy(seed, &copy).unwrap();
            let conn = rusqlite::Connection::open(&copy).unwrap();
            let ids: Vec<(i64, String)> = {
                let mut st = conn.prepare("select id, data from records").unwrap();
                st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
                    .unwrap()
                    .filter_map(|r| r.ok())
                    .collect()
            };
            if ids.is_empty() {
                continue;
            }
            for _ in 0..1 + rng.below(3) {
                let (id, data) = &ids[rng.below(ids.len())];
                if let Ok(mut v) = serde_json::from_str::<serde_json::Value>(data) {
                    hurt(&mut v, &mut rng);
                    conn.execute(
                        "update records set data = ?1 where id = ?2",
                        rusqlite::params![v.to_string(), id],
                    )
                    .unwrap();
                }
            }
            drop(conn);
            let started = Instant::now();
            let result = catch_unwind(AssertUnwindSafe(|| {
                match novo_storage::ProjectFile::read(&copy) {
                    Ok(p) => {
                        use_everywhere(&p);
                        true
                    }
                    Err(_) => false,
                }
            }));
            match result {
                Ok(true) => read_ok += 1,
                Ok(false) => refused += 1,
                Err(_) => {
                    let keep = std::env::temp_dir().join(format!("hostile-{name}-{round}.novo"));
                    let _ = std::fs::copy(&copy, &keep);
                    failures.push(format!(
                        "{name} round {round}: PANIC (kept {})",
                        keep.display()
                    ));
                }
            }
            if started.elapsed() > SLOW {
                failures.push(format!(
                    "{name} round {round}: SLOW {:?}",
                    started.elapsed()
                ));
            }
        }
    }
    eprintln!("hostile values: {read_ok} read and used, {refused} refused");
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

#[test]
fn damaged_macro_files_are_refused_or_read_whole() {
    for (name, m) in novo_parts::starter_macros().into_iter().take(6) {
        let seed = m.to_json();
        sweep(&format!("macro {name}"), seed.as_bytes(), 150, |data| {
            let text = String::from_utf8_lossy(data);
            let _ = novo_parts::MacroFile::from_json(&text, std::path::Path::new("m.kcmacro"));
        });
    }
}

#[test]
fn damaged_eplan_part_lists_are_refused_or_read_whole() {
    let seed = "Manufacturer;Order number;Description\nABB;1SBL;Contactor\n";
    sweep("eplan parts", seed.as_bytes(), 300, |data| {
        let text = String::from_utf8_lossy(data);
        let _ = novo_interop::eplan::import_auto(&text, "ACME");
    });
    let xml = "<?xml version=\"1.0\"?><Parts><Part><PARTNR>A1</PARTNR><MANUFACTURER>X</MANUFACTURER><DESCR1>d</DESCR1></Part></Parts>";
    sweep("eplan parts xml", xml.as_bytes(), 300, |data| {
        let text = String::from_utf8_lossy(data);
        let _ = novo_interop::eplan::import_auto(&text, "ACME");
    });
}
