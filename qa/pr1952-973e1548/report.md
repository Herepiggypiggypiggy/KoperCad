# PR 1952 QA, source at git 973e1548514d9778244c6f7971bf36569cf8d11b

Passed with no findings: 9600 at 1366x768 (1366/) and 1920x1080 (1920/), help text 9603 (1366-help/) and 9604 (1920-help/), winch run with lamps 9605 at 1366x768 (1366-winch/, three frames while the hook rises).

9606 (winch run, 1920x1080) fails with S1 'app gone: Resource temporarily unavailable (os error 11)' at the mouse_down step, twice. The existing scenario 1767b (same steps) fails the same way on clean main (origin/main): main-baseline-1767b-1920/. So it is not caused by this PR; the 1920 winch-run crash is a separate existing problem (Linux headless). Lit lamps at 1920 are covered by the headless renders truck-lamps.png (green, red) and truck-lamps-amber-off.png in /mnt/project-files/qa/pr1952-lamps/.

Pictures: /mnt/project-files/qa/pr1952-973e1548. This commit adds only these evidence files and the scenario/code changes listed in the PR.
