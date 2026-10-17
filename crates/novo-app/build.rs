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

/// Sets `KOPERCAD_BUILD_COMMIT`: `KOPERCAD_BUILD_ID` if given, else the CI
/// commit, else the short commit of the checkout, else "unknown".
fn build_commit() {
    println!("cargo:rerun-if-env-changed=KOPERCAD_BUILD_ID");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    let git = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git");
    let head = git.join("HEAD");
    if head.is_file() {
        println!("cargo:rerun-if-changed={}", head.display());
        if let Ok(text) = std::fs::read_to_string(&head)
            && let Some(r) = text.trim().strip_prefix("ref: ")
            && git.join(r).is_file()
        {
            println!("cargo:rerun-if-changed={}", git.join(r).display());
        }
    }
    let short = |s: &str| s.trim().chars().take(12).collect::<String>();
    let commit = std::env::var("KOPERCAD_BUILD_ID")
        .ok()
        .or_else(|| std::env::var("GITHUB_SHA").ok().map(|s| short(&s)))
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            let out = std::process::Command::new("git")
                .args(["rev-parse", "--short=12", "HEAD"])
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .ok()?;
            out.status
                .success()
                .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=KOPERCAD_BUILD_COMMIT={commit}");
}

/// Sets `KOPERCAD_BUILD_TIME` (UTC, to the minute): `KOPERCAD_BUILD_TIME` if
/// given, else the time this build script ran. Tells a tester at a glance how
/// old the exe in front of them is.
fn build_time() {
    println!("cargo:rerun-if-env-changed=KOPERCAD_BUILD_TIME");
    let time = std::env::var("KOPERCAD_BUILD_TIME")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            // Civil date from days since 1970 (Howard Hinnant's algorithm).
            let days = (secs / 86_400) as i64 + 719_468;
            let era = days.div_euclid(146_097);
            let doe = days.rem_euclid(146_097);
            let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let day = doy - (153 * mp + 2) / 5 + 1;
            let month = if mp < 10 { mp + 3 } else { mp - 9 };
            let year = yoe + era * 400 + i64::from(month <= 2);
            let rem = secs % 86_400;
            format!(
                "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
                rem / 3600,
                rem % 3600 / 60
            )
        });
    println!("cargo:rustc-env=KOPERCAD_BUILD_TIME={time}");
}

fn main() {
    build_commit();
    build_time();
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
