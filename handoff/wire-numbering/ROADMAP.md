# Wire numbering: roadmap

Scope: automatic and manual wire/potential numbering (schemes, stable fill, renumber with preview and
undo, locks, conflict detection) and feeding the numbers to reports, labels, cut lists and cable plans.
Design: docs/design/wire-numbering.md. Mandate: complete and special, replaces suites costing thousands.

Done (PR 1, branch claude/project-thread-lhuvv3): crate novo-numbering; macro placement uses it; palette
commands and the Wire numbering window in novo-app.

Next, in order:
1. Design checks: wrap `check()` as problems with locations (checks thread).
2. Inspector field for a wire's number with the lock toggle; right-click "Number wire...".
3. Per-page or per-structure schemes (power by potential, control by page).
4. Number labels on the drawing: show the number at the wire's best segment (canvas thread).
5. Cable plans: write cable core designation back as a scheme (ByCable already numbers by cable and core).
6. Excel round trip of wire numbers (interop sheets thread) and a "Wire numbers" report for audit.
7. Number on page move option; renumber preview filters (by page, by potential).
