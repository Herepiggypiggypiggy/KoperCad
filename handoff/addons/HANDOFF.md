# Add-ons (lane C0b) handoff

Branch `claude/project-thread-25hpzk`, based on main after #1574 (registration). C0b-1 PR: see PROGRESS.md for the number and head. Guide for everyone: `docs/design/addons.md`.

## State
- C0b-1 is complete: `novo-addons` (format, trust, store, extension tables), Manage add-ons window (`crates/novo-app/src/app/addons.rs`, tests in `addons_tests.rs`), `addons.manage` command and palette rows, `kopercad-cli addons ...`, Python `addons_*`, fixtures in `qa/fixtures/addons`, QA scenario `qa/scenarios/560-addons-manager.qa` (passes at 1366x768 and 1920x1080; pictures in `/mnt/project-files/qa/addons-c0b1/`).
- Not done: C0b-2 (wasmi host). `[engine]` is parsed and checked only; buttons in add-on panels do nothing until it lands.
- Windows is not verified (built and tested on Linux only).

## Resume
```
cargo test -p novo-addons -p novo-workflow -p novo-cli -p novo-py -p novo-tests --test addons_fixtures
cargo test -p novo-app addons
KOPERCAD_REGENERATE=1 cargo test -p novo-tests --test addons_fixtures   # after editing qa/fixtures/addons/src
scripts/qa.sh --only addons
```

## Decisions already made
- Zip `.kcaddon`, SHA-256 of every file in the manifest, optional Ed25519 signature over the manifest; unsigned needs the "I trust this file" tick; `BUILTIN_KEYS` is empty until the release thread makes the KoperVonk key (never in the repo).
- Rows follow `docs/design/visual-redesign-addons.md`: at most 4 per menu area, 6 per rail flyout; add-on icons are shape data on the 24 grid; panels are data drawn by `panel.rs`, no colours.
- Store: `<data_local_dir>/KoperCAD/addons` (the `KOPERCAD_PROFILE_DIR` override applies); atomic registry; one previous version kept.
- Code that offers an add-on feature asks `NovoApp::addon_enabled(id)`.

## Follow-ups (see ROADMAP.md)
C0b-2 wasm host; gate the controller import wizard once the controller add-on exists; project data blocks that survive a missing add-on (needs WS-02/WS-04); rail slot, menu footer, Ctrl K chip, drawn icons (V3a, V2b, V5, V0); signing key and pack in CI.

## Traps
- Cloud disk fills during a workspace build: `scripts/free-disk.sh`, use `CARGO_INCREMENTAL=0` and delete test binaries in the target dir.
- `Panel::new` takes only `&'static str` ids, so add-on panels use `panel::dialog(...).id(...)`.
