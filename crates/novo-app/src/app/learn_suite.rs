//! The Learning suite: the Learn window as a place to find your way, not a
//! list. A side rail names Home, the learning paths and the topics; Home
//! offers the lesson to do next and the paths as cards; a path or a topic
//! lists its lessons as cards with level, time, progress and what each builds
//! on. Search and the level chips narrow any of them. Prerequisites are shown
//! and never block a lesson.
//!
//! Every word comes from `assets/lessons/ui.toml`; colours from
//! `crate::theme::ui_palette()`. The window selects what it shows through
//! `LearnState::only_path` and `only_track` (neither: Home).

use eframe::egui;
use novo_learn::lesson::Level;
use novo_learn::{Entry, LearningPath, Standing, Track, TrackSummary};
use novo_theme::widgets::section_label;

use super::NovoApp;
use crate::theme::ui_palette;

/// The side rail's width, and the window width below which it folds into a
/// row of chips above the content.
const RAIL_W: f32 = 200.0;
const WIDE: f32 = 720.0;
/// Below this width the cards stack in one column instead of two.
const TWO_COLUMNS: f32 = 560.0;
const LEVELS: [Level; 3] = [Level::Beginner, Level::Intermediate, Level::Advanced];

/// What the suite shows in its main area.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Section {
    Home,
    Path(String),
    Topic(String),
}

type Start = Option<(String, bool)>;

fn level_rank(level: Level) -> usize {
    LEVELS.iter().position(|l| *l == level).unwrap_or(0)
}

fn level_key(level: Level) -> &'static str {
    match level {
        Level::Beginner => "level_beginner",
        Level::Intermediate => "level_intermediate",
        Level::Advanced => "level_advanced",
    }
}

impl NovoApp {
    pub(super) fn learn_window(&mut self, ctx: &egui::Context) {
        if !self.learn.window {
            return;
        }
        let title = self.learn.say("learn");
        let mut open = true;
        let mut start: Start = None;
        let mut reload = false;
        let mut reset = false;
        let first_frame = !self.learn.window_drawn;
        self.learn.window_drawn = true;
        super::panel::dialog(ctx, title, [980.0, 640.0])
            .id(super::panel::keep_id(ctx, egui::Id::new("learn_window")))
            .open(super::panel::esc(ctx, &mut open))
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .show(ctx, |ui| {
                self.suite_body(ui, first_frame, &mut start, &mut reload, &mut reset);
            });
        if !open {
            self.learn.window = false;
            self.learn.window_drawn = false;
            self.learn.only_track = None;
            self.learn.only_path = None;
            self.learn.level = None;
        }
        if reload {
            self.learn.reload();
        }
        if reset {
            self.learn.reset_progress();
        }
        if let Some((id, resume)) = start {
            if resume {
                self.continue_lesson();
            } else {
                self.start_lesson(&id);
            }
        }
    }

    pub(super) fn suite_section(&self) -> Section {
        match (&self.learn.only_path, &self.learn.only_track) {
            (Some(p), _) => Section::Path(p.clone()),
            (None, Some(t)) => Section::Topic(t.clone()),
            (None, None) => Section::Home,
        }
    }

    fn go(&mut self, section: Section) {
        self.learn.only_path = None;
        self.learn.only_track = None;
        match section {
            Section::Home => {}
            Section::Path(id) => self.learn.only_path = Some(id),
            Section::Topic(id) => self.learn.only_track = Some(id),
        }
    }

    fn suite_body(
        &mut self,
        ui: &mut egui::Ui,
        first_frame: bool,
        start: &mut Start,
        reload: &mut bool,
        reset: &mut bool,
    ) {
        let enter = self.suite_toolbar(ui, first_frame);
        ui.add_space(8.0);
        let footer = 48.0;
        let height = (ui.available_height() - footer).max(140.0);
        let wide = ui.available_width() >= WIDE;
        if wide {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(RAIL_W, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        egui::ScrollArea::vertical()
                            .id_salt("suite_rail")
                            .max_height(height)
                            .auto_shrink([false, false])
                            .show(ui, |ui| self.suite_rail(ui));
                    },
                );
                ui.add_space(4.0);
                let width = ui.available_width();
                ui.allocate_ui_with_layout(
                    egui::vec2(width, height),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| self.suite_main(ui, height, enter, start),
                );
            });
        } else {
            self.suite_chips(ui);
            ui.add_space(4.0);
            let h = (height - 30.0).max(120.0);
            self.suite_main(ui, h, enter, start);
        }
        ui.separator();
        self.suite_footer(ui, reload, reset);
    }

    /// Most shown lessons are early access, so a tag on each says nothing:
    /// the home says it once and only the exceptions are marked.
    fn early_is_the_norm(&self) -> bool {
        let shown = self.learn.catalog.visible(self.learn.show_drafts);
        let (all, early) = shown.fold((0, 0), |(all, early), e| {
            (
                all + 1,
                early + usize::from(e.lesson.status == novo_learn::Status::Early),
            )
        });
        early * 2 > all
    }

    /// Search and the level chips. Returns whether Enter was pressed in the
    /// search field, which starts the first result.
    fn suite_toolbar(&mut self, ui: &mut egui::Ui, first_frame: bool) -> bool {
        let mut enter = false;
        ui.horizontal_wrapped(|ui| {
            let hint = self.learn.say("search");
            let field = ui.add(
                egui::TextEdit::singleline(&mut self.learn.filter)
                    .id(egui::Id::new("learn_search"))
                    .hint_text(hint)
                    .desired_width(260.0),
            );
            if first_frame {
                field.request_focus();
            }
            enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            ui.add_space(8.0);
            let all = self.learn.say("level_all");
            if ui
                .selectable_label(self.learn.level.is_none(), all)
                .clicked()
            {
                self.learn.level = None;
            }
            let drafts = self.learn.show_drafts;
            for level in LEVELS {
                let on = self.learn.level == Some(level);
                let n = self
                    .learn
                    .catalog
                    .visible(drafts)
                    .filter(|e| e.lesson.level == level)
                    .count();
                let label = format!("{} · {n}", self.learn.say(level_key(level)));
                // A level with no lessons cannot be picked (a picked one can still be left).
                let chip = ui.add_enabled(n > 0 || on, egui::Button::selectable(on, label));
                if chip.clicked() {
                    self.learn.level = if on { None } else { Some(level) };
                }
            }
        });
        enter
    }

    fn suite_footer(&mut self, ui: &mut egui::Ui, reload: &mut bool, reset: &mut bool) {
        ui.horizontal(|ui| {
            if ui.small_button(self.learn.say("reload")).clicked() {
                *reload = true;
            }
            if self.learn.confirm_reset {
                ui.label(self.learn.say("reset_confirm"));
                if ui.small_button(self.learn.say("yes_reset")).clicked() {
                    *reset = true;
                }
                if ui.small_button(self.learn.say("cancel")).clicked() {
                    self.learn.confirm_reset = false;
                }
            } else if ui.small_button(self.learn.say("reset_progress")).clicked() {
                self.learn.confirm_reset = true;
            }
            let drafts_label = self.learn.say("show_drafts");
            ui.checkbox(&mut self.learn.show_drafts, drafts_label);
        });
    }

    // ---- the rail ----------------------------------------------------

    fn suite_rail(&mut self, ui: &mut egui::Ui) {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        let current = self.suite_section();
        let width = ui.available_width();
        let home = self.learn.say("suite_home");
        if rail_item(ui, width, current == Section::Home, &home, None) {
            self.go(Section::Home);
        }
        let paths = self.learn.catalog.paths.clone();
        ui.add_space(10.0);
        section_label(ui, &self.learn.say("suite_paths"));
        ui.add_space(2.0);
        for path in &paths {
            let sum = self
                .learn
                .catalog
                .path_summary(self.learn.progress(), path, drafts);
            if sum.total == 0 {
                continue;
            }
            let on = current == Section::Path(path.id.clone());
            let count = format!("{}/{}", sum.done, sum.total);
            if rail_item(ui, width, on, path.title(lang), Some(&count)) {
                self.go(Section::Path(path.id.clone()));
            }
        }
        let tracks = self.learn.catalog.tracks.clone();
        ui.add_space(10.0);
        section_label(ui, &self.learn.say("suite_topics"));
        ui.add_space(2.0);
        for track in &tracks {
            let sum = self
                .learn
                .catalog
                .track_summary(self.learn.progress(), &track.id, drafts);
            if sum.total == 0 {
                continue;
            }
            let on = current == Section::Topic(track.id.clone());
            let count = format!("{}/{}", sum.done, sum.total);
            if rail_item(ui, width, on, track.title(lang), Some(&count)) {
                self.go(Section::Topic(track.id.clone()));
            }
        }
    }

    /// The rail folded into chips, for a narrow window.
    fn suite_chips(&mut self, ui: &mut egui::Ui) {
        let lang = self.learn.lang;
        let current = self.suite_section();
        let paths = self.learn.catalog.paths.clone();
        let tracks = self.learn.catalog.tracks.clone();
        ui.horizontal_wrapped(|ui| {
            let home = self.learn.say("suite_home");
            if ui
                .selectable_label(current == Section::Home, home)
                .clicked()
            {
                self.go(Section::Home);
            }
            for path in &paths {
                let on = current == Section::Path(path.id.clone());
                if ui.selectable_label(on, path.title(lang)).clicked() {
                    self.go(Section::Path(path.id.clone()));
                }
            }
            for track in &tracks {
                let on = current == Section::Topic(track.id.clone());
                if ui.selectable_label(on, track.title(lang)).clicked() {
                    self.go(Section::Topic(track.id.clone()));
                }
            }
        });
    }

    // ---- the main area ---------------------------------------------

    fn suite_main(&mut self, ui: &mut egui::Ui, height: f32, enter: bool, start: &mut Start) {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        let section = self.suite_section();
        let filter = self.learn.filter.to_lowercase();
        let words: Vec<String> = filter.split_whitespace().map(str::to_owned).collect();
        let filtering = !words.is_empty() || self.learn.level.is_some();
        egui::ScrollArea::vertical()
            .id_salt("suite_main")
            .max_height(height)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if filtering {
                    self.suite_results(ui, &section, &words, enter, start);
                    return;
                }
                match &section {
                    Section::Home => self.suite_home(ui, start),
                    Section::Path(id) => {
                        let path = self.learn.catalog.paths.iter().find(|p| p.id == *id);
                        if let Some(path) = path.cloned() {
                            self.suite_path(ui, &path, start);
                        }
                    }
                    Section::Topic(id) => {
                        let track = self.learn.catalog.tracks.iter().find(|t| t.id == *id);
                        if let Some(track) = track.cloned() {
                            self.suite_topic(ui, &track, start);
                        }
                    }
                }
                self.suite_problems(ui);
            });
        let _ = (lang, drafts);
    }

    /// Lessons that match the words and the level, within the section.
    pub(super) fn suite_matches(&self, section: &Section, words: &[String]) -> Vec<Entry> {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        let catalog = &self.learn.catalog;
        let pool: Vec<Entry> = match section {
            Section::Home => catalog.visible(drafts).cloned().collect(),
            Section::Path(id) => catalog
                .paths
                .iter()
                .find(|p| p.id == *id)
                .map(|p| catalog.path_lessons(p, drafts).cloned().collect())
                .unwrap_or_default(),
            Section::Topic(id) => catalog
                .visible(drafts)
                .filter(|e| e.lesson.track == *id)
                .cloned()
                .collect(),
        };
        pool.into_iter()
            .filter(|e| self.learn.level.is_none_or(|lv| e.lesson.level == lv))
            .filter(|e| {
                let l = &e.lesson;
                let hay = format!(
                    "{} {} {}",
                    l.title(lang),
                    l.summary(lang),
                    l.topics.join(" ")
                )
                .to_lowercase();
                words.iter().all(|w| hay.contains(w.as_str()))
            })
            .collect()
    }

    fn suite_results(
        &mut self,
        ui: &mut egui::Ui,
        section: &Section,
        words: &[String],
        enter: bool,
        start: &mut Start,
    ) {
        let rows = self.suite_matches(section, words);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(
                    self.learn
                        .say_with("results_count", &[("n", &rows.len().to_string())]),
                )
                .strong(),
            );
            if ui.small_button(self.learn.say("clear_filters")).clicked() {
                self.learn.filter.clear();
                self.learn.level = None;
            }
        });
        ui.add_space(6.0);
        if rows.is_empty() {
            let key = if words.is_empty() {
                "no_level"
            } else {
                "no_match"
            };
            ui.weak(self.learn.say(key));
            return;
        }
        if enter && let Some(first) = rows.first() {
            let running = self.learn.running() == Some(first.lesson.id.as_str());
            if !self.learn.is_coming_soon(&first.lesson.id) {
                *start = Some((first.lesson.id.clone(), running));
            }
        }
        for entry in &rows {
            self.lesson_card(ui, entry, None, start);
            ui.add_space(6.0);
        }
    }

    fn suite_home(&mut self, ui: &mut egui::Ui, start: &mut Start) {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        ui.add_space(2.0);
        ui.heading(self.learn.say("learn_title"));
        ui.weak(self.learn.say("learn_hint"));
        if self.early_is_the_norm() {
            ui.weak(self.learn.say("early_note"));
        }
        ui.add_space(8.0);
        let hero = self.recommendation();
        match &hero {
            Some((entry, reason)) => self.hero_card(ui, entry, reason, start),
            None => {
                egui::Frame::new()
                    .fill(ui_palette().success_wash)
                    .corner_radius(egui::CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(self.learn.say("all_done"));
                    });
            }
        }
        // Other lessons under way.
        let hero_id = hero.as_ref().map(|(e, _)| e.lesson.id.clone());
        let under_way: Vec<Entry> = self
            .learn
            .catalog
            .visible(drafts)
            .filter(|e| {
                matches!(
                    self.learn.progress().standing(&e.lesson),
                    Standing::InProgress { .. }
                )
            })
            .filter(|e| Some(&e.lesson.id) != hero_id.as_ref())
            .take(3)
            .cloned()
            .collect();
        if !under_way.is_empty() {
            ui.add_space(14.0);
            section_label(ui, &self.learn.say("pick_up"));
            ui.add_space(4.0);
            for entry in &under_way {
                self.lesson_card(ui, entry, None, start);
                ui.add_space(6.0);
            }
        }
        // The paths.
        ui.add_space(14.0);
        section_label(ui, &self.learn.say("suite_paths"));
        ui.add_space(4.0);
        let paths: Vec<LearningPath> = self
            .learn
            .catalog
            .paths
            .iter()
            .filter(|p| self.learn.catalog.path_lessons(p, drafts).next().is_some())
            .cloned()
            .collect();
        let cols = if ui.available_width() >= TWO_COLUMNS {
            2
        } else {
            1
        };
        let gap = 10.0;
        let cell = (ui.available_width() - gap * (cols as f32 - 1.0)) / cols as f32;
        egui::Grid::new("suite_paths")
            .num_columns(cols)
            .spacing([gap, gap])
            .show(ui, |ui| {
                for (i, path) in paths.iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_width(cell);
                        self.path_card(ui, path, start);
                    });
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        // The topics.
        ui.add_space(14.0);
        section_label(ui, &self.learn.say("suite_topics"));
        ui.add_space(4.0);
        let tracks: Vec<Track> = self.learn.catalog.tracks.clone();
        let tracks: Vec<Track> = tracks
            .into_iter()
            .filter(|t| {
                self.learn
                    .catalog
                    .track_summary(self.learn.progress(), &t.id, drafts)
                    .total
                    > 0
            })
            .collect();
        egui::Grid::new("suite_topics")
            .num_columns(cols)
            .spacing([gap, gap])
            .show(ui, |ui| {
                for (i, track) in tracks.iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.set_width(cell);
                        self.topic_card(ui, track);
                    });
                    if (i + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
        let _ = lang;
    }

    fn suite_path(&mut self, ui: &mut egui::Ui, path: &LearningPath, start: &mut Start) {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        self.crumb(ui);
        ui.label(egui::RichText::new(path.title(lang)).heading());
        ui.add_space(2.0);
        ui.weak(path.summary(lang));
        ui.add_space(6.0);
        let sum = self
            .learn
            .catalog
            .path_summary(self.learn.progress(), path, drafts);
        self.progress_line(ui, &sum);
        ui.add_space(10.0);
        let rows: Vec<Entry> = self
            .learn
            .catalog
            .path_lessons(path, drafts)
            .cloned()
            .collect();
        for (i, entry) in rows.iter().enumerate() {
            self.lesson_card(ui, entry, Some(i + 1), start);
            ui.add_space(6.0);
        }
    }

    fn suite_topic(&mut self, ui: &mut egui::Ui, track: &Track, start: &mut Start) {
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        self.crumb(ui);
        ui.label(egui::RichText::new(track.title(lang)).heading());
        ui.add_space(2.0);
        let summary = match (lang, &track.summary_nl) {
            (novo_learn::Lang::Nl, Some(nl)) => nl.as_str(),
            _ => track.summary.as_str(),
        };
        ui.weak(summary);
        ui.add_space(6.0);
        let sum = self
            .learn
            .catalog
            .track_summary(self.learn.progress(), &track.id, drafts);
        self.progress_line(ui, &sum);
        ui.add_space(10.0);
        let rows: Vec<Entry> = self
            .learn
            .catalog
            .visible(drafts)
            .filter(|e| e.lesson.track == track.id)
            .cloned()
            .collect();
        for entry in &rows {
            self.lesson_card(ui, entry, None, start);
            ui.add_space(6.0);
        }
    }

    fn crumb(&mut self, ui: &mut egui::Ui) {
        if ui.small_button(self.learn.say("back_home")).clicked() {
            self.go(Section::Home);
        }
        ui.add_space(4.0);
    }

    fn progress_line(&self, ui: &mut egui::Ui, sum: &TrackSummary) {
        let frac = if sum.total == 0 {
            0.0
        } else {
            sum.done as f32 / sum.total as f32
        };
        progress_bar(ui, frac);
        ui.add_space(2.0);
        ui.small(self.learn.say_with(
            "track_progress",
            &[
                ("done", &sum.done.to_string()),
                ("total", &sum.total.to_string()),
                ("minutes", &sum.minutes.to_string()),
            ],
        ));
    }

    fn suite_problems(&mut self, ui: &mut egui::Ui) {
        for (path, error) in self.learn.catalog.rejected.clone() {
            ui.add_space(4.0);
            let why = match error {
                novo_learn::LessonError::NeedsNewer(_)
                | novo_learn::LessonError::UnknownKind(_) => "needs_newer",
                _ => "cannot_open",
            };
            ui.colored_label(
                ui_palette().warning,
                format!("{path}: {} ({error})", self.learn.say(why)),
            );
        }
        for note in self.learn.notes.clone() {
            ui.weak(note);
        }
    }

    // ---- recommendation --------------------------------------------

    /// The lesson to offer first and why: the one on the card, one under
    /// way, the next of a path the person has begun, else the next lesson
    /// whose prerequisites are done.
    pub(super) fn recommendation(&self) -> Option<(Entry, String)> {
        let catalog = &self.learn.catalog;
        let progress = self.learn.progress();
        let drafts = self.learn.show_drafts;
        if let Some(entry) = self.learn.running().and_then(|id| catalog.get(id)) {
            return Some((entry.clone(), self.learn.say("reason_resume_now")));
        }
        if let Some(entry) = catalog
            .visible(drafts)
            .find(|e| matches!(progress.standing(&e.lesson), Standing::InProgress { .. }))
        {
            return Some((entry.clone(), self.learn.say("reason_resume")));
        }
        let lang = self.learn.lang;
        for path in &catalog.paths {
            let sum = catalog.path_summary(progress, path, drafts);
            if sum.done > 0
                && sum.done < sum.total
                && let Some(entry) = catalog.path_next(progress, path, drafts)
            {
                let reason = self
                    .learn
                    .say_with("reason_continue_path", &[("path", path.title(lang))]);
                return Some((entry.clone(), reason));
            }
        }
        let any_done = catalog
            .visible(drafts)
            .any(|e| progress.standing(&e.lesson) == Standing::Completed);
        catalog.next_lesson(progress, drafts).map(|entry| {
            let key = if any_done {
                "reason_next"
            } else {
                "reason_first"
            };
            (entry.clone(), self.learn.say(key))
        })
    }

    // ---- cards ------------------------------------------------------

    fn hero_card(&mut self, ui: &mut egui::Ui, entry: &Entry, reason: &str, start: &mut Start) {
        let p = ui_palette();
        let lang = self.learn.lang;
        let lesson = entry.lesson.clone();
        let running = self.learn.running() == Some(lesson.id.as_str());
        let under_way = matches!(
            self.learn.progress().standing(&lesson),
            Standing::InProgress { .. }
        );
        egui::Frame::new()
            .fill(p.accent_wash)
            .stroke(egui::Stroke::new(1.0, p.accent))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(14))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    let button_w = 120.0;
                    let text_w = (ui.available_width() - button_w - 12.0).max(120.0);
                    ui.vertical(|ui| {
                        ui.set_width(text_w);
                        section_label(ui, reason);
                        ui.add_space(2.0);
                        ui.label(egui::RichText::new(lesson.title(lang)).heading());
                        ui.label(lesson.summary(lang));
                        ui.add_space(4.0);
                        self.meta_line(ui, &lesson);
                    });
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        let label = if running || under_way {
                            self.learn.say("continue")
                        } else {
                            self.learn.say("start")
                        };
                        let button = egui::Button::new(
                            egui::RichText::new(label).strong().color(p.on_accent),
                        )
                        .fill(p.accent);
                        if ui.add_sized([button_w, 34.0], button).clicked() {
                            *start = Some((lesson.id.clone(), running));
                        }
                    });
                });
            });
    }

    fn path_card(&mut self, ui: &mut egui::Ui, path: &LearningPath, start: &mut Start) {
        let p = ui_palette();
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        let sum = self
            .learn
            .catalog
            .path_summary(self.learn.progress(), path, drafts);
        let next = self
            .learn
            .catalog
            .path_next(self.learn.progress(), path, drafts)
            .map(|e| e.lesson.id.clone());
        let levels: Vec<Level> = self
            .learn
            .catalog
            .path_lessons(path, drafts)
            .map(|e| e.lesson.level)
            .collect();
        let low = levels.iter().copied().min_by_key(|l| level_rank(*l));
        let high = levels.iter().copied().max_by_key(|l| level_rank(*l));
        egui::Frame::new()
            .fill(p.sunken)
            .stroke(egui::Stroke::new(1.0, p.border_subtle))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new(path.title(lang)).strong().size(15.0));
                ui.add_space(2.0);
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(path.summary(lang)).color(p.text_secondary),
                    )
                    .wrap(),
                );
                ui.add_space(6.0);
                if let (Some(low), Some(high)) = (low, high) {
                    ui.horizontal(|ui| {
                        level_dots(ui, high);
                        let text = if low == high {
                            self.learn.say(level_key(low))
                        } else {
                            self.learn.say_with(
                                "level_range",
                                &[
                                    ("from", &self.learn.say(level_key(low))),
                                    ("to", &self.learn.say(level_key(high))),
                                ],
                            )
                        };
                        ui.small(text);
                    });
                    ui.add_space(4.0);
                }
                self.progress_line(ui, &sum);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if let Some(id) = &next {
                        let running = self.learn.running() == Some(id.as_str());
                        let label = if running || sum.done > 0 {
                            self.learn.say("continue")
                        } else {
                            self.learn.say("start")
                        };
                        if ui.button(label).clicked() {
                            *start = Some((id.clone(), running));
                        }
                    }
                    if ui.button(self.learn.say("path_show")).clicked() {
                        self.go(Section::Path(path.id.clone()));
                    }
                });
            });
    }

    fn topic_card(&mut self, ui: &mut egui::Ui, track: &Track) {
        let p = ui_palette();
        let lang = self.learn.lang;
        let drafts = self.learn.show_drafts;
        let sum = self
            .learn
            .catalog
            .track_summary(self.learn.progress(), &track.id, drafts);
        egui::Frame::new()
            .fill(p.sunken)
            .stroke(egui::Stroke::new(1.0, p.border_subtle))
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(egui::RichText::new(track.title(lang)).strong());
                self.progress_line(ui, &sum);
                ui.add_space(6.0);
                if ui.button(self.learn.say("open_topic")).clicked() {
                    self.go(Section::Topic(track.id.clone()));
                }
            });
    }

    /// One lesson as a card: where it stands, what it is, how hard and how
    /// long, what it builds on, and a button that is never disabled by a
    /// prerequisite.
    fn lesson_card(
        &mut self,
        ui: &mut egui::Ui,
        entry: &Entry,
        number: Option<usize>,
        start: &mut Start,
    ) {
        let p = ui_palette();
        let lang = self.learn.lang;
        let lesson = entry.lesson.clone();
        let coming = self.learn.is_coming_soon(&lesson.id) || entry.project.is_none();
        let standing = self.learn.progress().standing(&lesson);
        let running = self.learn.running() == Some(lesson.id.as_str());
        let stroke = if running {
            egui::Stroke::new(1.5, p.accent)
        } else {
            egui::Stroke::new(1.0, p.border_subtle)
        };
        egui::Frame::new()
            .fill(p.sunken)
            .stroke(stroke)
            .corner_radius(egui::CornerRadius::same(8))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    status_badge(ui, &standing, number);
                    ui.add_space(4.0);
                    let button_w = 96.0;
                    let text_w = (ui.available_width() - button_w - 10.0).max(120.0);
                    ui.vertical(|ui| {
                        ui.set_width(text_w);
                        let title = egui::RichText::new(lesson.title(lang)).strong();
                        ui.label(if coming {
                            title.color(p.text_tertiary)
                        } else {
                            title
                        });
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(lesson.summary(lang)).color(p.text_secondary),
                            )
                            .wrap(),
                        );
                        ui.add_space(2.0);
                        self.meta_line(ui, &lesson);
                        if coming {
                            ui.small(self.learn.say("coming_soon"));
                        }
                        self.builds_on_line(ui, &lesson, start);
                    });
                    ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                        let label = if running {
                            self.learn.say("continue")
                        } else if standing == Standing::Completed {
                            self.learn.say("do_again")
                        } else if matches!(standing, Standing::InProgress { .. }) {
                            self.learn.say("continue")
                        } else {
                            self.learn.say("start")
                        };
                        let button = egui::Button::new(label);
                        if ui
                            .add_enabled_ui(!coming, |ui| ui.add_sized([button_w, 28.0], button))
                            .inner
                            .clicked()
                        {
                            *start = Some((lesson.id.clone(), running));
                        }
                    });
                });
            });
    }

    /// Level dots and name, minutes, steps left or done, early access.
    fn meta_line(&self, ui: &mut egui::Ui, lesson: &novo_learn::Lesson) {
        let standing = self.learn.progress().standing(lesson);
        ui.horizontal_wrapped(|ui| {
            level_dots(ui, lesson.level);
            ui.small(self.learn.say(level_key(lesson.level)));
            if lesson.minutes > 0 {
                ui.small("·");
                ui.small(
                    self.learn
                        .say_with("minutes", &[("n", &lesson.minutes.to_string())]),
                );
            }
            match standing {
                Standing::InProgress { behind, total } => {
                    ui.small("·");
                    ui.small(self.learn.say_with(
                        "steps_done",
                        &[("n", &behind.to_string()), ("total", &total.to_string())],
                    ));
                }
                Standing::Completed => {
                    ui.small("·");
                    ui.small(self.learn.say("status_done"));
                }
                Standing::Updated => {
                    ui.small("·");
                    ui.small(self.learn.say("updated"));
                }
                Standing::NotStarted => {}
            }
            if lesson.status == novo_learn::Status::Early && !self.early_is_the_norm() {
                ui.small("·");
                ui.small(self.learn.say("early_access"));
            }
        });
    }

    /// "Builds on: A, B": the prerequisites not done yet, as links that open
    /// them. Nothing here stops the lesson.
    fn builds_on_line(
        &mut self,
        ui: &mut egui::Ui,
        lesson: &novo_learn::Lesson,
        start: &mut Start,
    ) {
        let lang = self.learn.lang;
        let unmet: Vec<(String, String)> = self
            .learn
            .catalog
            .unmet_needs(self.learn.progress(), lesson)
            .into_iter()
            .map(|e| (e.lesson.id.clone(), e.lesson.title(lang).to_owned()))
            .collect();
        if unmet.is_empty() {
            return;
        }
        ui.horizontal_wrapped(|ui| {
            ui.small(self.learn.say("do_first"));
            for (id, title) in unmet {
                if ui.small_button(title).clicked() {
                    *start = Some((id, false));
                }
            }
        });
    }
}

/// One entry of the rail: a title that wraps, a count on the right, the
/// selected one on a wash. Focusable and named for assistive tools.
fn rail_item(
    ui: &mut egui::Ui,
    width: f32,
    selected: bool,
    label: &str,
    count: Option<&str>,
) -> bool {
    let p = ui_palette();
    let font = egui::TextStyle::Body.resolve(ui.style());
    let count_w = if count.is_some() { 36.0 } else { 0.0 };
    let galley = ui.painter().layout(
        label.to_owned(),
        font.clone(),
        if selected {
            p.text_strong
        } else {
            p.text_primary
        },
        (width - 16.0 - count_w).max(40.0),
    );
    let height = galley.size().y + 12.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, true, selected, label)
    });
    let fill = if selected {
        p.accent_wash
    } else if response.hovered() {
        p.hover
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(6), fill);
    if selected {
        let bar = egui::Rect::from_min_size(rect.min, egui::vec2(3.0, rect.height()));
        ui.painter()
            .rect_filled(bar, egui::CornerRadius::same(2), p.accent);
    }
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(6),
            egui::Stroke::new(1.5, p.accent),
            egui::StrokeKind::Inside,
        );
    }
    ui.painter().galley(
        egui::pos2(rect.left() + 10.0, rect.top() + 6.0),
        galley,
        p.text_primary,
    );
    if let Some(count) = count {
        ui.painter().text(
            egui::pos2(rect.right() - 8.0, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            count,
            egui::TextStyle::Small.resolve(ui.style()),
            p.text_secondary,
        );
    }
    response.clicked()
}

/// A thin bar, `fraction` of it filled.
fn progress_bar(ui: &mut egui::Ui, fraction: f32) {
    let p = ui_palette();
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 6.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(3), p.border_subtle);
    if fraction > 0.0 {
        let mut fill = rect;
        fill.set_width((rect.width() * fraction.clamp(0.0, 1.0)).max(6.0));
        let colour = if fraction >= 1.0 { p.success } else { p.accent };
        ui.painter()
            .rect_filled(fill, egui::CornerRadius::same(3), colour);
    }
}

/// Three dots, as many filled as the level is high.
fn level_dots(ui: &mut egui::Ui, level: Level) {
    let p = ui_palette();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(30.0, 12.0), egui::Sense::hover());
    let filled = level_rank(level) + 1;
    for i in 0..3 {
        let centre = egui::pos2(rect.left() + 5.0 + 10.0 * i as f32, rect.center().y);
        if i < filled {
            ui.painter().circle_filled(centre, 3.5, p.accent);
        } else {
            ui.painter()
                .circle_stroke(centre, 3.0, egui::Stroke::new(1.0, p.border_control));
        }
    }
}

/// The round mark at the left of a card: a tick when done, a half ring when
/// under way, else the number in the path (or an empty ring).
fn status_badge(ui: &mut egui::Ui, standing: &Standing, number: Option<usize>) {
    let p = ui_palette();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(26.0, 26.0), egui::Sense::hover());
    let centre = rect.center();
    match standing {
        Standing::Completed => {
            ui.painter().circle_filled(centre, 11.0, p.success);
            let tick = [
                centre + egui::vec2(-5.0, 0.5),
                centre + egui::vec2(-1.5, 4.0),
                centre + egui::vec2(5.0, -3.5),
            ];
            ui.painter().add(egui::Shape::line(
                tick.to_vec(),
                egui::Stroke::new(2.0, p.on_accent),
            ));
        }
        Standing::InProgress { .. } => {
            ui.painter()
                .circle_stroke(centre, 11.0, egui::Stroke::new(2.0, p.accent));
            ui.painter().circle_filled(centre, 5.0, p.accent);
        }
        _ => {
            ui.painter()
                .circle_stroke(centre, 11.0, egui::Stroke::new(1.5, p.border_control));
            if let Some(n) = number {
                ui.painter().text(
                    centre,
                    egui::Align2::CENTER_CENTER,
                    n.to_string(),
                    egui::TextStyle::Small.resolve(ui.style()),
                    p.text_secondary,
                );
            }
        }
    }
}
