//! Embeds the lessons into the program, and the application icon, version
//! info and a no-elevation manifest into the Windows executable.

use std::fmt::Write as _;
use std::path::Path;

/// Writes `lessons_embedded.rs`: every `.toml` and `.json` file under
/// `assets/lessons`, by its path relative to that folder, so the program
/// carries its lessons and needs no folder next to the exe.
fn embed_lessons() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons");
    println!("cargo:rerun-if-changed={}", root.display());
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    files.sort();
    let mut out = String::from("/// The lessons shipped with the program.\n");
    out.push_str("pub(crate) static EMBEDDED_LESSONS: &[(&str, &str)] = &[\n");
    for (key, path) in files {
        let _ = writeln!(out, "    ({key:?}, include_str!({path:?})),");
    }
    out.push_str("];\n");
    let dest = Path::new(&std::env::var("OUT_DIR").expect("OUT_DIR")).join("lessons_embedded.rs");
    std::fs::write(dest, out).expect("could not write the embedded lessons");
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, out);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("toml" | "json")
        ) && let Ok(rel) = path.strip_prefix(root)
        {
            let key = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            out.push((key, path.to_string_lossy().into_owned()));
        }
    }
}

fn main() {
    embed_lessons();
    println!("cargo:rerun-if-changed=../../assets/branding/icon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/branding/icon.ico");
    // Never ask for elevation: KoperCAD must run without admin rights.
    res.set_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
<trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
  <security>
    <requestedPrivileges>
      <requestedExecutionLevel level="asInvoker" uiAccess="false" />
    </requestedPrivileges>
  </security>
</trustInfo>
</assembly>"#,
    );
    // Fail the build rather than ship an exe without the asInvoker manifest.
    res.compile()
        .expect("could not embed the Windows icon and manifest");
}
