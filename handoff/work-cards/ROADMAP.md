# Work cards (workshop pack from Build order): roadmap

Scope: Build order becomes the workshop pack for hand work: 1:1 drilling templates, tool list, per-step
wire cut list, printable tick-off sheet (creative audit round 34, idea 1). Design: `docs/design/work-cards.md`.

Done (PR 1, branch claude/project-thread-uevsfv):
- Engine `novo_layout::workpack` (cards, templates, tool list), 10 integration tests.
- PDF `novo_pdf::workpack`: cover with tools, tick-off sheet, two cards per A4 page, 100 mm ruler per card.
- App: Reports > Export > Export work cards, Bench output button, Build order strip button, file in the
  workshop folder and on the bench packet cover.
- QA scenarios 183-187.

Next, one PR each:
1. Per-step print from the 3D strip (the current step's card only) and a "Print this step" on the card list.
2. (Vents and fans done as CUT-OUT cards, PR 3; still open: wiring openings and device-less door cut-outs as cards.) Door and plate layout from the build order (idea 5): door cut-outs, vents and fans as cards with
   dimensioned templates, as steps in the order.
3. Strip length and ferrule per end on the wire cards (needs a wire-rule field per wire).
4. Template of a whole rail run with the devices on it (clip positions drawn on the strip).

Open questions for Tadjiro: which printer and paper (A4 assumed), whether wire cards should split by size
and colour instead of by circuit.

PR 2 (this branch after #1471 merged): per-step print from the strip ("Print this step", scenario 188), R5's two nits (contains match, per-cabinet N of M). Still next: door cut-outs and vents as cards (idea 5), strip length per end, clip positions drawn on the rail strip.

PR 3: Heat panel grilles and fans as CUT-OUT cards (scenario 189), the one-card file name carries the cabinet tag when there are several cabinets. Still next: wiring openings and device-less door cut-outs as cards, strip length per end, clip positions on the rail strip.

PR 4 (wiring openings): `opening.*` openings as CUT-OUT cards (add_wiring_opening_cards, scenario 196, fixture cabinet-wiring-openings), scale-hint label overlap fixed, cross hair arms per axis (a wide slot no longer throws a vertical line out of its frame). Next: strip length per end (PR B), clip positions on the rail strip (PR C).
PR 5 (wire ends): wire cards carry a 'Per end' column (allowance per end from the cut list, ferrule or lug by size). Still open: strip length separate from the ferrule collar, different allowances per end (the wire rules have one value), clip positions on the rail strip.
PR 6 (rail clips): the rail card draws the clipped devices on the strip (TemplateClip, boxes with the designation, note with the distances). Built and tested locally, pushed after #1485 merged. All roadmap items of the coordinator's list are then done; still open: separate strip length and ferrule collar, different allowances per end.

PR 10 (outside the plate): the cut-list wires with no end on the plate (supply feeds, field wires) get "Wires outside the plate" wire cards (header NOT A STEP) after the step cards (CardKind::Outside, steps 0, tick boxes Cut and Run, lengths marked estimated), named on the cover and the tick-off sheet. Next: wire numbers used twice and missing sizes.

PR 8 (duct corner): the duct step text names the outline's top left corner, the same numbers as the card. The full bench walk is in `audit/workpack/bench-walk.md`; next PRs: device card names the rail's start end, wires outside the plate as a tick-off card, wire numbers used twice and missing sizes.

PR 11 (wire parts): a wire number used on several rows shows "1 of 2" under the number on the wire cards (WireCut.part), and a wire with no size and no colour reads "size not set" instead of a blank cell. Bench walk findings 3 and 4. Findings 6 to 8 (duct fixing rule, label stock, 12 m motor wires in the example) wait for a decision.

PR 9 (rail start end): a clip card says which end of its rail the distance is measured from ("R1 starts at its left end", "top end" when upright). Next: wires outside the plate as a tick-off card, wire numbers used twice and missing sizes.
