# Test report

Running report from the tester thread. Newest round first. Each round lists
what was tested, the result, and open findings. Integration tests live in
`crates/novo-tests`.

How a PR is tested: its branch is merged onto the current `main` (a
`Cargo.lock`-only conflict is resolved by regenerating the lock), the
`novo-tests` crate is added, and the CI commands run on Linux:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`. Windows results come from the PR's own CI.

## Gate 0 summary (main `11363c1`)

| Criterion | Status | Evidence |
| --- | --- | --- |
| Model API v0 frozen, with full unit tests | Met | #1, #11, #14, #21 (saved JSON format pinned) and #26 merged. F1 (non-finite numbers) fixed in #11 and covered by `robustness.rs` |
| Canvas draws model objects | Met | #4 merged; its `PageScene` adapter and headless GPU tests pass on main |
| 10,000 objects at 60 fps | Met | Release `canvas-demo` on Tadjiro's work PC (Quadro P1000, Vulkan, 2400x1350): at 10,000 objects 1135 fps average, 222 fps 1% low, 127 fps minimum. At 100,000 objects 113 fps average and 38.5 fps 1% low, with stutter while panning (not a Gate 0 criterion; worth a look before large projects) |
| Save, close and reopen gives an identical project | Met | `gate0_storage.rs`: Gate 1 and a 10k page reopen with identical JSON, nets and potentials, also after incremental saves, undo and crash recovery |
| Empty app installs and launches; CI green | Met | #3 merged. Its `installer` job on `798193a` built the installer, installed it as a standard (non-admin) user, found nothing in `HKLM`, and launched the app for 10 s. Linux launch verified under Xvfb |
| No admin rights | Met | Per-user installer (`PrivilegesRequired=lowest`), `asInvoker` manifest checked in CI, standard-user install passes, no machine-wide paths anywhere in `crates/` or `scripts/` |

main `11363c1` passes fmt, clippy and all 232 workspace tests on Linux.
**Every Gate 0 criterion is met.**

## Round 10 (2026-10-14, main `7bb466d`): main after the cloud merges; the starter's outputs

GitHub Actions is down, so main was re-tested locally after the cloud
session's merges (#53 Gate 1 in the app through #80):

- main `7bb466d` alone: fmt and clippy clean; 625 passed, 0 failed, 3 ignored
  (the two #38 gap tests this PR fills in and the `widget.rs` doc example).
- main plus this PR: 629 passed, 0 failed, 3 ignored (the doc example and the
  F4 and F5 gap tests below).

New in `gate1_scenario.rs`:

| Test | Checks | State |
| --- | --- | --- |
| `outputs_of_the_starter` | With every part assigned from the starter catalog: `novo_checks::check` reports nothing; each device's cross reference is its main function on page 1 and -X1's lists its four terminals in strip order; the BOM names every device; the PDF with `LibraryArt` is one page labelled `1`, has a bookmark tree, and carries the device tags and terminal names as text | Passes |
| `gap_bom_orders_a_terminal_block_per_terminal` | F4 below | Ignored, fails |
| `gap_pdf_names_the_terminal_strip` | F5 below | Ignored, fails |

Findings:
- **F4 (parts and BOM, medium):** the starter catalog's suggestion for -X1
  is a feed-through terminal block (Phoenix UT 2,5), and `assign_part`
  puts it on the strip device. `missing_parts()` then counts all four
  terminals as covered, but the bill of materials orders **one** UT 2,5 for
  a strip of four, and the PE terminal would get a grey feed-through block
  instead of a PE block. #72 makes the strip row, BOM and missing-parts
  check read one terminal article, but a part on the strip device itself is
  still not a terminal article there. Suggested fix: assigning a terminal
  block to a strip sets the strip's terminal article for that kind (or each
  terminal's part), not a device part. Sent to WS-07 and WS-13.
- **F5 (drawing and PDF, low):** terminals show only their own name (`1`,
  `2`, `3`, `PE`; that is how #28 fixed F2), and nothing on the page names
  the strip, so the printed page doesn't say which strip they belong to.
  IEC 61082 drawings name the strip once beside a group of its terminals.
  Sent to WS-03 (labels) and the outputs thread (PDF).

## Round 9 (2026-10-14, main `15c9992`): Gate 1 through the window, #38 gaps closed

#53 (`NovoApp::execute`) and #38 (novo-editor) are on main, so the Gate 1
scenario now also runs through the whole window, and the last two gap tests
are filled in. `gate1_scenario.rs` has no ignored test left.

| Test | Step | Closed by | State |
| --- | --- | --- | --- |
| `scenario_through_the_window` | Steps 1-7 headless through `NovoApp` with the real `EditorCanvas`: the file given on the command line opens on the first frame on its first page; every edit of the starter runs through `NovoApp::execute`; File > Save and File > Close leave nothing open and report no warning or error; a fresh window opens the same JSON and the same motor potentials | #53 | Passes |
| `gap_place_by_search_with_auto_tags_and_self_wiring` | The I search finds `mcb3`, `main3`, `ol`, `m3`; one click each in a column plus Enter on the proposed wires gives `-F1`, `-K1`, `-F2`, `-M1` and exactly 9 straight main wires, Q1 to K1 to F1 to M1 connected per phase; save and reopen keep it | #38 | Passes |
| `gap_draw_real_symbols` | Every placed symbol of the starter has `LibraryArt` artwork, drawn instead of the placeholder box, with its connection points where the placeholder had them | #38 | Passes |

Notes:
- The MCB is placed as `-F1`, not `-Q1`: novo-editor gives a miniature
  circuit breaker letter F (protecting, IEC 81346-2) and a motor protection
  breaker (`mpcb`) letter Q. The test pins this current behaviour; whether
  breakers should be Q is still the open question O1 (Round 7). The
  overload relay is then `-F2`.
- The window test uses `StorageBackend::default()` and default settings, so
  it writes only in its temp directory (no last-project or untitled files in
  the user profile).
- BOM terminal count equal to the strip's terminal count: #72 (open) adds
  exactly this check (`bom_counts_every_terminal_the_strip_shows`), so it is
  not duplicated here.

main `15c9992` plus this round passes fmt, clippy and all workspace tests on
Linux: 628 passed, 0 failed, 1 ignored (the `ignore` doc example in
`novo-canvas`'s `widget.rs`).

## Round 8 (2026-10-14, main `7379d42`): Gate 1 gaps after #30, #36

#30 (novo-terminals) and #36 (novo-parts) are on main, so their gap tests
drive the real APIs through the Gate 1 motor starter and are no longer
ignored.

| Gap test | Step | Closed by | State |
| --- | --- | --- | --- |
| `gap_place_terminals_on_wires_and_show_the_strip` | The starter wired straight from `-F1` to `-M1`; one `place_terminals` call across the four motor wires is one undo step and gives `-X1:1`, `:2`, `:3` (feed-through) and `:PE` (protective earth, from the wire's potential); the motor still reads L1, L2, L3, PE; the connection screen lists `-F1:2/4/6` and the PE feed inside and `-M1:U1/V1/W1/PE` in the field; `check()` is clean | #30 | Passes |
| `gap_assign_parts_from_the_catalog` | Each of the 5 devices gets the starter catalog's first suggestion for its letter code through `assign_part` (one undo step each); `missing_parts()` is empty; save and reopen keep the parts | #36 | Passes |
| `gap_place_by_search_with_auto_tags_and_self_wiring` | Place by search with the next free tag; a column wires itself | #38 | Open, panics |
| `gap_draw_real_symbols` | Library artwork instead of placeholders | #38 | Open, panics |

Also in this round:
- The Gate 1 fixture's `-X1:PE` is now a protective earth terminal
  (`handoff/tester/pe-fix.patch`, waiting for #30). novo-terminals'
  `gate1_checks_flag_the_pe_terminal_kind` became `gate1_checks_are_clean`:
  `check()` reports nothing on the fixture.
- **F3 (fixed here, novo-terminals, medium):** `place_terminals` decided
  which pin faces the start of a wire by which wire end was nearer, not by
  the wire's direction. A terminal dropped on the far half of a long wire
  went in backwards: the incoming wire ran through the symbol to pin 2, and
  internal and external sides swapped (the PE row of the gap test showed
  `-M1:PE` inside and the feed in the field). Pin 1 now faces the start
  when the wire runs down (or right). Regression test
  `a_terminal_dropped_near_the_far_end_keeps_its_sides`.

main `7379d42` plus this round passes fmt, clippy and all workspace tests on
Linux: 411 passed, 0 failed, 3 ignored (the 2 open #38 gaps and the
`ignore` doc example in `novo-canvas`'s `widget.rs`).

## Round 7 (2026-10-14, main `f6ee2f1`): Gate 1 gaps after #28, #29, #34, #35, #37

Since #42 merged, #35 (core: potential assignment, terminal rule, text
records), #37 (symbols), #34 (app storage and canvas), #28 (canvas: text
picking, junction glyphs, text drawing) and #29 (novo-workflow) are on main.
Each gap test whose pull request merged now drives the real public API
through the Gate 1 motor starter and is no longer ignored. Each gap still
waiting for an open pull request panics with `gap: closed by #NN`, so
`cargo test -p novo-tests --test gate1_scenario -- --ignored` shows the
open gaps as failures.

| Gap test | Step | Closed by | State |
| --- | --- | --- | --- |
| `gap_create_open_and_save_through_the_app` | Create, save, open through `StorageBackend`: the new project is on disk before the first save, the first save moves it to the chosen file, reopening shows the same project and potentials | #34 | Passes |
| `gap_insert_search_ignores_case` | `MCB3`, ` M3 ` etc. find the same symbol through `by_alias`, `mnemonic` and the I search (exact short name first); `next_tag` gives -K2, -M2, -F2 and starts a new location at 1 | #37, #29 | Passes |
| `gap_assign_potentials_to_selected_wires` | `AssignPotentials` on the three `-Q1`/`-K1` wires with a 400 V three-phase set: L1, L2, L3 at the motor, extras ignored, reruns create nothing, too many wires refused; `-X1:PE` defines PE by the terminal-name rule; undo removes it all | #35 | Passes |
| `gap_place_by_search_with_auto_tags_and_self_wiring` | Place by search with the next free tag; a column wires itself | #38 | Open, panics |
| `gap_place_terminals_on_wires_and_show_the_strip` | Terminals on the motor wires in one call; strip screen and checks | #30 | Open, panics |
| `gap_assign_parts_from_the_catalog` | Catalog parts on every device; nothing missing | #36 | Open, panics |
| `gap_draw_real_symbols` | Library artwork instead of placeholders | #38 | Open, panics (pickable tags and F2 are done by #28) |

Also in this round, from the review of #42:
- Line potentials are 400 V (between lines); PE has no voltage. The
  scenario asserts both.
- The scenario no longer builds a change set by hand: every command records
  its own `Outcome::changes` with the session (or the app backend), and the
  save writes exactly that. Reload still gives identical JSON.
- The canvas check asserts positions: every symbol's connection points are
  at its placement plus the library's connection points, and every wire
  runs from pin to pin.
- **F2 is fixed** (#28). `e2e_motor_starter.rs` now asserts that a click on
  `-X1:2` picks `-X1:2` first.

**O1 (observation, low, WS-08):** the I search codes a motor circuit
breaker (`mcb3`) as F (protection), so placing one in the Gate 1 project
proposes `-F2`, next to the overload relay `-F1`. The Gate 1 fixture and
many cabinet builders tag it `-Q1` (IEC 81346-2 class Q, switching). Worth
a decision in WS-08's letter-code table or as a project setting.

main `f6ee2f1` plus this round passes fmt, clippy and all workspace tests on
Linux: 337 passed, 0 failed, 5 ignored (the 4 open gaps above and the
`ignore` doc example in `novo-canvas`'s `widget.rs`).

## Round 6 (2026-10-14, main `9512822`): Gate 1 scenario

New `gate1_scenario.rs` runs the Gate 1 flow, "create a project, place a
motor starter, wire it, see its terminals and potentials, save and reload",
using `Command`s only and the real project file. On main it passes:
- it creates a `.novo` file;
- it places the supply feeds, `-Q1` MCB (`mcb3`), `-K1` contactor (`main3`),
  `-F1` overload relay (`ol`), the `-X1` terminals (`term`, `xpe`) and
  `-M1` motor (`m3`), each found by the short name the user types;
- it wires all 17 connections;
- it reads L1, L2, L3 and PE at the motor's U1, V1, W1 and PE with no
  conflicts;
- it reads the strip as 1, 2, 3, PE with full designations and kinds;
- it finds `-X1` by search, draws every symbol on the canvas, then saves
  and reloads to the same project and potentials.

`e2e_motor_starter.rs` runs the two-page fixture the same way, adding
cross references and canvas updates from change sets. fmt, clippy and all
238 workspace tests pass.

**Known gaps** (ignored tests in `gate1_scenario.rs`; run them with `--ignored`):

| Step | Gap | Closed by |
| --- | --- | --- |
| Create, open, save through the app | The app's project backend (`StorageBackend`) | #34 |
| Place by search with the next free tag; a column wires itself | `novo-editor` | #38 |
| Short names in any case (`MCB3`); mnemonic search, favourites | `by_alias` normalising; `novo-workflow` | #37, #29 |
| Give selected wires L1, L2, L3 in one step | `Command::AssignPotentials` | #35 |
| Put terminals on the motor wires in one call; strip screen and checks | `novo-terminals` | #30 |
| Assign catalog parts; nothing missing | `novo-parts` | #36 |
| Draw real symbol artwork and pickable tags | `LibraryArt`, text picking; F2 below | #38, #28 |

**F2 (bug, medium, WS-03), fixed in #28 (verified in Round 7): a terminal's
tag steals clicks from the next terminal.** `PageScene` puts each symbol's tag to its right at tag height,
and picking counts label boxes. On the Gate 1 strip (terminals 8 mm apart),
`-X1:1`'s tag `=A1+CAB-X1:1` runs across `-X1:2`'s body. A click on `-X1:2`
then picks `-X1:1` at distance 0, ahead of `-X1:2` at 0.8. The tags also
overlap one another on screen. Suggestions: show only the terminal name
(`1`, `2`) on terminal symbols, as strip drawings do; and let ink beat a
label at equal or near distance when picking. The scenario test checks
`pick_all` for now, and its comment points here.

## Round 5 (2026-10-14, main `af01a66`)

main now has #4 (canvas engine), #21 (saved JSON format pinned, 10k-symbol
scale test), #22 (safer project file replace) and #23 (the storage gate
tests). fmt, clippy and all 191 workspace tests
pass with `novo-tests` merged in, including #4's headless GPU tests, which
ran on llvmpipe here rather than skipping.

- **Canvas on main passes.** The GPU stress test draws 10,001 items (330k
  triangles) on llvmpipe at 53 ms a frame in a debug build. Picking under
  the cursor takes 0.004 ms, snapping 0.018 ms and box-selecting a quarter
  of the page 5.7 ms. Rebuilding the page mesh after one edit takes 37 ms
  (debug). 60 fps at 10,000 objects still needs a real GPU and a release
  build.
- **No admin rights:** #4 adds no machine-wide paths, registry access or
  elevation.
- **Open PRs merged onto `af01a66`.** #3 (app shell, head `798193a`) merges
  cleanly: fmt, clippy and 223 tests pass. #20 (theme and icons, head
  `a98788e`) has a real conflict in `Cargo.toml` with main: both sides add
  `egui` to `[workspace.dependencies]`. Main adds it with default
  features, and #20 adds it with `default-features = false`. With main's
  entry kept and #20's other additions (`novo-theme`, `egui-phosphor`), fmt,
  clippy and 197 tests pass. #20 needs a rebase to pick one `egui` entry.
  Neither PR adds machine-wide paths. #3's installer still asks for
  `PrivilegesRequired=lowest`, and its CI fails if anything lands in `HKLM`.
- **Storage tests tightened after review of #23.** The netlist comparison
  across a reload now covers each net's `defined`, `carried` and
  `reachable` potentials, not only the shown ones. A second crash-recovery
  test feeds the session the `ChangeSet`s that real transactions return,
  with autosaves in between, as the app will.

## Round 4 (2026-10-14, main `1a68be8`)

main now has #18 (148 symbols, tighter `PassThrough`) and #5 (project
file). fmt, clippy and all 117 workspace tests pass.

- **Gate 0 "save, close and reopen gives an identical project" is met on
  main.** New `gate0_storage.rs` saves the Gate 1 project to a `.novo`
  file and reopens it. The JSON is identical, and so are the netlist (every
  net as its set of pins, with its potentials) and every pin's potential.
  `-M1:U1` is still `L1` and coil A1 is still `L`, with no conflicts. The
  same holds after incremental saves with a move, a delete, an undo and a
  retag, and for a project recovered from the autosave after a simulated
  crash. A 10,000-object page saves in 221 ms and opens in 252 ms (debug
  build).

## Round 3 (2026-10-14, main `e487ccc`)

main now has #14 (potentials carried through switched poles, terminal level
fixes) and #16 (experimental build merges `Cargo.lock`-only conflicts).
fmt, clippy and all 87 workspace tests pass with `novo-tests` merged in.

- The Gate 1 fixture now sets `pass_through` on fuses, the contactor, the
  breaker and every contact, as the library says. L1, L2 and L3 now reach
  the motor by name (`-M1:U1` is `L1`), and L reaches the coil's A1 and
  stops there. There are no potential conflicts.
  `fixture_matches_library.rs` checks each placed symbol's `pass_through`
  against `Symbol::pass_through()` minus its bridges.
- **Round 2's low observation is fixed by #14.** A multi-level terminal
  given a single terminal is now refused with `TerminalLevels`, and
  `robustness.rs` tests it.

## Round 2 (2026-10-14, main `244c008`)

main now has #6 (experimental builds), #11 (non-finite numbers refused,
pin directions, multi-level terminals), #12 (symbol format hardening,
pass-through) and #13 (canvas demo in the experimental build). fmt, clippy
and all 80 workspace tests pass with `novo-tests` merged in. `novo-tests` needed one change for
#11: `SymbolRole::Terminal` now takes `terminals: vec![...]`.

- **F1 fixed by #11.** New test `robustness.rs` tries NaN, +inf and -inf in
  placements, pin offsets, junctions, wire routes, potential voltages, float
  properties, cable lengths and core cross-sections. Every one is refused,
  leaves the project and the undo history unchanged, and the project still
  round-trips. A hand-edited project file with `1e999` is refused on load.
- **Q1 resolved by #12.** The overload relay's main circuit is now a
  permanent bridge in the library. `fixture_matches_library.rs` now checks
  that every symbol the fixture places carries the library's bridges.
- **Multi-level terminals pass.** Every multi-level terminal in the symbol
  library (`iec/terminals/terminal_2level`) places through
  `Command::PlaceSymbol` with its library pins, directions and bridges. Each
  level is its own net with its own designation (`=A1+CAB-X2:1`, `:2`),
  levels stay apart, and a symbol with more terminals than levels is
  refused.
- **Observation (low, WS-02):** a multi-level terminal symbol given a
  single terminal is accepted. Its levels then stay separate nets that all
  carry the same designation (`-X2:1` twice). This matches the documented
  rule ("one terminal owns every pin"), but the editor should probably
  require one terminal per level for symbols with more than one bridge
  group.
- **Observation (low):** a refused edit still uses up ids (`next_id` moves
  on). This is harmless, since ids are never reused, but worth knowing
  when comparing saved files.
- **#6 passes.** The first `experimental` pre-release was published at
  15:03 with `novoelectric-experimental-windows.zip` (5.9 MB) and
  `build-info.json`. It included main `ae41e02` plus #2, #3 and #6. It
  skipped #4 and #5 for merge conflicts, although each merges cleanly onto
  main alone. The conflicts come from stacking PRs that all touch
  `Cargo.lock` and `[workspace.dependencies]`. Suggestion for the workflow:
  on a conflict limited to `Cargo.lock`, take main's lock and let cargo
  regenerate it, which is how these tests merge PRs.

## Round 1 (2026-10-14)

### Results

| Tested | Head | On main | fmt | clippy | tests | Verdict |
| --- | --- | --- | --- | --- | --- | --- |
| main | `3ed2cf8` | | pass | pass | pass | Pass |
| #1 WS-02 data model (merged) | `ab6c765` | `4c821df` | pass | pass | 31 pass | Pass, one bug open (F1) |
| #2 WS-05 symbols (merged) | `78f3d60` | `ae41e02` | pass | pass | 55 pass | Pass, one question (Q1) |
| #3 WS-01 app shell | `2c01d59` | `ae41e02` | pass | pass | 66 pass | Pass |
| #4 WS-03 canvas | `89921f6` | `ae41e02` | pass | pass | 105 pass | Pass; 60 fps needs a real GPU |
| #5 WS-04 storage | `216dc6d` | `a138eb0` | pass | pass | 74 pass | Pass; F1 shows up here as an unopenable file |
| #6 experimental builds | `7459722` | `a138eb0` | pass | pass | 55 pass | Pass (workflow not run here) |

Test counts include the `novo-tests` crate added on top.

### Gate 0 criteria

| Criterion | Status | Evidence |
| --- | --- | --- |
| Model API v0 frozen, with full unit tests | Met, with F1 open | #1 merged; 31 tests in `novo-core`; 14 gate tests in `novo-tests` |
| Canvas draws model objects | Met | #4 merged as `d91e961`; its `PageScene` adapter and headless GPU tests pass on main |
| 10,000 objects at 60 fps | Not verifiable here | The container only has llvmpipe (software): the release stress example measured 19 fps here with animation on (10,001 items, 330k triangles, tessellated in 35 ms), matching #4's own figure. Needs `cargo run --release -p novo-canvas --example stress` on a real GPU. Model side is fine: build 84 ms, netlist 17 ms, page read under 1 ms, one edit 0.3 ms (debug build) |
| Save, close and reopen gives an identical project | Met on #5, except F1 | Gate 1 project, incremental saves after edits and undo, crash recovery and a 10k-object page (save 214 ms, open 253 ms) all reopen byte-identical |
| Empty app installs and launches; CI green | Launch verified on Linux; installer pending | #3 under Xvfb with llvmpipe: window opens with menus and panels and stays up. The `installer` job on #3 was still running at the time of this round |

### No admin rights (hard rule from Tadjiro)

Everything must install and run as a standard user, with no elevation.

| Check | Status | Evidence |
| --- | --- | --- |
| Installer is per-user | Pass (static) | #3 `novoelectric.iss`: `PrivilegesRequired=lowest`, installs to `{localappdata}\Programs`, shortcuts in `{userprograms}`/`{userdesktop}`, file association under `HKCU` only |
| App never asks for elevation | Pass (static) | #3 `build.rs` embeds an `asInvoker` manifest; CI step fails if either exe contains `requireAdministrator` or `highestAvailable` |
| Installs and launches as a standard user | Pass | #3's `installer` job creates a non-admin local user, installs with its credentials, checks nothing was written to `HKLM`, and launches the app for 10 s. The first run failed on the job's `TEMP`; after the fix it passed on `798193a` (run 38597775983) |
| No machine-wide paths at runtime | Pass (static) | No `HKLM`, `Program Files`, `ProgramData` or system paths in main or any open PR. #5 writes the project and its `.autosave` next to the project file; #3 keeps settings in eframe's per-user storage; #6's launcher and shortcut use `%LOCALAPPDATA%` and the user's desktop |

From the next round, every PR is checked for new machine-wide paths or elevation requests.

### Findings

**F1 (bug, high, fixed in #11) A non-finite coordinate makes the project impossible to reopen.**
`novo-core` accepts `NaN` or infinite values in placements, points and
offsets. JSON writes them as `null`, so the project saves but
`Project::from_json` fails, and `novo-storage` (#5) then reports the file as
corrupt (`object 57: invalid type: null, expected f64`). With `NaN`, undo of
that edit also fails with `Conflict`, because `NaN != NaN` breaks the
before-state comparison. One bad value from a script, an import or a
division by zero in the editor loses the whole file.
Repro: `tx.move_symbol(id, Placement::at(Point::new(f64::NAN, 0.0)))`, then
save and reopen. Suggested fix (WS-02): reject non-finite numbers in the
integrity check for every geometry field and float property.

**Q1 (question for WS-02/WS-05, resolved in #12) Do overload relay heaters conduct in the netlist?**
`novo_symbols::Symbol::bridges()` only returns bridges for `conductor`
symbols, so `iec/protection/overload_relay` (`overload`) has none. Placed
that way, the motor is not on the same net as the contactor outputs, and
potentials and wire numbers stop at the relay. The heaters always conduct;
only 95-96 and 97-98 switch. The Gate 1 fixture sets the bridges by hand.
Decide whether `overload` main poles should be bridges.

**Observations (low, for WS-02 or WS-14 design checks)**
- Duplicates are accepted without error: a second page named `1`, a second
  terminal `1` on `-X1`, two pins with the same name on one symbol, and
  parallel wires between the same two points. `duplicate_tags()` covers tags
  only.
- `DeviceTag::parse("")` succeeds with an empty tag, and a device can carry
  it. `-K 1` silently becomes `-K1`.
- Unknown top-level keys in project JSON are ignored, while symbol files
  reject unknown keys.
- Linux needs `libxkbcommon-x11` at runtime (the app panics at start
  without it); worth a line in the README.

### Added tests (`crates/novo-tests`)

- `motor_starter` fixture: the Gate 1 direct-on-line starter over two pages
  (fuses, contactor, overload relay, terminals, motor cable, start/stop with
  self-hold), parts on every device, built through the public API with the
  WS-05 symbol ids and connection points.
- `gate1_motor_starter.rs`: completeness, main and control connectivity,
  cross-page devices, designations, parts and cable cores, JSON and records
  round trip, transaction replay, undo and redo, delete and undo.
- `gate0_stress.rs`: a 10,000-object page through build, read, netlist,
  JSON round trip, a single edit and undo.
- `fixture_matches_library.rs`: every symbol in the fixture exists in the
  built-in library with the same connection points.
- Ready once #5 merges: Gate 1 project through `Session` save/reopen,
  incremental saves, crash recovery and the 10k page.
