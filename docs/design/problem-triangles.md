# Problem triangles: one visual mechanism for every problem and result

Status: design, phase 2 of the "Redesign problems as warning triangles" thread. Input: `handoff/problem-triangles/LEDGER.md` (the audit: 76 check kinds, 50 other finding types, 776 message sites, 14 display channels). Docs only; no code changes in this PR.

## 1. The rule

Anything the program says is wrong, doubtful, unfinished or failed is a **flag**: a yellow or red triangle drawn **on the thing that causes it**, with the detail in a popover. No sentence about a problem is written on the screen. A sentence that explains how to use something is not a problem; it goes behind the question-mark icon (the text sweep thread owns those).

Two colours, as asked: **red** is wrong (cannot be built or run as drawn), **yellow** is doubtful or unfinished. No triangle means nothing to say. Information (the 9 `Info` kinds, the "N devices placed", "Saved") is not a flag.

## 2. Look (colour-blind safe, themed)

| Flag | Glyph | Why it is distinguishable without colour |
|---|---|---|
| Red | solid triangle, thick outline, white "!" | solid |
| Yellow | triangle with a thick coloured outline and the panel colour inside, "!" in the outline colour | hollow |
| Yellow, unchecked | the yellow triangle with a dashed outline | "I could not check this" (missing data), not "this is wrong" |
| Waived | the triangle at 40 % with a small tick, only in the Flags list and when "Show waived" is on | |
| Cluster | one triangle with a count badge when several flags land within 14 px | |

- The octagon used for errors today (`MarkKind::Error`) goes away; red and yellow differ in fill, not outline shape.
- Colours come from `crate::theme::ui_palette()` (`error`, `warning` and their washes). The Cabinet canvas style (`CabinetStyle::{error,warning}`) and the 3D look read the same two values. High-contrast mode already swaps the palette.
- Size: 14 pt at 1366x768 and 1920x1080, drawn at constant screen size on the schematic, the plate and in 3D. Hit area 24 pt. Reduce-motion: no pulse; otherwise a new flag pulses once (0.9 s, as `problem_flash`).

## 3. The flag record

A pure data type in a new crate `novo-flags` (to be added by the coordinator in its own small PR, as CLAUDE.md requires; pure, depends on `serde` only). Engines do not depend on it; `novo-app` has one adapter per engine.

```rust
pub struct Flag {
    pub id: FlagId,               // stable hash of (source, rule key, subjects): survives re-checks
    pub severity: Severity,       // Red | Yellow
    pub unchecked: bool,          // dashed: could not be judged
    pub source: Source,           // Issues(kind) | Layout | Routing | Heat | Sim | Emc | Safety | Calc | Parts | Job | ...
    pub anchors: Vec<Anchor>,     // one triangle per anchor, one flag, one count, one waiver
    pub title: String,            // <= 28 chars, noun phrase: "Door cannot close"
    pub detail: Detail,           // 1-3 short lines with the numbers: "-K1 is 12 mm too deep"
    pub actions: Vec<Action>,     // Locate | Fix(command id) | Open(panel) | Retry | Waive
    pub waiver_key: Option<String>, // Issues engine key, so existing waivers keep working
    pub life: Life,               // Live (recomputed with the model) | Until(fixed or dismissed) | Run(job id)
}
pub enum Anchor {
    Symbol(SymbolId), Wire(WireId), Pin(SymbolId, u32), Device(DeviceId), Terminal(TerminalId), Cable(CableId),
    Page(PageId, Point),                         // a spot on a sheet
    Plate(PlateId, Point), Rail(RailId), Duct(DuctId), Door(StructureId), Opening(OpeningId), Enclosure(StructureId),
    World(StructureId, [f32; 3]),                // a spot in 3D (hot pocket)
    Control(UiKey),                              // rail item, top bar tile, button
    Job(JobKey),                                 // export, import, simulation run, sweep
    Project,                                     // the file itself
}
pub struct Severity {Red, Yellow}  // the one vocabulary; engines map into it
```

`FlagStore` (also in `novo-flags`): `replace_source(source, Vec<Flag>)` keeps flags stable across recomputes, drops waived ones from counts, answers `at(anchor)`, `roll_up(anchor_set)` (worst severity and count for a workspace tab, a rail zone, a panel), `next(after)` for F8, and `by_area()`. The Issues cache `wb.problems` becomes one `replace_source`.

Severity mapping (all eleven vocabularies, section 3 of the ledger): Error, Problem, Blocking, Red, Over -> Red. Warning, Major, Busy, Warning-class toast -> Yellow. Info, Note, Minor, Idle, Ok, S1/S2 (not a severity) -> no flag.

## 4. Surfaces: where a triangle is drawn

| Surface | Anchors it draws | How |
|---|---|---|
| Schematic canvas | Symbol, Wire (middle of the longest segment), Pin, Page point, Terminal symbols | replaces `problem_marks` -> `MarkKind::{Warning,Error}`; `MarkKind::Flag(sev)` in `novo-canvas` with the new glyph; popover from the app |
| Cabinet plate | Device, Rail, Duct, Door, Opening, Plate frame, Enclosure | painter pass after the layout; replaces the outline-only colouring (`BoxState`) as the problem cue; the outline stays as a second cue |
| 3D view | Device, Duct, Rail, Wire, Door, Opening, World | egui overlay at the projected anchor, constant size, fades when hidden behind a part, always on top when selected; replaces coloured chips (`view3d_heat_labels`) as problem cues |
| UI controls | Control, Job | small triangle on the corner of a rail item, top bar tile, button or job chip; the roll-up of everything below it (the Cabinet tile shows the worst flag of the cabinet) |
| Lists and tables | any | a leading triangle in the row (Terminals drawer, parts table, PLC list, wire list) instead of a coloured sentence in a cell |
| Result chips | Job | `SimStatus` stays the single state; Failed / OutOfDate / not converged are flags on the run chip |
| Top bar | all | the Issues pill becomes a triangle pill: red count, yellow count, click opens the Flags list |

## 5. The popover

Opens on click, Enter, or hover after 400 ms; Esc closes; Tab/Shift+Tab step through flags on the surface; F8 / Shift+F8 step through all flags and locate them (existing keys). Content, top to bottom:

1. Title line with the triangle and the source badge (Check, Cabinet, Heat, Run...).
2. Detail: at most three lines, numbers in the model's units.
3. Actions as icon buttons with tooltips: **Locate**, **Fix** (the command the engine proposes, one undo step; EMC fixes, Apply plate size, Centre rails, Set depth), **Waive** (the existing reason form), **Explain** (the question-mark icon: the long text, the standard clause, "model, not compliance" statements), **Copy**.

Never more than one popover open. The popover is a real `egui::Popup` and reachable by keyboard.

## 6. One place that lists them all

The Issues list is promoted to the **Flags list** (same panel, same F8, same waivers, `problem_flash`). It lists every flag of every source, groups by object or by area, filters by severity and source, has "Show waived", and keeps the "Recovered copy" and "checks pending" notes as flags on the Project anchor. The Cabinet problems pill and popup, the "N rule problems" pill, the Calculations findings tables, the Terminals drawer's Issues, EMC's findings list and the commissioning punch list become **filtered views** of the same store (or deep links into it). One count.

## 7. Jobs and failed actions replace the red and yellow toasts

373 toast sites say "Could not ..." with no place. They become flags with `Anchor::Job` or `Anchor::Control` and `Life::Until`:

- Export / import / save failed: a triangle on the file chip (project name in the top bar) or the Export command's button, popover "Disk full. Free some space or pick another drive" (the `error_words::plain` advice), actions **Retry**, **Open folder**, **Copy details**. Clears when the same job succeeds.
- Edit refused ("Cannot move a locked page", "Select a device first"): a transient yellow triangle on the selection or the tool in hand, gone on the next input. Replaces the refusal card and the red toast.
- Info toasts stay in the status bar (they are not problems). "Written: file" keeps its toast with Open folder.
- A tray: the top bar triangle pill also lists job flags (they are in the Flags list under "Jobs").

## 8. Results: models, estimates, staleness

The honesty rule stays (models are labelled). It becomes state, not paragraphs:

- A quiet **Model** badge on any model result (no triangle; text behind "?").
- **Out of date**, **not converged**, **failed**, **guessed input**, **not checked** are flags on the result's chip and on every drawing derived from it (3D hot labels, duct tint). Unchecked (missing data) draws dashed.
- Typed state replaces string state: `SimStatus::Failed(Reason)` with a `Reason` enum; `HeatState::failure()` no longer inspects message prefixes.

## 9. What each area becomes (from the ledger)

- **Door**: the five restatements (Issues, device hover, pill, side panel, top view) collapse to flags: red "Door cannot close" on the device, yellow "Door nearly touches", yellow "Door swing hits" on the door with the arc highlighted, red "Plate too big" on the plate frame with **Fix: Apply plate size**, dashed yellow "No depth" on the device. The all-clear sentences disappear.
- **Heat**: red "Too hot" and yellow "Tight" on the device (3D and plate), yellow "Out of date" and "Not converged" on the run chip, red "Run failed" with the reason in the popover, yellow on the opening for a thin fan, on the wire for "no cross-section". The verdict sentence at the top of the Heat panel is replaced by a roll-up triangle plus the hot-spot list with triangles. Coordinates with heat lane H7 (shell surface temperatures use flags for "over limit" only; the temperature fields stay).
- **Routing**: red "Duct over-full" and yellow "Duct nearly full" on the duct (tint stays; the percentage stays as the label), yellow "No duct" / "Through device" / "Wire size guessed" on the wire, yellow bend-radius and heat-neighbour on the corner.
- **Wiring**: wire range, point occupied, ferrule on the wire end; wire panel errors on the field; production export failures on the job chip.
- **Schematic**: all existing kinds keep severity and anchor; new popover, new glyph, Device/Terminal/Cable anchors now drawn (they are ignored today).
- Everything else follows the `sites/<area>.md` tables.

## 10. Coordination

- **Text sweep thread (X47GPB…, "?" icons)**: it owns class T (instruction text); this workstream owns P/J/R (problem, job, result text). Both touch the same files. Rule: a PR in either stream touches only its classes in a file; the other stream rebases. The ledger rows carry the class; the sweep thread filters `sites/*.md` by T.
- **Heat lane H7 (heat shell view)** and the heat researchers: do not edit `heat_status.rs` / `heat_verdict.rs` until lane T6 lands the typed `Reason`; H7 adds surfaces and temperatures, not messages. If H7 needs an "over limit" marker before T6, it uses `Anchor::Device` through the foundation API.
- **Menu census ceilings**: this stream adds a census test (below) with **one ceiling file per area**, not one shared line, so two lane PRs never edit the same number (the lesson of the V4 menu ceilings).

## 11. Plan: lanes and order

No stacked PRs. The foundation merges first; the lanes then start from main in parallel. Each PR names where the user finds the change, has QA scenarios at 1366x768 and 1920x1080 built from the exact head, and takes a scenario block from 9400 up (block per lane below). Sites per lane from the ledger.

| Lane | Scope | Sites | Scenarios | Depends on |
|---|---|---|---|---|
| T0a | New crate `novo-flags` (coordinator adds the empty crate in its own PR; then model, store, severity mapping, roll-up, F8 order, tests) | - | - | - |
| T0b | App foundation: `flag_triangle` widget in `novo-theme`, popover, `MarkKind::Flag`, Issues -> `FlagStore`, top bar triangle pill, Flags list, Waive/Explain/Locate in the popover; **schematic migrated** (all schematic ProblemKinds now draw the new triangle with popover) | 123 | 9400-9409 | T0a |
| T1 | Cabinet plate: physical Issues kinds anchored (Device/Rail/Duct/Plate), Cabinet pill and popup retired, `problem_lines` retired, rule-problems pill | 78 | 9410-9419 | T0b |
| T2 | Door and depth (4.1): flags on device, door, plate, enclosure in plate and 3D; side panel sentences removed | 2 + door.rs | 9420-9429 | T0b, T1 |
| T3 | 3D view flags: overlay at anchors, occlusion, vents, rail/duct/device editing refusals | in `cabinet.md` | 9430-9439 | T0b |
| T4 | Routing (4.3): duct, run and corner flags, run card sentences | 32 | 9440-9449 | T0b, T3 |
| T5 | Wiring (4.4): wire range/occupied/ferrule, wire panel, production jobs | 15 | 9450-9459 | T0b |
| T6 | Heat (4.2): typed `Reason`, run chip, verdict and hot-spot flags, sweep/search/what-if jobs, cooling, vents | 15 + heat notes | 9460-9469 | T0b, T3 |
| T7 | Schematic edit refusals: inspector, clipboard, duplicate, pages, outline, macros (toasts to transient flags) | in `schematic.md` | 9470-9479 | T0b |
| T8 | Terminals, cables, PLC, numbering, xref | 50 | 9480-9489 | T0b |
| T9 | Parts and catalogue: import preview, quality, picker, pins | 68 | 9490-9499 | T0b |
| T10 | Simulation, drive, machine, controller, live: findings on the timeline | 89 | 9500-9509 | T0b |
| T11 | EMC, safety, protection, calculations, standards | 49 | 9510-9519 | T0b |
| T12 | Import, export, reports, bench, workpack, handover, files: Job flags | 143 | 9520-9529 | T0b |
| T13 | Commissioning punch list and defects card | 25 | 9530-9539 | T0b |
| T14 | Shell: settings, add-ons, learn, assistant, update, startup, diagnostics | 87 | 9540-9549 | T0b |
| T15 | Close the loop: `Level::Error/Warning` toast and coloured-sentence helpers removed or made private to the flags module; census test goes to zero; docs and manual | - | 9550 | all |

Review order for Main reviewers: T0b gets the full review (it sets the pattern); lane PRs are mostly mechanical per file and check the ledger rows they close.

## 12. Tests and gates

- `novo-flags`: unit tests for stable ids, replace_source, roll-up, waiver, F8 order, severity mapping of all eleven vocabularies.
- A **census test** per area (`problem_census_<area>`): counts `Level::Error`/`Level::Warning` toast sites, warning/error coloured `Label`s and `banner` calls in the area's files; the ceiling only goes down; each lane PR lowers its own ceiling file, T15 asserts zero.
- QA scenarios per lane at 1366x768 and 1920x1080: one scenario creates the problem, screenshots the triangle, opens the popover, runs Locate / Fix, and asserts the flag disappears. Colour-blind check: a screenshot converted to greyscale must still show red and yellow as different (assert on the glyph mask in the scene dump, not on pixels).
- Accessibility: every triangle has an accessible name ("Error: Door cannot close, -K1"); popover reachable by keyboard.

## 13. Decisions taken (change in review if wrong)

1. Two colours only. Info stays out of triangles. Missing data is a dashed yellow, not red.
2. The error octagon is dropped; the fill separates red from yellow.
3. One store, one list, one count. The Cabinet keeps no private problem list.
4. Toast errors and warnings go away in the end (T15); info toasts stay.
5. Waivers keep their key format so existing projects keep their waivers.
6. Standards rules stay yellow until a rule is marked "safety relevant" in its data (a per-rule decision, not a code one).

## 14. Open questions for Tadjiro (not blocking)

- Should a **yellow** flag on a wire that is only a *guess* (for example "wire size guessed") show at all, or only in the list? Default: shown dashed.
- **Red** only for "cannot be built or run"; is "over its rated limit" (a hot device 3 K over) red too? Default: yes.
- Do you want a flags **sound or flash** when a long run finishes with a red flag? Default: no sound; the run chip pulses once.
