//! The in-app supplier browser: a separate KoperCAD window (same program,
//! started with `--supplier-browser <id>`) that shows a supplier's site with
//! the Windows WebView2 runtime. The user signs in and downloads themselves;
//! the window only decides where a download is written (the supplier's
//! private landing folder, see `novo_parts::supplier_site`) and which file
//! kinds are kept. The Parts window watches the landing folder and takes the
//! file in, so nothing goes through Explorer.
//!
//! Rules kept here, and checked by a test over the host source:
//! * no page script is injected and no message from a page is read: the
//!   window never scrapes, fills in, or clicks anything;
//! * KoperCAD stores no password; the site's own cookies live in the
//!   supplier's profile folder under the user profile (`forget_profile`);
//! * only http and https pages open; pop-ups open in the same window.
//!
//! A separate process because the web view needs its own event loop and a
//! crash or hang of a supplier's page must not take the drawing with it.
//! Where WebView2 is missing, or on other systems, [`launch`] says so and
//! the Parts window opens the normal browser and watches Downloads instead.

use std::path::PathBuf;
use std::process::{Child, Command};

use novo_parts::supplier_site::{self, Supplier};

#[cfg(windows)]
mod host;

/// The command-line flag that starts the supplier browser window.
pub const FLAG: &str = "--supplier-browser";
/// Exit code of the window when the WebView2 runtime is not installed.
pub const EXIT_NO_RUNTIME: i32 = 3;
/// Exit code for any other failure to open the window.
pub const EXIT_FAILED: i32 = 4;

/// What the command line asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserArgs {
    pub supplier: String,
    pub order: Option<String>,
}

/// Reads `--supplier-browser <id> [--order <number>]` from the arguments
/// after the program name.
pub fn parse_args(args: &[String]) -> Option<BrowserArgs> {
    let at = args.iter().position(|a| a == FLAG)?;
    let supplier = args.get(at + 1).filter(|s| !s.starts_with("--"))?.clone();
    let order = args
        .iter()
        .position(|a| a == "--order")
        .and_then(|i| args.get(i + 1))
        .filter(|s| !s.is_empty())
        .cloned();
    Some(BrowserArgs { supplier, order })
}

/// Whether the web view runtime can be used on this computer. WebView2 is
/// part of Windows 11 and of current Windows 10 with Edge; it installs per
/// user, no administrator rights.
pub fn runtime_available() -> bool {
    #[cfg(windows)]
    {
        host::runtime_available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Called first in `main`: runs the browser window when asked, and returns
/// the exit code; `None` when this is a normal start.
pub fn run_if_requested() -> Option<i32> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let wanted = parse_args(&args)?;
    Some(run(&wanted))
}

fn run(wanted: &BrowserArgs) -> i32 {
    let Some(root) = supplier_site::browser_root() else {
        return EXIT_FAILED;
    };
    let (list, _) = supplier_site::load(Some(&supplier_site::user_list_path(&root)));
    let Some(supplier) = list.iter().find(|s| s.id == wanted.supplier) else {
        return EXIT_FAILED;
    };
    let url = supplier.open_url(wanted.order.as_deref());
    let (Ok(profile), Ok(landing)) = (
        supplier_site::profile_dir(&root, &supplier.id),
        supplier_site::landing_dir(&root, &supplier.id),
    ) else {
        return EXIT_FAILED;
    };
    if std::fs::create_dir_all(&profile)
        .and_then(|()| std::fs::create_dir_all(&landing))
        .is_err()
    {
        return EXIT_FAILED;
    }
    #[cfg(windows)]
    {
        match host::run(supplier, &url, profile, landing) {
            Ok(never) => match never {},
            Err(host::HostError::NoRuntime) => EXIT_NO_RUNTIME,
            Err(host::HostError::Failed(e)) => {
                log::error!("supplier browser: {e}");
                EXIT_FAILED
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (url, profile, landing);
        EXIT_NO_RUNTIME
    }
}

/// The result of asking for a supplier window.
pub enum Launch {
    /// The window is starting; watch the child for an early failure.
    Started(Child),
    /// No web view runtime: open the normal browser instead.
    NoRuntime,
    /// The program could not start the window.
    Failed(String),
}

/// A function that starts the window; replaced in tests.
pub type Launcher = fn(&Supplier, Option<&str>) -> Launch;

/// Starts the supplier browser window for `supplier`, at its search for
/// `order` when it has one.
pub fn launch(supplier: &Supplier, order: Option<&str>) -> Launch {
    if !runtime_available() {
        return Launch::NoRuntime;
    }
    let exe = match std::env::current_exe() {
        Ok(e) => e,
        Err(e) => return Launch::Failed(e.to_string()),
    };
    let mut cmd = Command::new(exe);
    cmd.arg(FLAG).arg(&supplier.id);
    if let Some(o) = order.filter(|o| !o.trim().is_empty()) {
        cmd.arg("--order").arg(o.trim());
    }
    // A portable install keeps its files beside the program; the window
    // must use the same folder.
    if let Some(p) = std::env::var_os(novo_storage::profile::ENV) {
        cmd.env(novo_storage::profile::ENV, p);
    }
    match cmd.spawn() {
        Ok(child) => Launch::Started(child),
        Err(e) => Launch::Failed(e.to_string()),
    }
}

/// Where the supplier's downloads land, created when needed.
pub fn landing_for(supplier: &Supplier) -> Option<PathBuf> {
    let dir = supplier_site::landing_dir(&supplier_site::browser_root()?, &supplier.id).ok()?;
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_flag_is_read_with_an_optional_order_number() {
        assert_eq!(parse_args(&a(&[])), None);
        assert_eq!(parse_args(&a(&["--safe-graphics"])), None);
        assert_eq!(parse_args(&a(&[FLAG])), None);
        assert_eq!(parse_args(&a(&[FLAG, "--order", "1"])), None);
        assert_eq!(
            parse_args(&a(&[FLAG, "rittal"])),
            Some(BrowserArgs {
                supplier: "rittal".into(),
                order: None
            })
        );
        assert_eq!(
            parse_args(&a(&["--x", FLAG, "wago", "--order", "2002-1201"])),
            Some(BrowserArgs {
                supplier: "wago".into(),
                order: Some("2002-1201".into())
            })
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn without_a_web_view_runtime_the_launch_says_so() {
        let s = &supplier_site::builtin()[0];
        assert!(!runtime_available());
        assert!(matches!(launch(s, None), Launch::NoRuntime));
    }

    /// The window never reads or drives a page: none of the calls that would
    /// do so may appear in the host. (Suppliers such as the CADENAS portals
    /// forbid automated queries; the user drives the browser.)
    #[test]
    fn the_host_never_scripts_a_page() {
        let src = include_str!("host.rs");
        for banned in [
            ".evaluate_script",
            ".with_initialization_script",
            ".with_ipc_handler",
            ".with_custom_protocol",
            ".with_devtools(true",
            ".with_user_agent",
            ".with_clipboard",
        ] {
            assert!(!src.contains(banned), "host.rs must not use {banned}");
        }
        assert!(src.contains("with_download_started_handler"));
    }
}
