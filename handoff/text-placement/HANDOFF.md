# Text placement handoff

Branch: `claude/project-thread-img216` (kopercad). First PR: the plan, docs only.
State: plan in `docs/TEXT-PLACEMENT.md`; no code yet.
Resume: read the plan, section 1 (today's code: `novo-canvas/src/model.rs` sync_symbol/sync_wire, `note_place.rs`, `declutter.rs`),
then build stage 0 (scene dump overlap counts) and stage 1 (pure engine, reuse `declutter::Boxes` grid).
Traps: no stacked PRs; once READY do not push; novo-core API only via WS-02; use `theme::ui_palette()`.
