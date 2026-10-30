//! Embeds `addons/keys/kopervonk.pub` (the publisher keys and revocations that ship with the
//! program) into the build. The file may be empty or absent: then nothing is embedded and every
//! package from a publisher is "unknown key" until the person adds the key. Adding the real key
//! is therefore a data-only commit.

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    let source = manifest.join("../../addons/keys/kopervonk.pub");
    println!("cargo:rerun-if-changed={}", source.display());
    println!("cargo:rerun-if-changed=build.rs");
    let text = std::fs::read_to_string(&source).unwrap_or_default();
    let out =
        PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets it")).join("embedded-keys.toml");
    std::fs::write(out, text).expect("write the embedded key file");
}
