# WS-24 Import and export: handoff

## State (2026-10-15 17:50 UTC)
- Branch `claude/project-thread-dqgsq7`, first PR: design + `novo-interop` DXF engine.
  Head SHA and review state: see the PR and `PROGRESS.md`.
- Nothing half-done. The `dxf` engine is complete for its scope (design §3, §4).

## What exists
- `docs/design/interop.md`: the design, including the EPLAN parts plan (§5).
- `crates/novo-interop/src/dxf/`: `model.rs` (Drawing, Entity, Layer), `write.rs`
  (`write_dxf`, AC1009 ASCII, 7-bit, deterministic), `read.rs` (`read_dxf`).
- DXF-native axes: y up, degrees CCW. The y flip to KoperCAD's page belongs to the
  bridge (not written yet). `Drawing::scaled_to_mm` applies `$INSUNITS`.

## Resume
```
cargo test -p novo-interop
cargo clippy -p novo-interop --all-targets -- -D warnings
```
Next work: `ROADMAP.md` item 1 (EPLAN). New tests go in new files under `tests/`.

## Traps and decisions
- R12 dialect on purpose: no handles, widest support. Non-ASCII text is `\U+XXXX`.
- Binary DXF is refused, not misread. Splines, hatches, dimensions are skipped and
  counted (`DxfRead::skipped`).
- Only WS-02 changes `novo-core`; this crate does not depend on it yet.
- Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W (roster in
  `/mnt/project-files/coordination/threads.md`).
