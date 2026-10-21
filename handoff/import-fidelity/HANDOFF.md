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

Unresolved after the full crane import (2026-10-21, report on `scherpel-v95-full.epj`)
- 13 "Black box" devices (tag letters B x4, U x7, ES x1, K x1): no pins and no function in the file, so no symbol can be chosen. They stay labelled boxes with 2 points. Plan: give the box the size of the EPLAN drawing (exact from the sheet) and a name from the tag letters; never invent a function.
- 2 "Insulated wire termination" matched loosely (another variant of the library symbol): connection points may be off. Plan: compare the pin positions against the embedded library variant and pick the variant with the same pin count.
- 4-point motor protection breakers: stay boxes on purpose (two poles, no library symbol). Plan: a 2-pole symbol only when a real file needs it.
- 577 wire routes are plain right-angle runs, not EPLAN's lines. Which pins are one net is exact (agrees over 1115 of 1117 points). Plan: carry EPLAN's connection-line geometry when the file has it (waits on the text-anchor and line questions below).
- 24 cable cores without a cable: the core names a cable that is not in the file. Nothing to read; reported as is.
- 1 mounting plate device with no project device of that name in its location: reported; placed only when the name exists.
- 16 groups and 36 generated report pages: left out on purpose (KoperCAD generates its own lists).
- 18 objects of unknown type (kinds 17 and 91), now the whole of "other objects"; the 2520 report cells and 72 report texts are named separately.
- Net check leftovers: 57 points that face a direction and join nothing, 26 node legs that reach nothing, 10 interruption names used more than twice, 2 pairs KoperCAD joins that EPLAN keeps apart (same-name terminals).

Next PRs, in order
1. Black box size and name from the sheet.
2. Loosely matched symbols: choose the variant by pin count and positions.
3. Kind 17 and 91 objects: identify, or keep named.
4. EPLAN connection-line geometry for wire routes.
