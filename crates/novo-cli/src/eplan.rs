//! `kopercad-cli eplan`: the user-run "Export from EPLAN" helper. `script`
//! writes the EPLAN script, instructions (English and Dutch) and a self-test
//! file; `check` reads an export folder and says whether the script that made
//! it is current. Nothing here needs EPLAN or any elevation.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Subcommand;
use novo_interop::eplan::export_script::{self as es, Lang};

use crate::{Failure, exit, print};

#[derive(Debug, Subcommand)]
pub(crate) enum Eplan {
    /// Write the EPLAN export script, instructions in English and Dutch
    /// (`README-EN.txt`, `README-NL.txt`) and a synthetic self-test file
    /// into a folder. Load the script in your own EPLAN (Utilities >
    /// Scripts); it needs no administrator rights and no API licence.
    Script {
        /// The folder to write into (created when missing).
        #[arg(short, long)]
        output: PathBuf,
        /// Print the instructions in this language (`en` or `nl`) as well.
        #[arg(long, value_name = "LANG")]
        show: Option<String>,
    },
    /// Check an export folder (or a `.epj` file): the exchange file's
    /// header and the version of the script that wrote it. Exits with 1 when
    /// the script is older or newer than this KoperCAD.
    Check { path: PathBuf },
    /// List the object kinds, element and attribute names of a PXF file with
    /// counts and the shape of values (digits as 0, letters as a), never the
    /// values. Safe to share when the project itself must stay private; it
    /// lets the reader's field table be checked against a real export.
    Survey { path: PathBuf },
}

pub(crate) fn run(cmd: Eplan, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Eplan::Script { output, show } => {
            let lang = match show.as_deref() {
                None => None,
                Some(s) => Some(Lang::parse(s).ok_or_else(|| {
                    Failure::new(exit::USAGE, format!("unknown language {s:?}; use en or nl"))
                })?),
            };
            std::fs::create_dir_all(&output)
                .map_err(|e| Failure::new(exit::WRITE, format!("{}: {e}", output.display())))?;
            for file in es::bundle() {
                let path = output.join(file.name);
                std::fs::write(&path, file.text)
                    .map_err(|e| Failure::new(exit::WRITE, format!("{}: {e}", path.display())))?;
                print(out, &format!("Wrote {}", path.display()))?;
            }
            if let Some(lang) = lang {
                print(out, &es::instructions(lang))?;
            }
            Ok(exit::OK)
        }
        Eplan::Check { path } => check(&path, out),
        Eplan::Survey { path } => {
            let file = std::fs::File::open(&path).map_err(|e| {
                Failure::new(exit::READ, format!("cannot read {}: {e}", path.display()))
            })?;
            let survey = novo_interop::eplan::pxf::survey_pxf(std::io::BufReader::new(file))
                .map_err(|e| {
                    Failure::new(exit::READ, format!("cannot survey {}: {e}", path.display()))
                })?;
            print(out, survey.text().trim_end())?;
            Ok(exit::OK)
        }
    }
}

fn read_head(path: &Path) -> Result<String, Failure> {
    let fail = |e: std::io::Error| Failure::new(exit::READ, format!("{}: {e}", path.display()));
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .map_err(fail)?
        .take(64 * 1024)
        .read_to_end(&mut buf)
        .map_err(fail)?;
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn check(path: &Path, out: &mut dyn Write) -> Result<i32, Failure> {
    let (dir, pxf) = if path.is_dir() {
        (path.to_path_buf(), path.join(es::EXCHANGE_FILE))
    } else {
        (
            path.parent().map(Path::to_path_buf).unwrap_or_default(),
            path.to_path_buf(),
        )
    };
    let head = read_head(&pxf)?;
    let manifest = std::fs::read_to_string(dir.join(es::MANIFEST_FILE)).ok();
    let (check, report) = es::check_exchange(manifest.as_deref(), &head)
        .map_err(|e| Failure::new(exit::READ, format!("{}: {e}", pxf.display())))?;
    print(out, &report.text())?;
    print(out, &check.message())?;
    Ok(if check.is_current() {
        exit::OK
    } else {
        exit::PROBLEMS
    })
}
