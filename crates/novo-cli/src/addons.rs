//! `kopercad-cli addons`: the add-on manager from the command line, for scripts and for
//! building packages in CI. It reads and writes only the add-on folder
//! (`%LOCALAPPDATA%\KoperCAD\addons`, or `--root`) and the files it is given. No admin rights,
//! no network.

use std::io::Write;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use novo_addons::pack::{Signer32, pack_dir};
use novo_addons::report::{inspection_json, set_json};
use novo_addons::trust::from_hex;
use novo_addons::{AppInfo, InstallOptions, Store, Version, default_root};

use crate::{Failure, exit, print};

#[derive(Debug, Args)]
pub(crate) struct RootArg {
    /// The add-on folder. Default: the per-user folder, `%LOCALAPPDATA%\KoperCAD\addons`.
    #[arg(long, value_name = "DIR", global = true)]
    root: Option<PathBuf>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Addons {
    /// List what is installed, with its state.
    List {
        #[command(flatten)]
        root: RootArg,
        /// Print JSON instead of text.
        #[arg(long)]
        json: bool,
    },
    /// Read a `.kcaddon` and say what installing it would do; changes nothing.
    Inspect {
        #[command(flatten)]
        root: RootArg,
        file: PathBuf,
        #[arg(long)]
        json: bool,
    },
    /// Install (or update) from a `.kcaddon` file. A file not signed by a trusted publisher
    /// needs `--trust-unsigned`, the same tick as in the program.
    Install {
        #[command(flatten)]
        root: RootArg,
        file: PathBuf,
        /// Install even though no trusted publisher signed the file.
        #[arg(long)]
        trust_unsigned: bool,
        /// Install it switched off.
        #[arg(long)]
        disabled: bool,
    },
    /// Turn an add-on on.
    Enable {
        #[command(flatten)]
        root: RootArg,
        id: String,
    },
    /// Turn an add-on off; its files stay.
    Disable {
        #[command(flatten)]
        root: RootArg,
        id: String,
    },
    /// Remove an add-on and all its versions.
    Remove {
        #[command(flatten)]
        root: RootArg,
        id: String,
    },
    /// Go back to the previous version.
    Rollback {
        #[command(flatten)]
        root: RootArg,
        id: String,
    },
    /// Build a `.kcaddon` from a folder holding `manifest.toml` (without a `[files]` table) and
    /// the add-on's files. The same folder always gives the same file.
    Pack {
        /// The folder.
        folder: PathBuf,
        /// The file to write.
        #[arg(short, long)]
        output: PathBuf,
        /// Sign with this key id (as listed in trusted-keys.toml) ...
        #[arg(long, requires = "seed_file")]
        key_id: Option<String>,
        /// ... using the 32-byte Ed25519 seed in this file, as 64 hex digits.
        #[arg(long, requires = "key_id")]
        seed_file: Option<PathBuf>,
    },
}

fn store(root: &RootArg) -> Result<Store, Failure> {
    let dir = match &root.root {
        Some(d) => d.clone(),
        None => novo_storage::profile::data_local_dir()
            .map(|d| default_root(&d))
            .ok_or_else(|| {
                Failure::new(
                    exit::READ,
                    "no profile folder on this computer; name one with --root",
                )
            })?,
    };
    Ok(Store::new(
        dir,
        AppInfo {
            version: Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or_default(),
            api_version: novo_addons::API_VERSION,
        },
    ))
}

fn fail(e: novo_addons::AddonError) -> Failure {
    Failure::new(exit::READ, e.to_string())
}

pub(crate) fn run(cmd: Addons, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Addons::List { root, json } => {
            let set = store(&root)?.scan();
            if json {
                print(
                    out,
                    &serde_json::to_string_pretty(&set_json(&set)).unwrap_or_default(),
                )?;
            } else {
                for e in &set.entries {
                    let state = match &e.status {
                        novo_addons::Status::Enabled => "on".to_owned(),
                        novo_addons::Status::Disabled => "off".to_owned(),
                        novo_addons::Status::Incompatible(why) => {
                            format!("needs another version: {why}")
                        }
                        novo_addons::Status::Failed(why) => format!("failed: {why}"),
                    };
                    print(
                        out,
                        &format!("{}\t{}\t{}\t{state}", e.id, e.active, e.display_name()),
                    )?;
                }
                if set.entries.is_empty() {
                    print(out, "No add-ons installed.")?;
                }
                for w in &set.warnings {
                    print(out, &format!("warning: {w}"))?;
                }
            }
            Ok(exit::OK)
        }
        Addons::Inspect { root, file, json } => {
            let i = store(&root)?.inspect_file(&file).map_err(fail)?;
            if json {
                print(
                    out,
                    &serde_json::to_string_pretty(&inspection_json(&i)).unwrap_or_default(),
                )?;
            } else {
                let a = &i.package.manifest.addon;
                print(out, &format!("{} {} by {}", a.name, a.version, a.publisher))?;
                print(out, &format!("fingerprint {}", i.package.fingerprint))?;
                let perms: Vec<&str> = a.permissions.iter().map(|p| p.key()).collect();
                print(
                    out,
                    &format!(
                        "permissions: {}",
                        if perms.is_empty() {
                            "none".into()
                        } else {
                            perms.join(", ")
                        }
                    ),
                )?;
                print(
                    out,
                    &if i.package.trust.is_signed() {
                        "signed by a trusted publisher".to_owned()
                    } else {
                        "unsigned, from file".to_owned()
                    },
                )?;
                match i.blocked() {
                    Some(why) => print(out, &format!("cannot be installed: {why}"))?,
                    None => print(out, "can be installed")?,
                }
            }
            Ok(exit::OK)
        }
        Addons::Install {
            root,
            file,
            trust_unsigned,
            disabled,
        } => {
            let i = store(&root)?
                .install_file(
                    &file,
                    InstallOptions {
                        accept_unsigned: trust_unsigned,
                        enable: !disabled,
                    },
                )
                .map_err(fail)?;
            let a = &i.package.manifest.addon;
            print(out, &format!("Installed {} {}", a.name, a.version))?;
            Ok(exit::OK)
        }
        Addons::Enable { root, id } => {
            store(&root)?.set_enabled(&id, true).map_err(fail)?;
            print(out, &format!("{id} is on"))?;
            Ok(exit::OK)
        }
        Addons::Disable { root, id } => {
            store(&root)?.set_enabled(&id, false).map_err(fail)?;
            print(out, &format!("{id} is off"))?;
            Ok(exit::OK)
        }
        Addons::Remove { root, id } => {
            store(&root)?.remove(&id).map_err(fail)?;
            print(out, &format!("{id} removed"))?;
            Ok(exit::OK)
        }
        Addons::Rollback { root, id } => {
            let v = store(&root)?.roll_back(&id).map_err(fail)?;
            print(out, &format!("{id} is back at {v}"))?;
            Ok(exit::OK)
        }
        Addons::Pack {
            folder,
            output,
            key_id,
            seed_file,
        } => {
            let seed = match &seed_file {
                Some(path) => {
                    let text = std::fs::read_to_string(path).map_err(|e| {
                        Failure::new(exit::READ, format!("{}: {e}", path.display()))
                    })?;
                    let bytes = from_hex(text.trim())
                        .and_then(|v| <[u8; 32]>::try_from(v).ok())
                        .ok_or_else(|| {
                            Failure::new(exit::READ, "the seed file must hold 64 hex digits")
                        })?;
                    Some(bytes)
                }
                None => None,
            };
            let signer = match (&key_id, seed) {
                (Some(id), Some(seed)) => Some(Signer32 { key_id: id, seed }),
                _ => None,
            };
            let bytes = pack_dir(&folder, signer.as_ref()).map_err(fail)?;
            std::fs::write(&output, &bytes)
                .map_err(|e| Failure::new(exit::WRITE, format!("{}: {e}", output.display())))?;
            print(
                out,
                &format!("Wrote {} ({} bytes)", output.display(), bytes.len()),
            )?;
            Ok(exit::OK)
        }
    }
}
