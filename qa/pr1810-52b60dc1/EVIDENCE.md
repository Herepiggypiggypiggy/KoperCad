# PR 1810 evidence: three example projects

Built and run from git 52b60dc1db4f786e71399f7fbf4f4d2b69aa328b (HEAD.txt; the binary reports git 52b60dc1db). Folder: qa/pr1810-52b60dc1.

Where the user finds them: File > Templates and examples... > Examples (rows "Open roller door example", "Open conveyor line example", "Open filling line example"); also Ctrl+K and the Start centre.

Scenarios 2440 (roller door, small), 2441 (conveyor line, medium), 2442 (bottle filling line, large) at 1366x768 and 1920x1080. door/, conveyor/ and filling/ each hold report.md, summary.md, shots/ and logs/. Pictures per example: gallery row with its text, first schematic page, two more pages, cabinet, 3D and the live view (Shift+L).

Result: 6 runs, 0 failed. One S4 layout heuristic (conveyor, 1366x768, live view: 'Switch page' over 'Search' in the top bar when the page title is long); it is the top bar, not this PR's content.

Visible wording: gallery rows end "Illustration, not a safety design" (door, filling line) or "Illustration only" (conveyor); the project description says "Illustration only: sizes and ratings are planning values, not a verified design", plus "not a safety design" for the door and the filling line.

Code changes since the first head: roller door and crane hoist reversing fixed (K2 outputs were crossed on top of crossed inputs, so the swap cancelled); phase-order tests for both (example_new_tests.rs); start centre test and sentences.
