# QA suite roadmap

Scope: an easy test and screenshot suite so Claude threads can drive the real app, take
screenshots at fixed sizes, walk workflows, and report findings in one format.

Done
- `kopercad-cli qa run|list|report` (crates/novo-cli/src/qa.rs), `scripts/qa.sh`.
- Scenario files `qa/scenarios/*.qa` (format in docs/testing/qa-suite.md).
- Cheap checks: layout from the widget tree, blank frame, app log panics, window size.
- Sweeps over every command (`sweep_commands`) and every lesson (`sweep_lessons`), `--slow`.
- MCP tools `check_layout`, `finding`, `report` in `kopercad-cli test-user`.
- Fixtures: golden motor starter, AML hoist feeder, bundled example.

Next, in order
1. Scenarios per area: Heat (Cabinet Heat panel, 3D heat), Parts (assign, bulk), Learn (run one lesson),
   Reports (open each report, export), Commissioning (pass an item), Save/recover, PDF export.
2. Round trips: export then import (AML, spreadsheet) and compare through `expect_scene`.
3. Pixel baselines for regression screenshots (same renderer only), a `--baseline DIR` compare.
4. Windows run through Remote Control (see visual-test-user.md); note GPU differences.
5. Nightly workflow that runs `scripts/qa.sh --slow` on Linux and attaches the report (needs a CI PR).

Open questions
- Where baselines live (repo vs project mount) and how big they may get.
