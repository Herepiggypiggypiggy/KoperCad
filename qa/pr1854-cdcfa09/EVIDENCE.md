# Evidence for PR 1854 (head cdcfa09b3a22ae22d05d89b23e39c2182e944c8d)

Binary built from this head (report.md: "source at git cdcfa09b3a"). Scenarios 210 and 284 pass at 1366x768 and 1920x1080 (summary-*.md). Pictures: /mnt/project-files/qa/pr1854-cdcfa09/shots-210 and shots-284.

Where the user finds it: lesson "Test every wire" step "Open the Test tab" (click Test in the Commissioning rail), and lesson "Will the door close?" step "Open Inside and door" (click Plate setup in the Cabinet bar or rail).

What the pictures show: 210 step 3 ticks after the click and the card moves to 4 of 9 (04-sheet-step), and the later steps (record, miss, list, PDF) follow. 284 step 2 ticks and the card moves to 4 of 10 with the Inside and door form ringed.

Scenario change: 210 typed into the Test boxes by absolute pixel (48,475 / 48,523), which no longer hit the boxes in the docked panel; it now uses `click_text "" TextInput #1` (the second box is #1 again once the first holds text).

Not tested: other lessons that click a rail row (only the two ids that no scenario covered were checked; the code path is the same for every anchor id), Windows, real mouse hardware. Remaining non-blocking findings are layout-overlap heuristics ('7. Wire wire' over 'Write test sheet PDF' in the docked panel).
