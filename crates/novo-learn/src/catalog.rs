//! The set of lessons (design §1): tracks, glossary and lesson folders read
//! from a map of files, so the same code serves the lessons embedded in the
//! program, a folder on disk and the tests.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;

use crate::lesson::{Lang, Lesson, LessonError, Status};

/// Text files by path relative to the lessons folder, `/` separated:
/// `tracks.toml`, `glossary.toml`, `<id>/lesson.toml`, `<id>/project.json`.
pub type Files = BTreeMap<String, String>;

/// Reads a lessons folder into [`Files`]. Only `.toml` and `.json` files
/// are read; a file that is not UTF-8 text is an error.
pub fn read_dir(root: &Path) -> std::io::Result<Files> {
    fn walk(root: &Path, dir: &Path, out: &mut Files) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_dir() {
                walk(root, &path, out)?;
            } else if matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("toml" | "json")
            ) {
                let key = path
                    .strip_prefix(root)
                    .map_err(std::io::Error::other)?
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(key, std::fs::read_to_string(&path)?);
            }
        }
        Ok(())
    }
    let mut files = Files::new();
    walk(root, root, &mut files)?;
    Ok(files)
}

/// A group of lessons in the Learn window.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Track {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub title_nl: Option<String>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub summary_nl: Option<String>,
}

impl Track {
    pub fn title(&self, lang: Lang) -> &str {
        match (lang, &self.title_nl) {
            (Lang::Nl, Some(nl)) => nl,
            _ => &self.title,
        }
    }
}

/// A word explained once, for `{tip:term}`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Term {
    pub text: String,
    #[serde(default)]
    pub text_nl: Option<String>,
}

impl Term {
    pub fn text(&self, lang: Lang) -> &str {
        match (lang, &self.text_nl) {
            (Lang::Nl, Some(nl)) => nl,
            _ => &self.text,
        }
    }
}

/// Where a lesson's sample project comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectRef {
    /// The built-in motor starter, as Open example builds it.
    Example,
    /// A fresh project with one empty page.
    Blank,
    /// A JSON project among the files, by its path.
    File(String),
}

/// A lesson with where it was found.
#[derive(Debug, Clone)]
pub struct Entry {
    pub lesson: Arc<Lesson>,
    /// The lesson's folder in the files.
    pub dir: String,
    /// `None` when the lesson names a file that is not there.
    pub project: Option<ProjectRef>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TracksFile {
    #[serde(default)]
    track: Vec<Track>,
}

#[derive(Deserialize)]
struct GlossaryFile {
    #[serde(default)]
    term: BTreeMap<String, Term>,
}

/// Every lesson that parsed, in Learn-window order, and what did not.
#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub tracks: Vec<Track>,
    pub glossary: BTreeMap<String, Term>,
    pub entries: Vec<Entry>,
    /// Files that were refused: path and why.
    pub rejected: Vec<(String, LessonError)>,
    /// Problems in `tracks.toml` or `glossary.toml`.
    pub notes: Vec<String>,
}

impl Catalog {
    /// Builds the catalog. Lessons sort by track order, then `order`, then id.
    pub fn from_files(files: &Files) -> Catalog {
        let mut catalog = Catalog::default();
        if let Some(text) = files.get("tracks.toml") {
            match toml::from_str::<TracksFile>(text) {
                Ok(t) => catalog.tracks = t.track,
                Err(e) => catalog.notes.push(format!("tracks.toml: {e}")),
            }
        }
        if let Some(text) = files.get("glossary.toml") {
            match toml::from_str::<GlossaryFile>(text) {
                Ok(g) => catalog.glossary = g.term,
                Err(e) => catalog.notes.push(format!("glossary.toml: {e}")),
            }
        }
        for (path, text) in files {
            let Some(dir) = path.strip_suffix("/lesson.toml") else {
                continue;
            };
            if dir.contains('/') {
                continue;
            }
            match Lesson::parse(text) {
                Ok(lesson) => {
                    if lesson.id != dir {
                        catalog.rejected.push((
                            path.clone(),
                            LessonError::Invalid {
                                place: format!("lesson `{}`", lesson.id),
                                why: format!("the id must be the folder name `{dir}`"),
                            },
                        ));
                        continue;
                    }
                    let project = resolve_project(files, dir, &lesson.project);
                    catalog.entries.push(Entry {
                        lesson: Arc::new(lesson),
                        dir: dir.to_owned(),
                        project,
                    });
                }
                Err(e) => catalog.rejected.push((path.clone(), e)),
            }
        }
        let track_rank = |id: &str| {
            catalog
                .tracks
                .iter()
                .position(|t| t.id == id)
                .unwrap_or(usize::MAX)
        };
        catalog.entries.sort_by(|a, b| {
            (track_rank(&a.lesson.track), a.lesson.order, &a.lesson.id).cmp(&(
                track_rank(&b.lesson.track),
                b.lesson.order,
                &b.lesson.id,
            ))
        });
        catalog
    }

    pub fn get(&self, id: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.lesson.id == id)
    }

    /// The lessons to list: drafts only when asked for.
    pub fn visible(&self, show_drafts: bool) -> impl Iterator<Item = &Entry> {
        self.entries
            .iter()
            .filter(move |e| show_drafts || e.lesson.status == Status::Ready)
    }

    /// Lessons that carry a topic, for the "Lesson: ..." links in panels.
    pub fn for_topic<'a>(&'a self, topic: &'a str) -> impl Iterator<Item = &'a Entry> {
        self.visible(false)
            .filter(move |e| e.lesson.topics.iter().any(|t| t == topic))
    }
}

/// Resolves `project` against the lesson folder: `..` may climb to a
/// sibling lesson's folder but never out of the lessons folder.
fn resolve_project(files: &Files, dir: &str, project: &str) -> Option<ProjectRef> {
    match project {
        "@example" => return Some(ProjectRef::Example),
        "@blank" => return Some(ProjectRef::Blank),
        _ => {}
    }
    let mut parts: Vec<&str> = vec![dir];
    for part in project.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    let path = parts.join("/");
    files.contains_key(&path).then_some(ProjectRef::File(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lesson(id: &str, track: &str, order: i32, project: &str, status: &str) -> String {
        format!(
            "format = 1\nid = \"{id}\"\nversion = 1\nstatus = \"{status}\"\ntitle = \"{id}\"\nsummary = \"s\"\ntrack = \"{track}\"\norder = {order}\nproject = \"{project}\"\ntopics = [\"wiring\"]\n[[step]]\nid = \"a\"\ntitle = \"A\"\ntext = \"t\"\n"
        )
    }

    fn files() -> Files {
        let mut f = Files::new();
        f.insert(
            "tracks.toml".into(),
            "[[track]]\nid = \"start\"\ntitle = \"Getting started\"\n[[track]]\nid = \"draw\"\ntitle = \"Drawing\"\n".into(),
        );
        f.insert(
            "glossary.toml".into(),
            "[term.pin]\ntext = \"A connection point.\"\ntext_nl = \"Een aansluitpunt.\"\n".into(),
        );
        f.insert(
            "wiring/lesson.toml".into(),
            lesson("wiring", "draw", 20, "project.json", "ready"),
        );
        f.insert("wiring/project.json".into(), "{}".into());
        f.insert(
            "tour/lesson.toml".into(),
            lesson("tour", "start", 10, "@example", "ready"),
        );
        f.insert(
            "later/lesson.toml".into(),
            lesson("later", "draw", 10, "../wiring/project.json", "draft"),
        );
        f.insert(
            "lost/lesson.toml".into(),
            lesson("lost", "draw", 30, "missing.json", "ready"),
        );
        f.insert(
            "wrong/lesson.toml".into(),
            lesson("other", "draw", 40, "@blank", "ready"),
        );
        f.insert(
            "broken/lesson.toml".into(),
            "format = 1\nid = \"broken\"".into(),
        );
        f
    }

    #[test]
    fn lessons_sort_by_track_then_order_and_bad_ones_are_set_aside() {
        let c = Catalog::from_files(&files());
        let ids: Vec<_> = c.entries.iter().map(|e| e.lesson.id.as_str()).collect();
        assert_eq!(ids, ["tour", "later", "wiring", "lost"]);
        let rejected: Vec<_> = c.rejected.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(rejected, ["broken/lesson.toml", "wrong/lesson.toml"]);
        assert!(c.notes.is_empty());
        assert_eq!(c.glossary["pin"].text(Lang::Nl), "Een aansluitpunt.");
    }

    #[test]
    fn projects_resolve_inside_the_lessons_folder() {
        let c = Catalog::from_files(&files());
        assert_eq!(c.get("tour").unwrap().project, Some(ProjectRef::Example));
        assert_eq!(
            c.get("wiring").unwrap().project,
            Some(ProjectRef::File("wiring/project.json".into()))
        );
        assert_eq!(
            c.get("later").unwrap().project,
            Some(ProjectRef::File("wiring/project.json".into()))
        );
        assert_eq!(c.get("lost").unwrap().project, None);
        assert_eq!(resolve_project(&files(), "a", "../../x.json"), None);
    }

    #[test]
    fn drafts_are_hidden_unless_asked_for() {
        let c = Catalog::from_files(&files());
        assert!(c.visible(false).all(|e| e.lesson.id != "later"));
        assert!(c.visible(true).any(|e| e.lesson.id == "later"));
        let topic: Vec<_> = c
            .for_topic("wiring")
            .map(|e| e.lesson.id.as_str())
            .collect();
        assert_eq!(topic, ["tour", "wiring", "lost"]);
    }

    #[test]
    fn a_broken_tracks_file_is_noted_not_fatal() {
        let mut f = files();
        f.insert("tracks.toml".into(), "[[track]]\nidd = 1".into());
        let c = Catalog::from_files(&f);
        assert_eq!(c.notes.len(), 1);
        assert_eq!(c.entries.len(), 4);
    }

    #[test]
    fn read_dir_reads_toml_and_json_only() {
        let dir = std::env::temp_dir().join(format!("novo-learn-{}-dir", std::process::id()));
        std::fs::create_dir_all(dir.join("a")).unwrap();
        std::fs::write(dir.join("a/lesson.toml"), "x").unwrap();
        std::fs::write(dir.join("a/project.json"), "{}").unwrap();
        std::fs::write(dir.join("a/notes.txt"), "no").unwrap();
        let files = read_dir(&dir).unwrap();
        assert_eq!(
            files.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a/lesson.toml", "a/project.json"]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
