# PR 1810 evidence: three example projects

Source built and run at git a61c8a9c96b3d1aa0be725c9331b2aad60191d10 (full sha in HEAD.txt; the binary reports git a61c8a9c96).

Where the user finds them: File > Templates and examples... > Examples (rows "Open roller door example", "Open conveyor line example", "Open filling line example"), also Ctrl+K and the Start centre.

Scenarios 2440 (roller door, small), 2441 (conveyor line, medium), 2442 (bottle filling line, large), each at 1366x768 and 1920x1080: door/, conveyor/ and filling/ hold report.md and shots/.
Pictures per example: gallery row with its text ("Illustration only: planning values, not a safety design" for door and filling line), first schematic page, two more pages, cabinet, 3D and the live view (Shift+L).

Result: 6 runs, 0 failed. One S4 heuristic finding (conveyor, 1366x768: 'Switch page' over 'Search' in the top bar with a long page title, live view), not caused by this PR.
