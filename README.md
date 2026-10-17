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

## Status (2026-10-17, release 0.1.0 beta in preparation)

KoperCAD is feature complete for a first beta and is being tested before the `v0.1.0` tag. The plan, blocker
list and test scenarios are in `handoff/release-0-1/`. What is in the release, and what is not, is in
[`RELEASE_NOTES.md`](RELEASE_NOTES.md); test users start with [`docs/TESTER-README.md`](docs/TESTER-README.md).

| Area | State |
| --- | --- |
| Schematics (symbols, wiring, tags, potentials, numbering, cross references) | In the app |
| Terminals, cables, PLC and I/O, reports and checks, PDF and Excel export | In the app |
| Parts catalogue, import of maker data, bill of materials, bench output | In the app |
| Cabinet layout, ducts and wire routing, enclosure sizing, 3D view | In the app |
| Heat and airflow (model, accuracy stated), protection, sizing, functional safety (estimate) | In the app, labelled as models |
| Live simulation, motor start, fault scenarios | In the app |
| Revisions, compare, commissioning and field mode | In the app |
| Learn (33 lessons, 10 finished), example project, fast-keys walk-through | In the app |
| DXF and STEP import, `kopercad-cli`, Python module | Available |
| EPLAN project connectivity and PLC program export | Paused, not in 0.1 |
| AI assistant, multi-user editing, symbol editor | Not started |

What is verified and what is not:

- **Linux:** `cargo fmt`, `cargo clippy -D warnings` and the whole workspace test suite (about 3,900 test
  functions) run on every pull request.
- **Windows:** the Windows job builds and tests on every push to main and nightly. Experimental Windows
  builds are published regularly. Hands-on testing on real PCs, from a clean standard user account, is what
  the beta is for; the checks are in `docs/WINDOWS-CHECK.md`.
- **Models, not exact:** heat and airflow, functional-safety figures, motor start and cable tables are
  estimates or planning values and are labelled so in the program. See "Numbers" in `RELEASE_NOTES.md`.

## What works today

Each crate has one job and one owner (see `CLAUDE.md`); the list below is by area.

- **Data and files:** one data model (`novo-core`) that every edit goes through as a command, so undo, scripts
  and imports share one path; one SQLite project file (`novo-storage`) with autosave, recovery and migrations.
- **Drawing:** GPU canvas (`novo-canvas`), IEC 60617 symbols with NFPA alternatives (`novo-symbols`),
  schematic editor with tags, wiring modes and potentials (`novo-editor`), keyboard workflow and command search
  (`novo-workflow`), wire and potential numbering (`novo-numbering`).
- **Intelligence:** cross references (`novo-xref`), terminal strips (`novo-terminals`), cables (`novo-cables`),
  PLC and I/O (`novo-plc`), design checks and standards checks (`novo-checks`), revisions (`novo-revisions`),
  functional safety (`novo-safety`).
- **Parts and output:** part catalogue and bulk import (`novo-parts`), reports, bill of materials and Excel
  (`novo-reports`), vector PDF (`novo-pdf`), DXF and STEP (`novo-interop`).
- **Physical design:** cabinet layout (`novo-layout`), duct routing and wire lengths (`novo-routing`), 3D view
  (`novo-view3d`), heat field (`novo-heatview`), thermal and airflow solvers (`novo-thermal`, `novo-cfd`).
- **Simulation and commissioning:** live circuit simulation (`novo-sim`), commissioning mode
  (`novo-commissioning`).
- **Learning:** lessons that run in the program (`novo-learn`, `assets/lessons`).
- **Scripting:** `kopercad-cli` (`novo-cli`) and the Python module `kopercad` (`novo-py`).
- **Shell:** the application, theme and tests (`novo-app`, `novo-theme`, `novo-tests`).

### Two wiring modes

Wiring has two modes, switched with **Shift+W** (also in the middle of a wire) and a chip next to
Connect in the tool dock:

- **Corners** (default): you place corner points, or drag from pin to pin and the router finds a path.
- **Lines:** you draw straight line segments yourself.

In both modes wires of different conductors never share length. A corner or segment that would overlap is
refused with the reason. Only paths that cannot ask (scripts, undo, opening a file) accept an overlap, and
they report it as a `wire.overlap` warning. The design is in `docs/DESIGN.md` and `docs/design/wiring-modes.md`.

## Roadmap

The summary and what comes after 0.1 are in [`docs/ROADMAP.md`](docs/ROADMAP.md). The full roadmap with
dependencies is the artifact linked there.

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

Test users get the installer or the portable zip from the release page and start with
[`docs/TESTER-README.md`](docs/TESTER-README.md). To follow the newest experimental build instead, paste
`scripts/setup-new-pc.ps1` into PowerShell once (no admin rights needed); it puts a "KoperCAD (latest)" desktop
shortcut that always starts the newest build. See `docs/LOCAL-BUILDS.md` for building on your own PC.

**No admin rights, ever.** Everything installs, runs, updates and is scripted as a standard Windows
user: per-user install, writes only under `%LOCALAPPDATA%`, `%APPDATA%` and Documents, and `HKCU`.

## Repository map

| Path | What |
| --- | --- |
| `crates/novo-*` | The Rust workspace; owners and purposes are listed in `CLAUDE.md` |
| `docs/DESIGN.md` | The design of the app: interaction, pages, wiring, revisions |
| `docs/design/` | Per-feature designs (PLC, wiring modes, UI guidelines) and mock-ups |
| `docs/ROADMAP.md` | Where the project is, what 0.1 contains, what comes next |
| `docs/TESTER-README.md` | The first page for test users (also shipped in the release zip) |
| `RELEASE_NOTES.md` | What is in the current release and what is not |
| `THIRD_PARTY_NOTICES.txt` | Licences of the software KoperCAD includes (`tools/third-party-notices.py` writes it) |
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

Not yet decided. Until a licence file is added, all rights are reserved by the author. Third-party licences are
in `THIRD_PARTY_NOTICES.txt`.
