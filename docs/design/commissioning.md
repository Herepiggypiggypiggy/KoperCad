# Commissioning mode and punch list

Owner: Commissioning thread. Status: design, nothing built yet.

## Problem

At the machine the drawings are used to prove the build: every wire is continuity-checked, every
terminal torqued, every device labelled and tested. Today that is a marked-up paper printout. We want
the same walk-through inside the program, with who and when recorded, progress per page and per
cabinet, a punch list of what is still open, and a report to hand over. It must be usable on a laptop
or tablet in a cabinet: big targets, few clicks, no typing needed to tick something off.

## Principles

1. **Not part of the drawing.** A tick is a fact about the built machine, not an edit of the design.
   It is not undoable with design edits, does not dirty the schematic, and does not show up in
   Changes mode or revision diffs.
2. **Stored in the project file**, locally (SQLite, no server), so the checked state travels with the
   project and survives autosave and Save As.
3. **Engine first, UI thin** (CLAUDE.md). The rules live in a pure crate; the app only draws it.
4. **Reports reuse the existing machinery**: the report is a `novo_reports::ReportOutput`, so PDF
   pages and CSV/Excel come from the paths that already exist.

## What can be checked

A *target* is one object in the model: a wire, a terminal, or a device (a strip device is checked
through its terminals). Each target kind has the checks that make sense for it:

| Target | Checks |
| --- | --- |
| Wire | Continuity, Labelled |
| Terminal | Torque, Labelled |
| Device | Labelled, Tested |

A *check* is a fixed kind (`Continuity`, `Torque`, `Labelled`, `Tested`) with a state per target:
`Pending`, `Passed`, `Failed`, `NotApplicable`. Defaults: everything starts `Pending`. Which checks apply is a pure function
of the target kind, so progress totals are computed from the model, never stored. A target added after
commissioning started simply appears as open; a deleted target's records are ignored and cleaned up on
save.

A pass or fail records **who** (free text, defaulting to the OS user name, remembered per machine in
the app settings) and **when** (UTC, shown local), and an optional value (the measured torque in Nm,
the continuity reading in ohm) and note.

## Punch list

A punch item is an open issue, either linked to a target or free-standing:

- title, note, severity (Minor, Major, Blocking), state (Open, Fixed, Verified, WontFix), raised by/at, closed by/at;
- optional target (wire, terminal, device, or a page) so a click jumps to it on the canvas.

A `Failed` check creates a linked punch item automatically (so a failure can never be forgotten);
fixing the item sets the check back to `Pending` for a re-test. Items can also be added by hand for
things that are not drawn ("door seal damaged").

## Progress

Progress is derived: for any scope, `passed / applicable` over the checks that apply, plus the count of
failed checks and open punch items.

- **Per page**: targets that are drawn on the page (wires by their page, devices and terminals by the
  pages their symbols are on).
- **Per cabinet**: an enclosure structure node (`+CAB1`); a device counts in the enclosure of its
  location tag, a wire in the enclosure of its page's location, a terminal in the enclosure of its
  strip.
- **Project**: all targets.

A target belonging to more than one page counts once in the project and in each page it is drawn on.

## Storage

A new table in the project file, schema version 3 (a migration, with a committed v2 fixture as the
autosave design requires):

```
commissioning_checks (kind TEXT, id INTEGER, check TEXT, state TEXT, by TEXT, at TEXT, value TEXT, note TEXT,
                      PRIMARY KEY (kind, id, check))
punch_items (id INTEGER PRIMARY KEY, data TEXT NOT NULL)   -- JSON, like records
```

It is a side table next to `records` and `snapshots`, so `novo-core` needs no new record kind or API
(that is WS-02's) and the model stays a pure design. The recovery (autosave) file copies the tables
along with the rest, so a crash does not lose a day of ticks. Opening an older file creates the empty
tables; a newer file is refused as today.

## Where the code lives

- New crate `novo-commissioning` (pure, depends on `novo-core`, `novo-reports`): check kinds, state,
  progress queries, punch list, report builder. Needs the coordinator to add it to the crate list.
- `novo-storage`: the side tables and their load/save, in its own PR (small, with migration test).
- `novo-app`: Commissioning mode (below), kept thin.
- `novo-cli`: `kopercad-cli commissioning-report project.novo --csv out.csv --pdf out.pdf`.

## Report

Owned by the sibling thread "Commissioning report and tablet layout". It defines a plain
`novo_reports::commissioning::CommissioningInput` (sign-offs, check rows, punch rows); this crate only
provides `to_report_input`. The sections below are the agreed content.

A `ReportOutput` with these sections, so the PDF and the CSV come from the same rows:

1. **Summary**: per cabinet and per page, applicable / passed / failed / open and a percent.
2. **Checks**: one row per target and check: target tag or number, page, check, state, value, by, at, note.
3. **Punch list**: open items first, with severity, target, raised and closed by/at, note.

CSV uses the existing `novo-reports` CSV export (its delimiter option), one file per section
(`<name>-summary.csv`, `-checks.csv`, `-punch.csv`). PDF uses the generated-page path of `novo-pdf`, so
it gets the title block and bookmarks. A signature line (commissioned by, date) ends the PDF.

## Commissioning mode in the app

Entered from the mode switch (next to Changes mode) or the palette (`Commissioning`). The drawing stays
on the canvas; the dock switches to a **Commissioning panel**:

- Items are coloured on the canvas by state: grey open, green passed, red failed. Click a wire, terminal
  or device to select it; the panel shows its checks.
- Big buttons (min 44 px, larger text, spaced for touch): **Pass**, **Fail**, **Note**. `Space` passes
  the selected item and moves to the next open one in wire-number order; `F` fails it. The next open
  item is also a tap target, so a whole page can be walked without the mouse.
- A progress bar for the current page, the current cabinet and the project at the top.
- A **Punch list** tab: filter by state and severity, tap an item to jump, add/close/edit with a note.
- A compact layout switch (also automatic below ~1100 px width) that drops everything but the canvas,
  the progress bar and the three buttons.
- **Export report** opens the normal export dialog with PDF and CSV.

Editing the drawing is disabled while commissioning mode is open, so a stray touch cannot move a wire
at the machine. Leave the mode to edit.

## Build order (small pull requests)

1. This design.
2. `novo-commissioning` crate: model, progress, punch list (pure, tested). Crate registration by the coordinator.
3. `novo-storage`: side tables, migration v3, fixture, recovery copy.
4. Report builder and CSV/PDF export, plus the CLI command.
5. App: Commissioning mode, panel, canvas colouring, keyboard flow.
6. App: punch list tab, compact touch layout.

## Open questions (defaults chosen, change on feedback)

- The check kinds are fixed to four. If users want their own (insulation test, polarity), a later PR
  makes the list a project setting; the table already stores the kind as text.
- "Who" is free text, not a user account: the project is local-first and single-machine.
