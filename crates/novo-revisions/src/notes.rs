//! Review notes: comments on a page or an object, kept with the project.
//!
//! A reviewer (or the designer, or the person at the machine) leaves a note
//! on what they are looking at: the whole project, one page, one object, or a
//! point on a page. A note has an author, a time, a text, an open or resolved
//! status and a thread of replies. The notes are not part of the drawing:
//! they are never printed on it unless asked for, and they do not change the
//! model or its revision counter. The app keeps them as JSON in the project
//! file's sidecar under [`SIDECAR_KEY`].
//!
//! The engine here is pure: [`ReviewNotes`] edits the list, [`locate`] says
//! where a note's target is in a given project (or that it is gone),
//! [`NoteFilter`] narrows the list, and [`pdf_markers`] and [`appendix_lines`]
//! turn the notes into what a PDF or a handover shows. Times are text the
//! caller supplies (ISO 8601), so the engine has no clock.

use std::collections::BTreeMap;

use novo_core::{AnyId, PageId, Point, Project, SymbolId};
use novo_reports::{Cell, Column, ReportOutput, Section};
use serde::{Deserialize, Serialize};

/// The sidecar document the notes are kept under.
pub const SIDECAR_KEY: &str = "review_notes";

/// What a note is about.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "snake_case")]
pub enum NoteTarget {
    /// The project as a whole.
    Project,
    Page {
        page: PageId,
    },
    /// A device, symbol, wire, terminal, cable or text.
    Object {
        object: AnyId,
    },
    /// A spot on a page, in page coordinates (mm).
    Point {
        page: PageId,
        x: f64,
        y: f64,
    },
}

impl NoteTarget {
    pub fn page(page: PageId) -> Self {
        NoteTarget::Page { page }
    }
    pub fn object(object: impl Into<AnyId>) -> Self {
        NoteTarget::Object {
            object: object.into(),
        }
    }
    pub fn point(page: PageId, at: Point) -> Self {
        NoteTarget::Point {
            page,
            x: at.x,
            y: at.y,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoteStatus {
    #[default]
    Open,
    Resolved,
}

impl NoteStatus {
    pub fn label(self) -> &'static str {
        match self {
            NoteStatus::Open => "Open",
            NoteStatus::Resolved => "Resolved",
        }
    }
}

/// One answer in a note's thread.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reply {
    pub author: String,
    pub at: String,
    pub text: String,
}

/// A note's number, stable for the life of the project file. Never reused.
pub type NoteId = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewNote {
    pub id: NoteId,
    pub target: NoteTarget,
    pub author: String,
    pub at: String,
    pub text: String,
    #[serde(default)]
    pub status: NoteStatus,
    /// Who resolved it and when, while it is resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_at: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub replies: Vec<Reply>,
}

impl ReviewNote {
    pub fn is_open(&self) -> bool {
        self.status == NoteStatus::Open
    }

    /// `N7`: how a note is named in lists and on the printed marker.
    pub fn name(&self) -> String {
        format!("N{}", self.id)
    }

    /// The date part of the creation time (`2026-10-22`).
    pub fn date(&self) -> &str {
        self.at.split('T').next().unwrap_or(&self.at)
    }
}

/// Why an edit of the notes was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoteError {
    #[error("Type something first: a note needs a text")]
    Empty,
    #[error("There is no note N{0}")]
    Unknown(NoteId),
    #[error("The notes could not be read: {0}")]
    Unreadable(String),
}

/// Every note of a project.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ReviewNotes {
    #[serde(default)]
    next_id: NoteId,
    #[serde(default)]
    notes: Vec<ReviewNote>,
}

impl ReviewNotes {
    pub fn from_json(json: &str) -> Result<Self, NoteError> {
        serde_json::from_str(json).map_err(|e| NoteError::Unreadable(e.to_string()))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    pub fn is_empty(&self) -> bool {
        self.notes.is_empty()
    }

    pub fn len(&self) -> usize {
        self.notes.len()
    }

    /// Every note, oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &ReviewNote> {
        self.notes.iter()
    }

    pub fn get(&self, id: NoteId) -> Option<&ReviewNote> {
        self.notes.iter().find(|n| n.id == id)
    }

    fn get_mut(&mut self, id: NoteId) -> Result<&mut ReviewNote, NoteError> {
        self.notes
            .iter_mut()
            .find(|n| n.id == id)
            .ok_or(NoteError::Unknown(id))
    }

    pub fn counts(&self) -> Counts {
        let open = self.notes.iter().filter(|n| n.is_open()).count();
        Counts {
            open,
            resolved: self.notes.len() - open,
        }
    }

    /// The authors that appear anywhere in the notes, sorted, each once.
    pub fn authors(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .notes
            .iter()
            .flat_map(|n| std::iter::once(&n.author).chain(n.replies.iter().map(|r| &r.author)))
            .filter(|a| !a.is_empty())
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// Adds a note and returns its number.
    pub fn add(
        &mut self,
        target: NoteTarget,
        author: &str,
        at: &str,
        text: &str,
    ) -> Result<NoteId, NoteError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(NoteError::Empty);
        }
        // Numbers start at 1 and are never reused, even after a delete.
        let highest = self.notes.iter().map(|n| n.id).max().unwrap_or(0);
        let id = self.next_id.max(highest) + 1;
        self.next_id = id;
        self.notes.push(ReviewNote {
            id,
            target,
            author: author.trim().to_owned(),
            at: at.to_owned(),
            text: text.to_owned(),
            status: NoteStatus::Open,
            resolved_by: None,
            resolved_at: None,
            replies: Vec::new(),
        });
        Ok(id)
    }

    /// Answers a note. Replying to a resolved note opens it again: somebody
    /// still has something to say.
    pub fn reply(
        &mut self,
        id: NoteId,
        author: &str,
        at: &str,
        text: &str,
    ) -> Result<(), NoteError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(NoteError::Empty);
        }
        let note = self.get_mut(id)?;
        note.replies.push(Reply {
            author: author.trim().to_owned(),
            at: at.to_owned(),
            text: text.to_owned(),
        });
        note.status = NoteStatus::Open;
        note.resolved_by = None;
        note.resolved_at = None;
        Ok(())
    }

    pub fn edit(&mut self, id: NoteId, text: &str) -> Result<(), NoteError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(NoteError::Empty);
        }
        self.get_mut(id)?.text = text.to_owned();
        Ok(())
    }

    pub fn resolve(&mut self, id: NoteId, by: &str, at: &str) -> Result<(), NoteError> {
        let note = self.get_mut(id)?;
        note.status = NoteStatus::Resolved;
        note.resolved_by = Some(by.trim().to_owned());
        note.resolved_at = Some(at.to_owned());
        Ok(())
    }

    pub fn reopen(&mut self, id: NoteId) -> Result<(), NoteError> {
        let note = self.get_mut(id)?;
        note.status = NoteStatus::Open;
        note.resolved_by = None;
        note.resolved_at = None;
        Ok(())
    }

    /// Resolves every open note `filter` selects. Returns how many.
    pub fn resolve_all(
        &mut self,
        project: &Project,
        filter: &NoteFilter,
        by: &str,
        at: &str,
    ) -> usize {
        let ids: Vec<NoteId> = self
            .notes
            .iter()
            .filter(|n| n.is_open() && filter.matches(project, n))
            .map(|n| n.id)
            .collect();
        for id in &ids {
            let _ = self.resolve(*id, by, at);
        }
        ids.len()
    }

    pub fn remove(&mut self, id: NoteId) -> Result<ReviewNote, NoteError> {
        let at = self
            .notes
            .iter()
            .position(|n| n.id == id)
            .ok_or(NoteError::Unknown(id))?;
        Ok(self.notes.remove(at))
    }

    /// Puts a removed note back (undo of a delete), keeping its number.
    pub fn restore(&mut self, note: ReviewNote) {
        if self.get(note.id).is_none() {
            let at = self.notes.partition_point(|n| n.id < note.id);
            self.notes.insert(at, note);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub open: usize,
    pub resolved: usize,
}

/// Where a note's target is in a project.
#[derive(Debug, Clone, PartialEq)]
pub enum Located {
    /// The target exists. `page` is `None` for a note on the project or on
    /// an object that is not drawn (a part, a cable without wires).
    Here(Spot),
    /// The page or object the note was about has been deleted.
    Gone,
}

/// A place the view can jump to, and a short description of the target.
#[derive(Debug, Clone, PartialEq)]
pub struct Spot {
    pub page: Option<PageId>,
    pub symbol: Option<SymbolId>,
    pub at: Option<Point>,
    /// `Page 3`, `Device -K1`, `Wire 12`, `Project`.
    pub what: String,
}

/// Finds a note's target in `project`.
pub fn locate(project: &Project, target: &NoteTarget) -> Located {
    match *target {
        NoteTarget::Project => Located::Here(Spot {
            page: None,
            symbol: None,
            at: None,
            what: "Project".into(),
        }),
        NoteTarget::Page { page } => match project.page(page) {
            Some(p) => Located::Here(Spot {
                page: Some(page),
                symbol: None,
                at: None,
                what: format!("Page {}", p.name),
            }),
            None => Located::Gone,
        },
        NoteTarget::Point { page, x, y } => match project.page(page) {
            Some(p) => Located::Here(Spot {
                page: Some(page),
                symbol: None,
                at: Some(Point::new(x, y)),
                what: format!("Page {}", p.name),
            }),
            None => Located::Gone,
        },
        NoteTarget::Object { object } => {
            let Some(what) = crate::list::name_of(project, object) else {
                return Located::Gone;
            };
            let (page, symbol, at) = object_spot(project, object);
            Located::Here(Spot {
                page,
                symbol,
                at,
                what,
            })
        }
    }
}

fn symbol_spot(project: &Project, symbol: SymbolId) -> Option<(PageId, SymbolId, Point)> {
    let s = project.symbol(symbol)?;
    Some((s.page, symbol, s.placement.origin))
}

fn object_spot(project: &Project, id: AnyId) -> (Option<PageId>, Option<SymbolId>, Option<Point>) {
    let from_symbol = |s: Option<(PageId, SymbolId, Point)>| match s {
        Some((p, s, at)) => (Some(p), Some(s), Some(at)),
        None => (None, None, None),
    };
    match id {
        AnyId::Symbol(s) => from_symbol(symbol_spot(project, s)),
        AnyId::Device(d) => from_symbol(
            project
                .symbols_of_device(d)
                .first()
                .and_then(|s| symbol_spot(project, s.id)),
        ),
        AnyId::Terminal(t) => from_symbol(
            project
                .symbols_of_terminal(t)
                .first()
                .and_then(|s| symbol_spot(project, s.id)),
        ),
        AnyId::Wire(w) => match project.wire(w) {
            Some(wire) => {
                let at = wire
                    .route
                    .first()
                    .copied()
                    .or_else(|| project.endpoint_position(wire.from));
                (Some(wire.page), None, at)
            }
            None => (None, None, None),
        },
        AnyId::Cable(c) => {
            let wire = project
                .cable(c)
                .and_then(|c| c.cores.iter().find_map(|core| core.wire))
                .and_then(|w| project.wire(w));
            match wire {
                Some(w) => (Some(w.page), None, project.endpoint_position(w.from)),
                None => (None, None, None),
            }
        }
        AnyId::Text(t) => match project.text(t) {
            Some(text) => (Some(text.page), None, None),
            None => (None, None, None),
        },
        AnyId::Junction(j) => match project.record(AnyId::Junction(j)) {
            Some(r) => (r.page(), None, None),
            None => (None, None, None),
        },
        AnyId::Page(p) => (Some(p), None, None),
        _ => (None, None, None),
    }
}

/// Which notes a list shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoteFilter {
    pub status: Option<NoteStatus>,
    pub author: Option<String>,
    /// Only notes whose target is on this page.
    pub page: Option<PageId>,
    /// Words that must all appear in the text, a reply, the author or the
    /// target's name, in any case.
    pub text: String,
}

impl NoteFilter {
    pub fn open() -> Self {
        NoteFilter {
            status: Some(NoteStatus::Open),
            ..Self::default()
        }
    }

    pub fn is_plain(&self) -> bool {
        *self == Self::default()
    }

    pub fn matches(&self, project: &Project, note: &ReviewNote) -> bool {
        if self.status.is_some_and(|s| s != note.status) {
            return false;
        }
        if let Some(author) = &self.author
            && note.author != *author
            && !note.replies.iter().any(|r| r.author == *author)
        {
            return false;
        }
        let located = locate(project, &note.target);
        if let Some(page) = self.page {
            match &located {
                Located::Here(spot) if spot.page == Some(page) => {}
                _ => return false,
            }
        }
        let words = self.text.to_lowercase();
        if words.trim().is_empty() {
            return true;
        }
        let mut haystack = format!("{} {} {}", note.text, note.author, note.name());
        for reply in &note.replies {
            haystack.push(' ');
            haystack.push_str(&reply.text);
            haystack.push(' ');
            haystack.push_str(&reply.author);
        }
        if let Located::Here(spot) = &located {
            haystack.push(' ');
            haystack.push_str(&spot.what);
        }
        let haystack = haystack.to_lowercase();
        words.split_whitespace().all(|w| haystack.contains(w))
    }
}

impl ReviewNotes {
    /// The notes `filter` selects, in page order and then by number; notes
    /// on the project and on deleted targets come last.
    pub fn filtered<'a>(&'a self, project: &Project, filter: &NoteFilter) -> Vec<&'a ReviewNote> {
        let order: BTreeMap<PageId, usize> = project
            .pages_in_order()
            .into_iter()
            .enumerate()
            .map(|(i, p)| (p.id, i))
            .collect();
        let mut out: Vec<&ReviewNote> = self
            .notes
            .iter()
            .filter(|n| filter.matches(project, n))
            .collect();
        out.sort_by_key(|n| {
            let rank = match locate(project, &n.target) {
                Located::Here(Spot { page: Some(p), .. }) => {
                    order.get(&p).copied().unwrap_or(usize::MAX - 1)
                }
                Located::Here(_) => usize::MAX - 1,
                Located::Gone => usize::MAX,
            };
            (rank, n.id)
        });
        out
    }

    /// The open notes that sit on `page`, for the canvas.
    pub fn open_on_page<'a>(&'a self, project: &Project, page: PageId) -> Vec<&'a ReviewNote> {
        self.notes
            .iter()
            .filter(|n| n.is_open())
            .filter(|n| matches!(locate(project, &n.target), Located::Here(Spot { page: Some(p), .. }) if p == page))
            .collect()
    }
}

/// A numbered marker on a printed page.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Marker {
    pub note: NoteId,
    /// Page coordinates, mm.
    pub at: Point,
    pub resolved: bool,
}

/// Where page notes (no point of their own) are drawn: a column down the
/// left margin, 12 mm apart.
const MARGIN_X: f64 = 6.0;
const MARGIN_Y: f64 = 12.0;
const MARGIN_STEP: f64 = 9.0;

/// The markers to print on each page for the notes `filter` selects. A note
/// on a point or an object sits where it points (a little up and to the
/// right of an object, so it does not cover it); a note on the page stands
/// in the left margin.
pub fn pdf_markers(
    project: &Project,
    notes: &ReviewNotes,
    filter: &NoteFilter,
) -> BTreeMap<PageId, Vec<Marker>> {
    let mut out: BTreeMap<PageId, Vec<Marker>> = BTreeMap::new();
    for note in notes.filtered(project, filter) {
        let Located::Here(spot) = locate(project, &note.target) else {
            continue;
        };
        let Some(page) = spot.page else { continue };
        let list = out.entry(page).or_default();
        let at = match (&note.target, spot.at) {
            (NoteTarget::Point { .. }, Some(at)) => at,
            (_, Some(at)) => Point::new(at.x + 6.0, at.y - 6.0),
            (_, None) => {
                let slot = list.iter().filter(|m| m.at.x == MARGIN_X).count();
                Point::new(MARGIN_X, MARGIN_Y + slot as f64 * MARGIN_STEP)
            }
        };
        list.push(Marker {
            note: note.id,
            at,
            resolved: !note.is_open(),
        });
    }
    out
}

/// One line of the printed list of notes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub bold: bool,
}

impl Line {
    fn plain(text: String) -> Self {
        Line { text, bold: false }
    }
    fn bold(text: String) -> Self {
        Line { text, bold: true }
    }
}

/// The notes `filter` selects as lines for a PDF page or a handover list:
/// a heading with the counts, then each note with where it is, who wrote it,
/// its replies and, if resolved, who resolved it. Empty when nothing matches.
pub fn appendix_lines(project: &Project, notes: &ReviewNotes, filter: &NoteFilter) -> Vec<Line> {
    let selected = notes.filtered(project, filter);
    if selected.is_empty() {
        return Vec::new();
    }
    let open = selected.iter().filter(|n| n.is_open()).count();
    let mut lines = vec![
        Line::bold(format!(
            "{} note{}: {open} open, {} resolved",
            selected.len(),
            if selected.len() == 1 { "" } else { "s" },
            selected.len() - open
        )),
        Line::plain(String::new()),
    ];
    for note in selected {
        let place = match locate(project, &note.target) {
            Located::Here(spot) => match (&note.target, spot.page.and_then(|p| project.page(p))) {
                (NoteTarget::Object { .. }, Some(p)) => format!("{}, page {}", spot.what, p.name),
                (NoteTarget::Point { .. }, _) => format!("{}, marked point", spot.what),
                _ => spot.what,
            },
            Located::Gone => "target deleted".to_owned(),
        };
        lines.push(Line::bold(format!(
            "{}  {}  [{}]",
            note.name(),
            place,
            note.status.label()
        )));
        lines.push(Line::plain(format!(
            "{} {}: {}",
            note.author,
            note.date(),
            note.text
        )));
        for reply in &note.replies {
            let date = reply.at.split('T').next().unwrap_or(&reply.at);
            lines.push(Line::plain(format!(
                "  > {} {date}: {}",
                reply.author, reply.text
            )));
        }
        if let (NoteStatus::Resolved, Some(by)) = (note.status, &note.resolved_by) {
            let date = note
                .resolved_at
                .as_deref()
                .and_then(|a| a.split('T').next())
                .unwrap_or("");
            lines.push(Line::plain(format!("  Resolved by {by} {date}")));
        }
        lines.push(Line::plain(String::new()));
    }
    lines
}

/// The id of the notes table as a report.
pub const NOTES_REPORT_ID: &str = "review_notes";

/// The notes `filter` selects as a table, for the handover package: number,
/// status, what and where, who and when, the text and the answers.
pub fn notes_table(project: &Project, notes: &ReviewNotes, filter: &NoteFilter) -> ReportOutput {
    let mut section = Section::new(
        "notes",
        "Review notes",
        vec![
            Column::new("note", "Note", 6.0),
            Column::new("status", "Status", 10.0),
            Column::new("about", "About", 22.0),
            Column::new("from", "From", 14.0),
            Column::new("text", "Note", 38.0),
            Column::new("answers", "Replies and resolution", 36.0),
        ],
    );
    for note in notes.filtered(project, filter) {
        let about = match locate(project, &note.target) {
            Located::Here(spot) => match spot.page.and_then(|p| project.page(p)) {
                Some(page) if !matches!(note.target, NoteTarget::Page { .. }) => {
                    format!("{}, page {}", spot.what, page.name)
                }
                _ => spot.what,
            },
            Located::Gone => "(deleted since)".to_owned(),
        };
        let mut answers: Vec<String> = note
            .replies
            .iter()
            .map(|r| {
                format!(
                    "{} {}: {}",
                    r.author,
                    r.at.split('T').next().unwrap_or(""),
                    r.text
                )
            })
            .collect();
        if let (NoteStatus::Resolved, Some(by)) = (note.status, &note.resolved_by) {
            answers.push(format!(
                "Resolved by {by} {}",
                note.resolved_at
                    .as_deref()
                    .and_then(|a| a.split('T').next())
                    .unwrap_or("")
            ));
        }
        section.push(
            vec![
                Cell::text(note.name()),
                Cell::text(note.status.label()),
                Cell::text(about),
                Cell::text(format!("{} {}", note.author, note.date())),
                Cell::text(&note.text),
                Cell::text(answers.join("\n")),
            ],
            Vec::new(),
        );
    }
    ReportOutput {
        report: NOTES_REPORT_ID.to_owned(),
        title: format!("{}: review notes", project.info().name),
        revision: project.revision(),
        sections: vec![section],
    }
}
