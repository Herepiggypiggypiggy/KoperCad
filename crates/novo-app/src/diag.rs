//! Diagnostics (REL-DIAG): a rolling log file, a panic hook that writes a crash
//! report, and the text of the problem report a tester can paste into a message.
//!
//! Everything stays on this computer, in `%LOCALAPPDATA%\KoperCAD\logs` (the
//! platform's per-user data folder elsewhere). Nothing is sent anywhere and no
//! admin rights are needed. Autosave and recovery do not use this module.

use std::fmt::Write as _;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// The current log file; older ones are `kopercad.1.log`, `kopercad.2.log`, ...
pub const LOG_FILE: &str = "kopercad.log";

/// Size at which the log file is rotated.
const LOG_LIMIT: u64 = 1024 * 1024;

/// Rotated log files kept next to the current one.
const LOG_KEEP: usize = 3;

/// Written by the panic hook, read and deleted on the next start.
const PENDING_FILE: &str = "crash-pending.txt";

/// Log lines copied into a crash or problem report.
const REPORT_LINES: usize = 40;

/// Bytes read from the end of the log to find those lines.
const TAIL_BYTES: u64 = 64 * 1024;

/// The commit the program was built from (see `build.rs`).
pub const BUILD_COMMIT: &str = env!("KOPERCAD_BUILD_COMMIT");

static LOG_DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
static LAST_COMMAND: Mutex<String> = Mutex::new(String::new());

thread_local! {
    /// The command this thread ran last; the window thread's is the one that
    /// matters for a crash, and it cannot be overwritten by another thread.
    static THREAD_COMMAND: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
}
static GPU: OnceLock<String> = OnceLock::new();
static OS: OnceLock<String> = OnceLock::new();

/// `%LOCALAPPDATA%\KoperCAD\logs`, or the platform's per-user data folder.
/// With a profile folder (`--profile-dir`) it is `<profile>/local/KoperCAD/logs`.
pub fn default_log_dir() -> Option<PathBuf> {
    let env = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    let base = if let Some(root) = novo_storage::profile::root() {
        root.join("local")
    } else if cfg!(windows) {
        env("LOCALAPPDATA")?
    } else if cfg!(target_os = "macos") {
        env("HOME")?.join("Library/Application Support")
    } else {
        env("XDG_DATA_HOME").or_else(|| env("HOME").map(|h| h.join(".local/share")))?
    };
    Some(base.join(crate::DATA_DIR_NAME).join("logs"))
}

/// The folder the log and crash reports are in, once [`init`] has run.
pub fn log_dir() -> Option<PathBuf> {
    lock(&LOG_DIR).clone()
}

/// Sets up the log file and the panic hook. Call once, first thing in `main`.
/// Without a usable folder the log goes to the console, as before.
pub fn init() {
    let dir = default_log_dir().filter(|d| std::fs::create_dir_all(d).is_ok());
    let env = env_logger::Env::default().default_filter_or("info");
    let mut builder = env_logger::Builder::from_env(env);
    if let Some(dir) = &dir {
        builder
            .format(|buf, record| {
                writeln!(
                    buf,
                    "{} {:<5} {} {}",
                    utc_stamp(SystemTime::now()),
                    record.level(),
                    record.target(),
                    record.args()
                )
            })
            .target(env_logger::Target::Pipe(Box::new(RollingFile::open(
                dir.join(LOG_FILE),
                LOG_LIMIT,
                LOG_KEEP,
            ))));
    }
    // A second init (tests) must not abort the program.
    let _ = builder.try_init();
    *lock(&LOG_DIR) = dir;
    install_panic_hook();
    log::info!(
        "KoperCAD {} ({BUILD_COMMIT}) starting on {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    // The Windows version needs a child process; do not hold the window up.
    std::thread::spawn(|| {
        let _ = OS.set(os_description());
    });
}

/// Remembers the command that is running, for the crash report.
pub fn note_command(id: &str) {
    THREAD_COMMAND.with(|c| {
        let mut c = c.borrow_mut();
        c.clear();
        c.push_str(id);
    });
    if let Ok(mut last) = LAST_COMMAND.try_lock() {
        last.clear();
        last.push_str(id);
    }
}

/// Remembers the graphics adapter, for the reports.
pub fn set_gpu(description: String) {
    log::info!("Graphics: {description}");
    let _ = GPU.set(description);
}

/// The text a tester pastes into a message: version, build, system, graphics
/// and the last log lines.
pub fn problem_report() -> String {
    let mut out = String::new();
    write_header(&mut out);
    let _ = writeln!(out, "Last command: {}", last_command());
    if let Some(dir) = log_dir() {
        let _ = writeln!(out, "Log folder: {}", dir.display());
        out.push('\n');
        let _ = writeln!(out, "Last log lines:");
        out.push_str(&log_tail(&dir, REPORT_LINES));
    } else {
        out.push_str("\nNo log folder on this computer.\n");
    }
    mask_user(&out)
}

/// The crash report the last run left behind, if it left one. Asking removes
/// the marker, so the offer is made once.
pub fn take_pending_crash() -> Option<PathBuf> {
    take_pending_in(&log_dir()?)
}

/// Removes the crash marker on a clean exit: a panic the program survived
/// (a solver that failed and was reported) must not read as a crash next time.
/// The report file itself stays in the log folder.
pub fn clear_pending_crash() {
    if let Some(dir) = log_dir() {
        let _ = std::fs::remove_file(dir.join(PENDING_FILE));
    }
}

fn take_pending_in(dir: &Path) -> Option<PathBuf> {
    let marker = dir.join(PENDING_FILE);
    let name = std::fs::read_to_string(&marker).ok()?;
    let _ = std::fs::remove_file(&marker);
    // Only a file name: the marker must not point outside the log folder.
    let name = Path::new(name.trim()).file_name()?.to_owned();
    let report = dir.join(name);
    report.is_file().then_some(report)
}

fn write_header(out: &mut String) {
    let _ = writeln!(
        out,
        "Local only: this stays on this computer unless you share it. Read it first; \
         your user name is masked in paths."
    );
    let _ = writeln!(
        out,
        "KoperCAD {} (build {BUILD_COMMIT}, {})",
        env!("CARGO_PKG_VERSION"),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
    );
    let _ = writeln!(
        out,
        "System: {} ({} {})",
        OS.get().map_or("unknown", String::as_str),
        std::env::consts::OS,
        std::env::consts::ARCH,
    );
    let _ = writeln!(
        out,
        "Graphics: {}",
        GPU.get().map_or("not known yet", String::as_str)
    );
    let _ = writeln!(out, "Time: {}", utc_stamp(SystemTime::now()));
}

fn last_command() -> String {
    let own = THREAD_COMMAND.with(|c| c.borrow().clone());
    if !own.is_empty() {
        return own;
    }
    match LAST_COMMAND.try_lock() {
        Ok(last) if !last.is_empty() => last.clone(),
        Ok(_) => "none yet".to_owned(),
        Err(_) => "unknown".to_owned(),
    }
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Never panic in here: a failed report must not hide the real one.
        let dir = LOG_DIR.try_lock().ok().and_then(|d| d.clone());
        if let Some(dir) = dir {
            write_crash_report(&dir, info);
        }
        previous(info);
    }));
}

fn write_crash_report(dir: &Path, info: &std::panic::PanicHookInfo<'_>) {
    let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
        (*s).to_owned()
    } else if let Some(s) = info.payload().downcast_ref::<String>() {
        s.clone()
    } else {
        "(no message)".to_owned()
    };
    let place = info
        .location()
        .map_or_else(|| "unknown".to_owned(), ToString::to_string);
    let thread = std::thread::current();
    let mut out = String::from("KoperCAD crash report\n\n");
    write_header(&mut out);
    let _ = writeln!(out, "Last command: {}", last_command());
    let _ = writeln!(
        out,
        "Thread: {}",
        thread.name().unwrap_or("(unnamed thread)")
    );
    let _ = writeln!(out, "\nPanic: {message}\nAt: {place}");
    let _ = writeln!(
        out,
        "\nBacktrace:\n{}",
        std::backtrace::Backtrace::force_capture()
    );
    let _ = writeln!(out, "Last log lines:");
    out.push_str(&log_tail(dir, REPORT_LINES));

    let stamp = file_stamp(SystemTime::now());
    let mut name = format!("crash-{stamp}.txt");
    let mut n = 2;
    while dir.join(&name).exists() {
        name = format!("crash-{stamp}-{n}.txt");
        n += 1;
    }
    if std::fs::write(dir.join(&name), mask_user(&out)).is_ok() {
        let _ = std::fs::write(dir.join(PENDING_FILE), &name);
    }
    log::error!("Panic at {place}: {message} (report: {name})");
    log::logger().flush();
}

/// The last `lines` lines of the current log file.
fn log_tail(dir: &Path, lines: usize) -> String {
    let current = tail_of(&dir.join(LOG_FILE), lines);
    // Just after a rotation the current file is short: the older one fills in.
    let have = current.as_ref().map_or(0, |t| t.lines().count());
    if have < lines
        && let Some(older) = tail_of(&dir.join("kopercad.1.log"), lines - have)
    {
        let mut out = older;
        if let Some(cur) = current {
            out.push_str(&cur);
        }
        return out;
    }
    current.unwrap_or_else(|| "(no log file)\n".to_owned())
}

fn tail_of(path: &Path, lines: usize) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().map_or(0, |m| m.len());
    let start = len.saturating_sub(TAIL_BYTES);
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(start)).is_err() || file.read_to_end(&mut bytes).is_err() {
        return None;
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut all: Vec<&str> = text.lines().collect();
    if start > 0 && !all.is_empty() {
        all.remove(0); // cut mid-line
    }
    let from = all.len().saturating_sub(lines);
    let mut out = all[from..].join("\n");
    out.push('\n');
    Some(out)
}

/// Replaces the user's name in `\Users\name\` style paths, for the reports.
fn mask_user(text: &str) -> String {
    let mut out = text.to_owned();
    for var in ["USERNAME", "USER"] {
        if let Some(name) = std::env::var_os(var).and_then(|v| v.into_string().ok()) {
            out = mask_name(&out, &name);
        }
    }
    out
}

/// `name` between path separators (or ending a path) becomes `<user>`,
/// whatever its case. Names under three characters are left alone: they
/// would also hit ordinary words.
fn mask_name(text: &str, name: &str) -> String {
    let name = name.trim();
    if name.chars().count() < 3 {
        return text.to_owned();
    }
    let (lower, needle) = (text.to_lowercase(), name.to_lowercase());
    if lower.len() != text.len() {
        return text.to_owned(); // case folding moved the offsets
    }
    let is_end = |c: Option<char>| {
        c.is_none_or(|c| matches!(c, '\\' | '/' | '"' | '\'' | ')') || c.is_whitespace())
    };
    let mut out = String::new();
    let mut at = 0;
    while let Some(found) = lower[at..].find(&needle) {
        let start = at + found;
        let end = start + needle.len();
        let before = text[..start].chars().next_back();
        let after = text[end..].chars().next();
        out.push_str(&text[at..start]);
        if matches!(before, Some('\\' | '/')) && is_end(after) {
            out.push_str("<user>");
        } else {
            out.push_str(&text[start..end]);
        }
        at = end;
    }
    out.push_str(&text[at..]);
    out
}

/// A log file that starts a new one when it is full: `kopercad.log` becomes
/// `kopercad.1.log`, that one `kopercad.2.log`, and the oldest is deleted.
struct RollingFile {
    path: PathBuf,
    limit: u64,
    keep: usize,
    inner: Mutex<Option<(std::fs::File, u64)>>,
}

impl RollingFile {
    fn open(path: PathBuf, limit: u64, keep: usize) -> Self {
        let this = Self {
            path,
            limit,
            keep,
            inner: Mutex::new(None),
        };
        *lock(&this.inner) = this.open_append();
        this
    }

    fn open_append(&self) -> Option<(std::fs::File, u64)> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .ok()?;
        let len = file.metadata().map_or(0, |m| m.len());
        Some((file, len))
    }

    fn numbered(&self, n: usize) -> PathBuf {
        let stem = self
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.path.with_file_name(format!("{stem}.{n}.log"))
    }

    fn rotate(&self) {
        let _ = std::fs::remove_file(self.numbered(self.keep));
        for n in (1..self.keep).rev() {
            let _ = std::fs::rename(self.numbered(n), self.numbered(n + 1));
        }
        let _ = std::fs::rename(&self.path, self.numbered(1));
    }
}

impl Write for &RollingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = lock(&self.inner);
        if inner.as_ref().is_some_and(|(_, len)| *len >= self.limit) {
            *inner = None;
            self.rotate();
        }
        if inner.is_none() {
            *inner = self.open_append();
        }
        if let Some((file, len)) = inner.as_mut() {
            file.write_all(buf)?;
            *len += buf.len() as u64;
        }
        // A log that cannot be written must not take the program down.
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        if let Some((file, _)) = lock(&self.inner).as_mut() {
            file.flush()?;
        }
        Ok(())
    }
}

impl Write for RollingFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        (&*self).write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        (&*self).flush()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Windows version from `cmd /C ver`, or the OS name where that does not apply.
fn os_description() -> String {
    if cfg!(windows) {
        let mut cmd = std::process::Command::new("cmd");
        cmd.args(["/C", "ver"]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        if let Ok(out) = cmd.output() {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if !text.is_empty() {
                return text;
            }
        }
    } else if let Ok(text) = std::fs::read_to_string("/etc/os-release")
        && let Some(name) = text.lines().find_map(|l| l.strip_prefix("PRETTY_NAME="))
    {
        return name.trim_matches('"').to_owned();
    }
    std::env::consts::OS.to_owned()
}

/// `2026-10-17 09:34:15 UTC`.
fn utc_stamp(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}:{s:02} UTC")
}

/// `20261017-093415`, for file names.
fn file_stamp(t: SystemTime) -> String {
    let (y, mo, d, h, mi, s) = civil(t);
    format!("{y:04}{mo:02}{d:02}-{h:02}{mi:02}{s:02}")
}

/// Calendar date and time (UTC) of `t` (days-to-civil, Howard Hinnant).
fn civil(t: SystemTime) -> (i64, u32, u32, u32, u32, u32) {
    let secs = t
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(0));
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (
        y,
        m,
        d,
        (rem / 3600) as u32,
        (rem % 3600 / 60) as u32,
        (rem % 60) as u32,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// The tests share the global log folder.
    static GLOBALS: Mutex<()> = Mutex::new(());

    #[test]
    fn stamps_match_known_dates() {
        let t = |s| UNIX_EPOCH + Duration::from_secs(s);
        assert_eq!(utc_stamp(t(0)), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc_stamp(t(1_781_689_455)), "2026-06-17 09:44:15 UTC");
        assert_eq!(file_stamp(t(951_782_400)), "20000229-000000");
    }

    #[test]
    fn the_log_rolls_over_and_keeps_a_few_old_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = RollingFile::open(dir.path().join(LOG_FILE), 100, 2);
        for i in 0..40 {
            writeln!(log, "line number {i:03} with some padding to fill it").unwrap();
        }
        let names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert!(names.contains(&"kopercad.log".to_owned()), "{names:?}");
        assert!(names.contains(&"kopercad.1.log".to_owned()), "{names:?}");
        assert!(names.contains(&"kopercad.2.log".to_owned()), "{names:?}");
        assert!(!names.contains(&"kopercad.3.log".to_owned()), "{names:?}");
        let current = std::fs::read_to_string(dir.path().join(LOG_FILE)).unwrap();
        assert!(current.contains("line number 039"));
    }

    #[test]
    fn the_tail_holds_the_last_lines_only() {
        let dir = tempfile::tempdir().unwrap();
        let text: String = (0..100).map(|i| format!("entry {i}\n")).collect();
        std::fs::write(dir.path().join(LOG_FILE), text).unwrap();
        let tail = log_tail(dir.path(), 3);
        assert_eq!(tail, "entry 97\nentry 98\nentry 99\n");
        let empty = tempfile::tempdir().unwrap();
        assert!(log_tail(empty.path(), 3).contains("no log file"));
        // Right after a rotation the older file fills the tail.
        let fresh = tempfile::tempdir().unwrap();
        std::fs::write(fresh.path().join(LOG_FILE), "new 1\n").unwrap();
        std::fs::write(fresh.path().join("kopercad.1.log"), "old 1\nold 2\nold 3\n").unwrap();
        assert_eq!(log_tail(fresh.path(), 3), "old 2\nold 3\nnew 1\n");
    }

    #[test]
    fn a_forced_panic_writes_a_crash_report_that_is_offered_once() {
        let _g = lock(&GLOBALS);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOG_FILE), "earlier line\n").unwrap();
        *lock(&LOG_DIR) = Some(dir.path().to_owned());
        install_panic_hook();
        note_command("test.forced_panic");
        let handle = std::thread::Builder::new()
            .name("forced-panic".into())
            .spawn(|| panic!("forced test panic"))
            .unwrap();
        assert!(handle.join().is_err());

        let report = take_pending_in(dir.path()).expect("a crash report is offered");
        let text = std::fs::read_to_string(&report).unwrap();
        assert!(
            report
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("crash-")
        );
        for needle in [
            "forced test panic",
            "forced-panic",
            "test.forced_panic",
            "Backtrace:",
            "earlier line",
            BUILD_COMMIT,
            env!("CARGO_PKG_VERSION"),
        ] {
            assert!(text.contains(needle), "{needle} missing in:\n{text}");
        }
        assert!(
            take_pending_in(dir.path()).is_none(),
            "the offer is made once"
        );
        *lock(&LOG_DIR) = None;
    }

    #[test]
    fn a_marker_cannot_point_outside_the_log_folder_and_a_clean_exit_removes_it() {
        let _g = lock(&GLOBALS);
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(
            dir.path().join(PENDING_FILE),
            outside.path().to_string_lossy().as_bytes(),
        )
        .unwrap();
        assert!(take_pending_in(dir.path()).is_none());

        std::fs::write(dir.path().join("crash-x.txt"), "report").unwrap();
        std::fs::write(dir.path().join(PENDING_FILE), "crash-x.txt").unwrap();
        *lock(&LOG_DIR) = Some(dir.path().to_owned());
        clear_pending_crash();
        *lock(&LOG_DIR) = None;
        assert!(
            take_pending_in(dir.path()).is_none(),
            "a survived panic is no crash"
        );
        assert!(dir.path().join("crash-x.txt").is_file(), "the report stays");
    }

    #[test]
    fn user_names_are_masked_in_paths_only() {
        let text = "C:\\Users\\Tvelzel\\Documents\\a.novo and /home/tvelzel/x, tvelzel wrote";
        assert_eq!(
            mask_name(text, "tvelzel"),
            "C:\\Users\\<user>\\Documents\\a.novo and /home/<user>/x, tvelzel wrote"
        );
        assert_eq!(
            mask_name("/a/ab/b", "ab"),
            "/a/ab/b",
            "short names are left"
        );
    }

    #[test]
    fn the_problem_report_names_version_build_and_log() {
        let _g = lock(&GLOBALS);
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(LOG_FILE), "a log line\n").unwrap();
        *lock(&LOG_DIR) = Some(dir.path().to_owned());
        let text = problem_report();
        for needle in [
            "KoperCAD ",
            "Graphics:",
            "System:",
            "a log line",
            BUILD_COMMIT,
        ] {
            assert!(text.contains(needle), "{needle} missing in:\n{text}");
        }
        *lock(&LOG_DIR) = None;
    }
}
