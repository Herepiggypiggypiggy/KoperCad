# Drill and cut-out templates: roadmap

Scope: 1:1 printable drill and door cut-out templates for a workshop that drills and cuts by hand.

Already there before this lane: tiled 1:1 drilling template per surface (cross hairs, hole sizes, 100 mm ruler, overlap marks), one-sheet plate PDF, Workshop pack.

Done (this PR)
- `novo-layout/src/cutout_cards.rs`: cut-out cards (one per shape per surface, distances from all four edges) and `scale_advice` (what to do with a measured check square).
- `novo-pdf/src/template_cards.rs`: print-scale check page (100 mm square with diagonals, 250 mm ruler, measured-to-percent table) and the cut-out cards (outline at true size, centre lines, punch marks, 50 mm square and 100 mm ruler per card, positions continue on more pages).
- App: Reports > Bench output > "Print scale check (PDF)" with two boxes for the measured square and the advice live; command `export.scale_check` (Ctrl+K, Reports > Export); Workshop pack writes `0 Print scale check.pdf` and `1 Cut-out cards <enclosure>.pdf`.
- QA scenario `qa/scenarios/39-print-scale-check.qa`.

Next
1. Print on a real printer and measure (never done here): the 0.5 mm figure is an estimate.
2. Rounded-corner rectangles and slots when the part data carries a corner radius (CutoutShape has only rect and round).
3. Cut-outs larger than 150 mm are left to the tiled template; a tiled card could cover them.
4. Check the door viewing side (outside or inside) against the drill table convention, then name it on the cards.
5. Option to print cards on A3 or at the card's own size for a print shop.
