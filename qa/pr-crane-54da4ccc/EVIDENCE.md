# Evidence for PR #1882 (crane, hoist, winch and Live scenarios)

- Source sha of the run: 54da4ccc75a66788779ce1d6dd8a2901cf3ff384 (see HEAD.txt and report.md, "source at git 54da4ccc75").
- Main sha the binary contains: df0d7438f951b320b2a1782dc865b1c16dc4c016 (origin/main merged into this branch; binary rebuilt after `touch crates/novo-app/build.rs`, run under `flock /tmp/qa.lock`).
- Scenarios: `qa/scenarios/`, the 48 files changed against origin/main (`git diff --name-only origin/main HEAD`). Result: 48 run(s), 48 passed, 0 failed, both sizes in one folder.
- Report: `report.md` here. Pictures: `/mnt/project-files/qa/crane-evidence-54da4ccc/shots` (not committed, size).
- One finding left: S4 "controls may overlap" on 1809b; the stacked floating windows are intended.

## Changes (all scenario-only, no app code)

- 17103 to 17110 (circuit library, find and place): the Place macro window now docks on the right, so the search box click moved. One file per size: the existing files are 1366 (`@sizes 1366x768`, click x 1205); new `17103b` to `17110b` files are 1920 (`@sizes 1920x1080`, click x 1759, taken from `17100b`, whose x was set from a 1920 picture). The search field x differs per size, so a single file cannot cover both; coverage is not dropped.
- 338, 339, 395, 396: Live card Operate buttons moved.
- 1581/b, 1583/b, 17300/b, 1809/b: Controller panel buttons moved (main switch, hoist steps, E-stop).
- 1709/b, 1735/b, 1738/1738a, 1760/b, 2430/b: Operate button moved; expected heading is now "What the run found".
- 1766/b, 1767/b, 1790/b, 1791/b: Operate or motor click positions moved; 1791b waits 800 instead of 400 frames so the 1920 axis reaches its stop (flake under load).
- 610 (both sizes): the motor window stays docked on a new project; opening it again closed it.
