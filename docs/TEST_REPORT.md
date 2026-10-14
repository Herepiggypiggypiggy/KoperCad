# Test report

Running report from the tester thread. Newest round first. Each round lists
what was tested, the result, and open findings. Integration tests live in
`crates/novo-tests`.

How a PR is tested: its branch is merged onto the current `main` (a
`Cargo.lock`-only conflict is resolved by regenerating the lock), the
`novo-tests` crate is added, and the CI commands run on Linux:
`cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace`. Windows results come from the PR's own CI.

## Round 2 (2026-10-14, main `7b9392e`)

main now has #6 (experimental builds) and #11 (non-finite numbers refused,
pin directions, multi-level terminals). fmt, clippy and all 82 workspace
tests pass with `novo-tests` merged in. `novo-tests` needed one change for
#11: `SymbolRole::Terminal` now takes `terminals: vec![...]`.

- **F1 fixed by #11.** New test `robustness.rs` tries NaN, +inf and -inf in
  placements, pin offsets, junctions, wire routes, potential voltages, float
  properties, cable lengths and core cross-sections. Every one is refused,
  leaves the project and the undo history unchanged, and the project still
  round-trips. A hand-edited project file with `1e999` is refused on load.
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
- #6 merged; the first `experimental` pre-release has not been checked yet.

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
| Canvas draws model objects | Pending merge of #4 | #4's `PageScene` adapter and tests pass on main |
| 10,000 objects at 60 fps | Not verifiable here | The container only has llvmpipe (software): the release stress example measured 19 fps here with animation on (10,001 items, 330k triangles, tessellated in 35 ms), matching #4's own figure. Needs `cargo run --release -p novo-canvas --example stress` on a real GPU. Model side is fine: build 84 ms, netlist 17 ms, page read under 1 ms, one edit 0.3 ms (debug build) |
| Save, close and reopen gives an identical project | Met on #5, except F1 | Gate 1 project, incremental saves after edits and undo, crash recovery and a 10k-object page (save 214 ms, open 253 ms) all reopen byte-identical |
| Empty app installs and launches; CI green | Launch verified on Linux; installer pending | #3 under Xvfb with llvmpipe: window opens with menus and panels and stays up. The `installer` job on #3 was still running at the time of this round |

### No admin rights (hard rule from Tadjiro)

Everything must install and run as a standard user, with no elevation.

| Check | Status | Evidence |
| --- | --- | --- |
| Installer is per-user | Pass (static) | #3 `novoelectric.iss`: `PrivilegesRequired=lowest`, installs to `{localappdata}\Programs`, shortcuts in `{userprograms}`/`{userdesktop}`, file association under `HKCU` only |
| App never asks for elevation | Pass (static) | #3 `build.rs` embeds an `asInvoker` manifest; CI step fails if either exe contains `requireAdministrator` or `highestAvailable` |
| Installs and launches as a standard user | Pending | #3's `installer` job creates a non-admin local user, installs with its credentials, checks nothing was written to `HKLM`, and launches the app for 10 s. The first run failed because the test user could not write the job's `TEMP`; #3 pushed a fix (`14ebe0a`) and the re-run is pending |
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

**Q1 (question for WS-02/WS-05) Do overload relay heaters conduct in the netlist?**
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
