//! `kopercad-cli parts`: bulk import of the catalog files in one folder (the
//! inbox) into the user's parts catalog. Reads only the inbox and writes only
//! the catalog, its `.bak` and `import-report.txt`; no network.

use std::io::Write;
use std::path::{Path, PathBuf};

use clap::{Args, Subcommand};
use novo_parts::{
    InboxOptions, default_inbox, import_inbox, maker_guide, prepare_inbox, user_catalog_path,
};

use crate::{Failure, exit, print};

#[derive(Debug, Args)]
pub(crate) struct Where {
    /// Folder holding the downloaded catalog files, one subfolder per maker.
    /// Default: `inbox` next to the catalog (`%APPDATA%\KoperCAD\inbox`).
    #[arg(long, value_name = "DIR", global = true)]
    inbox: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Parts {
    /// Read every BMEcat, CSV and ZIP in the inbox, check the parts and merge
    /// them into the parts catalog. Safe to run again.
    Import {
        #[command(flatten)]
        at: Where,
        /// The catalog file to update. Default: the user's parts catalog.
        #[arg(long, value_name = "FILE")]
        catalog: Option<PathBuf>,
        /// Read and report only; change nothing.
        #[arg(long)]
        dry_run: bool,
    },
    /// Create the inbox with one folder per maker.
    Prepare {
        #[command(flatten)]
        at: Where,
    },
    /// Which file to download from which maker, and what needs a login.
    Guide {
        #[command(flatten)]
        at: Where,
    },
}

fn inbox(at: &Where) -> Result<PathBuf, Failure> {
    at.inbox
        .clone()
        .or_else(default_inbox)
        .ok_or_else(|| Failure::new(exit::READ, "no profile folder; name one with --inbox"))
}

fn fail(code: i32, e: impl std::fmt::Display) -> Failure {
    Failure::new(code, e.to_string())
}

pub(crate) fn run(cmd: Parts, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Parts::Prepare { at } => {
            let dir = inbox(&at)?;
            let made = prepare_inbox(&dir).map_err(|e| fail(exit::WRITE, e))?;
            print(out, &format!("Inbox: {}", dir.display()))?;
            print(out, &format!("{} new folders.", made.len()))?;
            Ok(exit::OK)
        }
        Parts::Guide { at } => {
            let dir = inbox(&at)?;
            let have: Vec<String> = novo_parts::RECIPES
                .iter()
                .filter(|r| has_files(&dir, r.manufacturer))
                .map(|r| r.manufacturer.to_owned())
                .collect();
            print(out, &format!("Put downloads in: {}\n", dir.display()))?;
            for l in maker_guide(&have) {
                print(out, &l)?;
            }
            Ok(exit::OK)
        }
        Parts::Import {
            at,
            catalog,
            dry_run,
        } => {
            let dir = inbox(&at)?;
            let fresh = !dir.exists();
            prepare_inbox(&dir).map_err(|e| fail(exit::WRITE, e))?;
            let opts = InboxOptions {
                today: today(),
                dry_run,
            };
            let report =
                import_inbox(&dir, catalog.as_deref(), &opts).map_err(|e| fail(exit::READ, e))?;
            let text = report.render();
            print(out, &text)?;
            if !dry_run {
                let target = catalog.or_else(user_catalog_path);
                if let Some(folder) = target.as_deref().and_then(Path::parent) {
                    let _ = std::fs::write(folder.join("import-report.txt"), &text);
                }
            }
            if fresh || report.total_read() == 0 {
                print(
                    out,
                    &format!(
                        "\nNo catalog files were found. Put downloads in {} (run `kopercad-cli parts guide` for where to get them) and run this again.",
                        dir.display()
                    ),
                )?;
                return Ok(exit::PROBLEMS);
            }
            Ok(exit::OK)
        }
    }
}

fn has_files(inbox: &Path, maker: &str) -> bool {
    std::fs::read_dir(inbox)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            novo_parts::recipe_for(&e.file_name().to_string_lossy().replace('-', " "))
                .is_some_and(|r| r.manufacturer == maker)
        })
        .any(|e| {
            std::fs::read_dir(e.path())
                .map(|d| d.flatten().next().is_some())
                .unwrap_or(false)
        })
}

fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}
