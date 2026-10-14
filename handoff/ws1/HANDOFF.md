# WS-01 App shell: handoff (updated 2026-10-14 17:11 UTC)

## State
- Repo: github.com/Herepiggypiggypiggy/NovoElectric. Push only to branch `claude/app-shell-x8aamm`.
- **PR #34** "WS-01: wire storage and canvas into the app" is open. Head `baf6e72`, base main (it already includes main up to #25). CI was queued at last check. The reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) was asked to review at baf6e72; no verdict yet. WS-04 has read backend.rs and confirmed its fixes.
- Rules (coordination/threads.md): once the reviewer says READY, never push to that PR again; new work goes in a new PR from main. After every push, message the reviewer with the head SHA. Work only against merged main and never stack on open PRs. After a PR merges, restart the same branch from main: `git fetch origin main && git checkout -B claude/app-shell-x8aamm origin/main`, then force-with-lease push.

## Code map (crates/novo-app/src)
- `app.rs`: `NovoApp`, which owns the menus, panels, status bar, dialogs (unsaved, recovery, about) and action dispatch.
  - Frame order in `show()`: start-up (open the file from args, or `restore_untitled`), fix the page, `tick` autosave, close request, title, shortcuts, panels, canvas, palette, dialogs, picked actions.
- `backend.rs`: the `ProjectBackend` trait, `StorageBackend` (novo_storage::Session plus an untitled dir) and `MemoryBackend` for tests.
  - Every ChangeSet must reach `record()`.
- `canvas.rs`: the `CanvasView` trait, `OpenPage` (wraps the backend; `transact` and `execute` record automatically) and `PlaceholderCanvas`.
- `model_canvas.rs`: `ModelCanvas`, with one novo-canvas `PageScene` and widget view per page.
- `actions.rs`: `Action` enum, ids, shortcuts and `RESERVED_SHORTCUTS`. `palette.rs`: Ctrl+K. `settings.rs`: persisted through eframe storage.
- `lib.rs`: `run()`, which installs the canvas from `cc.wgpu_render_state`. Untitled projects live in `data_dir()/untitled` (%LOCALAPPDATA%\NovoElectric\untitled on Windows).

## Local-only work (not on GitHub; patches saved next to this file)
- `theme-wiring.patch`, built on #34 with #20 merged in:
  - `novo_theme::apply` in `NovoApp::new`; the theme actions and the `Settings.theme` field are removed.
  - Icon from `novo_theme::brand::ICON_PNG_256`; `build.rs`, the .iss and the workflow paths point at `assets/branding/icon.ico`; `assets/icon/` is deleted.
  - `src/theme.rs` maps CANVAS_DARK onto `CanvasStyle` and `ModelStyle`; `section_label` and `primary_button` are used.
  - Tests call `novo_theme::apply` on their context, because the theme fonts are needed.
  - Adds `ron` as a dev-dep for an old-settings test.
  - Apply it after #20 and #34 merge: `git am theme-wiring.patch` (expect small conflicts).
- `workbench.patch` (branch workbench-wip, 29ba4e2 + 8c69f1b + 118d44d, on top of #34): the docs/DESIGN.md §3 Workbench, plus start-up reopening the last project, a start panel (New, Open, Recent), `NovoApp::execute(Command) -> Option<Result<Outcome>>` / `document()` / `current_page()` for the tester, and Ctrl+3/4/5 reserved for the drawer.
  - 42 px top bar: mark menu (New, Open, Recent, Save, Save as, Close, About, Quit), Schematic | Cabinet (Cabinet disabled), breadcrumb project / page (click opens the flyout), "Search or run" (Ctrl+K), saved state.
  - Outline: pinned `Panel::left` 248 px when `settings.outline_pinned.unwrap_or(width >= 1700)`, else a flyout `Area` from a "Pages" button at the canvas top left. Pages mode only (Issues/Changes disabled), filter, RECENT, ALL PAGES.
  - HUD (canvas status text, top right) and toasts (bottom left; info fades after 5 s, warnings/errors stay until clicked).
  - Actions: ShowSchematic (Ctrl+1), ShowCabinet (Ctrl+2, disabled), PinOutline, FocusMode (Ctrl+Shift+F), FullScreen (F11). Panel toggles and `show_*_panel` settings removed.
  - Design criteria (workflow thread session_01JHYmmTDsVHDL7pEab8MGwy, ping it with the head to check): still missing Open example, issue pill, new projects saved to Documents\NovoElectric\<name>.novo via the known-folder lookup (#43), drawer keeps the canvas-centre world point fixed.
  - No save prompts (lead decision): `StorageBackend::tick` saves a saved project into its own file when dirty (app ticks 0.5 s after a change), `close` writes first; untitled closed with work is renamed `kept-*.novo`, listed by `unsaved_projects()`, opened as untitled by `open()`. `settings.last_open` = `backend.file()` each frame, reopened on start. Revisions/exports are the checkpoint (DESIGN §8), not Save.
  - Not done yet: issue pill, tool dock, inspector, drawer, naming an untitled project into Documents\NovoElectric (#43).
  - After #34 merges: restart the branch from main, `git am workbench.patch`, open a new PR, ask the design reviewer (cse_0114emQ5fSFrzcALbR4S18TU). Tell WS-08 about the actions.rs changes.
- `shelved-panels-search.patch`: named object panels, tag search in Ctrl+K, filterable properties, Shift+G for "go to all references", and `browse.rs` queries. Shelved until DESIGN.md lands. Mine `browse.rs` (rows, places, fields, owner) for later.

## Gate 1 plan (the next big task, against merged main only)
1. When #38 (novo-editor) merges, write an adapter `EditorCanvas: CanvasView` around `novo_editor::view::SchematicView`.
   - As of #38 head 81c0caa, `show(ui, &mut Document, PageId)` returns one merged `ChangeSet` per frame, empty when nothing changed. Pass it to `backend.record` and to the other views' `document_changed`, but not to SchematicView itself.
   - Call `take_page_request()` each frame and switch to that page.
   - Install with `novo_canvas::widget::install(rs, 1)`.
2. #29 (WS-08) wires the palette and keymap in novo-app themselves. Avoid touching palette.rs and actions.rs; tell WS-08 if that changes.
3. #30 terminals and #31 xref: wire them as their authors describe (messages in the WS-10 note: G, Shift+G, Alt+Left/Right, `XrefIndex::find_tag`).
4. Coordinate the scenario with the tester (cse_01LMMcDMt9qaHXoSXy7iDu58).

## Resume
```
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
cargo run -p novo-app            # GUI; headless: Xvfb :99 + mesa lavapipe, xdotool clicks work, typed keys don't
```

## Traps
- egui shortcut matching is loose. Bare and shifted keys use `consume_exact`. Try more specific shortcuts first.
- Call `output.textures_delta.clear()` after `ctx.run_ui` in tests.
- `StorageBackend` implements Drop, so struct-update syntax doesn't work on it.
- Windows CI is slow (about 12 min). The installer job runs as a freshly created standard user.

## People
Coordinator session_018btnHEHYdedX4jueCivWjT, integration lead session_011PfTJ1QaP1JQJPwXcFnxBE, WS-04 storage cse_01QgejHGqkNNMoeosEZQ62wb, WS-06 editor cse_019siuxbyyRNZRgBBww1KocJ, WS-08 cse_01TAFG6LPGy2WMwhJYXmxqxF, layout/theme cse_01UXbCeQW22y6tv74F2Xvbvm.
