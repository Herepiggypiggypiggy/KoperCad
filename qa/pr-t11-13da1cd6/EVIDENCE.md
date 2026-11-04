# Problem triangles T11 evidence (source at git 13da1cd6ad)

Binary built from head 13da1cd6ad7572204ded43a96d4a9c6c0e4709a2; scenarios `9511-emc-triangles-popover-1366x768` and `9512-emc-triangles-popover-1920x1080`, 0 failed each. The one finding per size is an S4 "may overlap" note (the Cabinet tab behind the EMC window, which docks over it on purpose). Reports and pictures in `1366x768/` and `1920x1080/`.

## Where the user finds it
- Schematic, with Check > EMC check > "Run the EMC rules with the design checks" on: each EMC finding is a triangle on its wire or device (a device finding sits on the device's symbol). Click it: popover with title, EMC badge, a "?" with the design-aid caveat, Locate, Preview the fix (only when the program can apply a suggestion), Open EMC check, Waive, Copy.
- Safety function, cable rating, short-circuit, disconnection, thermal, voltage drop, starter and protective conductor findings keep their Issues triangles but carry the Safety or Calc badge, the caveat and an Open button for their window (Safety functions, Calculations); standards findings get Open standards report.

## Tested
fmt, clippy -D warnings (novo-app), novo-app lib tests (2229 passed, 17 ignored), novo-flags tests, the two QA scenarios.

## Not tested / not in this PR
- Full workspace test run (cloud disk fills); CI covers it.
- The findings tables of the EMC window, Calculations and Safety are not yet filtered views of the flag store, and the 66 toast/coloured/banner sites of the area are only baselined in `handoff/problem-triangles/ceilings/safety.txt`, not migrated: next PR from main.
- Terminal, cable and plate/structure anchors are still not drawn on the schematic. No QA scenario for the Safety and Calc sources yet (unit-tested adapter only).
