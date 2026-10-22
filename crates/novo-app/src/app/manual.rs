//! Help > User manual and F1 (`help.manual`, `help.context`): the offline
//! manual of `novo_workflow::manual`, written under the profile and opened
//! in the default browser. Local only: no network, no admin rights.
//!
//! Where the user finds it: Help > User manual, F1 for the page that fits
//! the workspace or window in front, Ctrl+K "manual" or "help".

use std::path::PathBuf;

use super::workspace::Workspace;
use super::{Level, NovoApp};

/// Screenshots bundled with the manual, refreshed by `scripts/build-manual.sh`.
const IMAGES: &[(&str, &[u8])] = &[
    (
        "start",
        include_bytes!("../../../../docs/manual/images/start.png"),
    ),
    (
        "example-schematic",
        include_bytes!("../../../../docs/manual/images/example-schematic.png"),
    ),
    (
        "help-menu",
        include_bytes!("../../../../docs/manual/images/help-menu.png"),
    ),
    (
        "cabinet",
        include_bytes!("../../../../docs/manual/images/cabinet.png"),
    ),
    (
        "view-3d",
        include_bytes!("../../../../docs/manual/images/view-3d.png"),
    ),
    (
        "command-search",
        include_bytes!("../../../../docs/manual/images/command-search.png"),
    ),
    (
        "build-mode",
        include_bytes!("../../../../docs/manual/images/build-mode.png"),
    ),
];

/// Where the manual is written: beside the logs, so `--profile-dir` moves it
/// with everything else.
pub(crate) fn manual_dir() -> Option<PathBuf> {
    crate::diag::default_log_dir()?
        .parent()
        .map(|p| p.join("manual"))
}

/// A `file:` URL for a local page, with the heading to land on.
fn file_url(path: &std::path::Path, anchor: &str) -> String {
    let mut url = String::from("file:///");
    let text = path.to_string_lossy().replace('\\', "/");
    for c in text.trim_start_matches('/').chars() {
        match c {
            ' ' => url.push_str("%20"),
            '#' => url.push_str("%23"),
            '%' => url.push_str("%25"),
            c => url.push(c),
        }
    }
    if !anchor.is_empty() {
        url.push('#');
        url.push_str(anchor);
    }
    url
}

/// Opens a page in the default browser (per user, no elevation). Windows
/// hands the URL to the default handler so the heading anchor is kept.
fn open_page(path: &std::path::Path, anchor: &str) -> Result<(), String> {
    let url = file_url(path, anchor);
    let mut command = if cfg!(windows) {
        let mut c = std::process::Command::new("rundll32");
        c.arg("url.dll,FileProtocolHandler").arg(&url);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(&url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(&url);
        c
    };
    command.spawn().map(|_| ()).map_err(|e| format!("{e}"))
}

/// The context-help topic of a workspace.
pub(crate) fn topic_of(workspace: Workspace) -> &'static str {
    match workspace {
        Workspace::Schematic => "schematic",
        Workspace::Cabinet => "cabinet",
        Workspace::View3d => "3d",
        Workspace::Reports => "reports",
        Workspace::Commissioning => "commissioning",
    }
}

impl NovoApp {
    /// The topic for what is in front: a window first, then the workspace,
    /// and the start page when no project is open.
    pub(super) fn help_topic(&self) -> &'static str {
        if self.show_numbers {
            "numbers"
        } else if self.wb.build.is_some() {
            "build-mode"
        } else if self.wb.live.on {
            "live"
        } else if !self.is_open() {
            "start"
        } else {
            topic_of(self.workspace())
        }
    }

    /// Writes the manual and opens the page for `topic` (the front page for
    /// `None`).
    pub(super) fn open_manual(&mut self, topic: Option<&'static str>) {
        let Some(dir) = manual_dir() else {
            self.message(Level::Warning, "No place for the manual on this computer");
            return;
        };
        let version = env!("CARGO_PKG_VERSION");
        if let Err(e) = novo_workflow::manual::write_site(&dir, version, IMAGES) {
            self.message(
                Level::Warning,
                format!("Could not write the manual to {}: {e}", dir.display()),
            );
            return;
        }
        let page = novo_workflow::manual::topic_file(topic.unwrap_or(""));
        let (file, anchor) = page.split_once('#').unwrap_or((page, ""));
        let target = dir.join(file);
        // Tests and the QA suite (`KOPERCAD_NO_BROWSER=1`) write the pages but
        // do not start a browser.
        if cfg!(test) || std::env::var_os("KOPERCAD_NO_BROWSER").is_some() {
            self.message_file("Manual written", &target);
            return;
        }
        match open_page(&target, anchor) {
            Ok(()) => self.message_file("Manual opened in your browser", &target),
            Err(e) => self.message(
                Level::Warning,
                format!(
                    "Could not open the manual ({e}). Open {} yourself.",
                    target.display()
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_picture_a_page_names_is_bundled() {
        let site = novo_workflow::manual::build_site("test", IMAGES);
        for file in site.iter().filter(|f| f.name.ends_with(".html")) {
            let html = String::from_utf8_lossy(&file.bytes);
            assert!(
                !html.contains("Screenshot: "),
                "{} names a picture that is not in docs/manual/images (run scripts/build-manual.sh)",
                file.name
            );
        }
        for (name, bytes) in IMAGES {
            assert!(bytes.starts_with(b"\x89PNG"), "{name} is not a PNG");
        }
    }

    #[test]
    fn the_page_url_keeps_spaces_and_the_heading() {
        let url = file_url(
            std::path::Path::new("/home/a b/manual/cabinet.html"),
            "the-3d-view",
        );
        assert_eq!(url, "file:///home/a%20b/manual/cabinet.html#the-3d-view");
        let win = file_url(std::path::Path::new("C:\\Users\\x y\\m.html"), "");
        assert_eq!(win, "file:///C:/Users/x%20y/m.html");
    }

    #[test]
    fn every_workspace_has_a_topic_that_exists() {
        for ws in Workspace::ALL {
            let topic = topic_of(ws);
            assert_ne!(
                novo_workflow::manual::topic_file(topic),
                "index.html",
                "{topic} has no page"
            );
        }
    }

    #[test]
    fn every_context_topic_the_app_asks_for_is_in_the_manual() {
        for topic in [
            "numbers",
            "build-mode",
            "live",
            "start",
            "schematic",
            "cabinet",
            "3d",
            "reports",
            "commissioning",
        ] {
            assert!(
                novo_workflow::manual::TOPICS
                    .iter()
                    .any(|(t, _)| *t == topic),
                "{topic}"
            );
        }
    }

    #[test]
    fn the_commands_are_run_by_the_app() {
        assert!(crate::app::workbench::app_runs("help.manual"));
        assert!(crate::app::workbench::app_runs("help.context"));
    }
}
