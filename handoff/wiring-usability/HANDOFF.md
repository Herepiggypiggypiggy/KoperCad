# Wiring and schematic editing usability: handoff

Branch `claude/project-thread-tda1xm` in `kopercad`. #1593 is merged; the follow-up PR from main adds the pointer drop (scenarios 797, 797b) and the workspace toast fix. Scenarios 790 to 795 in `qa/scenarios` cover 13, 29, 16, 22, 30/35.
Checks run: fmt, clippy -D warnings, tests for the workspace (lesson project files regenerated with `UPDATE_LESSONS=1 cargo test -p novo-tests --test lessons` and `-p novo-learn`).
Resume: `scripts/qa.sh --only <title words> --size 1366x768 --out /tmp/q`; the QA step `click X Y double` makes a real double click.
Traps: the dock is drawn after the canvas (Middle layer), so a click on it needs the `parked` rule in dock.rs; a lone fake corner direction must not trigger the detour (use `route_plain`).
Open items are listed in ROADMAP.md.
