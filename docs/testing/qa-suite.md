# Test and screenshot suite

One command drives the real KoperCAD window, walks named scenarios, saves
labelled screenshots at fixed window sizes, runs cheap layout checks and writes
a machine-readable result plus an audit report.

```
scripts/qa.sh                                  # quick scenarios, 1366x768 and 1920x1080
scripts/qa.sh --only workspaces --size 1366x768
scripts/qa.sh --slow --out /mnt/project-files/qa/2026-10-21-sweep
scripts/qa.sh list
```

Same thing without the wrapper: `kopercad-cli qa run [FILES|FOLDERS] [--size WxH]...
[--out DIR] [--only TEXT] [--slow] [--app PATH]`. Exit code 0 when every run passed,
1 when a step failed or a finding of severity S1 to S3 was raised.

It builds on the visual test user (`docs/testing/visual-test-user.md`): the same
`kopercad --control` app, the same empty first-run profile for every scenario, the
same virtual screen on Linux (Xvfb, openbox, software Vulkan). Needs on Linux:
`apt install xvfb openbox mesa-vulkan-drivers libxkbcommon-x11-0`.

## What a run writes

```
<out>/report.md        audit report: scenario table, findings with steps, expected, saw, picture
<out>/results.json     the same as data: runs, steps with status and time, findings, sweeps
<out>/shots/           <scenario>-<size>-<nn>-<label>.png
<out>/logs/            app log per run, widget-tree dumps from `tree` steps
```

Default `--out` is `target/qa/<time>`. For a report other threads read, use
`/mnt/project-files/qa/<name>`.

## Scenario files

`qa/scenarios/NN-name.qa`, plain text, one step per line, `#` starts a comment,
double quotes group words. Header lines: `@title`, `@area` (Schematic, Cabinet,
3D, Heat, Commissioning, Reports, Learn, Parts, Fixtures...), `@sizes 1366x768`
(override the run's sizes), `@slow` (only with `--slow`).

```
@title Every workspace of the example
@area Workspaces
click_text "Open example"
wait 20
click_text Cabinet #1
wait 20
shot cabinet
check_blank cabinet
check_layout
```

Every scenario starts a fresh app with an empty profile at the run's window size.

| Step | What it does |
| --- | --- |
| `open FILE` | restart the app with this project (fresh profile); paths are relative to the repository root |
| `fresh` | restart with no project |
| `click_text "Label" [Role] [#n]` | click the enabled widget with this label (exact match first); `#2` = second match (a label can be a menu and a workspace button) |
| `click X Y`, `move X Y`, `drag X1 Y1 X2 Y2`, `scroll X Y DY` | pointer, window points |
| `key ctrl+k Escape`, `text some words` | keyboard |
| `answer_dialog FILE` | the next native file dialog gets this path (queue it first) |
| `wait N`, `resize W H` | run N frames; resize the window |
| `shot label` | labelled screenshot |
| `expect "text"`, `expect_not "text"` | a label or value containing the text is (not) in the widget tree; a miss is a finding (S3) |
| `expect_enabled "Label"`, `expect_disabled "Label"` | the control with this label (icon glyphs ignored) is enabled / disabled; a miss is a finding |
| `expect_scene "text"` | the project data dump (`scene`) contains the text: checks the canvas without reading pixels |
| `check_layout` | cheap layout checks, see below |
| `check_blank [label]` | takes a screenshot; finding S2 when the window is one flat colour |
| `tree [filter]` | write the labelled widgets to `logs/` (for authors: find the right label) |
| `sweep_commands [Group]` | every command of the command field, by its own name (slow) |
| `sweep_lessons` | every shipped lesson, opened from the command field (slow) |
| `try STEP` | run a step; its failure does not end the scenario |
| `note ...` | a comment that shows in the step list |

A step that cannot run (widget not found, app gone) ends the scenario: that is a
finding with the steps so far and a picture; a dead app is S1.

## Checks

Always on: the app's own log is scanned for `panicked at`, `fatal runtime error`,
GPU validation errors (S1 for a panic, S3 otherwise); a window that is not the
requested size is a finding (shots at that size would not be what was asked).

`check_layout` reads the widget tree (egui AccessKit) and flags: controls cut off
by the window edge; a toolbar that needs horizontal scrolling; two small controls
overlapping; controls under 8 points; icon-only buttons with no label (S4).
Honest limits: it is a heuristic from rectangles. Content inside a scroll area is
not reported as clipped, popup menus over the page are ignored, wide list rows are
not compared. The painted canvas and 3D view are not in the tree. A clean check
does not mean a clean screen: look at the pictures.

`check_blank` catches an empty or black workspace by pixel statistics only.

## Audit report format

Every finding has: id (`F-001`), severity, area, window size, title, the steps that
led there, expected, saw, and a screenshot. Severity: S1 data loss, crash or cannot
start; S2 a core workflow cannot be finished, a wrong number is shown as fact, or
the window is blank; S3 everything else wrong or confusing; S4 polish and
accessibility. `kopercad-cli qa report results.json` rewrites `report.md`.

## Interactive audits (Claude threads)

The MCP server (`.mcp.json`, `kopercad-cli test-user`) has three more tools for a
thread that drives the window by hand: `check_layout` (same checks on the current
screen), `finding` (severity, title, area, steps, expected, saw; the newest
screenshot is attached) and `report` (writes `report.md` and `results.json` with
the pictures copied beside them, same format as `qa run`).

How other threads use it:

1. Before changing UI: `scripts/qa.sh --only <area> --out /mnt/project-files/qa/<thread>-before`,
   after: the same with `-after`. Attach the two `report.md` links to the PR.
2. To audit something new: drive it with the MCP tools, file each problem with
   `finding`, finish with `report out=/mnt/project-files/qa/<name>`.
3. To keep a check: copy a scenario in `qa/scenarios/`, change it, commit it with
   your PR. Use `tree` to find labels, `expect` for what must stay true.
4. Sweeps (`--slow`) take minutes: run them before a release, not per push.

## Regression scenarios

`qa/scenarios/20-regressions-critic.qa` checks the fixed behaviour of defects from
`audit/critic/defects.md` (ids in the comments). A finding means the defect is back.
Add a block to it (or a new `2x-` file) when an audit defect is fixed and merged:
state the fixed behaviour with `expect*` steps, and add a `shot` for the ones only a
picture can show. Run it with `scripts/qa.sh --only critic`.

## Fixtures

Scenarios use files in the repository only: the golden motor starter
(`crates/novo-tests/tests/golden/`), the synthetic AutomationML hoist feeder
(`crates/novo-interop/tests/fixtures/aml/`) and the bundled example (Open example).
No customer data goes into a scenario in this public repository; customer-derived
projects stay in the private repository and are opened by absolute path from a
local scenario that is not committed.

## Where it writes

Everything goes under `--out` (screenshots, logs, reports) and the temporary profile
folder of the run (`kopercad-test-user-<pid>` in the temp folder); `--slow` sweeps
included. Nothing is written beside the fixtures in the repository. The MCP `report`
tool accepts `out` only below the shots folder, the temp folder, `/mnt/project-files`
or the current folder, and `finding` attaches only pictures from the shots folder.

## Windows

`scripts/qa.sh` is a bash script: run it from Git Bash (no admin rights needed).
`KOPERCAD_BIN_DIR` can point at the folder holding `kopercad.exe` and
`kopercad-cli.exe` (for example the unzipped portable build) instead of `target/debug`.
Not verified on Windows yet.

## Gaps, said plainly

* Linux headless (Xvfb, lavapipe software Vulkan, debug build) is what is verified.
  Windows and real GPUs are untested for this suite; the harness has the Windows
  path in `docs/testing/visual-test-user.md`. Screenshots from software rendering
  differ from a GPU (anti-aliasing, SSAO); do not compare them pixel for pixel
  across machines.
* No pixel-diff baseline yet: pictures are for a person or Claude to look at.
* Scale other than 1.0, IME and non-US layouts are not exercised.
* Native file dialogs are answered, not seen.
