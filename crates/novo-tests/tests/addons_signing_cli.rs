//! `kopercad-cli addons keygen | sign | verify`, through the same entry point the binary uses,
//! with the committed TEST-ONLY key, plus a guard that no other private key is in the repository.

use std::path::{Path, PathBuf};

use novo_addons::keyfile::KeyFile;

fn cli(args: &[&str]) -> (i32, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let argv = std::iter::once("kopercad-cli").chain(args.iter().copied());
    let code = novo_cli::run(argv.map(std::ffi::OsString::from), &mut out, &mut err);
    (
        code,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn test_key() -> PathBuf {
    manifest_dir().join("tests/keys/TEST-ONLY-addon-signing.key")
}

fn unsigned_fixture(dir: &Path) -> PathBuf {
    let from = manifest_dir().join("../../qa/fixtures/addons/demo-tools-0.1.0.kcaddon");
    let to = dir.join("demo.kcaddon");
    std::fs::copy(from, &to).unwrap();
    to
}

fn s(p: &Path) -> &str {
    p.to_str().unwrap()
}

/// A trust file with just the test key, as `keygen` prints it.
fn test_pub(dir: &Path) -> PathBuf {
    let key = KeyFile::parse(&std::fs::read_to_string(test_key()).unwrap()).unwrap();
    let path = dir.join("test.pub");
    std::fs::write(&path, key.trusted_key(None).unwrap().toml_block()).unwrap();
    path
}

#[test]
fn keygen_writes_the_private_key_only_where_asked_and_prints_the_public_one() {
    let dir = tempfile::tempdir().unwrap();
    let out_key = dir.path().join("keys/koper-vonk-2026.key");
    let (code, out, err) = cli(&[
        "addons",
        "keygen",
        "--id",
        "koper-vonk-2026",
        "--out",
        s(&out_key),
        "--valid-from",
        "2026-11-01",
    ]);
    assert_eq!(code, 0, "{err}");
    let key = KeyFile::parse(&std::fs::read_to_string(&out_key).unwrap()).unwrap();
    assert_eq!(key.id, "koper-vonk-2026");
    // The public half is printed in both forms; the secret never appears on screen.
    let public = key.public_hex();
    assert!(
        out.contains(&format!(
            "key koper-vonk-2026 {} 2026-11-01 KoperVonk",
            key.public_hex()
        )),
        "{out}"
    );
    assert!(out.contains("[[key]]") && out.contains(&format!("public = \"{public}\"")));
    assert!(out.contains("valid_from = \"2026-11-01\""));
    assert!(out.contains(&format!(
        "(\"koper-vonk-2026\", \"KoperVonk\", \"{public}\"),"
    )));
    let secret = novo_addons::trust::to_hex(&key.seed);
    assert!(!out.contains(&secret) && !err.contains(&secret));
    assert!(!out.contains(&key.to_base64()));
    // a block that keygen printed is accepted by the loader the program uses
    let pub_text =
        std::fs::read_to_string(dir.path().join("keys/koper-vonk-2026.key.pub")).unwrap();
    assert!(
        novo_addons::TrustStore::default()
            .extend_from_toml(&pub_text)
            .is_empty()
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&out_key).unwrap().permissions().mode();
        assert_eq!(mode & 0o077, 0, "group and others have no access: {mode:o}");
    }
    // never overwrites a key
    let (code, _, err) = cli(&[
        "addons",
        "keygen",
        "--id",
        "koper-vonk-2026",
        "--out",
        s(&out_key),
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("never overwrites"), "{err}");
    // two keys are different keys
    let second = dir.path().join("two.key");
    cli(&[
        "addons",
        "keygen",
        "--id",
        "koper-vonk-2027",
        "--out",
        s(&second),
    ]);
    let two = KeyFile::parse(&std::fs::read_to_string(&second).unwrap()).unwrap();
    assert_ne!(two.public_hex(), public);
}

#[test]
fn keygen_refuses_a_path_inside_a_git_work_tree_and_bad_ids() {
    // this crate sits inside the repository's work tree (a .git folder or file above it)
    let inside = manifest_dir().join("tests/keys/should-not-exist.key");
    let (code, _, err) = cli(&[
        "addons",
        "keygen",
        "--id",
        "koper-vonk-2026",
        "--out",
        s(&inside),
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("git work tree"), "{err}");
    assert!(!inside.exists());
    let dir = tempfile::tempdir().unwrap();
    let (code, _, err) = cli(&[
        "addons",
        "keygen",
        "--id",
        "Bad Id",
        "--out",
        s(&dir.path().join("x.key")),
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("key id"), "{err}");
}

#[test]
fn sign_then_verify_and_install_without_the_tick() {
    let dir = tempfile::tempdir().unwrap();
    let file = unsigned_fixture(dir.path());
    let public = test_pub(dir.path());

    // before: not signed
    let (code, out, _) = cli(&["addons", "verify", s(&file), "--keys", s(&public)]);
    assert_eq!(code, 1);
    assert!(out.contains("Unsigned"), "{out}");

    let key = test_key();
    let (code, out, err) = cli(&[
        "addons",
        "sign",
        s(&file),
        "--key",
        s(&key),
        "--date",
        "2026-11-05",
    ]);
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("test-only-addon-key") && out.contains("2026-11-05"),
        "{out}"
    );

    let (code, out, _) = cli(&["addons", "verify", s(&file), "--keys", s(&public)]);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("Signed by TEST ONLY"), "{out}");
    // the wrong key is not the signer
    let other = dir.path().join("other.pub");
    std::fs::write(
        &other,
        novo_addons::trust::TrustedKey::new(
            "test-only-addon-key",
            "Somebody Else",
            &KeyFile::new("other-key", "x", [1u8; 32])
                .unwrap()
                .public_hex(),
        )
        .toml_block(),
    )
    .unwrap();
    let (code, out, _) = cli(&["addons", "verify", s(&file), "--keys", s(&other)]);
    assert_eq!(code, 1);
    assert!(out.contains("does not match"), "{out}");
    // the default trust (built in + this user's file) does not know the test key yet
    let root = dir.path().join("addons");
    let (code, out, _) = cli(&["addons", "verify", s(&file), "--root", s(&root)]);
    assert_eq!(code, 1);
    assert!(out.contains("unknown key"), "{out}");
    // once the person trusts the key, installing needs no tick
    std::fs::create_dir_all(&root).unwrap();
    std::fs::copy(&public, root.join("trusted-keys.toml")).unwrap();
    let (code, out, err) = cli(&["addons", "install", s(&file), "--root", s(&root)]);
    assert_eq!(code, 0, "{err}");
    assert!(out.contains("Installed"), "{out}");
    let (_, list, _) = cli(&["addons", "inspect", s(&file), "--root", s(&root), "--json"]);
    assert!(list.contains("\"state\": \"signed\""), "{list}");
}

#[test]
fn signing_twice_needs_replace_and_the_same_inputs_give_the_same_file() {
    let dir = tempfile::tempdir().unwrap();
    let key = test_key();
    let a = unsigned_fixture(dir.path());
    let b = dir.path().join("b.kcaddon");
    std::fs::copy(&a, &b).unwrap();
    for f in [&a, &b] {
        let (code, _, err) = cli(&[
            "addons",
            "sign",
            s(f),
            "--key",
            s(&key),
            "--date",
            "2026-11-05",
        ]);
        assert_eq!(code, 0, "{err}");
    }
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    let (code, _, err) = cli(&[
        "addons",
        "sign",
        s(&a),
        "--key",
        s(&key),
        "--date",
        "2026-11-05",
    ]);
    assert_ne!(code, 0);
    assert!(err.contains("--replace"), "{err}");
    let (code, _, _) = cli(&[
        "addons",
        "sign",
        s(&a),
        "--key",
        s(&key),
        "--date",
        "2026-11-05",
        "--replace",
    ]);
    assert_eq!(code, 0);
    assert_eq!(std::fs::read(&a).unwrap(), std::fs::read(&b).unwrap());
    // --output leaves the input alone
    let c = unsigned_fixture(dir.path());
    let before = std::fs::read(&c).unwrap();
    let out_file = dir.path().join("signed.kcaddon");
    let (code, _, _) = cli(&[
        "addons",
        "sign",
        s(&c),
        "--key",
        s(&key),
        "-o",
        s(&out_file),
        "--no-date",
    ]);
    assert_eq!(code, 0);
    assert_eq!(std::fs::read(&c).unwrap(), before);
    assert!(out_file.exists());
}

#[test]
fn a_package_tampered_after_signing_does_not_verify() {
    let dir = tempfile::tempdir().unwrap();
    let file = unsigned_fixture(dir.path());
    let public = test_pub(dir.path());
    let key = test_key();
    cli(&["addons", "sign", s(&file), "--key", s(&key), "--no-date"]);
    // swap the manifest for the unsigned original's, keeping the signature entry
    let signed = std::fs::read(&file).unwrap();
    let original =
        std::fs::read(manifest_dir().join("../../qa/fixtures/addons/demo-tools-0.1.0.kcaddon"))
            .unwrap();
    let read = |bytes: &[u8]| {
        novo_addons::Package::read(bytes, &novo_addons::TrustStore::default()).unwrap()
    };
    let (s_pkg, o_pkg) = (read(&signed), read(&original));
    // same files, but a manifest that differs by one comment line, and the old signature
    let mut changed_manifest = o_pkg.manifest_bytes.clone();
    changed_manifest.extend_from_slice(b"\n# edited after signing\n");
    let mut zip_bytes = Vec::new();
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_bytes));
        let opts = zip::write::SimpleFileOptions::default();
        zip.start_file("manifest.toml", opts).unwrap();
        zip.write_all(&changed_manifest).unwrap();
        zip.start_file("signature", opts).unwrap();
        zip.write_all(s_pkg.signature.as_ref().unwrap()).unwrap();
        for (name, data) in &o_pkg.files {
            zip.start_file(name.as_str(), opts).unwrap();
            zip.write_all(data).unwrap();
        }
        zip.finish().unwrap();
    }
    let bad = dir.path().join("bad.kcaddon");
    std::fs::write(&bad, zip_bytes).unwrap();
    let (code, out, _) = cli(&["addons", "verify", s(&bad), "--keys", s(&public)]);
    assert_eq!(code, 1);
    assert!(out.contains("changed after it was signed"), "{out}");
}

#[test]
fn a_key_in_an_environment_variable_signs_without_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let file = unsigned_fixture(dir.path());
    let public = test_pub(dir.path());
    let key = KeyFile::parse(&std::fs::read_to_string(test_key()).unwrap()).unwrap();
    // Setting an environment variable from a test is not safe next to parallel tests, so the
    // signing itself goes through the library call the command uses; the command's own
    // `--key-env` path is exercised below with a variable that is not set.
    let signed = novo_addons::pack::sign_package(
        &std::fs::read(&file).unwrap(),
        &novo_addons::pack::Signer32 {
            key_id: &key.id,
            seed: key.seed,
            date: None,
        },
        false,
    )
    .unwrap();
    std::fs::write(&file, signed).unwrap();
    let (code, _, _) = cli(&["addons", "verify", s(&file), "--keys", s(&public)]);
    assert_eq!(code, 0);
    // and the base64 form a CI secret holds round-trips to the same key
    assert_eq!(KeyFile::from_base64(&key.to_base64()).unwrap(), key);
    let (code, _, err) = cli(&[
        "addons",
        "sign",
        s(&file),
        "--key-env",
        "KOPERCAD_TEST_VARIABLE_THAT_IS_NOT_SET",
        "--replace",
    ]);
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("not set"), "{err}");
}

#[test]
fn no_other_private_key_is_in_the_repository() {
    fn walk(dir: &Path, found: &mut Vec<PathBuf>, depth: usize) {
        if depth > 12 {
            return;
        }
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if matches!(
                name.as_ref(),
                "target" | ".git" | "node_modules" | ".claude"
            ) {
                continue;
            }
            let Ok(meta) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.is_dir() {
                walk(&path, found, depth + 1);
            } else if meta.len() < 4096
                && std::fs::read(&path)
                    .map(|b| b.starts_with(novo_addons::keyfile::KEY_MAGIC.as_bytes()))
                    .unwrap_or(false)
            {
                found.push(path);
            }
        }
    }
    let repo = manifest_dir().join("../..");
    let mut found = Vec::new();
    walk(&repo, &mut found, 0);
    let allowed = test_key().canonicalize().unwrap();
    let others: Vec<_> = found
        .into_iter()
        .filter(|p| p.canonicalize().ok().as_deref() != Some(allowed.as_path()))
        .collect();
    assert!(
        others.is_empty(),
        "a private key is in the repository: {others:?}"
    );
}
