# Discoverability: how a new user finds what is built

Status: proposal for review. Nothing here is built yet. Thin UI over existing commands, as
`docs/DESIGN.md` asks. Owner: the Onboarding and discoverability thread.

## Why

Features land faster than the shell shows them. Every command is in Ctrl+K, but a new user does not
know to press Ctrl+K, and a user who does can find rows that say "is not in the app yet". The tool dock,
the palette and the outline each explain themselves a little; nothing explains the whole.

This page defines the layers that make a feature findable, audits the current state, and orders the
fixes. It builds on, and does not repeat, `docs/ONBOARDING-DESIGN.md` (example project and fast-keys
walk-through, PR #428) and `docs/design/ui-guidelines.md` (tokens and widgets).

## Rule: a feature is not done until it is findable

A feature PR is complete only when all of these hold. The reviewer checks them like a test.

1. It has a command in `novo-workflow::commands` (so Ctrl+K finds it) with keywords a user would type.
2. It has a one-line tip (layer 3) if it has a control, a mode or a key.
3. Its panel has an empty state that says what to do next.
4. A refusal says why and what to do, in the dock hint or a message, never a silent no-op.
5. If it has a default key, the keymap has it and the shortcut sheet shows it (generated, no list to edit).

## The layers

### 1. Shortcut sheet (`?`, Help menu, `view.shortcuts`)

A floating, searchable sheet generated from the live `Keymap` and `COMMANDS`, so a remapped key shows
its real key and a new command appears with no extra work.

- Grouped by `GROUPS` (Project, Draw, Edit, Navigate, View, Check, Export). A filter field at the top:
  typing filters, Esc clears, a second Esc closes. Each row is label, keycaps, and the scope when it is
  not global ("on the canvas").
- Rows are clickable: a click runs the command and closes the sheet, so it doubles as a menu.
- Commands with no key are listed too, with "Ctrl+K" as the way in.
- Opens and closes on `?`, from the Help menu and from the palette. It never takes a key from a text
  field (`Key::types_text`).
- The first-run card (PR #428) links to it from its Ctrl+K row.

### 2. Help menu

The mark menu is the only menu and holds project actions. Add a Help group at its end, above About.
There is no menu bar, by design.

| Item | Does |
| --- | --- |
| Keyboard shortcuts `?` | Opens the sheet |
| Fast keys walk-through | Brings the card back (`help.walkthrough`, PR #428) |
| Open example | Same as the start panel |
| What's new | Opens the panel (layer 8) |
| Show hints again | Resets the hint counters (layer 4) |
| Documentation | Opens the project's docs page in the browser (no install, no admin) |
| About | As now |

Every item is also a command, so Ctrl+K "help" lists them all.

### 3. Tips: tooltips that carry the key

One helper in the app shell, `tip(response, label, command_id)`, reads the key from the keymap so it is
never typed by hand.

- Every icon-only control and every dock chip has a tooltip: `Label · Key`, and where it has a second
  mode, `· Shift+W switches` (the wiring chip already does this; it becomes the rule).
- A disabled control says why: `Revisions · Issue a revision first`. "Comes later" is only for planned
  work, never for a precondition.
- Default egui tooltip delay; no tour, no pop-up.
- A test walks the `Action` list and the dock chips and fails when one has no tip.

### 4. Hints that teach once

The dock hint and the empty-page hint name the next action and its key (DESIGN §3, "first five
minutes") and stop showing once followed three times. Counters are per hint id in `Settings`
(`hints_seen`). Hint ids are strings declared next to the hint text, so a new hint is one line.
"Show hints again" clears the counters.

Missing today: the empty page ("Press Q or Space to place a symbol · ? for keys"), the empty board
("Add a page under a function") and the first-wire hint.

### 5. Empty states

Every panel that can be empty says what puts something in it and offers the one button that does.
Format: a faint line, then at most one button from `novo_theme::widgets`. No illustrations.

Already done: the Terminals and Cables drawers, Changes, the cabinet. Gaps are in the audit table.

### 6. Messages and the status line

There is no status bar and this page adds none (DESIGN §3: hint in the dock, coordinates in the HUD,
checks in the pill). The rule is for `App::message`: it names the object and the fix.

### 7. Error and refusal wording

Pattern: what happened, to what, and what to do, in plain words.

| Now | Better |
| --- | --- |
| "Keyboard shortcuts is not in the app yet" | Fixed by building the sheet. For real gaps: "Paste is not available yet." |
| "That device is not drawn on any page" | "-K1 is not drawn on any page. Place it with Q." |
| "Text · Comes later" | Keep for planned work only |

An audit pass lists every `Level::Error` and `Level::Warning` call site and rewrites each to the
pattern. Raw SQLite, I/O or parser strings never reach the user unwrapped: the message names the file
or action, with the cause after it.

### 8. What's new

A small panel, opened from Help and once after an update (never on a first run), listing the release's
changes in three to eight plain lines, each with a "Try it" that runs the command. The text is a data
file `assets/whats-new.md`, edited by the release thread, one section per version, read at runtime.
The last seen version lives in `Settings`. No network.

### 9. Start panel

Already offers New, Open, Open example and Recent; PR #428 adds Try the fast keys. Add one line under
the buttons: `Ctrl+K finds every command · ? lists the keys`. Nothing else, so the panel stays calm.

## Audit: gaps found on main (074138a)

| # | Finding | Evidence | Fix | Layer |
| --- | --- | --- | --- | --- |
| 1 | `?` is bound and does nothing. The sheet is promised in DESIGN §3 and the keymap, but nothing handles `view.shortcuts`; Ctrl+K answers "Keyboard shortcuts is not in the app yet" | `keymap.rs:397`, fallthrough at `workbench.rs:535` | Build the sheet | 1 |
| 2 | Eleven listed commands have no handler in the app: `draw.tidy`, `edit.paste`, `edit.paste_keep_tags`, `edit.renumber`, `edit.select_all`, `export.bom`, `nav.changes`, `project.rules`, `view.cabinet_table`, `view.override_marks`, `view.shortcuts`. This is a string search, so some may be reached another way; the audit PR confirms each | `commands.rs` against `crates/novo-app/src` | Wire each, or hide it from the palette until wired, so no row ends in "not in the app yet" | 7 |
| 3 | No Help entry anywhere; About is the only help-like item | `project_menu` in `app.rs` | Help group | 2 |
| 4 | About 20 tooltips in the whole app; the issue pill, saved state, outline modes and most drawer buttons have none | `on_hover_text` count | `tip()` and the coverage test | 3 |
| 5 | The empty-page and empty-board hints from DESIGN §3 are not built | no "Press Q" in the code | Hints with counters | 4 |
| 6 | "Show hints again" is promised and absent | DESIGN §3 | Counters and the Help item | 4 |
| 7 | Commands with no other entry: Preferences, Project standard, Issue revision, PLC cards | `run_command` arms | A labelled control where each applies (inspector, outline, drawer) | 5 |
| 8 | New panels from other threads (Parts window #429, contents page #415, commissioning) have no rule for being findable | open PRs | The rule at the top, added to the PR template | rule |
| 9 | No what's-new; users of the experimental builds cannot tell what changed | none | Panel | 8 |
| 10 | Error text is written per call site, some raw | `Level::Error` sites | Wording pass | 7 |

## Order of work (small draft PRs, no build)

1. This page.
2. Shortcut sheet, `?` and the Help group (findings 1, 3). Highest value; fixes a broken promise.
   Files: `app/help.rs`, hooks in `app.rs` and `actions.rs`.
3. `tip()` and the tooltip pass with its coverage test (4).
4. Hint counters, empty-page and board hints, Show hints again (5, 6).
5. Dead-command audit (2). This opens as a list for the owning threads (editor for paste, reports for
   the BOM), not as code.
6. Empty-state and error-wording passes (7, 10).
7. What's new (9).

## Coordination

- First-run thread (PR #428) owns the card, practice project and example. This work adds only the Help
  item for `help.walkthrough` and the shortcut-sheet link, and edits none of its files.
- Workspaces and design-system threads: the sheet and `tip()` use `novo_theme` tokens and widgets; a
  new widget goes to the design-system thread first. Panels that move keep their command ids, so the
  sheet and hints do not change.
- Workflow thread: owns `commands.rs` and the keymap. This work reads them and adds only `help.*` ids
  and keywords.
- Feature threads: the rule at the top is theirs to meet; the audit PR sends each its list.

## Out of scope

Video or animated tours, Dutch text (the language work), telemetry on which hints are used (local-first,
nothing leaves the machine).
