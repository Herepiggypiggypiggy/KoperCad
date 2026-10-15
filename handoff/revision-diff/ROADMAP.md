# Revision diff on the canvas: roadmap

Scope: the compare experience on top of novo-revisions' `diff`/`compare` and the
kept snapshots. The Revision tracking thread owns the engine (`compare`,
`FieldChange`, `ChangeRow` in `list.rs`), CLI/Python, the From/To pickers on the
Changes card and the Compare window. This thread owns the Changes list, the
change report PDF and the overlay.

Done:
- `change_list` / `ChangeRow` (#401, merged).
- Clickable Changes list in the outline, click reveals the object (#442, merged).
- PDF change report: `export.change_report` (#469, READY on 0c1ba09).
- Legend and next/previous change: `view.next_change`, `view.previous_change` (#485, in review).

Next, in order (not started):
1. Overlay: a ghost at the old position for moved items (old outline dashed, new outline amber).
2. Change report: choose the two revisions (use the Compare window's From/To once it exposes them) and a save dialog; today it is latest revision vs now, next to the project file.
3. Cache the Changes list rows without the per-frame `to_vec` copy (reviewer B's note on #442).
4. Export menu item for the report: owned by the "UI integration: reports and checks" thread (#452); command id `export.change_report`.
