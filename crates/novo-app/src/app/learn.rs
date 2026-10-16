//! Learning modules, the application side (docs/design/learning-modules.md).
//!
//! `novo-learn` decides what a lesson is and when a step has ticked; this
//! file feeds it what the person does (events), how things are now (facts)
//! and the screen positions of controls (anchors), and keeps the run, the
//! progress file and the catalog. The card, the pointer and the Learn
//! window are drawn in `learn_ui.rs`.
//!
//! Nothing here edits a lesson file or the person's drawing: a lesson opens
//! its own sample project as an untitled project and only reads it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_core::{AnyId, PageId, PinRef, Project};
use novo_editor::Tool;
use novo_editor::tool::WiringMode;
use novo_learn::{
    Catalog, Entry, Event, Facts, Files, Lang, Phase, Progress, ProjectRef, Registry, Run,
    Severity, UiStrings, lint,
};
use novo_workflow::commands::COMMANDS;

use super::workspace::Workspace;
use super::{Level, NovoApp};

include!(concat!(env!("OUT_DIR"), "/lessons_embedded.rs"));

/// Where a person's own lessons go, under the KoperCAD data folder.
pub(crate) const USER_LESSONS_DIR: &str = "lessons";
/// The progress file, under the KoperCAD config folder.
pub(crate) const PROGRESS_FILE: &str = "learn.json";
/// A folder of lessons to load on top of the built-in ones (authoring).
pub(crate) const LESSONS_ENV: &str = "KOPERCAD_LESSONS";

/// How long a ticked step shows its check mark before the next one opens.
const TICK_SHOWN: Duration = Duration::from_millis(500);

/// Signals the shell raises for things that do not pass through a command.
pub(crate) const SIGNALS: &[&str] = &[
    "wheel_opened",
    "proposal_next",
    "proposal_accept",
    "problem_waived",
    "waiver_restored",
    "cabinet_dropped",
    "cabinet_rotated",
    "cabinet_removed",
    "cabinet_plan_accepted",
    "wire_cut_exported",
];

/// Every control a lesson may point at. A control joins this list when its
/// owner calls `NovoApp::anchor` for it; `docs/design/lesson-anchors.md` is
/// generated from it. A lesson naming an id that is not here shows as
/// "Coming soon" instead of breaking.
pub(crate) const ANCHORS: &[&str] = &[
    "start.learn",
    "menu.project",
    "palette.button",
    "dock.select",
    "dock.place",
    "dock.connect",
    "dock.corners",
    "dock.lines",
    "dock.hint",
    "dock.proposals",
    "palette.field",
    "issue.pill",
    "drawer.terminals",
    "drawer.terminals.tab",
    "drawer.cables",
    "drawer.cables.tab",
    "drawer.cables.accept",
    "drawer.devices",
    "drawer.devices.tab",
    "live.card",
    "live.pole",
    "live.coil",
    "live.output",
    "cabinet.place_all",
    "cabinet.accept",
    "cabinet.rules",
    "cabinet.plate",
    "cabinet.routes",
    "cabinet.cutlist",
    "cabinet.table",
    "cabinet.heat",
    "heat.chips",
    "heat.legend",
    "heat.simulate",
    "cabinet.canvas",
    "workspace.view3d",
    "switcher.schematic",
    "switcher.cabinet",
    "switcher.view3d",
    "switcher.reports",
    "switcher.commissioning",
    "outline.pages",
    "outline.toggle",
    "commissioning.who",
    "commissioning.punch",
    "commissioning.report",
    "commissioning.next_open",
    "commissioning.pass",
    "commissioning.fail",
    "commissioning.resolve",
    "cabinet.unplaced",
    "cabinet.problems",
    "cabinet.inspector",
    "cabinet.door",
    "cabinet.surface",
    "cabinet.door.apply",
    "cabinet.routes_panel",
    "cabinet.wire_rules",
    "cabinet.sizing",
    "cabinet.rails",
    "cabinet.plate_drawing",
    "cabinet.more",
    "issues.list",
    "macro.place",
    "macros.window",
];

/// What the shell keeps about learning.
pub(crate) struct LearnState {
    files: Files,
    pub(super) catalog: Catalog,
    /// Problems found while reading the lesson folders.
    pub(super) notes: Vec<String>,
    pub(super) ui: UiStrings,
    pub(super) lang: Lang,
    progress: Progress,
    progress_path: Option<PathBuf>,
    user_dir: Option<PathBuf>,
    /// The lesson being done, and the id of its entry.
    pub(super) run: Option<Run>,
    /// The card was closed with its cross; the run waits for Continue.
    pub(super) hidden: bool,
    /// The Learn window is open.
    pub(super) window: bool,
    pub(super) filter: String,
    pub(super) show_drafts: bool,
    pub(super) confirm_restart: bool,
    /// Lessons whose lint has errors, so they are not offered yet.
    coming: Option<BTreeSet<String>>,
    /// Controls registered so far this frame, and last frame.
    anchors: BTreeMap<String, egui::Rect>,
    seen: BTreeMap<String, egui::Rect>,
    /// The visible name of a control, for `{ui:...}` in step text.
    pub(super) labels: BTreeMap<String, String>,
    /// The step whose target was last revealed: lesson and step id.
    revealed: Option<(String, String)>,
    ticked_at: Option<Instant>,
    /// The pointer pulses harder until this moment (Show me how).
    pub(super) flash_until: Option<Instant>,
    /// The "Show me how" demonstration that is playing, and the how-to line
    /// it is on.
    pub(super) demo: Option<super::learn_demo::DemoRun>,
    pub(super) demo_caption: Option<usize>,
    /// Where the card was last drawn, where the demonstration's pointer
    /// starts.
    pub(super) card_pos: Option<egui::Pos2>,
    /// A lesson is opening its own project; `document_replaced` must not
    /// end the run.
    opening: bool,
    /// How long a tick shows before the next step opens.
    pub(super) advance_after: Duration,
}

impl LearnState {
    pub(crate) fn new() -> Self {
        let mut state = LearnState {
            files: Files::new(),
            catalog: Catalog::default(),
            notes: Vec::new(),
            ui: UiStrings::default(),
            lang: Lang::En,
            progress: Progress::default(),
            progress_path: None,
            user_dir: None,
            run: None,
            hidden: false,
            window: false,
            filter: String::new(),
            show_drafts: false,
            confirm_restart: false,
            coming: None,
            anchors: BTreeMap::new(),
            seen: BTreeMap::new(),
            labels: BTreeMap::new(),
            revealed: None,
            ticked_at: None,
            flash_until: None,
            demo: None,
            demo_caption: None,
            card_pos: None,
            opening: false,
            advance_after: TICK_SHOWN,
        };
        state.reload();
        state
    }

    /// Reads the built-in lessons and, when there are any, the person's own
    /// and the authoring folder. A folder that cannot be read is a note in
    /// the Learn window, never a failure.
    pub(crate) fn reload(&mut self) {
        let mut files = Files::new();
        for (key, text) in EMBEDDED_LESSONS {
            files.insert((*key).to_owned(), (*text).to_owned());
        }
        let mut notes = Vec::new();
        let env = std::env::var_os(LESSONS_ENV).map(PathBuf::from);
        for dir in [self.user_dir.clone(), env].into_iter().flatten() {
            if dir.is_dir() {
                add_folder(&mut files, &mut notes, &dir);
            }
        }
        self.ui = match files.get("ui.toml").map(|t| UiStrings::parse(t)) {
            Some(Ok(ui)) => ui,
            Some(Err(e)) => {
                notes.push(format!("ui.toml: {e}"));
                UiStrings::default()
            }
            None => UiStrings::default(),
        };
        self.catalog = Catalog::from_files(&files);
        self.files = files;
        self.notes = notes;
        self.coming = None;
    }

    /// Replaces the lessons with these files, for the tests.
    #[cfg(test)]
    pub(crate) fn use_files(&mut self, files: Files) {
        self.catalog = Catalog::from_files(&files);
        self.files = files;
        self.coming = None;
    }

    #[cfg(test)]
    pub(crate) fn use_progress_file(&mut self, path: PathBuf) {
        self.progress = Progress::load(&path);
        self.progress_path = Some(path);
    }

    pub(crate) fn set_paths(&mut self, progress: Option<PathBuf>, user_dir: Option<PathBuf>) {
        self.progress = progress.as_deref().map(Progress::load).unwrap_or_default();
        self.progress_path = progress;
        self.user_dir = user_dir;
        self.reload();
    }

    /// The words of the lesson screens.
    pub(super) fn say(&self, key: &str) -> String {
        self.ui.get(key, self.lang).to_owned()
    }

    pub(super) fn say_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.ui.format(key, self.lang, args)
    }

    pub(super) fn progress(&self) -> &Progress {
        &self.progress
    }

    pub(super) fn reset_progress(&mut self) {
        self.progress.reset_all();
        self.save();
    }

    pub(super) fn is_coming_soon(&mut self, id: &str) -> bool {
        if self.coming.is_none() {
            self.coming = Some(self.lint_errors());
        }
        self.coming.as_ref().is_some_and(|c| c.contains(id))
    }

    /// What this build offers a lesson: its commands, anchors and signals.
    pub(crate) fn lint_registry(&self) -> Registry {
        Registry {
            commands: COMMANDS.iter().map(|c| c.id.to_owned()).collect(),
            anchors: ANCHORS
                .iter()
                .map(|a| (*a).to_owned())
                .chain(
                    super::learn_labels::menu_anchors()
                        .iter()
                        .map(|m| m.id.clone()),
                )
                .collect(),
            signals: SIGNALS.iter().map(|s| (*s).to_owned()).collect(),
        }
    }

    /// What the lint says about the lessons against this build's commands,
    /// anchors and signals.
    pub(crate) fn lint_report(&self) -> novo_learn::Report {
        let registry = self.lint_registry();
        let example = example_project();
        lint(&self.catalog, &self.files, &registry, example.as_ref())
    }

    /// Lessons with a lint error, which are listed as coming soon. An early
    /// access lesson only warns: it starts, and a pointer whose control this
    /// build does not have yet is left out.
    fn lint_errors(&self) -> BTreeSet<String> {
        self.lint_report()
            .findings
            .iter()
            .filter(|f| f.severity == Severity::Error)
            .map(|f| f.lesson.clone())
            .collect()
    }

    fn save(&self) {
        if let Some(path) = &self.progress_path
            && let Err(e) = self.progress.save(path)
        {
            log::warn!("could not save lesson progress: {e}");
        }
    }

    /// Records where the run is, and saves.
    fn record(&mut self) {
        if let Some(run) = &self.run {
            self.progress.record(run, &today());
            self.save();
        }
    }

    pub(super) fn file(&self, path: &str) -> Option<&str> {
        self.files.get(path).map(String::as_str)
    }

    /// The id of the lesson being done.
    pub(super) fn running(&self) -> Option<&str> {
        self.run.as_ref().map(|r| r.lesson().id.as_str())
    }

    /// Whether a card should be on show.
    pub(crate) fn card_on(&self) -> bool {
        self.run.is_some() && !self.hidden
    }
}

/// Adds the lessons in `dir` to `files`. Lessons the program ships cannot be
/// replaced, and tracks, glossary and words stay the program's own.
fn add_folder(files: &mut Files, notes: &mut Vec<String>, dir: &Path) {
    let shipped: BTreeSet<String> = files
        .keys()
        .filter_map(|k| k.split_once('/').map(|(d, _)| d.to_owned()))
        .collect();
    match novo_learn::catalog::read_dir(dir) {
        Ok((found, skipped)) => {
            notes.extend(skipped);
            for (key, text) in found {
                match key.split_once('/') {
                    None if matches!(key.as_str(), "tracks.toml" | "glossary.toml" | "ui.toml") => {
                    }
                    None => notes.push(format!("{key}: ignored, lessons live in folders")),
                    Some((folder, _)) if shipped.contains(folder) => {
                        notes.push(format!(
                            "{key}: ignored, KoperCAD ships a lesson `{folder}`"
                        ));
                    }
                    Some(_) => {
                        files.insert(key, text);
                    }
                }
            }
        }
        Err(e) => notes.push(format!("{}: {e}", dir.display())),
    }
}

/// The built-in motor starter as a project, for the lint.
fn example_project() -> Option<Project> {
    let mut doc = crate::backend::new_document("Example").ok()?;
    super::example::build_example(&mut doc).ok()?;
    Some(doc.into_project())
}

/// Today as `YYYY-MM-DD` (UTC), for the progress file.
fn today() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    civil_date((secs / 86_400) as i64)
}

/// The date `days` after 1970-01-01, proleptic Gregorian.
fn civil_date(days: i64) -> String {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// `%APPDATA%\KoperCAD\learn.json`.
pub(crate) fn progress_path() -> Option<PathBuf> {
    Some(
        dirs::config_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(PROGRESS_FILE),
    )
}

/// `%APPDATA%\KoperCAD\lessons`.
pub(crate) fn user_lessons_dir() -> Option<PathBuf> {
    Some(
        dirs::data_dir()?
            .join(crate::DATA_DIR_NAME)
            .join(USER_LESSONS_DIR),
    )
}

impl NovoApp {
    /// Points learning at the person's folders. Called once by `new`; tests
    /// leave it out, so they never touch the real progress file.
    pub(super) fn learn_init_paths(&mut self) {
        self.learn.set_paths(progress_path(), user_lessons_dir());
    }

    /// Whether a lesson card is on, so the fast-keys card stays away.
    pub(super) fn learn_active(&self) -> bool {
        self.learn.card_on()
    }

    // ------------------------------------------------------------------
    // What the person does

    /// Something the person did, for the running lesson.
    pub(super) fn learn_event(&mut self, event: Event) {
        if let Some(run) = self.learn.run.as_mut() {
            run.event(event);
        }
    }

    /// A command ran, from a key, the palette, the wheel or a menu.
    pub(super) fn learn_command(&mut self, id: &str) {
        self.learn_event(Event::Command(id.to_owned()));
    }

    /// Raises a signal for something that does not pass through a command.
    pub(super) fn learn_signal(&mut self, name: &str) {
        debug_assert!(SIGNALS.contains(&name), "unknown signal `{name}`");
        self.learn_event(Event::Signal(name.to_owned()));
    }

    /// Registers a control's place on screen this frame, so a lesson can
    /// point at it. `label` is the name `{ui:id}` shows in step text.
    pub(super) fn anchor(&mut self, id: &str, rect: egui::Rect, label: &str) {
        debug_assert!(
            ANCHORS.contains(&id)
                || super::learn_labels::menu_anchors()
                    .iter()
                    .any(|m| m.id == id),
            "unregistered anchor `{id}`"
        );
        self.learn.anchors.insert(id.to_owned(), rect);
        if !self.learn.labels.contains_key(id) {
            self.learn.labels.insert(id.to_owned(), label.to_owned());
        }
    }

    /// [`Self::anchor`] for a widget: its rect, plus a click event.
    pub(super) fn anchor_response(&mut self, id: &str, response: &egui::Response, label: &str) {
        self.anchor(id, response.rect, label);
        if response.clicked() {
            self.learn_event(Event::Clicked(id.to_owned()));
        }
    }

    /// The tool in hand and the keys that take a proposal.
    pub(super) fn learn_watch(&mut self, ctx: &egui::Context) {
        if self.learn.run.is_none() {
            return;
        }
        let Some(view) = self.canvas.schematic_ref() else {
            return;
        };
        let proposals = !view.editor.proposals().is_empty() || self.terminal_proposals_have_keys();
        if !proposals {
            return;
        }
        let (next, accept) = ctx.input(|i| {
            (
                !i.modifiers.any() && i.key_pressed(egui::Key::Tab),
                !i.modifiers.any() && i.key_pressed(egui::Key::Enter),
            )
        });
        if next {
            self.learn_signal("proposal_next");
        }
        if accept {
            self.learn_signal("proposal_accept");
        }
    }

    /// How things are now.
    fn learn_facts(&mut self) -> Facts {
        let mut facts = Facts {
            tool: "select".to_owned(),
            wiring_mode: match self.settings.wiring_mode() {
                WiringMode::Corners => "corners",
                WiringMode::Lines => "lines",
            }
            .to_owned(),
            live: self.wb.live.on,
            zoom: 1.0,
            ..Facts::default()
        };
        let Some(doc) = self.backend.document() else {
            return facts;
        };
        let project = doc.project();
        if let Some(page) = self.current_page.and_then(|p| project.page(p)) {
            facts.page = Some(page.name.clone());
            facts.page_title = Some(page.title.clone());
        }
        if let Some(view) = self.canvas.schematic_ref() {
            facts.tool = match view.editor.tool() {
                Tool::Place(_) => "place",
                Tool::Wire(_) => "wire",
                Tool::Cable(_) => "cable",
                Tool::Select | Tool::Move(_) | Tool::Paste(_) => "select",
            }
            .to_owned();
            facts.selection = view
                .selection()
                .into_iter()
                .filter_map(|id| match id {
                    AnyId::Symbol(symbol) => symbol_tag(project, symbol),
                    _ => None,
                })
                .collect();
            facts.zoom = f64::from(view.canvas.viewport.zoom) / self.learn_fit_zoom(view).max(1e-6);
        }
        // In the Cabinet and the 3D view the selection is the one on the
        // plate, so a plate-only device (no symbol on a page) can be picked
        // in a lesson and a stale schematic selection does not mask it.
        let plate: Vec<String> = self
            .cabinet
            .view
            .selection
            .ids()
            .iter()
            .filter_map(|i| novo_canvas::cabinet::box_of(*i))
            .filter_map(|id| project.get::<novo_core::Device>(novo_core::DeviceId::from(id)))
            .map(|d| d.tag.to_string())
            .collect();
        if matches!(self.workspace(), Workspace::Cabinet | Workspace::View3d) {
            facts.selection = plate;
        }
        if facts.live {
            let (coils, pins, closed) = self.wb.live.learn_facts(project);
            facts.live_coils = coils;
            facts.live_pins = pins;
            facts.live_closed = closed;
        }
        facts
    }

    /// The zoom that fits the page, so `zoom_at_least` reads 1.0 at fit.
    fn learn_fit_zoom(&self, view: &novo_editor::view::SchematicView) -> f64 {
        let Some(scene) = view.scene() else {
            return 1.0;
        };
        let bounds = scene.scene().bounds();
        let size = view.canvas.viewport.size;
        if bounds.is_empty() || size.x <= 0.0 || size.y <= 0.0 {
            return 1.0;
        }
        let extent = bounds.size().max(novo_canvas::Vec2::splat(1e-3));
        let avail = (size - novo_canvas::Vec2::splat(48.0)).max(novo_canvas::Vec2::ONE);
        f64::from((avail / extent).min_element())
    }

    // ------------------------------------------------------------------
    // The run

    /// Once per frame, late: looks at the drawing, ticks and advances the
    /// step, and reveals its target.
    pub(super) fn learn_step(&mut self, ctx: &egui::Context) {
        if self.learn.run.is_none() {
            return;
        }
        if !self.is_open() {
            self.learn_end();
            return;
        }
        let facts = self.learn_facts();
        let Some(mut run) = self.learn.run.take() else {
            return;
        };
        let mut changed = false;
        if let Some(doc) = self.backend.document()
            && run.update(doc.project(), &facts).is_some()
        {
            self.learn.ticked_at = Some(Instant::now());
            changed = true;
            ctx.request_repaint_after(self.learn.advance_after);
        }
        match run.phase() {
            Phase::Active => self.learn.ticked_at = None,
            Phase::Ticked => {
                let auto = run.step().is_some_and(|s| s.auto_advances());
                let due = self
                    .learn
                    .ticked_at
                    .is_some_and(|at| at.elapsed() >= self.learn.advance_after);
                if auto && due && run.advance() {
                    self.learn.ticked_at = None;
                    changed = true;
                }
            }
            Phase::Finished => {}
        }
        self.learn.run = Some(run);
        if changed {
            self.learn.record();
        }
        self.learn_reveal();
    }

    /// When a step opens, brings its canvas target into view and switches
    /// to its page (a target says `view = "keep"` to stay put).
    fn learn_reveal(&mut self) {
        let Some(run) = self.learn.run.as_ref() else {
            return;
        };
        let Some(step) = run.step() else {
            return;
        };
        let key = (run.lesson().id.clone(), step.id.clone());
        if self.learn.revealed.as_ref() == Some(&key) {
            return;
        }
        self.learn.revealed = Some(key);
        self.learn.flash_until = Some(Instant::now() + Duration::from_millis(1500));
        let Some(target) = step.point.clone() else {
            return;
        };
        // The door form sits below the plate fields: scroll to what the step
        // points at, so the ring is not drawn around a clipped control.
        if let Some(ui) = &target.ui {
            let ids = ui.ids();
            if ids.contains(&"cabinet.door.apply") {
                self.cabinet.door.reveal = Some("cabinet.door.apply");
            } else if ids.contains(&"cabinet.door") {
                self.cabinet.door.reveal = Some("cabinet.door");
            }
        }
        if target.view == novo_learn::lesson::View::Keep {
            return;
        }
        let Some(spot) = self.learn_canvas_spot(&target) else {
            return;
        };
        if self.current_page != Some(spot.page) {
            self.show_page(spot.page);
        }
        let Some(view) = self.canvas.schematic() else {
            return;
        };
        let bounds = novo_canvas::Aabb::new(
            novo_canvas::vec2(spot.min.0 as f32, spot.min.1 as f32),
            novo_canvas::vec2(spot.max.0 as f32, spot.max.1 as f32),
        );
        let readable = view.canvas.reveal_zoom;
        let margin = view.canvas.reveal_margin;
        view.canvas.viewport.reveal(&bounds, margin, readable);
    }

    /// Where a canvas target is on its page, in millimetres.
    pub(super) fn learn_canvas_spot(&self, target: &novo_learn::Target) -> Option<Spot> {
        let project = self.backend.document()?.project();
        let page_named = |name: &str| -> Option<PageId> {
            project
                .pages()
                .find(|p| p.name == name || p.title == name)
                .map(|p| p.id)
        };
        if let Some(area) = &target.area {
            let page = page_named(&area.page)?;
            return Some(Spot {
                page,
                min: (area.x, area.y),
                max: (area.x + area.w, area.y + area.h),
            });
        }
        let at = target.at.as_ref()?;
        if let Some([a, b]) = &at.wire {
            let from = novo_learn::check::pins_named(project, a)
                .into_iter()
                .next()?;
            let to = novo_learn::check::pins_named(project, b)
                .into_iter()
                .next()?;
            let (page, p) = pin_spot(project, from)?;
            let (_, q) = pin_spot(project, to)?;
            return Some(Spot {
                page,
                min: (p.0.min(q.0), p.1.min(q.1)),
                max: (p.0.max(q.0), p.1.max(q.1)),
            });
        }
        if let Some(name) = &at.feed {
            return feed_spot(project, name, self.current_page);
        }
        if at.device.is_none()
            && let Some(pin) = &at.pin
        {
            let found = novo_learn::check::pins_named(project, pin);
            let (page, p) = pin_spot(project, *found.first()?)?;
            return Some(Spot::point(page, p));
        }
        if let Some(device) = &at.device {
            if let Some(pin) = &at.pin {
                let found = novo_learn::check::pins_named(project, &format!("{device}:{pin}"));
                let (page, p) = pin_spot(project, *found.first()?)?;
                return Some(Spot::point(page, p));
            }
            for symbol in project.symbols() {
                let have = symbol_tag(project, symbol.id)
                    .and_then(|t| novo_core::DeviceTag::parse(&t).ok());
                if have.is_some_and(|tag| novo_learn::check::tag_matches(&tag, device)) {
                    let o = symbol.placement.origin;
                    return Some(Spot::point(symbol.page, (o.x, o.y)));
                }
            }
            return None;
        }
        let page = match &at.page {
            Some(name) => page_named(name)?,
            None => self.current_page?,
        };
        Some(Spot::point(page, (at.x?, at.y?)))
    }

    // ------------------------------------------------------------------
    // Starting, moving and ending

    /// Opens a lesson's sample project and starts the lesson at its first
    /// step.
    pub(super) fn start_lesson(&mut self, id: &str) {
        let Some(entry) = self.learn.catalog.get(id).cloned() else {
            self.message(Level::Error, format!("There is no lesson `{id}`."));
            return;
        };
        self.learn.opening = true;
        let opened = self.open_lesson_project(&entry);
        self.learn.opening = false;
        if let Err(why) = opened {
            self.message(
                Level::Error,
                format!("Could not open lesson {}: {why}", entry.lesson.title),
            );
            return;
        }
        if let Some(name) = &entry.lesson.page
            && let Some(page) = self.backend.document().and_then(|d| {
                d.project()
                    .pages()
                    .find(|p| p.name == *name || p.title == *name)
                    .map(|p| p.id)
            })
        {
            self.show_page(page);
        }
        self.learn.run = Some(Run::new(entry.lesson.clone()));
        self.learn.hidden = false;
        self.learn.window = false;
        self.learn.confirm_restart = false;
        self.learn.revealed = None;
        self.learn.ticked_at = None;
        self.learn.record();
        self.settings.onboarding.dismissed = true;
    }

    /// The card's Continue: the card back on a lesson still under way.
    pub(super) fn continue_lesson(&mut self) {
        self.learn.hidden = false;
        self.learn.window = false;
    }

    fn open_lesson_project(&mut self, entry: &Entry) -> Result<(), String> {
        let lesson = &entry.lesson;
        let title = self.learn.say_with(
            "lesson_project",
            &[("title", lesson.title(self.learn.lang))],
        );
        match &entry.project {
            None => return Err("its sample project is missing".to_owned()),
            Some(ProjectRef::Example) => {
                self.open_example();
                return Ok(());
            }
            Some(ProjectRef::Blank) => self
                .backend
                .new_project(&title)
                .map_err(|e| e.to_string())?,
            Some(ProjectRef::File(path)) => {
                let text = self
                    .learn
                    .file(path)
                    .ok_or_else(|| format!("{path} is missing"))?;
                let mut project = Project::from_json(text).map_err(|e| format!("{path}: {e}"))?;
                project
                    .transact("Lesson", |tx| tx.set_info(|info| info.name = title.clone()))
                    .map_err(|e| e.to_string())?;
                self.backend
                    .new_project_from(project)
                    .map_err(|e| e.to_string())?;
            }
        }
        self.project_path = None;
        self.current_page = None;
        self.document_replaced();
        self.fix_current_page();
        if let Some(doc) = self.backend.document_mut() {
            doc.mark_saved();
        }
        Ok(())
    }

    /// A different project was opened or the project closed: the lesson
    /// ends there (its progress stays), unless the lesson opened it.
    pub(super) fn learn_project_changed(&mut self) {
        if !self.learn.opening {
            self.learn_end();
        }
    }

    fn learn_end(&mut self) {
        if self.learn.run.is_some() {
            self.learn.record();
        }
        self.learn.run = None;
        self.learn.hidden = false;
        self.learn.confirm_restart = false;
        self.learn.revealed = None;
        self.learn.ticked_at = None;
    }

    /// Back, Skip, Next and Restart, from the card or the commands.
    pub(super) fn learn_move(&mut self, how: Move) {
        let Some(run) = self.learn.run.as_mut() else {
            return;
        };
        let moved = match how {
            Move::Next => run.advance(),
            Move::Back => run.back(),
            Move::Skip => run.skip(),
        };
        if moved {
            self.learn.ticked_at = None;
            self.learn.record();
        }
    }

    /// Restart lesson: a fresh copy of the sample project and no progress.
    pub(super) fn learn_restart(&mut self) {
        let Some(id) = self.learn.running().map(str::to_owned) else {
            return;
        };
        self.learn.progress.reset(&id);
        self.start_lesson(&id);
    }

    /// The pointer flashes again.
    pub(super) fn learn_show_me(&mut self) {
        self.learn.flash_until = Some(Instant::now() + Duration::from_millis(2500));
        self.learn.revealed = None;
        self.learn_start_demo();
    }

    /// `help.learn`, with or without a word to look for, and the
    /// `learn.*` commands.
    pub(super) fn learn_run_command(&mut self, id: &str, argument: Option<&str>) {
        match id {
            "help.learn" => match argument.map(str::trim).filter(|a| !a.is_empty()) {
                Some(words) => match self.learn_best_match(words) {
                    Some(lesson) => self.start_lesson(&lesson),
                    None => {
                        self.learn.filter = words.to_owned();
                        self.learn.window = true;
                    }
                },
                None => self.learn.window = !self.learn.window,
            },
            "learn.next" => self.learn_move(Move::Next),
            "learn.back" => self.learn_move(Move::Back),
            "learn.restart" => self.learn.confirm_restart = self.learn.run.is_some(),
            "learn.close" => self.learn.hidden = self.learn.run.is_some(),
            _ => {}
        }
    }

    /// The lesson that best matches some words, among the ones on offer.
    fn learn_best_match(&mut self, words: &str) -> Option<String> {
        let wanted: Vec<String> = words.split_whitespace().map(str::to_lowercase).collect();
        let show_drafts = self.learn.show_drafts;
        let mut best: Option<(usize, String)> = None;
        let ids: Vec<(String, String)> = self
            .learn
            .catalog
            .visible(show_drafts)
            .map(|e| {
                let l = &e.lesson;
                let hay = format!("{} {} {} {}", l.id, l.title, l.summary, l.topics.join(" "))
                    .to_lowercase();
                (l.id.clone(), hay)
            })
            .collect();
        for (id, hay) in ids {
            let score = wanted.iter().filter(|w| hay.contains(w.as_str())).count();
            if score == wanted.len() && best.as_ref().is_none_or(|(s, _)| score > *s) {
                best = Some((score, id));
            }
        }
        best.map(|(_, id)| id)
            .filter(|id| !self.learn.is_coming_soon(id))
    }

    /// The last thing in a frame: this frame's controls become the ones the
    /// next frame knows, and a control that has just appeared is an
    /// `opened` event.
    pub(super) fn learn_end_frame(&mut self) {
        let learn = &mut self.learn;
        if let Some(run) = learn.run.as_mut() {
            for id in learn.anchors.keys() {
                if !learn.seen.contains_key(id) {
                    run.event(Event::Opened(id.clone()));
                }
            }
        }
        learn.seen = std::mem::take(&mut learn.anchors);
    }

    /// The person's key for a command; a command with no key reads as its
    /// name in the command palette, never as a blank.
    pub(super) fn learn_key_text(&self, command: &str) -> String {
        self.wb.keymap.shortcut_text(command).unwrap_or_else(|| {
            let label = COMMANDS
                .iter()
                .find(|c| c.id == command)
                .map_or(command, |c| c.label);
            self.learn.say_with("no_key", &[("label", label)])
        })
    }

    /// Where a control was last drawn.
    pub(super) fn learn_anchor(&self, id: &str) -> Option<egui::Rect> {
        let drawn = |id: &str| {
            self.learn
                .anchors
                .get(id)
                .or_else(|| self.learn.seen.get(id))
                .copied()
        };
        // A control that is folded away or inside a closed menu is pointed
        // at through the one that opens it.
        let mut id = id.to_owned();
        for _ in 0..3 {
            if let Some(rect) = drawn(&id) {
                return Some(rect);
            }
            id = super::learn_labels::anchor_fallback(&id)?;
        }
        None
    }
}

/// How the card moves through the steps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Move {
    Next,
    Back,
    Skip,
}

/// A place on a page, in millimetres: a point has `min == max`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Spot {
    pub page: PageId,
    pub min: (f64, f64),
    pub max: (f64, f64),
}

impl Spot {
    fn point(page: PageId, p: (f64, f64)) -> Spot {
        Spot {
            page,
            min: p,
            max: p,
        }
    }
}

/// The tag of the device a symbol belongs to.
fn symbol_tag(project: &Project, symbol: novo_core::SymbolId) -> Option<String> {
    let designation = project.pin_designation(PinRef::new(symbol, 0))?;
    designation.rsplit_once(':').map(|(tag, _)| tag.to_owned())
}

/// Where the feed of a potential is: the triangle and its stub, on the
/// current page when it has one there.
fn feed_spot(project: &Project, name: &str, current: Option<PageId>) -> Option<Spot> {
    let mut found: Option<&novo_core::SymbolInstance> = None;
    for symbol in project.symbols() {
        let novo_core::SymbolRole::PotentialFeed { potential } = &symbol.role else {
            continue;
        };
        if project.potential(*potential).is_none_or(|p| p.name != name) {
            continue;
        }
        if found.is_none() || Some(symbol.page) == current {
            found = Some(symbol);
            if Some(symbol.page) == current {
                break;
            }
        }
    }
    let symbol = found?;
    let o = symbol.placement.origin;
    Some(Spot {
        page: symbol.page,
        min: (o.x - 3.0, o.y - 1.0),
        max: (o.x + 3.0, o.y + 9.0),
    })
}

/// Where a pin is on its page.
fn pin_spot(project: &Project, pin: PinRef) -> Option<(PageId, (f64, f64))> {
    let symbol = project.symbol(pin.symbol)?;
    let at = symbol.pin_position(pin.pin)?;
    Some((symbol.page, (at.x, at.y)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_are_civil() {
        assert_eq!(civil_date(0), "1970-01-01");
        assert_eq!(civil_date(20_000), "2024-10-04");
        assert_eq!(civil_date(11_016), "2000-02-29");
    }
}
