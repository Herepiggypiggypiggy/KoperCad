# Visual tester playbook

You are a first-time KoperCAD user with a screen, a mouse and a keyboard, not a
code reviewer. Use only the MCP tools of `kopercad-test-user`
(`docs/testing/visual-test-user.md`). Do not read the source to find out how
something works; if you would need to, that is a finding.

## Rules

1. Start from the goal in the scenario, in your own words. Take a screenshot
   before and after every step that matters.
2. Act like a person: read what is on screen, try the obvious thing first
   (menus, buttons, hints in the status bar), then the keyboard. Note every
   moment you hesitate, guess, misclick, or cannot tell what happened.
3. Do not work around a stuck point silently. Write it down, then use the
   shortest honest way forward and say which one you used.
4. Check every number you are shown for plausibility (tags, counts, lengths,
   totals) and whether it carries its label (`~`, "estimate", "planning value").
5. Run each scenario at 1366x768 first (the smallest supported laptop), then
   1920x1080 once. Dark theme is supported; light only if the scenario says so.
6. A tool error, a frozen window or a crash is S1 until proven otherwise:
   screenshot, `info`, then `restart`.

## Severity (from the v0.1 plan)

S1 data loss, crash, cannot start. S2 a core workflow cannot be finished, or a
wrong number is shown as fact. S3 everything else (confusing, ugly, slow).

## Friction log format

One file per run: `/mnt/project-files/qa/v0.1/<scenario>-<build>.md` (copy it to
`qa/` in the repository when it matters for a PR). Screenshots next to it.

```
# Scenario A, build <BUILD_INFO id>, 1366x768, <date UTC>, tester <name>

Goal as I understood it: ...
Result: finished | finished with detours | not finished at step N
Time-ish: <number of tool calls>   Detours: <count>

| # | Step | Sev | What I expected | What I saw | Evidence | Where I looked first |
|---|------|-----|-----------------|------------|----------|----------------------|
| 1 | Placing a contactor | S3 | power contacts | only a coil | shot-004.png | Q search "contactor" |

## What worked well (keep it)
## Numbers I checked
## Not tested / blocked (say why)
```

## Rubric (score each 0 to 2 at the end of the run)

- Found the first action without help (start screen, hints).
- Every action gave visible feedback (selection, toast, status bar).
- Errors say what is wrong and what to do next, in plain words.
- Nothing clipped, overlapping or unreadable at 1366x768.
- Numbers are plausible and labelled honestly.
- Undo, save and recovery behave as a person expects.

## Scenarios (ids match `release/v0.1-plan.md`)

Each is a goal. Do not follow a script; the steps are only a sketch of "done".

| Id | Goal | Done looks like |
|----|------|-----------------|
| A | Draw a direct-on-line motor starter on two pages: breaker, contactor, overload, motor, stop/start, feed lines; wire it; assign parts; number the wires | tags and cross references right, numbering preview, standards check finds the thin wire |
| B | Save it, close, reopen, simulate a crash and recover; Save as; export the drawing as PDF | model identical after reopen, recovery card appears, file path is shown after export |
| C | Produce what a workshop needs: cut list, ferrule table, labels, contents, Excel | counts add up, no empty first pages, success message, placeholders marked `~` |
| D | Choose real parts for the devices and get a BOM | suggestions fit the device and voltage, BOM totals match |
| E | Lay the starter out in a cabinet: plate, rails, ducts, place all, drag, fix an overlap, route, size the enclosure | placed, checks flag overlaps, wire lengths shown |
| F | Look at the cabinet in 3D, measure, run heat Simulate, cancel it, move a device | no freeze, labels honest |
| G | Run the live simulation: hold Start, latch, Stop; motor start; a fault | coil and contact states, estimates labelled |
| H | Terminals, cables, reports, revisions | generated pages update live |
| I | Workspaces and layouts at both sizes, light and dark | nothing clipped, menus scroll |
| J | Install, upgrade, uninstall as a standard user (Windows, Remote Control) | no admin prompt |

## Known harness limits (do not report as product bugs)

Native file dialogs (Open, Save as, folder pickers) are separate OS windows
and cannot be seen or clicked: a Ctrl+S on a new project may appear to do
nothing. Report it as a tool limit, then use in-app paths (`restart project=`,
recent files). The schematic canvas and 3D view have no entries in `ui_tree`.
