//! The controller add-on's engine: `novo-controller` behind host API 1.
//!
//! [`Engine::handle`] takes one request text (`{"api":1,"op":"...","data":{...}}`) and returns
//! the reply text; it is plain Rust, so the tests run natively. On `wasm32` the module in
//! `abi` exports the three functions the host calls (`kc_alloc`, `kc_call`, `kc_free`).
//!
//! Ops (all keyed by instance id, signals as `instance/signal`):
//!
//! | Op | Data | Answer |
//! | --- | --- | --- |
//! | `hello` | none | `{"api":1}` |
//! | `profile.add` | `{"toml": "<profile text>"}` | `{"id","name"}`; refusal lists the problems |
//! | `instance.add` | `{"id","label","profile"}` | snapshot |
//! | `instance.duplicate` | `{"from","id","label"}` | snapshot |
//! | `instance.remove` | `{"id"}` | snapshot |
//! | `instance.rename` | `{"id","label"}` | snapshot |
//! | `act` | `{"instance", "action": {"action":"press","control":"..."}}` | `{"applied","snapshot"}` |
//! | `advance` | `{"dt_ms"}` | snapshot |
//! | `snapshot` | none | every instance, control position and signal level |
//! | `records.save` / `records.load` | instance records | the project data and a snapshot |
//! | `record.start` / `record.stop` | `{"name"}` for stop | `{"sequence":"<toml>"}` |
//! | `replay` | `{"sequence":"<toml>","sample_ms","settle_ms"}` | the trace; the live set is not touched |
//! | `button` | `{"panel","id"}` | `lamps` for the panel (see below) |
//!
//! The `button` op serves the add-on's declared panel `radio`: ids `press:<control>` and
//! `release:<control>` act on the first instance; the answer carries `lamps` for every signal.

mod ops;

#[cfg(target_arch = "wasm32")]
mod abi;

pub use ops::Engine;

/// The host API generation this engine speaks.
pub const API: u32 = 1;
