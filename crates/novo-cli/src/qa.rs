//! `kopercad-cli qa`: the test and screenshot suite. Runs scenario files
//! (`qa/scenarios/*.qa`) against the real app through the control harness,
//! saves labelled screenshots, runs cheap layout checks and writes a
//! machine-readable `results.json` plus an audit `report.md`.
//!
//! See `docs/testing/qa-suite.md` for the scenario format and the workflow.

use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::testuser::{Server, TestUser};
use crate::{Failure, exit};

#[derive(Debug, Subcommand)]
pub(crate) enum Qa {
    /// Run scenario files (or every `.qa` file in a folder) and write the
    /// results, screenshots and an audit report.
    Run(Run),
    /// List the scenarios and what each one does.
    List {
        /// Scenario files or folders (default: `qa/scenarios`).
        paths: Vec<PathBuf>,
    },
    /// Turn a `results.json` back into the Markdown report.
    Report {
        /// The results file written by `qa run`.
        results: PathBuf,
    },
}

#[derive(Debug, Args)]
pub(crate) struct Run {
    /// Scenario files or folders (default: `qa/scenarios`).
    paths: Vec<PathBuf>,
    /// Window sizes in points; repeat for several. Default: 1366x768 and 1920x1080.
    #[arg(long = "size")]
    sizes: Vec<String>,
    /// Output folder (default: `target/qa/<time>`). For a report other threads
    /// read, use `/mnt/project-files/qa/<name>`.
    #[arg(long)]
    out: Option<PathBuf>,
    /// The KoperCAD program to start (default: `kopercad` beside this tool).
    #[arg(long)]
    app: Option<PathBuf>,
    /// Only scenarios whose file name or title contains this text.
    #[arg(long)]
    only: Option<String>,
    /// Include the long sweeps (every command, every lesson).
    #[arg(long)]
    slow: bool,
    /// Folder that scenario paths such as `crates/...` are relative to
    /// (default: the repository root found from the current folder).
    #[arg(long)]
    root: Option<PathBuf>,
}

pub(crate) fn run(cmd: Qa, out: &mut dyn Write) -> Result<i32, Failure> {
    match cmd {
        Qa::List { paths } => {
            for s in load_all(&paths, None).map_err(usage)? {
                writeln!(
                    out,
                    "{:<28} {:<14} {:>3} steps  {}{}",
                    s.file,
                    s.area,
                    s.steps.len(),
                    s.title,
                    if s.slow { "  [slow: --slow]" } else { "" }
                )
                .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
            }
            Ok(exit::OK)
        }
        Qa::Report { results } => {
            let text = std::fs::read_to_string(&results)
                .map_err(|e| Failure::new(exit::READ, e.to_string()))?;
            let v: Value =
                serde_json::from_str(&text).map_err(|e| Failure::new(exit::READ, e.to_string()))?;
            let md = render_report(&v);
            let path = results.with_file_name("report.md");
            std::fs::write(&path, md).map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
            writeln!(out, "wrote {}", path.display())
                .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
            Ok(exit::OK)
        }
        Qa::Run(r) => run_scenarios(r, out),
    }
}

fn usage(e: String) -> Failure {
    Failure::new(exit::USAGE, e.to_string())
}

// ---- scenario files -----------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct Scenario {
    file: String,
    title: String,
    area: String,
    /// Sizes this scenario insists on (empty: the run's sizes).
    sizes: Vec<String>,
    /// Long scenarios (the sweeps) run only with `--slow`.
    slow: bool,
    /// Environment the app starts with (`@env NAME=VALUE`).
    env: Vec<(String, String)>,
    steps: Vec<StepLine>,
}

#[derive(Debug, Clone)]
struct StepLine {
    line: usize,
    text: String,
    words: Vec<String>,
}

fn load_all(paths: &[PathBuf], only: Option<&str>) -> Result<Vec<Scenario>, String> {
    let mut files = Vec::new();
    let defaults = [PathBuf::from("qa/scenarios")];
    for p in if paths.is_empty() {
        &defaults[..]
    } else {
        paths
    } {
        if p.is_dir() {
            let mut inner: Vec<PathBuf> = std::fs::read_dir(p)
                .map_err(|e| format!("{}: {e}", p.display()))?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "qa"))
                .collect();
            inner.sort();
            files.extend(inner);
        } else {
            files.push(p.clone());
        }
    }
    let mut out = Vec::new();
    for f in files {
        let text = std::fs::read_to_string(&f).map_err(|e| format!("{}: {e}", f.display()))?;
        let name = f
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("scenario")
            .trim_end_matches(".qa")
            .to_owned();
        let s = parse_scenario(&name, &text).map_err(|e| format!("{}: {e}", f.display()))?;
        if only.is_none_or(|o| {
            let o = o.to_lowercase();
            s.file.to_lowercase().contains(&o) || s.title.to_lowercase().contains(&o)
        }) {
            out.push(s);
        }
    }
    if out.is_empty() {
        return Err("no scenario files found".into());
    }
    Ok(out)
}

/// Header lines `@title`, `@area`, `@sizes`, `@slow`, `@env`; every other non-blank, non-`#`
/// line is one step: a verb followed by words (quotes group words).
pub(crate) fn parse_scenario(file: &str, text: &str) -> Result<Scenario, String> {
    let mut s = Scenario {
        file: file.to_owned(),
        title: file.to_owned(),
        area: "General".into(),
        sizes: Vec::new(),
        slow: false,
        env: Vec::new(),
        steps: Vec::new(),
    };
    for (i, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words = split_words(line).map_err(|e| format!("line {}: {e}", i + 1))?;
        if let Some(key) = words[0].strip_prefix('@') {
            let rest = words[1..].join(" ");
            match key {
                "title" => s.title = rest,
                "area" => s.area = rest,
                "sizes" => s.sizes = words[1..].to_vec(),
                "slow" => s.slow = true,
                "env" => {
                    let (name, value) = rest
                        .split_once('=')
                        .filter(|(n, _)| !n.is_empty())
                        .ok_or_else(|| format!("line {}: @env wants NAME=VALUE", i + 1))?;
                    s.env.push((name.to_owned(), value.to_owned()));
                }
                other => return Err(format!("line {}: unknown header @{other}", i + 1)),
            }
            continue;
        }
        if !KNOWN_VERBS.contains(&words[0].as_str()) {
            return Err(format!("line {}: unknown step `{}`", i + 1, words[0]));
        }
        s.steps.push(StepLine {
            line: i + 1,
            text: line.to_owned(),
            words,
        });
    }
    Ok(s)
}

const KNOWN_VERBS: &[&str] = &[
    "open",
    "fresh",
    "shot",
    "wait",
    "resize",
    "click_text",
    "click",
    "move",
    "drag",
    "scroll",
    "key",
    "text",
    "answer_dialog",
    "expect",
    "expect_not",
    "expect_scene",
    "expect_file",
    "expect_enabled",
    "expect_disabled",
    "check_layout",
    "check_blank",
    "tree",
    "sweep_commands",
    "sweep_lessons",
    "note",
    "try",
];

/// Splits on spaces; `"double quotes"` keep words together.
fn split_words(line: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut has = false;
    for c in line.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                has = true;
            }
            c if c.is_whitespace() && !quoted => {
                if has {
                    out.push(std::mem::take(&mut cur));
                    has = false;
                }
            }
            c => {
                cur.push(c);
                has = true;
            }
        }
    }
    if quoted {
        return Err("unclosed quote".into());
    }
    if has {
        out.push(cur);
    }
    Ok(out)
}

// ---- findings -----------------------------------------------------------

#[derive(Debug, Clone)]
pub(crate) struct Finding {
    pub severity: &'static str,
    pub kind: &'static str,
    pub area: String,
    pub title: String,
    pub scenario: String,
    pub size: String,
    pub steps: Vec<String>,
    pub expected: String,
    pub saw: String,
    pub screenshot: Option<String>,
}

impl Finding {
    fn json(&self, id: usize) -> Value {
        json!({
            "id": format!("F-{id:03}"),
            "severity": self.severity,
            "kind": self.kind,
            "area": self.area,
            "title": self.title,
            "scenario": self.scenario,
            "size": self.size,
            "steps": self.steps,
            "expected": self.expected,
            "saw": self.saw,
            "screenshot": self.screenshot,
        })
    }
}

// ---- cheap checks -------------------------------------------------------

fn rect(n: &Value) -> Option<[f64; 4]> {
    let r = n["rect"].as_array()?;
    Some([
        r[0].as_f64()?,
        r[1].as_f64()?,
        r[2].as_f64()?,
        r[3].as_f64()?,
    ])
}

/// A label for matching: icon glyphs (private-use characters) dropped, trimmed, lower case.
fn norm(label: &str) -> String {
    label
        .chars()
        .filter(|c| !('\u{e000}'..='\u{f8ff}').contains(c))
        .collect::<String>()
        .trim()
        .to_lowercase()
}

fn interactive(role: &str) -> bool {
    matches!(
        role.to_lowercase().as_str(),
        "button"
            | "menuitem"
            | "checkbox"
            | "radiobutton"
            | "textinput"
            | "combobox"
            | "slider"
            | "tab"
            | "link"
            | "switch"
    )
}

/// What the layout check found, as (severity, kind, title, detail).
pub(crate) type Hit = (&'static str, &'static str, String, String);

/// Clipping, overlap, tiny targets and unlabelled buttons from the widget
/// tree. `window` is the window size in points. Cheap and heuristic: it flags
/// what a person should look at, it does not prove a layout wrong.
pub(crate) fn layout_hits(nodes: &[Value], window: (f64, f64)) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    let (w, h) = window;
    // Scroll bars: content that runs past the window on a scrolled axis is
    // scrolling, not clipping. A bar as wide as the window is a finding itself.
    let bars: Vec<[f64; 4]> = nodes
        .iter()
        .filter(|n| {
            n["role"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case("scrollbar"))
        })
        .filter_map(rect)
        .collect();
    let windows: Vec<[f64; 4]> = nodes
        .iter()
        .filter(|n| {
            n["role"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case("window"))
        })
        .filter_map(rect)
        .collect();
    let vertical_scroll = bars.iter().any(|b| b[3] - b[1] > b[2] - b[0]);
    let horizontal_scroll = bars.iter().any(|b| b[2] - b[0] > b[3] - b[1]);
    let wide_bar = bars
        .iter()
        .any(|b| b[2] - b[0] >= 0.9 * w && b[3] - b[1] < b[2] - b[0] && b[1] < 0.5 * h);
    let mut clipped = Vec::new();
    let mut tiny = Vec::new();
    let mut unlabelled = 0usize;
    let mut items: Vec<(String, String, [f64; 4])> = Vec::new();
    for n in nodes {
        let role = n["role"].as_str().unwrap_or("");
        let label = n["label"].as_str().unwrap_or("").to_owned();
        let Some(r) = rect(n) else { continue };
        if !interactive(role) || n["disabled"] == json!(true) {
            continue;
        }
        let (rw, rh) = (r[2] - r[0], r[3] - r[1]);
        if rw <= 0.0 || rh <= 0.0 {
            continue; // not laid out (hidden)
        }
        let cut_x = r[0] < -2.0 || r[2] > w + 2.0;
        let cut_y = r[1] < -2.0 || r[3] > h + 2.0;
        let scrolled = (cut_x && horizontal_scroll) || (cut_y && vertical_scroll);
        let fully_visible = !cut_x && !cut_y;
        if (cut_x || cut_y) && !scrolled {
            clipped.push(format!(
                "{role} '{}' {:?}",
                short(&label),
                r.map(f64::round)
            ));
        }
        // A button that is just one or two characters (a page crumb "1") is as wide as its text.
        if (rw < 8.0 || rh < 8.0) && norm(&label).chars().count() > 2 {
            tiny.push(format!("{role} '{}' {rw:.0}x{rh:.0}", short(&label)));
        }
        if label.trim().is_empty() && role.eq_ignore_ascii_case("button") {
            unlabelled += 1;
        }
        if fully_visible {
            items.push((role.to_owned(), label, r));
        }
    }
    if wide_bar {
        hits.push((
            "S3",
            "layout",
            "a toolbar needs horizontal scrolling at this window width".into(),
            "A scroll bar as wide as the window sits in the upper part of the window: some controls are reachable only by scrolling sideways.".into(),
        ));
    }
    if !clipped.is_empty() {
        hits.push((
            "S3",
            "layout",
            format!("{} control(s) cut off by the window edge", clipped.len()),
            clipped.join("; "),
        ));
    }
    if !tiny.is_empty() {
        hits.push((
            "S3",
            "layout",
            format!("{} control(s) smaller than 8 points", tiny.len()),
            tiny.join("; "),
        ));
    }
    // Overlap: two small, fully visible, labelled controls whose boxes share
    // most of the smaller one. Wide rows (list entries under a floating dock)
    // are left out on purpose; at most five pairs are named.
    let mut overlaps = Vec::new();
    for (i, a) in items.iter().enumerate() {
        for b in &items[i + 1..] {
            if a.1.trim().is_empty() || b.1.trim().is_empty() || a.1 == b.1 {
                continue;
            }
            // A dialog or floating panel lies over the page by design: only
            // controls inside the same window are compared.
            let home = |r: &[f64; 4]| {
                let (cx, cy) = ((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0);
                windows
                    .iter()
                    .position(|w| cx >= w[0] && cx <= w[2] && cy >= w[1] && cy <= w[3])
            };
            if home(&a.2) != home(&b.2) {
                continue;
            }
            // A popup menu lies over the page by design.
            if a.0.eq_ignore_ascii_case("menuitem") != b.0.eq_ignore_ascii_case("menuitem") {
                continue;
            }
            let (aw, bw) = (a.2[2] - a.2[0], b.2[2] - b.2[0]);
            if aw > 300.0 || bw > 300.0 {
                continue;
            }
            let ox = (a.2[2].min(b.2[2]) - a.2[0].max(b.2[0])).max(0.0);
            let oy = (a.2[3].min(b.2[3]) - a.2[1].max(b.2[1])).max(0.0);
            let smaller = (aw * (a.2[3] - a.2[1])).min(bw * (b.2[3] - b.2[1]));
            if ox >= 4.0 && oy >= 4.0 && ox * oy >= 0.4 * smaller {
                overlaps.push(format!("'{}' over '{}'", short(&a.1), short(&b.1)));
            }
        }
    }
    if !overlaps.is_empty() {
        let n = overlaps.len();
        overlaps.truncate(5);
        hits.push((
            "S4",
            "layout-maybe",
            format!("{n} pair(s) of controls may overlap (dialogs and docks can stack on purpose; check the picture)"),
            overlaps.join("; "),
        ));
    }
    if unlabelled > 0 {
        hits.push((
            "S4",
            "accessibility",
            format!("{unlabelled} icon-only button(s) with no label in the widget tree"),
            "A screen reader and the test tool see no name; only a tooltip tells the user.".into(),
        ));
    }
    hits
}

/// A label cut to a readable length for a finding.
fn short(label: &str) -> String {
    let l = label.trim();
    if l.chars().count() > 40 {
        format!("{}...", l.chars().take(40).collect::<String>())
    } else {
        l.to_owned()
    }
}

/// Share of pixels that differ from the most common colour, 0 to 1. A frame
/// that is one flat colour (a blank or black window) gives about 0.
pub(crate) fn busy_fraction(png: &Path) -> Result<f64, String> {
    let img = image::open(png).map_err(|e| format!("{}: {e}", png.display()))?;
    let rgb = img.to_rgb8();
    let mut counts: std::collections::HashMap<[u8; 3], u32> = std::collections::HashMap::new();
    // Every 3rd pixel in both directions is plenty.
    let mut total = 0u32;
    for y in (0..rgb.height()).step_by(3) {
        for x in (0..rgb.width()).step_by(3) {
            *counts.entry(rgb.get_pixel(x, y).0).or_default() += 1;
            total += 1;
        }
    }
    let top = counts.values().copied().max().unwrap_or(0);
    Ok(if total == 0 {
        0.0
    } else {
        1.0 - f64::from(top) / f64::from(total)
    })
}

/// Lines of an app log that mean trouble.
pub(crate) fn log_trouble(log: &str) -> Vec<String> {
    log.lines()
        .filter(|l| {
            let l = l.to_lowercase();
            l.contains("panicked at")
                || l.contains("fatal runtime error")
                || l.contains("validation error")
                || l.contains("segmentation fault")
                || l.contains("stack overflow")
        })
        .take(6)
        .map(str::to_owned)
        .collect()
}

enum Palette {
    Ran,
    Disabled,
    Missing,
}

/// (id, title) of each shipped lesson that is not a draft.
fn lesson_titles(root: &Path) -> Vec<(String, String)> {
    let dir = root.join("assets/lessons");
    let mut out = Vec::new();
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return out;
    };
    for e in rd.flatten() {
        let Ok(text) = std::fs::read_to_string(e.path().join("lesson.toml")) else {
            continue;
        };
        let field = |key: &str| {
            text.lines().find_map(|l| {
                let rest = l.strip_prefix(key)?.trim_start().strip_prefix('=')?;
                Some(rest.trim().trim_matches('"').to_owned())
            })
        };
        if field("status").as_deref() == Some("draft") {
            continue;
        }
        if let (Some(id), Some(title)) = (field("id"), field("title")) {
            out.push((id, title));
        }
    }
    out.sort();
    out
}

// ---- running ------------------------------------------------------------

struct StepResult {
    n: usize,
    text: String,
    status: &'static str,
    ms: u128,
    error: Option<String>,
}

struct ScenarioRun {
    name: String,
    title: String,
    area: String,
    size: String,
    status: &'static str,
    steps: Vec<StepResult>,
    shots: Vec<String>,
    findings: Vec<usize>,
    seconds: f64,
}

struct Ctx<'a> {
    srv: Server,
    out: &'a Path,
    root: &'a Path,
    scenario: &'a Scenario,
    size: String,
    shot_n: usize,
    tree_n: usize,
    notes: Vec<String>,
    sweeps: &'a mut Vec<Value>,
    shots: Vec<String>,
    done: Vec<String>,
    findings: &'a mut Vec<Finding>,
    mine: Vec<usize>,
    last_shot: Option<String>,
}

impl Ctx<'_> {
    fn add(
        &mut self,
        severity: &'static str,
        kind: &'static str,
        title: String,
        expected: &str,
        saw: String,
    ) {
        // One screenshot of the moment, so the finding always has a picture.
        let shot = self
            .take_shot(&format!("finding-{}", self.findings.len() + 1))
            .ok();
        self.findings.push(Finding {
            severity,
            kind,
            area: self.scenario.area.clone(),
            title,
            scenario: self.scenario.file.clone(),
            size: self.size.clone(),
            steps: self.done.clone(),
            expected: expected.to_owned(),
            saw,
            screenshot: shot,
        });
        self.mine.push(self.findings.len() - 1);
    }

    fn take_shot(&mut self, label: &str) -> Result<String, String> {
        self.shot_n += 1;
        let label: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let name = format!(
            "{}-{}-{:02}-{}.png",
            self.scenario.file, self.size, self.shot_n, label
        );
        let path = self.srv.shot_file(&name)?;
        // The app writes where `--shots` points; keep the file there.
        let rel = path
            .strip_prefix(self.out)
            .map_or_else(|_| name.clone(), |p| p.to_string_lossy().replace('\\', "/"));
        self.shots.push(rel.clone());
        self.last_shot = Some(rel.clone());
        Ok(rel)
    }

    /// Runs each (id, label) through the command field and records what
    /// happened: found, unavailable, ran, new layout problems, a crash.
    fn sweep(&mut self, kind: &'static str, todo: Vec<(String, String)>) -> Result<(), String> {
        let mut rows: Vec<Value> = Vec::new();
        let total = todo.len();
        let mut before = self.layout_titles();
        for (i, (id, label)) in todo.into_iter().enumerate() {
            let log_before = std::fs::metadata(self.srv.app_log()).map_or(0, |m| m.len());
            self.done.push(format!("{kind}: {label} ({id})"));
            let outcome = self.run_in_palette(&label, kind == "lessons");
            let mut status = "ran";
            match outcome {
                Ok(Palette::Ran) => {}
                Ok(Palette::Disabled) => status = "unavailable",
                Ok(Palette::Missing) => {
                    status = "not-found";
                    self.add(
                        "S3",
                        "palette",
                        format!("{label} is not found by its own name in the command field"),
                        "typing the label lists the command",
                        format!("no row starting with `{label}` after typing it ({id})"),
                    );
                }
                Err(e) => {
                    status = "crashed";
                    self.add(
                        "S1",
                        "crash",
                        format!("{label} stopped the app"),
                        "the command runs or reports why not",
                        e,
                    );
                    // Carry on with a fresh app.
                    self.srv.stop();
                    self.srv.set_project(None);
                    self.srv.ensure()?;
                    let _ = self.step(&["click_text".into(), "Open example".into()]);
                    let _ = self.srv.tool("wait", &json!({"frames": 20}));
                }
            }
            if status == "ran" {
                if let (Ok(nodes), Ok(win)) = (self.tree(), self.window()) {
                    let hits = layout_hits(&nodes, win);
                    for (sev, k, title, detail) in &hits {
                        if !before.contains(title) {
                            self.add(
                                sev,
                                k,
                                format!("after {label}: {title}"),
                                "no new layout problem",
                                detail.clone(),
                            );
                        }
                    }
                    // The next command is judged against this state.
                    before = hits.into_iter().map(|h| h.2).collect();
                }
                let grown = std::fs::read_to_string(self.srv.app_log())
                    .map(|t| t.get(log_before as usize..).unwrap_or("").to_owned())
                    .unwrap_or_default();
                let trouble = log_trouble(&grown);
                if !trouble.is_empty() {
                    self.add(
                        "S1",
                        "log",
                        format!("{label} made the app log a panic or GPU error"),
                        "a clean log",
                        trouble.join(" | "),
                    );
                }
            }
            self.done.pop();
            rows.push(json!({"id": id, "label": label, "status": status}));
            // Whatever it opened, close it again.
            let _ = self.srv.tool("key", &json!({"keys": "Escape Escape"}));
            if (i + 1) % 25 == 0 {
                eprintln!("  ... {}/{total} {kind}", i + 1);
            }
        }
        let count = |s: &str| rows.iter().filter(|r| r["status"] == s).count();
        self.sweeps.push(json!({
            "scenario": self.scenario.file, "size": self.size, "kind": kind,
            "total": rows.len(), "ran": count("ran"), "unavailable": count("unavailable"),
            "not_found": count("not-found"), "crashed": count("crashed"), "rows": rows,
        }));
        Ok(())
    }

    fn layout_titles(&mut self) -> Vec<String> {
        match (self.tree(), self.window()) {
            (Ok(nodes), Ok(win)) => layout_hits(&nodes, win).into_iter().map(|h| h.2).collect(),
            _ => Vec::new(),
        }
    }

    /// Opens the command field, types `label`, clicks the row that starts
    /// with it (a lesson row ends in "Lesson").
    fn run_in_palette(&mut self, label: &str, lesson: bool) -> Result<Palette, String> {
        self.srv.tool("key", &json!({"keys": "Escape"}))?;
        self.srv.tool("key", &json!({"keys": "ctrl+k"}))?;
        self.srv.tool("wait", &json!({"frames": 2}))?;
        self.srv.tool("text", &json!({"text": label}))?;
        self.srv.tool("wait", &json!({"frames": 2}))?;
        let nodes = self.tree()?;
        let row = nodes.iter().find(|n| {
            let l = n["label"].as_str().unwrap_or("").trim_start();
            n["role"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case("button"))
                && l.strip_prefix(label)
                    .is_some_and(|rest| rest.starts_with("  ") || rest.starts_with(':'))
                && (!lesson || l.trim_end().ends_with("Lesson"))
        });
        let Some(row) = row else {
            return Ok(Palette::Missing);
        };
        if row["disabled"] == json!(true) {
            return Ok(Palette::Disabled);
        }
        let r = rect(row).ok_or("row has no rect")?;
        // A row below the window edge is scrolled out: Enter takes the first.
        if r[3] > 760.0 {
            self.srv.tool("key", &json!({"keys": "Enter"}))?;
        } else {
            let args = json!({"x": (r[0] + r[2]) / 2.0, "y": (r[1] + r[3]) / 2.0});
            self.srv.tool("click", &args)?;
        }
        self.srv.tool("wait", &json!({"frames": 6}))?;
        // Still answering?
        self.tree()?;
        Ok(Palette::Ran)
    }

    fn tree(&mut self) -> Result<Vec<Value>, String> {
        let r = self.srv.send(json!({"cmd": "ui_tree"}))?;
        Ok(r["nodes"].as_array().cloned().unwrap_or_default())
    }

    fn window(&mut self) -> Result<(f64, f64), String> {
        let r = self.srv.send(json!({"cmd": "info"}))?;
        Ok((
            r["size"][0].as_f64().unwrap_or(0.0),
            r["size"][1].as_f64().unwrap_or(0.0),
        ))
    }

    /// `~/` is the app's own home folder (where exports without a file dialog
    /// land, e.g. `~/Documents/KoperCAD/*.pdf`); other relative paths start at the repository.
    fn resolve(&self, p: &str) -> Result<PathBuf, String> {
        if let Some(rest) = p.strip_prefix("~/") {
            return app_home_path(rest);
        }
        let p = PathBuf::from(p);
        Ok(if p.is_absolute() {
            p
        } else {
            self.root.join(p)
        })
    }

    /// Runs one step. `Err` stops the scenario (the app is gone or the step
    /// cannot go on); findings are recorded and the run continues.
    fn step(&mut self, words: &[String]) -> Result<(), String> {
        let a = |i: usize| words.get(i).map(String::as_str);
        let need = |i: usize, what: &str| a(i).ok_or_else(|| format!("{} needs {what}", words[0]));
        let num = |i: usize, what: &str| -> Result<f64, String> {
            need(i, what)?
                .parse::<f64>()
                .map_err(|_| format!("{} needs a number for {what}", words[0]))
        };
        match words[0].as_str() {
            "open" => {
                let mut p = self.resolve(need(1, "a project file")?)?;
                if !p.exists() {
                    return Err(format!("project file {} not found", p.display()));
                }
                // A project in the repository is opened as a copy: the app autosaves edits,
                // and a run must not change a committed fixture.
                if p.starts_with(self.root) {
                    let dir = std::env::temp_dir()
                        .join(format!("kopercad-qa-open-{}", std::process::id()));
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    let copy = dir.join(p.file_name().unwrap_or_default());
                    std::fs::copy(&p, &copy)
                        .map_err(|e| format!("copying {}: {e}", p.display()))?;
                    p = copy;
                }
                self.srv.stop();
                // A project under `~/` was saved by this run: keep the profile folder it lives in.
                self.srv.keep_profile(words[1].starts_with("~/"));
                self.srv.set_project(Some(p));
                self.srv.ensure()?;
                Ok(())
            }
            "fresh" => {
                self.srv.stop();
                self.srv.keep_profile(false);
                self.srv.set_project(None);
                self.srv.ensure()
            }
            "shot" => self.take_shot(need(1, "a label")?).map(|_| ()),
            "wait" => {
                let frames = a(1).map_or(Ok(4.0), |_| num(1, "frames"))?;
                self.srv
                    .tool("wait", &json!({"frames": frames}))
                    .map(|_| ())
            }
            "resize" => {
                let args = json!({"w": num(1, "w")?, "h": num(2, "h")?});
                self.srv.tool("resize", &args)?;
                self.srv.tool("wait", &json!({"frames": 10}))?;
                let w = self.window()?;
                self.notes
                    .push(format!("window after resize: {}x{}", w.0, w.1));
                Ok(())
            }
            "click_text" => {
                // `click_text "Reports" #2` picks the second match: the same
                // label can be a menu and a workspace button.
                let want = need(1, "the label")?.to_lowercase();
                let mut role = None;
                let mut nth = 1usize;
                for w in &words[2..] {
                    if let Some(n) = w.strip_prefix('#') {
                        nth = n.parse().map_err(|_| format!("bad index {w}"))?;
                    } else {
                        role = Some(w.to_lowercase());
                    }
                }
                let nodes = self.tree()?;
                let ok = |n: &&Value, exact: bool| {
                    let label = norm(n["label"].as_str().unwrap_or(""));
                    (if exact {
                        label == want
                    } else {
                        label.contains(&want)
                    }) && n["disabled"] != json!(true)
                        && role.as_ref().is_none_or(|r| {
                            n["role"]
                                .as_str()
                                .is_some_and(|x| x.eq_ignore_ascii_case(r))
                        })
                        && rect(n).is_some_and(|r| r[2] > r[0] && r[3] > r[1])
                };
                let mut hits: Vec<&Value> = nodes.iter().filter(|n| ok(n, true)).collect();
                if hits.is_empty() {
                    hits = nodes.iter().filter(|n| ok(n, false)).collect();
                }
                let n = hits.get(nth - 1).ok_or_else(|| {
                    format!("no enabled widget labelled `{}` (match {nth})", words[1])
                })?;
                let r = rect(n).ok_or("widget has no rect")?;
                let args = json!({"x": (r[0] + r[2]) / 2.0, "y": (r[1] + r[3]) / 2.0});
                self.srv.tool("click", &args).map(|_| ())
            }
            "click" | "move" => {
                let args = json!({"x": num(1, "x")?, "y": num(2, "y")?});
                self.srv.tool(&words[0], &args).map(|_| ())
            }
            "drag" => {
                let args = json!({"from": [num(1, "x1")?, num(2, "y1")?],
                                  "to": [num(3, "x2")?, num(4, "y2")?]});
                self.srv.tool("drag", &args).map(|_| ())
            }
            "scroll" => {
                let args = json!({"x": num(1, "x")?, "y": num(2, "y")?, "dy": num(3, "dy")?});
                self.srv.tool("scroll", &args).map(|_| ())
            }
            "key" => {
                let keys = words[1..].join(" ");
                if keys.is_empty() {
                    return Err("key needs keys".into());
                }
                self.srv.tool("key", &json!({"keys": keys})).map(|_| ())
            }
            "text" => self
                .srv
                .tool("text", &json!({"text": words[1..].join(" ")}))
                .map(|_| ()),
            "answer_dialog" => {
                let p = self.resolve(need(1, "a path")?)?;
                // Only the app's home folder is created on demand; a typo elsewhere must not leave folders behind.
                if words[1].starts_with("~/")
                    && let Some(dir) = p.parent()
                {
                    let _ = std::fs::create_dir_all(dir);
                }
                self.srv
                    .tool("answer_dialog", &json!({"path": p.to_string_lossy()}))
                    .map(|_| ())
            }
            "expect" | "expect_not" => {
                let want = need(1, "text")?.to_lowercase();
                let nodes = self.tree()?;
                let present = nodes.iter().any(|n| {
                    n["label"]
                        .as_str()
                        .is_some_and(|l| l.to_lowercase().contains(&want))
                        || n["value"]
                            .as_str()
                            .is_some_and(|l| l.to_lowercase().contains(&want))
                });
                if words[0] == "expect" && !present {
                    self.add(
                        "S3",
                        "expect",
                        format!("`{}` not on screen", words[1]),
                        &format!("a control or text containing `{}`", words[1]),
                        "not found in the widget tree".into(),
                    );
                } else if words[0] == "expect_not" && present {
                    self.add(
                        "S3",
                        "expect",
                        format!("`{}` is on screen", words[1]),
                        &format!("nothing containing `{}`", words[1]),
                        "found in the widget tree".into(),
                    );
                }
                Ok(())
            }
            "expect_enabled" | "expect_disabled" => {
                // The control with this label (exact first) is enabled or disabled.
                let want = need(1, "a label")?.trim().to_lowercase();
                let nodes = self.tree()?;
                let label = |n: &Value| norm(n["label"].as_str().unwrap_or(""));
                let node = nodes
                    .iter()
                    .find(|n| label(n) == want && interactive(n["role"].as_str().unwrap_or("")))
                    .or_else(|| {
                        nodes.iter().find(|n| {
                            label(n).starts_with(&want)
                                && interactive(n["role"].as_str().unwrap_or(""))
                        })
                    });
                let Some(n) = node else {
                    let near: Vec<String> = nodes
                        .iter()
                        .filter(|n| label(n).contains(&want))
                        .map(|n| format!("{} {:?}", n["role"], n["label"]))
                        .collect();
                    return Err(format!(
                        "no control labelled `{}` (near: {})",
                        words[1],
                        near.join(", ")
                    ));
                };
                let disabled = n["disabled"] == json!(true);
                if (words[0] == "expect_disabled") != disabled {
                    let (exp, saw) = if disabled {
                        ("enabled", "disabled")
                    } else {
                        ("disabled", "enabled")
                    };
                    self.add(
                        "S3",
                        "expect",
                        format!("`{}` is {saw}", words[1]),
                        &format!("`{}` is {exp}", words[1]),
                        format!("the widget tree says {saw}"),
                    );
                }
                Ok(())
            }
            "expect_scene" => {
                let want = need(1, "text")?;
                let r = self.srv.send(json!({"cmd": "scene"}))?;
                if !r["scene"].to_string().contains(want) {
                    self.add(
                        "S3",
                        "expect",
                        format!("`{want}` not in the project data"),
                        &format!("the scene dump contains `{want}`"),
                        "not found".into(),
                    );
                }
                Ok(())
            }
            "expect_file" => {
                let path = self.resolve(need(1, "path")?)?;
                let min: u64 = a(2).and_then(|w| w.parse().ok()).unwrap_or(1);
                let len = biggest_match(&path);
                if len < min {
                    self.add(
                        "S2",
                        "expect",
                        format!("file `{}` missing or too small", path.display()),
                        &format!("the file exists and has at least {min} bytes"),
                        format!("{len} bytes"),
                    );
                }
                Ok(())
            }
            "check_layout" => {
                let nodes = self.tree()?;
                let win = self.window()?;
                for (sev, kind, title, detail) in layout_hits(&nodes, win) {
                    self.add(
                        sev,
                        kind,
                        title,
                        "every control fully visible and not overlapped",
                        detail,
                    );
                }
                Ok(())
            }
            "check_blank" => {
                let label = a(1).unwrap_or("blank-check");
                let rel = self.take_shot(label)?;
                let path = self.out.join(&rel);
                let busy = busy_fraction(&path)?;
                if busy < 0.02 {
                    self.add(
                        "S2",
                        "blank",
                        "the window is (almost) one flat colour".into(),
                        "a drawn workspace",
                        format!(
                            "only {:.2}% of pixels differ from the background",
                            busy * 100.0
                        ),
                    );
                }
                Ok(())
            }
            "sweep_commands" => {
                // Every command of the command field, by its own label.
                let mut group = None;
                let mut max = usize::MAX;
                for w in &words[1..] {
                    if let Some(n) = w.strip_prefix("max=") {
                        max = n.parse().map_err(|_| format!("bad {w}"))?;
                    } else {
                        group = Some(w.to_lowercase());
                    }
                }
                let skip = [
                    "file.new",
                    "file.open",
                    "file.open_read_only",
                    "file.close",
                    "file.quit",
                ];
                let todo: Vec<(String, String)> = novo_workflow::commands::COMMANDS
                    .iter()
                    .filter(|c| !skip.contains(&c.id))
                    .filter(|c| group.as_ref().is_none_or(|g| c.group.to_lowercase() == *g))
                    .map(|c| (c.id.to_owned(), c.label.to_owned()))
                    .take(max)
                    .collect();
                self.sweep("commands", todo)
            }
            "sweep_lessons" => {
                let todo = lesson_titles(self.root);
                if todo.is_empty() {
                    return Err("no lessons found under assets/lessons".into());
                }
                self.sweep("lessons", todo)
            }
            "tree" => {
                // For scenario authors: every labelled widget, saved beside the logs.
                let nodes = self.tree()?;
                let filter = a(1).map(str::to_lowercase);
                let mut text = String::new();
                for n in &nodes {
                    let line = format!(
                        "{} '{}' {} {}",
                        n["role"].as_str().unwrap_or("?"),
                        n["label"].as_str().unwrap_or(""),
                        n["value"].as_str().unwrap_or(""),
                        n["rect"]
                    );
                    if filter
                        .as_ref()
                        .is_none_or(|f| line.to_lowercase().contains(f))
                    {
                        text.push_str(&line);
                        text.push('\n');
                    }
                }
                self.tree_n += 1;
                let name = format!(
                    "logs/{}-{}-tree-{}.txt",
                    self.scenario.file, self.size, self.tree_n
                );
                std::fs::write(self.out.join(name), text).map_err(|e| e.to_string())
            }
            "note" => Ok(()),
            "try" => {
                // A step whose failure should not end the scenario.
                if words.len() > 1 {
                    let _ = self.step(&words[1..]);
                }
                Ok(())
            }
            other => Err(format!("unknown step {other}")),
        }
    }
}

fn run_scenarios(r: Run, out: &mut dyn Write) -> Result<i32, Failure> {
    let root = match &r.root {
        Some(p) => p.clone(),
        None => repo_root().unwrap_or_else(|| PathBuf::from(".")),
    };
    let paths: Vec<PathBuf> = r
        .paths
        .iter()
        .map(|p| if p.exists() { p.clone() } else { root.join(p) })
        .collect();
    let paths = if paths.is_empty() {
        vec![root.join("qa/scenarios")]
    } else {
        paths
    };
    let mut scenarios = load_all(&paths, r.only.as_deref()).map_err(usage)?;
    if !r.slow {
        scenarios.retain(|s| !s.slow);
    }
    if scenarios.is_empty() {
        return Err(usage("no scenario to run (the sweeps need --slow)".into()));
    }
    let sizes = if r.sizes.is_empty() {
        vec!["1366x768".to_owned(), "1920x1080".to_owned()]
    } else {
        r.sizes.clone()
    };
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let out_dir = r
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(format!("target/qa/{stamp}")));
    std::fs::create_dir_all(out_dir.join("logs"))
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;

    let mut findings: Vec<Finding> = Vec::new();
    let mut runs: Vec<ScenarioRun> = Vec::new();
    let mut sweeps: Vec<Value> = Vec::new();
    let mut info = Value::Null;
    for sc in &scenarios {
        let these: &[String] = if sc.sizes.is_empty() {
            &sizes
        } else {
            &sc.sizes
        };
        for size in these {
            let started = Instant::now();
            let opts = TestUser::for_qa(r.app.clone(), size, out_dir.join("shots"), sc.env.clone());
            let mut ctx = Ctx {
                srv: Server::new(opts),
                out: &out_dir,
                root: &root,
                scenario: sc,
                size: size.clone(),
                shot_n: 0,
                tree_n: 0,
                notes: Vec::new(),
                sweeps: &mut sweeps,
                shots: Vec::new(),
                done: Vec::new(),
                findings: &mut findings,
                mine: Vec::new(),
                last_shot: None,
            };
            writeln!(out, "== {} @ {}: {}", sc.file, size, sc.title).ok();
            let mut steps = Vec::new();
            let mut aborted = false;
            // The app starts on the first step, with a fresh profile.
            if let Err(e) = ctx.srv.ensure() {
                ctx.add(
                    "S1",
                    "crash",
                    "the app did not start".into(),
                    "a window and a control port",
                    e.clone(),
                );
                aborted = true;
            } else if let Ok(i) = ctx.srv.send(json!({"cmd": "info"})) {
                let want = size
                    .split_once('x')
                    .and_then(|(w, h)| Some([w.parse::<f64>().ok()?, h.parse::<f64>().ok()?]));
                let got = [i["size"][0].as_f64(), i["size"][1].as_f64()];
                if want.is_some_and(|w| got != [Some(w[0]), Some(w[1])]) {
                    ctx.add(
                        "S3",
                        "harness",
                        format!("the window is not {size}"),
                        &format!("a {size} window"),
                        format!(
                            "the window is {:?} points; shots at this size are not what was asked",
                            i["size"]
                        ),
                    );
                }
                if info.is_null() {
                    info = i;
                }
            }
            for (n, st) in sc.steps.iter().enumerate() {
                if aborted {
                    steps.push(StepResult {
                        n: n + 1,
                        text: st.text.clone(),
                        status: "skipped",
                        ms: 0,
                        error: None,
                    });
                    continue;
                }
                let t = Instant::now();
                ctx.done.push(st.text.clone());
                let res = ctx.step(&st.words);
                let (status, error) = match res {
                    Ok(()) => ("ok", None),
                    Err(e) => {
                        // A step error: the app died or the step could not run.
                        let gone = e.contains("app gone") || e.contains("app closed");
                        let (sev, kind, title) = if gone {
                            (
                                "S1",
                                "crash",
                                "the app stopped responding or exited".to_owned(),
                            )
                        } else {
                            ("S3", "step", format!("step failed: {}", st.text))
                        };
                        let saw = e.clone();
                        ctx.add(sev, kind, title, "the step runs", saw);
                        aborted = true;
                        ("failed", Some(format!("line {}: {e}", st.line)))
                    }
                };
                writeln!(out, "  {:<7} {}", status, st.text).ok();
                steps.push(StepResult {
                    n: n + 1,
                    text: st.text.clone(),
                    status,
                    ms: t.elapsed().as_millis(),
                    error,
                });
            }
            // The app's own log: panics and GPU validation errors count.
            let log_path = ctx.srv.app_log();
            let _ = ctx.srv.send(json!({"cmd": "quit"}));
            std::thread::sleep(Duration::from_millis(300));
            ctx.srv.stop();
            if let Ok(log) = std::fs::read_to_string(&log_path) {
                let trouble = log_trouble(&log);
                if !trouble.is_empty() {
                    let mut f = Finding {
                        severity: "S1",
                        kind: "log",
                        area: sc.area.clone(),
                        title: "the app log reports a panic or GPU error".into(),
                        scenario: sc.file.clone(),
                        size: size.clone(),
                        steps: ctx.done.clone(),
                        expected: "a clean log".into(),
                        saw: trouble.join(" | "),
                        screenshot: ctx.last_shot.clone(),
                    };
                    if !trouble.iter().any(|l| l.contains("panicked")) {
                        f.severity = "S3";
                    }
                    ctx.findings.push(f);
                    ctx.mine.push(ctx.findings.len() - 1);
                }
                let _ = std::fs::copy(
                    &log_path,
                    out_dir.join(format!("logs/{}-{}.log", sc.file, size)),
                );
            }
            let failed = steps.iter().any(|s| s.status == "failed")
                || ctx.mine.iter().any(|&i| ctx.findings[i].severity != "S4");
            let status = if failed { "fail" } else { "pass" };
            runs.push(ScenarioRun {
                name: sc.file.clone(),
                title: sc.title.clone(),
                area: sc.area.clone(),
                size: size.clone(),
                status,
                steps,
                shots: ctx.shots.clone(),
                findings: ctx.mine.clone(),
                seconds: started.elapsed().as_secs_f64(),
            });
        }
    }

    let failing = runs.iter().filter(|r| r.status == "fail").count();
    let results = json!({
        "tool": "kopercad-cli qa run",
        "generated": stamp,
        "platform": std::env::consts::OS,
        "app_info": info,
        "sizes": sizes,
        "summary": {
            "runs": runs.len(),
            "passed": runs.len() - failing,
            "failed": failing,
            "findings": findings.len(),
            "blocking": findings.iter().filter(|f| f.severity != "S4").count(),
        },
        "scenarios": runs.iter().map(|r| json!({
            "name": r.name, "title": r.title, "area": r.area, "size": r.size,
            "status": r.status, "seconds": (r.seconds * 10.0).round() / 10.0,
            "shots": r.shots,
            "findings": r.findings.iter().map(|i| format!("F-{:03}", i + 1)).collect::<Vec<_>>(),
            "steps": r.steps.iter().map(|s| json!({
                "n": s.n, "step": s.text, "status": s.status, "ms": s.ms, "error": s.error,
            })).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "findings": findings.iter().enumerate().map(|(i, f)| f.json(i + 1)).collect::<Vec<_>>(),
        "sweeps": sweeps,
    });
    let rjson = serde_json::to_string_pretty(&results)
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
    std::fs::write(out_dir.join("results.json"), rjson)
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
    std::fs::write(out_dir.join("report.md"), render_report(&results))
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
    std::fs::write(out_dir.join("summary.md"), render_summary(&results))
        .map_err(|e| Failure::new(exit::WRITE, e.to_string()))?;
    writeln!(
        out,
        "\n{} run(s), {} failed, {} finding(s). Report: {}",
        runs.len(),
        failing,
        findings.len(),
        out_dir.join("report.md").display()
    )
    .ok();
    Ok(if failing > 0 {
        exit::PROBLEMS
    } else {
        exit::OK
    })
}

fn repo_root() -> Option<PathBuf> {
    let mut d = std::env::current_dir().ok()?;
    loop {
        if d.join("Cargo.toml").exists() && d.join("crates").is_dir() {
            return Some(d);
        }
        if !d.pop() {
            return None;
        }
    }
}

/// A path under the app's own home folder. Only plain names are allowed: no
/// `..` and no root, so a scenario cannot leave that folder.
fn app_home_path(rest: &str) -> Result<PathBuf, String> {
    let plain = Path::new(rest)
        .components()
        .all(|c| matches!(c, std::path::Component::Normal(_)));
    if !plain {
        return Err(format!("`~/{rest}` leaves the app's home folder"));
    }
    Ok(std::env::temp_dir()
        .join(format!("kopercad-test-user-{}", std::process::id()))
        .join("home")
        .join(rest))
}

/// Size of the file, or the biggest file when the last component has one `*`
/// (`Motor*.pdf`); 0 when nothing matches.
fn biggest_match(path: &Path) -> u64 {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned());
    let Some((pre, post)) = name.as_deref().and_then(|n| n.split_once('*')) else {
        return std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    };
    let Some(dir) = path.parent() else { return 0 };
    // One `*` may also stand in the folder name (`*DXF/*.dxf`).
    let dir_name = dir.file_name().map(|n| n.to_string_lossy().into_owned());
    if let (Some((dpre, dpost)), Some(grand)) = (
        dir_name.as_deref().and_then(|n| n.split_once('*')),
        dir.parent(),
    ) {
        return std::fs::read_dir(grand)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.starts_with(dpre) && n.ends_with(dpost)
            })
            .map(|e| biggest_match(&e.path().join(path.file_name().unwrap_or_default())))
            .max()
            .unwrap_or(0);
    }
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.len() >= pre.len() + post.len() && n.starts_with(pre) && n.ends_with(post)
        })
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .max()
        .unwrap_or(0)
}

// ---- report -------------------------------------------------------------

/// The merge-gate view: one row per scenario, one column per window size,
/// then only the blocking findings (S1 to S3). Short enough for a PR comment.
pub(crate) fn render_summary(v: &Value) -> String {
    let sizes: Vec<String> = v["sizes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|x| x.as_str().map(str::to_string))
        .collect();
    let rows = v["scenarios"].as_array().cloned().unwrap_or_default();
    let mut names: Vec<String> = Vec::new();
    for r in &rows {
        let n = r["name"].as_str().unwrap_or("").to_string();
        if !names.contains(&n) {
            names.push(n);
        }
    }
    let sum = &v["summary"];
    let ok = sum["failed"].as_u64().unwrap_or(0) == 0 && sum["blocking"].as_u64().unwrap_or(0) == 0;
    let mut s = String::new();
    let _ = writeln!(
        s,
        "**QA gate: {}** ({} run(s), {} failed, {} blocking finding(s))\n",
        if ok { "PASS" } else { "FAIL" },
        sum["runs"],
        sum["failed"],
        sum["blocking"]
    );
    let _ = writeln!(s, "| Scenario | {} |", sizes.join(" | "));
    let _ = writeln!(s, "|---|{}", "---|".repeat(sizes.len()));
    for n in &names {
        let mut cells = Vec::new();
        for z in &sizes {
            let hit = rows
                .iter()
                .find(|r| r["name"].as_str() == Some(n) && r["size"].as_str() == Some(z));
            cells.push(match hit {
                None => "-".to_string(),
                Some(r) if r["status"].as_str() == Some("pass") => "pass".to_string(),
                Some(r) => {
                    let ids = r["findings"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                        .unwrap_or_default();
                    format!("FAIL {ids}")
                }
            });
        }
        let _ = writeln!(s, "| {} | {} |", n, cells.join(" | "));
    }
    let mut any = false;
    for f in v["findings"].as_array().into_iter().flatten() {
        if matches!(f["severity"].as_str(), Some("S1" | "S2" | "S3")) {
            if !any {
                let _ = writeln!(s, "\nBlocking findings:\n");
                any = true;
            }
            let _ = writeln!(
                s,
                "- {} {} ({}, {}): {}",
                f["id"].as_str().unwrap_or(""),
                f["severity"].as_str().unwrap_or(""),
                f["area"].as_str().unwrap_or(""),
                f["size"].as_str().unwrap_or(""),
                f["title"].as_str().unwrap_or("")
            );
        }
    }
    s
}

/// The audit report: summary, scenario table, then every finding with the
/// steps that led to it, what was expected, what was seen and the picture.
pub(crate) fn render_report(v: &Value) -> String {
    let mut s = String::new();
    let sum = &v["summary"];
    let _ = writeln!(s, "# KoperCAD QA run\n");
    let _ = writeln!(
        s,
        "Platform {}, window sizes {}. {} run(s): {} passed, {} failed; {} finding(s) ({} blocking, S1 to S3).\n",
        v["platform"].as_str().unwrap_or("?"),
        v["sizes"].as_array().map_or(String::new(), |a| a
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ")),
        sum["runs"],
        sum["passed"],
        sum["failed"],
        sum["findings"],
        sum["blocking"]
    );
    if let Some(i) = v["app_info"].as_object() {
        let _ = writeln!(
            s,
            "App: {}x{} points at scale {}.\n",
            i.get("size").map_or(Value::Null, |x| x[0].clone()),
            i.get("size").map_or(Value::Null, |x| x[1].clone()),
            i.get("pixels_per_point").cloned().unwrap_or(Value::Null)
        );
    }
    let _ = writeln!(
        s,
        "## Scenarios\n\n| Scenario | Area | Size | Result | Findings | Pictures |\n|---|---|---|---|---|---|"
    );
    for r in v["scenarios"].as_array().into_iter().flatten() {
        let f = r["findings"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default();
        let _ = writeln!(
            s,
            "| {} ({}) | {} | {} | {} | {} | {} |",
            r["title"].as_str().unwrap_or(""),
            r["name"].as_str().unwrap_or(""),
            r["area"].as_str().unwrap_or(""),
            r["size"].as_str().unwrap_or(""),
            r["status"].as_str().unwrap_or(""),
            if f.is_empty() { "-".into() } else { f },
            r["shots"].as_array().map_or(0, Vec::len),
        );
    }
    let sweeps = v["sweeps"].as_array().cloned().unwrap_or_default();
    if !sweeps.is_empty() {
        let _ = writeln!(
            s,
            "\n## Sweeps\n\n| Scenario | Size | Kind | Total | Ran | Unavailable | Not found | Crashed |\n|---|---|---|---|---|---|---|---|"
        );
        for w in &sweeps {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                w["scenario"].as_str().unwrap_or(""),
                w["size"].as_str().unwrap_or(""),
                w["kind"].as_str().unwrap_or(""),
                w["total"],
                w["ran"],
                w["unavailable"],
                w["not_found"],
                w["crashed"]
            );
        }
        let _ = writeln!(
            s,
            "\nPer-item results are in `results.json` under `sweeps`."
        );
    }
    let _ = writeln!(s, "\n## Findings\n");
    let findings = v["findings"].as_array().cloned().unwrap_or_default();
    if findings.is_empty() {
        let _ = writeln!(s, "None.\n");
    } else {
        let _ = writeln!(
            s,
            "| # | Sev | Area | Size | Title | Evidence |\n|---|---|---|---|---|---|"
        );
        for f in &findings {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} |",
                f["id"].as_str().unwrap_or(""),
                f["severity"].as_str().unwrap_or(""),
                f["area"].as_str().unwrap_or(""),
                f["size"].as_str().unwrap_or(""),
                f["title"].as_str().unwrap_or(""),
                f["screenshot"].as_str().unwrap_or("-"),
            );
        }
        for f in &findings {
            let _ = writeln!(
                s,
                "\n### {} {} {}\n",
                f["id"].as_str().unwrap_or(""),
                f["severity"].as_str().unwrap_or(""),
                f["title"].as_str().unwrap_or("")
            );
            let _ = writeln!(
                s,
                "Scenario `{}` at {}. Kind: {}.\n",
                f["scenario"].as_str().unwrap_or(""),
                f["size"].as_str().unwrap_or(""),
                f["kind"].as_str().unwrap_or("")
            );
            let steps = f["steps"].as_array().cloned().unwrap_or_default();
            if !steps.is_empty() {
                let _ = writeln!(s, "Steps:\n");
                for (i, st) in steps.iter().enumerate() {
                    let _ = writeln!(s, "{}. `{}`", i + 1, st.as_str().unwrap_or(""));
                }
                let _ = writeln!(s);
            }
            let _ = writeln!(s, "- Expected: {}", f["expected"].as_str().unwrap_or(""));
            let _ = writeln!(s, "- Saw: {}", f["saw"].as_str().unwrap_or(""));
            if let Some(p) = f["screenshot"].as_str() {
                let _ = writeln!(s, "\n![{}]({p})", f["id"].as_str().unwrap_or("shot"));
            }
        }
    }
    let _ = writeln!(
        s,
        "\nSeverity: S1 data loss, crash or cannot start. S2 a core workflow cannot be finished, a wrong number is shown as fact, or the window is blank. S3 everything else that is wrong or confusing. S4 polish and accessibility."
    );
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenario_parses_headers_quotes_and_comments() {
        let s = parse_scenario(
            "x",
            "# c\n@title Hello there\n@area Cabinet\n@sizes 1366x768\nclick_text \"Open example\"\nshot a\n",
        )
        .unwrap();
        assert_eq!(s.title, "Hello there");
        assert_eq!(s.area, "Cabinet");
        assert_eq!(s.sizes, ["1366x768"]);
        assert_eq!(s.steps.len(), 2);
        assert_eq!(s.steps[0].words, ["click_text", "Open example"]);
    }

    #[test]
    fn shipped_scenarios_all_parse_and_fixtures_exist() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let all = load_all(&[root.join("qa/scenarios")], None).unwrap();
        assert!(all.len() >= 7);
        for sc in &all {
            for st in &sc.steps {
                if matches!(st.words[0].as_str(), "open" | "answer_dialog")
                    && !st.words[1].starts_with("~/")
                {
                    let f = root.join(&st.words[1]);
                    assert!(f.exists(), "{}: {} missing", sc.file, f.display());
                }
            }
        }
        assert!(!lesson_titles(&root).is_empty());
    }

    #[test]
    fn unknown_step_and_open_quote_are_errors() {
        assert!(parse_scenario("x", "frobnicate\n").is_err());
        assert!(parse_scenario("x", "key \"ctrl+k\n").is_err());
        assert!(parse_scenario("x", "@bogus 1\n").is_err());
        let s = parse_scenario("x", "@env A=b=c\nshot a\n").unwrap();
        assert_eq!(s.env, vec![("A".to_string(), "b=c".to_string())]);
        assert!(parse_scenario("x", "@env nonsense\n").is_err());
        assert!(parse_scenario("x", "@env =x\n").is_err());
    }

    #[test]
    fn layout_flags_clipped_overlap_and_unlabelled() {
        let n =
            |role: &str, label: &str, r: [f64; 4]| json!({"role": role, "label": label, "rect": r});
        let nodes = vec![
            n("Button", "Fine", [10.0, 10.0, 80.0, 30.0]),
            n("Button", "Off", [1300.0, 10.0, 1400.0, 30.0]),
            n("Button", "A", [200.0, 200.0, 300.0, 230.0]),
            n("Button", "B", [210.0, 205.0, 310.0, 235.0]),
            n("Button", "", [10.0, 100.0, 40.0, 130.0]),
        ];
        let hits = layout_hits(&nodes, (1366.0, 768.0));
        let kinds: Vec<_> = hits.iter().map(|h| h.2.as_str()).collect();
        assert!(kinds.iter().any(|t| t.contains("cut off")), "{kinds:?}");
        assert!(kinds.iter().any(|t| t.contains("overlap")), "{kinds:?}");
        assert!(kinds.iter().any(|t| t.contains("icon-only")), "{kinds:?}");
        assert!(layout_hits(&nodes[..1], (1366.0, 768.0)).is_empty());
    }

    #[test]
    fn log_trouble_finds_panics_only() {
        assert!(log_trouble("all fine\nwarn: slow\n").is_empty());
        assert_eq!(
            log_trouble("thread 'main' panicked at src/x.rs:1:1:\nboom").len(),
            1
        );
    }

    #[test]
    fn home_paths_cannot_leave_the_home_folder() {
        assert!(app_home_path("Documents/KoperCAD/*.pdf").is_ok());
        assert!(app_home_path("../x").is_err());
        assert!(app_home_path("Documents/../../x/y").is_err());
        assert!(app_home_path("/etc/passwd").is_err());
    }

    #[test]
    fn report_lists_findings_with_steps_and_picture() {
        let v = json!({
            "platform": "linux", "sizes": ["1366x768"], "app_info": null,
            "summary": {"runs": 1, "passed": 0, "failed": 1, "findings": 1, "blocking": 1},
            "scenarios": [{"title": "T", "name": "t", "area": "A", "size": "1366x768",
                           "status": "fail", "findings": ["F-001"], "shots": ["a.png"]}],
            "findings": [{"id": "F-001", "severity": "S3", "kind": "layout", "area": "A",
                          "title": "cut off", "scenario": "t", "size": "1366x768",
                          "steps": ["click_text X"], "expected": "e", "saw": "s",
                          "screenshot": "a.png"}],
        });
        let md = render_report(&v);
        assert!(md.contains("F-001 S3 cut off"));
        assert!(md.contains("1. `click_text X`"));
        assert!(md.contains("![F-001](a.png)"));
        let sum = render_summary(&v);
        assert!(sum.contains("QA gate: FAIL"));
        assert!(sum.contains("| t | FAIL F-001 |"));
        assert!(sum.contains("- F-001 S3 (A, 1366x768): cut off"));
    }
}
