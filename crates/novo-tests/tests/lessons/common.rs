//! Shared helpers: the project files, and a lint for `lesson.toml` until
//! `novo-learn` brings the real one.

use std::collections::BTreeSet;
use std::path::PathBuf;

use novo_core::Project;

pub fn lessons_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/lessons")
}

fn updating() -> bool {
    std::env::var_os("UPDATE_LESSONS").is_some()
}

/// The lesson's `file` (`project.json` or `finished.json`) must equal what
/// the builder makes. With `UPDATE_LESSONS=1` the file is written instead.
pub fn check_file(id: &str, file: &str, project: &Project) {
    let json = format!("{}\n", project.to_json());
    let path = lessons_dir().join(id).join(file);
    if updating() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &json).unwrap();
        return;
    }
    let on_disk = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}. Run with UPDATE_LESSONS=1", path.display()));
    assert!(
        on_disk == json,
        "{} is out of date. Run: UPDATE_LESSONS=1 cargo test -p novo-tests --test lessons",
        path.display()
    );
    assert!(
        json.len() < 200 * 1024,
        "{id}/{file} is over 200 KB ({} bytes)",
        json.len()
    );
    Project::from_json(&on_disk).expect("the file loads");
}

const LESSON_KEYS: &[&str] = &[
    "format",
    "id",
    "version",
    "status",
    "title",
    "title_nl",
    "summary",
    "summary_nl",
    "track",
    "order",
    "level",
    "minutes",
    "topics",
    "needs",
    "next",
    "project",
    "finished",
    "page",
    "kind",
    "explain",
    "explain_nl",
    "show_problems",
    "step",
];
const STEP_KEYS: &[&str] = &[
    "id", "title", "title_nl", "text", "text_nl", "how", "how_nl", "point", "note", "expect",
    "solve", "pause", "demo", "hints", "hints_nl",
];

/// Lints one lesson folder against its sample project. Returns every
/// problem found, so a test can print them all at once.
pub fn lint(id: &str, project: &Project) -> Vec<String> {
    let mut problems = Vec::new();
    let dir = lessons_dir().join(id);
    let text = match std::fs::read_to_string(dir.join("lesson.toml")) {
        Ok(t) => t,
        Err(e) => return vec![format!("{id}: lesson.toml: {e}")],
    };
    let doc: toml::Table = match text.parse() {
        Ok(d) => d,
        Err(e) => return vec![format!("{id}: lesson.toml does not parse: {e}")],
    };
    let mut bad = |m: String| problems.push(format!("{id}: {m}"));

    for key in doc.keys() {
        if !LESSON_KEYS.contains(&key.as_str()) {
            bad(format!("unknown key `{key}`"));
        }
    }
    if doc.get("format").and_then(|v| v.as_integer()) != Some(1) {
        bad("format must be 1".into());
    }
    if doc.get("id").and_then(|v| v.as_str()) != Some(id) {
        bad("`id` must equal the folder name".into());
    }
    if !matches!(
        doc.get("status").and_then(|v| v.as_str()),
        Some("draft" | "early" | "ready")
    ) {
        bad("status must be draft, early or ready".into());
    }
    let tracks = std::fs::read_to_string(lessons_dir().join("tracks.toml")).unwrap();
    let tracks: toml::Table = tracks.parse().unwrap();
    let track_ids: Vec<&str> = tracks["track"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["id"].as_str())
        .collect();
    match doc.get("track").and_then(|v| v.as_str()) {
        Some(t) if track_ids.contains(&t) => {}
        other => bad(format!("track {other:?} is not in tracks.toml")),
    }
    for key in ["title", "summary", "level", "version", "order", "minutes"] {
        if !doc.contains_key(key) {
            bad(format!("missing `{key}`"));
        }
    }
    if !matches!(
        doc.get("level").and_then(|v| v.as_str()),
        Some("beginner" | "intermediate" | "advanced")
    ) {
        bad("level must be beginner, intermediate or advanced".into());
    }

    let tags: BTreeSet<String> = project.devices().map(|d| d.tag.to_string()).collect();
    let short_tag = |t: &str| tags.iter().any(|full| full == t || full.ends_with(t));
    let page_names: BTreeSet<&str> = project.pages().map(|p| p.title.as_str()).collect();
    let commands: BTreeSet<&str> = novo_workflow::commands::COMMANDS
        .iter()
        .map(|c| c.id)
        .collect();

    if let Some(page) = doc.get("page").and_then(|v| v.as_str())
        && !page_names.contains(page)
    {
        bad(format!("page `{page}` is not a page title in the project"));
    }

    let steps = doc
        .get("step")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !(6..=12).contains(&steps.len()) {
        bad(format!("{} steps; a lesson has 6 to 12", steps.len()));
    }
    let mut seen = BTreeSet::new();
    for (n, step) in steps.iter().enumerate() {
        let Some(step) = step.as_table() else {
            bad(format!("step {n} is not a table"));
            continue;
        };
        let sid = step.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let at = format!("step `{sid}`");
        if !seen.insert(sid.to_string())
            || !sid
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            bad(format!("{at}: id must be unique and [a-z0-9-]"));
        }
        for key in step.keys() {
            if !STEP_KEYS.contains(&key.as_str()) {
                bad(format!("{at}: unknown key `{key}`"));
            }
        }
        let text = step.get("text").and_then(|v| v.as_str()).unwrap_or("");
        if text.trim().is_empty() {
            bad(format!("{at}: no text"));
        }
        if text.lines().filter(|l| !l.trim().is_empty()).count() > 4 {
            bad(format!("{at}: text is over four lines"));
        }
        if text.matches('{').count() != text.matches('}').count() {
            bad(format!("{at}: unbalanced braces in text"));
        }
        for part in text.split("{key:").skip(1) {
            let cmd = part.split('}').next().unwrap_or("");
            if !commands.contains(cmd) {
                bad(format!("{at}: `{{key:{cmd}}}` is not a command"));
            }
        }
        // No `expect` is a reading step with a Next button (novo-learn lint).
        let expect = step.get("expect").and_then(|v| v.as_table());
        if let Some(cmd) = expect
            .and_then(|e| e.get("command"))
            .and_then(|v| v.as_str())
            && !commands.contains(cmd)
        {
            bad(format!("{at}: expect command `{cmd}` does not exist"));
        }
        // Device targets must exist in the starting project. To point at
        // something the person creates in this lesson, use `ui` or `area`.
        for key in ["point", "note"] {
            let targets: Vec<&toml::Value> = match step.get(key) {
                Some(toml::Value::Array(a)) => a.iter().collect(),
                Some(v) => vec![v],
                None => vec![],
            };
            for t in targets {
                if let Some(device) = t
                    .get("at")
                    .and_then(|a| a.get("device"))
                    .and_then(|d| d.as_str())
                    && !short_tag(device)
                {
                    bad(format!(
                        "{at}: {key} device `{device}` is not in the project"
                    ));
                }
            }
        }
    }
    for key in ["needs", "next"] {
        if let Some(list) = doc.get(key).and_then(|v| v.as_array()) {
            for other in list.iter().filter_map(|v| v.as_str()) {
                if !lessons_dir().join(other).exists() {
                    // Another thread's lesson may land later; say so softly.
                    eprintln!(
                        "note: {id}: `{key}` names `{other}`, which is not in assets/lessons yet"
                    );
                }
            }
        }
    }
    problems
}

/// Panics with every problem of every lesson given.
pub fn assert_lint(lessons: &[(&str, &Project)]) {
    let problems: Vec<String> = lessons.iter().flat_map(|(id, p)| lint(id, p)).collect();
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}
