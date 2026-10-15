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
