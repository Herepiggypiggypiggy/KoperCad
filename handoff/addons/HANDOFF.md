# Add-ons (lane C0b) handoff

Branch `claude/project-thread-25hpzk`, based on main after #1574 (registration). C0b-1 PR: see PROGRESS.md for the number and head. Guide for everyone: `docs/design/addons.md`.

## State
- C0b-1 is complete: `novo-addons` (format, trust, store, extension tables), Manage add-ons window (`crates/novo-app/src/app/addons.rs`, tests in `addons_tests.rs`), `addons.manage` command and palette rows, `kopercad-cli addons ...`, Python `addons_*`, fixtures in `qa/fixtures/addons`, QA scenario `qa/scenarios/560-addons-manager.qa` (passes at 1366x768 and 1920x1080; pictures in `/mnt/project-files/qa/addons-c0b1/`).
- C0b-2 (this branch `claude/c0b-2-wasm-host`): sandboxed engine host `crates/novo-addons/src/host.rs` (wasmi, host API 1: `kc_alloc`/`kc_call`/`kc_free`, one import `kopercad.log`, step budget, memory/stack caps, a stop takes the add-on out of menus and panels for the session), app wiring (panel buttons send `button`, commands without a panel send `command`, replies set lamps and notices, Details shows why it stopped), fixture `demo-engine` (WebAssembly text in `qa/fixtures/addons/engine-src`), tests `crates/novo-addons/tests/host.rs`, QA scenario 561.
- Not done: the controller engine as `wasm32-unknown-unknown` (lane C3) and `scripts/pack-addon` in CI.
- Windows is not verified (built and tested on Linux only).

## Resume
```
cargo test -p novo-addons -p novo-workflow -p novo-cli -p novo-py -p novo-tests --test addons_fixtures
cargo test -p novo-app addons
KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures   # after editing qa/fixtures/addons/src
scripts/qa.sh --only addons      # scenarios 560 and 561
```

## Decisions already made
- Zip `.kcaddon`, SHA-256 of every file in the manifest, optional Ed25519 signature over the manifest; unsigned needs the "I trust this file" tick; `BUILTIN_KEYS` is empty until the release thread makes the KoperVonk key (never in the repo).
- Rows follow `docs/design/visual-redesign-addons.md`: at most 4 per menu area, 6 per rail flyout; add-on icons are shape data on the 24 grid; panels are data drawn by `panel.rs`, no colours.
- Store: `<data_local_dir>/KoperCAD/addons` (the `KOPERCAD_PROFILE_DIR` override applies); atomic registry; one previous version kept.
- Code that offers an add-on feature asks `NovoApp::addon_enabled(id)`.

## Follow-ups (see ROADMAP.md)
gate the controller import wizard once the controller add-on exists; project data blocks that survive a missing add-on (needs WS-02/WS-04); rail slot, menu footer, Ctrl K chip, drawn icons (V3a, V2b, V5, V0); signing key and pack in CI.

## Traps
- Cloud disk fills during a workspace build: `scripts/free-disk.sh`, use `CARGO_INCREMENTAL=0` and delete test binaries in the target dir.
- `Panel::new` takes only `&'static str` ids, so add-on panels use `panel::dialog(...).id(...)`.
- wasmi needs the `portable-dispatch` feature (set in the workspace dependency): without it a debug build overflows the thread stack in the interpreter loop.
- Test engines are WebAssembly text built with the `wat` crate in tests only; the program accepts binary modules only.
