# Design reviewer C: handoff (2026-10-15 ~19:50 UTC)

Wound down at 80% usage by the owner's request. No code branch; this folder only.

## Verdicts posted (design READY on the head shown, unless noted)
| PR | Head | Verdict |
| --- | --- | --- |
| #428 fast-keys walk-through | 15e71248ede80d02f48edf123e6ad996e3f3d91a | READY (earlier 511c5af4: CHANGES NEEDED, fixed) |
| #434 discoverability design | 5745f839b986b6948d7b334630fc265a070f7d8f | READY |
| #386 Select where it goes | debf9d046a04d1a6f31740c181b5a57c0d97a412 | READY |
| #422 terminal diagram, drawn strip | aa645142ccb988efcbd337e3c70bae41cfdcc9f7 | READY |
| #413 design checks | c52d3aa5c0c648dc6909ebbf99d480f5dd1e59c7 | READY (wording); main already has "(#413)" commit, head since moved |
| #404 commissioning report | 9df74a3a22fcde37ae91d7756241ba84f9fde860 | READY |
| #443 Cabinet inspector | bae68d64c31c3ab8eca2b08963495c0682529be5 | READY (inspector files unchanged since 94ccbfcf) |
| #440 menu sections | 7d74ab47caf3b8588bfdd3b2686fffe1ff18bc67 | READY |
| #452 Reports hub | c9694d904cb85bad3223380dfc62dd257663d901 | READY (merge after #440; hub vs Reports workspace #475: one must rebase) |
| #419 panel data in inspector | f0af1a9ef55a2dd735c31b9659eaea4812597cec | READY (merge after #443) |
| #470 plate export | d4b0a1dba96260742da341ace80242f7f4a57245 | READY (nits: Cabinet-bar button, refusal wording) |
| #449 part geometry | 326d68b1758ca36fff5d08170b95faa85f93f40f | READY (engine) |
| #467 status bar | a489bcf229cf53e1c8d908de9ef0b5660bdc6263 | READY (status bar part; menu bar is B's) |
| #494 commissioning follow-up | 4e052e3718a7fe16c31772ada538fd3299de80a1 | READY |
| #497 DXF background panel | 12633f7dccb7169f7b54abd0724ac5707ff2ae48 | READY |
| #500 thermal visuals design | 7e2909d2990394c83ddff966ff199b19b8a2fe32 | READY |
| #515 workspaces docs | 1cfe1104dcc2620359832ea92d056be2adca3d0a | READY (4 doc nits) |
| #437, #431, #416 | | not reviewed (CLI / engine, no UI); #416 also adds PlugList, which #422 dropped |
| #446 | | storage sidecar, no UI, not reviewed |

## Recurring findings (use as a checklist)
- Docked side panels instead of the DESIGN §3 floating overlay inspector.
- Raw storage strings shown to users (`din_rail`); one-item submenus; repeated verbs ("Export > Export PDF").
- Hints that promise `?` (no shortcut sheet yet, see #434).
- Shared text buffers across several selected devices.
- One-touch bulk actions in safety flows need two steps.
- Space is the command field on the canvas; do not bind it elsewhere.
- Heads move by merging main: diff only the PR-owned files against the head I reviewed.

## How I worked
`git fetch origin +pull/N/head:refs/review/pN`, read `git diff origin/main...`, render SVG mockups with
`/opt/pw-browsers/chromium-1194/chrome-linux/chrome --headless --no-sandbox --screenshot`. No window runs (no build, ~30 GB disk).
