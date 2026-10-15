<p align="center">
  <img src="assets/branding/lockup.png" alt="KoperCAD by KoperVonk" width="420">
</p>

# KoperCAD

A lightweight, local-first electrical CAD program by **KoperVonk**: schematics, terminal and cable
connection screens, cross references, generated reports, PLC and I/O, revisions, and (later) cabinet
layout and circuit simulation. Native Rust, no web technology, fully offline, scriptable from Python.

> **Name.** The product is KoperCAD and the maker is KoperVonk (decided 2026-10-15); it was called
> NovoElectric before. Names people never see keep the old name for now: the repository, the `novo-*`
> crates, `novoelectric.exe`, the `novoelectric` Python module, the release asset name and the data
> folders (`%LOCALAPPDATA%\NovoElectric`, `Documents\NovoElectric`, `%APPDATA%\NovoElectric`). Moving
> the data folders needs a migration from the old location first.

It is a design of its own, aimed at what EPLAN Electric P8 and AutoCAD Electrical do, not a copy of either.

## Status at a glance (2026-10-15)

| Area | State |
| --- | --- |
| Phase 0 Foundation (gate 0) | Done |
| Phase 1 Schematics (gate 1: motor starter over two pages, saved, printed to PDF) | Done |
| Phase 2 Intelligence (gate 2: terminal, cable and report pages generate live) | Met on Linux, per the tester's report |
| Phase 3 Physical design | Started: PLC and I/O, revisions. Cabinet layout and routing not started |
| Phase 4 Simulation | Not started (placeholder crate) |
| Phase 5 AI and interop | Not started |

What is verified and what is not:

- **Verified on Linux:** `cargo fmt`, `cargo clippy -D warnings` and all workspace tests pass on main
  (about 860 tests, 0 failing, a few ignored). Every merge was checked this way.
- **Windows:** CI on the Windows runner was only just switched on (the repository went public, so
  Actions minutes are free). Windows clippy passes; the first full Windows test run on main was still
  pending when this was written. The app has run on a Windows PC (canvas benchmark, installer test)
  but is not yet verified end to end there.
- **Not yet done:** the in-app dialogs for several finished engines (see below), the full set of
  app-side revision features, and the cabinet, routing and simulation phases.

The newest test round is in `docs/TEST_REPORT.md`. The hand-over state is in `handoff/MAIN-HANDOFF.md`.

## What works today

Engines and data (all in the workspace, tested):

- **Data model** (`novo-core`): one source of truth for projects, pages, symbols, wires, nets,
  devices, tags (IEC 81346, `=plant +location -device`), potentials, structure, blocks, page freezing,
  undo and redo through commands that scripts and the AI share.
- **Project file** (`novo-storage`): a single SQLite file with autosave, crash recovery, migrations
  (schema v2) and revision snapshots. A 10,000-object page opens in about a quarter of a second.
- **Canvas** (`novo-canvas`): GPU drawing with pan, zoom, snap, hit testing, selection, marks and
  the page frame. Benchmarked at 10,000 objects at over 1,000 fps on an old workstation GPU.
- **Symbols** (`novo-symbols`): IEC 60617 symbol library with connection points, letter codes per
  IEC 81346-2:2019 through one table, and behaviour slots for the later simulation.
- **Schematic editor** (`novo-editor`): tools, automatic tags, wiring, potentials, moves that keep
  wires connected, and the two wiring modes (below).
- **Parts** (`novo-parts`): catalogue, part assignment, property sheet model and bulk edit.
- **Keyboard workflow** (`novo-workflow`): keymap, commands, palette search, R and X orientation,
  smart duplicate.
- **Cross references** (`novo-xref`): contact images, jumps between coil and contacts, linking.
- **Terminals and cables** (`novo-terminals`, `novo-cables`): terminal strips with the same-name
  join rule, terminal connection screens, cable proposals at location boundaries, cores, shields and
  cable screens.
- **PDF** (`novo-pdf`): vector export with pages, title block, bookmarks, links and embedded Unicode text.
- **Reports and checks** (`novo-reports`, `novo-checks`): bill of materials, device list, wire list,
  contents, terminal, cable and I/O list reports as generated pages (sheets `20a`, `20b`, ...),
  Excel and CSV export, and design checks with severity and location.
- **PLC and I/O** (`novo-plc`): cards, channel indexing, address labels, free channels, I/O list rows,
  add, move and renumber commands, and PLC problems in the Issues list.
- **Revisions** (`novo-revisions`): revision index rules, issuing a revision with page freezing,
  snapshots in the project file, change sets per page, page revisions for the title block, marking
  generated sheets whose content changed, and one `issue_revision` call.
- **CLI and Python** (`novo-cli`, `novo-py`): a first `novo` command-line tool and Python bindings.

Application (`novo-app`):

- Schematic editing with the dark theme, command palette, outline with an Issues list, generated
  report pages that follow edits live, page labels per sheet, and new or opened projects getting their
  generated pages (skipped for an issued project with frozen pages).

### Two wiring modes

Wiring has two modes, switched with **Shift+W** (also in the middle of a wire) and a chip next to
Connect in the tool dock:

- **Corners** (default, EPLAN style): you place corner points, or drag from pin to pin and the router
  finds a path.
- **Lines** (AutoCAD style): you draw straight line segments yourself.

In both modes wires of different conductors never share length. The router nudges its own legs; a
corner or segment that would overlap is refused with the reason; an interactive symbol move that cannot
be nudged clear is refused. Only paths that cannot ask (scripts, AI edits, undo, opening a file)
accept an overlap, and they report it as a `wire.overlap` warning. The design is in
`docs/DESIGN.md` and `docs/design/wiring-modes.md`. The engine is on main; the dock chip is in
progress.

## Roadmap and phases

The full roadmap with dependencies and done-criteria is the artifact linked from `docs/ROADMAP.md`.

| Phase | Workstreams | Gate at the end |
| --- | --- | --- |
| 0 Foundation | App shell and CI, data model, canvas engine, project file | Model API v0 frozen, canvas draws it |
| 1 Schematics | Symbol library, schematic editor, parts, fast workflow, PDF and print | Motor starter drawn over 2 pages, saved, printed to PDF |
| 2 Intelligence | Cross references, terminal strips, cables, reports, design checks, Python API and CLI | Terminal, cable and report pages generate live |
| 3 Physical design | Cabinet layout, wire routing, PLC and I/O, revisions | |
| 4 Simulation | Simulation engine, device models, animation, fault scenarios | Star-delta starter simulates and animates |
| 5 AI and interop | AI assistant, import and export (DXF, parts, Excel round trip), standards packs, 3D cabinet, release polish | |

## Planned features

- **Revisions in the app:** the Issue revision dialog and command line, a Changes mode with marks on
  the drawing, and revision fields in the title block (the engine is done).
- **PLC in the app:** dialogs and commands mapped onto the finished PLC engine.
- **Cabinet layout** (`novo-layout`): mounting plate and rail layout with hatches, dimensions and
  snapping to rails and ducts. The canvas primitives for it are on main.
- **Wire routing** (`novo-routing`): routing wires through ducts.
- **Simulation** (`novo-sim`): connectivity-based solving of energised conductors, coil and contact
  states and timers, with animation and fault scenarios.
- **AI assistant** (`novo-ai`) that edits through the same command path as the user.
- **Import and export** (`novo-interop`): DXF, EPLAN parts, Excel round trip.
- **Standards packs:** NFPA after IEC 60617.
- **Python layer:** more of the app scriptable (Python is the integration layer, never the core).
- The rest of the **rename**: the repository, executable, Python module and data folders (with a migration).

## Build and run

```
cargo build                      # debug build
cargo run -p novo-app            # launch the app
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs fmt, clippy and tests on Linux for every pull request, and Linux and Windows on every push to
main. A pre-release named `experimental` is rebuilt when main or an open pull request changes: main plus all open
pull requests merged in order.

On a Windows PC, paste `scripts/setup-new-pc.ps1` into PowerShell once (no admin rights needed); it
puts a "KoperCAD (latest)" desktop shortcut that always starts the newest experimental build. See `docs/LOCAL-BUILDS.md`
for building on your own PC.

**No admin rights, ever.** Everything installs, runs, updates and is scripted as a standard Windows
user: per-user install, writes only under `%LOCALAPPDATA%`, `%APPDATA%` and Documents, and `HKCU`.

## Repository map

| Path | What |
| --- | --- |
| `crates/novo-*` | The Rust workspace; owners and purposes are listed in `CLAUDE.md` |
| `docs/DESIGN.md` | The design of the app: interaction, pages, wiring, revisions |
| `docs/design/` | Per-feature designs (PLC, wiring modes, UI guidelines) and mock-ups |
| `docs/ROADMAP.md` | Roadmap summary and link |
| `docs/TEST_REPORT.md` | The tester's rounds and gate results |
| `handoff/` | Per-workstream hand-over files and the project-wide `MAIN-HANDOFF.md` |
| `assets/branding/` | Icons and the KoperCAD / KoperVonk lockup |
| `scripts/` | Launchers, installer and build scripts |

## How the work is organised

The project is built by parallel workstreams, each in its own crate with its own pull request, with
one integration lead merging once the named reviewer says READY on the current head. Only the data
model thread changes the public API of `novo-core`. Other crates read the model through queries and
change it only through commands, so undo, scripts and the AI all share one path. `unsafe` is denied
workspace-wide. Conventions: IEC 81346 tags, IEC 60617 symbols first, millimetres for geometry and SI units
for electrical values. See `CLAUDE.md`.

## Licence

Not yet decided. Until a licence file is added, all rights are reserved by the author.
