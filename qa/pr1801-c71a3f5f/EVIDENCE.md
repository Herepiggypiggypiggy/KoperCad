# QA evidence for PR 1801

Code head c71a3f5f9706a6cdeaaaac17104a6671a387d0c6 (merge of origin/main 0fffafb plus a QA-only fix to 5632).
Run: `scripts/qa.sh --only ontroller`, 17 runs at 1366x768 and 1920x1080, program built from that source.

Pass: 1580, 1582, 5620, 5630 (both sizes), 5631, 5632 and the other controller scenarios not listed below.
5630 has two S4 "may overlap" notes: the Add-ons dialog and the Radio panel stack on purpose.

Fail (6 runs):
- 1583 and 1583b (`brake open` not on screen) and 1584 and 1584b (add menu entry not found): these fail on main
  too (known, a separate fix thread).
- 1581 and 1581b (`Signals (N of 15 high)` not on screen after a click at fixed window points): the same kind of
  drift, main's card got taller rows (instance tabs, Pop out, Duplicate) so the hard-coded click points miss. This
  PR does not touch those rows; I did not run main to confirm 1581 is red there too.
