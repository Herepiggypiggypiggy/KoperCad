//! The add-on engine host (lane C0b-2): runs an add-on's `engine.wasm` in a `wasmi` interpreter
//! with nothing to reach the PC with. The program speaks **host API 1**: JSON messages in the
//! engine's own linear memory.
//!
//! What the engine can do: compute. What it cannot: open a file, use the network, read a clock,
//! start a thread or touch the program's memory. It gets one import, `kopercad.log(ptr, len)`,
//! whose text the host keeps (a few kilobytes) for the Details screen. A module that imports
//! anything else is refused when it loads. Every call has a step budget (fuel), so an endless
//! loop ends; memory and stack are capped; a trap, a broken reply or a budget overrun *stops* the
//! add-on for the session and the program goes on (`EngineHost::stopped`).
//!
//! # The engine's side (what a `wasm32-unknown-unknown` build exports)
//!
//! | Export | Signature | What |
//! | --- | --- | --- |
//! | `memory` | memory | the linear memory the messages go through |
//! | `kc_alloc` | `(len: i32) -> i32` | a buffer of `len` bytes the host may write a request into |
//! | `kc_call` | `(ptr: i32, len: i32) -> i64` | handle the request at `ptr`; return `(reply_ptr << 32) \| reply_len` |
//! | `kc_free` | `(ptr: i32, len: i32)` | optional; the host calls it for the request and for the reply once read |
//!
//! Request: `{"api":1,"op":"<name>","data":{...}}`. Reply: `{"ok":true,"data":{...}}` or
//! `{"ok":false,"error":"<words for the person>"}`. An `ok:false` reply is an ordinary refusal
//! (the add-on stays on); anything that is not a reply is a failure (the add-on stops).
//!
//! Ops the program sends: `hello` (once, at load; the engine answers `{"api":1}`), `command`
//! (`{"id":"<command>"}`) and `button` (`{"panel":"<panel>","id":"<button>"}`). Data an op may
//! answer with: `{"lamps":{"<id>":{"state":"ok","lit":true}},"notice":"text"}`. Lane C3 adds ops
//! for the controller; the transport does not change.

use serde_json::{Value, json};
use wasmi::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    TypedFunc,
};

use crate::error::{AddonError, Result};

/// The host API generation this program speaks (`[engine] host_api` of the manifest).
pub const HOST_API: u32 = 1;

/// Largest `engine.wasm`: 4 MiB. A real engine is a few hundred kilobytes; a bigger file is
/// refused when the package is read and again when the engine loads.
pub const MAX_ENGINE_BYTES: usize = 4 * 1024 * 1024;

/// Fuel one `kopercad.log` call costs (a call copies up to 512 bytes and keeps a line).
const LOG_FUEL: u64 = 2_000;

/// Budgets of one engine. The defaults suit a controller or a drive model stepped every few
/// milliseconds; the tests lower them.
#[derive(Debug, Clone)]
pub struct HostLimits {
    /// Interpreter steps one call (and the start-up) may use. At the default an endless loop
    /// ends in a fraction of a second.
    pub fuel_per_call: u64,
    /// Most bytes of linear memory.
    pub memory_bytes: usize,
    /// Largest request or reply.
    pub max_message: usize,
    /// Deepest call nesting inside the engine.
    pub recursion_depth: usize,
    /// Most bytes of log text kept (older lines drop first).
    pub log_bytes: usize,
}

impl Default for HostLimits {
    fn default() -> Self {
        HostLimits {
            fuel_per_call: 50_000_000,
            memory_bytes: 32 * 1024 * 1024,
            max_message: 1024 * 1024,
            recursion_depth: 512,
            log_bytes: 8 * 1024,
        }
    }
}

/// Why a call did not return data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CallError {
    /// The engine said no (`ok:false`). The add-on stays on.
    Refused(String),
    /// The add-on is stopped for this session, now or earlier; the text says why.
    Stopped(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CallError::Refused(t) => write!(f, "{t}"),
            CallError::Stopped(t) => write!(f, "stopped: {t}"),
        }
    }
}

impl std::error::Error for CallError {}

struct State {
    limits: StoreLimits,
    log: std::collections::VecDeque<String>,
    log_bytes: usize,
    log_cap: usize,
}

/// A loaded engine.
pub struct EngineHost {
    store: Store<State>,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    call: TypedFunc<(i32, i32), i64>,
    free: Option<TypedFunc<(i32, i32), ()>>,
    limits: HostLimits,
    stopped: Option<String>,
    calls: u64,
    fuel_used: u64,
}

fn engine_error(why: impl std::fmt::Display) -> AddonError {
    AddonError::Engine(why.to_string())
}

/// Plain words for what stopped the engine.
fn why_stopped(e: &wasmi::Error, limits: &HostLimits) -> String {
    use wasmi::TrapCode;
    match e.as_trap_code() {
        Some(TrapCode::OutOfFuel) => format!(
            "it used more than its step budget of {} steps in one call (an endless loop?)",
            limits.fuel_per_call
        ),
        Some(TrapCode::StackOverflow) => "it called itself too deeply".to_owned(),
        Some(code) => format!("it hit an error and stopped ({code})"),
        None => format!("it failed ({e})"),
    }
}

impl EngineHost {
    /// Loads `wasm` (binary only), checks what it imports and exports, starts it and shakes
    /// hands (`hello`). Nothing of the engine runs outside the budgets.
    pub fn load(wasm: &[u8], limits: HostLimits) -> Result<EngineHost> {
        if wasm.len() > MAX_ENGINE_BYTES {
            return Err(engine_error(format!(
                "engine.wasm is larger than {} MiB",
                MAX_ENGINE_BYTES / (1024 * 1024)
            )));
        }
        let mut config = Config::default();
        config
            .consume_fuel(true)
            .set_max_recursion_depth(limits.recursion_depth)
            .wasm_multi_memory(false)
            .wasm_custom_page_sizes(false)
            .wasm_wide_arithmetic(false)
            .ignore_custom_sections(true);
        let engine = Engine::new(&config);
        if wasm.starts_with(b"\0asm") {
            // binary: fine
        } else {
            return Err(engine_error("engine.wasm is not a WebAssembly binary"));
        }
        let module = Module::new(&engine, wasm)
            .map_err(|e| engine_error(format!("engine.wasm is not a valid module: {e}")))?;
        for import in module.imports() {
            if !(import.module() == "kopercad" && import.name() == "log") {
                return Err(engine_error(format!(
                    "the engine asks the host for `{}.{}`, which this version of KoperCAD does not give (add-ons cannot use files, network, clock or threads)",
                    import.module(),
                    import.name()
                )));
            }
        }
        let state = State {
            limits: StoreLimitsBuilder::new()
                .memory_size(limits.memory_bytes)
                .table_elements(10_000)
                .instances(1)
                .memories(1)
                .tables(4)
                .build(),
            log: std::collections::VecDeque::new(),
            log_bytes: 0,
            log_cap: limits.log_bytes,
        };
        let mut store = Store::new(&engine, state);
        store.limiter(|s| &mut s.limits);
        store.set_fuel(limits.fuel_per_call).map_err(engine_error)?;
        let mut linker = <Linker<State>>::new(&engine);
        linker
            .func_wrap(
                "kopercad",
                "log",
                |mut caller: Caller<'_, State>,
                 ptr: i32,
                 len: i32|
                 -> std::result::Result<(), wasmi::Error> {
                    // A host call is not free: it costs fuel like the steps around it, so a loop
                    // of log calls ends as fast as any other loop.
                    let left = caller.get_fuel().unwrap_or(0);
                    if left < LOG_FUEL {
                        let _ = caller.set_fuel(0);
                        return Err(wasmi::Error::from(wasmi::TrapCode::OutOfFuel));
                    }
                    let _ = caller.set_fuel(left - LOG_FUEL);
                    let Some(memory) = caller.get_export("memory").and_then(Extern::into_memory)
                    else {
                        return Ok(());
                    };
                    let len = (len.max(0) as usize).min(512);
                    let mut buf = vec![0u8; len];
                    if memory.read(&caller, ptr.max(0) as usize, &mut buf).is_err() {
                        return Ok(());
                    }
                    let line = String::from_utf8_lossy(&buf).into_owned();
                    let state = caller.data_mut();
                    state.log_bytes += line.len();
                    state.log.push_back(line);
                    while state.log_bytes > state.log_cap {
                        match state.log.pop_front() {
                            Some(old) => state.log_bytes -= old.len(),
                            None => break,
                        }
                    }
                    Ok(())
                },
            )
            .map_err(engine_error)?;
        let instance = linker
            .instantiate_and_start(&mut store, &module)
            .map_err(|e| {
                engine_error(format!(
                    "the engine could not start: {}",
                    why_stopped(&e, &limits)
                ))
            })?;
        let memory = instance
            .get_memory(&store, "memory")
            .ok_or_else(|| engine_error("the engine does not export `memory`"))?;
        let alloc = instance
            .get_typed_func::<i32, i32>(&store, "kc_alloc")
            .map_err(|_| engine_error("the engine does not export `kc_alloc(i32) -> i32`"))?;
        let call = instance
            .get_typed_func::<(i32, i32), i64>(&store, "kc_call")
            .map_err(|_| engine_error("the engine does not export `kc_call(i32, i32) -> i64`"))?;
        let free = instance
            .get_typed_func::<(i32, i32), ()>(&store, "kc_free")
            .ok();
        let mut host = EngineHost {
            store,
            memory,
            alloc,
            call,
            free,
            limits,
            stopped: None,
            calls: 0,
            fuel_used: 0,
        };
        match host.call("hello", &json!({})) {
            Ok(data) if data.get("api").and_then(Value::as_u64) == Some(u64::from(HOST_API)) => {
                Ok(host)
            }
            Ok(data) => Err(engine_error(format!(
                "the engine speaks another host API ({}); this program speaks {HOST_API}",
                data.get("api")
                    .map_or("none".to_owned(), ToString::to_string)
            ))),
            Err(CallError::Refused(why)) => {
                Err(engine_error(format!("the engine refused to start: {why}")))
            }
            Err(CallError::Stopped(why)) => {
                Err(engine_error(format!("the engine failed at start: {why}")))
            }
        }
    }

    /// Sends one request and returns the reply's `data`.
    pub fn call(&mut self, op: &str, data: &Value) -> std::result::Result<Value, CallError> {
        if let Some(why) = &self.stopped {
            return Err(CallError::Stopped(why.clone()));
        }
        let request = serde_json::to_vec(&json!({"api": HOST_API, "op": op, "data": data}))
            .map_err(|e| CallError::Refused(e.to_string()))?;
        if request.len() > self.limits.max_message {
            return Err(CallError::Refused("the request is too large".to_owned()));
        }
        self.calls += 1;
        match self.run(&request) {
            Ok(reply) => match reply.get("ok").and_then(Value::as_bool) {
                Some(true) => Ok(reply.get("data").cloned().unwrap_or(Value::Null)),
                Some(false) => Err(CallError::Refused(
                    reply
                        .get("error")
                        .and_then(Value::as_str)
                        .unwrap_or("the engine said no")
                        .chars()
                        .take(300)
                        .collect(),
                )),
                None => self.stop("its reply has no `ok`".to_owned()),
            },
            Err(why) => self.stop(why),
        }
    }

    fn stop(&mut self, why: String) -> std::result::Result<Value, CallError> {
        self.stopped = Some(why.clone());
        Err(CallError::Stopped(why))
    }

    fn run(&mut self, request: &[u8]) -> std::result::Result<Value, String> {
        let result = self.run_inner(request);
        // the call that stops counts too
        let left = self.store.get_fuel().unwrap_or(0);
        self.fuel_used += self.limits.fuel_per_call.saturating_sub(left);
        result
    }

    fn run_inner(&mut self, request: &[u8]) -> std::result::Result<Value, String> {
        let limits = self.limits.clone();
        let fail = |e: wasmi::Error| why_stopped(&e, &limits);
        self.store
            .set_fuel(limits.fuel_per_call)
            .map_err(|e| e.to_string())?;
        let len =
            i32::try_from(request.len()).map_err(|_| "the request is too large".to_owned())?;
        let ptr = self.alloc.call(&mut self.store, len).map_err(fail)?;
        self.memory
            .write(&mut self.store, ptr as u32 as usize, request)
            .map_err(|_| "its buffer for the request lies outside its memory".to_owned())?;
        let packed = self.call.call(&mut self.store, (ptr, len)).map_err(fail)?;
        let reply_ptr = (packed >> 32) as u32;
        let reply_len = (packed & 0xffff_ffff) as u32;
        if reply_len as usize > limits.max_message {
            return Err(format!(
                "its reply is larger than {} bytes",
                limits.max_message
            ));
        }
        let mut reply = vec![0u8; reply_len as usize];
        self.memory
            .read(&self.store, reply_ptr as usize, &mut reply)
            .map_err(|_| "its reply lies outside its memory".to_owned())?;
        if let Some(free) = self.free {
            free.call(&mut self.store, (ptr, len)).map_err(fail)?;
            free.call(&mut self.store, (reply_ptr as i32, reply_len as i32))
                .map_err(fail)?;
        }
        serde_json::from_slice::<Value>(&reply)
            .map_err(|e| format!("its reply is not valid JSON ({e})"))
    }

    /// Why the engine stopped, if it did. A stopped engine answers every call with this.
    pub fn stopped(&self) -> Option<&str> {
        self.stopped.as_deref()
    }

    /// Lines the engine wrote with `kopercad.log` (the most recent few kilobytes).
    pub fn log(&self) -> Vec<String> {
        self.store.data().log.iter().cloned().collect()
    }

    /// Requests sent so far, `hello` included.
    pub fn calls(&self) -> u64 {
        self.calls
    }

    /// Interpreter steps used in finished calls.
    pub fn fuel_used(&self) -> u64 {
        self.fuel_used
    }
}
