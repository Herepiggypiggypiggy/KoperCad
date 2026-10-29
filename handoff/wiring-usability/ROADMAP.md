# Wiring and schematic editing usability: roadmap

Scope: findings 5, 11, 12, 13, 14, 16, 22, 28, 29, 30, 31 and 35 of Tadjiro's laptop test report (cooling tower cell, experimental build 64b31c2f).

Done in the PR from `claude/project-thread-tda1xm`:
- 5 corner routing goes around both symbols instead of through contact graphics (geometry.rs `detour`, overlap.rs `plan_route`)
- 11 PLC network drive port is no longer a main function, so the help's rename -T2 to -T1 works
- 13 Esc or right click cancels a wire or cable, shown in the hint; the dock moves aside while a wire is parked
- 14 the whole interruption arrow picks its only pin
- 16 macros land in the visible part of a drawn page (`origin_in_view`) and are revealed when they cannot
- 22 the empty-page hint sits behind windows and is hidden while a tool is active
- 29 palette `go to page 1a` finds the page
- 30 and 35 a click on label text with Select no longer starts a wire; the Inspector card does not shift an empty view
- 31 a cable of several cores is labelled once

Not reproduced: 12 (symbol placed pin on pin stays not connected), 28 (Home right after placing a circuit; works headless).

Open:
- 11 the error toast follows the user across workspaces
- 16 "at the cursor" (uses the view corner, not the pointer) and the DOL circuit's 4 warnings and 7 infos
- 12 and 28 need a recording from the laptop
