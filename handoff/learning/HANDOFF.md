# Learning modules: handoff

## State
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-7lpw0d` (restart from main after each merge; it is the only branch this thread may push to).
- Merged: design (#456), tracks and glossary (#474), walk-through #428 (not mine), `novo-learn` registered (#466). Open: engine PR #479 (crate `novo-learn`, head dbb3100, CI green, waiting on a main reviewer).
- Shell is done locally on branch `learn-shell` (commit e46ecf7 on top of the engine): `crates/novo-app/src/app/learn.rs` (state, catalog, events, facts, run, start/restart/continue, progress), `learn_ui.rs` (card, pointer ring, numbered notes, Learn window), `learn_tests.rs` (15 headless tests incl. anchors doc check), `build.rs` embeds `assets/lessons`, `ProjectBackend::new_project_from`, `Action::Learn` + `help.learn`/`learn.*` commands (novo-workflow, with Dutch words), `assets/lessons/ui.toml` + `novo_learn::UiStrings`, `docs/design/lesson-anchors.md` (generated; `UPDATE_ANCHORS_DOC=1 cargo test -p novo-app anchors_doc`).
- To ship it after #479 merges: `git fetch origin && git rebase --onto origin/main dbb3100 learn-shell && git push -f origin learn-shell:claude/project-thread-7lpw0d`, then open the PR (base main).
- Deviations from the design, to put in the shell PR: Continue works only while the lesson's own project is still open (a later session starts at step 1; progress still shows steps done); authoring folder is the env var `KOPERCAD_LESSONS` (no `--lessons` flag yet); `zoom_at_least` is relative to the fit zoom of the page; the language is English until a language setting exists (`LearnState.lang`).
- Next after the shell: replay test (`solve` ops through the real command path), more anchors (start panel buttons, outline, wheel, live card, inspector, parts, preferences, shortcuts sheet), the `fast-keys` conversion with the first-run thread, a problem-id registry for `problem_free`.
- Anchors requested by content threads: drawing (dock.* done, wheel, live.card, pages.list, menu.canvas.place_component, outline.toggle, palette.field); productivity (issue.pill, outline.issues, inspector.part, parts.*, changes.list, compare.swap, preferences.*, shortcuts.sheet, project_settings.standard).

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
