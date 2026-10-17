//! Control mode (`kopercad --control`): lets a test user that cannot hold a
//! mouse, such as an AI assistant, use the real window the way a person does.
//!
//! The app opens its normal window and also listens on a loopback TCP port
//! (127.0.0.1 only, never a network address; no admin rights). Each line in is
//! one JSON request, each line out one JSON reply. Input goes through egui's
//! own input path (pointer, wheel, keys, text), so it behaves like a real
//! mouse and keyboard, on every platform, with no OS-level automation.
//!
//! Requests (`{"id":1,"cmd":"click","x":100,"y":80}`; coordinates are window
//! points, the same units `ui_tree` and `info` report):
//!
//! - `info`: window size, scale, frame count.
//! - `screenshot` `{path}`: the window as a PNG; replies with the path and size.
//! - `ui_tree`: every labelled widget with role, label, value, state and rect
//!   (egui's AccessKit tree).
//! - `move` `{x,y}`, `click` `{x,y,button?,count?,modifiers?}`,
//!   `drag` `{from:[x,y],to:[x,y],button?,steps?}`, `scroll` `{x,y,dx?,dy?}`.
//! - `key` `{keys}`, for example `"Enter"` or `"ctrl+shift+k"`; `text` `{text}`.
//! - `wait` `{frames?}`, `resize` `{w,h}`, `quit`.
//!
//! Every input command ends with a few settle frames before it replies, so a
//! screenshot right after reflects the result.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, Sender, channel};

use eframe::egui::{
    self, Context, Event, FullOutput, Modifiers, PointerButton, Pos2, RawInput, ViewportCommand,
    accesskit,
};
use serde_json::{Value, json};

/// Frames run after an input before the reply, for animations and layout to settle.
const SETTLE_FRAMES: u32 = 4;

/// Whether `--control` was given, and on which port (0 picks a free one).
pub fn requested_port() -> Option<u16> {
    std::env::args().find_map(|a| {
        if a == "--control" {
            Some(0)
        } else {
            a.strip_prefix("--control=")?.parse().ok()
        }
    })
}

/// True for the flags this module reads, so the startup-file lookup skips them.
pub fn is_control_arg(arg: &std::ffi::OsStr) -> bool {
    arg.to_str().is_some_and(|a| a.starts_with("--control"))
}

/// Starts the listener and registers the plugin on `ctx`.
pub fn install(ctx: &Context, port: u16) {
    let listener = match TcpListener::bind(("127.0.0.1", port)) {
        Ok(l) => l,
        Err(e) => {
            log::error!("control mode: cannot listen: {e}");
            return;
        }
    };
    let bound = listener.local_addr().map(|a| a.port()).unwrap_or(port);
    println!("KOPERCAD_CONTROL_PORT={bound}");
    log::info!("control mode: listening on 127.0.0.1:{bound}");
    if let Some(file) = std::env::var_os("KOPERCAD_CONTROL_PORT_FILE") {
        let _ = std::fs::write(file, bound.to_string());
    }
    let (tx, rx) = channel::<Request>();
    let wake = ctx.clone();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            serve(stream, &tx, &wake);
        }
    });
    ctx.enable_accesskit();
    ctx.add_plugin(Control::with_rx(rx));
}

/// One client at a time: read lines, hand each to the UI thread, write the reply.
fn serve(stream: TcpStream, tx: &Sender<Request>, wake: &Context) {
    let Ok(mut out) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let (reply_tx, reply_rx) = channel();
        let body: Result<Value, _> = serde_json::from_str(&line);
        let reply = match body {
            Err(e) => json!({"ok": false, "error": format!("not JSON: {e}")}),
            Ok(body) => {
                if tx
                    .send(Request {
                        body: body.clone(),
                        reply: reply_tx,
                    })
                    .is_err()
                {
                    break;
                }
                wake.request_repaint();
                let mut r = reply_rx
                    .recv()
                    .unwrap_or_else(|_| json!({"ok": false, "error": "the app closed"}));
                if let (Some(id), Some(obj)) = (body.get("id"), r.as_object_mut()) {
                    obj.insert("id".into(), id.clone());
                }
                r
            }
        };
        if writeln!(out, "{reply}").is_err() {
            break;
        }
    }
}

struct Request {
    body: Value,
    reply: Sender<Value>,
}

enum Step {
    Events(Vec<Event>),
    Idle(u32),
    Shot(PathBuf, bool),
    Tree(bool),
    Resize(f32, f32),
    Quit,
    Done(Value),
}

struct Active {
    steps: VecDeque<Step>,
    reply: Sender<Value>,
    result: Value,
}

#[derive(Default)]
struct Control {
    rx: Mutex<Option<Receiver<Request>>>,
    active: Option<Active>,
    frame: u64,
    pointer: Pos2,
    tree: Option<accesskit::TreeUpdate>,
    want_tree: bool,
    size: [f32; 2],
    ppp: f32,
    focused_sent: bool,
}

impl Control {
    fn with_rx(rx: Receiver<Request>) -> Self {
        Self {
            rx: Mutex::new(Some(rx)),
            ..Self::default()
        }
    }
}

impl egui::Plugin for Control {
    fn debug_name(&self) -> &'static str {
        "kopercad-control"
    }

    fn input_hook(&mut self, ctx: &Context, input: &mut RawInput) {
        self.frame += 1;
        self.ppp = ctx.pixels_per_point();
        let r = input.screen_rect.unwrap_or(ctx.content_rect());
        self.size = [r.width(), r.height()];
        // A window under a bare X server may never be "focused"; the test user is.
        input.focused = true;
        if !self.focused_sent {
            self.focused_sent = true;
            input.events.push(Event::WindowFocused(true));
        }
        loop {
            if self.active.is_none() {
                let next = self
                    .rx
                    .lock()
                    .ok()
                    .and_then(|g| g.as_ref()?.try_recv().ok());
                let Some(req) = next else { return };
                let (steps, result) = self.plan(&req.body);
                self.active = Some(Active {
                    steps,
                    reply: req.reply,
                    result,
                });
            }
            let Some(active) = self.active.as_mut() else {
                return;
            };
            let Some(step) = active.steps.pop_front() else {
                self.active = None;
                continue;
            };
            match step {
                Step::Events(ev) => {
                    input.events.extend(ev);
                    ctx.request_repaint();
                    return;
                }
                Step::Idle(n) => {
                    if n > 1 {
                        active.steps.push_front(Step::Idle(n - 1));
                    }
                    ctx.request_repaint();
                    return;
                }
                Step::Shot(path, sent) => {
                    let shot = input.events.iter().find_map(|e| match e {
                        Event::Screenshot { image, .. } => Some(image.clone()),
                        _ => None,
                    });
                    match shot {
                        Some(img) => {
                            let res = save_png(&path, &img);
                            match res {
                                Ok(()) => {
                                    active.result["path"] = json!(path.display().to_string());
                                    active.result["pixels"] = json!([img.size[0], img.size[1]]);
                                }
                                Err(e) => {
                                    active.result = json!({"ok": false, "error": e});
                                }
                            }
                            continue;
                        }
                        None => {
                            if !sent {
                                ctx.send_viewport_cmd(ViewportCommand::Screenshot(
                                    egui::UserData::default(),
                                ));
                            }
                            active.steps.push_front(Step::Shot(path, true));
                            ctx.request_repaint();
                            return;
                        }
                    }
                }
                Step::Tree(asked) => {
                    if !asked {
                        self.want_tree = true;
                        self.tree = None;
                        active.steps.push_front(Step::Tree(true));
                        ctx.request_repaint();
                        return;
                    }
                    if let Some(update) = self.tree.take() {
                        active.result["nodes"] = tree_json(&update);
                        continue;
                    }
                    active.steps.push_front(Step::Tree(true));
                    ctx.request_repaint();
                    return;
                }
                Step::Resize(w, h) => {
                    ctx.send_viewport_cmd(ViewportCommand::InnerSize(egui::vec2(w, h)));
                    ctx.request_repaint();
                    return;
                }
                Step::Quit => {
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                    continue;
                }
                Step::Done(extra) => {
                    let mut result = std::mem::take(&mut active.result);
                    if let (Some(o), Some(e)) = (result.as_object_mut(), extra.as_object()) {
                        for (k, v) in e {
                            o.insert(k.clone(), v.clone());
                        }
                    }
                    if result.get("ok").is_none() {
                        result["ok"] = json!(true);
                    }
                    result["frame"] = json!(self.frame);
                    result["size"] = json!(self.size);
                    result["pixels_per_point"] = json!(self.ppp);
                    let _ = active.reply.send(result);
                    self.active = None;
                    continue;
                }
            }
        }
    }

    fn output_hook(&mut self, _ctx: &Context, output: &mut FullOutput) {
        if self.want_tree
            && let Some(update) = output.platform_output.accesskit_update.clone()
        {
            self.tree = Some(update);
            self.want_tree = false;
        }
    }
}

impl Control {
    /// Turns a request into steps. The returned value starts the reply.
    fn plan(&mut self, req: &Value) -> (VecDeque<Step>, Value) {
        let cmd = req.get("cmd").and_then(Value::as_str).unwrap_or("");
        let num = |k: &str| req.get(k).and_then(Value::as_f64).map(|v| v as f32);
        let pos = |v: Option<&Value>| -> Option<Pos2> {
            let a = v?.as_array()?;
            Some(Pos2::new(
                a.first()?.as_f64()? as f32,
                a.get(1)?.as_f64()? as f32,
            ))
        };
        let at = |this: &Self| match (num("x"), num("y")) {
            (Some(x), Some(y)) => Pos2::new(x, y),
            _ => this.pointer,
        };
        let mods = parse_modifiers(req.get("modifiers"));
        let button = match req.get("button").and_then(Value::as_str) {
            Some("right") => PointerButton::Secondary,
            Some("middle") => PointerButton::Middle,
            _ => PointerButton::Primary,
        };
        let mut steps = VecDeque::new();
        let fail = |msg: String| {
            (
                VecDeque::from([Step::Done(json!({}))]),
                json!({"ok": false, "error": msg}),
            )
        };
        match cmd {
            "info" | "wait" => {
                let frames = num("frames").map_or(SETTLE_FRAMES, |f| f.clamp(1.0, 600.0) as u32);
                steps.push_back(Step::Idle(frames));
            }
            "screenshot" => {
                let path = req
                    .get("path")
                    .and_then(Value::as_str)
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        std::env::temp_dir().join(format!("kopercad-shot-{}.png", self.frame))
                    });
                steps.push_back(Step::Idle(1));
                steps.push_back(Step::Shot(path, false));
            }
            "ui_tree" => {
                steps.push_back(Step::Tree(false));
            }
            "move" => {
                let p = at(self);
                self.pointer = p;
                steps.push_back(Step::Events(vec![Event::PointerMoved(p)]));
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "click" => {
                let p = at(self);
                self.pointer = p;
                let count = num("count").map_or(1, |c| c.clamp(1.0, 3.0) as u32);
                // Hover first so the widget under the pointer is known, then press and release.
                steps.push_back(Step::Events(vec![Event::PointerMoved(p)]));
                steps.push_back(Step::Idle(1));
                for _ in 0..count {
                    steps.push_back(Step::Events(vec![Event::PointerButton {
                        pos: p,
                        button,
                        pressed: true,
                        modifiers: mods,
                    }]));
                    steps.push_back(Step::Events(vec![Event::PointerButton {
                        pos: p,
                        button,
                        pressed: false,
                        modifiers: mods,
                    }]));
                }
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "drag" => {
                let (Some(from), Some(to)) = (pos(req.get("from")), pos(req.get("to"))) else {
                    return fail("drag needs from:[x,y] and to:[x,y]".into());
                };
                let n = num("steps").map_or(12, |s| s.clamp(2.0, 200.0) as u32);
                steps.push_back(Step::Events(vec![Event::PointerMoved(from)]));
                steps.push_back(Step::Idle(1));
                steps.push_back(Step::Events(vec![Event::PointerButton {
                    pos: from,
                    button,
                    pressed: true,
                    modifiers: mods,
                }]));
                for i in 1..=n {
                    let p = from + (to - from) * (i as f32 / n as f32);
                    steps.push_back(Step::Events(vec![Event::PointerMoved(p)]));
                }
                steps.push_back(Step::Events(vec![Event::PointerButton {
                    pos: to,
                    button,
                    pressed: false,
                    modifiers: mods,
                }]));
                self.pointer = to;
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "scroll" => {
                let p = at(self);
                self.pointer = p;
                steps.push_back(Step::Events(vec![Event::PointerMoved(p)]));
                steps.push_back(Step::Idle(1));
                steps.push_back(Step::Events(vec![Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(num("dx").unwrap_or(0.0), num("dy").unwrap_or(0.0)),
                    phase: egui::TouchPhase::Move,
                    modifiers: mods,
                }]));
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "key" => {
                let Some(spec) = req.get("keys").and_then(Value::as_str) else {
                    return fail("key needs keys, for example \"ctrl+s\" or \"Enter\"".into());
                };
                // Several chords may be given, separated by spaces.
                for chord in spec.split_whitespace() {
                    match parse_chord(chord) {
                        Some((key, m)) => {
                            for pressed in [true, false] {
                                steps.push_back(Step::Events(vec![Event::Key {
                                    key,
                                    physical_key: Some(key),
                                    pressed,
                                    repeat: false,
                                    modifiers: m,
                                }]));
                            }
                        }
                        None => return fail(format!("unknown key `{chord}`")),
                    }
                }
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "text" => {
                let Some(t) = req.get("text").and_then(Value::as_str) else {
                    return fail("text needs text".into());
                };
                steps.push_back(Step::Events(vec![Event::Text(t.to_owned())]));
                steps.push_back(Step::Idle(SETTLE_FRAMES));
            }
            "resize" => {
                let (Some(w), Some(h)) = (num("w"), num("h")) else {
                    return fail("resize needs w and h".into());
                };
                steps.push_back(Step::Resize(w, h));
                steps.push_back(Step::Idle(SETTLE_FRAMES + 2));
            }
            "quit" => steps.push_back(Step::Quit),
            other => return fail(format!("unknown command `{other}`")),
        }
        steps.push_back(Step::Done(json!({})));
        (steps, json!({}))
    }
}

fn save_png(path: &std::path::Path, img: &egui::ColorImage) -> Result<(), String> {
    let mut rgba = Vec::with_capacity(img.pixels.len() * 4);
    for p in &img.pixels {
        rgba.extend_from_slice(&[p.r(), p.g(), p.b(), 255]);
    }
    let buf = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, rgba)
        .ok_or("bad screenshot size")?;
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    buf.save(path).map_err(|e| e.to_string())
}

fn parse_modifiers(v: Option<&Value>) -> Modifiers {
    let mut m = Modifiers::NONE;
    for name in v.and_then(Value::as_array).into_iter().flatten() {
        apply_modifier(&mut m, name.as_str().unwrap_or(""));
    }
    m
}

fn apply_modifier(m: &mut Modifiers, name: &str) -> bool {
    match name.to_ascii_lowercase().as_str() {
        "ctrl" | "control" => {
            m.ctrl = true;
            m.command = true;
        }
        "shift" => m.shift = true,
        "alt" => m.alt = true,
        _ => return false,
    }
    true
}

/// `"ctrl+shift+k"`, `"Enter"`, `"F2"`, `"+"` (a lone plus is the Plus key).
fn parse_chord(chord: &str) -> Option<(egui::Key, Modifiers)> {
    let mut m = Modifiers::NONE;
    let mut parts: Vec<&str> = chord.split('+').collect();
    if chord.ends_with('+') {
        parts.pop();
        parts.pop();
        parts.push("+");
    }
    let key = parts.pop()?;
    for p in parts {
        if !apply_modifier(&mut m, p) {
            return None;
        }
    }
    let key = egui::Key::from_name(key).or_else(|| {
        let mut cap = key.to_owned();
        cap[..1].make_ascii_uppercase();
        egui::Key::from_name(&cap)
    })?;
    Some((key, m))
}

fn tree_json(update: &accesskit::TreeUpdate) -> Value {
    use accesskit::{Role, Toggled};
    let nodes: std::collections::HashMap<_, _> = update.nodes.iter().cloned().collect();
    let mut out = Vec::new();
    let root = update.tree.as_ref().map(|t| t.root);
    let mut stack: Vec<(accesskit::NodeId, usize)> = root.into_iter().map(|r| (r, 0)).collect();
    while let Some((id, depth)) = stack.pop() {
        let Some(node) = nodes.get(&id) else { continue };
        let label = node.label().or_else(|| node.value()).unwrap_or("");
        let role = node.role();
        let interesting = (!label.is_empty() && role != Role::TextRun)
            || !matches!(
                role,
                Role::GenericContainer | Role::Window | Role::Unknown | Role::Label | Role::TextRun
            );
        if interesting && let Some(b) = node.bounds() {
            let mut n = json!({
                "role": format!("{role:?}"),
                "label": label,
                "rect": [b.x0.round(), b.y0.round(), b.x1.round(), b.y1.round()],
            });
            if let Some(v) = node.value().filter(|v| *v != label) {
                n["value"] = json!(v);
            }
            if node.is_disabled() {
                n["disabled"] = json!(true);
            }
            match node.toggled() {
                Some(Toggled::True) => n["checked"] = json!(true),
                Some(Toggled::False) => n["checked"] = json!(false),
                _ => {}
            }
            out.push(n);
        }
        for child in node.children().iter().rev() {
            stack.push((*child, depth + 1));
        }
    }
    Value::Array(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chords_parse_with_modifiers() {
        let (k, m) = parse_chord("ctrl+shift+k").unwrap();
        assert_eq!(k, egui::Key::K);
        assert!(m.ctrl && m.shift && m.command && !m.alt);
        assert_eq!(parse_chord("Enter").unwrap().0, egui::Key::Enter);
        assert_eq!(parse_chord("escape").unwrap().0, egui::Key::Escape);
        assert_eq!(parse_chord("ctrl+0").unwrap().0, egui::Key::Num0);
        assert!(parse_chord("hyper+k").is_none());
        assert!(parse_chord("nokey").is_none());
    }

    #[test]
    fn control_flags_are_not_project_files() {
        assert!(is_control_arg("--control".as_ref()));
        assert!(is_control_arg("--control=4000".as_ref()));
        assert!(!is_control_arg("plant.novo".as_ref()));
    }
}
