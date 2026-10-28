//! A start that never reached its first frame (REL-START).
//!
//! A native crash, for example an access violation inside a graphics driver,
//! gives the panic hook no chance to run: no window, no crash report, no
//! message. This module leaves a trail instead. At start the program creates
//! `start-pending.txt` in the log folder, holds an exclusive lock on it and
//! keeps the current step and the graphics adapter in it. Once a few frames
//! have been drawn the file is removed. A file that is still there at the next
//! start, with no process holding its lock, belongs to a start that died:
//!
//! * the step and adapter it names go to the log and into the question asked
//!   before the window opens (start with safe graphics?), and
//! * the adapter is remembered in `graphics-avoid.txt`, so that the adapter
//!   order tries it last from then on (see `gpu::order`). A normal start that
//!   does reach the first frame on such an adapter removes it from the list.
//!
//! The lock is what tells a dead start from a second copy that is starting at
//! the same moment: the operating system drops it when the process ends,
//! however it ends. Everything is per user, in the log folder; no admin rights.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Exists from the start of the program until the first frames are drawn.
const MARKER_FILE: &str = "start-pending.txt";

/// Adapters that stopped a start; one `name<TAB>backend` per line, lower case.
const AVOID_FILE: &str = "graphics-avoid.txt";

/// Adapters kept in the avoid list; the oldest go first.
const AVOID_KEEP: usize = 8;

/// Frames drawn before the start counts as done. The first frame is the one a
/// driver crash is most likely to kill; a few more show that presenting works.
pub(crate) const FRAMES_FOR_DONE: u32 = 3;

/// A graphics adapter, as the log and the marker name it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Adapter {
    pub(crate) name: String,
    pub(crate) backend: String,
}

impl Adapter {
    /// What the avoid list and [`crate::gpu`] compare: case does not matter.
    pub(crate) fn key(&self) -> String {
        key(&self.name, &self.backend)
    }

    fn describe(&self) -> String {
        format!("{} ({})", self.name, self.backend)
    }
}

/// The avoid-list entry for an adapter.
pub(crate) fn key(name: &str, backend: &str) -> String {
    format!(
        "{}\t{}",
        one_line(name).to_lowercase(),
        backend.to_lowercase()
    )
}

/// What the marker file says.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marker {
    safe: bool,
    stage: String,
    adapter: Option<Adapter>,
    time: String,
}

const MARKER_HEADER: &str = "kopercad-start-pending 1";

impl Marker {
    fn render(&self) -> String {
        let mut out = format!(
            "{MARKER_HEADER}\nmode={}\ntime={}\nstage={}\n",
            if self.safe { "safe" } else { "normal" },
            self.time,
            one_line(&self.stage)
        );
        if let Some(a) = &self.adapter {
            out.push_str(&format!(
                "adapter={}\t{}\n",
                one_line(&a.name),
                one_line(&a.backend)
            ));
        }
        out
    }

    /// `None` for an empty or foreign file.
    fn parse(text: &str) -> Option<Marker> {
        let mut lines = text.lines();
        if lines.next()?.trim() != MARKER_HEADER {
            return None;
        }
        let mut marker = Marker {
            safe: false,
            stage: String::new(),
            adapter: None,
            time: String::new(),
        };
        for line in lines {
            let Some((k, v)) = line.split_once('=') else {
                continue;
            };
            match k {
                "mode" => marker.safe = v.trim() == "safe",
                "time" => marker.time = v.trim().to_owned(),
                "stage" => marker.stage = v.trim().to_owned(),
                "adapter" => {
                    if let Some((name, backend)) = v.split_once('\t') {
                        marker.adapter = Some(Adapter {
                            name: name.trim().to_owned(),
                            backend: backend.trim().to_owned(),
                        });
                    }
                }
                _ => {}
            }
        }
        Some(marker)
    }
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A start that died before its first frames, as found at the next start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// That start already used safe graphics (the software renderer).
    pub was_safe_graphics: bool,
    /// What it was doing when it stopped.
    pub stage: String,
    /// The graphics adapter it was testing or using, as "name (backend)".
    pub adapter: Option<String>,
    /// When it began, UTC.
    pub time: String,
    /// The folder with the log.
    pub log_dir: PathBuf,
}

impl Failure {
    /// The question shown before the window opens. Plain words: the person
    /// reading it has just seen KoperCAD fail to appear.
    pub fn message(&self) -> String {
        let step = if self.stage.is_empty() {
            "starting".to_owned()
        } else {
            self.stage.clone()
        };
        let mut text = format!(
            "The last time KoperCAD was started it stopped before its window was ready, \
             while it was: {step}."
        );
        if let Some(a) = &self.adapter {
            text.push_str(&format!("\nGraphics adapter involved: {a}."));
        }
        if self.was_safe_graphics {
            text.push_str(
                "\n\nThat start already used safe graphics, so the graphics driver is probably not \
                 the cause. KoperCAD will start normally now. If it fails again, please send the \
                 newest log file.",
            );
        } else {
            text.push_str(
                "\n\nStart with safe graphics now? It uses the software renderer: slower, but it \
                 needs no graphics driver.\n\nYes: start with safe graphics.\nNo: start normally",
            );
            if self.adapter.is_some() {
                text.push_str(" (that adapter is tried last from now on)");
            }
            text.push('.');
        }
        text.push_str(
            "\n\nThis question also comes after KoperCAD was ended in Task Manager while it was starting.",
        );
        text.push_str(&format!("\n\nThe log is in {}", self.log_dir.display()));
        text
    }
}

/// What the person chose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    SafeGraphics,
    Normal,
}

struct Tracker {
    dir: PathBuf,
    file: File,
    marker: Marker,
}

static TRACKER: Mutex<Option<Tracker>> = Mutex::new(None);

fn tracker() -> std::sync::MutexGuard<'static, Option<Tracker>> {
    TRACKER.lock().unwrap_or_else(|e| e.into_inner())
}

/// Starts the trail for this run. Call in `main`, after `diag::init` and before
/// the window opens. When the previous start died, `ask` is called with what is
/// known (not under the test harness, where there is nobody to ask).
pub fn begin(ask: impl FnOnce(&Failure) -> Answer) {
    let Some(dir) = crate::diag::log_dir() else {
        return;
    };
    let headless = crate::control::requested_port().is_some();
    let outcome = begin_in(&dir, headless, crate::gpu::safe_graphics(), ask);
    if outcome.safe && !crate::gpu::safe_graphics() {
        log::info!("Safe graphics chosen after a start that did not finish");
        crate::gpu::set_safe_graphics(true);
    }
}

struct Outcome {
    safe: bool,
}

fn begin_in(
    dir: &Path,
    headless: bool,
    safe_flag: bool,
    ask: impl FnOnce(&Failure) -> Answer,
) -> Outcome {
    let mut outcome = Outcome { safe: safe_flag };
    let path = dir.join(MARKER_FILE);
    let mut file = match File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
    {
        Ok(f) => f,
        Err(e) => {
            log::warn!("Start tracking is off: cannot open {}: {e}", path.display());
            return outcome;
        }
    };
    match file.try_lock() {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            // Another copy is starting right now; its file is not ours to judge.
            log::info!("Another KoperCAD is starting at the same time; start tracking is off");
            return outcome;
        }
        Err(e) => {
            log::warn!("Start tracking is off: cannot lock {}: {e}", path.display());
            return outcome;
        }
    }
    let mut text = String::new();
    let _ = file.read_to_string(&mut text);
    if let Some(prev) = Marker::parse(&text) {
        log::warn!(
            "The previous start (began {}, {}) did not reach its first frame. It was: {}{}",
            prev.time,
            if prev.safe {
                "safe graphics"
            } else {
                "normal graphics"
            },
            prev.stage,
            prev.adapter
                .as_ref()
                .map_or(String::new(), |a| format!("; adapter {}", a.describe()))
        );
        // Safe graphics is the way out of a bad adapter, so a safe start that
        // died says nothing against the adapters.
        if !prev.safe
            && let Some(a) = &prev.adapter
        {
            remember_avoid(dir, a);
        }
        if !headless {
            let failure = Failure {
                was_safe_graphics: prev.safe,
                stage: prev.stage.clone(),
                adapter: prev.adapter.as_ref().map(Adapter::describe),
                time: prev.time.clone(),
                log_dir: dir.to_path_buf(),
            };
            if ask(&failure) == Answer::SafeGraphics {
                outcome.safe = true;
            }
        }
    }
    let marker = Marker {
        safe: outcome.safe,
        stage: "starting".to_owned(),
        adapter: None,
        time: crate::diag::utc_now(),
    };
    let mut t = Tracker {
        dir: dir.to_path_buf(),
        file,
        marker,
    };
    t.write();
    *tracker() = Some(t);
    outcome
}

impl Tracker {
    fn write(&mut self) {
        let text = self.marker.render();
        let result = self
            .file
            .seek(SeekFrom::Start(0))
            .and_then(|_| self.file.set_len(0))
            .and_then(|()| self.file.write_all(text.as_bytes()))
            .and_then(|()| self.file.flush());
        if let Err(e) = result {
            log::warn!("Cannot update the start marker: {e}");
        }
    }
}

/// Records the step the start is in, before a call that may not return.
pub(crate) fn stage(text: &str) {
    log::info!("Start: {text}");
    if let Some(t) = tracker().as_mut() {
        t.marker.stage = text.to_owned();
        t.write();
    }
}

/// Records the adapter being tested or used, and the step.
pub(crate) fn adapter(adapter: &Adapter, text: &str) {
    log::info!("Start: {} - {text}", adapter.describe());
    if let Some(t) = tracker().as_mut() {
        t.marker.stage = text.to_owned();
        t.marker.adapter = Some(adapter.clone());
        t.write();
    }
}

/// The first frames are drawn: the start worked. An adapter that stopped an
/// earlier start and works now is taken off the avoid list.
pub fn first_frames_done() {
    let Some(t) = tracker().take() else {
        return;
    };
    log::info!("Start: the first frames were drawn");
    if let Some(a) = &t.marker.adapter {
        forget_avoid(&t.dir, a);
    }
    finish(t);
}

/// The program returned from the window loop by itself: the window was closed
/// before the first frames, or the start failed with an error it reports
/// itself. Either way the next start has nothing to ask about.
pub fn ended() {
    if let Some(t) = tracker().take() {
        finish(t);
    }
}

fn finish(t: Tracker) {
    let path = t.dir.join(MARKER_FILE);
    let _ = t.file.set_len(0);
    drop(t);
    let _ = std::fs::remove_file(path);
}

/// Test hook: `KOPERCAD_GRAPHICS_AVOID="name/backend;name/backend"` adds
/// adapters to the avoid list for this run only (the QA scenarios use it).
const AVOID_ENV: &str = "KOPERCAD_GRAPHICS_AVOID";

/// The adapters that stopped a start, as [`key`]s.
pub(crate) fn avoided() -> Vec<String> {
    let mut keys = crate::diag::log_dir()
        .map(|d| read_avoid(&d))
        .unwrap_or_default();
    if let Ok(list) = std::env::var(AVOID_ENV) {
        keys.extend(parse_avoid_env(&list));
    }
    keys
}

fn parse_avoid_env(list: &str) -> Vec<String> {
    list.split(';')
        .filter_map(|entry| entry.rsplit_once('/'))
        .map(|(name, backend)| key(name, backend.trim()))
        .collect()
}

fn read_avoid(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join(AVOID_FILE))
        .map(|t| parse_avoid(&t))
        .unwrap_or_default()
}

fn parse_avoid(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| l.contains('\t'))
        .map(|l| l.trim_end().to_owned())
        .collect()
}

fn write_avoid(dir: &Path, keys: &[String]) {
    let path = dir.join(AVOID_FILE);
    if keys.is_empty() {
        let _ = std::fs::remove_file(path);
    } else if let Err(e) = std::fs::write(&path, keys.join("\n") + "\n") {
        log::warn!("Cannot write {}: {e}", path.display());
    }
}

fn remember_avoid(dir: &Path, adapter: &Adapter) {
    let mut keys = read_avoid(dir);
    let key = adapter.key();
    keys.retain(|k| *k != key);
    keys.push(key);
    let excess = keys.len().saturating_sub(AVOID_KEEP);
    keys.drain(..excess);
    log::warn!(
        "{} is tried last from now on, because a start stopped while using it",
        adapter.describe()
    );
    write_avoid(dir, &keys);
}

fn forget_avoid(dir: &Path, adapter: &Adapter) {
    let mut keys = read_avoid(dir);
    let before = keys.len();
    let key = adapter.key();
    keys.retain(|k| *k != key);
    if keys.len() != before {
        log::info!(
            "{} reached the first frame, so it is no longer avoided",
            adapter.describe()
        );
        write_avoid(dir, &keys);
    }
}

/// A line for the problem report: the adapters that are tried last.
pub(crate) fn avoided_for_report() -> Option<String> {
    let keys = avoided();
    (!keys.is_empty()).then(|| format_avoid(&keys))
}

fn format_avoid(keys: &[String]) -> String {
    keys.iter()
        .map(|k| k.replace('\t', " / "))
        .collect::<Vec<_>>()
        .join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("kopercad-startup-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn intel_vulkan() -> Adapter {
        Adapter {
            name: "Intel(R) UHD Graphics".into(),
            backend: "Vulkan".into(),
        }
    }

    /// Plays the part of a start that died: the lock goes with the process.
    fn die(dir: &Path) {
        let t = tracker().take().expect("a start was tracked");
        assert_eq!(t.dir, dir);
        drop(t); // lock released, file stays
    }

    fn never_asked(_: &Failure) -> Answer {
        panic!("nothing to ask on a clean start");
    }

    // The tracker is process-wide state: one test drives the whole story.
    #[test]
    fn a_start_that_dies_is_found_asked_about_and_its_adapter_avoided() {
        let dir = temp("story");

        // 1. A clean first start: no question, a marker while it runs.
        begin_in(&dir, false, false, never_asked);
        assert!(dir.join(MARKER_FILE).is_file());
        adapter(&intel_vulkan(), "opening the graphics adapter");
        let text = std::fs::read_to_string(dir.join(MARKER_FILE)).unwrap();
        assert!(
            text.contains("adapter=Intel(R) UHD Graphics\tVulkan"),
            "{text}"
        );
        assert!(
            text.contains("stage=opening the graphics adapter\n"),
            "{text}"
        );

        // 2. The process dies there (no first_frames_done).
        die(&dir);

        // 3. The next start finds it, asks once, and avoids the adapter.
        let mut asked = None;
        let out = begin_in(&dir, false, false, |f| {
            asked = Some(f.clone());
            Answer::SafeGraphics
        });
        let f = asked.expect("the question was asked");
        assert!(!f.was_safe_graphics);
        assert_eq!(f.adapter.as_deref(), Some("Intel(R) UHD Graphics (Vulkan)"));
        assert!(f.stage.starts_with("opening the graphics adapter"));
        assert!(out.safe, "the answer was safe graphics");
        assert_eq!(read_avoid(&dir), vec![intel_vulkan().key()]);
        // The new marker records the mode that is now used.
        let text = std::fs::read_to_string(dir.join(MARKER_FILE)).unwrap();
        assert!(text.contains("mode=safe"), "{text}");

        // 4. That safe start worked: the marker is gone, the avoid list stays.
        first_frames_done();
        assert!(!dir.join(MARKER_FILE).exists());
        assert_eq!(read_avoid(&dir).len(), 1);

        // 5. A normal start proves the avoided adapter fine: it is forgiven.
        begin_in(&dir, false, false, never_asked);
        adapter(&intel_vulkan(), "using the graphics adapter");
        first_frames_done();
        assert!(read_avoid(&dir).is_empty());
        assert!(!dir.join(AVOID_FILE).exists());
        assert!(!dir.join(MARKER_FILE).exists());

        // 6. A start that came back by itself leaves nothing to ask.
        begin_in(&dir, false, false, never_asked);
        ended();
        begin_in(&dir, false, false, never_asked);
        first_frames_done();

        // 7. The test harness is never asked, and a safe start that died
        // does not blame the adapter.
        begin_in(&dir, false, true, never_asked);
        adapter(&intel_vulkan(), "using the graphics adapter");
        die(&dir);
        let mut asked = None;
        let out = begin_in(&dir, true, false, |f| {
            asked = Some(f.clone());
            Answer::Normal
        });
        assert!(asked.is_none(), "headless runs are not asked");
        assert!(!out.safe);
        assert!(read_avoid(&dir).is_empty());
        first_frames_done();

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_second_copy_starting_at_the_same_time_is_not_a_dead_start() {
        let dir = temp("second");
        // The first copy holds the lock.
        let first = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(dir.join(MARKER_FILE))
            .unwrap();
        first.lock().unwrap();
        let mut f = &first;
        f.write_all(
            Marker {
                safe: false,
                stage: "opening".into(),
                adapter: Some(intel_vulkan()),
                time: "t".into(),
            }
            .render()
            .as_bytes(),
        )
        .unwrap();
        let out = begin_in(&dir, false, false, never_asked);
        assert!(!out.safe);
        assert!(read_avoid(&dir).is_empty());
        // The second copy tracks nothing, so it cannot remove the first one's file.
        assert!(dir.join(MARKER_FILE).is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_marker_round_trips_and_foreign_files_are_ignored() {
        let m = Marker {
            safe: true,
            stage: "testing\nsomething".into(),
            adapter: Some(intel_vulkan()),
            time: "2026-10-28 12:00:00".into(),
        };
        let parsed = Marker::parse(&m.render()).unwrap();
        assert_eq!(parsed.stage, "testing something");
        assert_eq!(parsed.adapter, m.adapter);
        assert!(parsed.safe);
        assert_eq!(Marker::parse(""), None);
        assert_eq!(Marker::parse("hello\nworld"), None);
    }

    #[test]
    fn the_avoid_list_keeps_the_newest_few_without_duplicates() {
        let dir = temp("avoid");
        for i in 0..12 {
            remember_avoid(
                &dir,
                &Adapter {
                    name: format!("GPU {i}"),
                    backend: "Vulkan".into(),
                },
            );
        }
        remember_avoid(
            &dir,
            &Adapter {
                name: "gpu 11".into(),
                backend: "VULKAN".into(),
            },
        );
        let keys = read_avoid(&dir);
        assert_eq!(keys.len(), AVOID_KEEP);
        assert_eq!(keys.last().unwrap(), "gpu 11\tvulkan");
        assert_eq!(keys.iter().filter(|k| k.starts_with("gpu 11")).count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_test_hook_and_the_report_line_use_the_same_form() {
        let keys = parse_avoid_env("Intel(R) UHD Graphics/Vulkan; Other GPU/Dx12");
        assert_eq!(keys[0], key("Intel(R) UHD Graphics", "Vulkan"));
        assert_eq!(keys.len(), 2);
        assert_eq!(
            format_avoid(&keys),
            "intel(r) uhd graphics / vulkan; other gpu / dx12"
        );
        assert!(parse_avoid_env("no slash here").is_empty());
    }

    #[test]
    fn the_question_names_the_step_the_adapter_and_the_log() {
        let f = Failure {
            was_safe_graphics: false,
            stage: "opening the graphics device".into(),
            adapter: Some("Intel UHD (Vulkan)".into()),
            time: "t".into(),
            log_dir: PathBuf::from("C:/logs"),
        };
        let text = f.message();
        assert!(text.contains("opening the graphics device"));
        assert!(text.contains("Graphics adapter involved: Intel UHD (Vulkan)"));
        assert!(text.contains("Start with safe graphics now?"));
        assert!(text.contains("tried last"));
        assert!(text.contains("Task Manager"));
        // The adapter is named once, on its own line.
        assert_eq!(text.matches("Intel UHD (Vulkan)").count(), 1, "{text}");
        assert!(text.contains("C:/logs"));
        let safe = Failure {
            was_safe_graphics: true,
            ..f
        };
        assert!(!safe.message().contains("Start with safe graphics now?"));
    }
}
