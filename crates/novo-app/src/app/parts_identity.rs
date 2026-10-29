//! Reading what a CAD file says about itself, off the window's thread.
//!
//! A STEP file can be 64 MB and the solid is measured under a 20 s deadline,
//! so reading a batch of them in a frame froze the Parts window. The window
//! asks for the files first ([`prefetch`]), keeps painting, and uses the files
//! once [`ready`] says every one has been read; [`identity_of`] then finds the
//! answer in the cache. A file that was not asked for is still read (in the
//! caller's thread), which only tests and the worker that stores files rely
//! on.
//!
//! One worker thread reads the files one after the other, so a batch never
//! runs more than one STEP measurement at a time. An entry is valid for the
//! file's size and modification time: a file that was downloaded again is
//! read again.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use eframe::egui;
use novo_parts::CadIdentity;

use super::parts_cad::read_identity;

/// Entries kept; the oldest go first.
const MAX_ENTRIES: usize = 512;
/// Entries looked at, from the oldest, when one has to go; a cache full of
/// files that are all still wanted is not searched to the end for every file
/// stored.
const EVICT_SCAN: usize = 32;
/// A file asked for in this time is not forgotten, however many entries there
/// are: the window asks for its waiting files every frame, so a batch larger
/// than [`MAX_ENTRIES`] is still found whole once it has been read.
const KEEP_ASKED: Duration = Duration::from_secs(10);

#[derive(Clone, PartialEq, Eq)]
struct Stamp {
    len: Option<u64>,
    modified: Option<SystemTime>,
}

fn stamp_of(path: &Path) -> Stamp {
    let meta = std::fs::metadata(path).ok();
    Stamp {
        len: meta.as_ref().map(std::fs::Metadata::len),
        modified: meta.and_then(|m| m.modified().ok()),
    }
}

#[derive(Default)]
struct State {
    done: HashMap<PathBuf, (Stamp, CadIdentity)>,
    order: VecDeque<PathBuf>,
    /// Waiting for the worker.
    queue: VecDeque<PathBuf>,
    /// The same files as `queue`, to ask in constant time.
    queued: HashSet<PathBuf>,
    /// The file the worker is reading now.
    running: Option<PathBuf>,
    worker: bool,
    /// Files asked for, and read, since the worker was last idle.
    requested: usize,
    finished: usize,
    /// When each file was last asked for.
    asked: HashMap<PathBuf, Instant>,
}

impl State {
    fn fresh(&self, path: &Path) -> Option<&CadIdentity> {
        let (stamp, id) = self.done.get(path)?;
        (*stamp == stamp_of(path)).then_some(id)
    }

    fn wanted(&self, path: &Path) -> bool {
        self.asked
            .get(path)
            .is_some_and(|at| at.elapsed() < KEEP_ASKED)
    }

    /// Notes that the window wants these files and queues those not read or
    /// waiting yet. With `check`, a file in the cache is compared with the
    /// disk (a call per file); without it a file in the cache is trusted, so
    /// asking again every frame for thousands of files costs no disk calls.
    fn ask(&mut self, paths: &[PathBuf], now: Instant, check: bool) {
        self.asked
            .retain(|_, at| now.duration_since(*at) < KEEP_ASKED);
        for p in paths {
            self.asked.insert(p.clone(), now);
            let queued = self.queued.contains(p) || self.running.as_deref() == Some(p.as_path());
            let read = if check {
                self.fresh(p).is_some()
            } else {
                self.done.contains_key(p)
            };
            if !queued && !read {
                self.queue.push_back(p.clone());
                self.queued.insert(p.clone());
                self.requested += 1;
            }
        }
    }

    /// Files waiting for the worker or being read.
    fn pending(&self) -> usize {
        self.queue.len() + usize::from(self.running.is_some())
    }

    fn store(&mut self, path: &Path, stamp: Stamp, id: CadIdentity) {
        if self.done.insert(path.to_path_buf(), (stamp, id)).is_none() {
            self.order.push_back(path.to_path_buf());
        }
        // The oldest entry that is not wanted goes; the one just stored stays.
        while self.order.len() > MAX_ENTRIES {
            let last = self.order.len() - 1;
            let Some(i) = self
                .order
                .iter()
                .take(last.min(EVICT_SCAN))
                .position(|old| !self.wanted(old))
            else {
                break;
            };
            if let Some(old) = self.order.remove(i) {
                self.done.remove(&old);
            }
        }
    }
}

fn cell() -> &'static Mutex<State> {
    static STATE: OnceLock<Mutex<State>> = OnceLock::new();
    STATE.get_or_init(Mutex::default)
}

fn lock() -> MutexGuard<'static, State> {
    cell().lock().unwrap_or_else(|e| e.into_inner())
}

/// What the file at `path` says about itself. Never fails: an unreadable or
/// very large file just says less. Answers from the cache when the file is
/// unchanged; otherwise reads it here.
pub(crate) fn identity_of(path: &Path) -> CadIdentity {
    if let Some(id) = cached(path) {
        return id;
    }
    let stamp = stamp_of(path);
    let id = read_identity(path);
    lock().store(path, stamp, id.clone());
    id
}

/// The answer for `path` if it has been read and the file has not changed.
pub(crate) fn cached(path: &Path) -> Option<CadIdentity> {
    lock().fresh(path).cloned()
}

/// Whether every file in `paths` has been read.
pub(crate) fn ready(paths: &[PathBuf]) -> bool {
    let s = lock();
    paths.iter().all(|p| s.fresh(p).is_some())
}

/// Starts reading the files in `paths` that are not read yet, on the worker
/// thread. Cheap to call every frame: a file already read or waiting is not
/// queued again. `wake` repaints the window as each file is done.
pub(crate) fn prefetch(paths: &[PathBuf], wake: Option<egui::Context>) {
    ask(paths, wake, true);
}

/// Asks again for files already asked for, trusting those in the cache: no
/// call to the disk per file, so a window waiting for thousands of dropped
/// files can do it every frame. [`still_reading`] says whether to wait on.
pub(crate) fn keep_asking(paths: &[PathBuf], wake: Option<egui::Context>) {
    ask(paths, wake, false);
}

/// Whether files are queued or being read.
pub(crate) fn still_reading() -> bool {
    lock().pending() > 0
}

fn ask(paths: &[PathBuf], wake: Option<egui::Context>, check: bool) {
    let start = {
        let mut s = lock();
        s.ask(paths, Instant::now(), check);
        let start = !s.worker && !s.queue.is_empty();
        if start {
            s.worker = true;
        }
        start
    };
    if !start {
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("parts-identity".into())
        .spawn({
            let wake = wake.clone();
            move || work(wake.as_ref())
        });
    if spawned.is_err() {
        // No thread to be had: read here, as the window did before.
        work(wake.as_ref());
    }
}

fn work(wake: Option<&egui::Context>) {
    loop {
        let path = {
            let mut s = lock();
            let Some(path) = s.queue.pop_front() else {
                s.worker = false;
                s.running = None;
                (s.requested, s.finished) = (0, 0);
                break;
            };
            s.queued.remove(&path);
            s.running = Some(path.clone());
            path
        };
        slow_for_qa();
        let stamp = stamp_of(&path);
        let id = std::panic::catch_unwind(|| read_identity(&path)).unwrap_or_else(|_| {
            let name = path
                .file_name()
                .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            CadIdentity::from_file_name(&name)
        });
        {
            let mut s = lock();
            s.store(&path, stamp, id);
            s.running = None;
            s.finished += 1;
        }
        if let Some(ctx) = wake {
            ctx.request_repaint();
        }
    }
    if let Some(ctx) = wake {
        ctx.request_repaint();
    }
}

/// Holds the worker before each file for `KOPERCAD_QA_SLOW_READ_MS`
/// milliseconds (at most a minute), so a QA scenario can take a picture of
/// the window while files are being read. Unset, it does nothing.
fn slow_for_qa() {
    static DELAY: OnceLock<u64> = OnceLock::new();
    let ms = *DELAY.get_or_init(|| {
        std::env::var("KOPERCAD_QA_SLOW_READ_MS")
            .ok()
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map_or(0, |ms| ms.min(60_000))
    });
    if ms > 0 {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// `(read, asked for)` while files are being read, `None` when idle.
pub(crate) fn progress() -> Option<(usize, usize)> {
    let s = lock();
    (s.worker || !s.queue.is_empty()).then_some((s.finished, s.requested))
}

/// The line the window shows while files are read.
pub(crate) fn reading_text() -> Option<String> {
    let (done, total) = progress()?;
    Some(if total <= 1 {
        "Reading the file\u{2026}".to_owned()
    } else {
        format!("Reading the files\u{2026} {} of {total}", done.min(total))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache is global, so a test that fills it must not run beside one
    /// that expects its own entries to stay.
    fn one_at_a_time() -> MutexGuard<'static, ()> {
        static TESTS: Mutex<()> = Mutex::new(());
        TESTS.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn step_text(a: f64, b: f64, c: f64) -> String {
        format!(
            "ISO-10303-21;\nHEADER;\nFILE_NAME('X-1','2000-01-01T00:00:00',('A'),('M'),'','','');\nENDSEC;\nDATA;\n#9=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));\n#1=CARTESIAN_POINT('',(0.,0.,0.));\n#2=CARTESIAN_POINT('',({a},{b},{c}));\n#7=PRODUCT('X-1','X-1','NOT SPECIFIED',(#8));\nENDSEC;\nEND-ISO-10303-21;\n"
        )
    }

    fn wait_until_ready(paths: &[PathBuf]) {
        let started = std::time::Instant::now();
        while !ready(paths) {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(20),
                "the files were not read"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    #[test]
    fn files_are_read_on_the_worker_and_then_answered_from_the_cache() {
        let _one = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..30)
            .map(|i| {
                let p = dir.path().join(format!("X-{i}.stp"));
                std::fs::write(&p, step_text(10.0 + i as f64, 20.0, 30.0)).unwrap();
                p
            })
            .collect();
        assert!(!ready(&paths));
        let caller = std::thread::current().id();
        prefetch(&paths, None);
        // Asking again while the files are queued does not queue them again.
        prefetch(&paths, None);
        assert!(
            progress().is_some() || ready(&paths),
            "the worker was started"
        );
        wait_until_ready(&paths);
        assert_eq!(std::thread::current().id(), caller);
        for (i, p) in paths.iter().enumerate() {
            let id = identity_of(p);
            let size = id.size.expect("the size of the solid is read");
            let mut axes = size.size_mm;
            axes.sort_by(f64::total_cmp);
            let mut want = [10.0 + i as f64, 20.0, 30.0];
            want.sort_by(f64::total_cmp);
            assert_eq!(axes, want, "{i}");
        }
    }

    #[test]
    fn asking_for_a_large_file_returns_at_once_and_the_file_is_read_later() {
        let _one = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("BIG-1.stp");
        let mut text = step_text(10.0, 20.0, 30.0);
        let points: String = (3..120_000)
            .map(|i| format!("#{i}0=CARTESIAN_POINT('',({}.,5.,5.));\n", i % 10))
            .collect();
        text = text.replace("ENDSEC;\nEND-ISO", &format!("{points}ENDSEC;\nEND-ISO"));
        std::fs::write(&path, &text).unwrap();
        let asked = std::time::Instant::now();
        prefetch(std::slice::from_ref(&path), None);
        let took = asked.elapsed();
        assert!(
            took < std::time::Duration::from_millis(250),
            "asking took {took:?}: the file was read in the caller's thread"
        );
        wait_until_ready(std::slice::from_ref(&path));
        let again = std::time::Instant::now();
        let _ = identity_of(&path);
        assert!(
            again.elapsed() < std::time::Duration::from_millis(50),
            "the answer was not in the cache"
        );
    }

    #[test]
    fn a_file_that_changed_is_read_again() {
        let _one = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("X-1.stp");
        std::fs::write(&path, step_text(10.0, 20.0, 30.0)).unwrap();
        let first = identity_of(&path);
        assert!(cached(&path).is_some());
        std::fs::write(&path, step_text(11.0, 22.0, 3333.0)).unwrap();
        assert!(cached(&path).is_none(), "an entry for the old file");
        let second = identity_of(&path);
        assert_ne!(
            first.size.map(|s| s.size_mm),
            second.size.map(|s| s.size_mm)
        );
    }

    #[test]
    fn a_missing_file_still_answers_from_its_name() {
        let _one = one_at_a_time();
        let path = PathBuf::from("/no/such/dir/MCADPP0003371_3D.stp");
        prefetch(std::slice::from_ref(&path), None);
        wait_until_ready(std::slice::from_ref(&path));
        assert_eq!(identity_of(&path).doc_id.as_deref(), Some("MCADPP0003371"));
    }

    #[test]
    fn the_cache_forgets_the_oldest_files() {
        let mut s = State::default();
        for i in 0..(MAX_ENTRIES + 10) {
            let p = PathBuf::from(format!("/x/{i}.stp"));
            s.store(&p, stamp_of(&p), CadIdentity::from_file_name("x.stp"));
        }
        assert_eq!(s.done.len(), MAX_ENTRIES);
        assert!(!s.done.contains_key(Path::new("/x/0.stp")));
    }

    #[test]
    fn a_batch_larger_than_the_cache_is_still_read_whole() {
        let _one = one_at_a_time();
        let dir = tempfile::tempdir().unwrap();
        let paths: Vec<PathBuf> = (0..MAX_ENTRIES + 100)
            .map(|i| {
                let p = dir.path().join(format!("BIG-{i}.stp"));
                std::fs::write(&p, step_text(10.0 + (i % 50) as f64, 20.0, 30.0)).unwrap();
                p
            })
            .collect();
        // The window asks for its waiting files every frame until all are read.
        let started = Instant::now();
        loop {
            prefetch(&paths, None);
            if ready(&paths) {
                break;
            }
            assert!(
                started.elapsed() < Duration::from_secs(30),
                "{} files were never all read",
                paths.len()
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn asking_again_for_very_many_files_costs_time_in_proportion_to_their_number() {
        // A window waiting for a huge drop asks for all of its files every
        // frame; with the queue searched per file that was quadratic.
        let mut s = State::default();
        let paths: Vec<PathBuf> = (0..100_000)
            .map(|i| PathBuf::from(format!("/no/such/{i}.stp")))
            .collect();
        let started = Instant::now();
        s.ask(&paths, Instant::now(), true);
        for _ in 0..5 {
            s.ask(&paths, Instant::now(), false);
        }
        assert_eq!(s.queue.len(), paths.len(), "each file is queued once");
        assert_eq!(s.queued.len(), paths.len());
        assert_eq!(s.pending(), paths.len());
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "asking took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn storing_into_a_cache_of_wanted_files_does_not_search_all_of_it_each_time() {
        let mut s = State::default();
        let n = 200_000;
        let now = Instant::now();
        for i in 0..n {
            let p = PathBuf::from(format!("/w/{i}.stp"));
            s.asked.insert(p.clone(), now);
            // No disk call per file: on Windows 200,000 of them took longer
            // than KEEP_ASKED, so the entries stopped being wanted mid-loop.
            let stamp = Stamp {
                len: None,
                modified: None,
            };
            s.store(&p, stamp, CadIdentity::from_file_name("x.stp"));
        }
        assert_eq!(s.done.len(), n, "wanted files are all kept");
        assert!(
            now.elapsed() < Duration::from_secs(10),
            "storing took {:?}",
            now.elapsed()
        );
    }

    #[test]
    fn asking_without_checking_trusts_the_cache_and_asking_with_checking_does_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("X-1.stp");
        std::fs::write(&path, step_text(10.0, 20.0, 30.0)).unwrap();
        let paths = [path.clone()];
        let mut s = State::default();
        s.store(&path, stamp_of(&path), read_identity(&path));
        // A longer file, so the change shows whatever the clock's grain.
        std::fs::write(&path, step_text(12.0, 23.0, 4444444.0)).unwrap();
        s.ask(&paths, Instant::now(), false);
        assert_eq!(s.pending(), 0, "a file in the cache is trusted");
        s.ask(&paths, Instant::now(), true);
        assert_eq!(s.pending(), 1, "a changed file is read again");
    }

    #[test]
    fn files_that_are_still_wanted_are_not_forgotten() {
        let mut s = State::default();
        let wanted: Vec<PathBuf> = (0..MAX_ENTRIES + 5)
            .map(|i| PathBuf::from(format!("/w/{i}.stp")))
            .collect();
        for p in &wanted {
            s.asked.insert(p.clone(), Instant::now());
        }
        for p in &wanted {
            s.store(p, stamp_of(p), CadIdentity::from_file_name("x.stp"));
        }
        assert_eq!(
            s.done.len(),
            wanted.len(),
            "nothing asked for just now goes"
        );
        // Once they have not been asked for a while, the oldest go.
        s.asked.clear();
        let extra = PathBuf::from("/w/extra.stp");
        s.store(
            &extra,
            stamp_of(&extra),
            CadIdentity::from_file_name("x.stp"),
        );
        assert_eq!(s.done.len(), MAX_ENTRIES);
    }
}
