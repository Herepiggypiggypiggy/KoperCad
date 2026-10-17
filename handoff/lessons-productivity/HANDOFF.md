# Handoff: lessons, productivity track

Branch `claude/project-thread-gggjw8`, draft PR from it. Framework thread: cse_018gxMVUe63W8rvrNBwroP8s (PR #479 engine, #456 design).
Coordinator: session_01PgJyn3Yh6bBF3XNhYuLmpc.

## How the files are made
- `lesson.toml` is hand written. `project.json` and `finished.json` come from
  `cargo run -p novo-tests --example lesson_projects` (crates/novo-tests/examples). They are the Gate 1 motor
  starter with small edits, or a purpose built macro project. Rerun it when the project JSON format moves.
- Tested against the engine of PR #479 (checked out in a scratch worktree): all lessons parse, files resolve,
  glossary terms exist, canvas targets resolve. Remaining lint output is only "not in this build" warnings.

## What each lesson needs before it can be `ready`
| Lesson | Waiting for |
| --- | --- |
| design-checks | anchors `issue.pill`, `outline.issues`, `inspector.part`; waivers PR #413 (`check.waive`, `check.restore_waived`, anchors `issues.waive`, `issues.waived`); problem id `pin.unconnected` |
| parts-library | anchors `parts.search`, `parts.import`, `parts.confirm`, `parts.window`, `palette.field` (features are on main) |
| blocks-and-macros | macro browser PR #455; `macro.save` command; anchors `macro.*`; page anchors `outline.page.<slug>` |
| spreadsheets | PR #430 (`export.sheets`) and PR #433 (`import.spreadsheet`, anchors `import.*`) |
| revisions-and-compare | anchors `changes.list`, `compare.swap`, `palette.field` (features on main) |
| dxf-and-backgrounds | PR #385; commands `export.dxf`, `dxf.background_import`, `dxf.background_remove`; `menu.project.*` anchors |
| commissioning | PR #471 (`view.commissioning`); anchors `commissioning.*`, `workspace.commissioning` |
| time-savers | anchors `preferences.auto_connections`, `preferences.move_carries`, `shortcuts.sheet`, `project_settings.standard` (features on main) |

## Decisions
- `needs` only names lessons that exist; add `fast-keys` and `duplicate-and-numbering` when they land.
- `commissioning` sits in track `cabinet` (design section 10), written here at the coordinator's request.
- Ids of commands, anchors and problems are guesses for the unmerged parts; grep for them when the PRs land.
- Reading steps carry no `pause` (the engine warns that it does nothing).

## Phase 2 state (2026-10-16)

All eight lessons now have how/how_nl, Dutch text and the audit fixes (PRs #628, #634, #635). Known gaps:
- No part-assign UI on main: parts-library and design-checks have an honest "not yet" reading step. Add `assign_part` solve and a check when the picker exists.
- No solve (unverified, cannot reach `ready`): spreadsheet apply, DXF import/remove, parts import, macro place/export/import, commissioning steps after "open". Needs anchors or a file-dialog-free path.
- revisions-and-compare issues revision A itself because lesson projects carry no kept snapshot (app bug, audit item 3). If the app is fixed, the start project can carry A again (generator `revisions_start`).

## Still unscripted (2026-10-17)
Waive, restore, macro place and tags steps wait for the `list_row` op (click or right-click a list row). The inspector description edit in revisions-and-compare (`change-motor`) has no solve. Add them when the ops land.
