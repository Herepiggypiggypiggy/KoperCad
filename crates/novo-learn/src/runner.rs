//! The step runner (design §5 and §6): which step is open, whether it has
//! ticked, and what Back, Skip, Next and Restart do.
//!
//! The runner never touches the screen or the clock. The application calls
//! [`Run::event`] and [`Run::update`], draws the card from the getters, and
//! calls [`Run::advance`] when the person presses Next or, for a step that
//! [`auto_advances`](crate::Step::auto_advances), after its check mark has
//! been on show for a moment.

use std::collections::BTreeSet;
use std::sync::Arc;

use novo_core::{Netlist, Project};

use crate::check::{Context, Event, Facts};
use crate::lesson::{Lesson, Step};

/// Where the run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// The current step waits for the person.
    Active,
    /// The current step's check holds; it waits to be moved past.
    Ticked,
    /// Every step is behind the person.
    Finished,
}

/// A lesson being done.
#[derive(Debug, Clone)]
pub struct Run {
    lesson: Arc<Lesson>,
    index: usize,
    phase: Phase,
    done: Vec<String>,
    skipped: Vec<String>,
    events: BTreeSet<Event>,
    netlist: Option<std::sync::Arc<Netlist>>,
}

impl Run {
    /// A fresh run from the first step.
    pub fn new(lesson: Arc<Lesson>) -> Run {
        Run {
            lesson,
            index: 0,
            phase: Phase::Active,
            done: Vec::new(),
            skipped: Vec::new(),
            events: BTreeSet::new(),
            netlist: None,
        }
    }

    /// Picks up where progress left off: `done` and `skipped` are step ids
    /// from the progress file (unknown ids are dropped, so a lesson that
    /// lost a step still resumes), and the run opens `current`, or the first
    /// step that is neither done nor skipped.
    pub fn resume(
        lesson: Arc<Lesson>,
        done: &[String],
        skipped: &[String],
        current: Option<&str>,
    ) -> Run {
        let known = |ids: &[String]| -> Vec<String> {
            lesson
                .steps
                .iter()
                .filter(|s| ids.contains(&s.id))
                .map(|s| s.id.clone())
                .collect()
        };
        let done = known(done);
        let skipped = known(skipped);
        let index = current
            .and_then(|id| lesson.step_index(id))
            .or_else(|| {
                lesson
                    .steps
                    .iter()
                    .position(|s| !done.contains(&s.id) && !skipped.contains(&s.id))
            })
            .unwrap_or(lesson.steps.len());
        let phase = if index >= lesson.steps.len() {
            Phase::Finished
        } else {
            Phase::Active
        };
        Run {
            lesson,
            index,
            phase,
            done,
            skipped,
            events: BTreeSet::new(),
            netlist: None,
        }
    }

    pub fn lesson(&self) -> &Lesson {
        &self.lesson
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    /// Position of the open step (the step count once finished).
    pub fn index(&self) -> usize {
        self.index
    }

    /// The open step, none once finished.
    pub fn step(&self) -> Option<&Step> {
        self.lesson.steps.get(self.index)
    }

    /// Ids of the steps the person has done, in lesson order.
    pub fn done(&self) -> &[String] {
        &self.done
    }

    pub fn skipped(&self) -> &[String] {
        &self.skipped
    }

    /// Whether this step ticked or was skipped, for the card's list.
    pub fn is_done(&self, id: &str) -> bool {
        self.done.iter().any(|d| d == id)
    }

    pub fn is_skipped(&self, id: &str) -> bool {
        self.skipped.iter().any(|d| d == id)
    }

    /// Steps done or skipped out of all steps.
    pub fn progress(&self) -> (usize, usize) {
        let behind = self
            .lesson
            .steps
            .iter()
            .filter(|s| self.is_done(&s.id) || self.is_skipped(&s.id))
            .count();
        (behind, self.lesson.steps.len())
    }

    /// Whether the open step can be moved past with [`Run::advance`]: it has
    /// ticked, or it is a reading step.
    pub fn can_advance(&self) -> bool {
        match (self.phase, self.step()) {
            (Phase::Ticked, _) => true,
            (Phase::Active, Some(step)) => step.expect.is_none(),
            _ => false,
        }
    }

    /// Something the person did. Only the open step hears it, and only
    /// events after it opened count.
    pub fn event(&mut self, event: Event) {
        if self.phase != Phase::Finished {
            self.events.insert(event);
        }
    }

    /// Looks at the project and the facts. Returns the id of a step that
    /// ticked just now. A ticked step whose state check stops holding (the
    /// work was undone before the person moved on) opens again.
    pub fn update(&mut self, project: &Project, facts: &Facts) -> Option<String> {
        let step = self.lesson.steps.get(self.index)?;
        let check = step.expect.as_ref()?;
        if !self.netlist.as_ref().is_some_and(|n| n.is_current(project)) {
            self.netlist = Some(project.netlist());
        }
        let netlist = self.netlist.as_ref()?;
        let holds = check.eval(&Context {
            project,
            netlist,
            facts,
            events: &self.events,
        });
        let id = step.id.clone();
        match (self.phase, holds) {
            (Phase::Active, true) => {
                self.phase = Phase::Ticked;
                if !self.is_done(&id) {
                    self.done.push(id.clone());
                }
                self.skipped.retain(|s| *s != id);
                Some(id)
            }
            (Phase::Ticked, false) if !check.reads_events() => {
                self.phase = Phase::Active;
                self.done.retain(|d| *d != id);
                None
            }
            _ => None,
        }
    }

    /// Moves past the open step if it ticked or only needs reading; a
    /// reading step is recorded as done. Returns whether it moved.
    pub fn advance(&mut self) -> bool {
        if !self.can_advance() {
            return false;
        }
        if let Some(step) = self.step()
            && !self.is_done(&step.id)
        {
            let id = step.id.clone();
            self.done.push(id);
        }
        self.open(self.index + 1);
        true
    }

    /// Moves past the open step without a tick.
    pub fn skip(&mut self) -> bool {
        let Some(step) = self.step() else {
            return false;
        };
        let id = step.id.clone();
        if !self.is_done(&id) && !self.is_skipped(&id) {
            self.skipped.push(id);
        }
        self.open(self.index + 1);
        true
    }

    /// Opens the previous step. The drawing is not undone.
    pub fn back(&mut self) -> bool {
        if self.index == 0 {
            return false;
        }
        self.open(self.index - 1);
        true
    }

    /// Starts again from the first step with nothing done.
    pub fn restart(&mut self) {
        self.done.clear();
        self.skipped.clear();
        self.open(0);
    }

    fn open(&mut self, index: usize) {
        self.index = index.min(self.lesson.steps.len());
        self.phase = if self.index >= self.lesson.steps.len() {
            Phase::Finished
        } else {
            Phase::Active
        };
        self.events.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use novo_core::{Document, Project};

    const LESSON: &str = r#"
format = 1
id = "t"
version = 1
title = "T"
summary = "S"
track = "x"
[[step]]
id = "tool"
title = "Tool"
text = "t"
expect = { tool = "wire" }
[[step]]
id = "key"
title = "Key"
text = "t"
expect = { command = "edit.duplicate" }
[[step]]
id = "wire"
title = "Wire"
text = "t"
expect = { wire = { from = "-F1:2", to = "-K1:1" } }
[[step]]
id = "read"
title = "Read"
text = "t"
"#;

    fn run() -> Run {
        Run::new(Arc::new(Lesson::parse(LESSON).unwrap()))
    }

    fn starter() -> Document {
        let mut doc = Document::new(Project::new("T"));
        novo_tests::motor_starter(&mut doc).unwrap();
        doc
    }

    fn facts(tool: &str) -> Facts {
        Facts {
            tool: tool.into(),
            zoom: 1.0,
            ..Facts::default()
        }
    }

    #[test]
    fn facts_tick_and_the_person_moves_on() {
        let doc = starter();
        let mut run = run();
        assert_eq!(run.update(doc.project(), &facts("select")), None);
        assert_eq!(
            run.update(doc.project(), &facts("wire")),
            Some("tool".into())
        );
        assert_eq!(run.phase(), Phase::Ticked);
        assert_eq!(
            run.update(doc.project(), &facts("wire")),
            None,
            "ticks once"
        );
        assert!(run.advance());
        assert_eq!(run.step().unwrap().id, "key");
        assert_eq!(run.done(), ["tool"]);
    }

    #[test]
    fn events_count_only_after_the_step_opened() {
        let doc = starter();
        let mut run = run();
        run.event(Event::Command("edit.duplicate".into()));
        run.update(doc.project(), &facts("wire"));
        run.advance();
        // The duplicate before the step opened was forgotten when step 1 ticked.
        assert_eq!(run.update(doc.project(), &facts("wire")), None);
        run.event(Event::Command("edit.duplicate".into()));
        assert_eq!(
            run.update(doc.project(), &facts("wire")),
            Some("key".into())
        );
    }

    #[test]
    fn state_checks_read_the_drawing() {
        let doc = starter();
        let mut run = run();
        run.skip();
        run.skip();
        // In the starter -F1:2 feeds -K1:1 already; the lint is what stops a
        // lesson from asking for what the sample project already has.
        assert_eq!(run.step().unwrap().id, "wire");
        assert_eq!(
            run.update(doc.project(), &facts("select")),
            Some("wire".into())
        );
    }

    #[test]
    fn reading_steps_wait_for_next_and_finish_the_run() {
        let doc = starter();
        let mut run = run();
        run.skip();
        run.skip();
        run.skip();
        assert_eq!(run.step().unwrap().id, "read");
        assert!(run.can_advance());
        assert_eq!(run.update(doc.project(), &facts("select")), None);
        assert!(run.advance());
        assert_eq!(run.phase(), Phase::Finished);
        assert_eq!(run.progress(), (4, 4));
        assert!(!run.advance());
        assert!(!run.skip());
    }

    #[test]
    fn next_is_refused_until_the_step_ticked() {
        let mut run = run();
        assert!(!run.can_advance());
        assert!(!run.advance());
        assert_eq!(run.index(), 0);
    }

    #[test]
    fn skip_back_and_restart() {
        let mut run = run();
        assert!(!run.back());
        run.skip();
        assert_eq!(run.skipped(), ["tool"]);
        assert!(run.back());
        assert_eq!(run.index(), 0);
        run.restart();
        assert!(run.skipped().is_empty());
        assert_eq!(run.progress(), (0, 4));
    }

    #[test]
    fn resume_drops_unknown_ids_and_opens_the_first_open_step() {
        let lesson = Arc::new(Lesson::parse(LESSON).unwrap());
        let run = Run::resume(
            lesson.clone(),
            &["tool".into(), "gone".into()],
            &["key".into()],
            None,
        );
        assert_eq!(run.done(), ["tool"]);
        assert_eq!(run.step().unwrap().id, "wire");
        let at = Run::resume(lesson.clone(), &[], &[], Some("read"));
        assert_eq!(at.index(), 3);
        let all = Run::resume(
            lesson,
            &["tool".into(), "key".into(), "wire".into(), "read".into()],
            &[],
            None,
        );
        assert_eq!(all.phase(), Phase::Finished);
    }

    #[test]
    fn undoing_a_ticked_state_step_opens_it_again() {
        let mut doc = starter();
        let mut run = run();
        run.skip();
        run.skip();
        assert_eq!(
            run.update(doc.project(), &facts("select")),
            Some("wire".into())
        );
        // Undo the whole starter: the wire no longer joins the pins.
        assert!(doc.undo().unwrap().is_some());
        assert_eq!(run.update(doc.project(), &facts("select")), None);
        assert_eq!(run.phase(), Phase::Active);
        assert!(!run.is_done("wire"));
    }

    #[test]
    fn event_steps_stay_ticked() {
        let doc = starter();
        let mut run = run();
        run.skip();
        run.event(Event::Command("edit.duplicate".into()));
        assert_eq!(
            run.update(doc.project(), &facts("select")),
            Some("key".into())
        );
        assert_eq!(run.update(doc.project(), &facts("select")), None);
        assert_eq!(run.phase(), Phase::Ticked);
    }
}
