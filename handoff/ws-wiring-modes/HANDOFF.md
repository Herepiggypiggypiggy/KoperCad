# Two wiring modes: handoff
Thread session: session_011TpANb8LDXUp93s9E33xqv. Branch claude/project-thread-s1dfbt. Repo copy of this folder: handoff/ws-wiring-modes/.
State: DESIGN §4 (#169) and plan (#168) merged. Engine PR 1 = #181 (overlap.rs, plan_route in draw_wire), awaiting review. Keymap Shift+W/wire.mode is WS-08 #180. Next: settle_wire for accept/insert_into_wires/transform/tidy_junctions; WiringMode + Lines (WireTool); wire.overlap check; app shell chip.
Resume: git fetch origin && git checkout claude/project-thread-s1dfbt; then fmt, clippy -D warnings, test --workspace (Actions is billing-blocked).
Contacts: editor cse_019siuxbyyRNZRgBBww1KocJ and keyboard cse_01TAFG6LPGy2WMwhJYXmxqxF (own the code; coordinate before editing), Workflow cse_01JHYmmTDsVHDL7pEab8MGwy, Layout cse_01UXbCeQW22y6tv74F2Xvbvm, tester cse_01LMMcDMt9qaHXoSXy7iDu58, main reviewer cse_016C9nTT8cN7voW8t1NQUG4W, design reviewer cse_0114emQ5fSFrzcALbR4S18TU, integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S (merges).
Test list: docs/design/wiring-modes.md, last section.
Decisions: overlap rule in editor engine plus a novo-checks problem; nudge rather than auto-join; mode is a per-user setting.
Traps: do not reopen "no corner/T pieces" (corners are route bend points). After READY do not push; message reviewer after each push with SHA.

## Engine plan notes from WS-06 (2026-10-15)
One `edit::settle_wire(tx,page,wire)` called from draw_wire, accept (facing proposals), insert_into_wires, transform, tidy_junctions (also check `rg "\.connect\(" crates`). Move ghosts run the same step. Stub at pins: leave along endpoint_direction one grid step, then shift. Deterministic nudge. Same-pin overlap: second wire starts on a derived junction (test potentials/xrefs unchanged). Backspace needs EditorKey::Backspace (keyboard thread); Enter finishes only when no proposal highlighted. overlaps() treats an end touching a wire's interior collinearly as overlap. Tests in crates/novo-editor/tests/editing.rs; WS-06 reviews.

## Decisions after main review of #168 (2026-10-15)
1. Command path: core never hard-rejects (load/undo must not fail). Enforcement is the shared `edit::settle_wire` used by every editor path; scripts/AI edits go through the editor edit API; anything that still gets in (raw Command::AddWire/SetRoute, imports) is caught by novo-checks `wire.overlap`.
2. Move with no room for a nudge: the move is accepted, overlap left for `wire.overlap` (a move is never refused). Drawing a new wire with no room is refused with the reason.
3. Diagonals deferred: v1 Lines mode is axis-locked only (no Shift 45 degrees); overlaps handles axis-aligned runs.
4. Also: routes may not pass through another net's pin or junction; same-pin overlap is one undo step; use a spatial index (per-page segment grid) and add a benchmark.

## Clarifications from DESIGN follow-up #177
Moving a symbol stretches a hand-drawn wire by moving only its end segment; if the pin leaves that segment's axis, add a bend (never diagonal). R flips a Corners elbow only; in Lines R does nothing. Corners elbow runs along the axis of greater pointer movement. Pin drag over 4 px is always auto-routed. I own geometry.rs, edit.rs, tool.rs during engine PRs (WS-06 agreed). Reviewer asks: tests for settle_wire from tidy_junctions and accept; multi-selection interactive move refused atomically; diagonal pin move on hand-drawn wire.
