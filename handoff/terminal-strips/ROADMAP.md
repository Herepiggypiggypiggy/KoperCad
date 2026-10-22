# Terminal strips and bulk location: roadmap

Scope: RM-16 (all boundary terminals on one strip) and RM-14 (no bulk Set location) from `audit/real-machine/findings.md`.

Done
- Strips by kind: power and wires without a potential -X1, control (DC rails, extra-low voltage) -X2, signals -X3, protective earth -X4, per location. Saved project setting `terminals.strip_grouping` (`class` or `single`), default `class`.
- Set location for any selection of devices and cables (property sheet row `location`), in the Devices tab, the Inspector and Edit > Properties.
- QA: scenarios 69 (1366x768) and 70 (1920x1080), fixture `qa/fixtures/strip-grouping.novo`.

Next (not started)
- Per-class strip names the user can change (today the class-to-strip names are fixed X1 to X4; a terminal can be moved with Move terminal).
- Split power by voltage level (400 V vs 230 V) when a project has both.
- Wire-level "Set location" for cables' ends is not needed; cables use their own tag.
