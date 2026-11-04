# Problem triangles: audit ledger (phase 1)

Thread: "Redesign problems as warning triangles". Source state: main `4ac0057` (2026-11-04). Written by the problem-triangles thread. Companion: `docs/design/problem-triangles.md` (phase 2: the shared mechanism and the migration plan), and `sites/*.md` (every message site, one file per area).

## 0. What this is and how it was made

Goal from Tadjiro: every problem or result that pops up becomes a yellow or red triangle at the place that causes it (door, heat, routing, wiring, schematic, everything), instead of text menus and messages all over the program.

This ledger lists **every place in the code that tells the person something is wrong, doubtful or failed**, where it is shown today, a severity proposal and the object it belongs to. It was made by scanning the whole workspace (687 000 lines, 40 crates), not by sampling, with these scans:

| Scan | What it finds | Count |
|---|---|---|
| Check kinds | `novo_checks::ProblemKind`, the Issues list (section 2) | 76 kinds (13 red, 54 yellow, 9 info) |
| Other finding engines | structs and enums named Issue / Finding / Problem / Conflict / Note / Warning outside novo-checks (section 3) | 50 types in 22 crates |
| Toast sites | `Level::Error` and `Level::Warning` messages | 373 (230 red, 143 yellow) |
| Refusal sites | `Level::Info` messages that read as refusals ("Cannot", "Nothing to", "needs") | 42 (a lower bound; keyword based) |
| Coloured text | text, banners and fills painted with the warning or error colour | 298 sites in 91 files |
| Banners | `widgets::banner` calls | 31 |
| Notice strings | `notice`, `error`, `result` fields holding a sentence | 61 |

Together these are **776 rows** in `sites/*.md`, one per source line and channel (a line that two scans both found appears once per channel). Each row has file and line, the function, today's level, the message, a class and a triangle proposal.

What a scan cannot see: messages assembled far from where they are shown (door sentences, route sentences, heat verdicts). Those are listed by hand in section 4 and are the reason the "(text built in code)" rows exist. Where a row says "text built in code" the migration thread reads the function.

Class letters used in the site files: **P** problem that belongs to an object (triangle sits on it), **J** a job or file action that failed (triangle on the control or job chip, kept in a tray), **R** caveat on a result (triangle at the result), **N** plain information (no triangle), **T** instruction text (goes behind the question-mark icon; the text sweep thread owns it). The class is a first guess made by rules; the thread that migrates a file confirms every row.

## 1. The channels: how a problem reaches the screen today

There is no single path. Fourteen different ones exist.

| # | Channel | Where | Persists | Located | Notes |
|---|---|---|---|---|---|
| C1 | Toast, bottom left of the canvas | `app.rs toasts`, `Level::{Info,Warning,Error}` | Info fades; warning and error stay until clicked | no | 373 error and warning sources. Same text renews a count (x3). OS errors rewritten by `error_words::plain` |
| C2 | Status bar line | `status_bar.rs` | fades (TOAST_INFO) | no | takes Info toasts when the bar is on; last three on hover |
| C3 | Refusal card, top left of the canvas | `status_bar.rs refusal_card` | while the tool is held | no | long refusals (60+ chars) from the tool hint, also `dock.rs` |
| C4 | Issue pill in the top bar | `checks.rs issue_pill`, `widgets::issue_pill` | live | list only | counts errors and warnings ("2 errors . 5 warnings") |
| C5 | Issues list | `checks.rs issues_list` | live | click selects and jumps (F8) | the only complete list; waivers live here |
| C6 | Marks on the schematic | `checks.rs problem_marks` -> `novo_canvas::MarkKind::{Warning,Error}` | live | yes, but only for Symbol and Wire ids | triangle (warning), octagon (error), hover text |
| C7 | Cabinet problems pill and popup | `cabinet.rs` (`problem_summary`, `problem_lines`, `panel_problem_lines`) | live | device row select | a **second** problem system, built from `novo_layout::CabinetLayout::report()`; its count differs from the Issue pill |
| C8 | Cabinet banners | `cabinet.rs` (plate too big + Apply, last result) | until closed | no | `widgets::banner` |
| C9 | Door and depth side panel | `cabinet_door.rs sentences / swing_sentences` | live | no (top view colours bars) | the same facts as C5 and C7, third wording |
| C10 | Coloured outlines and tints | `novo-canvas cabinet.rs BoxState`, `cabinet_routes::tone`, `view3d_duct_fill`, `view3d_heat_labels` | live | yes, by colour only | colour is the only cue for the outline; duct tint also prints a percentage |
| C11 | Panel notes and `notice` strings | 61 fields; heat panels, routes panel, sizing, print preview, parts quality | until replaced | no | `HeatState::failure()` finds a failure by `starts_with("Cannot simulate")` |
| C12 | Banners in panels | `widgets::banner` (31) | until closed | no | variants, add-ons, build mode, bench, autopilot, EMC |
| C13 | Coloured labels in forms | 298 sites | live | no | warning-coloured sentences under a field or list |
| C14 | Result and run chips | `sim_chip.rs`, `heat_status.rs SimStatus`, 3D status chip | live | no | one state for every view (a good base: "Heat: out of date, simulate again", "not converged", Failed(why)) |

Also: modal dialogs with an error line (`dialogs.rs`, `panel::modal` users), the problem report (`diagnostics.rs`), reports and PDF text (`novo-reports` issues pages, standards report), and the CLI (`novo-cli`, out of scope: terminal output stays text).

## 2. The Issues engine (`novo-checks`, 76 kinds)

Severity is fixed per kind (`ProblemKind::severity`). `Problem.at` holds ids of any kind (Symbol, Wire, Device, Terminal, Cable, Structure, Page). **`problem_marks` marks only Symbol and Wire ids** (`checks.rs:49`); a problem whose `at` is a Device, Terminal, Cable or Structure gets only its list row and the F8 jump. That is every physical kind (21 of them: plate, rail, duct, door), which is why the Cabinet grew its own system (C7).

Proposal column: red for Error, yellow for Warning, "none" for Info (nothing is wrong yet; listed in the Issues list and in the object's popover, no triangle). This keeps the existing `Summary::is_clean` rule (infos are not issues).

| Kind | Label | Severity today | Triangle proposal | Anchor | Shown today |
|---|---|---|---|---|---|
| `DuplicateTag` | Duplicate tag | Error | red | each symbol of the devices | schematic mark + list |
| `TooManyContacts` | Too many contacts | Error | red | the surplus contact symbols | schematic mark + list |
| `PotentialConflict` | Potential conflict | Error | red | wires of the net | schematic mark + list |
| `UnconnectedPin` | Unconnected point | Warning | yellow | pin of a symbol | schematic mark + list |
| `LoneInterruption` | Interruption point without partner | Warning | yellow | the interruption symbol | schematic mark + list |
| `NoMainFunction` | No main function | Warning | yellow | the device's symbols | schematic mark + list |
| `SeveralMainFunctions` | Several main functions | Warning | yellow | the device's coils | schematic mark + list |
| `DuplicateTerminal` | Duplicate terminal | Warning | yellow | terminal symbols | schematic mark + list |
| `ZeroLengthWire` | Zero-length wire | Warning | yellow | wire | schematic mark + list |
| `CableCore` | Cable core | Warning | yellow | cable, core wire | schematic mark + list; cable screen text |
| `TerminalUse` | Terminal use | Warning | yellow | terminal symbol, wire | schematic mark + list; terminals drawer text |
| `Plc` | PLC | Warning | yellow | card or channel symbol | schematic mark + list; PLC list text |
| `WrongTerminalBlock` | Wrong terminal block | Warning | yellow | terminal symbol | schematic mark + list |
| `UndersizedWire` | Undersized wire | Warning | yellow | wire | schematic mark + list |
| `MissingPart` | No part | Info | none (listed, not a triangle) | device (symbols) | list only (Info) |
| `WireOverlap` | Wires overlap | Warning | yellow | both wires | schematic mark + list |
| `OpenWire` | Open wire end | Warning | yellow | wire end | schematic mark + list |
| `VoltageDrop` | Voltage drop | Warning | yellow | wire | schematic mark + list; Calculations text |
| `ShortCircuit` | Short-circuit rating | Error | red | protective device symbol | schematic mark + list; Calculations text |
| `DisconnectionTime` | Disconnection time | Error | red | wire | schematic mark + list; Calculations text |
| `ThermalWithstand` | Thermal withstand | Error | red | wire | schematic mark + list; Calculations text |
| `MotorStarter` | Motor starter | Warning | yellow | starter symbols | schematic mark + list; Calculations text |
| `ProtectiveConductor` | Protective conductor | Warning | yellow | PE wire | schematic mark + list; Calculations text |
| `UnplacedDevice` | Unplaced device | Info | none (listed, not a triangle) | device (no plate position) | list only (Info); Cabinet "unplaced" pill |
| `WireWithoutDuct` | Wire without duct | Warning | yellow | wire (plate and 3D path) | list; Cabinet routes note; 3D run card |
| `WireThroughDevice` | Wire through device | Warning | yellow | wire (plate and 3D path) | list; Cabinet routes note; 3D run card |
| `OverfullDuct` | Overfull duct | Warning | yellow | duct (stored as the structure) | list; duct tint with % on plate and in 3D |
| `UnsizedDuct` | Duct without size | Info | none (listed, not a triangle) | duct | list only (Info); routes panel text |
| `GuessedWire` | Wire size guessed | Info | none (listed, not a triangle) | the wires that were guessed | list only (Info); routes panel text; 3D run card |
| `Standard` | Standards | Warning | yellow | depends on the rule: symbol, wire, device, page | schematic mark if the object is drawn + list + Standards report |
| `CableRating` | Cable rating | Error | red | cable | schematic mark + list; Calculations text |
| `DeviceCollision` | Devices overlap | Error | red | both devices on the plate | list; Cabinet red outline and problems pill |
| `DeviceOutsidePlate` | Device off the plate | Error | red | device on the plate | list; Cabinet red outline and pill |
| `DevicesTooClose` | Devices too close | Warning | yellow | both devices | list; Cabinet yellow outline and pill |
| `RailOverflow` | Rail overfull | Warning | yellow | rail | list; Cabinet pill text |
| `DeviceOffRail` | Device off its rail's middle | Warning | yellow | device | list; Cabinet pill text |
| `FieldDeviceInCabinet` | Field device in the cabinet | Warning | yellow | device | list |
| `DuctOverlap` | Duct over duct, rail or device | Error | red | duct, rail or device | list; Cabinet pill text |
| `DeviceTooDeep` | Device too deep | Error | red | device (door cannot close) | list; Cabinet red top view bar, side panel sentence, pill |
| `DoorGapTight` | Door gap tight | Warning | yellow | device | list; Cabinet top view, side panel sentence, pill |
| `DepthUnknown` | Depth unknown | Info | none (listed, not a triangle) | device | list only (Info); side panel sentence |
| `PlateTooBig` | Plate too big | Error | red | plate / enclosure | list; Cabinet banner with Apply plate, side panel |
| `SafetyLevelNotMet` | Safety level not met | Error | red | safety function | list; Safety report text |
| `SafetyBinding` | Safety function binding | Warning | yellow | safety function | list; Safety panel text |
| `DoorSwing` | Door swing | Warning | yellow | device on the door | list; Cabinet side panel, pill |
| `Emc` | EMC | Warning | yellow | device, wire, cable or enclosure per finding | list; EMC view text; schematic ribbons |
| `AmbiguousInterruption` | Interruption branch | Info | none (listed, not a triangle) | interruption symbols | list only (Info) |
| `WireNumber` | Wire number | Warning | yellow | wires | schematic mark + list |
| `UntaggedDevice` | Device without tag | Warning | yellow | device symbol | schematic mark + list |
| `UnnamedTerminal` | Unnamed terminal | Warning | yellow | terminal symbol | schematic mark + list |
| `SameShortTag` | Same short tag | Info | none (listed, not a triangle) | devices | list only (Info) |
| `DuplicatePage` | Duplicate page | Warning | yellow | pages | list (page position only) |
| `DanglingJunction` | Dangling junction | Warning | yellow | junction | schematic mark + list |
| `UnwiredTerminal` | Unwired terminal | Warning | yellow | terminal symbol | schematic mark + list |
| `WireRule` | Wire breaks a rule | Warning | yellow | wire | schematic mark + list; wire panel |
| `StrayWireEnd` | Wire end on something | Warning | yellow | wire end | schematic mark + list |
| `ContactType` | Contact numbering | Warning | yellow | contact symbol | schematic mark + list |
| `ContactPosition` | Contact beyond relay | Warning | yellow | contact symbol | schematic mark + list |
| `ContactDrawnTwice` | Contact drawn twice | Warning | yellow | contact symbols | schematic mark + list |
| `VoltageClassMix` | Voltage classes mixed | Warning | yellow | symbols under one function code | schematic mark + list |
| `ConductorContinuity` | Earth or neutral broken | Warning | yellow | PE/N terminal | schematic mark + list |
| `MotorPhases` | Motor phases | Warning | yellow | motor symbol | schematic mark + list |
| `CoilSupply` | Coil supply | Warning | yellow | coil symbol | schematic mark + list |
| `PlugPinOneSided` | Plug pin on one half | Warning | yellow | plug pin | schematic mark + list |
| `RelaySize` | Relay too large | Warning | yellow | relay device | schematic mark + list |
| `CoilOnPowerFeed` | Coil on a power feed | Warning | yellow | coil symbol | schematic mark + list |
| `CableText` | Cable text | Warning | yellow | cable | schematic mark + list |
| `NeutralUse` | Neutral use | Warning | yellow | device | schematic mark + list |
| `PlugPins` | Plug pins | Info | none (listed, not a triangle) | plug device | list only (Info) |
| `TagRule` | Tag rule | Warning | yellow | device | schematic mark + list |
| `DisplayedTag` | Displayed tag | Warning | yellow | tag text on a symbol | schematic mark + list |
| `StructureCode` | Unknown structure code | Warning | yellow | device | schematic mark + list |
| `WireRange` | Wire size at terminal | Warning | yellow | wire end at a connection point | schematic mark + list; Terminals drawer |
| `PointOccupied` | Terminal point full | Warning | yellow | connection point | schematic mark + list |
| `FerruleUse` | Ferrule at terminal | Warning | yellow | wire end | schematic mark + list |
| `PinNames` | Pin names | Info | none (listed, not a triangle) | device | list only (Info); parts pins window |

Counts by proposal: 13 red, 54 yellow, 9 no triangle.

Needs a human decision (not code): `Standard` kinds are all yellow today whatever the rule; some IEC 60204-1 rules are safety relevant and could be red per rule. `DuplicatePage` and the physical kinds have no good anchor today (see section 5, finding F2).

## 3. Other finding engines (outside `novo-checks`)

Each of these has its own type, severity words and display. "Shown by" is what the search found in `novo-app`; a dash means no consumer was found by name, and it may reach the person through a `ProblemKind`, a text line, or not at all. The owner confirms in the migration.

| Crate :: type | Severity words | What it says | Shown by (today) | Anchor to use |
|---|---|---|---|---|
| `novo-terminals` check `Issue` | Info / Warning / Error | unused, duplicate, wrongly used terminals | Terminals drawer rows, Issues via `TerminalUse` | terminal on the strip and on the page |
| `novo-terminals` `RenameProblem` | Empty / Taken | terminal rename refused | `terminal_edit.rs` toast | the edited cell |
| `novo-cables` check `Finding` (7 kinds) | none (all Warning/Error via `CableCore`) | spare core in use, PE core misused, too many cores, shield open, thin core | `cable_controls.rs`, cable screen text | the core row, the cable |
| `novo-plc` index `Issue` (10 kinds) | none (all `Plc` Warning) | overlapping addresses, channel drawn twice, no layout | `plc_card.rs`, `plc_link.rs`, PLC list text | the card or channel |
| `novo-plc` generate `Note` | none | notes on a generated list | `plc_list.rs` text | the list row |
| `novo-xref` index `Issue` (4) | none | no main, several mains, too many contacts, lone interruption | through `ProblemKind` | the symbol |
| `novo-numbering` check `Conflict` | Info / Warning / Error | wire number reused, gaps, locks | `numbering.rs` panel | the wires, on the page |
| `novo-emc` `Finding` | Error / Warning / Info | EMC-01..20 findings, with fixes | EMC view, `ProblemKind::Emc` marks | the device, wire, cable or enclosure it names |
| `novo-checks` calc `Finding` | Error / Warning / Info | voltage drop, short-circuit, motor starter | `calculations.rs` text; `ProblemKind` | wire or protective device |
| `novo-checks` protection coordination `Finding` | Error / Warning / Info | selectivity, SCCR | `protection.rs`, `calculations.rs` text | the node in the protection tree and its device |
| `novo-checks` standards `Finding` (document) | text, page, reason | design report items | `standards_report.rs` | the page |
| `novo-safety` `Problem` (one `message`) | none | subsystem cannot reach its PL | `safety_*.rs` text | the safety function |
| `novo-safety` `Severity` S1/S2 | (not a problem severity: ISO 13849 risk graph) | | | must not be confused: rename or keep out of the triangle code |
| `novo-commissioning` punch `Severity` | Minor / Major / Blocking | punch list | `commissioning.rs`, `defects_card.rs` | the checklist item; Blocking = red, Major = yellow, Minor = none |
| `novo-controller` validate `Issue` | Warning / Error | controller profile mistakes | `controller_import.rs`, `controller_card.rs` | the profile path (`control[hoist]`) |
| `novo-controller` import `Warning` | Info / Warning | import guesses and skipped rows | `controller_import.rs` | the import row |
| `novo-controller` `BindingProblem` (13) / `BindingFinding` | none | unbound, duplicate, machine syntax | `controller_live.rs` | the signal row and the bound device |
| `novo-machine` `*Finding` (5 types: hoist, drive-hoist, travel, cl-winch, cl-pair) | kind + time | runtime findings of a simulation (brake slip, limit hit) | `drive_run.rs`, `hoist_run.rs`, `winch_scenarios.rs` | the machine part at the time on the timeline |
| `novo-drive` closed loop `Finding` | Info / Warning / Problem | setting lets the load move or trip | `drive_run.rs`, winch scenarios | the drive parameter |
| `novo-drive` `ResistorFlag` (6) | none | braking resistor too small | - | the resistor device |
| `novo-view3d` `LimitFlag` | Upper / Lower | hoist limit reached | `hoist_3d.rs`, `hoist_run.rs` | the hook in 3D |
| `novo-sim` `PinProblem` (7) | none | simulation pin mapping | - (to confirm) | the symbol pin |
| `novo-sim` network `Note` | kind | solver notes | `simulate.rs`, `live.rs` | the symbol |
| `novo-routing` `Problem` (6) | none | free air, stub through device, guessed wire, unsized duct, overfull duct, clearance unmet | `ProblemKind` (5 of 6), `cabinet_routes.rs` notes | the wire or duct |
| `novo-layout` check `Problem` (WrongZone, Clearance, Separation) + `Violation` | none | zone rules | `cabinet_rules.rs` "N rule problems" pill | the device pair |
| `novo-layout` `OpeningConflict`, `opening_problems` | none | device covers a wiring opening | `cabinet.rs` problem_lines | the opening and the device |
| `novo-layout` `SurfaceProblem` (3) | none | surface name or size wrong | `cabinet.rs` coloured text under the bar | the surface |
| `novo-layout` sizing `Note` (8) | none | no catalogue, guessed sizes, no loss data, nothing fits | `sizing.rs`, heat panels | the enclosure sizing result |
| `novo-layout` `RowNote`, `BarNote` | none | rail rows and rail bars | `cabinet_rail_ends.rs`, rules panel | the rail |
| `novo-parts` quality `Finding` | criterion / fill / note | part data quality | `parts_quality.rs` | the part row |
| `novo-parts` tablepreview `Issue` | Note / Warning / Error | spreadsheet import preview | `parts_table.rs`, `parts_import.rs` | the row and column of the sheet |
| `novo-reports` `DesignProblem` (7) | none | report designer layout mistakes | `designer.rs` | the object on the designed page |
| `novo-reports` i18n `CatalogProblem` | none | translation catalogue | `languages.rs` | the language row |
| `novo-workflow` `parametric::Problem` (9) / `options::Finding` (3) / `SheetProblem` / `userkeys::Problem` | none | variant parameter, option, sheet generation, key binding mistakes | `variants.rs`, `userkeymap.rs` | the parameter, option or key row |
| `novo-learn` lint `Finding` | Error / Warning | lesson authoring mistakes | CI only | not user facing; stays text |
| `novo-interop` `NetIssue` (9) | none | EPLAN net import: unpaired interruption, dangling leg, skew line | - (to confirm: `eplan_import.rs` notes) | the imported object |
| `novo-symbols` validate `Issue` | none | symbol definition faults | `symbol_folder.rs` problems list | the symbol file row |
| `novo-canvas` `text_audit::Conflict` | none | overlapping text | `numbering.rs`, scene dump | the two texts |
| `novo-revisions` `ReviewNote` | status | review notes, not problems (they use the Note mark) | `review_notes.rs` | keep as is: notes are not triangles |
| `novo-ai` `stream.rs`, `eval.rs` | (colours) | assistant errors | `assistant.rs` | the assistant message |
| `novo-theme` `widgets.rs` | `Level::{Info,Success,Warning,Error}` | banner, pill, toast icons | all | the shared vocabulary to reuse |

Severity vocabularies found: `Severity {Error,Warning,Info}` in novo-checks, novo-emc; `{Info,Warning,Error}` (opposite order) in novo-terminals, novo-numbering; `{Warning,Error}` in novo-controller validate and novo-learn; `{Info,Warning}` in controller import; `{Note,Warning,Error}` in novo-parts; `{Info,Warning,Problem}` in novo-drive; `{Minor,Major,Blocking}` in commissioning; `Level {Info,Success,Warning,Error}` for toasts and banners; `Tone {Idle,Ok,Busy,Over}` for duct fill; `BoxState` for plate outlines. Eleven ways to say red or yellow.

## 4. Area chapters (what was asked for first)

Each lists the messages by hand where the scan sees only code that builds text. The full per-site tables are in `sites/<area>.md`.

### 4.1 Door opening and depth

Sources: `novo-checks/src/door.rs`, `novo-layout` enclosure and swing, `cabinet_door.rs`, `cabinet.rs`.

The same facts are told in **five** places with different words, and none of them is on the thing itself in 3D:

| Fact | Issues list (C5) | Device hover and pill (C7) | Side panel (C9) | Top view | Other |
|---|---|---|---|---|---|
| A device is too deep, the door cannot close | `DeviceTooDeep` red "…" | "N mm too deep: the door cannot close" | "-K1 is N mm too deep: the door cannot close." | bar painted red | Cabinet pill count |
| Less than 10 mm (MIN_DOOR_GAP_MM) in front of a device | `DoorGapTight` yellow | "-K1 leaves only N mm in front of it: the door nearly touches" | "-K1 leaves N mm to the door (at least 10 mm)." | bar painted yellow | |
| Plate bigger than the enclosure | `PlateTooBig` red | "The mounting plate is bigger than the inside of the enclosure" | "The mounting plate is bigger than the inside." | | banner with "Apply plate W x H mm" |
| Open door needs room in front | `DoorSwing` yellow | "The open door needs N mm more free room in front of the enclosure" | "The open door needs N mm in front, M mm are free (K mm short)." | arc drawn | |
| Open door needs room beside the hinge | `DoorSwing` yellow | "...beside the hinge side" | "It also needs N mm beside the hinge, ..." | arc | |
| A device on the door touches something when it swings | `DoorSwing` | `h.message(tag_of)` | same | | |
| Device has no depth in its part | `DepthUnknown` info | | "N placed devices have no depth in their part and are not checked." | | |
| A device on the door has no depth, was not swept | | | "-K1 on the door has no depth and was not swept." | | |
| Not checked against door devices (plate devices without depth) | | | "Not checked against the door devices: N on the plate without a depth." | | |
| All clear | | | "Every placed device fits behind the door (N mm usable depth)." / "Devices on the door clear everything up to N degrees." | | |

Proposal: red triangle on the device (too deep, plate too big on the plate frame), yellow triangle on the device (gap tight, door swing hit, front or hinge room short on the **door itself** in the top view and on the enclosure in 3D). "Has no depth" is not a problem of the design, it is missing data: a grey-yellow "unchecked" triangle on the device (see design, state `Unchecked`). The all-clear sentences disappear: no triangle is the all-clear. The side panel keeps the form (enclosure size, door), not the sentences.

### 4.2 Heat simulation (and cooling)

Sources: `heat.rs`, `heat_status.rs`, `heat_verdict.rs`, `heat_spots.rs`, `heat_live.rs`, `heat_sweep.rs`, `heat_whatif.rs`, `heat_layout.rs`, `heat_cooling.rs`, `heat_wires.rs`, `heat_flow.rs`, `view3d_heat*.rs`, `view3d_vents.rs`, `sizing.rs`, `novo-layout/src/sizing.rs` (Note), `novo-thermal`, `novo-cfd`.

| Message (as written) | Where shown | Proposal | Anchor |
|---|---|---|---|
| "Too hot: -F1 reaches 90 C, 3 K over its limit (1 more over)." (`heat_verdict`) | top of the 3D Heat panel | red | the hot device in 3D and on the plate |
| "Tight: -F1 is at its limit (83 C), but the model's uncertainty band reaches 2 K over." | same | yellow | the device |
| "No device in this result gives off heat, so there is nothing to judge." | Heat panel, hot-spot list | no triangle (empty state) | - |
| Hot-spot rows (devices by margin), air pockets (`heat_spots::air_pockets`) | Heat strip list | red or yellow per margin; pockets yellow | device; the pocket position in 3D |
| `SimStatus::Failed(why)`: "Cannot simulate...", "The simulation failed: {e}", "The heat-up failed: {e}", "Open an enclosure with a mounting plate to simulate" | status chip, Cabinet strip, 3D chip | red | the run chip (and the enclosure when it is the cause) |
| `SimStatus::OutOfDate` "Heat: out of date, simulate again"; "The result shown is from before your last change: run Simulate again." (vents) | chip, panels | yellow | the run chip |
| "(not converged)" `SimStatus::Done{converged:false}`; "Stopped: the last result is not converged" | chip | yellow | the run chip and every result drawn from it |
| "Coarse 3D . not validated", "Model estimate: ... Not validated against measurements" footers (`heat_sweep::FOOTER`, `heat_layout`), "MODEL, NOT MEASURED" badge, "(model)" tags | footers, badges | not a problem: stays as a quiet "Model" badge; the triangle is used only when the model is outside its range | - |
| "The sweep failed: {e}", "The search failed: {e}", "The what-if failed: {e}", "...stopped unexpectedly" | `notice` in each panel | red | the job's button (Sweep, Search, What-if) |
| "Cannot search: set the enclosure size and the plate first", "Cannot try: {e}", "No device stands on the plate, so there is no heat to cool.", "No device stands on a rail..." | `notice` | yellow (precondition missing) | the button and the missing enclosure or plate |
| "...list is out of date: sweep again", "The cabinet changed after this search. Search again before applying." | panels | yellow | the list header and the Apply button |
| "Nothing listed is large enough: open for the options." (cooling) | Cooling selection | yellow | the cooling result row |
| "{n} wire(s) in ducts have a current but no cross-section: no heat counted for them." (`heat_wires`) | Heat panel note | yellow | the wires (they have a position in 3D) |
| "No device states an ambient limit, so it starts at 40 C." / "The heat includes N W from wires in ducts (planning upper value)." | cooling panel | no triangle (an assumption): small "?" note | - |
| Vents: "Opening N is a fan only M mm across: ... Check its size and datasheet flow." ("thin fan"), "None: the enclosure is sealed. Add a grille low and another high...", "The grille already in the cabinet is too small for this flow..." | `view3d_vents.rs` | yellow | the opening in 3D |
| "Air in X, out Y m3/h (model); Z m3/h (N %) apart." | Vents panel | yellow when over a tolerance | the openings |
| Sizing `Note`: NoCatalog, GuessedSizes, NoLossData, NothingFits, DeviceTooBig, TooDeep | `sizing.rs` | yellow (NothingFits, DeviceTooBig, TooDeep red) | the enclosure; the device |
| `list_path()` errors "{LIST_FILE}: {first}" and "Could not read {path}: {e}" (own cooling list) | cooling panel | yellow / red | the list row |

### 4.3 Cabinet routing

Sources: `cabinet_routes.rs`, `novo-routing` plan Problem, `view3d_runs.rs`, `view3d_run_checks.rs`, `view3d_duct_fill.rs`, `view3d_wires.rs`.

| Message | Where | Proposal | Anchor |
|---|---|---|---|
| `OverfullDuct` (fill over limit), tint `Tone::Over`, label with % | plate and 3D tint, list | red | the duct |
| `Tone::Busy` (above 80 % of the limit) | tint only | yellow | the duct |
| `UnsizedDuct` | list (info), routes panel | yellow (the fill cannot be checked) | the duct |
| `WireWithoutDuct`: "No duct reaches both ends: a straight line. Place a duct or move the device." | list, wire panel, run card | yellow | the wire |
| `WireThroughDevice` | list, run card | yellow | the wire at the device |
| `GuessedWire`: "No wire size set: a 3 mm wire is assumed in the fill" | list (info), run card | yellow | the wire |
| "Routed, but a device depth is unknown, so it is a little short. Add the depth to the part." | Run card | yellow | the wire |
| "The stated cable length is shorter than the route inside the cabinet, so it cannot be right..." | Run card | red | the cable |
| "...the wire outside it is not stated, so the length stays an estimate" and similar (4 more length-basis sentences) | Run card | yellow, or no triangle where the basis is stated by the person | the run |
| "Bend radius (model): the wire 3.0 mm needs 18 mm", "Bend radius not checked: no wire size or cable diameter is set", "Heat (model): 20 mm or more from every other device with a known loss of 5 W" | Run card checks | yellow when violated, "unchecked" state when data is missing | the corner or the neighbour in 3D |
| Entry point: "A stored entry point is incomplete or outside the plate and cannot be used. Set a new one." | routes panel | yellow | the entry marker |
| "Not moved: a duct cannot lie over another duct. Move it clear..." (3D duct drag) | transient text in 3D | yellow | the duct while dragged |
| `DuctOverlap` ("Lies under a duct", "The rail at x runs under a duct for N mm") | list, device hover | red | the rail or device and the duct |

### 4.4 Cabinet wiring (wire it, production, cut lists)

Sources: `view3d_wire_it.rs`, `wire_it_export.rs`, `wire_production.rs`, `wire_panel.rs`, `view3d_wires.rs`, `workshop.rs`, `workpack.rs`, `novo-checks` `WireRange`, `PointOccupied`, `FerruleUse`.

| Message | Where | Proposal | Anchor |
|---|---|---|---|
| `WireRange` (wire too thick or thin for the point), `PointOccupied`, `FerruleUse` | list, schematic mark, Terminals drawer column | yellow (red for maker or typed data stating a hard limit: today `WireRange` is "Error only for maker/typed data" in the checks, shown as Warning kind: confirm) | the wire end |
| "A core of a cable: the cable is pulled once, so no path is drawn for the core." | Wire it card | no triangle (explanation) | - |
| "This wire runs on another cabinet's plate: open that cabinet to see it." | Wire it card | no triangle (navigation hint, becomes a link) | - |
| `wire_panel`: `Level::Warning "Wire: {e}"` | toast | yellow | the wire or the field |
| `wire_production.rs` 3 error toasts, `wire_it_export.rs` 2 | toast | red (J) | the Export button / job chip |
| Cut list and label lines "Cut ~N mm (routed + 2 x each end)" | Wire it card | no triangle (a result) | - |

Wires have no tag, so a triangle on a wire sits at its middle on the plate and in 3D, and at its middle in the schematic.

### 4.5 Schematic (editing, wiring, checks)

Sources: the 76 `ProblemKind`s of section 2 (all schematic kinds already have a triangle or octagon mark and an Issues row), `inspector.rs` (13 toasts), `clipboard.rs`, `duplicate.rs`, `pages.rs`, `outline*.rs`, `net_trace.rs`, `potentials.rs`, `tidy.rs`, `pin_offer.rs`, `macro_*.rs`, `numbering.rs`, `device_renumber.rs`. 123 sites in `sites/schematic.md`.

What is new here: (a) the mark shape (octagon for red) becomes a triangle with a different fill so red and yellow differ without colour; (b) the mark gets a popover with the detail, the waive action and a fix action where one exists; (c) refusals of edits (13 inspector toasts, clipboard, duplicate, pages) become a transient yellow triangle on the selection or the object the refusal is about.

### 4.6 The rest

`sites/terminals.md` (terminals, cables, PLC, 50), `parts.md` (68), `sim.md` (89), `safety.md` (49), `files.md` (143), `commissioning.md` (25), `shell.md` (87), `cabinet.md` (78, plate, rails, surfaces, 3D editing). Per-area anchors are in the first lines of each file.

## 5. Findings of the audit

F1. **Eleven severity vocabularies** (section 3). Needs one `Severity` in a shared place (proposal in the design: `novo-theme` holds the vocabulary because novo-core and novo-checks must not depend on it; the engines map into it).

F2. **Triangles only reach Symbol and Wire.** `problem_marks` ignores Device, Terminal, Cable, Structure and Page ids. Physical, door, duct and rail problems therefore get no triangle on the object in any view. The Cabinet answers with its own parallel system (C7).

F3. **Two problem counts.** The top bar pill counts Issues; the Cabinet pill counts `layout.report()` entries plus door and swing problems (`cabinet.rs:782`). Same facts, different numbers, different words (4.1).

F4. **Shape.** The schematic uses a triangle for warnings and an octagon for errors (`marks.rs`). The ask is a yellow or red triangle. The design keeps a second cue for colour-blind users in the triangle's fill (red solid, yellow outlined) and glyph, not in a different outline.

F5. **Failed actions are anonymous toasts.** 230 red and 143 yellow toast sites say "Could not ..." with no place; they vanish on click, and the count sticks (x3). Class J.

F6. **Result caveats are prose.** "Out of date", "not converged", "model", "estimate", "guessed", "not validated", "not checked" appear in ~150 sentences across heat, routing, sizing and calculations. They carry the honesty rule of this project (models are labelled), so they must stay reachable, but as a state of the result (badge plus triangle when it matters), not as paragraphs.

F7. **String-typed failure state.** `HeatState::failure()` decides what a failure is by `starts_with("Cannot simulate" | "The simulation" | "The heat-up" | "Open an enclosure")`. Any reworded message silently stops being a failure. 61 `notice` strings have the same weakness.

F8. **Colour-only cues**: plate outlines (`BoxState`), duct tint (percentage is also printed, good), `Tone`, hot labels in 3D. Needs the shape cue.

F9. **Three palettes for red and yellow**: `tokens::{WARNING,ERROR}` (D6A55C / E0766C), `ui_palette()` (themed), and `CabinetStyle` (215,60,60 / 190,120,20). The UI rule says `ui_palette()`; the cabinet canvas and 3D draw their own.

F10. **Engines with no visible consumer found**: novo-sim `PinProblem`, novo-drive `ResistorFlag`, novo-interop `NetIssue`, novo-symbols `Issue` (see section 3). The migration confirms each; if none is shown, the finding is invisible today.

F11. **Waivers exist only for Issues kinds** (`waive`, `waived_problems`). A triangle from any other engine cannot be waived; the shared mechanism must carry the waiver key.

F12. **The lesson `quiet_checks`** hides problem marks while a lesson about something else runs. Keep it in the shared mechanism.

F13. **Existing good bases to reuse**: Issues engine and waivers, locate-on-click (F8), `problem_flash`, `MarkKind` glyph drawing with hover text, `SimStatus` (one state for every heat view), `widgets::Level` icon and wash, `error_words::plain`, `error_fg` palette fields.

## 6. What stays text

Triangles do not replace: file and path names in confirmations (the written-file toast with "Open folder"), the problem report text, PDF and report content (exports keep their lists), CLI output, lesson text, and the legal and standards statements the honesty rules require ("model, not compliance"). They are shorter and move behind the question-mark icon where the text sweep thread (X47GPB…) takes them.
