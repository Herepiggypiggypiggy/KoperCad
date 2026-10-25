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
2. Door and plate layout from the build order (idea 5): door cut-outs, vents and fans as cards with
   dimensioned templates, as steps in the order.
3. Strip length and ferrule per end on the wire cards (needs a wire-rule field per wire).
4. Template of a whole rail run with the devices on it (clip positions drawn on the strip).

Open questions for Tadjiro: which printer and paper (A4 assumed), whether wire cards should split by size
and colour instead of by circuit.
