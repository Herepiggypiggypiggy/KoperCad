# NovoElectric

A lightweight, local-first electrical CAD program (schematics, terminal and
cable connection screens, cross references, cabinet layout, simulation).
Native Rust, no web technology. Python is the scripting and integration layer
(WS-28), never the core.

Roadmap: https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e
(summary in `docs/ROADMAP.md`).

## Build and check

```
cargo build                      # debug build
cargo run -p novo-app            # launch the app
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs fmt, clippy (warnings are errors) and tests on Linux and Windows.

## How parallel work is organised

Work is split into workstreams (WS-xx), each run by its own Claude thread.

| Crate | Owner | Purpose |
| --- | --- | --- |
| `novo-core` | WS-02 | Data model: the single source of truth |
| `novo-canvas` | WS-03 | GPU canvas: pan, zoom, snap, hit testing, selection |
| `novo-storage` | WS-04 | SQLite project file, autosave, migrations |
| `novo-symbols` | WS-05 | IEC 60617 symbols, connection points, behavior slots |
| `novo-sim` | WS-19 | Simulation engine (Phase 4) |
| `novo-app` | WS-01 | Application shell, wiring crates together |

Rules:

- One branch and one pull request per workstream (the thread's assigned
  branch is fine); name the workstream (WS-xx) in the PR title. Keep pull
  requests focused on your own crate.
- Only the WS-02 thread changes the public API of `novo-core`. Other threads
  that need a model change describe it in their pull request or ask WS-02.
- Other crates read the model through queries and change it only through
  commands (undo/redo, scripts and AI edits share that path).
- New crates are added under `crates/` and picked up by the workspace glob;
  shared dependencies go in `[workspace.dependencies]`.
- `unsafe` is denied workspace-wide; justify any exception in the pull request.
- Everything must work without admin rights: install, run, update and file
  associations stay per-user (`%LOCALAPPDATA%`, `HKCU`), and nothing may
  request elevation.

## Conventions

- Device tags follow IEC 81346: `=plant +location -device`.
- Symbols follow IEC 60617 first; NFPA comes later as a standards pack.
- Units: millimetres for geometry, SI units for electrical values.
