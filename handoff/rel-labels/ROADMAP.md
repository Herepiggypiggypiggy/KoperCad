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

## Done (slice 2, #970)
- DXF wiring table: `door, no gap` for door wires.

## Done (slice 3)
- Calculations window: "(planning value)" badge on the Overload and Voltage drop rows and on the Iz line; the copy text carries the label on those rows.
- Dutch: length basis words (`routed`, `routed, no depth`, `free air`, `door, no gap`, `estimate`) in `nl.json` and `FIXED_CELL_TEXTS`, so Dutch wire lists, cut lists and the DXF-style tables read Dutch.

## Next, in order
1. Ferrule/cut list PDF footers: one line "~ = estimate, door wires leave out the gap" (check bench footers).
2. Dutch for the standards report footer and safety PDF footer (app side, not on the report translator yet).
3. Guard test: also check a number sits next to its label (today it only checks the label is somewhere in the file).
4. Comment on `is_estimate` (bench.rs) that any basis other than routed is an estimate on purpose.

## Open questions
None blocking.

## B11 findings from the visual test (main 39775cf)
- Example wires numbered: PR rel-labels-5-example-numbers (#1002).
- Counts: wire list, cut list, wire labels and wire production now carry their count and unit in the section title (`WIRE_LIST_COUNT_TITLE` etc.), Dutch patterns in nl.json. Wire list = connections between device pins (19 on the example); cut list = the same rows; wire labels = 2 per connection; wire production = those plus the supply and PE leads (25). Open: decide whether the cut list and labels should also include the supply and PE leads (a workshop cutting from the cut list misses them today); they are only named in the titles.
- Later (coordinator note): Dutch for the "no number" phrases of the terminal diagram, after #1000 merges.
- Supply and PE leads in the cut list and wire labels (coordinator, delegated by Tadjiro): done in branch rel-labels-7-feed-leads, after #1009. `WireList::build(project, true)` feeds cut list and labels; titles name the leads; "(feed)" has a Dutch cell pattern.
