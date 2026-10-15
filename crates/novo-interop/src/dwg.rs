//! DWG through a free converter, then the DXF reader.
//!
//! DWG is a closed format: there is no public specification, and the library
//! that reads it for the big CAD vendors is licensed per product. KoperCAD
//! therefore never links, bundles or reimplements a DWG reader. Instead the
//! user keeps a free converter next to the program and this module runs it as
//! a separate process:
//!
//! - **LibreDWG** `dwg2dxf` (GNU, open source; a portable zip needs no
//!   installer and no admin rights), or
//! - the **ODA File Converter** (free of charge from the Open Design
//!   Alliance, under their own terms, which the user accepts when installing).
//!
//! The converter writes an ASCII DXF, and everything after that is the normal
//! DXF import (`dxf::read_dxf`), with its counted list of what was skipped.
//! Running another program as a process keeps the converters' licences out of
//! KoperCAD's: nothing of theirs is distributed here.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use thiserror::Error;

/// The converters KoperCAD can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConverterKind {
    /// LibreDWG's `dwg2dxf`: one file in, one DXF out.
    LibreDwg,
    /// The ODA File Converter: works on folders, so a filter names the file.
    Oda,
}

impl ConverterKind {
    pub fn label(self) -> &'static str {
        match self {
            ConverterKind::LibreDwg => "LibreDWG (dwg2dxf)",
            ConverterKind::Oda => "ODA File Converter",
        }
    }

    /// The program's file names to look for, without folder.
    fn file_names(self) -> &'static [&'static str] {
        match self {
            ConverterKind::LibreDwg => &["dwg2dxf.exe", "dwg2dxf"],
            ConverterKind::Oda => &["ODAFileConverter.exe", "ODAFileConverter"],
        }
    }

    /// Guesses the kind from a program's file name (a path the user chose).
    pub fn of_program(path: &Path) -> Option<ConverterKind> {
        // Either separator, whatever system this runs on.
        let text = path.to_string_lossy().to_ascii_lowercase();
        let name = text.rsplit(['/', '\\']).next()?;
        let name = name.trim_end_matches(".exe");
        match name {
            "dwg2dxf" => Some(ConverterKind::LibreDwg),
            "odafileconverter" => Some(ConverterKind::Oda),
            _ => None,
        }
    }
}

/// A converter found on this computer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converter {
    pub kind: ConverterKind,
    pub program: PathBuf,
}

#[derive(Debug, Error)]
pub enum DwgError {
    #[error("this is not a DWG file (it does not start with a DWG version mark)")]
    NotDwg,
    #[error(
        "this DWG is {0}, older than AutoCAD R13; save it as a newer DWG or as DXF in the program that made it"
    )]
    TooOld(&'static str),
    #[error("no DWG converter was found; {}", guidance())]
    NoConverter,
    #[error("the converter could not be started: {0}")]
    Start(String),
    #[error("the converter did not finish within {0} seconds and was stopped")]
    TimedOut(u64),
    #[error("the converter failed: {0}")]
    Failed(String),
    #[error("the converter finished but wrote no DXF file")]
    NoOutput,
}

/// The DWG release a file was saved by, from its first six bytes
/// (`AC1015`...). `None` for anything that is not a DWG.
pub fn dwg_version(head: &[u8]) -> Option<&'static str> {
    let mark = head.get(..6)?;
    Some(match mark {
        b"AC1012" => "AutoCAD R13",
        b"AC1014" => "AutoCAD R14",
        b"AC1015" => "AutoCAD 2000",
        b"AC1018" => "AutoCAD 2004",
        b"AC1021" => "AutoCAD 2007",
        b"AC1024" => "AutoCAD 2010",
        b"AC1027" => "AutoCAD 2013",
        b"AC1032" => "AutoCAD 2018",
        b"AC1037" => "AutoCAD 2024",
        b"AC1009" | b"AC1006" | b"AC1004" | b"AC1003" | b"AC1002" => {
            "an AutoCAD release before R13"
        }
        _ if mark.starts_with(b"AC") && mark[2..].iter().all(u8::is_ascii_digit) => {
            "a newer or unknown AutoCAD release"
        }
        _ => return None,
    })
}

/// Checks a DWG file's head before a converter is started.
pub fn check_dwg(head: &[u8]) -> Result<&'static str, DwgError> {
    match dwg_version(head) {
        None => Err(DwgError::NotDwg),
        Some(v) if v.contains("before R13") => Err(DwgError::TooOld(v)),
        Some(v) => Ok(v),
    }
}

/// What to tell a user who has no converter: where to get one without admin
/// rights and where KoperCAD looks for it.
pub fn guidance() -> String {
    "get the free LibreDWG tools (a portable zip with dwg2dxf) or the ODA File Converter, \
     unpack LibreDWG into %LOCALAPPDATA%\\KoperCAD\\tools\\libredwg, or choose the program with \
     \"Choose converter\""
        .to_string()
}

/// Folders searched, in order: the user's own tools folder first, then the
/// folders on `PATH`, then the usual per-user and per-machine install places
/// of the ODA File Converter. Nothing is created or written.
pub fn search_folders(
    local_app_data: Option<&Path>,
    path_var: Option<&OsString>,
    program_files: &[PathBuf],
) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(local) = local_app_data {
        let tools = local.join("KoperCAD").join("tools");
        out.push(tools.clone());
        if let Ok(read) = std::fs::read_dir(&tools) {
            let mut subs: Vec<PathBuf> = read
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            subs.sort();
            for s in subs {
                out.push(s.join("bin"));
                out.push(s);
            }
        }
        out.push(local.join("Programs").join("ODA"));
    }
    if let Some(path) = path_var {
        out.extend(std::env::split_paths(path));
    }
    for pf in program_files {
        if let Ok(read) = std::fs::read_dir(pf.join("ODA")) {
            let mut subs: Vec<PathBuf> = read
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect();
            subs.sort();
            out.extend(subs);
        }
    }
    out
}

/// The first converter in `folders` (LibreDWG before ODA: it needs no
/// installer). `chosen` is a program the user picked, which wins.
pub fn find_converter(chosen: Option<&Path>, folders: &[PathBuf]) -> Option<Converter> {
    if let Some(program) = chosen.filter(|p| p.is_file())
        && let Some(kind) = ConverterKind::of_program(program)
    {
        return Some(Converter {
            kind,
            program: program.to_path_buf(),
        });
    }
    for kind in [ConverterKind::LibreDwg, ConverterKind::Oda] {
        for dir in folders {
            for name in kind.file_names() {
                let program = dir.join(name);
                if program.is_file() {
                    return Some(Converter { kind, program });
                }
            }
        }
    }
    None
}

/// The converter for this computer, from the environment.
pub fn find_installed(chosen: Option<&Path>) -> Option<Converter> {
    let local = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    let path = std::env::var_os("PATH");
    let program_files: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)"]
        .iter()
        .filter_map(|v| std::env::var_os(v).map(PathBuf::from))
        .collect();
    find_converter(
        chosen,
        &search_folders(local.as_deref(), path.as_ref(), &program_files),
    )
}

impl Converter {
    /// The command that converts `dwg` to an ASCII DXF in `out_dir`. The
    /// output file is `out_dir/<name>.dxf` for both converters.
    pub fn command(&self, dwg: &Path, out_dir: &Path) -> Command {
        let stem = dwg
            .file_stem()
            .map(|s| s.to_os_string())
            .unwrap_or_default();
        let mut cmd = Command::new(&self.program);
        match self.kind {
            ConverterKind::LibreDwg => {
                let mut out = out_dir.join(&stem);
                out.set_extension("dxf");
                cmd.arg("-y").arg("-o").arg(out).arg(dwg);
            }
            ConverterKind::Oda => {
                let dir = dwg.parent().filter(|p| !p.as_os_str().is_empty());
                let filter = dwg
                    .file_name()
                    .map(|n| n.to_os_string())
                    .unwrap_or_default();
                cmd.arg(dir.unwrap_or_else(|| Path::new(".")))
                    .arg(out_dir)
                    .arg("ACAD2013")
                    .arg("DXF")
                    .arg("0")
                    .arg("1")
                    .arg(filter);
            }
        }
        cmd
    }

    /// Converts `dwg` to DXF in `out_dir` and returns the DXF path. Stops the
    /// converter after `timeout`.
    pub fn convert(
        &self,
        dwg: &Path,
        out_dir: &Path,
        timeout: Duration,
    ) -> Result<PathBuf, DwgError> {
        let mut head = [0u8; 6];
        {
            use std::io::Read;
            let mut f = std::fs::File::open(dwg)
                .map_err(|e| DwgError::Start(format!("{}: {e}", dwg.display())))?;
            let n = f.read(&mut head).unwrap_or(0);
            check_dwg(&head[..n])?;
        }
        std::fs::create_dir_all(out_dir).map_err(|e| DwgError::Start(e.to_string()))?;
        let mut expected = out_dir.join(dwg.file_stem().unwrap_or_default());
        expected.set_extension("dxf");
        // A stale file from an earlier run must not pass for this run's output.
        let _ = std::fs::remove_file(&expected);

        let mut cmd = self.command(dwg, out_dir);
        cmd.stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // No console window flashing up behind the app.
            cmd.creation_flags(0x0800_0000);
        }
        let mut child = cmd.spawn().map_err(|e| DwgError::Start(e.to_string()))?;
        // Drained while the converter runs: a chatty one would otherwise fill
        // the pipe and stall until the timeout. Only the first 4 KB is kept.
        let drain = child.stderr.take().map(|mut s| {
            std::thread::spawn(move || {
                use std::io::Read;
                let mut kept = Vec::new();
                let _ = s.by_ref().take(4096).read_to_end(&mut kept);
                let _ = std::io::copy(&mut s, &mut std::io::sink());
                kept
            })
        });
        let start = Instant::now();
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if start.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(DwgError::TimedOut(timeout.as_secs()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(40)),
                Err(e) => return Err(DwgError::Start(e.to_string())),
            }
        };
        let err = drain
            .and_then(|h| h.join().ok())
            .map(|buf| String::from_utf8_lossy(&buf).trim().to_string())
            .unwrap_or_default();
        if !status.success() {
            return Err(DwgError::Failed(if err.is_empty() {
                format!(
                    "exit code {}",
                    status.code().map_or("?".into(), |c| c.to_string())
                )
            } else {
                err
            }));
        }
        if expected.is_file() {
            return Ok(expected);
        }
        // The ODA converter keeps the input's name but may change its case.
        if let Ok(read) = std::fs::read_dir(out_dir) {
            for e in read.filter_map(Result::ok) {
                let p = e.path();
                let lower = |p: &Path| {
                    p.file_stem()
                        .map(|s| s.to_string_lossy().to_ascii_lowercase())
                };
                let same_stem = lower(&p) == lower(&expected);
                if same_stem && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("dxf")) {
                    return Ok(p);
                }
            }
        }
        Err(DwgError::NoOutput)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_read_from_the_head() {
        assert_eq!(dwg_version(b"AC1032\0\0"), Some("AutoCAD 2018"));
        assert_eq!(dwg_version(b"AC1015"), Some("AutoCAD 2000"));
        assert_eq!(
            dwg_version(b"AC9999"),
            Some("a newer or unknown AutoCAD release")
        );
        assert_eq!(dwg_version(b"%PDF-1"), None);
        assert_eq!(dwg_version(b"AC10"), None);
    }

    #[test]
    fn old_and_foreign_files_are_refused_before_a_converter_runs() {
        assert!(matches!(check_dwg(b"AC1009"), Err(DwgError::TooOld(_))));
        assert!(matches!(check_dwg(b"hello!"), Err(DwgError::NotDwg)));
        assert!(check_dwg(b"AC1027").is_ok());
    }

    #[test]
    fn programs_are_recognised_by_name() {
        assert_eq!(
            ConverterKind::of_program(Path::new("C:\\x\\dwg2dxf.exe")),
            Some(ConverterKind::LibreDwg)
        );
        assert_eq!(
            ConverterKind::of_program(Path::new("/opt/ODAFileConverter")),
            Some(ConverterKind::Oda)
        );
        assert_eq!(ConverterKind::of_program(Path::new("notepad.exe")), None);
    }

    #[test]
    fn commands_match_each_converters_command_line() {
        let l = Converter {
            kind: ConverterKind::LibreDwg,
            program: "dwg2dxf".into(),
        };
        let args: Vec<_> = l
            .command(Path::new("/in/plan.dwg"), Path::new("/out"))
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["-y", "-o", "/out/plan.dxf", "/in/plan.dwg"]);
        let o = Converter {
            kind: ConverterKind::Oda,
            program: "ODAFileConverter".into(),
        };
        let args: Vec<_> = o
            .command(Path::new("/in/plan.dwg"), Path::new("/out"))
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            args,
            ["/in", "/out", "ACAD2013", "DXF", "0", "1", "plan.dwg"]
        );
    }

    #[test]
    fn the_users_tools_folder_comes_first_and_libredwg_before_oda() {
        let dir = tempfile::tempdir().unwrap();
        let tools = dir
            .path()
            .join("KoperCAD")
            .join("tools")
            .join("libredwg-0.13")
            .join("bin");
        std::fs::create_dir_all(&tools).unwrap();
        std::fs::write(tools.join("dwg2dxf.exe"), b"x").unwrap();
        let other = dir.path().join("elsewhere");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("ODAFileConverter.exe"), b"x").unwrap();
        let folders = search_folders(Some(dir.path()), Some(&other.clone().into_os_string()), &[]);
        let found = find_converter(None, &folders).unwrap();
        assert_eq!(found.kind, ConverterKind::LibreDwg);
        assert!(found.program.starts_with(dir.path().join("KoperCAD")));
        // Without the user's tools folder the ODA converter on PATH is found.
        let folders = search_folders(None, Some(&other.into_os_string()), &[]);
        assert_eq!(
            find_converter(None, &folders).unwrap().kind,
            ConverterKind::Oda
        );
        assert!(find_converter(None, &[]).is_none());
    }

    #[cfg(unix)]
    #[test]
    fn a_converter_runs_as_a_separate_process_and_its_dxf_is_returned() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        // A stand-in dwg2dxf: writes the file named after -o.
        let fake = dir.path().join("dwg2dxf");
        std::fs::write(
            &fake,
            "#!/bin/sh\nwhile [ \"$1\" != \"-o\" ]; do shift; done\nprintf '0\\nSECTION\\n2\\nENTITIES\\n0\\nENDSEC\\n0\\nEOF\\n' > \"$2\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dwg = dir.path().join("plan.dwg");
        std::fs::write(&dwg, b"AC1027\0\0\0\0").unwrap();
        let conv = find_converter(Some(&fake), &[]).unwrap();
        let out = conv
            .convert(&dwg, &dir.path().join("out"), Duration::from_secs(20))
            .unwrap();
        assert_eq!(out.file_name().unwrap(), "plan.dxf");
        assert!(std::fs::read_to_string(out).unwrap().contains("ENTITIES"));

        // A program that fails is reported with its own words.
        let bad = dir.path().join("bad").join("dwg2dxf");
        std::fs::create_dir_all(bad.parent().unwrap()).unwrap();
        std::fs::write(&bad, "#!/bin/sh\necho 'corrupt file' >&2\nexit 3\n").unwrap();
        std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o755)).unwrap();
        let conv = find_converter(Some(&bad), &[]).unwrap();
        let e = conv
            .convert(&dwg, &dir.path().join("out2"), Duration::from_secs(20))
            .unwrap_err();
        assert!(e.to_string().contains("corrupt file"), "{e}");

        // A hung program is stopped.
        let slow = dir.path().join("slow").join("dwg2dxf");
        std::fs::create_dir_all(slow.parent().unwrap()).unwrap();
        std::fs::write(&slow, "#!/bin/sh\nsleep 30\n").unwrap();
        std::fs::set_permissions(&slow, std::fs::Permissions::from_mode(0o755)).unwrap();
        let conv = find_converter(Some(&slow), &[]).unwrap();
        let e = conv
            .convert(&dwg, &dir.path().join("out3"), Duration::from_millis(300))
            .unwrap_err();
        assert!(matches!(e, DwgError::TimedOut(_)));
    }
}
