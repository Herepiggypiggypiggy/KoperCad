//! The requests and what they do (see the table in `lib.rs`).

use novo_controller::{
    Action, ControllerSet, InstanceRecord, Profile, ProfileLibrary, Sequence, Severity, validate,
};
use serde_json::{Value, json};

use crate::API;

/// Longest profile text taken, in bytes.
const MAX_PROFILE_BYTES: usize = 256 * 1024;
/// Most instances one project may hold.
const MAX_INSTANCES: usize = 64;
/// Longest time step one `advance` may take (one hour).
const MAX_STEP_MS: u64 = 3_600_000;

/// The generic radio shipped in the pack (`data/`); the engine starts with it and one instance.
const BUNDLED_RADIO: &str = include_str!("../../pack/data/generic-radio-3x2.controller.toml");

/// Id of the instance the panel acts on at start.
pub const DEFAULT_INSTANCE: &str = "radio";

/// One engine: the profile library, the instances and a recording in progress.
#[derive(Default)]
pub struct Engine {
    library: ProfileLibrary,
    set: ControllerSet,
}

type Reply = Result<Value, String>;

fn text<'a>(data: &'a Value, key: &str) -> Result<&'a str, String> {
    data.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("the request needs `{key}` as text"))
}

impl Engine {
    /// An engine with no profile and no instance.
    pub fn empty() -> Engine {
        Engine::default()
    }

    /// The engine as the add-on starts it: the bundled generic radio and one instance of it,
    /// `radio`, so the panel works the moment the add-on is on.
    pub fn new() -> Engine {
        let mut engine = Engine::empty();
        if let Ok(profile) = Profile::load(BUNDLED_RADIO) {
            let _ = engine.set.add_instance(DEFAULT_INSTANCE, "Radio", &profile);
            engine.library.insert(profile);
        }
        engine
    }

    /// Handles one request text and returns the reply text. Never panics on bad input: a
    /// request that is not understood is a refusal (`ok:false`), which leaves the add-on on.
    pub fn handle(&mut self, request: &str) -> String {
        let reply = match serde_json::from_str::<Value>(request) {
            Ok(v) => self.dispatch(&v),
            Err(e) => Err(format!("the request is not valid JSON ({e})")),
        };
        match reply {
            Ok(data) => json!({"ok": true, "data": data}),
            Err(error) => json!({"ok": false, "error": error}),
        }
        .to_string()
    }

    fn dispatch(&mut self, request: &Value) -> Reply {
        if request.get("api").and_then(Value::as_u64) != Some(u64::from(API)) {
            return Err(format!("this engine speaks host API {API}"));
        }
        let op = text(request, "op")?;
        let null = Value::Null;
        let data = request.get("data").unwrap_or(&null);
        match op {
            "hello" => Ok(json!({"api": API})),
            "profile.add" => self.profile_add(data),
            "instance.add" => {
                if self.set.instances().len() >= MAX_INSTANCES {
                    return Err(format!(
                        "a project holds at most {MAX_INSTANCES} controllers"
                    ));
                }
                let profile = self
                    .library
                    .get(text(data, "profile")?)
                    .ok_or_else(|| {
                        format!(
                            "no controller profile `{}`",
                            text(data, "profile").unwrap_or("")
                        )
                    })?
                    .clone();
                let label = text(data, "label").unwrap_or("");
                self.set
                    .add_instance(text(data, "id")?, label, &profile)
                    .map_err(|e| e.to_string())?;
                Ok(self.snapshot())
            }
            "instance.duplicate" => {
                if self.set.instances().len() >= MAX_INSTANCES {
                    return Err(format!(
                        "a project holds at most {MAX_INSTANCES} controllers"
                    ));
                }
                self.set
                    .duplicate_instance(
                        text(data, "from")?,
                        text(data, "id")?,
                        text(data, "label").unwrap_or(""),
                    )
                    .map_err(|e| e.to_string())?;
                Ok(self.snapshot())
            }
            "instance.remove" => {
                self.set
                    .remove_instance(text(data, "id")?)
                    .map_err(|e| e.to_string())?;
                Ok(self.snapshot())
            }
            "instance.rename" => {
                self.set
                    .rename(text(data, "id")?, text(data, "label")?)
                    .map_err(|e| e.to_string())?;
                Ok(self.snapshot())
            }
            "act" => {
                let action: Action =
                    serde_json::from_value(data.get("action").cloned().unwrap_or(Value::Null))
                        .map_err(|e| format!("not an action ({e})"))?;
                let applied = self
                    .set
                    .act(text(data, "instance")?, &action)
                    .map_err(|e| e.to_string())?;
                Ok(json!({"applied": format!("{applied:?}"), "snapshot": self.snapshot()}))
            }
            "advance" => {
                let dt = data.get("dt_ms").and_then(Value::as_u64).unwrap_or(0);
                if dt > MAX_STEP_MS {
                    return Err("one step may be at most an hour".to_owned());
                }
                self.set.advance(dt);
                Ok(self.snapshot())
            }
            "snapshot" => Ok(self.snapshot()),
            "records.save" => Ok(json!({"instances": self.set.to_records()})),
            "records.load" => {
                let records: Vec<InstanceRecord> =
                    serde_json::from_value(data.get("instances").cloned().unwrap_or(Value::Null))
                        .map_err(|e| format!("not instance records ({e})"))?;
                if records.len() > MAX_INSTANCES {
                    return Err(format!(
                        "a project holds at most {MAX_INSTANCES} controllers"
                    ));
                }
                self.set = ControllerSet::from_records(&records, &self.library)
                    .map_err(|e| e.to_string())?;
                Ok(self.snapshot())
            }
            "record.start" => {
                self.set.start_recording();
                Ok(json!({}))
            }
            "record.stop" => {
                let name = text(data, "name").unwrap_or("recording");
                let seq = self
                    .set
                    .stop_recording(name)
                    .ok_or("no recording is running")?;
                Ok(json!({"sequence": seq.to_toml()?}))
            }
            "replay" => self.replay(data),
            "button" => self.button(data),
            "command" => Err("this add-on's commands open its panel".to_owned()),
            other => Err(format!("unknown op `{other}`")),
        }
    }

    fn profile_add(&mut self, data: &Value) -> Reply {
        let toml = text(data, "toml")?;
        if toml.len() > MAX_PROFILE_BYTES {
            return Err("the profile is larger than 256 KiB".to_owned());
        }
        let profile = Profile::parse(toml).map_err(|e| e.to_string())?;
        let errors: Vec<String> = validate(&profile)
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.to_string())
            .collect();
        if !errors.is_empty() {
            return Err(format!("the profile has problems: {}", errors.join("; ")));
        }
        let reply = json!({"id": profile.controller.id, "name": profile.controller.name});
        self.library.insert(profile);
        Ok(reply)
    }

    fn replay(&mut self, data: &Value) -> Reply {
        let sequence = Sequence::parse(text(data, "sequence")?)?;
        let sample = data.get("sample_ms").and_then(Value::as_u64).unwrap_or(0);
        let settle = data.get("settle_ms").and_then(Value::as_u64).unwrap_or(0);
        // The live set is not touched: a replay runs on a copy.
        let mut copy = self.set.clone();
        let trace = sequence
            .replay(&mut copy, sample, settle)
            .map_err(|e| e.to_string())?;
        let samples: Vec<Value> = trace
            .samples
            .iter()
            .map(|s| {
                let signals: serde_json::Map<String, Value> = s
                    .signals
                    .iter()
                    .map(|(k, v)| (k.to_string(), json!(v)))
                    .collect();
                json!({"t_ms": s.t_ms, "signals": signals})
            })
            .collect();
        Ok(json!({"samples": samples}))
    }

    /// The declared panel `radio`. Button ids (panel ids are lower case and hyphens, so a
    /// control `hoist_sel` is written `hoist-sel`): `press-<control>`, `tap-<control>` (press and let go), `release-<control>`,
    /// `reset-<control>` and `set-<control>-<position>`. They act on the instance `radio` (or
    /// the first one). The answer has a lamp for every signal of that instance.
    fn button(&mut self, data: &Value) -> Reply {
        let id = text(data, "id")?;
        let instance = if self.set.instance(DEFAULT_INSTANCE).is_some() {
            DEFAULT_INSTANCE.to_owned()
        } else {
            self.set
                .instances()
                .first()
                .map(|i| i.id.clone())
                .ok_or("there is no controller yet; add one first")?
        };
        let action = self.button_action(&instance, id)?;
        self.set
            .act(&instance, &action)
            .map_err(|e| e.to_string())?;
        if let Some(release) = self.tap_release(&instance, id)? {
            self.set
                .act(&instance, &release)
                .map_err(|e| e.to_string())?;
        }
        Ok(json!({"lamps": self.lamps(&instance)}))
    }

    /// `tap-<control>` presses and lets go at once (a momentary push button).
    fn tap_release(&self, instance: &str, id: &str) -> Result<Option<Action>, String> {
        match id.strip_prefix("tap-") {
            Some(rest) => match self.button_action(instance, &format!("release-{rest}"))? {
                action @ Action::Release { .. } => Ok(Some(action)),
                _ => Ok(None),
            },
            None => Ok(None),
        }
    }

    fn button_action(&self, instance: &str, id: &str) -> Result<Action, String> {
        let (verb, rest) = id
            .split_once('-')
            .ok_or_else(|| format!("unknown button `{id}`"))?;
        let controls = &self
            .set
            .instance(instance)
            .ok_or("no such controller")?
            .controller
            .profile()
            .controls;
        // Longest control id first, so `hoist-sel` wins over `hoist`.
        let mut ids: Vec<&str> = controls.iter().map(|c| c.id.as_str()).collect();
        ids.sort_by_key(|c| std::cmp::Reverse(c.len()));
        let (control, tail) = ids
            .into_iter()
            .find_map(|c| {
                let key = panel_id(c);
                if rest == key {
                    Some((c, String::new()))
                } else {
                    rest.strip_prefix(&format!("{key}-"))
                        .map(|t| (c, t.to_owned()))
                }
            })
            .ok_or_else(|| format!("unknown button `{id}`"))?;
        let control = control.to_owned();
        match verb {
            "press" | "tap" if tail.is_empty() => Ok(Action::Press { control }),
            "release" if tail.is_empty() => Ok(Action::Release { control }),
            "reset" if tail.is_empty() => Ok(Action::Reset { control }),
            "set" if !tail.is_empty() => {
                let def = controls
                    .iter()
                    .find(|c| c.id == control)
                    .ok_or("no control")?;
                let position = def
                    .position_list()
                    .into_iter()
                    .find(|p| panel_id(p) == tail)
                    .ok_or_else(|| format!("unknown button `{id}`"))?;
                Ok(Action::SetPosition { control, position })
            }
            _ => Err(format!("unknown button `{id}`")),
        }
    }

    /// One lamp per signal of `instance`, ids as the panel writes them.
    fn lamps(&self, instance: &str) -> Value {
        let mut lamps = serde_json::Map::new();
        for (key, level) in self.set.signals() {
            if key.controller == instance {
                let on = level != 0.0;
                lamps.insert(
                    panel_id(&key.signal),
                    json!({"state": if on { "ok" } else { "neutral" }, "lit": on}),
                );
            }
        }
        Value::Object(lamps)
    }

    /// Every instance with its controls and signals.
    fn snapshot(&self) -> Value {
        let instances: Vec<Value> = self
            .set
            .instances()
            .iter()
            .map(|i| {
                let c = &i.controller;
                let controls: Vec<Value> = c
                    .profile()
                    .controls
                    .iter()
                    .map(|d| {
                        json!({
                            "id": d.id,
                            "label": d.label,
                            "kind": d.kind.as_str(),
                            "positions": d.position_list(),
                            "position": c.position(&d.id),
                            "effective": c.effective_position(&d.id),
                        })
                    })
                    .collect();
                let signals: Vec<Value> = c
                    .signals()
                    .into_iter()
                    .map(|s| json!({"name": s.name, "level": s.level, "high": s.is_high()}))
                    .collect();
                json!({
                    "id": i.id,
                    "label": i.label,
                    "profile": c.profile().controller.id,
                    "generic": c.profile().controller.generic,
                    "link_ok": c.link_ok(),
                    "battery_low": c.battery_low(),
                    "powered": c.powered(),
                    "warnings": c.warnings(),
                    "controls": controls,
                    "signals": signals,
                })
            })
            .collect();
        json!({"time_ms": self.set.time_ms(), "instances": instances})
    }
}

/// How a control, position or signal name is written in a panel id: lower case, hyphens.
fn panel_id(name: &str) -> String {
    name.to_ascii_lowercase().replace('_', "-")
}
