//! How the Manage add-ons window says who signed a package: signed, unsigned, unknown key,
//! revoked key, and a signature that does not match. Headless: the words that are painted, the
//! mark and the colour family of each state, and what the install flow does with each.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use eframe::egui;
use novo_addons::pack::{Signer32, pack_parts};
use novo_addons::trust::{Trust, TrustedKey, to_hex};
use novo_theme::widgets::Level as Tone;

use super::addons::{tick_warning, trust_look};
use super::gate1_tests::Drive;

const SEED: [u8; 32] = [7u8; 32];
const SCREEN: egui::Vec2 = egui::vec2(1366.0, 768.0);

const MANIFEST: &str = r#"[addon]
id = "signed-demo"
name = "Signed demo"
version = "1.0.0"
publisher = "Test Publisher"
description = "A demo add-on."
requires_app = ">=0.0.1"
api_version = 1
permissions = []
"#;

fn public() -> String {
    to_hex(&novo_addons::trust::public_key_for_seed(&SEED))
}

fn signed_package(key_id: &str) -> Vec<u8> {
    pack_parts(
        MANIFEST.as_bytes(),
        &[("data/x.toml".into(), b"x = 1\n".to_vec())],
        Some(&Signer32 {
            key_id,
            seed: SEED,
            date: None,
        }),
    )
    .unwrap()
}

fn write_package(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, bytes).unwrap();
    path
}

/// A window with an add-on folder whose `trusted-keys.toml` is `keys`.
fn drive_with_keys(keys: &str) -> (Drive, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let mut d = Drive::new(&dir.path().join("untitled"));
    set_keys(&mut d, dir.path(), keys);
    (d, dir)
}

fn set_keys(d: &mut Drive, dir: &Path, keys: &str) {
    let root = dir.join("addons");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("trusted-keys.toml"), keys).unwrap();
    d.app.init_addons_at(root);
}

fn key_toml() -> String {
    TrustedKey::new("test-key", "Test Publisher", &public()).toml_block()
}

fn painted_text(d: &mut Drive) -> String {
    // A window measures itself on its first frames; paint until it has settled.
    for _ in 0..3 {
        d.frame_sized(SCREEN, Vec::new());
    }
    let input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, SCREEN)),
        ..Default::default()
    };
    let app = &mut d.app;
    let output = d.ctx.run_ui(input, |ui| app.show(ui));
    fn collect(shape: &egui::Shape, out: &mut String) {
        match shape {
            egui::Shape::Text(t) => {
                out.push_str(t.galley.text());
                out.push('\n');
            }
            egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, out)),
            _ => {}
        }
    }
    let mut text = String::new();
    for clipped in &output.shapes {
        collect(&clipped.shape, &mut text);
    }
    text
}

fn states() -> [Trust; 5] {
    [
        Trust::Signed {
            key_id: "k".into(),
            owner: "KoperVonk".into(),
        },
        Trust::Unsigned {
            note: "the package has no signature".into(),
        },
        Trust::UnknownKey {
            key_id: "stranger-1".into(),
        },
        Trust::Revoked {
            key_id: "koper-vonk-2026".into(),
            owner: "KoperVonk".into(),
            reason: "retired".into(),
        },
        Trust::Tampered {
            key_id: "koper-vonk-2026".into(),
            owner: "KoperVonk".into(),
        },
    ]
}

#[test]
fn each_state_has_its_own_words_mark_and_a_colour_that_only_repeats_them() {
    let looks: Vec<_> = states().iter().map(|t| trust_look(t, false)).collect();
    let words: Vec<&str> = looks.iter().map(|l| l.0.as_str()).collect();
    let marks: BTreeSet<&str> = looks.iter().map(|l| l.2).collect();
    assert_eq!(marks.len(), 5, "five states, five different marks");
    assert!(words[0].starts_with("Signed by KoperVonk"));
    assert!(words[1].starts_with("Unsigned"));
    assert!(words[2].contains("unknown key") && words[2].contains("stranger-1"));
    assert!(words[3].contains("revoked key") && words[3].contains("retired"));
    assert!(words[4].contains("does not match") && words[4].contains("changed after"));
    // the two states that can never be installed are the only errors
    let tones: Vec<Tone> = looks.iter().map(|l| l.1).collect();
    assert_eq!(tones[0], Tone::Success);
    assert_eq!(tones[1], Tone::Warning);
    assert_eq!(tones[2], Tone::Warning);
    assert_eq!(tones[3], Tone::Error);
    assert_eq!(tones[4], Tone::Error);
    // "unsigned" is never said of a signed file
    for w in &words[2..] {
        assert!(!w.starts_with("Unsigned"), "{w}");
    }
    // the tick only exists where it can be used, and says which of the two cases it is
    assert!(
        tick_warning(&states()[1])
            .unwrap()
            .contains("Nobody you trust")
    );
    assert!(tick_warning(&states()[2]).unwrap().contains("stranger-1"));
    assert!(tick_warning(&states()[0]).is_none());
    assert!(tick_warning(&states()[3]).is_none() && tick_warning(&states()[4]).is_none());
    // accepted wording is kept where it already existed
    assert!(trust_look(&states()[1], true).0.contains("you accepted it"));
    assert!(trust_look(&states()[2], true).0.contains("you accepted it"));
}

#[test]
fn a_signed_file_installs_without_the_tick_and_the_window_says_signed_by() {
    let (mut d, dir) = drive_with_keys(&key_toml());
    let file = write_package(dir.path(), "a.kcaddon", &signed_package("test-key"));
    d.app.begin_install(file);
    d.frame();
    let prompt = d.app.wb.addons.manager.as_ref().unwrap().prompt.as_ref();
    let prompt = prompt.expect("the install prompt is shown");
    assert!(!prompt.inspection.needs_trust_tick());
    let text = painted_text(&mut d);
    assert!(text.contains("Signed by Test Publisher"), "{text}");
    assert!(!text.contains("I trust this file"), "{text}");
}

#[test]
fn a_key_nobody_here_knows_shows_unknown_key_with_the_tick_not_unsigned() {
    let (mut d, dir) = drive_with_keys("");
    let file = write_package(dir.path(), "a.kcaddon", &signed_package("test-key"));
    d.app.begin_install(file);
    d.frame();
    let text = painted_text(&mut d);
    assert!(
        text.contains("Signed with an unknown key (test-key)"),
        "{text}"
    );
    assert!(text.contains("I trust this file"), "{text}");
    assert!(text.contains("could be anyone"), "{text}");
    assert!(!text.contains("Unsigned, from file"), "{text}");
}

#[test]
fn a_revoked_key_is_refused_with_its_reason_and_no_tick_to_press() {
    let keys = format!(
        "{}[[revoked]]\nid = \"test-key\"\nreason = \"the signing laptop was lost\"\n",
        key_toml()
    );
    let (mut d, dir) = drive_with_keys(&keys);
    let file = write_package(dir.path(), "a.kcaddon", &signed_package("test-key"));
    d.app.begin_install(file);
    d.frame();
    let manager = d.app.wb.addons.manager.as_ref().unwrap();
    assert!(manager.prompt.is_none(), "there is nothing to confirm");
    let (tone, why) = manager.notice.clone().expect("the refusal is shown");
    assert_eq!(tone, Tone::Error);
    assert!(
        why.contains("revoked") && why.contains("signing laptop was lost"),
        "{why}"
    );
    let text = painted_text(&mut d);
    assert!(text.contains("revoked"), "{text}");
    assert!(!text.contains("I trust this file"), "{text}");
}

#[test]
fn a_file_changed_after_signing_is_refused_as_a_mismatch_not_as_unsigned() {
    let (mut d, dir) = drive_with_keys(&key_toml());
    // the real signature, laid over a manifest that was edited afterwards
    let good = signed_package("test-key");
    let read = novo_addons::Package::read(&good, &novo_addons::TrustStore::default()).unwrap();
    let mut edited = read.manifest_bytes.clone();
    edited.extend_from_slice(b"\n# edited after signing\n");
    let sig = read.signature.clone().unwrap();
    let mut entries: Vec<(String, Vec<u8>)> =
        vec![("manifest.toml".into(), edited), ("signature".into(), sig)];
    entries.extend(read.files.clone());
    let mut bytes = Vec::new();
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut bytes));
        let opts = zip::write::SimpleFileOptions::default();
        for (name, data) in &entries {
            zip.start_file(name.as_str(), opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    let file = write_package(dir.path(), "tampered.kcaddon", &bytes);
    d.app.begin_install(file);
    d.frame();
    let manager = d.app.wb.addons.manager.as_ref().unwrap();
    assert!(manager.prompt.is_none());
    let (tone, why) = manager.notice.clone().expect("the refusal is shown");
    assert_eq!(tone, Tone::Error);
    assert!(why.contains("changed after it was signed"), "{why}");
    assert!(!why.to_lowercase().contains("unsigned"), "{why}");
}

#[test]
fn an_installed_add_on_whose_key_is_revoked_later_shows_revoked_in_its_details() {
    let (mut d, dir) = drive_with_keys(&key_toml());
    let file = write_package(dir.path(), "a.kcaddon", &signed_package("test-key"));
    let store = d.app.wb.addons.store.as_ref().unwrap();
    store
        .install_file(&file, novo_addons::InstallOptions::default())
        .unwrap();
    d.app.refresh_addons();
    d.app.open_addons();
    d.frame();
    let text = painted_text(&mut d);
    assert!(text.contains("Signed by Test Publisher"), "{text}");
    assert!(d.app.addon_enabled("signed-demo"));

    // a program update (or the person) now lists the key as withdrawn
    let revoked = format!(
        "{}[[revoked]]\nid = \"test-key\"\nreason = \"stolen\"\n",
        key_toml()
    );
    set_keys(&mut d, dir.path(), &revoked);
    d.app.open_addons();
    d.frame();
    assert!(!d.app.addon_enabled("signed-demo"), "it stops loading");
    let text = painted_text(&mut d);
    assert!(
        text.contains("Signed with a revoked key (test-key), stolen"),
        "{text}"
    );
    assert!(text.contains("It did not load"), "{text}");
    assert!(!text.contains("Signed by Test Publisher"), "{text}");
}
