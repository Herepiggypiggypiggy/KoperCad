//! What a lesson looks like on screen: the card at the bottom right of the
//! canvas, the pointer ring and notes over the control or the spot a step is
//! about, and the Learn window.
//!
//! The card never takes a key and never blocks input. Words come from the
//! lesson files and `ui.toml`; nothing a person reads is typed here.

use eframe::egui;
use novo_learn::{Demo, Entry, Phase, Span, Standing, Target, parse_markup};
use novo_theme::{tokens, widgets::section_label};

use super::NovoApp;
use super::learn::Move;

/// What a press on the card asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
enum CardAction {
    Move(Move),
    ShowMe,
    StopDemo,
    Restart,
    ConfirmRestart,
    CancelRestart,
    Close,
    Open(String),
}

/// The card's words for one frame, copied out so drawing needs no borrow.
struct Card {
    header: String,
    step_title: String,
    text: String,
    /// The author's numbered instructions (`how`), with markup.
    steps: Vec<String>,
    ticked: bool,
    finished: bool,
    can_next: bool,
    first: bool,
    show_me: bool,
    /// The how-to lines of the demonstration, and the one being done.
    how: Vec<(String, bool)>,
    /// A demonstration is on show (playing, or settled on its markers).
    demo_on: bool,
    next_lessons: Vec<(String, String)>,
    words: Words,
}

#[derive(Default)]
struct Words {
    back: String,
    show_me: String,
    how_title: String,
    replay: String,
    stop: String,
    skip: String,
    next: String,
    restart: String,
    restart_confirm: String,
    yes_restart: String,
    cancel: String,
    close: String,
    done: String,
    done_text: String,
    next_lessons: String,
}

impl NovoApp {
    /// The lesson card, the pointer and the notes, late in the frame.
    pub(super) fn learn_overlay(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        if !self.learn.card_on() {
            return;
        }
        self.pointer(ctx, canvas);
        self.learn_demo_overlay(ctx, canvas);
        let Some(card) = self.card() else {
            return;
        };
        let confirm = self.learn.confirm_restart;
        let mut action = None;
        let card_rect = egui::Area::new(egui::Id::new("lesson_card"))
            .order(egui::Order::Middle)
            .pivot(egui::Align2::RIGHT_BOTTOM)
            .fixed_pos(canvas.right_bottom() - egui::vec2(12.0, 72.0))
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_max_width(340.0);
                    action = self.draw_card(ui, &card, confirm);
                });
            })
            .response
            .rect;
        self.learn.card_pos = Some(card_rect.center());
        match action {
            Some(CardAction::Move(how)) => self.learn_move(how),
            Some(CardAction::ShowMe) => self.learn_show_me(),
            Some(CardAction::StopDemo) => self.learn_stop_demo(),
            Some(CardAction::Restart) => self.learn.confirm_restart = true,
            Some(CardAction::ConfirmRestart) => {
                self.learn.confirm_restart = false;
                self.learn_restart();
            }
            Some(CardAction::CancelRestart) => self.learn.confirm_restart = false,
            Some(CardAction::Close) => self.learn.hidden = true,
            Some(CardAction::Open(id)) => self.start_lesson(&id),
            None => {}
        }
    }

    fn card(&self) -> Option<Card> {
        let run = self.learn.run.as_ref()?;
        let lang = self.learn.lang;
        let lesson = run.lesson();
        let say = |key: &str| self.learn.say(key);
        let (behind, total) = run.progress();
        let words = Words {
            back: say("back"),
            show_me: say("show_me"),
            how_title: say("how_title"),
            replay: say("demo_replay"),
            stop: say("demo_stop"),
            skip: say("skip_step"),
            next: say("next"),
            restart: say("restart_lesson"),
            restart_confirm: say("restart_confirm"),
            yes_restart: say("yes_restart"),
            cancel: say("cancel"),
            close: say("close"),
            done: say("done"),
            done_text: say("done_text"),
            next_lessons: say("next_lessons"),
        };
        let n = (run.index() + 1).min(total).to_string();
        let header = format!(
            "{} · {}",
            lesson.title(lang),
            self.learn
                .say_with("step_of", &[("n", &n), ("total", &total.to_string())])
        );
        let _ = behind;
        let finished = run.phase() == Phase::Finished;
        let next_lessons = if finished {
            lesson
                .next
                .iter()
                .filter_map(|id| self.learn.catalog.get(id))
                .map(|e| (e.lesson.id.clone(), e.lesson.title(lang).to_owned()))
                .collect()
        } else {
            Vec::new()
        };
        let step = run.step();
        Some(Card {
            header,
            step_title: step.map(|s| s.title(lang).to_owned()).unwrap_or_default(),
            text: step.map(|s| s.text(lang).to_owned()).unwrap_or_default(),
            steps: step
                .map(|s| {
                    s.how(lang)
                        .iter()
                        .enumerate()
                        .map(|(i, line)| format!("{}. {line}", i + 1))
                        .collect()
                })
                .unwrap_or_default(),
            ticked: run.phase() == Phase::Ticked,
            finished,
            can_next: run.can_advance(),
            first: run.index() == 0,
            show_me: step.is_some_and(|s| {
                s.point.is_some() || !s.note.is_empty() || !Demo::for_step(s).is_empty()
            }),
            how: self.learn_how_lines(),
            demo_on: self.learn.demo.is_some(),
            next_lessons,
            words,
        })
    }

    fn draw_card(&self, ui: &mut egui::Ui, card: &Card, confirm: bool) -> Option<CardAction> {
        let mut action = None;
        ui.horizontal(|ui| {
            section_label(ui, &card.header);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .small_button("✕")
                    .on_hover_text(&card.words.close)
                    .clicked()
                {
                    action = Some(CardAction::Close);
                }
            });
        });
        ui.add_space(2.0);
        if card.finished {
            ui.label(egui::RichText::new(format!("✓ {}", card.words.done)).strong());
            ui.label(&card.words.done_text);
            if !card.next_lessons.is_empty() {
                ui.add_space(4.0);
                ui.weak(&card.words.next_lessons);
                for (id, title) in &card.next_lessons {
                    if ui.link(title).clicked() {
                        action = Some(CardAction::Open(id.clone()));
                    }
                }
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                if ui.button(&card.words.close).clicked() {
                    action = Some(CardAction::Close);
                }
                if ui.small_button(&card.words.restart).clicked() {
                    action = Some(CardAction::Restart);
                }
            });
        } else {
            let mark = if card.ticked { "✓" } else { "●" };
            let title = egui::RichText::new(format!("{mark} {}", card.step_title)).strong();
            ui.label(if card.ticked {
                title.color(tokens::SUCCESS)
            } else {
                title
            });
            self.markup(ui, &card.text);
            if !card.steps.is_empty() {
                ui.add_space(4.0);
                section_label(ui, &card.words.how_title);
                for line in &card.steps {
                    self.markup(ui, line);
                }
            }
            if card.steps.is_empty() && !card.how.is_empty() {
                ui.add_space(4.0);
                section_label(ui, &card.words.how_title);
                for (line, active) in &card.how {
                    let text = egui::RichText::new(line);
                    ui.label(if *active {
                        text.strong().color(tokens::ACCENT_HOVER)
                    } else {
                        text
                    });
                }
            }
            ui.add_space(4.0);
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add_enabled(!card.first, egui::Button::new(&card.words.back))
                    .clicked()
                {
                    action = Some(CardAction::Move(Move::Back));
                }
                if card.show_me {
                    let label = if card.demo_on {
                        &card.words.replay
                    } else {
                        &card.words.show_me
                    };
                    if ui.button(label).clicked() {
                        action = Some(CardAction::ShowMe);
                    }
                }
                if card.demo_on && ui.button(&card.words.stop).clicked() {
                    action = Some(CardAction::StopDemo);
                }
                if card.can_next {
                    if novo_theme::widgets::primary_button(ui, &card.words.next).clicked() {
                        action = Some(CardAction::Move(Move::Next));
                    }
                } else if ui.button(&card.words.skip).clicked() {
                    action = Some(CardAction::Move(Move::Skip));
                }
            });
            ui.add_space(2.0);
            if confirm {
                ui.label(&card.words.restart_confirm);
                ui.horizontal(|ui| {
                    if ui.button(&card.words.yes_restart).clicked() {
                        action = Some(CardAction::ConfirmRestart);
                    }
                    if ui.button(&card.words.cancel).clicked() {
                        action = Some(CardAction::CancelRestart);
                    }
                });
            } else if ui
                .small_button(format!("⟲ {}", card.words.restart))
                .clicked()
            {
                action = Some(CardAction::Restart);
            }
        }
        action
    }

    /// Step text with its keys, control names and glossary words.
    fn markup(&self, ui: &mut egui::Ui, text: &str) {
        let spans = match parse_markup(text) {
            Ok(spans) => spans,
            Err(e) => {
                ui.colored_label(tokens::ERROR, e.to_string());
                return;
            }
        };
        ui.scope(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            ui.horizontal_wrapped(|ui| {
                for span in &spans {
                    match span {
                        Span::Text(t) => {
                            ui.label(t);
                        }
                        Span::Bold(t) => {
                            ui.label(egui::RichText::new(t).strong());
                        }
                        Span::Key(command) => {
                            let key = self.learn_key_text(command);
                            ui.label(
                                egui::RichText::new(format!(" {key} "))
                                    .monospace()
                                    .color(tokens::ACCENT_HOVER)
                                    .background_color(tokens::ACCENT_SELECTION),
                            );
                        }
                        Span::Ui(id) => {
                            let name = super::learn_labels::any_anchor_label(id)
                                .or_else(|| self.learn.labels.get(id).cloned())
                                .unwrap_or_else(|| id.rsplit('.').next().unwrap_or(id).to_owned());
                            ui.label(egui::RichText::new(name).strong());
                        }
                        Span::Tip(term) => {
                            let tip = self
                                .learn
                                .catalog
                                .glossary
                                .get(term)
                                .map(|t| t.text(self.learn.lang).to_owned());
                            let label = egui::RichText::new(term).underline();
                            let response = ui.label(label);
                            if let Some(tip) = tip {
                                response.on_hover_text(tip);
                            }
                        }
                        Span::Break => {
                            ui.end_row();
                            ui.add_space(4.0);
                            ui.end_row();
                        }
                    }
                }
            });
        });
    }

    /// The ring over the step's target and its notes.
    fn pointer(&mut self, ctx: &egui::Context, canvas: egui::Rect) {
        let Some(run) = self.learn.run.as_ref() else {
            return;
        };
        let Some(step) = run.step() else {
            return;
        };
        if run.phase() == Phase::Ticked {
            return;
        }
        let lang = self.learn.lang;
        let mut targets: Vec<(Option<usize>, Target, Option<String>)> = Vec::new();
        if let Some(point) = &step.point {
            targets.push((None, point.clone(), None));
        }
        for (i, note) in step.note.iter().enumerate() {
            targets.push((
                Some(i + 1),
                note.target.clone(),
                Some(note.text(lang).to_owned()),
            ));
        }
        let flashing = self
            .learn
            .flash_until
            .is_some_and(|until| std::time::Instant::now() < until);
        let calm = self.settings.reduce_motion;
        let pulse = if calm {
            1.0
        } else {
            let t = ctx.input(|i| i.time) as f32;
            if flashing {
                0.5 + 0.5 * (t * 6.0).sin().abs()
            } else {
                0.6 + 0.4 * (t * 2.5).sin().abs()
            }
        };
        if !calm {
            ctx.request_repaint_after(std::time::Duration::from_millis(33));
        }
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("lesson_pointer"),
        ));
        for (number, target, note) in targets {
            let Some(rect) = self.target_rect(&target, canvas) else {
                continue;
            };
            let color = tokens::ACCENT_HOVER.gamma_multiply(pulse);
            let ring = rect.expand(4.0);
            painter.rect_stroke(
                ring,
                ring.height().min(ring.width()).min(10.0),
                egui::Stroke::new(2.0, color),
                egui::StrokeKind::Outside,
            );
            if let Some(n) = number {
                let badge = egui::pos2(ring.left() - 2.0, ring.top() - 2.0);
                painter.circle_filled(badge, 9.0, tokens::ACCENT);
                painter.text(
                    badge,
                    egui::Align2::CENTER_CENTER,
                    n.to_string(),
                    egui::FontId::proportional(11.0),
                    tokens::TEXT_ON_ACCENT,
                );
            }
            if let Some(text) = note {
                let below = egui::pos2(ring.left(), ring.bottom() + 6.0);
                egui::Area::new(egui::Id::new((
                    "lesson_note",
                    below.x as i32,
                    below.y as i32,
                )))
                .order(egui::Order::Foreground)
                .interactable(false)
                .fixed_pos(below)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_max_width(220.0);
                        ui.label(text);
                    });
                });
            }
        }
    }

    /// The screen rectangle of a target: a registered control, or a spot on
    /// the page shown.
    pub(super) fn target_rect(&self, target: &Target, canvas: egui::Rect) -> Option<egui::Rect> {
        if let Some(ui) = &target.ui {
            return ui.ids().iter().rev().find_map(|id| self.learn_anchor(id));
        }
        let spot = self.learn_canvas_spot(target)?;
        if self.current_page != Some(spot.page) {
            return None;
        }
        let view = self.canvas.schematic_ref()?;
        let to_screen = |x: f64, y: f64| {
            let s = view
                .canvas
                .viewport
                .world_to_screen(novo_canvas::vec2(x as f32, y as f32));
            egui::pos2(canvas.left() + s.x, canvas.top() + s.y)
        };
        let a = to_screen(spot.min.0, spot.min.1);
        let b = to_screen(spot.max.0, spot.max.1);
        let rect = egui::Rect::from_two_pos(a, b);
        let rect = if rect.width() < 1.0 && rect.height() < 1.0 {
            egui::Rect::from_center_size(a, egui::vec2(24.0, 24.0))
        } else {
            rect
        };
        rect.intersects(canvas).then_some(rect)
    }

    // ------------------------------------------------------------------
    // The Learn window

    pub(super) fn learn_window(&mut self, ctx: &egui::Context) {
        if !self.learn.window {
            return;
        }
        let title = self.learn.say("learn");
        let mut open = true;
        let mut start: Option<(String, bool)> = None;
        let mut reload = false;
        let mut reset = false;
        super::panel::dialog(ctx, title, [560.0, 560.0])
            .id(egui::Id::new("learn_window"))
            .open(&mut open)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.content_rect().center())
            .collapsible(false)
            .show(ctx, |ui| {
                self.window_body(ui, &mut start, &mut reload, &mut reset);
            });
        if !open {
            self.learn.window = false;
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

    fn window_body(
        &mut self,
        ui: &mut egui::Ui,
        start: &mut Option<(String, bool)>,
        reload: &mut bool,
        reset: &mut bool,
    ) {
        let hint = self.learn.say("learn_hint");
        ui.weak(hint);
        ui.add_space(4.0);
        self.continue_card(ui, start);
        let search = self.learn.say("search");
        ui.add(
            egui::TextEdit::singleline(&mut self.learn.filter)
                .hint_text(search)
                .desired_width(f32::INFINITY),
        );
        ui.add_space(6.0);
        let lang = self.learn.lang;
        let filter = self.learn.filter.to_lowercase();
        let words: Vec<&str> = filter.split_whitespace().collect();
        let drafts = self.learn.show_drafts;
        let entries: Vec<Entry> = self.learn.catalog.visible(drafts).cloned().collect();
        let tracks = self.learn.catalog.tracks.clone();
        let mut shown = 0;
        egui::ScrollArea::vertical()
            .max_height(ui.available_height() - 40.0)
            .show(ui, |ui| {
                for track in &tracks {
                    let rows: Vec<&Entry> = entries
                        .iter()
                        .filter(|e| e.lesson.track == track.id)
                        .filter(|e| {
                            let l = &e.lesson;
                            let hay = format!(
                                "{} {} {}",
                                l.title(lang),
                                l.summary(lang),
                                l.topics.join(" ")
                            )
                            .to_lowercase();
                            words.iter().all(|w| hay.contains(w))
                        })
                        .collect();
                    if rows.is_empty() {
                        continue;
                    }
                    ui.add_space(6.0);
                    section_label(ui, track.title(lang));
                    let sum =
                        self.learn
                            .catalog
                            .track_summary(self.learn.progress(), &track.id, drafts);
                    ui.small(self.learn.say_with(
                        "track_progress",
                        &[
                            ("done", &sum.done.to_string()),
                            ("total", &sum.total.to_string()),
                            ("minutes", &sum.minutes.to_string()),
                        ],
                    ));
                    for entry in rows {
                        shown += 1;
                        self.lesson_row(ui, entry, start);
                    }
                }
                if shown == 0 {
                    ui.add_space(8.0);
                    let key = if entries.is_empty() {
                        "no_lessons"
                    } else {
                        "no_match"
                    };
                    ui.weak(self.learn.say(key));
                }
                for (path, error) in self.learn.catalog.rejected.clone() {
                    ui.add_space(4.0);
                    let why = match error {
                        novo_learn::LessonError::NeedsNewer(_)
                        | novo_learn::LessonError::UnknownKind(_) => "needs_newer",
                        _ => "cannot_open",
                    };
                    ui.colored_label(
                        tokens::WARNING,
                        format!("{path}: {} ({error})", self.learn.say(why)),
                    );
                }
                for note in self.learn.notes.clone() {
                    ui.weak(note);
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            if ui.small_button(self.learn.say("reload")).clicked() {
                *reload = true;
            }
            if ui.small_button(self.learn.say("reset_progress")).clicked() {
                *reset = true;
            }
            let drafts_label = self.learn.say("show_drafts");
            ui.checkbox(&mut self.learn.show_drafts, drafts_label);
        });
    }

    fn lesson_row(&mut self, ui: &mut egui::Ui, entry: &Entry, start: &mut Option<(String, bool)>) {
        let lang = self.learn.lang;
        let lesson = &entry.lesson;
        let coming = self.learn.is_coming_soon(&lesson.id) || entry.project.is_none();
        let standing = self.learn.progress().standing(lesson);
        let running = self.learn.running() == Some(lesson.id.as_str());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.vertical(|ui| {
                let title = egui::RichText::new(lesson.title(lang)).strong();
                ui.label(if coming { title.weak() } else { title });
                ui.weak(lesson.summary(lang));
                let mut meta = Vec::new();
                if lesson.minutes > 0 {
                    meta.push(
                        self.learn
                            .say_with("minutes", &[("n", &lesson.minutes.to_string())]),
                    );
                }
                match standing {
                    Standing::InProgress { behind, total } => {
                        meta.push(format!("{behind}/{total}"));
                    }
                    Standing::Completed => meta.push("✓".to_owned()),
                    Standing::Updated => meta.push(self.learn.say("updated")),
                    Standing::NotStarted => {}
                }
                if coming {
                    meta.push(self.learn.say("coming_soon"));
                }
                if lesson.status == novo_learn::Status::Early {
                    meta.push(self.learn.say("early_access"));
                }
                if !meta.is_empty() {
                    ui.small(meta.join(" · "));
                }
                self.do_first_line(ui, lesson, start);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if running {
                    self.learn.say("continue")
                } else if standing == Standing::Completed {
                    self.learn.say("do_again")
                } else {
                    self.learn.say("start")
                };
                if ui.add_enabled(!coming, egui::Button::new(label)).clicked() {
                    *start = Some((lesson.id.clone(), running));
                }
            });
        });
    }

    /// The card on top of the window: the lesson under way, else the next
    /// one the person can do, else a line saying everything is done.
    fn continue_card(&mut self, ui: &mut egui::Ui, start: &mut Option<(String, bool)>) {
        let lang = self.learn.lang;
        let next = self
            .learn
            .catalog
            .next_lesson(self.learn.progress(), self.learn.show_drafts)
            .cloned();
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let Some(entry) = next else {
                ui.label(self.learn.say("all_done"));
                return;
            };
            let lesson = &entry.lesson;
            let running = self.learn.running() == Some(lesson.id.as_str());
            let under_way = matches!(
                self.learn.progress().standing(lesson),
                Standing::InProgress { .. }
            );
            let (heading, button) = if running || under_way {
                ("continue_where", "continue")
            } else {
                ("next_lesson", "start")
            };
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.small(self.learn.say(heading));
                    ui.label(egui::RichText::new(lesson.title(lang)).strong());
                    if lesson.minutes > 0 {
                        ui.weak(
                            self.learn
                                .say_with("minutes", &[("n", &lesson.minutes.to_string())]),
                        );
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(self.learn.say(button)).clicked() {
                        *start = Some((lesson.id.clone(), running));
                    }
                });
            });
        });
        ui.add_space(6.0);
    }

    /// "Do first: A, B" with the prerequisites not done yet as links.
    fn do_first_line(
        &mut self,
        ui: &mut egui::Ui,
        lesson: &novo_learn::Lesson,
        start: &mut Option<(String, bool)>,
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

    /// The start panel's Learn entry: a button, and a line for a lesson
    /// that is still open.
    pub(super) fn learn_start_entry(&mut self, ui: &mut egui::Ui) {
        let label = self.learn.say("learn_menu");
        let response = ui.button(label);
        self.anchor_response("start.learn", &response, "Learn");
        if response.clicked() {
            self.learn.window = true;
        }
    }
}
