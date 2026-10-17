//! The native file dialogs, behind one type so the visual test user can
//! answer them. Normally this is exactly `rfd::FileDialog`. In control mode
//! (`kopercad --control`) a dialog is not shown: it takes the next answer the
//! test user queued with `dialog_answer`, or is cancelled when none is queued,
//! and every request is recorded for `dialogs`. Native dialogs are separate OS
//! windows that a screenshot of the app window cannot see.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

struct Harness {
    answers: VecDeque<Vec<PathBuf>>,
    log: Vec<String>,
}

static HARNESS: Mutex<Option<Harness>> = Mutex::new(None);

/// Switches dialogs to the test user's answers (control mode).
pub fn enable_harness() {
    if let Ok(mut h) = HARNESS.lock() {
        h.get_or_insert(Harness {
            answers: VecDeque::new(),
            log: Vec::new(),
        });
    }
}

/// Queues the answer for the next dialog: the chosen path(s), empty to cancel.
pub fn queue_answer(paths: Vec<PathBuf>) {
    if let Ok(mut h) = HARNESS.lock()
        && let Some(h) = h.as_mut()
    {
        h.answers.push_back(paths);
    }
}

/// The requests since the last call, one line each, and how many answers wait.
pub fn take_log() -> (Vec<String>, usize) {
    match HARNESS.lock().ok().as_deref_mut() {
        Some(Some(h)) => (std::mem::take(&mut h.log), h.answers.len()),
        _ => (Vec::new(), 0),
    }
}

/// `Some(answer)` when the harness handles this dialog.
fn harness(
    kind: &str,
    title: &str,
    dir: &Option<PathBuf>,
    name: &Option<String>,
) -> Option<Vec<PathBuf>> {
    let mut guard = HARNESS.lock().ok()?;
    let h = guard.as_mut()?;
    let answer = h.answers.pop_front();
    let mut line = format!("{kind} \"{title}\"");
    if let Some(n) = name {
        line.push_str(&format!(" suggested name {n}"));
    }
    if let Some(d) = dir {
        line.push_str(&format!(" in {}", d.display()));
    }
    line.push_str(match &answer {
        Some(a) if !a.is_empty() => " -> answered",
        _ => " -> cancelled (no answer queued)",
    });
    h.log.push(line);
    Some(answer.unwrap_or_default())
}

/// Same builder as `rfd::FileDialog`, for the calls this app makes.
pub struct FileDialog {
    inner: rfd::FileDialog,
    title: String,
    dir: Option<PathBuf>,
    name: Option<String>,
}

impl FileDialog {
    pub fn new() -> Self {
        FileDialog {
            inner: rfd::FileDialog::new(),
            title: String::new(),
            dir: None,
            name: None,
        }
    }

    pub fn set_title(mut self, title: impl Into<String>) -> Self {
        let title = title.into();
        self.inner = self.inner.set_title(title.clone());
        self.title = title;
        self
    }

    pub fn add_filter(mut self, name: impl Into<String>, extensions: &[impl ToString]) -> Self {
        self.inner = self.inner.add_filter(name, extensions);
        self
    }

    pub fn set_directory<P: AsRef<Path>>(mut self, dir: P) -> Self {
        self.inner = self.inner.set_directory(dir.as_ref());
        self.dir = Some(dir.as_ref().to_owned());
        self
    }

    pub fn set_file_name(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        self.inner = self.inner.set_file_name(name.clone());
        self.name = Some(name);
        self
    }

    pub fn pick_file(self) -> Option<PathBuf> {
        match harness("open file", &self.title, &self.dir, &self.name) {
            Some(a) => a.into_iter().next(),
            None => self.inner.pick_file(),
        }
    }

    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        match harness("open files", &self.title, &self.dir, &self.name) {
            Some(a) => Some(a).filter(|a| !a.is_empty()),
            None => self.inner.pick_files(),
        }
    }

    pub fn pick_folder(self) -> Option<PathBuf> {
        match harness("choose folder", &self.title, &self.dir, &self.name) {
            Some(a) => a.into_iter().next(),
            None => self.inner.pick_folder(),
        }
    }

    pub fn save_file(self) -> Option<PathBuf> {
        match harness("save file", &self.title, &self.dir, &self.name) {
            Some(a) => a.into_iter().next(),
            None => self.inner.save_file(),
        }
    }
}
