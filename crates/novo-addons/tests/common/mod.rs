//! Shared helpers: build packages in memory, plain and hostile.
#![allow(dead_code)]

use std::io::{Cursor, Write};

use ed25519_dalek::{Signer, SigningKey};
use novo_addons::pack::{Signer32, pack_parts};
use novo_addons::trust::{TrustedKey, signature_file, signed_message, to_hex};
use novo_addons::{AppInfo, Store, TrustStore, Version};

pub const SEED: [u8; 32] = [7u8; 32];

pub fn app() -> AppInfo {
    AppInfo {
        version: Version::new(0, 1, 0),
        api_version: 1,
    }
}

pub fn store(dir: &std::path::Path) -> Store {
    Store::new(dir.join("addons"), app())
}

pub fn key() -> TrustedKey {
    let sk = SigningKey::from_bytes(&SEED);
    TrustedKey {
        id: "test-key".into(),
        owner: "Test Publisher".into(),
        public: to_hex(sk.verifying_key().as_bytes()),
    }
}

pub fn trusting_store(dir: &std::path::Path) -> Store {
    let root = dir.join("addons");
    std::fs::create_dir_all(&root).unwrap();
    let k = key();
    std::fs::write(
        root.join("trusted-keys.toml"),
        format!(
            "[[key]]\nid = \"{}\"\nowner = \"{}\"\npublic = \"{}\"\n",
            k.id, k.owner, k.public
        ),
    )
    .unwrap();
    Store::new(root, app())
}

pub fn signer() -> Signer32<'static> {
    Signer32 {
        key_id: "test-key",
        seed: SEED,
    }
}

pub fn manifest(id: &str, version: &str, extra: &str) -> String {
    format!(
        r#"[addon]
id = "{id}"
name = "Demo {id}"
version = "{version}"
publisher = "Tester"
description = "A demo add-on."
requires_app = ">=0.1.0, <0.3"
api_version = 1
permissions = ["menu", "floating-panel", "live-view-card", "rail-item"]

[[panel]]
id = "main"
title = "Main"
slot = "floating"
file = "data/main.panel.toml"

[[command]]
id = "open"
label = "Open demo"
description = "Open the demo panel."
icon = "{id}.thing"
key = "Shift+D"
home = "menu"
menu = "Simulate"
opens = "main"

[[command]]
id = "rail"
label = "Demo on the rail"
home = "rail"
workspaces = ["3d", "schematic"]
opens = "main"

[[contribute]]
kind = "demo-profile"
file = "data/profile.toml"
{extra}"#
    )
}

pub fn icons(id: &str) -> String {
    format!(
        "[[icon]]\nname = \"{id}.thing\"\ntooltip = \"Thing\"\nshape = [{{ kind = \"rect\", x = 5, y = 5, w = 14, h = 14, r = 2 }}]\n"
    )
}

pub fn files(id: &str, payload: &str) -> Vec<(String, Vec<u8>)> {
    vec![
        (
            "data/main.panel.toml".into(),
            b"[[widget]]\nkind = \"label\"\ntext = \"Hello\"\n".to_vec(),
        ),
        ("data/profile.toml".into(), payload.as_bytes().to_vec()),
        ("data/icons.toml".into(), icons(id).into_bytes()),
        ("LICENSE".into(), b"MIT\n".to_vec()),
    ]
}

pub fn package(id: &str, version: &str, payload: &str, signed: bool) -> Vec<u8> {
    pack_parts(
        manifest(id, version, "").as_bytes(),
        &files(id, payload),
        signed.then(signer).as_ref(),
    )
    .unwrap()
}

/// A raw zip: `(name, bytes)` entries, stored with deflate, in the given order.
pub fn raw_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut out));
        let opts = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in entries {
            zip.start_file(*name, opts).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip.finish().unwrap();
    }
    out
}

/// A complete package with a hand-written manifest and `files` table (hashes computed here),
/// for tests that need a malformed shape the packer would refuse.
pub fn raw_package(manifest_without_files: &str, data: &[(&str, &[u8])]) -> Vec<u8> {
    let mut text = manifest_without_files.to_owned();
    text.push_str("\n[files]\n");
    for (name, bytes) in data {
        text.push_str(&format!(
            "{name:?} = \"{}\"\n",
            novo_addons::trust::sha256_hex(bytes)
        ));
    }
    let mut entries: Vec<(&str, &[u8])> = vec![("manifest.toml", text.as_bytes())];
    entries.extend_from_slice(data);
    raw_zip(&entries)
}

pub fn sign_manually(manifest: &[u8]) -> String {
    let sk = SigningKey::from_bytes(&SEED);
    signature_file("test-key", &sk.sign(&signed_message(manifest)).to_bytes())
}

pub fn empty_trust() -> TrustStore {
    TrustStore::default()
}

/// Everything under `dir`, relative, for "nothing was written" checks.
pub fn listing(dir: &std::path::Path) -> Vec<String> {
    fn walk(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            out.push(
                p.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
            if p.is_dir() {
                walk(root, &p, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}
