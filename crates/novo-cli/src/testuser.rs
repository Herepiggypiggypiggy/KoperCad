//! `kopercad-cli test-user`: an MCP server (stdio) that lets an AI assistant
//! use the real KoperCAD window like a person: see it (screenshots), read
//! its labelled widgets, click, drag, scroll and type. It starts the app in
//! control mode (`kopercad --control`) with an empty profile, and on Linux
//! without a display it brings its own virtual screen (Xvfb, openbox,
//! software Vulkan). Per user, loopback only, no admin rights.
//!
//! See `docs/testing/visual-test-user.md`.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use clap::Args;
use serde_json::{Value, json};

use crate::{Failure, exit};

#[derive(Debug, Args)]
pub(crate) struct TestUser {
    /// The KoperCAD program to start (default: `kopercad` beside this tool).
    #[arg(long)]
    app: Option<PathBuf>,
    /// Use an app that is already running in control mode on this port
    /// instead of starting one (for example on a Windows PC).
    #[arg(long)]
    connect: Option<u16>,
    /// The control token of that app (the second line of its port file).
    /// Without it, `--connect` reads `KOPERCAD_CONTROL_TOKEN`.
    #[arg(long)]
    token: Option<String>,
    /// Window size in points at start, `WIDTHxHEIGHT`.
    #[arg(long, default_value = "1366x768")]
    size: String,
    /// A project file to open on start.
    #[arg(long)]
    project: Option<PathBuf>,
    /// Where screenshots are written (default: a folder in the temp folder).
    #[arg(long)]
    shots: Option<PathBuf>,
    /// Keep the user's real settings instead of an empty first-run profile.
    #[arg(long)]
    real_profile: bool,
}

pub(crate) fn run(opts: TestUser, _out: &mut dyn Write) -> Result<i32, Failure> {
    let mut srv = Server::new(opts);
    srv.serve().map_err(|e| Failure::new(exit::WRITE, e))?;
    srv.stop();
    Ok(exit::OK)
}

struct Session {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next_id: u64,
}

struct Server {
    opts: TestUser,
    session: Option<Session>,
    children: Vec<Child>,
    shot_no: u32,
    shots: PathBuf,
    /// Keep the profile folder of the last start (a "crash" test relaunch).
    keep_profile: bool,
}

impl Server {
    fn new(opts: TestUser) -> Server {
        let shots = opts
            .shots
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("kopercad-test-user"));
        Server {
            opts,
            session: None,
            children: Vec::new(),
            shot_no: 0,
            shots,
            keep_profile: false,
        }
    }

    fn stop(&mut self) {
        self.session = None;
        for c in &mut self.children {
            let _ = c.kill();
            let _ = c.wait();
        }
        self.children.clear();
    }

    // ---- MCP over stdio -------------------------------------------------

    fn serve(&mut self) -> Result<(), String> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().lock();
        for line in stdin.lock().lines() {
            let line = line.map_err(|e| e.to_string())?;
            if line.trim().is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            let Some(id) = msg.get("id").cloned() else {
                continue; // a notification
            };
            let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
            let reply = match method {
                "initialize" => Ok(json!({
                    "protocolVersion": msg["params"]["protocolVersion"]
                        .as_str().unwrap_or("2024-11-05"),
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "kopercad-test-user", "version": "0.1.0"},
                    "instructions": INSTRUCTIONS,
                })),
                "ping" => Ok(json!({})),
                "tools/list" => Ok(json!({"tools": tools()})),
                "tools/call" => Ok(self.call(&msg["params"])),
                other => Err(format!("method not found: {other}")),
            };
            let out = match reply {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err(message) => {
                    json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": message}})
                }
            };
            writeln!(stdout, "{out}").map_err(|e| e.to_string())?;
            stdout.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    /// Runs one tool; errors become a tool result flagged `isError`.
    fn call(&mut self, params: &Value) -> Value {
        let name = params["name"].as_str().unwrap_or("");
        let args = &params["arguments"];
        match self.tool(name, args) {
            Ok(content) => json!({"content": content, "isError": false}),
            Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
        }
    }

    fn tool(&mut self, name: &str, a: &Value) -> Result<Vec<Value>, String> {
        let text = |s: String| Ok(vec![json!({"type": "text", "text": s})]);
        match name {
            "screenshot" => self.screenshot(),
            "ui_tree" => {
                let r = self.send(json!({"cmd": "ui_tree"}))?;
                let filter = a["filter"].as_str().map(str::to_lowercase);
                let nodes = r["nodes"].as_array().cloned().unwrap_or_default();
                let mut lines = Vec::new();
                for n in nodes {
                    let line = describe(&n);
                    if filter
                        .as_ref()
                        .is_none_or(|f| line.to_lowercase().contains(f))
                    {
                        lines.push(line);
                    }
                }
                text(format!(
                    "{} widgets (rects are [x0,y0,x1,y1] in window points):\n{}",
                    lines.len(),
                    lines.join("\n")
                ))
            }
            "click_text" => {
                let wanted = a["text"].as_str().ok_or("click_text needs text")?;
                let (x, y, line) = self.find(wanted, a["role"].as_str())?;
                let mut req = json!({"cmd": "click", "x": x, "y": y});
                copy(&mut req, a, &["button", "count", "modifiers"]);
                self.send(req)?;
                text(format!("clicked {line} at ({x:.0},{y:.0})"))
            }
            "click" | "mouse_down" | "mouse_up" | "move" | "scroll" | "drag" | "key" | "text"
            | "wait" | "resize" => {
                let cmd = match name {
                    "mouse_down" => "down",
                    "mouse_up" => "up",
                    other => other,
                };
                let mut req = json!({"cmd": cmd});
                copy(
                    &mut req,
                    a,
                    &[
                        "x",
                        "y",
                        "button",
                        "count",
                        "modifiers",
                        "dx",
                        "dy",
                        "from",
                        "to",
                        "steps",
                        "keys",
                        "text",
                        "frames",
                        "w",
                        "h",
                    ],
                );
                let r = self.send(req)?;
                text(format!(
                    "ok (window {}x{} points, frame {})",
                    r["size"][0], r["size"][1], r["frame"]
                ))
            }
            "scene" => {
                let r = self.send(json!({"cmd": "scene"}))?;
                let mut scene = r["scene"].clone();
                if let (Some(keys), Some(obj)) = (a["parts"].as_array(), scene.as_object_mut()) {
                    let keys: Vec<&str> = keys.iter().filter_map(Value::as_str).collect();
                    obj.retain(|k, _| {
                        keys.contains(&k.as_str())
                            || matches!(k.as_str(), "workspace" | "project" | "page")
                    });
                }
                text(serde_json::to_string_pretty(&scene).map_err(|e| e.to_string())?)
            }
            "answer_dialog" => {
                let mut req = json!({"cmd": "dialog_answer"});
                copy(&mut req, a, &["path", "paths"]);
                self.send(req)?;
                text("queued: the next native file dialog gets this answer (none = cancel)".into())
            }
            "dialogs" => {
                let r = self.send(json!({"cmd": "dialogs"}))?;
                text(r.to_string())
            }
            "info" => {
                let r = self.send(json!({"cmd": "info"}))?;
                text(r.to_string())
            }
            "restart" => {
                self.stop();
                self.keep_profile = a["fresh"] == json!(false);
                if let Some(p) = a["project"].as_str() {
                    self.opts.project = Some(PathBuf::from(p));
                }
                self.ensure()?;
                self.screenshot()
            }
            "quit" => {
                let _ = self.send(json!({"cmd": "quit"}));
                self.stop();
                text("closed".into())
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }

    fn find(&mut self, wanted: &str, role: Option<&str>) -> Result<(f64, f64, String), String> {
        let r = self.send(json!({"cmd": "ui_tree"}))?;
        let w = wanted.to_lowercase();
        let nodes = r["nodes"].as_array().cloned().unwrap_or_default();
        let pick = |exact: bool| {
            nodes.iter().find(|n| {
                let label = n["label"].as_str().unwrap_or("").to_lowercase();
                let hit = if exact {
                    label == w
                } else {
                    label.contains(&w)
                };
                hit && role.is_none_or(|r| {
                    n["role"]
                        .as_str()
                        .is_some_and(|nr| nr.eq_ignore_ascii_case(r))
                }) && n["disabled"] != json!(true)
            })
        };
        let n = pick(true)
            .or_else(|| pick(false))
            .ok_or_else(|| format!("no enabled widget labelled `{wanted}`; try ui_tree"))?;
        let rc = &n["rect"];
        let (x0, y0, x1, y1) = (
            rc[0].as_f64(),
            rc[1].as_f64(),
            rc[2].as_f64(),
            rc[3].as_f64(),
        );
        match (x0, y0, x1, y1) {
            (Some(x0), Some(y0), Some(x1), Some(y1)) => {
                Ok(((x0 + x1) / 2.0, (y0 + y1) / 2.0, describe(n)))
            }
            _ => Err("widget has no rect".into()),
        }
    }

    fn screenshot(&mut self) -> Result<Vec<Value>, String> {
        self.shot_no += 1;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs() % 100_000);
        let name = format!("shot-{stamp:05}-{:03}.png", self.shot_no);
        let r = self.send(json!({"cmd": "screenshot", "name": name}))?;
        // With --connect the app may keep its shots elsewhere: it says where.
        let path = r["path"]
            .as_str()
            .map_or_else(|| self.shots.join(&name), PathBuf::from);
        let bytes = std::fs::read(&path).map_err(|e| format!("cannot read screenshot: {e}"))?;
        Ok(vec![
            json!({"type": "image", "data": base64(&bytes), "mimeType": "image/png"}),
            json!({"type": "text", "text": format!(
                "{} pixels {}x{}, window {}x{} points (scale {}); saved {}",
                path.file_name().and_then(|n| n.to_str()).unwrap_or("shot"),
                r["pixels"][0], r["pixels"][1], r["size"][0], r["size"][1],
                r["pixels_per_point"], path.display())}),
        ])
    }

    // ---- talking to the app ---------------------------------------------

    fn send(&mut self, mut req: Value) -> Result<Value, String> {
        self.ensure()?;
        let s = self.session.as_mut().ok_or("not connected")?;
        s.next_id += 1;
        req["id"] = json!(s.next_id);
        writeln!(s.stream, "{req}").map_err(|e| format!("app gone: {e}"))?;
        let mut line = String::new();
        let n = s
            .reader
            .read_line(&mut line)
            .map_err(|e| format!("app gone: {e}"))?;
        if n == 0 {
            self.session = None;
            return Err("the app closed; use restart".into());
        }
        let v: Value = serde_json::from_str(&line).map_err(|e| e.to_string())?;
        if v["ok"] == json!(false) {
            return Err(v["error"].as_str().unwrap_or("failed").to_owned());
        }
        Ok(v)
    }

    fn ensure(&mut self) -> Result<(), String> {
        if self.session.is_some() {
            return Ok(());
        }
        let (port, token) = match self.opts.connect {
            Some(p) => {
                let token = self
                    .opts
                    .token
                    .clone()
                    .or_else(|| std::env::var("KOPERCAD_CONTROL_TOKEN").ok())
                    .ok_or("--connect needs --token (or KOPERCAD_CONTROL_TOKEN)")?;
                (p, token)
            }
            None => self.launch()?,
        };
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("cannot connect to the app on port {port}: {e}"))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut hello = stream.try_clone().map_err(|e| e.to_string())?;
        writeln!(hello, "{}", json!({"cmd": "hello", "token": token}))
            .map_err(|e| format!("app gone: {e}"))?;
        let mut ack = String::new();
        reader
            .read_line(&mut ack)
            .map_err(|e| format!("no answer to hello: {e}"))?;
        if !ack.contains("true") {
            return Err("the app refused the control token".into());
        }
        self.session = Some(Session {
            stream,
            reader,
            next_id: 0,
        });
        // Window size, then let the first frames draw.
        if let Some((w, h)) = self.opts.size.split_once('x')
            && let (Ok(w), Ok(h)) = (w.parse::<f64>(), h.parse::<f64>())
        {
            self.send(json!({"cmd": "resize", "w": w, "h": h}))?;
        }
        self.send(json!({"cmd": "wait", "frames": 20}))?;
        Ok(())
    }

    fn launch(&mut self) -> Result<(u16, String), String> {
        let app = self.opts.app.clone().unwrap_or_else(default_app);
        let work = std::env::temp_dir().join(format!("kopercad-test-user-{}", std::process::id()));
        if !self.keep_profile {
            let _ = std::fs::remove_dir_all(&work);
        }
        let _ = std::fs::remove_file(work.join("port"));
        std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(&self.shots).map_err(|e| e.to_string())?;
        let port_file = work.join("port");
        let mut cmd = Command::new(&app);
        cmd.arg("--control");
        if let Some(p) = &self.opts.project {
            cmd.arg(p);
        }
        // The app's own output is kept next to the screenshots: a crash shows there.
        let log = std::fs::File::create(self.shots.join("app.log")).map_err(|e| e.to_string())?;
        let log_err = log.try_clone().map_err(|e| e.to_string())?;
        cmd.env("KOPERCAD_CONTROL_PORT_FILE", &port_file)
            .env("KOPERCAD_CONTROL_SHOTS", &self.shots)
            .env("RUST_BACKTRACE", "1")
            .stdout(log)
            .stderr(log_err);
        if !self.opts.real_profile {
            // An empty profile: the first-run look a new user gets.
            let home = work.join("home");
            let _ = std::fs::create_dir_all(home.join("Documents"));
            // Linux finds Documents through this file (a Windows known folder elsewhere).
            let _ = std::fs::create_dir_all(home.join("config"));
            let _ = std::fs::write(
                home.join("config/user-dirs.dirs"),
                format!(
                    "XDG_DOCUMENTS_DIR=\"{}\"\n",
                    home.join("Documents").display()
                ),
            );
            cmd.env("HOME", &home)
                .env("XDG_DOCUMENTS_DIR", home.join("Documents"))
                .env("XDG_DATA_HOME", home.join("data"))
                .env("XDG_CONFIG_HOME", home.join("config"))
                .env("APPDATA", home.join("AppData"))
                .env("LOCALAPPDATA", home.join("Local"));
        }
        #[cfg(target_os = "linux")]
        if std::env::var_os("DISPLAY").is_none() && std::env::var_os("WAYLAND_DISPLAY").is_none() {
            self.virtual_screen(&mut cmd, &work)?;
        }
        let child = cmd
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", app.display()))?;
        self.children.push(child);
        let start = Instant::now();
        loop {
            if let Ok(s) = std::fs::read_to_string(&port_file) {
                let mut lines = s.lines();
                if let (Some(Ok(p)), Some(t)) = (lines.next().map(str::parse), lines.next())
                    && !t.is_empty()
                {
                    return Ok((p, t.to_owned()));
                }
            }
            if start.elapsed() > Duration::from_secs(90) {
                return Err("the app did not open its control port in 90 s".into());
            }
            if let Some(c) = self.children.last_mut()
                && let Ok(Some(status)) = c.try_wait()
            {
                return Err(format!("the app exited at start ({status})"));
            }
            std::thread::sleep(Duration::from_millis(200));
        }
    }

    /// Linux with no screen: Xvfb, a window manager if there is one, and
    /// software Vulkan when no GPU driver is present.
    #[cfg(target_os = "linux")]
    fn virtual_screen(&mut self, cmd: &mut Command, work: &Path) -> Result<(), String> {
        let n = 90 + (std::process::id() % 100);
        let display = format!(":{n}");
        let (w, h) = self
            .opts
            .size
            .split_once('x')
            .and_then(|(w, h)| Some((w.parse::<u32>().ok()?, h.parse::<u32>().ok()?)))
            .unwrap_or((1366, 768));
        // Room for a title bar and the window manager's border.
        let (w, h) = (w + 40, h + 80);
        let xvfb = Command::new("Xvfb")
            .args([&display, "-screen", "0", &format!("{w}x{h}x24")])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("no display and cannot start Xvfb: {e}"))?;
        self.children.push(xvfb);
        std::thread::sleep(Duration::from_millis(1200));
        if let Ok(wm) = Command::new("openbox")
            .env("DISPLAY", &display)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        {
            self.children.push(wm);
        }
        cmd.env("DISPLAY", &display);
        let lvp = Path::new("/usr/share/vulkan/icd.d/lvp_icd.json");
        if lvp.exists() && std::env::var_os("VK_ICD_FILENAMES").is_none() {
            cmd.env("VK_ICD_FILENAMES", lvp);
        }
        let _ = work;
        Ok(())
    }
}

fn default_app() -> PathBuf {
    let name = if cfg!(windows) {
        "kopercad.exe"
    } else {
        "kopercad"
    };
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join(name)))
        .unwrap_or_else(|| PathBuf::from(name))
}

fn copy(req: &mut Value, from: &Value, keys: &[&str]) {
    for k in keys {
        if !from[*k].is_null() {
            req[*k] = from[*k].clone();
        }
    }
}

fn describe(n: &Value) -> String {
    let mut s = format!(
        "{} '{}'",
        n["role"].as_str().unwrap_or("?"),
        n["label"].as_str().unwrap_or("")
    );
    if let Some(v) = n["value"].as_str() {
        s.push_str(&format!(" value='{v}'"));
    }
    if n["checked"] == json!(true) {
        s.push_str(" [checked]");
    }
    if n["disabled"] == json!(true) {
        s.push_str(" [disabled]");
    }
    s.push_str(&format!(" {}", n["rect"]));
    s
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

const INSTRUCTIONS: &str = "You are a first-time KoperCAD user with a screen, a mouse and a keyboard. \
Call screenshot to see the window. Coordinates are window points (the same units ui_tree reports; a \
screenshot's pixels are points times its scale). Prefer click_text for labelled buttons and menus, and \
click with coordinates for the drawing canvas, which has no labels. Work from the goal, not from code. \
Report where you got stuck, what you expected and what you saw.";

fn tools() -> Value {
    let num = json!({"type": "number"});
    let pt = json!({"type": "array", "items": {"type": "number"}, "minItems": 2, "maxItems": 2});
    let mods =
        json!({"type": "array", "items": {"type": "string", "enum": ["ctrl", "shift", "alt"]}});
    let btn = json!({"type": "string", "enum": ["left", "right", "middle"]});
    let tool = |name: &str, desc: &str, props: Value, req: Value| {
        json!({"name": name, "description": desc,
               "inputSchema": {"type": "object", "properties": props, "required": req}})
    };
    json!([
        tool(
            "screenshot",
            "Take a picture of the KoperCAD window as it looks now. Starts the app on first use.",
            json!({}),
            json!([])
        ),
        tool(
            "ui_tree",
            "List the labelled widgets on screen (buttons, menus, fields, checkboxes) with state and rect. The drawing canvas is not in the list; use a screenshot for it. filter: only lines containing this text.",
            json!({"filter": {"type": "string"}}),
            json!([])
        ),
        tool(
            "click_text",
            "Click the centre of the enabled widget whose label matches text (exact match first, then contains). Optional role (Button, MenuItem, CheckBox, TextInput, ...), button, count (2 = double click), modifiers.",
            json!({"text": {"type": "string"}, "role": {"type": "string"}, "button": btn, "count": num, "modifiers": mods}),
            json!(["text"])
        ),
        tool(
            "click",
            "Click at window point x,y. count 2 = double click.",
            json!({"x": num, "y": num, "button": btn, "count": num, "modifiers": mods}),
            json!(["x", "y"])
        ),
        tool(
            "move",
            "Move the pointer to x,y (hover; tooltips and previews appear).",
            json!({"x": num, "y": num}),
            json!(["x", "y"])
        ),
        tool(
            "drag",
            "Press at from, move to to in steps, release.",
            json!({"from": pt, "to": pt, "button": btn, "steps": num, "modifiers": mods}),
            json!(["from", "to"])
        ),
        tool(
            "scroll",
            "Mouse wheel at x,y: dy positive scrolls content down (zoom in the drawing with ctrl).",
            json!({"x": num, "y": num, "dx": num, "dy": num, "modifiers": mods}),
            json!(["dy"])
        ),
        tool(
            "key",
            "Press keys: \"Enter\", \"Escape\", \"ctrl+s\", \"ctrl+shift+k\", \"F2\"; several chords separated by spaces.",
            json!({"keys": {"type": "string"}}),
            json!(["keys"])
        ),
        tool(
            "text",
            "Type text into the focused field.",
            json!({"text": {"type": "string"}}),
            json!(["text"])
        ),
        tool(
            "wait",
            "Let the app run this many frames (default 4).",
            json!({"frames": num}),
            json!([])
        ),
        tool(
            "resize",
            "Resize the window (points), for example 1366x768 or 1920x1080.",
            json!({"w": num, "h": num}),
            json!(["w", "h"])
        ),
        tool(
            "info",
            "Window size, scale and frame count.",
            json!({}),
            json!([])
        ),
        tool(
            "restart",
            "Close the app and start a fresh first-run one, optionally opening a project file.",
            json!({"project": {"type": "string"}}),
            json!([])
        ),
        tool("quit", "Close the app.", json!({}), json!([])),
    ])
}

#[cfg(test)]
mod tests {
    use super::base64;

    #[test]
    fn base64_matches_the_standard_examples() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
