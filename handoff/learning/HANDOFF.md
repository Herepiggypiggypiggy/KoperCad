# Learning modules: handoff

## State (2026-10-16)
- Repo herepiggypiggypiggy/kopercad, branch `claude/project-thread-7lpw0d` (restart from main after each merge; the only branch this thread may push to).
- Merged: design (#456), tracks and glossary (#474), engine `novo-learn` (#479), app shell (#532), early access listing (#581).
- Open: #601 (head in the PR): real **Show me how** (`novo_learn::demo`, `app/learn_demo.rs`): `demo = [...]` ops list per step or derived from `expect`/`solve`; numbered Start/End markers + dashed ghost wire for wire steps; Replay/Stop; Esc. `how`/`how_nl` string lists on Step (author's numbered instructions on the card). Targets `at.feed` and whole pins (`at = { pin = "-K1:A1" }`) so highlights follow where the person's parts land. The card no longer folds to a header in the live view (Shift+L). Dead-wire colour is #590, not mine.
- Content PRs (drafts): #588 drawing, #591 cabinet, terminals/PLC/docs thread, curriculum/menus #583 (adds `Status::Early`; its owner rebases onto #581).
- Next, in order: (1) **replay gate**: run every `solve` op through the real command path per step, `expect` must tick (there is none yet); new solve ops asked for: `cabinet_place`, `cabinet_rotate`, `cabinet = place_all|accept`, `cabinet_remove`, `live_pole { device, shift }`, `set_potential { feed_at, chip }`, `rename`, SelectOp with terminals (`select = "-X1:5"`), wheel (hold then digit); (2) lints: every `{key:}` resolves to a bound chord, every ui/clicked/opened id is a registered anchor, every expect command/signal exists, `{wheel:symbol-id}` markup from the live wheel and favourites, no bare digits/key names in step text, expect without `how`; (3) anchors (only ~10 registered): `wheel`, `dock.live`, `dock.xref`, `dock.wire_mode`, `live.card`, `pages.list`, `palette.field`, `parts.*`, `import.*`, `menu.project.*`, `shortcuts.sheet`, `project_settings.standard`, `preferences.*`, `changes.list`, `compare.swap`, `switcher.cabinet`, `cabinet.*`, prefix rules `menu.`, `workspace.`, `zone.`, `panel.`, `command.`; `{ui:drawer.x.tab}` renders as the bare word "tab"; (4) from the audit (/mnt/project-files/lessons-plan/audit.md): `problem_free` is always true (`learn_facts` never fills problems), highlights ring only the origin pin and pick the first symbol with a tag on any page, lesson projects have no revision snapshot so compare lessons have nothing to compare; (5) never-cover-target placement of the card/key caps with a test at 1366x768 and 1100x700 (learn-layout.md); (6) parked nits: `FONT_SMALL` at learn_ui.rs, limit the 33 ms repaint to frames with a target on screen; "Open finished version"; language setting.
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
