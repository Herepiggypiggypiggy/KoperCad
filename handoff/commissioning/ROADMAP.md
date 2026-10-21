# Commissioning thread: roadmap

Scope: commissioning mode (tick off wires, terminals, devices; who and when), progress per page and
cabinet, punch list, report as PDF and CSV, compact touch layout. Design: `docs/design/commissioning.md`.

Order (small draft PRs):
1. Design doc (this PR).
2. `novo-commissioning` crate: model, progress, punch list. Needs the coordinator to add the crate.
3. `novo-storage`: side tables, migration v3, fixture.
4. Report builder, CSV/PDF export, CLI command.
5. App: Commissioning mode, panel, canvas colouring.
6. App: punch list tab, compact touch layout.

Open: none blocking.

## Guided walk (2026-10-21)
Done: Next open walks pages in order and wires by number; the big card shows from, to, ferrules, length and wire or cable core; a tap of Space (or the big button) passes what is open and moves on; the wire is selected in the drawing; "3D" button selects the wire run in the 3D view; list groups Wires and Cable cores apart; tablet rows have a second line; punch list (open items only) to PDF.
Next: Fail from the card with a reason picker; walk order by cabinet/duct route instead of page; show the wire lit in the Commissioning workspace's own 3D side view.
