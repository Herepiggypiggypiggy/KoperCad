//! The Claude assistant window (Help > Ask Claude, or Ctrl+K "Ask Claude").
//!
//! Off until the person turns it on in the window itself: they paste their
//! own Anthropic API key, choose how much of the project Claude may read and
//! agree to the notice. Messages run on a background thread; the window only
//! polls a channel. Claude never edits the project: its suggestions arrive as
//! cards with Apply and Reject, and Apply goes through the same command path
//! as the editor, so one Ctrl+Z undoes it. The engine is `novo-ai`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{SystemTime, UNIX_EPOCH};

use eframe::egui;
use novo_ai::agent::{Conversation, Event, TurnResult};
use novo_ai::api::{ApiError, Client, Transport, UreqTransport};
use novo_ai::consent::{Consent, DataScope, NOTICE};
use novo_ai::key::{ApiKey, KeySource, KeyStore};
use novo_ai::models::{self, MODELS};
use novo_ai::tools::{EditAction, Proposal, Snapshot, SnapshotHost};
use novo_ai::usage::{Ledger, format_usd, month_of};
use novo_core::{Command, DeviceTag, Project};
use serde_json::{Value, json};

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
    Proposal(Proposal, ProposalState),
}

enum Msg {
    Event(Event),
    Done(Result<TurnResult, ApiError>, Conversation),
}

/// Everything the window remembers while the app runs.
pub(super) struct State {
    pub open: bool,
    proposal_notes: HashMap<u32, String>,
    show_settings: bool,
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
    next_proposal: u32,
    store: Option<KeyStore>,
    transport: TransportFactory,
    /// Set by tests: the clock for the month and consent time.
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

impl State {
    pub fn new() -> Self {
        Self::with(
            KeyStore::in_profile(),
            novo_ai::key::default_dir().map(|d| d.join("usage.json")),
            Arc::new(|| Box::new(UreqTransport::new())),
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
            proposal_notes: HashMap::new(),
            show_settings: false,
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
            next_proposal: 1,
            store,
            transport,
            now,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(store: KeyStore, ledger: PathBuf, transport: TransportFactory) -> Self {
        Self::with(
            Some(store),
            Some(ledger),
            transport,
            Arc::new(|| 1_792_000_000),
        )
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
            .position(|e| matches!(e, Entry::Proposal(..)))
    }

    #[cfg(test)]
    pub(super) fn transcript_text(&self) -> String {
        self.transcript
            .iter()
            .map(|e| match e {
                Entry::User(t) | Entry::Claude(t) | Entry::Tool(t) | Entry::Problem(t) => t.clone(),
                Entry::Proposal(p, _) => p.summary.clone(),
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

    fn spent(&self) -> f64 {
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
        self.proposal_notes.clear();
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

/// The edits Claude may suggest. Each maps to one model command.
fn edit_actions() -> Vec<EditAction> {
    vec![
        EditAction {
            name: "rename_device",
            description: "Gives a device a new tag everywhere it is used.",
            schema: json!({"type": "object", "properties": {
                "device_id": {"type": "integer", "description": "id from the project data"},
                "tag": {"type": "string", "description": "IEC 81346 tag such as -K7 or =A1 +B2 -K7"}},
                "required": ["device_id", "tag"]}),
        },
        EditAction {
            name: "rename_page",
            description: "Renames a page (references print the page name).",
            schema: json!({"type": "object", "properties": {
                "page_id": {"type": "integer"}, "name": {"type": "string"}},
                "required": ["page_id", "name"]}),
        },
        EditAction {
            name: "assign_part",
            description: "Assigns a part from the project's parts (section \"parts\") to a device. \
                          Only available when the person agreed to share the whole project.",
            schema: json!({"type": "object", "properties": {
                "device_id": {"type": "integer"}, "part_id": {"type": "integer"}},
                "required": ["device_id", "part_id"]}),
        },
        EditAction {
            name: "set_property",
            description: "Sets one named property of a device (text, number or true/false).",
            schema: json!({"type": "object", "properties": {
                "device_id": {"type": "integer"},
                "key": {"type": "string"},
                "value": {"type": ["string", "number", "boolean"]}},
                "required": ["device_id", "key", "value"]}),
        },
        EditAction {
            name: "add_page",
            description: "Adds an empty schematic page at the end.",
            schema: json!({"type": "object", "properties": {
                "name": {"type": "string"}, "title": {"type": "string"}},
                "required": ["name"]}),
        },
        EditAction {
            name: "add_device",
            description: "Adds a device with a tag and description (not drawn on a page yet).",
            schema: json!({"type": "object", "properties": {
                "tag": {"type": "string"}, "description": {"type": "string"}},
                "required": ["tag"]}),
        },
    ]
}

const MAX_KEY_CHARS: usize = 60;
pub(super) const MAX_VALUE_CHARS: usize = 500;

/// What a proposal would replace, for the card only. It is never sent back to
/// the model: old values and part names can be above the agreed scope.
pub(super) fn proposal_note(project: &Project, p: &Proposal) -> Option<String> {
    let device = project.device(p.args["device_id"].as_u64()?.into())?;
    match p.action.as_str() {
        "set_property" => {
            let key = p.args["key"].as_str()?.trim();
            Some(match device.properties.get(key) {
                Some(old) => format!("Replaces the current value: {old}"),
                None => "There is no value yet.".into(),
            })
        }
        "assign_part" if !device.parts.is_empty() => Some(format!(
            "This device already has {} part(s) assigned.",
            device.parts.len()
        )),
        _ => None,
    }
}

/// What a proposal changes in the open project, by id, so the summary can
/// name the current tag and the check can refuse an id that is not there.
#[derive(Clone, Default)]
pub(super) struct Known {
    devices: HashMap<u64, String>,
    pages: HashMap<u64, String>,
    parts: HashMap<u64, String>,
}

pub(super) fn known_of(p: &Project) -> Known {
    Known {
        devices: p
            .devices()
            .map(|d| (d.id.raw(), d.tag.to_string()))
            .collect(),
        pages: p.pages().map(|g| (g.id.raw(), g.name.clone())).collect(),
        parts: p
            .parts()
            .map(|x| {
                (
                    x.id.raw(),
                    format!("{} {}", x.manufacturer, x.order_number)
                        .trim()
                        .to_string(),
                )
            })
            .collect(),
    }
}

/// Turns a proposal into the model command, and its one-line summary.
pub(super) fn proposal_command(
    known: &Known,
    action: &str,
    args: &Value,
) -> Result<(Command, String), String> {
    let id = |key: &str| {
        args[key]
            .as_u64()
            .ok_or_else(|| format!("{key} must be a whole number id from the project data"))
    };
    let text = |key: &str| {
        args[key]
            .as_str()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .ok_or_else(|| format!("{key} is missing"))
    };
    match action {
        "rename_device" => {
            let device = id("device_id")?;
            let old = known
                .devices
                .get(&device)
                .ok_or_else(|| format!("there is no device with id {device}"))?;
            let tag = text("tag")?;
            let parsed = DeviceTag::parse(tag).map_err(|e| format!("tag {tag:?}: {e}"))?;
            Ok((
                Command::SetTag {
                    device: device.into(),
                    tag: parsed,
                },
                format!("Rename device {old} to {tag}"),
            ))
        }
        "rename_page" => {
            let page = id("page_id")?;
            let old = known
                .pages
                .get(&page)
                .ok_or_else(|| format!("there is no page with id {page}"))?;
            let name = text("name")?;
            Ok((
                Command::RenamePage {
                    page: page.into(),
                    name: name.to_string(),
                },
                format!("Rename page {old} to {name}"),
            ))
        }
        "assign_part" => {
            let device = id("device_id")?;
            let part = id("part_id")?;
            let tag = known
                .devices
                .get(&device)
                .ok_or_else(|| format!("there is no device with id {device}"))?;
            let name = known
                .parts
                .get(&part)
                .ok_or_else(|| format!("there is no part with id {part} (see section parts)"))?;
            Ok((
                Command::AssignPart {
                    device: device.into(),
                    part: part.into(),
                },
                format!("Assign part {name} to {tag}"),
            ))
        }
        "set_property" => {
            let device = id("device_id")?;
            let tag = known
                .devices
                .get(&device)
                .ok_or_else(|| format!("there is no device with id {device}"))?;
            let key = text("key")?;
            if key.chars().count() > MAX_KEY_CHARS {
                return Err(format!("key is longer than {MAX_KEY_CHARS} characters"));
            }
            let (value, shown) = match &args["value"] {
                Value::String(s) if s.chars().count() > MAX_VALUE_CHARS => {
                    return Err(format!("value is longer than {MAX_VALUE_CHARS} characters"));
                }
                Value::String(s) => (novo_core::Value::Text(s.clone()), format!("{s:?}")),
                Value::Bool(b) => (novo_core::Value::Bool(*b), b.to_string()),
                Value::Number(n) => match n.as_i64() {
                    Some(i) => (novo_core::Value::Int(i), i.to_string()),
                    None => {
                        let f = n.as_f64().unwrap_or_default();
                        (novo_core::Value::Float(f), f.to_string())
                    }
                },
                _ => return Err("value must be text, a number or true/false".into()),
            };
            Ok((
                Command::SetProperty {
                    target: novo_core::AnyId::Device(device.into()),
                    key: key.to_string(),
                    value: Some(value),
                },
                format!("Set {key} of {tag} to {shown}"),
            ))
        }
        "add_page" => {
            let name = text("name")?;
            if known.pages.values().any(|n| n == name) {
                return Err(format!("a page {name} already exists"));
            }
            let title = args["title"]
                .as_str()
                .unwrap_or_default()
                .trim()
                .to_string();
            let shown = if title.is_empty() {
                format!("Add page {name}")
            } else {
                format!("Add page {name} ({title})")
            };
            Ok((
                Command::AddPage {
                    name: name.to_string(),
                    title,
                    kind: Default::default(),
                    format: Default::default(),
                    structure: Default::default(),
                },
                shown,
            ))
        }
        "add_device" => {
            let tag = text("tag")?;
            let parsed = DeviceTag::parse(tag).map_err(|e| format!("tag {tag:?}: {e}"))?;
            if known.devices.values().any(|t| t == tag) {
                return Err(format!("a device {tag} already exists"));
            }
            let description = args["description"].as_str().unwrap_or_default().to_string();
            let shown = if description.is_empty() {
                format!("Add device {tag}")
            } else {
                format!("Add device {tag} ({description})")
            };
            Ok((
                Command::AddDevice {
                    tag: parsed,
                    description,
                    kind: Default::default(),
                },
                shown,
            ))
        }
        other => Err(format!("unknown action {other}")),
    }
}

/// The project as data, sized by what the person allowed.
pub(super) fn snapshot_of(p: &Project, selected: &[u64], scope: DataScope) -> Snapshot {
    let mut snap = Snapshot::default();
    if scope == DataScope::None {
        return snap;
    }
    let devices = novo_py::devices_json(p);
    let tags: Vec<Value> = devices
        .as_array()
        .map(|a| {
            a.iter()
                .map(|d| json!({"id": d["id"], "tag": d["tag"]}))
                .collect()
        })
        .unwrap_or_default();
    snap.outline = json!({
        "project": {
            "name": p.info().name,
            "pages": p.pages().count(),
            "devices": p.devices().count(),
            "wires": p.wires().count(),
            "terminals": p.terminals().count(),
            "cables": p.cables().count(),
            "parts": p.parts().count(),
        },
        "pages": novo_py::pages_json(p),
        "devices": tags,
        "sections_for_read_project": ["pages", "devices", "wires", "nets", "potentials", "terminals", "parts", "problems", "wire_numbers"],
    });
    if scope >= DataScope::Selection {
        let items: Vec<Value> = p
            .symbols()
            .filter(|s| selected.contains(&s.id.raw()))
            .map(|s| {
                let device = match &s.role {
                    novo_core::SymbolRole::Device { device, .. } => p.device(*device),
                    _ => None,
                };
                json!({
                    "symbol_id": s.id.raw(),
                    "symbol": s.symbol.0,
                    "page_id": s.page.raw(),
                    "at_mm": [s.placement.origin.x, s.placement.origin.y],
                    "device": device.map(|d| json!({
                        "id": d.id.raw(), "tag": d.tag.to_string(),
                        "description": d.description})),
                })
            })
            .collect();
        snap.selection = json!({"selected_symbols": items});
    }
    if scope >= DataScope::Project {
        let mut terminals: Vec<_> = p.terminals().collect();
        terminals.sort_by_key(|t| t.id);
        let terminals: Vec<Value> = terminals
            .into_iter()
            .map(|t| {
                json!({"id": t.id.raw(), "strip": t.strip.raw(), "name": t.name,
                       "designation": p.terminal_designation(t.id)})
            })
            .collect();
        snap.project = json!({
            "pages": novo_py::pages_json(p),
            "devices": devices,
            "wires": novo_py::wires_json(p),
            "nets": novo_py::nets_json(p),
            "potentials": novo_py::potentials_json(p),
            "terminals": terminals,
            "parts": p.parts().map(|x| json!({
                "id": x.id.raw(), "manufacturer": x.manufacturer,
                "order_number": x.order_number, "type_number": x.type_number,
                "description": x.description})).collect::<Vec<_>>(),
            // The design checks, read-only: what the Issues list would show.
            "problems": novo_checks::check(p).iter().map(|q| json!({
                "kind": format!("{:?}", q.kind),
                "severity": format!("{:?}", q.severity),
                "message": q.message,
                "page_id": q.page.map(|g| g.raw()),
                "objects": q.at.len(),
                "rule": q.rule})).collect::<Vec<_>>(),
            // Wire-number conflicts against the project's own scheme, read only.
            "wire_numbers": novo_numbering::check(p, &novo_numbering::Scheme::load(p))
                .iter()
                .map(|c| json!({
                    "kind": format!("{:?}", c.kind),
                    "severity": format!("{:?}", c.severity),
                    "number": c.number,
                    "wires": c.wires.iter().map(|w| w.raw()).collect::<Vec<_>>(),
                    "message": c.message}))
                .collect::<Vec<_>>(),
        });
    }
    snap
}

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
        let (snapshot, known) = match self.backend.document() {
            Some(doc) => (
                snapshot_of(doc.project(), &self.selected_ids(), scope),
                known_of(doc.project()),
            ),
            None => (Snapshot::default(), Known::default()),
        };
        let st = &mut self.wb.assistant;
        st.enforce_scope(scope);
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
        let first = st.next_proposal;
        let (tx, rx) = channel();
        st.running = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let mut client = Client::new(factory(), key);
            let validator_known = known;
            let mut host = SnapshotHost::new(
                snapshot,
                scope,
                edit_actions(),
                Box::new(move |action, args| {
                    // The answer goes back to the model, so it must not reveal
                    // more than the agreed scope (part names are Whole project).
                    if action == "assign_part" && scope != DataScope::Project {
                        return Err("assign_part is not allowed above the agreed scope".into());
                    }
                    proposal_command(&validator_known, action, args).map(|(_, summary)| summary)
                }),
            )
            .starting_at(first);
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
            let _ = tx.send(Msg::Done(result, conversation));
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
                Ok(Msg::Event(Event::Tool { name, .. })) => {
                    st.flush_live();
                    st.transcript.push(Entry::Tool(tool_phrase(&name).into()));
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
                Ok(Msg::Done(result, conversation)) => {
                    st.conversation = conversation;
                    st.running = None;
                    st.thinking = false;
                    match result {
                        Ok(turn) => {
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
                            for p in turn.proposals {
                                st.next_proposal = st.next_proposal.max(p.id + 1);
                                if let Some(doc) = self.backend.document()
                                    && let Some(note) = proposal_note(doc.project(), &p)
                                {
                                    st.proposal_notes.insert(p.id, note);
                                }
                                st.transcript
                                    .push(Entry::Proposal(p, ProposalState::Pending));
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

    fn assistant_decide(&mut self, index: usize, apply: bool) {
        let known = self
            .backend
            .document()
            .map(|d| known_of(d.project()))
            .unwrap_or_default();
        let Some(Entry::Proposal(p, _)) = self.wb.assistant.transcript.get(index) else {
            return;
        };
        let p = p.clone();
        let outcome = if !apply {
            ProposalState::Rejected
        } else if self.backend.is_read_only() || self.commissioning_locks_edits() {
            ProposalState::Failed("The project is read-only right now.".into())
        } else {
            match proposal_command(&known, &p.action, &p.args) {
                Err(why) => ProposalState::Failed(why),
                Ok((command, _)) => match self.execute(command) {
                    Some(Ok(_)) => ProposalState::Applied,
                    Some(Err(e)) => ProposalState::Failed(e.to_string()),
                    None => ProposalState::Failed("No project is open.".into()),
                },
            }
        };
        let st = &mut self.wb.assistant;
        let note = match &outcome {
            ProposalState::Applied => {
                format!("The person applied proposal {}: {}", p.id, p.summary)
            }
            ProposalState::Rejected => {
                format!("The person rejected proposal {}: {}", p.id, p.summary)
            }
            ProposalState::Failed(why) => {
                format!("Proposal {} could not be applied: {why}", p.id)
            }
            ProposalState::Pending => String::new(),
        };
        st.notes.push(note);
        if outcome == ProposalState::Applied {
            self.message(Level::Info, format!("{}. Ctrl+Z undoes it.", p.summary));
        }
        if let Some(Entry::Proposal(_, state)) = self.wb.assistant.transcript.get_mut(index) {
            *state = outcome;
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
        super::panel::dialog(ctx, "Ask Claude", [460.0, 700.0])
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
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            self.assistant_settings_ui(ui, ctx);
                            ui.separator();
                            if self.settings.ai.enabled {
                                send = self.assistant_chat_ui(ui, &mut decide);
                            } else {
                                ui.weak("Turn Claude on above to start a conversation.");
                            }
                        });
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

    /// Section 4 of the settings, pinned under the scrolling sections 1 to 3
    /// so the box and the button are reachable on a small window.
    fn assistant_agree_ui(&mut self, ui: &mut egui::Ui) {
        let pal = ui_palette();
        let enabled = self.settings.ai.enabled;
        let key = self.wb.assistant.key();
        // Sections 1 to 3 have already stored a change of scope.
        let scope = self
            .wb
            .assistant
            .pending_scope
            .unwrap_or_else(|| self.settings.ai.consent.effective());
        ui.strong("4. Agree");
        ui.label(egui::RichText::new(NOTICE).color(pal.text_secondary));
        if enabled {
            if ui.button("Turn Claude off").clicked() {
                self.settings.ai.enabled = false;
                // A fresh start when it is turned on again.
                self.wb.assistant.clear_chat();
                self.wb.assistant.conv_scope = DataScope::None;
                self.wb.assistant.pending_scope = None;
            }
        } else {
            ui.checkbox(&mut self.wb.assistant.agreed, "I have read this and agree");
            let ready = self.wb.assistant.agreed && key.is_some();
            if ui
                .add_enabled(ready, egui::Button::new("Turn Claude on"))
                .on_disabled_hover_text("Add a key and tick the box first.")
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
                        Entry::Proposal(p, state) => {
                            egui::Frame::group(ui.style()).show(ui, |ui| {
                                ui.label(egui::RichText::new("Suggested change").strong());
                                ui.label(&p.summary);
                                if let Some(note) = self.wb.assistant.proposal_notes.get(&p.id) {
                                    ui.weak(note);
                                }
                                match state {
                                    ProposalState::Pending => {
                                        ui.horizontal(|ui| {
                                            if ui.button("Apply").clicked() {
                                                *decide = Some((i, true));
                                            }
                                            if ui.button("Reject").clicked() {
                                                *decide = Some((i, false));
                                            }
                                        });
                                    }
                                    ProposalState::Applied => {
                                        ui.colored_label(pal.success, "Applied. Ctrl+Z undoes it.");
                                    }
                                    ProposalState::Rejected => {
                                        ui.weak("Rejected.");
                                    }
                                    ProposalState::Failed(why) => {
                                        ui.colored_label(pal.error, why);
                                    }
                                }
                            });
                        }
                    }
                }
                if running {
                    let st = &self.wb.assistant;
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
        send
    }
}

fn tool_phrase(name: &str) -> &'static str {
    match name {
        "project_outline" => "Looked at the project outline",
        "selection" => "Looked at your selection",
        "read_project" => "Read part of the project",
        "propose_edit" => "Prepared a suggested change",
        _ => "Used a tool",
    }
}
