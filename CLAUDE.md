# KoperCAD

Product KoperCAD, maker KoperVonk (formerly NovoElectric; the `novo-*` crates, the `novoelectric` Python
module and the release asset name keep the old name, see README; the repository is `kopercad`).

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

CI runs fmt, clippy (warnings are errors) and tests on Linux for every pull
request, and on Windows for main and nightly.

## How parallel work is organised

Work is split into workstreams (WS-xx), each run by its own Claude thread.

The crate list below is fixed and already registered in `[workspace.dependencies]`;
pull requests never edit it or those lines (that used to make every new-crate PR
conflict with every other). A crate that is not on the list is added by the
coordinator in its own small PR first.

| Crate | Owner | Purpose |
| --- | --- | --- |
| `novo-core` | WS-02 | Data model: the single source of truth |
| `novo-canvas` | WS-03 | GPU canvas: pan, zoom, snap, hit testing, selection, marks, page frame |
| `novo-storage` | WS-04 | SQLite project file, autosave, migrations |
| `novo-symbols` | WS-05 | IEC 60617 symbols, connection points, behavior slots |
| `novo-editor` | WS-06 | Schematic editor: tools, auto tags, wiring, potentials |
| `novo-parts` | WS-07 | Part catalog, part assignment, property sheet model, bulk edit |
| `novo-workflow` | WS-08 | Keyboard workflow: keymap, commands, palette search, R/X orientation, smart duplicate |
| `novo-pdf` | WS-09 | Vector PDF export: pages, title block, bookmarks, links |
| `novo-xref` | WS-10 | Cross references: contact images, jumps, linking |
| `novo-terminals` | WS-11 | Terminal strips, same-name join rule, terminal connection screens |
| `novo-cables` | WS-12 | Cables: proposals at location boundaries, cores, shields, cable screens |
| `novo-reports` | WS-13 | Generated pages and reports: BOM, device list, wire list, contents, Excel |
| `novo-checks` | WS-14 | Design checks: problems with severity and location |
| `novo-layout` | WS-15 | Cabinet layout (Phase 3) |
| `novo-routing` | WS-16 | Wire routing through ducts (Phase 3) |
| `novo-plc` | WS-17 | PLC and I/O (Phase 3) |
| `novo-revisions` | WS-18 | Revisions and visual diff (Phase 3) |
| `novo-sim` | WS-19 | Simulation engine (Phase 4) |
| `novo-ai` | WS-23 | AI assistant (Phase 5) |
| `novo-interop` | WS-24 | Import and export: DXF, EPLAN parts, Excel round trip (Phase 5) |
| `novo-cli` | WS-28 | `kopercad-cli` command-line tool |
| `novo-py` | WS-28 | Python bindings (PyO3) |
| `novo-theme` | Layout and branding | Dark theme, design tokens, icons, logo (`assets/branding`) |
| `novo-view3d` | WS-29 | 3D cabinet view: renderer, scene from cabinet data, camera (design `docs/design/3d-view.md`) |
| `novo-commissioning` | Commissioning | Commissioning mode: checks, progress and punch list (pure engine; design PR #399) |
| `novo-learn` | Learning | Learning modules engine: lessons, steps, checks, progress (design `docs/design/learning-modules.md`) |
| `novo-heatview` | Thermal visuals | Heat field engine: contours, hot spots, streamlines, compare, synthetic field (pure, no UI deps) |
| `novo-thermal` | Thermal simulation | Physical heat and airflow simulation engine (pure, depends on novo-layout) |
| `novo-numbering` | Wire numbering | Wire and potential numbering: schemes, stable fill, renumber, locks, conflicts (design `docs/design/wire-numbering.md`) |
| `novo-tests` | Tester | Cross-crate tests and the Gate 1 fixture |
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
- Every thread keeps `handoff/<thread>/ROADMAP.md`, `HANDOFF.md` and
  `PROGRESS.md` current (convention in `handoff/README.md`; the project-wide
  handoff is `handoff/MAIN-HANDOFF.md`). The copy in this repository is what
  cloud sessions without the project mount read, so commit your updates here
  too.
- Until `docs/DESIGN.md` is merged, build engines and commands and keep UI code
  thin.

## Experimental builds

Every two hours, when main or an open pull request has changed (and on manual
dispatch), the `experimental` pre-release is rebuilt: main plus all open pull
requests merged in PR order (conflicting or non-compiling PRs are skipped).
It is not rebuilt per push, to stay inside the free Actions minutes. CI runs
Linux on every PR and main push; Windows only on main pushes and nightly. The
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
