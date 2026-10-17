# REL-LABELS roadmap

Scope: blocker B11 of `release/v0.1-plan.md`. Every number that is a model, an estimate or a placeholder says so where it is shown.

## Done (slice 1)
- Shared list and style: `novo-app/src/app/model_labels.rs` (`Model`, `badge`, `note`, `ROWS`), colours from `crate::theme::ui_palette()`.
- Help > "About the numbers" (`help.numbers`, Ctrl+K "About the numbers").
- Ampacity: Issues messages for thin wires and cable rating say "(planning value)".
- PFHd: Safety functions window badge and note, copy text carries the disclaimer, Issues message says "model, not exact".
- Motor start window: badge and note from the shared list.
- Door wires: length basis `door, no gap` (wire list, cut list, bench `~`, assembly count, Route all window).
- Fan heat balance: result note "Fan case ... about 3 %" with the balance, march note; 3D heat panel lists the run notes.
- Standards check text and PDF footer: "subset of the standard ... not a certification".
- Guard test: `a_file_that_shows_a_model_number_carries_its_label` in model_labels.rs.

## Next, in order
1. DXF wiring export (`novo-interop/src/wiring.rs`) still says `routed` for door wires (it has no access to the surface legs).
2. Calculations window and copy text: ampacity badge per row, not only the top note.
3. Ferrule/cut list PDF footers: one line "~ = estimate, door wires leave out the gap" (check bench footers).
4. Dutch strings for the new texts (DOC-2).
5. Extend the guard test with the reports' PDF footers.

## Open questions
None blocking.
