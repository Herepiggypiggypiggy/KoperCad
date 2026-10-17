# WORK-2 progress
- 2026-10-16: slice 1 (field results file, merge, signature type) built, tests in tests/field.rs and commissioning_tests.rs.
- 2026-10-17: as-built redlines: novo-commissioning redline.rs (Change, Redline, apply_commands, revision_description), store + .kcfield merge, Redlines tab in the Commissioning panel (app/commissioning_redlines.rs), tests in tests/redline.rs and commissioning_tests.rs.
- 2026-10-17: slice 3 photos built (PR from claude/work-2-photos).
- 2026-10-17: signature pad: Verified on a Fixed punch item opens a pad (name + strokes) under the item; Sign and verify calls store.sign_off (Signature::from_strokes clamps and caps). Reopen drops the signature. app/commissioning_signature.rs.
- 2026-10-17: photo viewer: punch items show picture thumbnails (PNG and JPEG, decoded once, longest side 1600) and a touched one opens a Photo window. app/commissioning_viewer.rs; `image` (png, jpeg) added to workspace deps for it.
