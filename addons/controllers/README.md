# The controller add-on (lane C3)

A virtual radio for the live view, packaged as an add-on (`controllers-<version>.kcaddon`). The
sandboxed engine is `novo-controller` (the C1 crate) behind add-on host API 1; the panel is data
(add-on API 2 widgets: rockers with steps, a rotary selector, tabs, a key map and a table) that the program draws; the generic radio profile is data too. Guide to add-ons:
`docs/design/addons.md`. Plan: `plans/controller-live-view.md` section 5b (project files).

| Folder | What |
| --- | --- |
| `engine/` | Standalone Rust project (own workspace and lock file) that builds to `wasm32-unknown-unknown`. `src/ops.rs` is the request handler (plain Rust, tested natively in `tests/ops.rs`); `src/abi.rs` is the three host exports and builds only for wasm |
| `pack/` | What goes into the package besides the engine: `manifest.toml`, `data/radio.panel.toml`, `data/generic-radio-3x2.controller.toml`, `data/icons.toml`, `LICENSE` |

## Build

```
rustup target add wasm32-unknown-unknown
scripts/build-controllers-addon.sh          # engine.wasm (cap 4 MiB) and target/addons/controllers-<version>.kcaddon
cargo test -p novo-tests --test controllers_addon -- --nocapture   # the package, installed and driven through the host
kopercad-cli addons install target/addons/controllers-0.2.0.kcaddon --trust-unsigned
```

CI job `controllers add-on` does the same on every pull request that touches an add-on part and
keeps the package as an artifact. Signing: `KOPERCAD_SIGN_KEY_ID` and `KOPERCAD_SIGN_SEED_FILE`
(the release thread; no key is in the repository).

## Where the user finds it

Install in File > Add-ons... (Ctrl+K "Add-ons"), turn it on, then **Simulate > Radio panel**
(Ctrl+K "Radio panel") opens the Radio panel. It has three tabs: Motions (hoist select as a
rotary, a two-step spring rocker per motion, lamps for the signals the engine answers with),
Keys (the keyboard map) and Contacts (what a two-step rocker closes). Every control sends a
`button` request such as `set-hoist-up2` to the engine. Needs KoperCAD with add-on API 2 (an older
program refuses the package and says so).

## What the engine does (host API 1 ops)

See the table at the top of `engine/src/lib.rs`: profiles, several instances, actions, time,
snapshot, project records (save and load), record and replay (deterministic, on a copy), and the
panel `button` op. The engine starts with the bundled generic radio and one instance `radio`.

## Honest limits

- It simulates wiring and logic of a generic radio, not a radio protocol, receiver firmware or any
  vendor product.
- The panel acts on one instance (`radio`). Several panels, the signal-to-drawing binding and the
  live-view card are the C2 and C3 UI work that follows. The live-view controllers (instances and
  bindings) are stored in the project's add-on data block `controllers`/`project` and stay there
  when this add-on is not installed.
- Keyboard map, gamepad, joystick and tabs wait for the extra panel widgets (see addons.md).
