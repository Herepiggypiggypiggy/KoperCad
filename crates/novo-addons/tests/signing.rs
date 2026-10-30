//! The signing workflow end to end in the library: sign a finished package, then every way the
//! result can be judged (signed, unknown key, revoked, tampered, wrong key) at install time and
//! on every later start, plus downgrade handling for signed packages.

mod common;

use common::*;
use novo_addons::keyfile::KeyFile;
use novo_addons::pack::{Signer32, sign_package};
use novo_addons::trust::{RevokedKey, Trust, TrustStore, TrustedKey};
use novo_addons::{AddonError, Change, InstallOptions, Package, Status, Store};

fn tick() -> InstallOptions {
    InstallOptions {
        accept_unsigned: true,
        enable: true,
    }
}

fn write_keys(root: &std::path::Path, text: &str) {
    std::fs::create_dir_all(root).unwrap();
    std::fs::write(root.join("trusted-keys.toml"), text).unwrap();
}

fn key_block(extra: &str) -> String {
    let k = key();
    format!("{}{extra}", k.toml_block())
}

#[test]
fn signing_a_finished_package_equals_packing_it_signed() {
    let unsigned = package("demo", "1.0.0", "x", false);
    let by_pack = package("demo", "1.0.0", "x", true);
    let by_sign = sign_package(&unsigned, &signer(), false).unwrap();
    assert_eq!(by_sign, by_pack, "same layout, same bytes");
    // signing twice is the same, and an already signed package asks for --replace
    assert_eq!(sign_package(&by_sign, &signer(), true).unwrap(), by_sign);
    assert!(matches!(
        sign_package(&by_sign, &signer(), false),
        Err(AddonError::BadSignature(_))
    ));
    // a package that is already broken is not signed (a signature would bless the damage)
    let read = Package::read(&unsigned, &TrustStore::default()).unwrap();
    let changed = b"not what the manifest hashed".to_vec();
    let broken = raw_zip(&[
        ("manifest.toml", &read.manifest_bytes),
        ("data/profile.toml", &changed),
    ]);
    assert!(sign_package(&broken, &signer(), false).is_err());
}

#[test]
fn a_dated_signature_installs_and_a_date_before_valid_from_does_not() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = signer();
    s.date = Some("2026-11-05");
    let bytes = sign_package(&package("demo", "1.0.0", "x", false), &s, false).unwrap();
    let root = dir.path().join("addons");
    write_keys(&root, &key_block("valid_from = \"2026-11-01\"\n"));
    let store = Store::new(root.clone(), app());
    let i = store
        .install_bytes(&bytes, InstallOptions::default())
        .unwrap();
    assert!(i.package.trust.is_signed());

    let early = {
        let mut s = signer();
        s.date = Some("2026-10-31");
        sign_package(&package("other", "1.0.0", "x", false), &s, false).unwrap()
    };
    let err = store.install_bytes(&early, tick()).unwrap_err();
    assert!(
        err.to_string().contains("became valid on 2026-11-01"),
        "{err}"
    );
}

#[test]
fn an_unknown_key_is_named_and_needs_the_tick() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let other = Signer32 {
        key_id: "someone-else",
        seed: [42u8; 32],
        date: None,
    };
    let bytes = sign_package(&package("demo", "1.0.0", "x", false), &other, false).unwrap();
    let inspection = store.inspect(&bytes).unwrap();
    assert!(matches!(
        &inspection.package.trust,
        Trust::UnknownKey { key_id } if key_id == "someone-else"
    ));
    assert!(inspection.needs_trust_tick() && inspection.blocked().is_none());
    assert!(matches!(
        store.install_bytes(&bytes, InstallOptions::default()),
        Err(AddonError::NotAccepted)
    ));
    store.install_bytes(&bytes, tick()).unwrap();
    // a later start says why in words that name the key
    let fresh = Store::new(store.root().to_path_buf(), app());
    let set = fresh.scan();
    assert_eq!(set.get("demo").unwrap().status, Status::Enabled);
    assert!(matches!(
        set.get("demo").unwrap().trust,
        Trust::UnknownKey { .. }
    ));
}

#[test]
fn a_revoked_key_refuses_the_install_even_with_the_tick() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("addons");
    write_keys(
        &root,
        &key_block("[[revoked]]\nid = \"test-key\"\nreason = \"the signing laptop was lost\"\n"),
    );
    let store = Store::new(root, app());
    let bytes = package("demo", "1.0.0", "x", true);
    let inspection = store.inspect(&bytes).unwrap();
    assert!(matches!(&inspection.package.trust, Trust::Revoked { .. }));
    let why = inspection.blocked().expect("blocked");
    assert!(
        why.contains("revoked") && why.contains("signing laptop was lost"),
        "{why}"
    );
    assert!(!inspection.package.trust.is_signed());
    let err = store.install_bytes(&bytes, tick()).unwrap_err();
    assert!(err.to_string().contains("revoked"), "{err}");
    assert!(store.scan().entries.is_empty());
}

#[test]
fn an_installed_add_on_stops_when_its_key_is_revoked_later() {
    let dir = tempfile::tempdir().unwrap();
    let trusting = trusting_store(dir.path());
    trusting
        .install_bytes(
            &package("demo", "1.0.0", "a", true),
            InstallOptions::default(),
        )
        .unwrap();
    assert!(trusting.scan().is_enabled("demo"));

    // the person (or a program update) now lists the key as revoked
    write_keys(
        trusting.root(),
        &key_block("[[revoked]]\nid = \"test-key\"\nreason = \"stolen\"\n"),
    );
    let fresh = Store::new(trusting.root().to_path_buf(), app());
    let set = fresh.scan();
    let e = set.get("demo").unwrap();
    assert!(!set.is_enabled("demo"));
    assert!(matches!(&e.trust, Trust::Revoked { reason, .. } if reason == "stolen"));
    assert!(
        matches!(&e.status, Status::Failed(m) if m.contains("revoked") && m.contains("stolen"))
    );
    // the tick does not bring it back: the same file is refused again
    assert!(
        fresh
            .install_bytes(&package("demo", "1.0.0", "a", true), tick())
            .is_err()
    );
}

#[test]
fn a_changed_signature_on_disk_shows_as_tampered_not_unsigned() {
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    store
        .install_bytes(
            &package("demo", "1.0.0", "a", true),
            InstallOptions::default(),
        )
        .unwrap();
    // someone swaps in a signature made by another holder of the "test-key" name
    let forged =
        novo_addons::trust::sign_manifest(&[9u8; 32], "test-key", None, b"whatever").unwrap();
    let sig = store.root().join("demo").join("1.0.0").join("signature");
    std::fs::write(sig, forged).unwrap();
    let set = Store::new(store.root().to_path_buf(), app()).scan();
    let e = set.get("demo").unwrap();
    assert!(matches!(&e.trust, Trust::Tampered { key_id, .. } if key_id == "test-key"));
    assert!(matches!(&e.status, Status::Failed(m) if m.contains("changed after it was signed")));
}

#[test]
fn a_package_changed_after_signing_fails_at_every_layer() {
    let signed = package("demo", "1.0.0", "x", true);
    let trust = TrustStore::default().with(key());
    // a different manifest with the old signature file
    let other = package("demo", "1.0.1", "x", false);
    let theirs = Package::read(&other, &TrustStore::default()).unwrap();
    let mine = Package::read(&signed, &trust).unwrap();
    assert!(mine.trust.is_signed());
    let spliced = {
        let sig = mine.signature.clone().unwrap();
        let mut entries: Vec<(&str, &[u8])> = vec![
            ("manifest.toml", &theirs.manifest_bytes),
            ("signature", &sig),
        ];
        for (n, b) in &theirs.files {
            entries.push((n.as_str(), b.as_slice()));
        }
        raw_zip(&entries)
    };
    assert!(matches!(
        Package::read(&spliced, &trust),
        Err(AddonError::Tampered(_))
    ));
}

#[test]
fn the_wrong_key_does_not_verify_and_the_right_one_does() {
    let wrong = TrustStore::default().with(TrustedKey::new(
        "test-key",
        "Not The Publisher",
        &KeyFile::new("test-key", "x", [1u8; 32])
            .unwrap()
            .public_hex(),
    ));
    let signed = package("demo", "1.0.0", "x", true);
    assert!(matches!(
        Package::read(&signed, &wrong),
        Err(AddonError::Tampered(_))
    ));
    assert!(
        Package::read(&signed, &TrustStore::default().with(key()))
            .unwrap()
            .trust
            .is_signed()
    );
    // a revocation list entry by public key catches the same key under any id
    let by_public = TrustStore::default().with(key()).with_revoked(RevokedKey {
        id: "old-name".into(),
        reason: "retired".into(),
        public: Some(key().public),
    });
    assert!(matches!(
        Package::read(&signed, &by_public).unwrap().trust,
        Trust::Revoked { .. }
    ));
}

#[test]
fn a_signed_older_version_is_still_refused_after_a_newer_one() {
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    let o = InstallOptions::default();
    store
        .install_bytes(&package("demo", "1.0.0", "a", true), o)
        .unwrap();
    store
        .install_bytes(&package("demo", "1.1.0", "b", true), o)
        .unwrap();
    // downgrade: a validly signed 1.0.0 file is not a reason to go back
    let old = package("demo", "1.0.0", "a", true);
    let inspection = store.inspect(&old).unwrap();
    assert!(matches!(inspection.change, Change::Older { .. }));
    assert!(inspection.blocked().is_some());
    assert!(store.install_bytes(&old, o).is_err());
    // the same version with other content is refused even though both are signed
    let same = package("demo", "1.1.0", "different", true);
    assert!(matches!(
        store.inspect(&same).unwrap().change,
        Change::SameVersionDifferent
    ));
    assert!(store.install_bytes(&same, o).is_err());
    assert_eq!(
        store.scan().get("demo").unwrap().active.to_string(),
        "1.1.0"
    );
}

#[test]
fn rolling_back_to_a_revoked_version_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let store = trusting_store(dir.path());
    let o = InstallOptions::default();
    store
        .install_bytes(&package("demo", "1.0.0", "a", true), o)
        .unwrap();
    // 1.1.0 is signed by a second key
    let k2 = KeyFile::new("second-key", "Test Publisher", [5u8; 32]).unwrap();
    let mut toml = key_block("");
    toml.push_str(&k2.trusted_key(None).unwrap().toml_block());
    write_keys(store.root(), &toml);
    let store = Store::new(store.root().to_path_buf(), app());
    let second = Signer32 {
        key_id: "second-key",
        seed: k2.seed,
        date: None,
    };
    store
        .install_bytes(
            &sign_package(&package("demo", "1.1.0", "b", false), &second, false).unwrap(),
            o,
        )
        .unwrap();
    // now the first key is revoked: the older version cannot be rolled back to
    toml.push_str("[[revoked]]\nid = \"test-key\"\nreason = \"retired\"\n");
    write_keys(store.root(), &toml);
    let store = Store::new(store.root().to_path_buf(), app());
    let err = store.roll_back("demo").unwrap_err();
    assert!(err.to_string().contains("revoked"), "{err}");
    assert_eq!(
        store.scan().get("demo").unwrap().active.to_string(),
        "1.1.0"
    );
}
