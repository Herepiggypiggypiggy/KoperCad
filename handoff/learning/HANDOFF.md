# Learning modules: handoff

## State (2026-10-16)
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-7lpw0d` (restart from main after each merge; the only branch this thread may push to).
- Merged: design (#456), tracks and glossary (#474), engine `novo-learn` (#479), app shell (#532), early access listing (#581).
- Merged also: #601 (Show me how, `how`/`how_nl`, wire markers), #642 (replay gate: every `solve` through the real command path, `expect` must tick; 142 steps tick, 0 failures).
- Merged also: #682 (ops rename/tee/feed_row/cable/issue_revision/assign_part/held key, `manual`, menu and menu-row anchors, control-name ratchet).
- Open: runner PR 3 on `claude/project-thread-7lpw0d`: solve ops `ui_click`, `list_row` (rows registered with `learn_row`; right click + menu entry), `type_text`; anchors `issues.waive_reason`, `issues.waive`, `issues.restore`; design-checks waive/restore now replay; the outline flyout no longer closes when a row's context menu entry is chosen (it dropped the Waive form below the pinning width); live-view seal-in/stop wording for momentary pushbuttons; replay closes dialogs left open by an earlier lesson (Escape x4).
- Next, in order: (1) steps still `manual` because no script clicked panel buttons: convert them to `ui_click`; (2) anchors still missing: `outline.*`, `switcher.cabinet`, `canvas.page`, `settings.standard.*`, `revision.*`, `commissioning.*`, `mark.menu.export`, `inspector.cable.part`, `canvas.menu.*`, PLC and cable controls; (3) from the audit (/mnt/project-files/lessons-plan/audit.md): `problem_free` is always true (`learn_facts` never fills problems), highlights ring only the origin pin and pick the first symbol with a tag on any page, lesson projects have no revision snapshot so compare lessons have nothing to compare; (4) never-cover-target placement of the card/key caps with a test at 1366x768 and 1100x700 (learn-layout.md); (5) parked nits: `FONT_SMALL` at learn_ui.rs, limit the 33 ms repaint to frames with a target on screen; "Open finished version"; language setting.
- Deviations from the design: Continue works only while the lesson's own project is open; authoring folder is `KOPERCAD_LESSONS`; `zoom_at_least` is relative to the fit zoom of the page; language is English until a language setting exists.

## Resume
```
git fetch origin && git checkout claude/project-thread-7lpw0d
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings
cargo test -p novo-learn   # once the crate exists
```
Read first: the design doc, `docs/ONBOARDING-DESIGN.md` (walk-through, PR #428), `docs/design/discoverability.md` (PR #434), `crates/novo-core/src/json.rs` (project JSON form the sample projects use).

## People
Roster: /mnt/project-files/coordination/threads.md. Coordinate with Sample project and first-run walk-through (cse_019sSSXcKhY8k5uYwY3cgSyf), Onboarding and discoverability (cse_01CpKBFnbxA5RuJyh1pGkTdZ), Workspaces (cse_01WAQSEGkMbyreCPG18tsSPc), Design system thread. Message the reviewer with the head SHA after every push; no pushes after READY.

## Decisions
- Lessons are TOML, sample projects are the existing diffable project JSON; both embedded by `novo-app/build.rs`, no Rust edit to add a lesson.
- Pure engine in a new crate `novo-learn` (depends on `novo-core` only). Crate registration is the coordinator's job (CLAUDE.md).
- Checks are events (commands, tools, panels) plus document state; lessons are replayed headless in CI with each step's `solve`.
- Progress in `%APPDATA%\KoperCAD\learn.json`, not the settings blob.
