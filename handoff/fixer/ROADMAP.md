# Fixer roadmap

## Scope
Review fixes and small follow-up PRs for paused project threads, assigned by the coordinator. No own crate. Everything goes through the owner crate's conventions and reviewers.

## Done (2026-10-14)
- #38 WS-06: all editor proposals dashed; the Enter target is shown by opacity/weight only (b4de5e3).
- #57 tester: Gate 1 gaps for #30/#36 filled; X1:PE kind is ProtectiveEarth; F3 fixed (terminal dropped on the far half of a wire went in backwards).
- #46 WS-01: main merge, plus the dark mark colours for #47's CanvasStyle fields (1fb64af).
- #62 WS-06: `coil K1` places -K1's coil (verb-noun search; a second coil is refused with a notice).
- #63 WS-08: `accept all` is the accept-all command; the `no ` device list is in natural tag order.
- #64 WS-11: accept-all numbering in drawing order. It regressed horizontal wires; #71 fixes that.
- #67 WS-07: #36 follow-ups (part cells only via Order number, ignored edits reported, duplicate rows, properties.<key> normalisation) and terminal parts (assign_terminal_part, missing_parts with terminals, unused_parts keeps style parts).
- Open: #70 (art feature), #71 (#64 regression), #72 (one terminal_part).

## Next, in order
1. Drive #70, #71 and #72 to READY: answer reviewer findings. Owners after 23:20 UTC are WS-06 (#70), WS-11 (#71, #72 terminals part) and WS-13/WS-07 (#72 BOM/parts).
2. WS-11 + WS-02 decision: one store for terminal parts. If Terminal.part wins, migrate the novo-terminals style part (strip `terminal_part.<kind>` → per-terminal Terminal.part, or a core strip-level field) and drop the `style_parts` workaround in novo-parts/src/assign.rs.
3. WS-03: make wgpu optional in novo-canvas (`render` feature), so `novo-editor --no-default-features --features art` has no GPU stack (WS-28 asked).
4. Non-blocking notes not yet done. Both come from the #36 review of 2a43650/8cc6e0e, for WS-07 in crates/novo-parts:
   - **CSV import (csv.rs).**
     - An unterminated quote swallows the rest of the file silently.
     - Skipped-row numbers count parsed rows, not file lines.
     - `import_catalog` lets the last of two duplicate headers win silently; apply_table already refuses this.
   - **Bulk table and catalog.**
     - An unparseable Bool cell becomes Text (table.rs).
     - Excel turns tags like `-K1` into formulas: document it, or prefix `'` on export.
     - Catalog import replaces whole entries (merge per field).
     - `Catalog::load_default` merges the starter catalog into the user's file on save.
5. Possible WS-11 follow-up: suggest_terminals' cabinet-side rule (more devices wins) can pick the field side. Consider preferring a location with a `+CAB`-like name, or asking.
6. Possible WS-06 follow-up: `coil K7` creates -K7 even when K7 does not follow the numbering. The insert search reads `m3` as a symbol, never a device, so a device -M3 cannot be the parent through `no m3` (use `no -M3`).
