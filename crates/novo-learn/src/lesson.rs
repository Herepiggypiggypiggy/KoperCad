//! The lesson file (`lesson.toml`, design §2): parsing and shape checks.
//!
//! Parsing is strict. Unknown keys are errors so a typo cannot silently
//! disable a check, and a file with a newer `format` is reported as such
//! before its keys are read.

use std::collections::BTreeSet;

use serde::Deserialize;

use crate::check::Check;
use crate::demo::DemoOp;
use crate::markup::{self, MarkupError};

/// The lesson format this build reads.
pub const FORMAT: u32 = 1;
/// Most steps in one lesson (design: 6 to 12 is the aim).
pub const MAX_STEPS: usize = 40;
/// Longest step text in characters (about three lines on the card).
pub const MAX_TEXT_CHARS: usize = 500;
/// Most notes on one step.
pub const MAX_NOTES: usize = 3;
/// Instruction lines on one step's card.
pub const MAX_HOW: usize = 8;

/// The language of the text to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    En,
    Nl,
}

/// Why a lesson file was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LessonError {
    #[error("not valid lesson TOML: {0}")]
    Syntax(String),
    #[error("lesson format {0} needs a newer KoperCAD")]
    NeedsNewer(u32),
    /// A check or script operation this build does not know, from a newer
    /// KoperCAD (or a typo; the lint says which).
    #[error("needs a newer KoperCAD, or has a typo: {0}")]
    UnknownKind(String),
    #[error("{place}: {why}")]
    Invalid { place: String, why: String },
}

fn invalid(place: impl Into<String>, why: impl Into<String>) -> LessonError {
    LessonError::Invalid {
        place: place.into(),
        why: why.into(),
    }
}

/// Whether a lesson is shown to everyone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    #[default]
    Ready,
    /// Shown to everyone with an "Early access" badge: the lesson runs, but
    /// its steps may still be rough or name controls that are not finished.
    /// Lints as a draft until it is promoted to `ready`.
    Early,
    /// Hidden unless drafts are shown; may name commands or anchors this
    /// build does not have yet.
    Draft,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    #[default]
    Beginner,
    Intermediate,
    Advanced,
}

/// What kind of lesson this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// Steps to follow.
    #[default]
    Lesson,
    /// A planted fault and a goal: each step names what must be true, and
    /// its `hints` help in three levels instead of telling the way.
    Challenge,
}

/// One lesson.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub format: u32,
    pub id: String,
    pub version: u32,
    #[serde(default)]
    pub status: Status,
    pub title: String,
    #[serde(default)]
    pub title_nl: Option<String>,
    pub summary: String,
    #[serde(default)]
    pub summary_nl: Option<String>,
    pub track: String,
    #[serde(default = "default_order")]
    pub order: i32,
    #[serde(default)]
    pub level: Level,
    #[serde(default)]
    pub kind: Kind,
    /// What was wrong and why, shown when a challenge ends.
    #[serde(default)]
    pub explain: String,
    #[serde(default)]
    pub explain_nl: Option<String>,
    #[serde(default)]
    pub minutes: u32,
    #[serde(default)]
    pub topics: Vec<String>,
    #[serde(default)]
    pub needs: Vec<String>,
    #[serde(default)]
    pub next: Vec<String>,
    /// `project.json`, a relative path, `@example` or `@blank`.
    #[serde(default = "default_project")]
    pub project: String,
    /// The same drawing at the end of the lesson, for "Open finished
    /// version to compare": a relative path like `project`.
    #[serde(default)]
    pub finished: Option<String>,
    /// Name or title of the page shown on open.
    #[serde(default)]
    pub page: Option<String>,
    /// The lesson is about the design checks: the issue pill, the marks on
    /// symbols and the Issues list stay on. Otherwise they are quiet while
    /// the lesson runs, so the sample project's own warnings are not shown
    /// to a learner as if they were theirs.
    #[serde(default)]
    pub show_problems: bool,
    #[serde(rename = "step")]
    pub steps: Vec<Step>,
}

fn default_order() -> i32 {
    100
}

fn default_project() -> String {
    "project.json".to_owned()
}

/// One step of a lesson.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub title_nl: Option<String>,
    pub text: String,
    #[serde(default)]
    pub text_nl: Option<String>,
    /// The instructions, one line each, numbered on the card: the keys and
    /// clicks and why. Same markup as `text` (`{key:..}`, `{ui:..}`).
    #[serde(default)]
    pub how: Vec<String>,
    /// The Dutch lines; as many as `how`, or none.
    #[serde(default)]
    pub how_nl: Vec<String>,
    #[serde(default)]
    pub point: Option<Target>,
    #[serde(default)]
    pub note: Vec<Note>,
    /// Hints from vague to specific, shown one at a time on request.
    #[serde(default)]
    pub hints: Vec<String>,
    /// The Dutch hints; as many as `hints`, or none.
    #[serde(default)]
    pub hints_nl: Vec<String>,
    /// What ticks the step. None is a reading step with a Next button.
    #[serde(default)]
    pub expect: Option<Check>,
    /// Wait for Next even after the check holds (read, then act).
    #[serde(default)]
    pub pause: bool,
    /// The step done by script: how CI replays the lesson, and what a later
    /// "Do it for me" runs.
    #[serde(default)]
    pub solve: Vec<SolveOp>,
    /// Why the step has no `solve`: a file dialog, a drag no script can
    /// make. The replay lists the step as "not scripted" with this reason
    /// instead of leaving a silent gap.
    #[serde(default)]
    pub manual: Option<String>,
    /// What "Show me how" plays (`demo`): key caps, typing, a moving pointer. A step
    /// that gives none gets one derived from its check, `solve` and `point`.
    #[serde(default)]
    pub demo: Vec<DemoOp>,
}

/// A small callout next to a target. (Unknown keys are refused by
/// [`Target`], which receives every key that is not `text` or `text_nl`.)
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Note {
    #[serde(flatten)]
    pub target: Target,
    pub text: String,
    #[serde(default)]
    pub text_nl: Option<String>,
}

/// Where a pointer or note goes: one of `ui`, `at` or `area`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Target {
    #[serde(default)]
    pub ui: Option<UiPath>,
    #[serde(default)]
    pub at: Option<At>,
    #[serde(default)]
    pub area: Option<Area>,
    #[serde(default)]
    pub view: View,
}

/// One anchor id, or a path of ids from outer to inner (a menu, then an item).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum UiPath {
    One(String),
    Path(Vec<String>),
}

impl UiPath {
    pub fn ids(&self) -> Vec<&str> {
        match self {
            UiPath::One(id) => vec![id.as_str()],
            UiPath::Path(ids) => ids.iter().map(String::as_str).collect(),
        }
    }
}

/// A spot on the drawing, by tag and pin name, never by entity id.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct At {
    #[serde(default)]
    pub device: Option<String>,
    #[serde(default)]
    pub pin: Option<String>,
    #[serde(default)]
    pub page: Option<String>,
    #[serde(default)]
    pub x: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// A wire by its two ends, `tag:pin`.
    #[serde(default)]
    pub wire: Option<[String; 2]>,
    /// A potential feed by the name of its potential (`L1`). Use it for a
    /// feed the person drew themselves, which lands wherever they dropped
    /// it, never a coordinate.
    #[serde(default)]
    pub feed: Option<String>,
}

/// A rectangle on a page, in millimetres.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Area {
    pub page: String,
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Whether a canvas target moves the view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum View {
    /// Move the view only when the target is off screen.
    #[default]
    Reveal,
    Keep,
}

/// One operation of a `solve` script.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum SolveOp {
    Command(CommandOp),
    Place(PlaceOp),
    Wire(WireOp),
    Tee(TeeOp),
    FeedRow(FeedRowOp),
    Select(SelectOp),
    Page(PageOp),
    Cable(CableOp),
    IssueRevision(IssueRevisionOp),
    AssignPart(AssignPartOp),
    Key(KeySolveOp),
    LivePole(LivePoleOp),
    LiveCoil(LiveCoilOp),
    SetPotential(SetPotentialOp),
    Rename(RenameOp),
    UiClick(UiClickOp),
    ListRow(ListRowOp),
    TypeText(TypeTextOp),
    CabinetPlace(CabinetPlaceOp),
    CabinetRotate(CabinetRotateOp),
    CabinetRemove(CabinetRemoveOp),
    Cabinet(CabinetOp),
}

/// A pointer click on the middle of an anchored control, as a person's
/// click: what a step that says "press the Apply button" does.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiClickOp {
    pub ui_click: String,
}

/// A click on one row of a list (`issues.list`), by the row's text (a part
/// of it is enough), then, for a right click, on an entry of the menu that
/// opens.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListRowOp {
    pub list_row: ListRowArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListRowArgs {
    /// The anchor of the list.
    pub list: String,
    /// The row's text, or a part of it.
    pub row: String,
    /// Click with the right button, which opens the row's menu.
    #[serde(default)]
    pub right: bool,
    /// The menu entry to click after a right click.
    #[serde(default)]
    pub choose: Option<String>,
}

/// Text typed into the field that has the keyboard focus.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeTextOp {
    pub type_text: String,
}

/// A drop on the Cabinet plate: the device (by tag) with its top left corner
/// at `x`, `y` millimetres on the plate, as dragging it there does.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetPlaceOp {
    pub cabinet_place: CabinetPlaceArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetPlaceArgs {
    pub device: String,
    pub x: f64,
    pub y: f64,
}

/// A device on the plate turned a quarter (the R key in the Cabinet).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetRotateOp {
    pub cabinet_rotate: String,
}

/// A device taken off the plate (Delete in the Cabinet).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetRemoveOp {
    pub cabinet_remove: String,
}

/// The Place all preview: `place_all` shows it, `accept` takes it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CabinetOp {
    pub cabinet: CabinetStep,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CabinetStep {
    PlaceAll,
    Accept,
}

/// Renames what a label names, as the rename box does: a device tag
/// (`-K1`), a terminal by designation (`-X1:4`), an interruption point
/// (`IP2`) or a feed's potential.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameOp {
    pub rename: RenameArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RenameArgs {
    pub target: String,
    pub to: String,
}

/// A click on a pole badge of the live view (Shift+click: this pole only).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivePoleOp {
    pub live_pole: LivePoleArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LivePoleArgs {
    pub device: String,
    #[serde(default)]
    pub shift: bool,
}

/// A click on a coil badge of the live view; `output` for a PLC output.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCoilOp {
    pub live_coil: LiveCoilArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LiveCoilArgs {
    pub device: String,
    #[serde(default)]
    pub output: bool,
}

/// The right-click chip of a feed: the feed at a spot takes the potential
/// the chip names (`PE`, `L1`, `+24 V`).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPotentialOp {
    pub set_potential: SetPotentialArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetPotentialArgs {
    /// Where the feed's symbol is, in millimetres.
    pub feed_at: [f64; 2],
    /// Its page; the page shown when missing.
    #[serde(default)]
    pub page: Option<String>,
    pub chip: String,
}

/// A key pressed on the keyboard, by name (`Enter`, `Escape`, `Tab`, `Y`):
/// what takes a preview or a dialog.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeySolveOp {
    pub key: String,
    /// Hold the key down this many seconds, then let go (a hold opens the
    /// wheel); a plain press otherwise.
    #[serde(default)]
    pub hold: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandOp {
    pub command: String,
    #[serde(default)]
    pub arg: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceOp {
    pub place: PlaceArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlaceArgs {
    pub symbol: String,
    pub page: String,
    pub x: f64,
    pub y: f64,
    /// Place it as another function of this existing device (a contact
    /// image of `-K1`) instead of a new device.
    #[serde(default)]
    pub parent: Option<String>,
    /// Quarter turns, as the R key gives them.
    #[serde(default)]
    pub turns: u8,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireOp {
    pub wire: [String; 2],
    /// Bend points the person clicks on the way, in millimetres.
    #[serde(default)]
    pub via: Vec<[f64; 2]>,
    /// `corners` or `lines`; the wiring mode the wire is drawn in.
    #[serde(default)]
    pub mode: Option<String>,
}

/// A wire from a pin to a pin that starts with a T on `onto`'s wire at a
/// point, as dragging a wire onto another one does.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeeOp {
    pub tee: TeeArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeeArgs {
    pub from: String,
    pub onto: String,
    pub at: [f64; 2],
}

/// A row of potential feeds, one per name, side by side from a point.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedRowOp {
    pub feed_row: FeedRowArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeedRowArgs {
    pub names: Vec<String>,
    pub page: String,
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SelectOp {
    pub select: String,
    /// Add to the selection instead of replacing it (Shift+click).
    #[serde(default)]
    pub add: bool,
}

/// A cable over the wires that join these pairs of pins.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CableOp {
    pub cable: CableArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CableArgs {
    pub name: String,
    pub wires: Vec<[String; 2]>,
}

/// Issues a revision of the project.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueRevisionOp {
    pub issue_revision: IssueRevisionArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IssueRevisionArgs {
    pub description: String,
}

/// Assigns a catalog part, by maker and order number, to a device.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignPartOp {
    pub assign_part: AssignPartArgs,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssignPartArgs {
    pub device: String,
    pub manufacturer: String,
    pub order_number: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PageOp {
    pub page: String,
}

impl Lesson {
    /// Parses and shape-checks a `lesson.toml`.
    pub fn parse(text: &str) -> Result<Lesson, LessonError> {
        #[derive(Deserialize)]
        struct Head {
            format: u32,
        }
        let head: Head = toml::from_str(text).map_err(|e| LessonError::Syntax(e.to_string()))?;
        if head.format != FORMAT {
            return Err(LessonError::NeedsNewer(head.format));
        }
        let lesson: Lesson = toml::from_str(text).map_err(|e| {
            let message = e.to_string();
            if message.contains("unknown variant") {
                LessonError::UnknownKind(message)
            } else {
                LessonError::Syntax(message)
            }
        })?;
        lesson.validate()?;
        Ok(lesson)
    }

    /// The title in a language, English when there is no twin.
    pub fn title(&self, lang: Lang) -> &str {
        pick(&self.title, self.title_nl.as_deref(), lang)
    }

    pub fn summary(&self, lang: Lang) -> &str {
        pick(&self.summary, self.summary_nl.as_deref(), lang)
    }

    pub fn explain(&self, lang: Lang) -> &str {
        pick(&self.explain, self.explain_nl.as_deref(), lang)
    }

    /// Position of the step with this id.
    pub fn step_index(&self, id: &str) -> Option<usize> {
        self.steps.iter().position(|s| s.id == id)
    }

    fn validate(&self) -> Result<(), LessonError> {
        if !is_id(&self.id) {
            return Err(invalid(
                format!("lesson `{}`", self.id),
                "ids use lowercase letters, digits and hyphens",
            ));
        }
        let at = |step: &str| format!("lesson `{}`, step `{step}`", self.id);
        if self.version == 0 {
            return Err(invalid(
                format!("lesson `{}`", self.id),
                "version starts at 1",
            ));
        }
        if self.steps.is_empty() || self.steps.len() > MAX_STEPS {
            return Err(invalid(
                format!("lesson `{}`", self.id),
                format!("needs 1 to {MAX_STEPS} steps"),
            ));
        }
        // Paths stay inside the lessons folder: `catalog` resolves them
        // against it and refuses a climb out, so only absolute and drive
        // forms need rejecting here.
        let plain_path = |path: &str| {
            !(path.is_empty()
                || path.starts_with('/')
                || path.contains('\\')
                || path.contains(':')
                || path.starts_with('@'))
        };
        if self.project != "@example" && self.project != "@blank" && !plain_path(&self.project) {
            return Err(invalid(
                format!("lesson `{}`", self.id),
                "project is a relative path, @example or @blank",
            ));
        }
        if let Some(finished) = &self.finished
            && !plain_path(finished)
        {
            return Err(invalid(
                format!("lesson `{}`", self.id),
                "finished is a relative path",
            ));
        }
        let mut seen = BTreeSet::new();
        for step in &self.steps {
            if !is_id(&step.id) {
                return Err(invalid(
                    at(&step.id),
                    "ids use lowercase letters, digits and hyphens",
                ));
            }
            if !seen.insert(step.id.as_str()) {
                return Err(invalid(at(&step.id), "this id is used twice"));
            }
            if step.text.chars().count() > MAX_TEXT_CHARS {
                return Err(invalid(
                    at(&step.id),
                    format!("text is longer than {MAX_TEXT_CHARS} characters; split the step"),
                ));
            }
            if step.note.len() > MAX_NOTES {
                return Err(invalid(at(&step.id), format!("at most {MAX_NOTES} notes")));
            }
            let markup_fail = |e: MarkupError| invalid(at(&step.id), e.to_string());
            markup::parse_markup(&step.title).map_err(markup_fail)?;
            markup::parse_markup(&step.text).map_err(markup_fail)?;
            if let Some(nl) = &step.text_nl {
                markup::parse_markup(nl).map_err(markup_fail)?;
            }
            if step.how.len() > MAX_HOW {
                return Err(invalid(
                    at(&step.id),
                    format!("at most {MAX_HOW} `how` lines"),
                ));
            }
            if !step.how_nl.is_empty() && step.how_nl.len() != step.how.len() {
                return Err(invalid(
                    at(&step.id),
                    "`how_nl` needs as many lines as `how`, or none",
                ));
            }
            for line in step.how.iter().chain(&step.how_nl) {
                markup::parse_markup(line).map_err(markup_fail)?;
            }
            if let Some(point) = &step.point {
                point.validate().map_err(|why| invalid(at(&step.id), why))?;
            }
            for note in &step.note {
                note.target
                    .validate()
                    .map_err(|why| invalid(at(&step.id), why))?;
                markup::parse_markup(&note.text).map_err(markup_fail)?;
            }
            if let Some(check) = &step.expect {
                check.validate().map_err(|why| invalid(at(&step.id), why))?;
            }
            for op in &step.demo {
                let targets: Vec<&Target> = match op {
                    DemoOp::Move(m) => vec![&m.to],
                    DemoOp::Drag(d) => vec![&d.drag.from, &d.drag.to],
                    _ => Vec::new(),
                };
                for target in targets {
                    target
                        .validate()
                        .map_err(|why| invalid(at(&step.id), why))?;
                }
            }
            if step.expect.is_none() && !step.solve.is_empty() {
                return Err(invalid(at(&step.id), "a reading step has nothing to solve"));
            }
        }
        Ok(())
    }
}

impl Step {
    pub fn title(&self, lang: Lang) -> &str {
        pick(&self.title, self.title_nl.as_deref(), lang)
    }

    pub fn text(&self, lang: Lang) -> &str {
        pick(&self.text, self.text_nl.as_deref(), lang)
    }

    /// The instruction lines in the language, English when there is no
    /// Dutch twin.
    pub fn how(&self, lang: Lang) -> &[String] {
        match lang {
            Lang::Nl if !self.how_nl.is_empty() => &self.how_nl,
            _ => &self.how,
        }
    }

    /// The hints in the language, English when there is no Dutch twin.
    pub fn hints(&self, lang: Lang) -> &[String] {
        match lang {
            Lang::Nl if !self.hints_nl.is_empty() => &self.hints_nl,
            _ => &self.hints,
        }
    }

    /// Whether the card moves on by itself once the check holds.
    pub fn auto_advances(&self) -> bool {
        self.expect.is_some() && !self.pause
    }
}

impl Note {
    pub fn text(&self, lang: Lang) -> &str {
        pick(&self.text, self.text_nl.as_deref(), lang)
    }
}

impl Target {
    /// Exactly one of `ui`, `at` and `area`, each well formed.
    fn validate(&self) -> Result<(), String> {
        let given = [self.ui.is_some(), self.at.is_some(), self.area.is_some()]
            .iter()
            .filter(|g| **g)
            .count();
        if given != 1 {
            return Err("a target has exactly one of ui, at and area".to_owned());
        }
        if let Some(ui) = &self.ui {
            let ids = ui.ids();
            if ids.is_empty() || ids.iter().any(|id| id.is_empty()) {
                return Err("a ui target needs anchor ids".to_owned());
            }
        }
        if let Some(at) = &self.at {
            let device = at.device.is_some();
            let spot = at.page.is_some() && at.x.is_some() && at.y.is_some();
            let wire = at.wire.is_some();
            let feed = at.feed.is_some();
            // A pin on its own is written whole, `-K1:A1`.
            let pin = at.pin.as_ref().is_some_and(|p| !device && p.contains(':'));
            if [device, spot, wire, feed, pin]
                .iter()
                .filter(|g| **g)
                .count()
                != 1
            {
                return Err(
                    "`at` is a device (with an optional pin), a pin (`-K1:A1`), a feed (by potential name), a page spot (page, x, y) or a wire"
                        .to_owned(),
                );
            }
            if at.pin.is_some() && !device && !pin {
                return Err("`pin` goes with `device`, or is a whole `-K1:A1`".to_owned());
            }
        }
        Ok(())
    }
}

fn pick<'a>(en: &'a str, nl: Option<&'a str>, lang: Lang) -> &'a str {
    match lang {
        Lang::Nl => nl.unwrap_or(en),
        Lang::En => en,
    }
}

/// Lowercase letters, digits and hyphens, starting with a letter.
pub fn is_id(id: &str) -> bool {
    id.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.ends_with('-')
        && !id.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabinet_solve_ops_parse() {
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\n\
             expect = {{ signal = \"cabinet_dropped\" }}\n\
             solve = [ {{ cabinet_place = {{ device = \"-F1\", x = 20, y = 20 }} }}, \
             {{ cabinet_rotate = \"-F1\" }}, {{ cabinet_remove = \"-F1\" }}, \
             {{ cabinet = \"place_all\" }}, {{ cabinet = \"accept\" }} ]\n"
        );
        let lesson = Lesson::parse(&text).unwrap();
        let solve = &lesson.steps.last().unwrap().solve;
        assert!(matches!(&solve[0], SolveOp::CabinetPlace(p) if p.cabinet_place.x == 20.0));
        assert!(matches!(&solve[1], SolveOp::CabinetRotate(_)));
        assert!(matches!(&solve[2], SolveOp::CabinetRemove(_)));
        assert!(matches!(&solve[3], SolveOp::Cabinet(c) if c.cabinet == CabinetStep::PlaceAll));
        assert!(matches!(&solve[4], SolveOp::Cabinet(c) if c.cabinet == CabinetStep::Accept));
        let bad = text.replace("place_all", "everything");
        assert!(Lesson::parse(&bad).is_err());
    }

    #[test]
    fn click_and_typing_solve_ops_parse() {
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\n\
             expect = {{ signal = \"problem_waived\" }}\n\
             solve = [ {{ ui_click = \"issues.waive\" }}, \
             {{ list_row = {{ list = \"issues.list\", row = \"-S1\", right = true, choose = \"Waive\" }} }}, \
             {{ type_text = \"Quote still open\" }} ]\n"
        );
        let lesson = Lesson::parse(&text).unwrap();
        let solve = &lesson.steps.last().unwrap().solve;
        assert!(matches!(&solve[0], SolveOp::UiClick(u) if u.ui_click == "issues.waive"));
        assert!(
            matches!(&solve[1], SolveOp::ListRow(l) if l.list_row.right && l.list_row.choose.is_some())
        );
        assert!(matches!(&solve[2], SolveOp::TypeText(t) if t.type_text.starts_with("Quote")));
        let bad = text.replace("right = true", "left = true");
        assert!(Lesson::parse(&bad).is_err());
    }

    #[test]
    fn how_lines_come_in_both_languages_with_markup() {
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\n\
             how = [\"Press {{key:tool.place}}\", \"Type a name\"]\n\
             how_nl = [\"Druk op {{key:tool.place}}\", \"Typ een naam\"]\n"
        );
        let lesson = Lesson::parse(&text).unwrap();
        let step = lesson.steps.last().unwrap();
        assert_eq!(step.how(Lang::En).len(), 2);
        assert_eq!(step.how(Lang::Nl)[1], "Typ een naam");
        // Without a Dutch twin the English lines show.
        let text = text.replace("how_nl", "x_nl");
        assert!(Lesson::parse(&text).is_err(), "unknown field is refused");
    }

    #[test]
    fn how_nl_must_match_how() {
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\nhow = [\"a\", \"b\"]\nhow_nl = [\"a\"]\n"
        );
        let err = Lesson::parse(&text).unwrap_err().to_string();
        assert!(err.contains("how_nl"), "{err}");
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\nhow = [\"{{key:\"]\n"
        );
        assert!(Lesson::parse(&text).is_err(), "bad markup in how");
    }

    #[test]
    fn newer_solve_ops_parse() {
        let text = format!(
            "{GOOD}\n[[step]]\nid = \"x\"\ntitle = \"X\"\ntext = \"x\"\nexpect = {{ tool = \"select\" }}\nsolve = [\n\
             {{ place = {{ symbol = \"s\", page = \"1\", x = 1, y = 2, turns = 2 }} }},\n\
             {{ select = \"-X1:7\", add = true }},\n\
             {{ cable = {{ name = \"-W1\", wires = [[\"-X1:4\", \"-B1:+\"]] }} }},\n\
             {{ issue_revision = {{ description = \"First issue\" }} }},\n\
             {{ assign_part = {{ device = \"-P1\", manufacturer = \"M\", order_number = \"1\" }} }},\n]\n"
        );
        let lesson = Lesson::parse(&text).unwrap();
        let ops = &lesson.steps.last().unwrap().solve;
        assert_eq!(ops.len(), 5);
        assert!(matches!(&ops[0], SolveOp::Place(p) if p.place.turns == 2));
        assert!(matches!(&ops[1], SolveOp::Select(s) if s.add));
    }

    const GOOD: &str = r#"
format = 1
id = "wiring-modes"
version = 1
title = "Corners and Lines"
title_nl = "Hoeken en lijnen"
summary = "Draw the same wire two ways."
track = "drawing"
order = 20
level = "beginner"
minutes = 6
topics = ["wiring"]
needs = ["fast-keys"]
project = "project.json"
page = "Main circuit"

[[step]]
id = "pick-tool"
title = "Pick the wire tool"
text = "Press {key:tool.connect}, or click **Connect**."
point = { ui = "dock.connect" }
expect = { tool = "wire" }

[[step]]
id = "first-wire"
title = "Wire it"
text = "Click the {tip:pin} **-F1:2**, then **-K1:1**."
point = { at = { device = "-F1", pin = "2" } }
note = [ { at = { device = "-K1", pin = "1" }, text = "Finish here" } ]
expect = { wire = { from = "-F1:2", to = "-K1:1" } }
solve = [ { wire = ["-F1:2", "-K1:1"] }, { command = "edit.escape" } ]

[[step]]
id = "done"
title = "Done"
text = "That is all of it."
"#;

    #[test]
    fn a_good_lesson_parses() {
        let lesson = Lesson::parse(GOOD).unwrap();
        assert_eq!(lesson.steps.len(), 3);
        assert_eq!(lesson.title(Lang::En), "Corners and Lines");
        assert_eq!(lesson.title(Lang::Nl), "Hoeken en lijnen");
        assert_eq!(lesson.summary(Lang::Nl), "Draw the same wire two ways.");
        assert!(lesson.steps[0].auto_advances());
        assert!(!lesson.steps[2].auto_advances());
        assert_eq!(lesson.steps[1].solve.len(), 2);
        assert_eq!(lesson.step_index("done"), Some(2));
    }

    #[test]
    fn unknown_keys_are_errors() {
        let bad = GOOD.replace("minutes = 6", "minutes = 6\nminuts = 3");
        assert!(matches!(Lesson::parse(&bad), Err(LessonError::Syntax(_))));
        let bad = GOOD.replace(
            "expect = { tool = \"wire\" }",
            "expect = { tooll = \"wire\" }",
        );
        assert!(matches!(
            Lesson::parse(&bad),
            Err(LessonError::UnknownKind(_))
        ));
    }

    #[test]
    fn solve_operations_parse() {
        let text = GOOD.replace(
            "solve = [ { wire = [\"-F1:2\", \"-K1:1\"] }, { command = \"edit.escape\" } ]",
            "solve = [ { wire = [\"-F1:2\", \"-K1:1\"], via = [[40, 8], [40, 16]], mode = \"lines\" }, { tee = { from = \"-K1:13\", onto = \"-S2:13\", at = [80, 96] } }, { feed_row = { names = [\"L1\", \"L2\"], page = \"1\", x = 80, y = 20 } }, { place = { symbol = \"iec/contacts/no\", parent = \"-K1\", page = \"1\", x = 1, y = 2 } }, { select = \"-K1\" }, { page = \"1\" } ]",
        );
        let lesson = Lesson::parse(&text).unwrap();
        let ops = &lesson.steps[1].solve;
        assert!(
            matches!(&ops[0], SolveOp::Wire(w) if w.via.len() == 2 && w.mode.as_deref() == Some("lines"))
        );
        assert!(matches!(&ops[1], SolveOp::Tee(_)));
        assert!(matches!(&ops[2], SolveOp::FeedRow(f) if f.feed_row.names.len() == 2));
        assert!(matches!(&ops[3], SolveOp::Place(p) if p.place.parent.as_deref() == Some("-K1")));
        assert!(matches!(&ops[4], SolveOp::Select(_)));
        assert!(matches!(&ops[5], SolveOp::Page(_)));
    }

    #[test]
    fn an_unknown_check_kind_is_listed_as_needing_a_newer_build() {
        let bad = GOOD.replace(
            "expect = { tool = \"wire\" }",
            "expect = { hologram = \"on\" }",
        );
        assert!(matches!(
            Lesson::parse(&bad),
            Err(LessonError::UnknownKind(_))
        ));
    }

    #[test]
    fn a_newer_format_is_reported_before_its_keys() {
        let newer = "format = 2\nsomething_new = true\n";
        assert_eq!(Lesson::parse(newer), Err(LessonError::NeedsNewer(2)));
    }

    #[test]
    fn shape_rules() {
        let cases = [
            (GOOD.replace("id = \"wiring-modes\"", "id = \"Wiring Modes\""), "ids use"),
            (GOOD.replace("id = \"done\"", "id = \"pick-tool\""), "used twice"),
            (GOOD.replace("version = 1", "version = 0"), "version"),
            (
                GOOD.replace("Press {key:tool.connect}", "Press {key:tool.connect"),
                "not closed",
            ),
            (
                GOOD.replace("point = { ui = \"dock.connect\" }", "point = { ui = \"a\", area = { page = \"1\", x = 0.0, y = 0.0, w = 1.0, h = 1.0 } }"),
                "exactly one",
            ),
            (
                GOOD.replace("point = { at = { device = \"-F1\", pin = \"2\" } }", "point = { at = { pin = \"2\" } }"),
                "device",
            ),
            (
                GOOD.replace("project = \"project.json\"", "project = \"/etc/passwd\""),
                "project",
            ),
        ];
        for (text, needle) in cases {
            match Lesson::parse(&text) {
                Err(LessonError::Invalid { why, .. }) => {
                    assert!(why.contains(needle), "{why} should mention {needle}");
                }
                other => panic!("expected `{needle}`, got {other:?}"),
            }
        }
    }

    #[test]
    fn a_reading_step_has_nothing_to_solve() {
        let bad = GOOD.replace(
            "text = \"That is all of it.\"",
            "text = \"That is all of it.\"\nsolve = [ { page = \"1\" } ]",
        );
        assert!(matches!(
            Lesson::parse(&bad),
            Err(LessonError::Invalid { .. })
        ));
    }

    #[test]
    fn ui_targets_take_one_id_or_a_path() {
        let one = "ui = \"dock.connect\"";
        let path = "ui = [\"mark.menu\", \"mark.menu.save\"]";
        for target in [one, path] {
            let text = GOOD.replace("ui = \"dock.connect\"", target);
            Lesson::parse(&text).unwrap();
        }
    }

    #[test]
    fn id_rules() {
        for ok in ["a", "fast-keys", "k1-2"] {
            assert!(is_id(ok), "{ok}");
        }
        for bad in ["", "1a", "A", "a--b", "a-", "a b", "a_b"] {
            assert!(!is_id(bad), "{bad}");
        }
    }
}
