# T13 Commissioning triangles: handoff

Where the user finds it: Commissioning workspace (outline tab Punch / Checks / Redlines), the schematic while the mode is on, and Simulate > Defects and walk-throughs.

- `Source::Commissioning` was added to `novo-flags` (key `commissioning`, badge "Commissioning"). Rule keys: `punch.<id>`, `check.<anchor key>`, `redline.<id>`.
- Severity: open Blocking punch item red, Major yellow, Minor none (circle icon stays). An item opened by a failed check is red while that check still fails. A failed check with no open item is a red flag of its own. Open redline yellow. Verified and Won't fix items have no flag; a Fixed item keeps its flag until verified.
- Anchors: the row (`Anchor::Control("commissioning.punch.<id>")`, `.check.<target>.<kind>`, `.redline.<id>`) and the wire, terminal or device. Only wire and device-with-symbol anchors draw on the schematic today (`commissioning_marks`); plate and 3D need T1 and T3.
- Jobs: commissioning jobs use T12's path (`job_fail`, `job_ok`, `job_pill`, Dismiss, forget on a new project); `job_error` / `job_doubtful` in `commissioning_flags.rs` wrap `job_fail` and add Retry, which `run_flag_action` handles for keys starting `commissioning.` (everything else goes to `run_job_action` first). Keys are static (`commissioning.field_export`, `.field_import`, `.punch_pdf`, `.report`, `.redlines_apply`, `.photo`, `.photo_unpack`, `.test_sheet`, `.load`). There is no separate strip in the panel any more: one pill in the top bar.
- The flags of the mode are replaced each frame (`sync_commissioning_flags`) and dropped when the mode closes. They are not in the top bar pill or the Issues list yet (they never were in `wb.problems`).
- Defects card has no store flags: it draws rows with `flag_rows::problem_row`. "Different from a healthy plant" lines are results of the experiment, now plain text (class N).
- Nothing here is a compliance sign-off.

Not shown: Minor punch items (no triangle by design), Verified and Won't fix items, passed checks (their green outline is unchanged); Device, Terminal anchors draw on the schematic only through `commissioning_marks`, wires through `flag_marks`; nothing on the plate or in 3D yet; the Redlines tab marks open redlines only.
