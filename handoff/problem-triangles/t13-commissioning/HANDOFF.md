# T13 Commissioning triangles: handoff

Where the user finds it: Commissioning workspace (outline tab Punch / Checks / Redlines), the schematic while the mode is on, and Simulate > Defects and walk-throughs.

- `Source::Commissioning` was added to `novo-flags` (key `commissioning`, badge "Commissioning"). Rule keys: `punch.<id>`, `check.<anchor key>`, `redline.<id>`.
- Severity: open Blocking punch item red, Major yellow, Minor none (circle icon stays). An item opened by a failed check is red while that check still fails. A failed check with no open item is a red flag of its own. Open redline yellow. Verified and Won't fix items have no flag; a Fixed item keeps its flag until verified.
- Anchors: the row (`Anchor::Control("commissioning.punch.<id>")`, `.check.<target>.<kind>`, `.redline.<id>`) and the wire, terminal or device. Only wire and device-with-symbol anchors draw on the schematic today (`commissioning_marks`); plate and 3D need T1 and T3.
- Jobs: `Source::Job` flags with `Anchor::Job("commissioning.*")`, set with `job_error` / `job_doubtful`, cleared with `job_ok`; drawn by `PanelFlags::job_strip` at the top of the panel; popover Retry calls `retry_commissioning_job`. Away from the Commissioning outline an Info line says where to look.
- The flags of the mode are replaced each frame (`sync_commissioning_flags`) and dropped when the mode closes. They are not in the top bar pill or the Issues list yet (they never were in `wb.problems`).
- Defects card has no store flags: it draws rows with `flag_rows::problem_row`. "Different from a healthy plant" lines are results of the experiment, now plain text (class N).
- Nothing here is a compliance sign-off.
