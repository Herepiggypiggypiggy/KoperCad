//! Building a `.kcaddon` from a folder: `kopercad-cli addons pack <folder>`. The folder holds
//! `manifest.toml` (without a `[files]` table), the data files and optionally `engine.wasm`.
//! The packer lists and hashes every file, appends `[files]`, optionally signs the manifest and
//! writes a zip with sorted names and fixed timestamps, so the same folder always gives the same
//! package.

use std::io::{Cursor, Write};
use std::path::Path;

use ed25519_dalek::{Signer, SigningKey};

use crate::error::{AddonError, Result};
use crate::manifest::check_path;
use crate::trust::{sha256_hex, signature_file, signed_message};

/// How to sign: a key id (as in the trusted-keys list) and the 32-byte Ed25519 seed.
pub struct Signer32<'a> {
    pub key_id: &'a str,
    pub seed: [u8; 32],
}

/// Packs `folder`. Returns the bytes of the `.kcaddon`.
pub fn pack_dir(folder: &Path, signer: Option<&Signer32<'_>>) -> Result<Vec<u8>> {
    let mut files = Vec::new();
    collect(folder, folder, &mut files, 0)?;
    files.sort();
    let mut manifest = None;
    let mut data = Vec::new();
    for rel in &files {
        let bytes = std::fs::read(folder.join(rel))?;
        match rel.as_str() {
            "manifest.toml" => manifest = Some(bytes),
            "signature" => {
                return Err(AddonError::Manifest(
                    "remove the file `signature` from the folder; the packer writes it".into(),
                ));
            }
            _ => data.push((rel.clone(), bytes)),
        }
    }
    let manifest = manifest.ok_or_else(|| AddonError::Manifest("the folder has no manifest.toml".into()))?;
    pack_parts(&manifest, &data, signer)
}

/// The same from memory: `manifest` without `[files]`, plus the data files.
pub fn pack_parts(
    manifest: &[u8],
    data: &[(String, Vec<u8>)],
    signer: Option<&Signer32<'_>>,
) -> Result<Vec<u8>> {
    let mut text = String::from_utf8(manifest.to_vec())
        .map_err(|_| AddonError::Manifest("manifest.toml is not UTF-8 text".into()))?;
    if toml::from_str::<toml::Table>(&text)
        .map_err(|e| AddonError::Manifest(e.to_string()))?
        .contains_key("files")
    {
        return Err(AddonError::Manifest(
            "remove the [files] table from manifest.toml; the packer writes it".into(),
        ));
    }
    let mut sorted: Vec<&(String, Vec<u8>)> = data.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str("\n[files]\n");
    for (name, bytes) in &sorted {
        check_path(name)?;
        text.push_str(&format!("{name:?} = \"{}\"\n", sha256_hex(bytes)));
    }
    let manifest_bytes = text.into_bytes();

    let mut out = Vec::new();
    {
        let mut zip = zip::ZipWriter::new(Cursor::new(&mut out));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut put = |name: &str, bytes: &[u8]| -> Result<()> {
            zip.start_file(name, options)
                .map_err(|e| AddonError::Io(e.to_string()))?;
            zip.write_all(bytes)?;
            Ok(())
        };
        put("manifest.toml", &manifest_bytes)?;
        if let Some(s) = signer {
            let sk = SigningKey::from_bytes(&s.seed);
            let sig = sk.sign(&signed_message(&manifest_bytes));
            put("signature", signature_file(s.key_id, &sig.to_bytes()).as_bytes())?;
        }
        for (name, bytes) in sorted {
            put(name, bytes)?;
        }
        zip.finish().map_err(|e| AddonError::Io(e.to_string()))?;
    }
    Ok(out)
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>, depth: usize) -> Result<()> {
    if depth > 8 {
        return Err(AddonError::UnsafePath("the folder nests deeper than 8 levels".into()));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let meta = std::fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            return Err(AddonError::UnsafePath(format!("{}: a symbolic link", path.display())));
        }
        if meta.is_dir() {
            collect(root, &path, out, depth + 1)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .map_err(|e| AddonError::Io(e.to_string()))?
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            check_path(&rel)?;
            out.push(rel);
        }
    }
    Ok(())
}
