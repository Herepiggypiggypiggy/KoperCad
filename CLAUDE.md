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
| `novo-terminals` | WS-11 | Terminal strips, same-name join rule, terminal connection screens |
| `novo-sim` | WS-19 | Simulation engine (Phase 4) |
| `novo-theme` | Layout and branding | Dark theme, design tokens, icons, logo (`assets/branding`) |
| `novo-app` | WS-01 | Application shell, wiring crates together |

Rules:

- One branch and one pull request per workstream (`ws-02-data-model`,
  `ws-03-canvas`, ...). Keep pull requests focused on your own crate.
- Only the WS-02 thread changes the public API of `novo-core`. Other threads
  that need a model change describe it in their pull request or ask WS-02.
- Other crates read the model through queries and change it only through
  commands (undo/redo, scripts and AI edits share that path).
- New crates are added under `crates/` and picked up by the workspace glob;
  shared dependencies go in `[workspace.dependencies]`.
- `unsafe` is denied workspace-wide; justify any exception in the pull request.
- The integration lead thread merges pull requests once the main reviewer says
  READY on the current head and CI is green. Don't ask the owner to review or
  merge.
- Once a pull request is marked READY, don't push to it again. Follow-up work
  goes in a new pull request from main. After every push, message the reviewer
  with the new head SHA. The thread roster with session ids is in
  `/mnt/project-files/coordination/threads.md`.
- Until `docs/DESIGN.md` is merged, build engines and commands and keep UI code
  thin.

## Experimental builds

Every push to main and every pull request update rebuilds the `experimental`
pre-release: main plus all open pull requests merged in PR order (conflicting
PRs are skipped; if the combined build fails it falls back to main). The
owner's desktop shortcut runs `scripts/run-latest.ps1`, which always starts
the newest of these builds. Keep main and open PRs buildable on Windows.

## No admin rights, ever

Everything must install, run, update and be scripted by a standard Windows
user without admin rights. That covers the app, its installer
(`PrivilegesRequired=lowest`, per-user install), launchers, the Python layer
(WS-28) and any helper tool. Write only under the user's profile
(`%LOCALAPPDATA%`, `%APPDATA%`, Documents) and `HKCU`; never `Program Files`,
`HKLM`, services, drivers or system-wide installs, and never require a tool
that needs elevation to install.

## Conventions

- Device tags follow IEC 81346: `=plant +location -device`.
- Symbols follow IEC 60617 first; NFPA comes later as a standards pack.
- Units: millimetres for geometry, SI units for electrical values.
