# WS-24 Import and export: handoff

## State (2026-10-15 17:45 UTC)
- #344 (design + DXF engine) and #347 (EPLAN parts import) are merged.
- #354 (`ws-24-bridge`, page to DXF) is open with the main reviewer. Branch
  `claude/project-thread-dqgsq7` is spent; restart branches from main.
- Nothing half-done. Next is ROADMAP item 1 (entry points).

## What exists
- `docs/design/interop.md`: the design (§3 export, §4 import, §5 EPLAN).
- `crates/novo-interop/src/dxf/`: `model.rs` (neutral Drawing, DXF axes: y up, degrees
  CCW), `write.rs` (`write_dxf`, AC1009 ASCII, 7-bit, deterministic), `read.rs`
  (`read_dxf`, 2M entity cap, INSERT array warning). `Drawing::scaled_to_mm` applies
  `$INSUNITS`.
- `crates/novo-interop/src/eplan/`: `xml.rs` (small XML reader, no external refs,
  depth 128), `parts.rs` (`import_xml`, `import_delimited`, `import_auto`; unmapped
  properties kept under `eplan.*`; `MAKER.order` part-number fallback). Property ids are
  from memory of EPLAN's public names: real exports that fail become fixtures.
- `crates/novo-interop/src/bridge.rs`: `page_drawing(project, page, art, style)`,
  `scene_to_drawing(scene, page_height_mm)`; y flip, arcs swap, one TEXT per label line.
- Tests: `tests/dxf.rs`, `dxf_limits.rs`, `eplan.rs`, `bridge.rs`.

## Resume
```
cargo test -p novo-interop
cargo clippy -p novo-interop --all-targets -- -D warnings
```
Next work: entry points (`kopercad-cli export dxf`, Python, app menu) in new PRs from main;
the CLI/app threads own their crates, so keep those PRs thin and message them.

## Traps and decisions
- R12 dialect on purpose: no handles, widest support. Non-ASCII text is `\U+XXXX`.
- Binary DXF is refused. Splines, hatches, dimensions are skipped and counted
  (`DxfRead::skipped`).
- Imported DXF as page background needs a WS-02 underlay record; not started.
- `ModelStyle` colours do not reach the DXF (layers carry colour by name).
- Reviewer: main reviewer cse_016C9nTT8cN7voW8t1NQUG4W; lead cse_01FiLvRBKEcNNHbQEGhYyL1S.
