//! The Claude assistant window (Help > Ask Claude, or Ctrl+K "Ask Claude").
//!
//! Off until the person turns it on in the window itself: they paste their
//! own Anthropic API key, choose how much of the project Claude may read and
//! agree to the notice. Messages run on a background thread; the window only
//! polls a channel. Claude never edits the real project: it builds in a private draft, its
//! change sets arrive as cards with Apply and Reject, and Apply goes through one transaction,
//! so one Ctrl+Z undoes it. The engine is `novo-ai`.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_ai::agent::{Conversation, Event, TurnResult};
use novo_ai::api::{ApiError, Client, Transport, UreqTransport};
use novo_ai::consent::{CONSENT_VERSION, Consent, DataScope, NOTICE};
use novo_ai::engineer::{
    ChangeSetOutcome, EngineerHost, PlanItem, Question, Resources, Step, commit,
};
use novo_ai::key::{ApiKey, KeySource, KeyStore};
use novo_ai::models::{self, MODELS};
use novo_ai::usage::{Ledger, format_usd, month_of};
use novo_core::Project;
use serde_json::Value;

mod datasheet;

use super::{Level, NovoApp};
use crate::theme::ui_palette;

type TransportFactory = Arc<dyn Fn() -> Box<dyn Transport> + Send + Sync>;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum ProposalState {
    Pending,
    Applied,
    Rejected,
    Failed(String),
    /// A later change set contains these steps and more: only that one can be applied.
    Superseded,
}

/// The change set of a turn: what Claude built in its draft, and what the
/// program itself says about it.
pub(super) struct Card {
    pub outcome: ChangeSetOutcome,
    pub state: ProposalState,
}

/// Where taking a picture of the window has got to.
#[derive(Clone, Copy, PartialEq)]
enum Shot {
    Idle,
    /// The assistant window steps out of the way for one frame.
    Hide,
    /// Waiting for the picture to come back (frames waited).
    Waiting(u32),
}

/// Longest side of a picture sent, in pixels.
const PICTURE_MAX_SIDE: u32 = 1400;

struct Picture {
    image: novo_ai::image::Image,
    size: [u32; 2],
}

/// The window's pixels as a PNG, shrunk to a size that reads well and costs little.
fn picture_of(img: &egui::ColorImage) -> Result<Picture, String> {
    let (w, h) = (img.size[0] as u32, img.size[1] as u32);
    let rgba: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    let mut buf = image::RgbaImage::from_raw(w, h, rgba).ok_or("the picture has the wrong size")?;
    if w.max(h) > PICTURE_MAX_SIDE {
        let f = f64::from(PICTURE_MAX_SIDE) / f64::from(w.max(h));
        let (nw, nh) = (
            ((f64::from(w) * f) as u32).max(1),
            ((f64::from(h) * f) as u32).max(1),
        );
        buf = image::imageops::resize(&buf, nw, nh, image::imageops::FilterType::Triangle);
    }
    let mut png = std::io::Cursor::new(Vec::new());
    buf.write_to(&mut png, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(Picture {
        size: [buf.width(), buf.height()],
        image: novo_ai::image::Image::png(&png.into_inner()),
    })
}

pub(super) enum Entry {
    User(String),
    Claude(String),
    /// A lookup Claude made, shown small ("Looked at the project outline").
    Tool(String),
    Problem(String),
    /// The change set to apply or reject.
    ChangeSet(Box<Card>),
    /// The checklist Claude worked through.
    Plan(Vec<PlanItem>),
    /// A question Claude waits on; the options answer it.
    Question(Question),
}

enum Msg {
    Event(Event),
    Done(
        Result<TurnResult, ApiError>,
        Conversation,
        Box<Option<ChangeSetOutcome>>,
    ),
}

/// Everything the window remembers while the app runs.
pub(super) struct State {
    pub open: bool,
    /// The plan Claude is showing while it works (replaced by each set_plan).
    live_plan: Vec<PlanItem>,
    show_settings: bool,
    /// The `CONSENT_VERSION` whose notice has been on screen in full: the box
    /// to agree stays off until it has been, whatever the window height.
    notice_seen: Option<u32>,
    /// Where the notice was drawn this frame, to tell whether it was in view.
    notice_rect: Option<egui::Rect>,
    /// Tests only: whether "Turn Claude on" could be pressed in the last frame.
    turn_on_ready: bool,
    /// Tests only: retries do not wait.
    instant_retries: bool,
    agreed: bool,
    key_input: String,
    key_note: Option<(bool, String)>,
    key_check: Option<Receiver<Result<(), ApiError>>>,
    draft: String,
    transcript: Vec<Entry>,
    conversation: Conversation,
    running: Option<Receiver<Msg>>,
    /// Answer text that has arrived for the turn in progress.
    live: String,
    /// Claude is reasoning before it writes.
    thinking: bool,
    /// Set by the Stop button; the background thread reads it.
    cancel: Arc<AtomicBool>,
    shot: Shot,
    picture: Option<Picture>,
    sheet: datasheet::State,
    /// The datasheet being read, for the review heading.
    datasheet_name: String,
    /// The scope that was agreed when the picture was taken.
    picture_scope: DataScope,
    /// What the person decided about proposals since their last message;
    /// sent along with the next one so Claude knows.
    notes: Vec<String>,
    ledger: Ledger,
    ledger_path: Option<PathBuf>,
    session_cost: f64,
    /// A question to send as soon as the window can (from a button elsewhere).
    pending_question: Option<String>,
    /// The widest scope any answer in the running conversation was given
    /// under. If the scope later narrows, the conversation is dropped: what
    /// Claude already read must not keep informing answers the person
    /// restricted.
    conv_scope: DataScope,
    /// A wider scope the person picked while Claude is on, waiting for a
    /// second, explicit agreement.
    pending_scope: Option<DataScope>,
    store: Option<KeyStore>,
    transport: TransportFactory,
    /// Set by tests: the clock for the month and consent time.
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

/// Name of the variable that makes the assistant answer from a file instead of the
/// network, for the QA harness: the file holds a JSON list of Messages API replies,
/// used in order. Nothing is sent anywhere while it is set, and the window says so.
pub(super) const REPLAY_VAR: &str = "KOPERCAD_AI_REPLAY";

/// The replay file read once: the script, or why it could not be read.
/// While the variable is set the network is never used, even when this fails.
fn replay() -> Option<&'static Result<novo_ai::api::testing::Script, String>> {
    static REPLAY: std::sync::OnceLock<Option<Result<novo_ai::api::testing::Script, String>>> =
        std::sync::OnceLock::new();
    REPLAY
        .get_or_init(|| {
            let path = std::env::var_os(REPLAY_VAR)?;
            Some(load_replay(&path))
        })
        .as_ref()
}

pub(super) fn load_replay(path: &std::ffi::OsStr) -> Result<novo_ai::api::testing::Script, String> {
    let shown = path.to_string_lossy();
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {shown}: {e}"))?;
    let list: Vec<Value> = serde_json::from_slice(&bytes)
        .map_err(|e| format!("{shown} is not a JSON list of replies: {e}"))?;
    let script = novo_ai::api::testing::Script::default();
    for reply in list {
        script.push(200, reply);
    }
    Ok(script)
}

/// The line the window shows while the variable is set.
fn replay_notice() -> Option<(String, bool)> {
    Some(match replay()? {
        Ok(_) => (
            "Test replay: answers come from a file, nothing is sent".to_string(),
            true,
        ),
        Err(why) => (
            format!("Test replay failed ({why}). Nothing is sent and no answers will come"),
            false,
        ),
    })
}

fn default_transport() -> TransportFactory {
    transport_for(replay())
}

pub(super) fn transport_for(
    replay: Option<&Result<novo_ai::api::testing::Script, String>>,
) -> TransportFactory {
    match replay {
        // A readable file answers from it; an unreadable one gives an empty
        // script, so every request fails here instead of reaching the network.
        Some(Ok(script)) => {
            let script = script.clone();
            Arc::new(move || Box::new(script.clone()))
        }
        Some(Err(_)) => {
            let empty = novo_ai::api::testing::Script::default();
            Arc::new(move || Box::new(empty.clone()))
        }
        None => Arc::new(|| Box::new(UreqTransport::new())),
    }
}

impl State {
    pub fn new() -> Self {
        Self::with(
            KeyStore::in_profile(),
            novo_ai::key::default_dir().map(|d| d.join("usage.json")),
            default_transport(),
            Arc::new(unix_now),
        )
    }

    fn with(
        store: Option<KeyStore>,
        ledger_path: Option<PathBuf>,
        transport: TransportFactory,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        let ledger = ledger_path.as_deref().map(Ledger::load).unwrap_or_default();
        Self {
            open: false,
            live_plan: Vec::new(),
            show_settings: false,
            notice_seen: None,
            notice_rect: None,
            turn_on_ready: false,
            instant_retries: false,
            agreed: false,
            key_input: String::new(),
            key_note: None,
            key_check: None,
            draft: String::new(),
            transcript: Vec::new(),
            conversation: Conversation::default(),
            running: None,
            live: String::new(),
            thinking: false,
            cancel: Arc::new(AtomicBool::new(false)),
            shot: Shot::Idle,
            picture: None,
            sheet: datasheet::State::default(),
            datasheet_name: String::new(),
            picture_scope: DataScope::None,
            notes: Vec::new(),
            ledger,
            ledger_path,
            session_cost: 0.0,
            pending_question: None,
            conv_scope: DataScope::None,
            pending_scope: None,
            store,
            transport,
            now,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(store: KeyStore, ledger: PathBuf, transport: TransportFactory) -> Self {
        let mut state = Self::with(
            Some(store),
            Some(ledger),
            transport,
            Arc::new(|| 1_792_000_000),
        );
        state.instant_retries = true;
        state
    }

    #[cfg(test)]
    pub(super) fn attach_picture_requested(&mut self) {
        self.shot = Shot::Hide;
    }

    #[cfg(test)]
    pub(super) fn has_picture(&self) -> bool {
        self.picture.is_some()
    }

    #[cfg(test)]
    pub(super) fn conversation_len(&self) -> usize {
        self.conversation.messages.len()
    }

    #[cfg(test)]
    pub(super) fn set_draft(&mut self, text: &str) {
        self.draft = text.to_string();
    }

    #[cfg(test)]
    pub(super) fn is_running(&self) -> bool {
        self.running.is_some()
    }

    #[cfg(test)]
    pub(super) fn session_cost(&self) -> f64 {
        self.session_cost
    }

    #[cfg(test)]
    pub(super) fn month_spent(&self) -> f64 {
        self.spent()
    }

    #[cfg(test)]
    pub(super) fn first_proposal_index(&self) -> Option<usize> {
        self.transcript
            .iter()
            .position(|e| matches!(e, Entry::ChangeSet(..)))
    }

    /// Tests only: puts a change set of `steps` (action, args), built in a draft of
    /// `project` by the same engine Claude uses, on a card. `Err` is the reason a step was refused.
    #[cfg(test)]
    pub(super) fn test_push_card(
        &mut self,
        project: &Project,
        steps: &[(&str, Value)],
    ) -> Result<usize, String> {
        let mut host = EngineerHost::new(
            project.clone(),
            Vec::new(),
            DataScope::Project,
            Resources::new(Vec::new()),
            Box::new(|_| novo_ai::tools::Snapshot::default()),
        );
        for (action, args) in steps {
            host.apply(action, args)?;
        }
        self.transcript.push(Entry::ChangeSet(Box::new(Card {
            outcome: host.outcome(),
            state: ProposalState::Pending,
        })));
        Ok(self.transcript.len() - 1)
    }

    /// Tests only: what the transcript holds, in order.
    #[cfg(test)]
    pub(super) fn entry_kinds(&self) -> Vec<&'static str> {
        self.transcript
            .iter()
            .map(|e| match e {
                Entry::User(_) => "user",
                Entry::Claude(_) => "claude",
                Entry::Tool(_) => "tool",
                Entry::Problem(_) => "problem",
                Entry::ChangeSet(_) => "changeset",
                Entry::Plan(_) => "plan",
                Entry::Question(_) => "question",
            })
            .collect()
    }

    /// Tests only: the change set on the card at `index`.
    #[cfg(test)]
    pub(super) fn card_outcome(&self, index: usize) -> Option<&ChangeSetOutcome> {
        match self.transcript.get(index) {
            Some(Entry::ChangeSet(c)) => Some(&c.outcome),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn turn_on_ready(&self) -> bool {
        self.turn_on_ready
    }

    #[cfg(test)]
    pub(super) fn notice_seen(&self) -> bool {
        self.notice_seen == Some(CONSENT_VERSION)
    }

    #[cfg(test)]
    pub(super) fn agree_for_test(&mut self) {
        self.agreed = true;
    }

    #[cfg(test)]
    pub(super) fn proposal_state(&self, index: usize) -> Option<ProposalState> {
        match self.transcript.get(index) {
            Some(Entry::ChangeSet(card)) => Some(card.state.clone()),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn transcript_text(&self) -> String {
        self.transcript
            .iter()
            .map(|e| match e {
                Entry::User(t) | Entry::Claude(t) | Entry::Tool(t) | Entry::Problem(t) => t.clone(),
                Entry::ChangeSet(c) => c
                    .outcome
                    .steps
                    .iter()
                    .map(|s| s.summary.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
                Entry::Plan(items) => items
                    .iter()
                    .map(|i| i.title.clone())
                    .collect::<Vec<_>>()
                    .join("\n"),
                Entry::Question(q) => q.text.clone(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Opens the window and sends `question` when no answer is in progress.
    pub(super) fn ask_later(&mut self, question: String) {
        self.open = true;
        self.pending_question = Some(question);
    }

    fn month(&self) -> String {
        month_of((self.now)())
    }

    pub(super) fn spent(&self) -> f64 {
        self.ledger.spent_in(&self.month())
    }

    fn key(&self) -> Option<(ApiKey, KeySource)> {
        self.store.as_ref().and_then(KeyStore::load)
    }

    /// Called before each message with the scope now in force. Narrower than
    /// the conversation's scope: forget the conversation and say so.
    fn enforce_scope(&mut self, scope: DataScope) {
        if self.picture.is_some() && scope < self.picture_scope {
            self.picture = None;
            self.shot = Shot::Idle;
            self.transcript.push(Entry::Problem(
                "Scope narrowed, so the attached picture was dropped.".into(),
            ));
        }
        if scope < self.conv_scope {
            self.conversation.clear();
            self.notes.clear();
            let had_picture = self.picture.take().is_some();
            self.shot = Shot::Idle;
            self.transcript.push(Entry::Problem(if had_picture {
                "Scope narrowed, so the earlier chat and the attached picture were cleared. \
                 Claude no longer has what it read or saw before."
                    .into()
            } else {
                "Scope narrowed, so the earlier chat was cleared. Claude no longer has what it \
                 read before."
                    .into()
            }));
        }
        self.conv_scope = scope;
    }

    /// Moves the text streamed so far into the transcript.
    fn flush_live(&mut self) {
        if !self.live.is_empty() {
            self.transcript
                .push(Entry::Claude(std::mem::take(&mut self.live)));
        }
    }

    pub(super) fn clear_chat(&mut self) {
        self.transcript.clear();
        self.conversation.clear();
        self.notes.clear();
        self.live_plan.clear();
        self.picture = None;
        self.shot = Shot::Idle;
        self.session_cost = 0.0;
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

const ON_ID: &str = "kopercad_ai_on";
const ASK_ID: &str = "kopercad_ai_ask";

/// True when Claude is turned on, so a panel may offer an "… with Claude" button.
pub(super) fn claude_is_on(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(egui::Id::new(ON_ID)))
        .unwrap_or(false)
}

/// Leaves a question for the Ask Claude window; it is sent on the next frame
/// when Claude is on and no answer is in progress.
pub(super) fn ask_claude_later(ctx: &egui::Context, question: String) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new(ASK_ID), question));
}

pub(super) use novo_ai::engineer::snapshot::snapshot_of;

impl NovoApp {
    pub(super) fn open_assistant(&mut self) {
        self.wb.assistant.open = true;
        // First time: land on the settings, where the choices are.
        if !self.settings.ai.enabled {
            self.wb.assistant.show_settings = true;
        }
    }

    /// Ctrl+K found no command: open the window and ask the question.
    pub(super) fn ask_claude(&mut self, ctx: &egui::Context, question: String) {
        self.wb.assistant.open = true;
        if self.wb.assistant.running.is_some() {
            self.wb.assistant.transcript.push(Entry::Problem(
                "Claude is still answering. Ask again when it has finished.".into(),
            ));
            return;
        }
        self.assistant_send(ctx, question);
    }

    fn selected_ids(&self) -> Vec<u64> {
        self.canvas
            .schematic_ref()
            .map(|v| v.selection().iter().map(|id| id.raw()).collect())
            .unwrap_or_default()
    }

    fn assistant_send(&mut self, ctx: &egui::Context, text: String) {
        let Some((key, _)) = self.wb.assistant.key() else {
            self.wb.assistant.transcript.push(Entry::Problem(
                "No API key yet. Add yours in the settings above.".into(),
            ));
            return;
        };
        let scope = self.settings.ai.consent.effective();
        let selected = self.selected_ids();
        let base = self
            .backend
            .document()
            .map_or_else(|| Project::new(""), |d| d.project().clone());
        let st = &mut self.wb.assistant;
        st.enforce_scope(scope);
        // Steps of a change set nobody has decided yet are carried into the new
        // draft, so "and also add ..." builds on them instead of dropping them.
        let carried: Vec<Step> = st
            .transcript
            .iter()
            .rev()
            .find_map(|e| match e {
                Entry::ChangeSet(c) if c.state == ProposalState::Pending => {
                    Some(c.outcome.steps.clone())
                }
                _ => None,
            })
            .unwrap_or_default();
        st.live_plan.clear();
        st.live.clear();
        st.thinking = true;
        st.cancel.store(false, Ordering::Relaxed);
        let cancel = Arc::clone(&st.cancel);
        let picture = st.picture.take();
        let mut message = String::new();
        for n in st.notes.drain(..) {
            message.push_str(&format!("[{n}]\n"));
        }
        message.push_str(&text);
        st.transcript.push(Entry::User(if picture.is_some() {
            format!("{text}\n[a picture of the window is attached]")
        } else {
            text
        }));
        let settings = self.settings.ai.clone();
        let spent = st.spent();
        let mut conversation = std::mem::take(&mut st.conversation);
        if let Some(p) = picture {
            conversation.images.push(p.image);
        }
        let factory = Arc::clone(&st.transport);
        let instant = st.instant_retries;
        let (tx, rx) = channel();
        st.running = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut client = Client::new(factory(), key);
            if instant {
                client = client.with_sleep(|_| {});
            }
            let mut host = EngineerHost::new(
                base,
                carried,
                scope,
                Resources::new(user_macros()),
                Box::new(move |p| snapshot_of(p, &selected, scope)),
            );
            let events = tx.clone();
            let events_ctx = ctx.clone();
            let result = conversation.run_turn(
                &mut client,
                &mut host,
                &settings,
                spent,
                &message,
                &mut |e| {
                    let _ = events.send(Msg::Event(e));
                    events_ctx.request_repaint();
                },
                &cancel,
            );
            let outcome = Box::new(Some(host.outcome()));
            let _ = tx.send(Msg::Done(result, conversation, outcome));
            ctx.request_repaint();
        });
    }

    /// Takes what the background thread has to say; called every frame.
    fn assistant_poll(&mut self) {
        self.datasheet_poll();
        let st = &mut self.wb.assistant;
        if let Some(rx) = &st.key_check {
            match rx.try_recv() {
                Ok(Ok(())) => {
                    st.key_note = Some((true, "Anthropic accepted the key.".into()));
                    st.key_check = None;
                }
                Ok(Err(e)) => {
                    st.key_note = Some((false, e.to_string()));
                    st.key_check = None;
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => st.key_check = None,
            }
        }
        while let Some(rx) = &st.running {
            match rx.try_recv() {
                Ok(Msg::Event(Event::Text(t))) => {
                    st.thinking = false;
                    st.live.push_str(&t);
                }
                Ok(Msg::Event(Event::Thinking)) => st.thinking = true,
                Ok(Msg::Event(Event::Tool { name, input })) => {
                    st.flush_live();
                    if name == "set_plan" {
                        st.live_plan = plan_of(&input);
                    } else {
                        st.transcript.push(Entry::Tool(tool_phrase(&name).into()));
                    }
                }
                Ok(Msg::Event(Event::Request {
                    model,
                    usage,
                    cost_usd,
                })) => {
                    let _ = model;
                    let month = month_of((st.now)());
                    st.ledger.record(&month, &usage, cost_usd);
                    st.session_cost += cost_usd;
                    if let Some(path) = &st.ledger_path {
                        let _ = st.ledger.save(path);
                    }
                }
                Ok(Msg::Done(result, conversation, outcome)) => {
                    st.conversation = conversation;
                    st.running = None;
                    st.thinking = false;
                    match result {
                        Ok(turn) => {
                            st.live_plan.clear();
                            if let Some(o) = outcome.as_ref()
                                && !o.plan.is_empty()
                            {
                                st.transcript.push(Entry::Plan(o.plan.clone()));
                            }
                            if st.live.is_empty() {
                                if !turn.text.is_empty() {
                                    st.transcript.push(Entry::Claude(turn.text));
                                }
                            } else {
                                st.flush_live();
                            }
                            if turn.truncated {
                                st.transcript.push(Entry::Problem(
                                    "The answer was cut off at the length limit. Ask for the rest."
                                        .into(),
                                ));
                            }
                            if turn.step_limit {
                                st.transcript.push(Entry::Problem(
                                    "Claude used the most steps allowed for one question and stopped. \
                                     Ask again more narrowly."
                                        .into(),
                                ));
                            }
                            if let Some(outcome) = *outcome {
                                if let Some(q) = outcome.question.clone() {
                                    st.transcript.push(Entry::Question(q));
                                }
                                if !outcome.steps.is_empty() {
                                    // This card holds the earlier pending steps too.
                                    for e in st.transcript.iter_mut() {
                                        if let Entry::ChangeSet(c) = e
                                            && c.state == ProposalState::Pending
                                        {
                                            c.state = ProposalState::Superseded;
                                        }
                                    }
                                    st.transcript.push(Entry::ChangeSet(Box::new(Card {
                                        outcome,
                                        state: ProposalState::Pending,
                                    })));
                                }
                            }
                        }
                        Err(ApiError::Cancelled(_)) => {
                            st.flush_live();
                            st.transcript.push(Entry::Tool("Stopped".into()));
                        }
                        Err(e) => {
                            st.flush_live();
                            st.transcript.push(Entry::Problem(e.to_string()));
                        }
                    }
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    st.running = None;
                    st.thinking = false;
                    // Keep what was streamed so far instead of dropping it.
                    let partial = std::mem::take(&mut st.live);
                    if !partial.trim().is_empty() {
                        st.transcript.push(Entry::Claude(partial));
                    }
                    st.transcript
                        .push(Entry::Problem("The assistant stopped unexpectedly.".into()));
                    break;
                }
            }
        }
    }

    /// Applies or rejects the change set on card `index`. Applying is ONE
    /// undo step for all of it, and all or nothing: if any step no longer
    /// fits the project, nothing changes and the card says which.
    fn assistant_decide(&mut self, index: usize, apply: bool) {
        let Some(Entry::ChangeSet(card)) = self.wb.assistant.transcript.get(index) else {
            return;
        };
        if card.state != ProposalState::Pending {
            return;
        }
        let steps = card.outcome.steps.clone();
        let n = steps.len();
        let outcome = if !apply {
            ProposalState::Rejected
        } else if self.backend.is_read_only() || self.commissioning_locks_edits() {
            ProposalState::Failed("The project is read-only right now.".into())
        } else {
            let resources = Resources::new(user_macros());
            let label = if n == 1 {
                "Claude: ".to_string() + &steps[0].summary
            } else {
                format!("Claude: change set of {n} steps")
            };
            match self.backend.document_mut() {
                None => ProposalState::Failed("No project is open.".into()),
                Some(doc) => match commit(doc, &resources, &steps, &label) {
                    Ok(changes) => {
                        self.changed_outside_canvas(&changes);
                        ProposalState::Applied
                    }
                    Err(why) => ProposalState::Failed(why),
                },
            }
        };
        let what = if n == 1 {
            steps[0].summary.clone()
        } else {
            format!("the change set of {n} steps")
        };
        let note = match &outcome {
            ProposalState::Applied => format!("The person applied {what}"),
            ProposalState::Rejected => format!("The person rejected {what}"),
            ProposalState::Failed(why) => format!("{what} could not be applied: {why}"),
            ProposalState::Pending | ProposalState::Superseded => String::new(),
        };
        self.wb.assistant.notes.push(note);
        if outcome == ProposalState::Applied {
            let tail = if n == 1 {
                "Ctrl+Z undoes it.".to_string()
            } else {
                format!("All {n} steps are one undo step: Ctrl+Z undoes them together.")
            };
            self.message(Level::Info, format!("{what}. {tail}"));
        }
        if let Some(Entry::ChangeSet(card)) = self.wb.assistant.transcript.get_mut(index) {
            card.state = outcome;
        }
    }

    #[cfg(test)]
    pub(super) fn assistant_test_send(&mut self, ctx: &egui::Context) {
        let text = std::mem::take(&mut self.wb.assistant.draft);
        self.assistant_send(ctx, text);
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_choose(&mut self, name: &str, bytes: Vec<u8>) {
        self.wb.assistant.sheet.stage = datasheet::Stage::Confirm {
            name: name.into(),
            bytes: Arc::new(bytes),
        };
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_send(&mut self, ctx: &egui::Context) {
        self.datasheet_start(ctx);
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_stage(&self) -> &'static str {
        match self.wb.assistant.sheet.stage {
            datasheet::Stage::Idle => "idle",
            datasheet::Stage::Confirm { .. } => "confirm",
            datasheet::Stage::Reading(_) => "reading",
            datasheet::Stage::Review(_) => "review",
            datasheet::Stage::Note(_) => "note",
        }
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_add(&mut self) -> Result<String, String> {
        self.datasheet_add()
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_stop(&mut self) {
        self.datasheet_stop();
    }

    #[cfg(test)]
    pub(super) fn datasheet_test_duplicate(&self) -> bool {
        self.datasheet_duplicate()
    }

    #[cfg(test)]
    pub(super) fn assistant_test_decide(&mut self, index: usize, apply: bool) {
        self.assistant_decide(index, apply);
    }

    pub(super) fn assistant_window(&mut self, ctx: &egui::Context) {
        // Panels elsewhere (the Heat panel) cannot reach this state; they read
        // whether Claude is on and leave a question here for the next frame.
        let on = self.settings.ai.enabled;
        ctx.data_mut(|d| d.insert_temp(egui::Id::new(ON_ID), on));
        if let Some(q) = ctx.data_mut(|d| d.remove_temp::<String>(egui::Id::new(ASK_ID)))
            && on
        {
            self.wb.assistant.ask_later(q);
        }
        if !self.wb.assistant.open {
            return;
        }
        self.assistant_poll();
        match self.wb.assistant.shot {
            Shot::Idle => {}
            Shot::Hide => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
                self.wb.assistant.shot = Shot::Waiting(0);
                ctx.request_repaint();
                return;
            }
            Shot::Waiting(n) => {
                let got = ctx.input(|i| {
                    i.events.iter().find_map(|e| match e {
                        egui::Event::Screenshot { image, .. } => Some(Arc::clone(image)),
                        _ => None,
                    })
                });
                let scope_now = self.settings.ai.consent.effective();
                let st = &mut self.wb.assistant;
                match got {
                    Some(img) => {
                        match picture_of(&img) {
                            Ok(p) => {
                                st.picture = Some(p);
                                st.picture_scope = scope_now;
                            }
                            Err(e) => st
                                .transcript
                                .push(Entry::Problem(format!("Could not take the picture: {e}"))),
                        }
                        st.shot = Shot::Idle;
                    }
                    None if n > 180 => {
                        st.shot = Shot::Idle;
                        st.transcript.push(Entry::Problem(
                            "This computer's graphics did not give a picture of the window.".into(),
                        ));
                    }
                    None => {
                        st.shot = Shot::Waiting(n + 1);
                        ctx.request_repaint();
                        return;
                    }
                }
            }
        }
        if self.wb.assistant.running.is_none()
            && let Some(q) = self.wb.assistant.pending_question.take()
        {
            if self.settings.ai.enabled {
                self.assistant_send(ctx, q);
            } else {
                self.wb
                    .assistant
                    .transcript
                    .push(Entry::Problem("Claude is off, so nothing was sent.".into()));
            }
        }
        if self.wb.assistant.running.is_some() || self.wb.assistant.key_check.is_some() {
            ctx.request_repaint_after(std::time::Duration::from_millis(120));
        }
        let mut open = true;
        let mut decide: Option<(usize, bool)> = None;
        let mut send: Option<String> = None;
        // The default height is "as tall as the screen allows" (the dialog caps
        // it): settings and chat are long, so a big screen shows more of them.
        super::panel::dialog(ctx, "Ask Claude", [600.0, 4000.0])
            .open(super::panel::esc(ctx, &mut open))
            // The agree controls are pinned below; the rest scrolls above them.
            .scroll([false, false])
            .show(ctx, |ui| {
                if self.wb.assistant.show_settings {
                    egui::Panel::bottom("ask_claude_agree")
                        .resizable(false)
                        .show(ui, |ui| self.assistant_agree_ui(ui));
                }
                egui::CentralPanel::default().show(ui, |ui| {
                    // Settings scroll in their own area, at most half the
                    // dialog while a chat sits below; the chat is outside it
                    // (inside a scroll area its height would be unbounded and
                    // the message box would leave the window).
                    let room = ui.available_height();
                    let settings_max = if self.settings.ai.enabled {
                        (room * 0.5).max(160.0)
                    } else {
                        room
                    };
                    let scrolled = egui::ScrollArea::vertical()
                        .id_salt("ask_claude_settings")
                        .scroll_bar_visibility(
                            egui::scroll_area::ScrollBarVisibility::AlwaysVisible,
                        )
                        .max_height(settings_max)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            self.assistant_settings_ui(ui, ctx);
                            if self.wb.assistant.show_settings {
                                self.assistant_notice_ui(ui);
                            }
                        });
                    // The notice counts as read once its last line has been in view.
                    if let Some(r) = self.wb.assistant.notice_rect.take()
                        && r.max.y <= scrolled.inner_rect.max.y + 1.0
                        && r.max.y >= scrolled.inner_rect.min.y
                    {
                        self.wb.assistant.notice_seen = Some(CONSENT_VERSION);
                    }
                    ui.separator();
                    if self.settings.ai.enabled {
                        send = self.assistant_chat_ui(ui, &mut decide);
                    } else {
                        ui.weak("Turn Claude on above to start a conversation.");
                    }
                });
            });
        self.wb.assistant.open = open;
        if let Some((i, apply)) = decide {
            self.assistant_decide(i, apply);
        }
        if let Some(text) = send {
            self.assistant_send(ctx, text);
        }
    }

    fn assistant_settings_ui(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let pal = ui_palette();
        let enabled = self.settings.ai.enabled;
        let spent = self.wb.assistant.spent();
        let cap = self.settings.ai.monthly_cap_usd;
        let model = models::resolve(&self.settings.ai.model);
        if let Some((line, loaded)) = replay_notice() {
            ui.label(egui::RichText::new(line).color(if loaded { pal.warning } else { pal.error }));
        }
        ui.horizontal(|ui| {
            if enabled {
                ui.label(egui::RichText::new("Claude is on").color(pal.success));
                ui.weak(format!("· {}", model.label));
            } else {
                ui.label(egui::RichText::new("Claude is off").color(pal.text_secondary));
                ui.weak("· nothing is sent");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let label = if self.wb.assistant.show_settings {
                    "Hide settings"
                } else {
                    "Settings"
                };
                if ui.small_button(label).clicked() {
                    self.wb.assistant.show_settings ^= true;
                }
            });
        });
        if !self.wb.assistant.show_settings {
            return;
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(
                "Optional. Uses your own Anthropic API key and your own API credits. \
                 KoperCAD has no account with Anthropic and sees none of your usage.",
            )
            .color(pal.text_secondary),
        );
        ui.add_space(6.0);

        // 1. The key.
        ui.strong("1. Your API key");
        let key = self.wb.assistant.key();
        match &key {
            Some((k, src)) => {
                let where_from = match src {
                    KeySource::Stored => "stored on this computer",
                    KeySource::Environment => "from the ANTHROPIC_API_KEY variable",
                };
                ui.label(format!("{} ({where_from})", k.masked()));
            }
            None => {
                ui.weak("No key yet. Create one at console.anthropic.com, API keys.");
            }
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.wb.assistant.key_input)
                    .password(true)
                    .hint_text("sk-ant-…")
                    .desired_width(220.0),
            );
            if ui
                .add_enabled(
                    !self.wb.assistant.key_input.trim().is_empty(),
                    egui::Button::new("Save key"),
                )
                .clicked()
            {
                let st = &mut self.wb.assistant;
                st.key_note = Some(match ApiKey::parse(&st.key_input) {
                    Err(e) => (false, e.to_string()),
                    Ok(k) => match st.store.as_ref().map(|s| s.save(&k)) {
                        Some(Ok(())) => {
                            st.key_input.clear();
                            (true, "Saved in your user profile.".into())
                        }
                        Some(Err(e)) => (false, e.to_string()),
                        None => (false, "No place to keep the key on this computer.".into()),
                    },
                });
            }
        });
        ui.horizontal(|ui| {
            let checking = self.wb.assistant.key_check.is_some();
            if ui
                .add_enabled(key.is_some() && !checking, egui::Button::new("Check key"))
                .on_hover_text("Asks Anthropic whether the key works. Costs nothing.")
                .clicked()
                && let Some((k, _)) = key.clone()
            {
                let (tx, rx) = channel();
                let factory = Arc::clone(&self.wb.assistant.transport);
                let ctx = ctx.clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Client::new(factory(), k).check_key());
                    ctx.request_repaint();
                });
                self.wb.assistant.key_check = Some(rx);
                self.wb.assistant.key_note = None;
            }
            if checking {
                ui.spinner();
            }
            if matches!(key, Some((_, KeySource::Stored)))
                && ui.button("Remove key").clicked()
                && let Some(s) = &self.wb.assistant.store
            {
                let _ = s.remove();
                self.wb.assistant.key_note = Some((true, "Key removed from this computer.".into()));
            }
        });
        if let Some((good, note)) = &self.wb.assistant.key_note {
            ui.colored_label(if *good { pal.success } else { pal.error }, note);
        }
        ui.weak(
            "The key is saved as a file in your user profile (not encrypted, not in the \
             project file; only its last four characters are ever shown). Anyone who can read \
             your profile can read it. Remove it here when you stop using Claude.",
        );
        ui.add_space(6.0);

        // 2. What Claude may see.
        ui.strong("2. What Claude may read");
        let mut scope = self
            .wb
            .assistant
            .pending_scope
            .unwrap_or_else(|| self.settings.ai.consent.effective());
        for s in DataScope::ALL {
            ui.radio_value(&mut scope, s, s.label());
            ui.indent(("scope", s.id()), |ui| {
                ui.weak(s.describes());
            });
        }
        ui.weak(
            "Buttons that say “with Claude” send the text shown on screen, whatever you chose here.",
        );
        ui.weak("A picture you attach shows everything on screen at that moment, whatever you chose here.");
        let current = self.settings.ai.consent.effective();
        if scope != self.wb.assistant.pending_scope.unwrap_or(current) {
            if enabled && scope > current {
                // Reading more needs its own yes.
                self.wb.assistant.pending_scope = Some(scope);
            } else {
                self.wb.assistant.pending_scope = None;
                let now = (self.wb.assistant.now)();
                self.settings.ai.consent = Consent::give(scope, now);
            }
        }
        if let Some(wider) = self.wb.assistant.pending_scope {
            ui.horizontal_wrapped(|ui| {
                ui.colored_label(
                    pal.warning,
                    format!(
                        "Claude is on. Allow it to read more: \"{}\"? {}",
                        wider.label(),
                        wider.describes()
                    ),
                );
            });
            ui.horizontal(|ui| {
                if ui.button("Allow").clicked() {
                    let now = (self.wb.assistant.now)();
                    self.settings.ai.consent = Consent::give(wider, now);
                    self.wb.assistant.pending_scope = None;
                }
                if ui.button("Keep the narrower scope").clicked() {
                    self.wb.assistant.pending_scope = None;
                }
            });
        }
        ui.add_space(6.0);

        // 3. Model and spend.
        ui.strong("3. Model and budget");
        egui::ComboBox::from_id_salt("ai_model")
            .selected_text(model.label)
            .show_ui(ui, |ui| {
                for m in MODELS {
                    ui.selectable_value(&mut self.settings.ai.model, m.id.to_string(), m.label)
                        .on_hover_text(m.note);
                }
            });
        ui.weak(format!(
            "{}. List price ${:.2} in / ${:.2} out per million tokens.",
            model.note, model.input, model.output
        ));
        ui.horizontal(|ui| {
            ui.label("Thinking");
            for e in novo_ai::config::EFFORTS {
                let shown = format!("{}{}", e[..1].to_uppercase(), &e[1..]);
                ui.selectable_value(&mut self.settings.ai.effort, e.to_string(), shown)
                    .on_hover_text("More thinking is slower and costs more.");
            }
        });
        ui.horizontal(|ui| {
            ui.label("Monthly limit (USD)");
            ui.add(
                egui::DragValue::new(&mut self.settings.ai.monthly_cap_usd)
                    .range(0.0..=10_000.0)
                    .speed(0.5)
                    .prefix("$"),
            );
            ui.weak("0 = no limit");
        });
        ui.label(format!(
            "This month: {} of {} · {} requests",
            format_usd(spent),
            if cap > 0.0 {
                format_usd(cap)
            } else {
                "no limit".into()
            },
            self.wb.assistant.ledger.requests
        ));
        ui.weak(
            "Costs are estimates from Anthropic's list prices. Your Anthropic account shows \
             the real bill.",
        );
        ui.add_space(6.0);
    }

    /// Section 4: the notice, at the end of the scrolling settings so it is
    /// read in full whatever the window height.
    fn assistant_notice_ui(&mut self, ui: &mut egui::Ui) {
        let pal = ui_palette();
        ui.add_space(6.0);
        ui.strong("4. Agree");
        let text = ui.label(egui::RichText::new(NOTICE).color(pal.text_secondary));
        self.wb.assistant.notice_rect = Some(text.rect);
    }

    /// The box and the button, pinned under the scrolling settings so they are
    /// reachable on a small window; the notice they refer to is above.
    fn assistant_agree_ui(&mut self, ui: &mut egui::Ui) {
        let enabled = self.settings.ai.enabled;
        let key = self.wb.assistant.key();
        // Sections 1 to 3 have already stored a change of scope.
        let scope = self
            .wb
            .assistant
            .pending_scope
            .unwrap_or_else(|| self.settings.ai.consent.effective());
        let seen = self.wb.assistant.notice_seen == Some(CONSENT_VERSION);
        if !seen && !enabled {
            ui.weak("Scroll down and read “4. Agree” to be able to agree.");
        }
        if enabled {
            if ui.button("Turn Claude off").clicked() {
                self.settings.ai.enabled = false;
                // A fresh start when it is turned on again.
                self.wb.assistant.clear_chat();
                self.wb.assistant.conv_scope = DataScope::None;
                self.wb.assistant.pending_scope = None;
                self.wb.assistant.notice_seen = None;
            }
        } else {
            ui.add_enabled(
                seen,
                egui::Checkbox::new(&mut self.wb.assistant.agreed, "I have read this and agree"),
            )
            .on_disabled_hover_text("Scroll down and read “4. Agree” first.");
            let ready = seen && self.wb.assistant.agreed && key.is_some();
            self.wb.assistant.turn_on_ready = ready;
            if ui
                .add_enabled(ready, egui::Button::new("Turn Claude on"))
                .on_disabled_hover_text("Read “4. Agree”, add a key and tick the box first.")
                .clicked()
            {
                let now = (self.wb.assistant.now)();
                self.settings.ai.enabled = true;
                self.settings.ai.consent = Consent::give(scope, now);
                self.wb.assistant.show_settings = false;
            }
        }
    }

    fn assistant_chat_ui(
        &mut self,
        ui: &mut egui::Ui,
        decide: &mut Option<(usize, bool)>,
    ) -> Option<String> {
        let pal = ui_palette();
        let running = self.wb.assistant.running.is_some();
        let mut answer: Option<String> = None;
        let height = (ui.available_height() - 140.0).max(120.0);
        egui::ScrollArea::vertical()
            .max_height(height)
            .min_scrolled_height(height)
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.wb.assistant.transcript.is_empty() {
                    ui.weak(
                        "Ask about your project, for example “Which devices have no part \
                         assigned?” or “Check my tags against IEC 81346.” Claude can \
                         suggest edits; you apply or reject each one.",
                    );
                }
                for (i, entry) in self.wb.assistant.transcript.iter().enumerate() {
                    match entry {
                        Entry::User(t) => {
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("You").strong().color(pal.accent));
                            ui.label(t);
                        }
                        Entry::Claude(t) => {
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Claude").strong());
                                if ui
                                    .small_button("Copy")
                                    .on_hover_text("Copy this answer")
                                    .clicked()
                                {
                                    ui.ctx().copy_text(t.clone());
                                }
                            });
                            ui.label(t);
                        }
                        Entry::Tool(t) => {
                            ui.weak(format!("· {t}"));
                        }
                        Entry::Problem(t) => {
                            ui.colored_label(pal.error, t);
                        }
                        Entry::ChangeSet(card) => change_set_ui(ui, pal, i, card, decide),
                        Entry::Plan(items) => plan_ui(ui, pal, items),
                        Entry::Question(q) => {
                            let open = !running
                                && self.wb.assistant.transcript[i + 1..]
                                    .iter()
                                    .all(|e| !matches!(e, Entry::User(_)));
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.label(egui::RichText::new("Claude asks").strong());
                                ui.label(&q.text);
                                if open {
                                    ui.horizontal_wrapped(|ui| {
                                        for o in &q.options {
                                            if ui.button(o).clicked() {
                                                answer = Some(o.clone());
                                            }
                                        }
                                    });
                                    ui.weak("Or type your answer below.");
                                }
                            });
                        }
                    }
                }
                if running {
                    let st = &self.wb.assistant;
                    if !st.live_plan.is_empty() {
                        plan_ui(ui, pal, &st.live_plan);
                    }
                    if !st.live.is_empty() {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Claude").strong());
                        ui.label(&st.live);
                    }
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.weak(if st.thinking || st.live.is_empty() {
                            "Claude is thinking…"
                        } else {
                            "Claude is writing…"
                        });
                        if ui.small_button("Stop").clicked() {
                            st.cancel.store(true, Ordering::Relaxed);
                        }
                    });
                }
            });
        ui.add_space(4.0);
        self.datasheet_ui(ui);
        ui.add_space(4.0);
        let mut send = None;
        let st = &mut self.wb.assistant;
        let edit = ui.add_enabled(
            !running,
            egui::TextEdit::multiline(&mut st.draft)
                .desired_rows(3)
                .desired_width(f32::INFINITY)
                .hint_text("Ask Claude…  (Ctrl+Enter sends)"),
        );
        let ctrl_enter = edit.has_focus()
            && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command);
        ui.horizontal(|ui| {
            let can = !running && !st.draft.trim().is_empty();
            if (ui.add_enabled(can, egui::Button::new("Send")).clicked() || (ctrl_enter && can))
                && can
            {
                send = Some(std::mem::take(&mut st.draft).trim().to_string());
            }
            if ui.small_button("Clear chat").clicked() && !running {
                st.clear_chat();
            }
            match &st.picture {
                Some(p) => {
                    ui.weak(format!("Picture attached ({}×{})", p.size[0], p.size[1]));
                    if ui.small_button("Remove picture").clicked() {
                        st.picture = None;
                    }
                }
                None => {
                    if ui
                        .add_enabled(
                            !running && st.shot == Shot::Idle,
                            egui::Button::new("Attach picture").small(),
                        )
                        .on_hover_text(
                            "Sends a picture of this whole KoperCAD window with your next \
                             message: everything visible in it, whatever the data scope. \
                             It goes with that one question only; Claude is not shown it again. \
                             This window steps aside for a moment.",
                        )
                        .clicked()
                    {
                        st.shot = Shot::Hide;
                    }
                }
            }
            ui.weak(format!(
                "Chat so far {} · month {}",
                format_usd(st.session_cost),
                format_usd(st.spent())
            ));
        });
        send.or(answer)
    }
}

/// The macros the person saved themselves, for Claude to place as well.
fn user_macros() -> Vec<novo_parts::MacroFile> {
    let Some(library) = novo_parts::MacroLibrary::user() else {
        return Vec::new();
    };
    let (entries, _) = library.list();
    entries
        .iter()
        .filter_map(|e| library.load(&e.path).ok())
        .collect()
}

/// The checklist in a `set_plan` call.
fn plan_of(input: &Value) -> Vec<PlanItem> {
    input["steps"]
        .as_array()
        .map(|a| {
            a.iter()
                .take(30)
                .filter_map(|i| {
                    Some(PlanItem {
                        title: i["title"].as_str()?.chars().take(160).collect(),
                        status: match i["status"].as_str() {
                            Some(s @ ("doing" | "done" | "skipped")) => s.to_string(),
                            _ => "todo".into(),
                        },
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn tool_phrase(name: &str) -> &'static str {
    match name {
        "project_outline" => "Looked at the project outline",
        "selection" => "Looked at your selection",
        "read_project" => "Read part of the project",
        "propose_edit" => "Made an edit in the draft",
        "run_checks" => "Ran the design checks on the draft",
        "standards_report" => "Looked at the standards report",
        "read_report" => "Read a report",
        "list_macros" => "Looked at the ready-made circuits",
        "list_symbols" => "Searched the symbol library",
        "ask_user" => "Asked you a question",
        _ => "Used a tool",
    }
}

/// The checklist Claude is working through.
fn plan_ui(ui: &mut egui::Ui, pal: &novo_theme::Palette, items: &[PlanItem]) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.label(egui::RichText::new("Plan").strong());
        for item in items {
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(14.0, 14.0), egui::Sense::hover());
                let c = rect.center();
                let p = ui.painter();
                match item.status.as_str() {
                    "done" => {
                        p.circle_filled(c, 5.0, pal.success);
                    }
                    "doing" => {
                        p.circle_stroke(c, 5.0, egui::Stroke::new(2.0, pal.accent));
                        p.circle_filled(c, 2.0, pal.accent);
                    }
                    "skipped" => {
                        p.line_segment(
                            [c - egui::vec2(4.0, 0.0), c + egui::vec2(4.0, 0.0)],
                            egui::Stroke::new(1.5, pal.text_secondary),
                        );
                    }
                    _ => {
                        p.circle_stroke(c, 5.0, egui::Stroke::new(1.0, pal.text_secondary));
                    }
                }
                let text = egui::RichText::new(format!("{}  ({})", item.title, item.status));
                ui.label(match item.status.as_str() {
                    "done" | "skipped" => text.weak(),
                    "doing" => text.strong(),
                    _ => text,
                });
            });
        }
    });
}

/// The change set Claude built: what it does, what the program's own checks
/// say about it, and Apply or Reject.
fn change_set_ui(
    ui: &mut egui::Ui,
    pal: &novo_theme::Palette,
    index: usize,
    card: &Card,
    decide: &mut Option<(usize, bool)>,
) {
    let o = &card.outcome;
    let n = o.steps.len();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        if n == 1 {
            ui.label(egui::RichText::new("Suggested change").strong());
            ui.label(&o.steps[0].summary);
            if let Some(note) = &o.steps[0].note {
                ui.weak(note);
            }
        } else {
            ui.label(egui::RichText::new(format!("Change set: {n} steps in a draft")).strong());
            ui.weak("Nothing is in your project yet.");
            egui::CollapsingHeader::new(format!("The {n} steps"))
                .id_salt(("assistant_steps", index))
                .default_open(false)
                .show(ui, |ui| {
                    for (k, s) in o.steps.iter().enumerate() {
                        ui.label(format!("{}. {}", k + 1, s.summary));
                        if let Some(note) = &s.note {
                            ui.weak(note);
                        }
                    }
                });
        }
        for line in o.diff.lines() {
            if line.starts_with("Removed devices") {
                // Deleting is the one change worth a second look: say it in colour and bold.
                ui.label(egui::RichText::new(line).strong().color(pal.warning));
            } else {
                ui.label(line);
            }
        }
        if !o.dropped.is_empty() {
            ui.colored_label(
                pal.warning,
                format!(
                    "{} earlier {} no longer fit and were left out: {}",
                    o.dropped.len(),
                    if o.dropped.len() == 1 { "step" } else { "steps" },
                    o.dropped.join("; ")
                ),
            );
        }
        let c = &o.checks;
        ui.add_space(2.0);
        ui.label(egui::RichText::new("Design checks, run by the program").small().strong());
        let colour = if c.after.errors > 0 {
            pal.error
        } else if c.after.warnings > 0 {
            pal.warning
        } else {
            pal.success
        };
        ui.horizontal_wrapped(|ui| {
            ui.weak(format!("Before: {}", c.before.text()));
            ui.colored_label(colour, format!("After: {}", c.after.text()));
        });
        for l in c.new.iter().filter(|l| l.severity != "info").take(5) {
            ui.colored_label(pal.warning, format!("New {}: {}", l.severity, l.message));
        }
        for l in c.resolved.iter().filter(|l| l.severity != "info").take(5) {
            ui.colored_label(pal.success, format!("Resolved: {}", l.message));
        }
        if n > 0 && !o.verified {
            ui.colored_label(
                pal.warning,
                "Claude did not run the checks after its last edit; the numbers above are the program's own.",
            );
        }
        ui.weak(
            "The checks find drawing problems. They do not show that the design is right or \
             that it meets a standard.",
        );
        match &card.state {
            ProposalState::Pending => {
                ui.horizontal(|ui| {
                    let label = if n == 1 { "Apply" } else { "Apply all (one undo step)" };
                    if ui.button(label).clicked() {
                        *decide = Some((index, true));
                    }
                    if ui.button("Reject").clicked() {
                        *decide = Some((index, false));
                    }
                });
            }
            ProposalState::Applied => {
                ui.colored_label(
                    pal.success,
                    if n == 1 {
                        "Applied. Ctrl+Z undoes it.".to_string()
                    } else {
                        format!("Applied: all {n} steps are one undo step. Ctrl+Z undoes them together.")
                    },
                );
            }
            ProposalState::Rejected => {
                ui.weak("Rejected.");
            }
            ProposalState::Superseded => {
                ui.weak("Replaced by the change set below, which includes these steps.");
            }
            ProposalState::Failed(why) => {
                ui.colored_label(pal.error, why);
            }
        }
    });
}
