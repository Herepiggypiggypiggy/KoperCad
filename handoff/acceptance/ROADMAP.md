# End-user acceptance: roadmap

Scope: drive the real window the way a first-time user does, read the exported files, file findings in
`/mnt/project-files/audit/acceptance/`, fix small defects with a QA scenario, hand larger ones to the owning thread.

Done
- Round 1 (2026-10-21, main d3a90d4): 17 findings, PRs #1197, #1203, #1207.
- Round 2 (2026-10-21, main d56346a): re-check of A-05, A-06, A-07, A-10 to A-12; PLC from scratch, I/O import, handover, bench packet, standards report, multi-page save/reopen/undo. Findings B-01 to B-12 in `round2-findings.md`; B-01 to B-06 fixed with scenarios 58 and 59.

Next
- Round 3: parts table import (when merged), Windows and a real GPU through Remote Control, Learn lessons end to end, Heat/3D duct pictures, 1920x1080 by hand.
- Follow B-07 to B-12 with their owners; retest when they land.
