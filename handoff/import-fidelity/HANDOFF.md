# Import fidelity: handoff

Thread: Import fidelity (project thread cmsg_01TV2QyH4AYw9X2aTmgUsuhCKscLXXAN8M846wwvJ4C8GG). Lead: project lead thread.

State
- #1069 merged. Step 1 (the `guessed` report section) is in a PR from branch `import-fidelity-report`; see PROGRESS.md for the head.
- `Report` (crates/novo-interop/src/report.rs): moved / lossy / guessed / notes. `Report::guess(what, count, why)`, `is_exact()`. Summary line ends `Guessed: ...`.
- AML: `AmlPlan::report()` adds boxes (`aml::draw::is_box`), wire routes when the file has nets but no runs, new devices without any footprint.
- EPLAN: `pxf/plan.rs` adds boxes, loosely matched symbols, wire routes, per-cabinet footprints and rails/ducts.

Resume
- `cargo test -p novo-interop -p novo-cli`; clippy with `-D warnings`.
- The private parts file is in the project mount, `interop/private/crane/part-sizes.csv`; never commit it or real project designations (Reviewer A blocks on invented-names hygiene).

Traps
- Never push to a PR marked READY; follow-ups go in a new PR from main.
- Test strings must be invented (`=A1+CAB1-...`).

Unresolved after the full crane import (2026-10-21, report on the full crane export, kept privately)
- 13 "Black box" devices (tag letters B x4, U x7, ES x1, K x1): no pins and no function in the file, so no symbol can be chosen. They stay labelled boxes with 2 points. Done (#1123): the box is captioned by the placement's own type text, then part number (8 of the 13 now say what they are). Left: the other 5 have neither in the file. Not invented.
- 2 "Insulated wire termination" matched loosely (another variant of the library symbol): connection points may be off. Checked 2026-10-21: the file's own library has no variant with that pin count in any match tier (preferring pin count across tiers changed nothing), so the loose match is the best the file offers. Nothing to fix.
- 4-point motor protection breakers: stay boxes on purpose (two poles, no library symbol). Plan: a 2-pole symbol only when a real file needs it.
- Wire routes: the file stores no wire lines; its connections are implied by pin directions and nodes. #1139: 459 of 577 wires now follow those runs through the nodes; 118 keep a guessed right-angle route (the pair only connects through another pin or across pages). Which pins are one net is exact (agrees over 1115 of 1117 points). Plan: route the remaining 118 through the pins they pass.
- 24 cable cores without a cable: the core names a cable that is not in the file. Nothing to read; reported as is.
- 1 mounting plate device with no project device of that name in its location: reported; placed only when the name exists.
- 16 groups and 36 generated report pages: left out on purpose (KoperCAD generates its own lists).
- 18 objects of unknown type (kinds 17 and 91), now the whole of "other objects"; the 2520 report cells and 72 report texts are named separately.
- Net check leftovers: 57 points that face a direction and join nothing, 26 node legs that reach nothing, 10 interruption names used more than twice, 2 pairs KoperCAD joins that EPLAN keeps apart (same-name terminals).

Next PRs, in order
1. Kind 17 and 91 objects: identify, or keep named.
2. The remaining 118 guessed wire routes.

Round-trip check (2026-10-21)
- `novo_interop::compare::compare_projects(before, after)` and `kopercad-cli compare a.novo b.novo` say where two projects differ: devices, terminals, cables, parts, pages, drawn symbols (to 0.1 mm), nets by designation. Exit 1 when they differ.
- `crates/novo-interop/tests/compare.rs` pins the known PXF round-trip differences by name. Only one remains: without an EPLAN symbol table nothing is drawn (no symbols, no nets). Fixed (product call delegated by the owner): the EPLAN part number `MAKER.ORDERNO` is split into maker and order number on import and recombined on export (`C-100` comes back as `C-100`); a page keeps its structure (`=A1+CAB1`), its number and suffix as the name, and its title separately. Imported pages are now named `1`, not `=A1+CAB1/1 Control`.
- Next: AML must compare equal after #1148 (add that test), then texts, lines and wire runs in AML.

Sample round trips (2026-10-21, `crates/novo-interop/tests/roundtrips.rs`)
- Every sample (both synthetic PXF files, both crane demo versions) goes import, PXF export, import, `compare_projects`; only "drawn symbols" and "nets" may differ (no EPLAN symbol table).
- Found and fixed by it: the PXF writer dropped devices and terminals that are not drawn (every imported project has none); they are now written on the first page without symbol or position, like cables (EPLAN reading of that is unverified). A leading zero in a device number (`Q011`) came back as `Q11`. A line break in a page title became a space.
- The AML sample (`hoist-feeder.aml`) and every PXF sample also go through AML export and import and compare equal. That found two AML bugs, fixed: free symbols (an earth) were not written, and a net crossing pages got extra interruption points on re-import because the file's own were not linked to the net.
