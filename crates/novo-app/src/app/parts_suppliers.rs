//! Get parts from a supplier: the Parts window's side of the in-app supplier
//! browser (`crate::supplier_browser`). It starts the window, watches that
//! supplier's private landing folder, and hands every finished download to the
//! window's normal file pipeline (the same one a file dropped on the window
//! goes through, so the matcher still reads it before anything is taken).
//! Where the web view cannot run it opens the normal browser and watches
//! Downloads instead.

use std::path::{Path, PathBuf};
use std::process::Child;
use std::time::{Duration, Instant, SystemTime};

use novo_parts::supplier_site::{self, LandingWatcher, Supplier};

use crate::supplier_browser::{self, Launch, Launcher};

/// Seconds between looks at the folder.
const POLL_SECONDS: f64 = 1.0;
/// A session ends by itself after this long.
const SESSION_LIMIT: Duration = Duration::from_secs(30 * 60);
/// Landed files older than this are removed when a session starts.
const KEEP_LANDED: Duration = Duration::from_secs(7 * 24 * 3600);

/// Where a session's files come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Source {
    /// The in-app browser window, downloads land in its private folder.
    Window,
    /// The normal browser; Downloads is watched for the supplier's kinds.
    Downloads,
}

struct Session {
    id: String,
    name: String,
    url: String,
    source: Source,
    started: Instant,
    child: Option<Child>,
    watcher: LandingWatcher,
    last_poll: f64,
    /// Files handed over, newest last (shown as a count).
    handed: usize,
}

/// What the window must do after a pump.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Pumped {
    /// Finished downloads to feed into the file pipeline.
    pub landed: Vec<PathBuf>,
    /// An address to open in the normal browser (the web view failed).
    pub open_in_system_browser: Option<String>,
}

/// How an open request went.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Opened {
    Window,
    /// Open this address in the normal browser.
    SystemBrowser(String),
}

pub(super) struct Suppliers {
    list: Vec<Supplier>,
    /// A problem found in the user's own supplier list, shown behind the "?".
    pub problem: Option<String>,
    /// The user prefers their normal browser.
    pub use_system_browser: bool,
    session: Option<Session>,
    launcher: Launcher,
    root: Option<PathBuf>,
    /// A line for the strip: what happened last.
    pub note: Option<String>,
}

impl Suppliers {
    pub fn load() -> Self {
        let root = supplier_site::browser_root();
        let (list, problem) = supplier_site::load(
            root.as_deref()
                .map(supplier_site::user_list_path)
                .as_deref(),
        );
        Suppliers {
            list,
            problem,
            use_system_browser: false,
            session: None,
            launcher: supplier_browser::launch,
            root,
            note: None,
        }
    }

    #[cfg(test)]
    pub fn for_test(list: Vec<Supplier>, root: PathBuf, launcher: Launcher) -> Self {
        Suppliers {
            list,
            problem: None,
            use_system_browser: false,
            session: None,
            launcher,
            root: Some(root),
            note: None,
        }
    }

    pub fn list(&self) -> &[Supplier] {
        &self.list
    }

    pub fn for_url(&self, url: &str) -> Option<&Supplier> {
        supplier_site::supplier_for_url(&self.list, url)
    }

    pub fn for_maker(&self, maker: &str) -> Option<&Supplier> {
        supplier_site::supplier_for_maker(&self.list, maker)
    }

    /// The running session's supplier name, if one is open.
    pub fn open_name(&self) -> Option<&str> {
        self.session.as_ref().map(|s| s.name.as_str())
    }

    pub fn handed_over(&self) -> usize {
        self.session.as_ref().map_or(0, |s| s.handed)
    }

    pub fn stop(&mut self) {
        self.session = None;
    }

    /// Opens `supplier_id` at its search for `order` (when it has one). The
    /// user signs in and downloads in the page; nothing is done for them.
    pub fn open(
        &mut self,
        supplier_id: &str,
        order: Option<&str>,
        downloads: Option<&Path>,
        wanted: Source,
    ) -> Option<Opened> {
        let supplier = self.list.iter().find(|s| s.id == supplier_id)?.clone();
        let url = supplier.open_url(order);
        let since = SystemTime::now();
        let order_text = order.unwrap_or("");
        let landing = self
            .root
            .as_deref()
            .and_then(|r| supplier_site::landing_dir(r, &supplier.id).ok());
        if let Some(dir) = &landing {
            let _ = std::fs::create_dir_all(dir);
            supplier_site::purge_landing(dir, KEEP_LANDED);
        }
        let mut child = None;
        let mut source = Source::Downloads;
        if wanted == Source::Window && !self.use_system_browser {
            match (self.launcher)(&supplier, order) {
                Launch::Started(c) => {
                    child = Some(c);
                    source = Source::Window;
                }
                Launch::NoRuntime => {
                    self.note = Some(
                        "The built-in browser needs the Microsoft WebView2 runtime, which was not found. Your normal browser is used; downloads are picked up from Downloads."
                            .into(),
                    );
                }
                Launch::Failed(e) => {
                    self.note = Some(format!(
                        "The built-in browser did not start ({e}). Your normal browser is used."
                    ));
                }
            }
        }
        let watch_dir = match (source, landing, downloads) {
            (Source::Window, Some(l), _) => Some(l),
            (_, _, Some(d)) => Some(d.to_path_buf()),
            _ => None,
        };
        let mut watcher = LandingWatcher::new(watch_dir.unwrap_or_default(), since, order_text);
        if source == Source::Downloads {
            watcher = watcher.with_kinds(&supplier.accepts);
        } else {
            self.note = None;
        }
        self.session = Some(Session {
            id: supplier.id.clone(),
            name: supplier.name.clone(),
            url: url.clone(),
            source,
            started: Instant::now(),
            child,
            watcher,
            last_poll: f64::NEG_INFINITY,
            handed: 0,
        });
        Some(if source == Source::Window {
            Opened::Window
        } else {
            Opened::SystemBrowser(url)
        })
    }

    /// Called every frame with the window's clock.
    pub fn pump(&mut self, now: f64) -> Pumped {
        let mut out = Pumped::default();
        let Some(mut s) = self.session.take() else {
            return out;
        };
        if s.started.elapsed() > SESSION_LIMIT {
            return out;
        }
        // The window failed after it started (no runtime, bad folder):
        // fall back to the normal browser and Downloads, once.
        if let Some(child) = s.child.as_mut()
            && let Ok(Some(status)) = child.try_wait()
        {
            s.child = None;
            if !status.success() {
                s.source = Source::Downloads;
                out.open_in_system_browser = Some(s.url.clone());
                self.note = Some(
                    if status.code() == Some(supplier_browser::EXIT_NO_RUNTIME) {
                        "The built-in browser needs the Microsoft WebView2 runtime, which was not found. Your normal browser is used; downloads are picked up from Downloads.".to_owned()
                    } else {
                        "The built-in browser did not open. Your normal browser is used; downloads are picked up from Downloads.".to_owned()
                    },
                );
                if let Some(d) = dirs::download_dir() {
                    s.watcher = LandingWatcher::new(d, SystemTime::now(), "")
                        .with_kinds(&self.list_accepts(&s.id));
                }
            }
        }
        if now - s.last_poll >= POLL_SECONDS {
            s.last_poll = now;
            out.landed = s.watcher.poll().into_iter().map(|a| a.path).collect();
            s.handed += out.landed.len();
        }
        self.session = Some(s);
        out
    }

    fn list_accepts(&self, id: &str) -> Vec<novo_parts::FileKind> {
        self.list
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.accepts.clone())
            .unwrap_or_default()
    }

    /// Removes the saved sign-in of a supplier.
    pub fn forget(&mut self, id: &str) -> Result<(), String> {
        let root = self.root.as_deref().ok_or("no profile folder")?;
        supplier_site::forget_profile(root, id).map_err(|e| e.to_string())
    }

    /// The landing folder of a supplier, for "Open downloads folder".
    pub fn landing(&self, id: &str) -> Option<PathBuf> {
        supplier_site::landing_dir(self.root.as_deref()?, id).ok()
    }

    pub fn source(&self) -> Option<Source> {
        self.session.as_ref().map(|s| s.source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn no_runtime(_: &Supplier, _: Option<&str>) -> Launch {
        Launch::NoRuntime
    }

    fn failed(_: &Supplier, _: Option<&str>) -> Launch {
        Launch::Failed("boom".into())
    }

    fn exits_ok(_: &Supplier, _: Option<&str>) -> Launch {
        #[cfg(windows)]
        let c = std::process::Command::new("cmd")
            .args(["/C", "exit 0"])
            .spawn();
        #[cfg(not(windows))]
        let c = std::process::Command::new("true").spawn();
        c.map_or_else(|e| Launch::Failed(e.to_string()), Launch::Started)
    }

    fn exits_bad(_: &Supplier, _: Option<&str>) -> Launch {
        #[cfg(windows)]
        let c = std::process::Command::new("cmd")
            .args(["/C", "exit 3"])
            .spawn();
        #[cfg(not(windows))]
        let c = std::process::Command::new("false").spawn();
        c.map_or_else(|e| Launch::Failed(e.to_string()), Launch::Started)
    }

    fn make(launcher: Launcher) -> (Suppliers, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let s = Suppliers::for_test(supplier_site::builtin(), dir.path().to_path_buf(), launcher);
        (s, dir)
    }

    fn wait_for_exit(s: &mut Suppliers) -> Pumped {
        // The child is a tiny program; bounded poll, no wall-clock assert.
        // Everything handed over on the way is kept.
        let mut total = Pumped::default();
        let mut now = 0.0;
        for _ in 0..500 {
            now += 2.0;
            let p = s.pump(now);
            total.landed.extend(p.landed);
            total.open_in_system_browser =
                total.open_in_system_browser.or(p.open_in_system_browser);
            if total.open_in_system_browser.is_some()
                || s.session.as_ref().is_some_and(|x| x.child.is_none())
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        total
    }

    #[test]
    fn a_window_download_is_handed_over_once_from_the_private_folder() {
        let (mut s, dir) = make(exits_ok);
        assert_eq!(
            s.open("rittal", None, None, Source::Window),
            Some(Opened::Window)
        );
        let landing = s.landing("rittal").unwrap();
        assert!(landing.starts_with(dir.path()));
        fs::write(landing.join("1-a.step.part"), b"half").unwrap();
        fs::write(landing.join("2-a.step"), b"ISO").unwrap();
        let mut got = wait_for_exit(&mut s).landed;
        let mut now = 1000.0;
        for _ in 0..4 {
            now += 2.0;
            got.extend(s.pump(now).landed);
        }
        assert_eq!(got, vec![landing.join("2-a.step")]);
        assert_eq!(s.handed_over(), 1);
        assert_eq!(s.source(), Some(Source::Window));
    }

    #[test]
    fn without_the_runtime_the_normal_browser_and_downloads_are_used() {
        for launcher in [no_runtime as Launcher, failed] {
            let (mut s, _dir) = make(launcher);
            let downloads = tempfile::tempdir().unwrap();
            let got = s.open(
                "wago",
                Some("2002-1201"),
                Some(downloads.path()),
                Source::Window,
            );
            let Some(Opened::SystemBrowser(url)) = got else {
                panic!("{got:?}");
            };
            assert!(url.ends_with("part=2002-1201"));
            assert!(s.note.is_some());
            assert_eq!(s.source(), Some(Source::Downloads));
            // Only the supplier's kinds arrive from Downloads.
            fs::write(downloads.path().join("x.step"), b"ISO").unwrap();
            fs::write(downloads.path().join("holiday.pdf"), b"%PDF").unwrap();
            fs::write(downloads.path().join("setup.exe"), b"MZ").unwrap();
            let mut got = Vec::new();
            let mut now = 0.0;
            for _ in 0..4 {
                now += 2.0;
                got.extend(s.pump(now).landed);
            }
            assert_eq!(got, vec![downloads.path().join("x.step")]);
        }
    }

    #[test]
    fn a_window_that_dies_at_start_falls_back_once() {
        let (mut s, _dir) = make(exits_bad);
        assert_eq!(
            s.open("rittal", None, None, Source::Window),
            Some(Opened::Window)
        );
        let p = wait_for_exit(&mut s);
        assert_eq!(
            p.open_in_system_browser.as_deref(),
            Some("https://rittal.partcommunity.com/3d-cad-models/")
        );
        assert_eq!(s.source(), Some(Source::Downloads));
        assert!(s.note.is_some());
        assert_eq!(s.pump(100.0).open_in_system_browser, None, "only once");
    }

    #[test]
    fn the_user_can_prefer_the_normal_browser_and_stop_ends_the_watch() {
        let (mut s, _dir) = make(exits_ok);
        s.use_system_browser = true;
        let got = s.open("traceparts", Some("LC1D09"), None, Source::Window);
        assert!(matches!(got, Some(Opened::SystemBrowser(u)) if u.contains("KeyWords=LC1D09")));
        assert!(s.open_name().is_some());
        s.stop();
        assert!(s.open_name().is_none());
        assert_eq!(s.pump(1.0), Pumped::default());
        assert_eq!(s.open("nobody", None, None, Source::Window), None);
    }

    #[test]
    fn forgetting_a_sign_in_removes_only_that_profile() {
        let (mut s, dir) = make(exits_ok);
        let p = supplier_site::profile_dir(dir.path(), "wago").unwrap();
        fs::create_dir_all(&p).unwrap();
        fs::write(p.join("Cookies"), b"x").unwrap();
        s.forget("wago").unwrap();
        assert!(!p.exists());
        assert!(s.forget("../x").is_err());
    }

    #[test]
    fn makers_and_pages_find_their_supplier() {
        let (s, _dir) = make(exits_ok);
        assert_eq!(s.for_maker("Rittal").unwrap().id, "rittal");
        assert_eq!(
            s.for_url("https://wago.partcommunity.com/3d-cad-models/?x=1")
                .unwrap()
                .id,
            "wago"
        );
        assert!(s.for_url("https://example.org/").is_none());
    }
}
