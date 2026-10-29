//! Review notes (novo-revisions `ReviewNotes`): comments on the project, a
//! page, an object or a spot on a page, with an author, an open or resolved
//! status and a thread of replies.
//!
//! Where the person finds them:
//!
//! * Check > Review notes (Ctrl+K "notes") opens the Review notes panel on the
//!   drawing: Open / Resolved / All, who wrote it, this page only, and a search.
//! * Right click on an object or on empty canvas > Add review note... (or
//!   Check > Add review note...) writes a note on what is selected, on that
//!   spot, on the page or on the project.
//! * Open notes show as a speech bubble on their object or spot; the status
//!   bar counts the open ones. Clicking a note in the panel goes to it.
//! * File > Export print > Export PDF with options... can print the open notes
//!   as numbered bubbles on the pages with a list at the end, and the handover
//!   package carries the full list.
//!
//! The notes live in the project file next to the drawing but are not part of
//! the model: not undone, and they leave the revision counter alone. Delete
//! asks a second click.

use eframe::egui;
use novo_canvas::model::item_id;
use novo_canvas::{Aabb, Mark, MarkKind, MarkTarget, Vec2};
use novo_core::{AnyId, PageId, Point, Project, SymbolRole};
use novo_revisions::{
    Located, NoteFilter, NoteId, NoteStatus, NoteTarget, ReviewNotes, locate_note,
    notes_appendix_lines, notes_pdf_markers,
};
use novo_theme::{icons, space, widgets};
use novo_xref::Place;

use super::changes::shown_on;
use super::commissioning::now;
use super::{Level, NovoApp};

/// Which status the list shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum Show {
    #[default]
    Open,
    Resolved,
    All,
}

impl Show {
    fn status(self) -> Option<NoteStatus> {
        match self {
            Show::Open => Some(NoteStatus::Open),
            Show::Resolved => Some(NoteStatus::Resolved),
            Show::All => None,
        }
    }
}

/// The form that writes a new note: what it is about (a choice among what
/// the person had selected, the spot they clicked, the page and the project)
/// and its text.
pub(super) struct Compose {
    options: Vec<(String, NoteTarget)>,
    chosen: usize,
    pub(super) text: String,
    focus: bool,
}

impl Compose {
    pub(super) fn target(&self) -> NoteTarget {
        self.options[self.chosen].1
    }

    /// Picks the last choice, which is always the whole project.
    #[cfg(test)]
    pub(super) fn choose_last(&mut self) {
        self.chosen = self.options.len() - 1;
    }
}

/// Everything the Review notes panel remembers while a project is open.
#[derive(Default)]
pub(super) struct NotesUi {
    /// The project's notes, read from the file on first use.
    store: Option<ReviewNotes>,
    pub(super) open: bool,
    pub(super) show: Show,
    /// Only the notes on the page on screen.
    this_page: bool,
    author: Option<String>,
    search: String,
    pub(super) compose: Option<Compose>,
    /// The note a reply is being written under, and the text so far.
    reply: Option<(NoteId, String)>,
    /// The reply box takes the keyboard when it opens.
    reply_focus: bool,
    confirm_delete: Option<NoteId>,
    /// The note last jumped to: its card is highlighted.
    pub(super) focus: Option<NoteId>,
    /// A spot to bring into view once the page it is on is shown.
    reveal: Option<(PageId, Point)>,
    /// A rejected save or read is told once.
    told: bool,
}

impl NotesUi {
    pub(super) fn store(&self) -> Option<&ReviewNotes> {
        self.store.as_ref()
    }

    /// How many notes are open (0 until the notes are read).
    pub(super) fn open_count(&self) -> usize {
        self.store.as_ref().map_or(0, |s| s.counts().open)
    }

    fn filter(&self, page: Option<PageId>) -> NoteFilter {
        NoteFilter {
            status: self.show.status(),
            author: self.author.clone(),
            page: if self.this_page { page } else { None },
            text: self.search.clone(),
        }
    }
}

/// What a click in the panel asks for, done after the frame is built.
enum Act {
    Jump(NoteId),
    Resolve(NoteId),
    Reopen(NoteId),
    Delete(NoteId),
    StartReply(NoteId),
    SendReply(NoteId, String),
    ResolveShown,
    /// Open the form for a new note (the app knows the selection).
    NewNote,
    /// Send the form.
    Compose,
}

/// The first line of a note, cut for a hover text.
fn headline(text: &str) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.chars().count() > 90 {
        format!("{}…", line.chars().take(89).collect::<String>())
    } else {
        line.to_owned()
    }
}

/// Dates for the cards: `22 Oct`, from an ISO time. Falls back to the text.
fn short_date(at: &str) -> String {
    let date = at.split('T').next().unwrap_or(at);
    let mut parts = date.split('-');
    let (Some(_), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) else {
        return date.to_owned();
    };
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    match (
        d.parse::<u32>(),
        m.parse::<usize>()
            .ok()
            .and_then(|m| MONTHS.get(m.wrapping_sub(1))),
    ) {
        (Ok(d), Some(m)) => format!("{d} {m}"),
        _ => date.to_owned(),
    }
}

/// The object a note on `selected` is about: a device's symbol stands for the
/// device, a terminal's for the terminal, anything else is itself.
fn subject_of(project: &Project, selected: AnyId) -> AnyId {
    if let AnyId::Symbol(s) = selected
        && let Some(symbol) = project.symbol(s)
    {
        return match &symbol.role {
            SymbolRole::Device { device, .. } => AnyId::Device(*device),
            SymbolRole::Terminal { terminals } => {
                terminals.first().map_or(selected, |&t| AnyId::Terminal(t))
            }
            _ => selected,
        };
    }
    selected
}

impl NovoApp {
    /// The name a note is written under.
    pub(super) fn note_author(&self) -> String {
        let initials = self.settings.initials.trim();
        if initials.is_empty() {
            "Me".to_owned()
        } else {
            initials.to_owned()
        }
    }

    /// Reads the notes from the project file the first time they are needed.
    pub(super) fn ensure_notes_loaded(&mut self) {
        if self.wb.notes.store.is_some() || self.backend.document().is_none() {
            return;
        }
        let store = match self.backend.review_notes_json() {
            None => ReviewNotes::default(),
            Some(json) => match ReviewNotes::from_json(&json) {
                Ok(store) => store,
                Err(e) => {
                    if !self.wb.notes.told {
                        self.wb.notes.told = true;
                        self.message(
                            Level::Warning,
                            format!("Review notes not read, starting empty: {e}"),
                        );
                    }
                    ReviewNotes::default()
                }
            },
        };
        self.wb.notes.store = Some(store);
    }

    /// Keeps the notes in the project file at once.
    fn save_notes(&mut self) {
        if let Some(store) = &self.wb.notes.store {
            let json = store.to_json();
            self.backend.set_review_notes_json(&json);
        }
    }

    /// The `notes.panel` command: the Review notes panel on or off.
    pub(super) fn toggle_notes_panel(&mut self) {
        self.ensure_notes_loaded();
        self.wb.notes.open ^= true;
        if self.wb.notes.open && self.workspace() != super::workspace::Workspace::Schematic {
            self.switch_workspace(super::workspace::Workspace::Schematic);
        }
    }

    /// The `notes.add` command: opens the panel with a form for a new note
    /// about what is selected, the spot that was right-clicked, the page or
    /// the project, in that order of preference.
    pub(super) fn begin_note(&mut self) {
        self.ensure_notes_loaded();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let mut options: Vec<(String, NoteTarget)> = Vec::new();
        let mut seen: Vec<AnyId> = Vec::new();
        for selected in self.selection() {
            let subject = subject_of(project, selected);
            if seen.contains(&subject) {
                continue;
            }
            seen.push(subject);
            if let Located::Here(spot) = locate_note(project, &NoteTarget::object(subject)) {
                options.push((spot.what, NoteTarget::object(subject)));
            }
            if seen.len() >= 3 {
                break;
            }
        }
        let page = self.current_page.filter(|&p| project.page(p).is_some());
        if let Some(page) = page {
            let at = self
                .canvas
                .schematic_ref()
                .and_then(|v| v.menu_world)
                .filter(|_| options.is_empty());
            if let Some(at) = at {
                options.push((
                    format!(
                        "This spot (page {})",
                        project.page(page).map_or("", |p| &p.name)
                    ),
                    NoteTarget::point(page, at),
                ));
            }
            options.push((
                format!("Page {}", project.page(page).map_or("", |p| &p.name)),
                NoteTarget::page(page),
            ));
        }
        options.push(("The whole project".to_owned(), NoteTarget::Project));
        self.wb.notes.compose = Some(Compose {
            options,
            chosen: 0,
            text: String::new(),
            focus: true,
        });
        self.wb.notes.open = true;
        // The spot of the right click is used once.
        if let Some(view) = self.canvas.schematic() {
            view.menu_world = None;
        }
        if self.workspace() != super::workspace::Workspace::Schematic {
            self.switch_workspace(super::workspace::Workspace::Schematic);
        }
    }

    /// Adds a note, keeps it and tells which number it got.
    pub(super) fn add_note(&mut self, target: NoteTarget, text: &str) -> Option<NoteId> {
        self.ensure_notes_loaded();
        let author = self.note_author();
        let store = self.wb.notes.store.as_mut()?;
        match store.add(target, &author, &now(), text) {
            Ok(id) => {
                self.save_notes();
                Some(id)
            }
            Err(e) => {
                self.message(Level::Info, e.to_string());
                None
            }
        }
    }

    /// Goes to what note `id` is about: its page, with the object selected
    /// or the spot in view.
    pub(super) fn jump_to_note(&mut self, id: NoteId) {
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let Some(note) = self.wb.notes.store().and_then(|s| s.get(id)) else {
            return;
        };
        let target = note.target;
        self.wb.notes.focus = Some(id);
        let spot = match locate_note(project, &target) {
            Located::Here(spot) => spot,
            Located::Gone => {
                self.message(
                    Level::Info,
                    format!("N{id} was about something that has since been deleted"),
                );
                return;
            }
        };
        let Some(page) = spot.page else {
            self.message(
                Level::Info,
                format!(
                    "N{id} is about {}, which is not on a page",
                    spot.what.to_lowercase()
                ),
            );
            return;
        };
        match (spot.symbol, target) {
            (Some(symbol), _) => self.jump(
                Place {
                    page,
                    symbol: Some(symbol),
                },
                true,
            ),
            (None, NoteTarget::Object { object }) => {
                if let Some(here) = self.here() {
                    self.wb.nav.history.record(here);
                }
                self.show_page(page);
                self.wb.pending_item = Some((page, object));
            }
            (None, NoteTarget::Point { x, y, .. }) => {
                if let Some(here) = self.here() {
                    self.wb.nav.history.record(here);
                }
                self.show_page(page);
                self.wb.notes.reveal = Some((page, Point::new(x, y)));
            }
            _ => self.jump(Place { page, symbol: None }, true),
        }
    }

    /// After the canvas showed its page: brings a pending spot into view.
    pub(super) fn notes_after_canvas_frame(&mut self) {
        if let Some((page, at)) = self.wb.notes.reveal
            && Some(page) == self.current_page
        {
            self.wb.notes.reveal = None;
            if let Some(view) = self.canvas.schematic() {
                let around = 18.0;
                let (x, y) = (at.x as f32, at.y as f32);
                view.canvas.reveal_area(Aabb::new(
                    Vec2::new(x - around, y - around),
                    Vec2::new(x + around, y + around),
                ));
            }
        }
    }

    /// The `notes.next` command: the next open note in page order, after the
    /// one last visited; wraps round.
    pub(super) fn next_note(&mut self) {
        self.ensure_notes_loaded();
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let Some(store) = self.wb.notes.store() else {
            return;
        };
        let open: Vec<NoteId> = store
            .filtered(project, &NoteFilter::open())
            .into_iter()
            .map(|n| n.id)
            .collect();
        if open.is_empty() {
            self.message(Level::Info, "No open review notes");
            return;
        }
        let at = self
            .wb
            .notes
            .focus
            .and_then(|f| open.iter().position(|&id| id == f));
        let next = open[at.map_or(0, |i| (i + 1) % open.len())];
        self.wb.notes.open = true;
        self.jump_to_note(next);
    }

    /// The bubbles for the open notes on `page`: on the object's symbols or
    /// wires, or at the spot. A note on a page or the project has none.
    pub(super) fn notes_marks(&mut self, page: PageId) -> Vec<Mark> {
        self.ensure_notes_loaded();
        let (Some(store), Some(doc)) = (self.wb.notes.store(), self.backend.document()) else {
            return Vec::new();
        };
        let project = doc.project();
        let mut marks = Vec::new();
        for note in store.open_on_page(project, page) {
            let text = format!("{} {}: {}", note.name(), note.author, headline(&note.text));
            match note.target {
                NoteTarget::Object { object } => {
                    for id in shown_on(project, object, page) {
                        marks.push(Mark::on(MarkKind::Note, item_id(id)).with_text(text.clone()));
                    }
                }
                NoteTarget::Point { x, y, .. } => {
                    let (x, y) = (x as f32, y as f32);
                    marks.push(Mark {
                        kind: MarkKind::Note,
                        target: MarkTarget::Area(Aabb::new(
                            Vec2::new(x - 1.5, y - 1.5),
                            Vec2::new(x + 1.5, y + 1.5),
                        )),
                        text,
                    });
                }
                _ => {}
            }
        }
        marks
    }

    /// The numbered bubbles of the open notes for each page of a PDF.
    pub(super) fn notes_pdf_markers(
        &mut self,
    ) -> std::collections::BTreeMap<PageId, Vec<novo_pdf::PdfMarker>> {
        self.ensure_notes_loaded();
        let (Some(store), Some(doc)) = (self.wb.notes.store(), self.backend.document()) else {
            return Default::default();
        };
        notes_pdf_markers(doc.project(), store, &NoteFilter::open())
            .into_iter()
            .map(|(page, markers)| {
                let list = markers
                    .into_iter()
                    .map(|m| novo_pdf::PdfMarker {
                        label: format!("N{}", m.note),
                        at: (m.at.x as f32, m.at.y as f32),
                        resolved: m.resolved,
                    })
                    .collect();
                (page, list)
            })
            .collect()
    }

    /// The list of notes as text pages for a PDF: `open_only` leaves out the
    /// resolved ones. `None` when there is nothing to list.
    pub(super) fn notes_appendix(&mut self, open_only: bool) -> Option<novo_pdf::Appendix> {
        self.ensure_notes_loaded();
        let (store, doc) = (self.wb.notes.store()?, self.backend.document()?);
        let filter = if open_only {
            NoteFilter::open()
        } else {
            NoteFilter::default()
        };
        let lines = notes_appendix_lines(doc.project(), store, &filter);
        (!lines.is_empty()).then(|| novo_pdf::Appendix {
            title: "Review notes".into(),
            lines: lines
                .into_iter()
                .map(|l| novo_pdf::AppendixLine {
                    text: l.text,
                    bold: l.bold,
                })
                .collect(),
        })
    }

    /// The Review notes panel, on the drawing's right edge.
    pub(super) fn notes_panel(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.wb.notes.open || self.backend.document().is_none() {
            return;
        }
        self.ensure_notes_loaded();
        let page = self.current_page;
        let author = self.note_author();
        let mut acts: Vec<Act> = Vec::new();
        let mut close = false;
        {
            let (Some(doc), notes) = (self.backend.document(), &mut self.wb.notes) else {
                return;
            };
            let project = doc.project();
            super::panel::Panel::new(
                "review_notes",
                format!("{} Review notes", icons::CHAT_TEXT),
                canvas,
                super::panel::Side::Right,
                330.0,
            )
            .above_card()
            .dockable()
            .height(460.0)
            .show(ctx, |ui| {
                notes_body(ui, project, notes, page, &author, &mut acts, &mut close);
            });
        }
        if close {
            self.wb.notes.open = false;
        }
        self.apply_note_acts(acts);
    }

    /// Resolves note `id` as the person at this desk.
    pub(super) fn resolve_note(&mut self, id: NoteId) {
        let (author, at) = (self.note_author(), now());
        if let Some(s) = self.wb.notes.store.as_mut()
            && s.resolve(id, &author, &at).is_ok()
        {
            self.save_notes();
        }
    }

    pub(super) fn reopen_note(&mut self, id: NoteId) {
        if let Some(s) = self.wb.notes.store.as_mut()
            && s.reopen(id).is_ok()
        {
            self.save_notes();
        }
    }

    /// Answers note `id`; a reply opens a resolved note again.
    pub(super) fn reply_note(&mut self, id: NoteId, text: &str) -> bool {
        let (author, at) = (self.note_author(), now());
        let Some(s) = self.wb.notes.store.as_mut() else {
            return false;
        };
        match s.reply(id, &author, &at, text) {
            Ok(()) => {
                self.wb.notes.reply = None;
                self.save_notes();
                true
            }
            Err(e) => {
                self.message(Level::Info, e.to_string());
                false
            }
        }
    }

    pub(super) fn delete_note(&mut self, id: NoteId) {
        if let Some(s) = self.wb.notes.store.as_mut()
            && s.remove(id).is_ok()
        {
            self.save_notes();
        }
        self.wb.notes.confirm_delete = None;
        if self.wb.notes.focus == Some(id) {
            self.wb.notes.focus = None;
        }
    }

    /// Resolves every open note the panel's filter shows.
    fn resolve_shown(&mut self) {
        let (author, at) = (self.note_author(), now());
        let filter = self.wb.notes.filter(self.current_page);
        let Some(project) = self.backend.document().map(|d| d.project()) else {
            return;
        };
        let Some(s) = self.wb.notes.store.as_mut() else {
            return;
        };
        let n = s.resolve_all(project, &filter, &author, &at);
        if n > 0 {
            self.save_notes();
            self.message(
                Level::Info,
                format!("Resolved {n} note{}", if n == 1 { "" } else { "s" }),
            );
        }
    }

    /// Sends the form: the note is added and keeps the list on what is open.
    /// An empty text is refused and the form stays, so nothing typed is lost.
    pub(super) fn send_compose(&mut self) {
        let Some(compose) = self.wb.notes.compose.take() else {
            return;
        };
        match self.add_note(compose.target(), &compose.text) {
            Some(id) => {
                if self.wb.notes.show == Show::Resolved {
                    self.wb.notes.show = Show::Open;
                }
                self.wb.notes.focus = Some(id);
            }
            None => self.wb.notes.compose = Some(compose),
        }
    }

    fn apply_note_acts(&mut self, acts: Vec<Act>) {
        for act in acts {
            match act {
                Act::Jump(id) => self.jump_to_note(id),
                Act::Resolve(id) => self.resolve_note(id),
                Act::Reopen(id) => self.reopen_note(id),
                Act::Delete(id) => self.delete_note(id),
                Act::StartReply(id) => {
                    self.wb.notes.reply = Some((id, String::new()));
                    self.wb.notes.reply_focus = true;
                }
                Act::SendReply(id, text) => {
                    self.reply_note(id, &text);
                }
                Act::ResolveShown => self.resolve_shown(),
                Act::NewNote => {
                    notes_reset_inline(&mut self.wb.notes);
                    self.begin_note();
                }
                Act::Compose => self.send_compose(),
            }
        }
    }
}

/// Closing the half-written reply and the delete question before a new form.
fn notes_reset_inline(notes: &mut NotesUi) {
    notes.reply = None;
    notes.confirm_delete = None;
}

/// The panel's content.
fn notes_body(
    ui: &mut egui::Ui,
    project: &Project,
    notes: &mut NotesUi,
    page: Option<PageId>,
    author: &str,
    acts: &mut Vec<Act>,
    close: &mut bool,
) {
    let counts = notes.store.as_ref().map(|s| s.counts()).unwrap_or_default();
    ui.horizontal(|ui| {
        for (show, label, n) in [
            (Show::Open, "Open", counts.open),
            (Show::Resolved, "Resolved", counts.resolved),
            (Show::All, "All", counts.open + counts.resolved),
        ] {
            if widgets::chip(ui, &format!("{label} {n}"), notes.show == show).clicked() {
                notes.show = show;
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button(icons::X).on_hover_text("Close").clicked() {
                *close = true;
            }
            if widgets::primary_button(ui, &format!("{} Note", icons::PLUS))
                .on_hover_text("Write a note on the selection, the page or the project")
                .clicked()
            {
                acts.push(Act::NewNote);
            }
        });
    });
    ui.add_space(space::S);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut notes.search)
                .hint_text(format!("{} Search notes", icons::MAGNIFYING_GLASS))
                .desired_width(150.0),
        );
        if widgets::chip(ui, "This page", notes.this_page)
            .on_hover_text("Only the notes on the page on screen")
            .clicked()
        {
            notes.this_page ^= true;
        }
        let authors = notes
            .store
            .as_ref()
            .map(|s| s.authors())
            .unwrap_or_default();
        if authors.len() > 1 {
            egui::ComboBox::from_id_salt("notes_author")
                .selected_text(notes.author.clone().unwrap_or_else(|| "Anyone".into()))
                .width(70.0)
                .show_ui(ui, |ui| {
                    if ui
                        .selectable_label(notes.author.is_none(), "Anyone")
                        .clicked()
                    {
                        notes.author = None;
                    }
                    for a in authors {
                        if ui
                            .selectable_label(notes.author.as_deref() == Some(a.as_str()), &a)
                            .clicked()
                        {
                            notes.author = Some(a);
                        }
                    }
                });
        }
    });
    ui.separator();

    // The form for a new note.
    if let Some(compose) = &mut notes.compose {
        let mut send = false;
        let mut cancel = false;
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.weak("About");
                for (i, (label, _)) in compose.options.iter().enumerate() {
                    if widgets::chip(ui, label, compose.chosen == i).clicked() {
                        compose.chosen = i;
                    }
                }
            });
            let edit = ui.add(
                egui::TextEdit::multiline(&mut compose.text)
                    .hint_text("What should be looked at or changed?")
                    .desired_rows(3)
                    .desired_width(f32::INFINITY),
            );
            if compose.focus {
                edit.request_focus();
                compose.focus = false;
            }
            if edit.has_focus()
                && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter))
            {
                send = true;
            }
            ui.horizontal(|ui| {
                let ready = !compose.text.trim().is_empty();
                if ui
                    .add_enabled(ready, egui::Button::new(format!("Add as {author}")))
                    .clicked()
                {
                    send = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel = true;
                }
            });
        });
        if send && !compose.text.trim().is_empty() {
            acts.push(Act::Compose);
        }
        if cancel || ui.input(|i| i.key_pressed(egui::Key::Escape)) && !send {
            notes.compose = None;
        }
        ui.add_space(space::S);
    }

    let filter = notes.filter(page);
    let list: Vec<NoteId> = notes
        .store
        .as_ref()
        .map(|s| {
            s.filtered(project, &filter)
                .into_iter()
                .map(|n| n.id)
                .collect()
        })
        .unwrap_or_default();
    if list.is_empty() {
        ui.add_space(space::M);
        let nothing = notes.store.as_ref().is_none_or(ReviewNotes::is_empty);
        ui.weak(if nothing {
            "No notes yet. Right click an object, or press Note, to leave one for the next reader."
        } else {
            "No note matches. Change the filter above."
        });
        return;
    }
    let open_shown = notes
        .store
        .as_ref()
        .map(|s| {
            s.filtered(project, &filter)
                .iter()
                .filter(|n| n.is_open())
                .count()
        })
        .unwrap_or(0);
    if open_shown > 1
        && ui
            .small_button(format!("Resolve these {open_shown} notes"))
            .on_hover_text("Marks every open note in this list as resolved")
            .clicked()
    {
        acts.push(Act::ResolveShown);
    }
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for id in list {
                note_card(ui, project, notes, id, author, acts);
                ui.add_space(space::S);
            }
        });
}

/// One note as a card: who and when, what it is about, the text, the replies
/// and the actions.
fn note_card(
    ui: &mut egui::Ui,
    project: &Project,
    notes: &mut NotesUi,
    id: NoteId,
    author: &str,
    acts: &mut Vec<Act>,
) {
    let p = crate::theme::ui_palette();
    let Some(note) = notes.store.as_ref().and_then(|s| s.get(id)).cloned() else {
        return;
    };
    let located = locate_note(project, &note.target);
    let focused = notes.focus == Some(id);
    let (place, gone) = match &located {
        Located::Here(spot) => match spot.page.and_then(|pg| project.page(pg)) {
            Some(pg) if matches!(note.target, NoteTarget::Object { .. }) => {
                (format!("{} · page {}", spot.what, pg.name), false)
            }
            Some(pg) if matches!(note.target, NoteTarget::Point { .. }) => {
                (format!("A spot on page {}", pg.name), false)
            }
            _ => (spot.what.clone(), false),
        },
        Located::Gone => ("Deleted since".to_owned(), true),
    };
    let fill = if focused { p.accent_wash } else { p.raised };
    egui::Frame::new()
        .fill(fill)
        .stroke(egui::Stroke::new(
            1.0,
            if focused { p.accent } else { p.border_subtle },
        ))
        .corner_radius(4)
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            let head = ui.horizontal(|ui| {
                let (icon, color) = match note.status {
                    NoteStatus::Open => (icons::CHAT_TEXT, p.info),
                    NoteStatus::Resolved => (icons::CHECK_CIRCLE, p.success),
                };
                ui.label(egui::RichText::new(icon).color(color));
                ui.label(egui::RichText::new(note.name()).strong());
                ui.weak(format!("{} · {}", note.author, short_date(&note.at)));
            });
            let place_line = ui.add(
                egui::Label::new(egui::RichText::new(&place).color(if gone {
                    p.text_tertiary
                } else {
                    p.accent_hover
                }))
                .sense(if gone {
                    egui::Sense::hover()
                } else {
                    egui::Sense::click()
                }),
            );
            if !gone
                && place_line
                    .on_hover_text("Go to it on the drawing")
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked()
                || head.response.clicked()
            {
                acts.push(Act::Jump(id));
            }
            ui.add(egui::Label::new(&note.text).wrap());
            for reply in &note.replies {
                ui.horizontal_wrapped(|ui| {
                    ui.weak(format!("↳ {} {}", reply.author, short_date(&reply.at)));
                    ui.label(&reply.text);
                });
            }
            if let (NoteStatus::Resolved, Some(by)) = (note.status, &note.resolved_by) {
                ui.weak(format!(
                    "Resolved by {by} {}",
                    note.resolved_at
                        .as_deref()
                        .map_or(String::new(), short_date)
                ));
            }
            let replying = matches!(&notes.reply, Some((r, _)) if *r == id);
            if replying && let Some((_, text)) = &mut notes.reply {
                let edit = ui.add(
                    egui::TextEdit::multiline(text)
                        .desired_rows(2)
                        .desired_width(f32::INFINITY)
                        .hint_text(format!("Reply as {author}")),
                );
                if std::mem::take(&mut notes.reply_focus) {
                    edit.request_focus();
                }
                let send = edit.has_focus()
                    && ui.input(|i| i.modifiers.command && i.key_pressed(egui::Key::Enter));
                let mut cancel = false;
                ui.horizontal(|ui| {
                    if (ui
                        .add_enabled(!text.trim().is_empty(), egui::Button::new("Reply"))
                        .clicked()
                        || send)
                        && !text.trim().is_empty()
                    {
                        acts.push(Act::SendReply(id, text.clone()));
                    }
                    cancel = ui.button("Cancel").clicked();
                });
                if cancel {
                    notes.reply = None;
                }
                return;
            }
            ui.horizontal(|ui| {
                match note.status {
                    NoteStatus::Open => {
                        if ui
                            .small_button(format!("{} Resolve", icons::CHECK))
                            .clicked()
                        {
                            acts.push(Act::Resolve(id));
                        }
                    }
                    NoteStatus::Resolved => {
                        if ui.small_button("Reopen").clicked() {
                            acts.push(Act::Reopen(id));
                        }
                    }
                }
                if ui.small_button("Reply").clicked() {
                    acts.push(Act::StartReply(id));
                }
                if notes.confirm_delete == Some(id) {
                    if widgets::danger_button(ui, "Delete it").clicked() {
                        acts.push(Act::Delete(id));
                    }
                    if ui.small_button("Keep").clicked() {
                        notes.confirm_delete = None;
                    }
                } else if ui
                    .small_button(icons::TRASH)
                    .on_hover_text("Delete note and its replies (asks to confirm)")
                    .clicked()
                {
                    notes.confirm_delete = Some(id);
                }
            });
        });
}
