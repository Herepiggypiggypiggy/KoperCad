# Main reviewer B: roadmap

## Scope
I review new-crate and worker PRs, together with reviewer C. I own no code. Blocking criteria are in `handoff/reviewer/HANDOFF.md`.

## Done (2026-10-14)
Verdicts on #49, #50, #54, #55, #56, #59, #65, #68 and #70. See the table in HANDOFF.md.

## Next, in order
1. #65 (WS-28) at 3354d4e, #68 (WS-13) at 02d6e29 and #70 (fixer) at ec6f2ff are READY from me. They need reviewer C's verdict and a merge. If a head moves only by a main merge, run the mechanical check (HANDOFF.md). If code moved, re-review.
2. Review the follow-up PRs that come out of the non-blocking notes below, as they arrive.

## Non-blocking notes still owed (not done anywhere yet, as far as I know)
### novo-pdf (#49, WS-09)
- No `handoff/ws9` folder at review time; check it exists now.
- Link destination uses a hard-coded 40 mm margin (`lib.rs`); make it a named const.
- `font.rs` widths: Latin-1 0xA0–0xFF fall back to 556; accented right-aligned or centred labels drift slightly. Fix this with the Inter CID-subset TODO.
- The canvas measures labels with its own advance and the PDF with Helvetica AFM widths, so the crate doc's "exactly what is on screen" overstates it.
- `write_pdf` uses a plain `fs::write`; switch to write-to-temp-then-rename.
- Test `empty_page_prints_its_frame...` should assert the `?` substitution for non-WinAnsi text.
- `novo-canvas` with `default-features=false` still pulls in wgpu. This is also the #70 note below.
### novo-checks (#50, WS-14)
- `potential_conflicts` message reads "L are defined" when fewer than 2 names resolve.
- `contacts.rs` duplicates WS-10's classifier; merge them into one, from the symbol library's `function` field.
- The `stress_page_checks_in_reasonable_time` wall-clock bound is risky on slow Windows runners.
- `UnconnectedPin` flags legitimately open pins. Ask WS-02/WS-05 for an "optional connection point" flag.
### novo-cables (#54, WS-12)
- `choose_type` `break`s before trying 3G for L + PE, which gives a numbered 3G. Use `continue`, or start at `needed.max(3)`.
- `cable_screen` rebuilds the netlist per cable, and `ends::rank` scans every device. #68's `CableDiagram` also builds the screens twice (overview + screens).
- A `terminal:<id>` shield connection still counts as connected after that terminal is deleted.
- Cabinet-side tie-break differs: `propose` breaks ties by DeviceId, `screen::oriented` by location text.
- `is_numbered` treats an empty core name as numbered.
### novo-reports (#55, #68, WS-13)
- The fixed xlsx creation date shows as 2000-01-01 in Excel's file properties.
- #68: generated sheets `20.1`, `20.2` can collide with a real sub-page named `20.1`.
### novo-cli / novo-py (#56, #65, WS-28)
- `execute_one` returns `NothingCreated` after the command ran, so the edit stays in place. Undo first. Use `find_map` instead of `first()`.
- `novo export pdf --page X` with an unknown page exits 2; the help text describes 2 only as "bad command line".
- `--delimiter` accepts `"` and newlines, which break the CSV.
- xlsx write errors map to exit 2 (USAGE) instead of 4 (WRITE).
- The CLI produces the project-file refusal indirectly via `write_output(file, path, &[])`.
- The CLI links egui through novo-editor; #70 adds the `art` feature, so switch novo-cli to `features = ["art"]` once #70 merges.
### novo-editor `art` (#70)
- `art` still pulls in wgpu and naga, because novo-canvas's `wgpu` dependency isn't optional. A render feature in novo-canvas is WS-03's call.
