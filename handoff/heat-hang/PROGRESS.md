2026-10-28 14:40 UTC  Found and fixed the meshing hang, cancel, empty heat map, slice solids, cloud scale, SimStatus; scenarios 1600 to 1602; PR from main.

2026-10-28 17:00 UTC  PR #1611 open, CI green on 21bab29. Lead B asked for evidence from the exact head plus lesson 209. The 209 walk showed the simulate-3d lesson still said the Vents and fans list is locked during a run: text (en and nl) and the 160 scenario comment fixed in a follow-up commit on the same branch. Evidence folder: /mnt/project-files/qa/heat-hang-final/ (HEAD.txt, report.md).

2026-10-28 18:30 UTC  PR #1611 merged (aebab73b). R1's two follow-ups in a new PR from main (branch claude/heat-hang-followups): a "Mesh note" in the snapshot notes when neighbouring cells exceed the grading bound on an axis (`grading_exceeded`, `worst_ratio`), and "Stopped: no result yet" when Cancel leaves no result (`stopped_notice`).
