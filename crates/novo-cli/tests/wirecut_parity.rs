//! `kopercad-cli export wirecut` writes the job file the app's File > Export
//! wire cut writes: `;` and a byte order mark in a file, plain rows on
//! standard output, nothing written when there is nothing to cut.

use std::path::{Path, PathBuf};

use novo_cli::exit;
use novo_py::{Role, ScriptProject};

fn novo(args: &[&str]) -> (i32, String, String) {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = novo_cli::run(
        std::iter::once("kopercad-cli").chain(args.iter().copied()),
        &mut out,
        &mut err,
    );
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn s(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// Two devices joined by one wire between their pins.
fn project(dir: &Path, wired: bool) -> PathBuf {
    let path = dir.join("cut.novo");
    let mut p = ScriptProject::create(&path, "Cut", false).unwrap();
    let page = p.add_page("1", "Main").unwrap();
    let k1 = p.add_device("-K1", "Contactor", "general").unwrap();
    let k2 = p.add_device("-K2", "Contactor", "general").unwrap();
    let place = |p: &mut ScriptProject, y: f64, device| {
        p.place(
            page,
            "coil",
            0.0,
            y,
            0,
            false,
            Role::Device { device, main: true },
        )
        .unwrap()
    };
    let a = place(&mut p, 0.0, k1);
    let b = place(&mut p, 60.0, k2);
    if wired {
        p.connect((a, "A2"), (b, "A1")).unwrap();
    }
    p.save().unwrap();
    p.close().unwrap();
    path
}

#[test]
fn a_file_gets_semicolons_and_a_byte_order_mark_like_the_app() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), true);
    let out = dir.path().join("cut.csv");
    let (code, _, err) = novo(&["export", "wirecut", s(&path), "-o", s(&out)]);
    assert_eq!(code, exit::OK, "{err}");
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]), "no byte order mark");
    let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
    assert!(text.starts_with("No;Wire no.;Qty;mm2;"), "{text}");
    assert_eq!(text.lines().count(), 2, "{text}");
}

#[test]
fn an_explicit_delimiter_wins_and_standard_output_stays_plain() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), true);
    let out = dir.path().join("cut.csv");
    let (code, _, err) = novo(&[
        "export",
        "wirecut",
        s(&path),
        "-o",
        s(&out),
        "--delimiter",
        "\t",
    ]);
    assert_eq!(code, exit::OK, "{err}");
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(text.contains("No\tWire no.\tQty"), "{text}");

    let (code, text, err) = novo(&["export", "wirecut", s(&path)]);
    assert_eq!(code, exit::OK, "{err}");
    assert!(text.starts_with("No,Wire no.,Qty,mm2,"), "{text}");
    assert!(!text.starts_with('\u{feff}'));
}

#[test]
fn with_nothing_to_cut_no_file_is_written() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), false);
    let out = dir.path().join("cut.csv");
    let (code, text, err) = novo(&["export", "wirecut", s(&path), "-o", s(&out)]);
    assert_eq!(code, exit::OK, "{err}");
    assert!(text.contains("No wires to cut yet"), "{text}");
    assert!(!out.exists());
}

#[test]
fn the_project_rules_reach_the_cli_lengths() {
    let dir = tempfile::tempdir().unwrap();
    let path = project(dir.path(), true);
    let (_, plain, _) = novo(&["export", "wirecut", s(&path)]);
    let length = |csv: &str| -> f64 {
        let row = csv.lines().nth(1).unwrap();
        row.split(',').nth(6).unwrap().parse().unwrap()
    };
    let before = length(&plain);

    let mut p = ScriptProject::open(&path).unwrap();
    let mut info = p.project().info().clone();
    info.properties
        .insert("wire.slack_mm".into(), novo_core::Value::Float(100.0));
    p.execute(novo_core::Command::SetProjectInfo { info })
        .unwrap();
    p.save().unwrap();
    p.close().unwrap();
    let (_, changed, _) = novo(&["export", "wirecut", s(&path)]);
    // Estimated wire: 50 mm more of service loop at each end.
    assert_eq!(length(&changed), before + 100.0, "{plain}\n{changed}");
}
