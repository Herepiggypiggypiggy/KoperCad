//! A person's progress (design §8): one small JSON file, local, never
//! uploaded. The application chooses the path (`learn.json` under the
//! user's config folder) and the date; this module only reads and writes.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::lesson::Lesson;
use crate::runner::{Phase, Run};

/// The progress file layout this build reads and writes.
pub const FORMAT: u32 = 1;

/// How far one lesson is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LessonProgress {
    /// The lesson's `version` when this was recorded.
    pub version: u32,
    pub done: Vec<String>,
    pub skipped: Vec<String>,
    /// The step that was open.
    pub current: Option<String>,
    /// `YYYY-MM-DD` of the last time every step was behind the person.
    pub completed: Option<String>,
}

/// What the Learn window shows for a lesson.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Standing {
    NotStarted,
    InProgress {
        behind: usize,
        total: usize,
    },
    Completed,
    /// The author changed the lesson since the person did it.
    Updated,
}

/// Every lesson's progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Progress {
    pub format: u32,
    pub lessons: BTreeMap<String, LessonProgress>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            format: FORMAT,
            lessons: BTreeMap::new(),
        }
    }
}

impl Progress {
    /// Reads the file. A missing file is a fresh start. A file that does not
    /// parse, or is from a newer build, is renamed to `<name>.bad` and
    /// replaced by a fresh start: the app never fails over it.
    pub fn load(path: &Path) -> Progress {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Progress::default();
        };
        match serde_json::from_str::<Progress>(&text) {
            Ok(p) if p.format == FORMAT => p,
            _ => {
                let mut bad = path.as_os_str().to_owned();
                bad.push(".bad");
                let _ = std::fs::rename(path, bad);
                Progress::default()
            }
        }
    }

    /// Writes the file by writing a sibling and renaming it over, so a
    /// crash leaves the old file or the new one.
    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let mut temp = path.as_os_str().to_owned();
        temp.push(".tmp");
        let text = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        std::fs::write(&temp, text)?;
        std::fs::rename(&temp, path)
    }

    pub fn of(&self, lesson_id: &str) -> Option<&LessonProgress> {
        self.lessons.get(lesson_id)
    }

    /// Where the person stands in a lesson.
    pub fn standing(&self, lesson: &Lesson) -> Standing {
        let Some(p) = self.lessons.get(&lesson.id) else {
            return Standing::NotStarted;
        };
        if p.version != lesson.version {
            return Standing::Updated;
        }
        if p.completed.is_some() {
            return Standing::Completed;
        }
        let behind = lesson
            .steps
            .iter()
            .filter(|s| p.done.contains(&s.id) || p.skipped.contains(&s.id))
            .count();
        if behind == 0 && p.current.is_none() {
            Standing::NotStarted
        } else {
            Standing::InProgress {
                behind,
                total: lesson.steps.len(),
            }
        }
    }

    /// Stores a run's state. `today` is `YYYY-MM-DD`, used when the run has
    /// just finished.
    pub fn record(&mut self, run: &Run, today: &str) {
        let lesson = run.lesson();
        let previous = self.lessons.get(&lesson.id);
        let completed = if run.phase() == Phase::Finished {
            Some(today.to_owned())
        } else {
            previous
                .filter(|p| p.version == lesson.version)
                .and_then(|p| p.completed.clone())
        };
        self.lessons.insert(
            lesson.id.clone(),
            LessonProgress {
                version: lesson.version,
                done: run.done().to_vec(),
                skipped: run.skipped().to_vec(),
                current: run.step().map(|s| s.id.clone()),
                completed,
            },
        );
    }

    /// A run for a lesson, picking up where the person left off. A lesson
    /// that was completed (or updated) starts again from the first step.
    pub fn run_for(&self, lesson: std::sync::Arc<Lesson>) -> Run {
        match self.lessons.get(&lesson.id) {
            Some(p) if p.completed.is_none() => {
                Run::resume(lesson, &p.done, &p.skipped, p.current.as_deref())
            }
            _ => Run::new(lesson),
        }
    }

    pub fn reset(&mut self, lesson_id: &str) {
        self.lessons.remove(lesson_id);
    }

    pub fn reset_all(&mut self) {
        self.lessons.clear();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    const LESSON: &str = r#"
format = 1
id = "t"
version = 2
title = "T"
summary = "S"
track = "x"
[[step]]
id = "a"
title = "A"
text = "t"
[[step]]
id = "b"
title = "B"
text = "t"
"#;

    fn lesson() -> Arc<Lesson> {
        Arc::new(Lesson::parse(LESSON).unwrap())
    }

    #[test]
    fn standing_follows_the_run() {
        let lesson = lesson();
        let mut progress = Progress::default();
        assert_eq!(progress.standing(&lesson), Standing::NotStarted);
        let mut run = progress.run_for(lesson.clone());
        assert!(run.advance());
        progress.record(&run, "2026-10-15");
        assert_eq!(
            progress.standing(&lesson),
            Standing::InProgress {
                behind: 1,
                total: 2
            }
        );
        let resumed = progress.run_for(lesson.clone());
        assert_eq!(resumed.step().unwrap().id, "b");
        let mut run = resumed;
        assert!(run.advance());
        progress.record(&run, "2026-10-16");
        assert_eq!(progress.standing(&lesson), Standing::Completed);
        assert_eq!(
            progress.of("t").unwrap().completed.as_deref(),
            Some("2026-10-16")
        );
        // Doing it again starts from the top and keeps the completion date.
        let again = progress.run_for(lesson.clone());
        assert_eq!(again.index(), 0);
        let mut again = again;
        again.skip();
        progress.record(&again, "2026-10-17");
        assert_eq!(
            progress.of("t").unwrap().completed.as_deref(),
            Some("2026-10-16")
        );
    }

    #[test]
    fn an_edited_lesson_shows_as_updated_and_keeps_known_steps() {
        let lesson = lesson();
        let mut progress = Progress::default();
        progress.lessons.insert(
            "t".into(),
            LessonProgress {
                version: 1,
                done: vec!["a".into(), "removed".into()],
                ..LessonProgress::default()
            },
        );
        assert_eq!(progress.standing(&lesson), Standing::Updated);
        let run = progress.run_for(lesson);
        assert_eq!(run.done(), ["a"]);
        assert_eq!(run.step().unwrap().id, "b");
    }

    #[test]
    fn save_and_load_round_trip() {
        let dir = std::env::temp_dir().join(format!("novo-learn-{}-rt", std::process::id()));
        let path = dir.join("learn.json");
        let lesson = lesson();
        let mut progress = Progress::default();
        let mut run = progress.run_for(lesson);
        run.skip();
        progress.record(&run, "2026-10-15");
        progress.save(&path).unwrap();
        assert_eq!(Progress::load(&path), progress);
        assert!(!dir.join("learn.json.tmp").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bad_or_newer_file_is_set_aside() {
        let dir = std::env::temp_dir().join(format!("novo-learn-{}-bad", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("learn.json");
        for (i, text) in ["{ not json", "{\"format\": 9, \"lessons\": {}}"]
            .iter()
            .enumerate()
        {
            std::fs::write(&path, text).unwrap();
            assert_eq!(Progress::load(&path), Progress::default(), "case {i}");
            assert!(!path.exists());
            assert!(dir.join("learn.json.bad").exists());
        }
        assert_eq!(Progress::load(&path), Progress::default());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reset_one_or_all() {
        let mut progress = Progress::default();
        progress
            .lessons
            .insert("a".into(), LessonProgress::default());
        progress
            .lessons
            .insert("b".into(), LessonProgress::default());
        progress.reset("a");
        assert!(progress.of("a").is_none());
        progress.reset_all();
        assert!(progress.lessons.is_empty());
    }
}
