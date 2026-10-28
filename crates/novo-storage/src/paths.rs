//! Checks on a path before anything is opened or written there.

use std::path::Path;

/// Whether any part of `path` is a name Windows reserves for a device
/// (`CON`, `PRN`, `AUX`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`, `CONIN$`,
/// `CONOUT$`), with or without an extension and in any case. Opening one
/// does not make a file: `CON` waits for keyboard input forever and `NUL`
/// swallows what is written. Checked on every platform, so a project or
/// export made on Linux can still be copied to Windows.
pub fn reserved_device_name(path: &Path) -> bool {
    path.components().any(|part| {
        let std::path::Component::Normal(name) = part else {
            return false;
        };
        let name = name.to_string_lossy();
        let stem = name.split('.').next().unwrap_or("").trim_end_matches(' ');
        let stem = stem.to_ascii_uppercase();
        matches!(
            stem.as_str(),
            "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
        ) || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
    })
}

/// Whether `path` is too long for Windows' ordinary file calls (260 with the
/// end mark). `extra` counts characters a temporary name will add. Always
/// false off Windows and for `\\?\` paths, which have no such limit.
pub fn too_long_for_windows(path: &Path, extra: usize) -> bool {
    if !cfg!(windows) {
        return false;
    }
    let text = path.to_string_lossy();
    !text.starts_with(r"\\?\") && text.chars().count() + extra >= 260
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn device_names_are_found_in_any_spelling() {
        for name in [
            "CON",
            "con",
            "CON.pdf",
            "out/CON.csv",
            "NUL.kopercad",
            "prn",
            "Aux.x",
            "COM1",
            "lpt9.txt",
            "CONIN$",
            "con .txt",
            "dir/con/x.pdf",
        ] {
            assert!(reserved_device_name(Path::new(name)), "{name}");
        }
        for name in [
            "console.pdf",
            "COM0",
            "COM10",
            "LPT",
            "a/b/c.pdf",
            "nul2",
            "xcon",
            "",
        ] {
            assert!(!reserved_device_name(Path::new(name)), "{name}");
        }
    }
}
