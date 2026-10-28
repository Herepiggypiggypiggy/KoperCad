# Heat hang: handoff

Branch `claude/heat-hang-empty-map` from main 78a1dab (one PR, see PROGRESS.md for the number).

Cause of the hang: Simulate on the 20 device cooling tower cell (800 x 1000 x 300 enclosure, 750 x 950 plate, no loss data) went into `reduce` (large-cabinet pitch search). At one pitch the edges of an axis alternate 10.15 / 6.22 / 7.8 mm; the old halving cascade in `enforce_ratio` split cells forever (3000+ edges from 100, still going), so the grid planner never returned, before any cancel check and before the first progress report. Now `enforce_ratio` gives the cascade a budget and then runs `graded_split`, a dynamic program over the part count of each cell (equal parts, ratio held, fewest cells, planes kept). The old cascade is untouched when it behaves, so existing meshes do not change.

Resume: `cargo test -p novo-cfd --test cooling_tower_setup`, `cargo test -p novo-app --lib heat`, `scripts/qa.sh --only 160` (scenarios 1600 to 1602). Pictures before and after: /mnt/project-files/qa/heat-hang/.

Traps: the QA harness needs libxkbcommon-x11-0, openbox, mesa-vulkan-drivers (apt). Disk fills at about 25 GB of target: `scripts/free-disk.sh`.
Cloud colours now come from the cloud's own scale; do not mix them with the shared scale in the legend.
