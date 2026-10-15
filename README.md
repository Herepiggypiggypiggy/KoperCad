<p align="center">
  <img src="assets/branding/lockup.png" alt="KoperCAD by KoperVonk" width="420">
</p>

# KoperCAD

A lightweight, local-first electrical CAD program by **KoperVonk**: schematics, terminal and cable
connection screens, cross references, generated reports, PLC and I/O, revisions, and (later) cabinet
layout and circuit simulation. Native Rust, no web technology, fully offline, scriptable from Python.

> **Name.** The product is KoperCAD and the maker is KoperVonk (decided 2026-10-15); it was called
> NovoElectric before. The repository is now `kopercad`. Names people never see keep the old name for now: the `novo-*`
> crates and the release asset name. The program is `kopercad.exe`, the Python module `kopercad`
> (`crates/novo-py/compat` is a shim that keeps `import novoelectric` working) and the command-line tool
> `kopercad-cli`
> (the experimental zip also carries a `novoelectric.exe` copy for launchers installed before the
> rename). Settings and the parts catalog moved to `%APPDATA%\KoperCAD`; projects stay in
> `Documents\NovoElectric` for anyone who already has one; `%LOCALAPPDATA%\NovoElectric` still holds
> the launcher's builds.

It is a design of its own, aimed at what EPLAN Electric P8 and AutoCAD Electrical do, not a copy of either.

## Status at a glance (2026-10-15, main `d91b4c2`)

| Area | State |
| --- | --- |
| Phase 0 Foundation (gate 0) | Done |
| Phase 1 Schematics (gate 1: motor starter over two pages, saved, printed to PDF) | Done |
| Phase 2 Intelligence (gate 2: terminal, cable and report pages generate live) | Met on Linux, per the tester's report |
| Phase 3 Physical design | Started: PLC and I/O, revisions, cabinet layout engine with placement storage and canvas scene; wire routing has a design and a pure duct router |
| Phase 4 Simulation | Started: live potentials with coils, changeover and timer contacts and glowing lamps and motors on main (awaiting hands-on test); device models and animation in review |
| Phase 5 AI and interop | Started: DXF reader and writer on main; EPLAN parts import and the NFPA symbol pack in review; AI assistant not started |

Where the work is today: all 26 points of the owner's workflow list (from the first hands-on tests of the
experimental builds) are merged on main. Points 14 and 16 and the live simulation view still need the
owner's hands-on test on Windows. Ten pull requests are in review (see `docs/ROADMAP.md`). See
[Near-term roadmap](#near-term-roadmap-the-26-point-list) below.

What is verified and what is not:

- **Verified on Linux:** `cargo fmt`, `cargo clippy -D warnings` and all workspace tests passed on
  every merge (several hundred tests per crate, about 920 at the last full count, 0 failing, a few ignored).
- **Windows:** the repository is public, so GitHub Actions runs again. The Windows CI job on main
  builds and tests the workspace and passes on every recent main push. Experimental Windows builds
  1 to 4 have been published (build 4 is main `f6be257`). Everything merged after build 4 is not in
  any build yet; no new build is cut until the owner asks for one.
- **Not yet done:** the in-app dialogs for several finished engines (see below), the full set of
  app-side revision features, and the cabinet, routing and full simulation phases.

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
- **CLI and Python** (`novo-cli`, `novo-py`): a first `kopercad-cli` command-line tool and Python bindings.

Application (`novo-app`):

- Schematic editing with the dark theme, command palette, outline with an Issues list, generated
  report pages that follow edits live, page labels per sheet, and new or opened projects getting their
  generated pages (skipped for an issued project with frozen pages).
- **Placing** (in builds 1 to 4): `Q` is the place key. Hold it for the place wheel (Terminal, Switch,
  Relay, Breaker, More...); the outer ring follows the pointer only after a short dwell so a path
  across a neighbouring wedge does not change the choice. Dragging places a row of symbols, and
  dropping a contact on a coil links it to that device. Right-click menu, tool dock with the wiring
  mode chip, `N selected` count, device properties on double-click and rename of tags and wire numbers
  by double-click.
- **On main since build 4, not yet in a build** (the full list is in `docs/ROADMAP.md`): live potentials and simulation view (Shift+L), Issue revision window and Changes mode, cable tool, user `keymap.txt`, Ctrl+D smart duplicate, the `kopercad.exe` rename, and earlier: autoconnect drawn with the drop (engine; the Preferences setting is still open); the pages
  navigator in the outline (Function and Location views, New page, group and panel, drag to move);
  one-ended terminals as the standard terminal, multi-pole drag of a one-pole part, strip naming by
  adjacency with Up and Ctrl+Up; PE terminal toggle and automatic PE marking; interruption auto-naming;
  set potential on a feed and a manual PE override; label kinds that win a pick over symbol ink;
  wire corner handles in the canvas; the `C` cable key and commands; and the first live-potentials
  solver.

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
`docs/DESIGN.md` and `docs/design/wiring-modes.md`. The engine and the dock chip are on main and in
build 4. Whether a symbol move that would force an overlap should be refused is the owner's call after
hands-on testing; plain drag moving only the component is the current decision (see the roadmap).

## Near-term roadmap: the 26-point list

The owner's workflow list of 2026-10-15 is the near-term roadmap. Its design is in `docs/DESIGN.md`
(PRs #238, #242, #247). State on main at `d91b4c2`:

| State | Points |
| --- | --- |
| Implemented on main | 1 to 13, 15, 17 to 21, 23 to 26 |
| Implemented, awaiting hands-on test | 14 and 16 (wheel dwell, held place key), 22 (live potentials and simulation view) |
| In review | palette ranking (#350), group and panel rename (#348), PLC potentials (#333), Shift+arrows nudge (#324) |

The per-point table with owners and the PRs that closed each part is in `docs/ROADMAP.md`.

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

- **Hands-on test** of points 14, 16 and the simulation view on Windows.
- **Revisions in the app:** the Issue revision window, Changes mode and title block revision fields are
  on main; the revision table above the title block is a draft (#343).
- **PLC in the app:** dialogs and commands mapped onto the finished PLC engine (add card, to PLC,
  renumber exist as commands).
- **Cabinet layout** (`novo-layout`): geometry engine, rails, place-all planner, layout state,
  placement storage in the model and the canvas cabinet scene are on main (#287 to #349). Next: dialogs
  and hatches, dimensions and snapping in the app.
- **Wire routing** (`novo-routing`): design and the pure duct routing engine are on main (#334); next
  is wiring it to the layout and the app.
- **Simulation** (`novo-sim`): coil to contact coupling, changeover and timer contacts and glowing
  lamps and motors are on main; device models and animation are in review (#346); next are latching
  circuits and fault scenarios.
- **AI assistant** (`novo-ai`) that edits through the same command path as the user (held until the
  threads above land).
- **Import and export** (`novo-interop`): DXF reader and writer are on main (#344); EPLAN parts import
  is in review (#347); Excel round trip is next.
- **Standards packs:** the NFPA core set is in review (#351), design in #335.
- **Parts library:** plan for Wago, Phoenix Contact, Schneider and Siemens catalogues with a one-click
  Import design (#342, in review).
- **Python layer:** more of the app scriptable (Python is the integration layer, never the core).
- The rename is done for the executable, Python module and data folders; the `novo-*` crate names stay.

## Build and run

```
cargo build                      # debug build
cargo run -p novo-app            # launch the app
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs fmt, clippy and tests on Linux for every pull request, and on Windows for pushes to main and
nightly. A pre-release named `experimental` is rebuilt when main or an open pull request changes: main plus all open
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
| `docs/ROADMAP.md` | Roadmap summary, link, the 26-point list and what is new since |
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
