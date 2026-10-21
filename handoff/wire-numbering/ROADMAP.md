# Wire numbering workflow: roadmap
1. Numbers tab in the Wire numbering window: every net, click to type (fixed), Unlock, filter, order. Done (PR 1).
2. Revision-lock interplay: nets on locked pages are never changed by a run and are reported. Done (PR 1).
3. Numbers column in the wire panel for a multi-selection, drag-to-renumber a series, "swap two numbers". Open.
4. Per-page schemes (power pages by potential, control pages by page). Open, needs a model decision (design doc "Later").
5. Design checks `wire.number.*` wrapping `novo_numbering::check` (checks thread). Open.
