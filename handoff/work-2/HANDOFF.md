# WORK-2 handoff
State: PR 1 adds crates/novo-commissioning/src/field.rs (FieldPackage, MergeReport), Signature on PunchItem, and Field results Export/Import buttons in the Commissioning panel (app/commissioning.rs). Merge rule: newer `at` wins per check; punch items matched by raised_at+raised_by+title+target, the further state wins.
Entry: Commissioning workspace (Ctrl+5) > Field results Export / Import.
Next: see ROADMAP.md item 2.
Redlines (PR from claude/work-2-redlines): Commissioning panel > Redlines tab. Pick a target on Checks, add a note, new wire number or new device tag. "Apply to drawing" runs one Batch command (undo in the schematic; undo is off while the mode shows); "Apply and issue revision" then opens the revision dialog with one "As built:" line per change. Redlines travel in .kcfield (key at+by+target+change, applied/rejected beat open). Commissioning never edits the drawing itself.
