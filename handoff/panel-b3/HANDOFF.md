# PANEL-B3 handoff

Branch claude/project-thread-mle5ww (repo kopercad), PRs from main, one per slice.
Resume: `CARGO_PROFILE_DEV_DEBUG=0 cargo test -p novo-app cabinet_surfaces && cargo clippy --workspace --all-targets -- -D warnings`.

State: slice 1 (picker) in PR, see PROGRESS.md for the head SHA.
- `cabinet_surfaces::summarize(project, structure)` reads the enclosure, `surface.<name>` structure properties and each placement's `surface` property.
- Picking a non-plate surface only shows a note; the plate drawing stays until slice 2.
- Anchor `cabinet.surface` is registered in learn.rs; command is handled in workbench.rs.
Reviewer R3 (see coordination/threads.md). PANEL-B2 is a separate thread.
