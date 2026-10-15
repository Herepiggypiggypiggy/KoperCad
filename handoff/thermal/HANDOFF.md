# Heat model and cooling: handoff

Branch `claude/project-thread-kcg14f` (repo kopercad), PR #406 (reviewer B: READY at 99300c6; later commits ask for re-READY).

How to reach it (user-visible): Cabinet canvas (Ctrl+2) header has a **Heat** toggle button (the Heat layer). On, the
plate shows a heat overlay under the devices (bands cool to very hot) and a bottom strip with the legend (W/dm2 limits),
total known loss, devices without loss data, biggest source, share in the top third, and the planning note.
No palette command yet.

State: engine (`novo-layout/src/thermal.rs`), canvas layer (`novo-canvas/src/cabinet.rs`: HeatView, Heat layer), app toggle and
strip (`novo-app/src/app/cabinet.rs`), tests in each. Enclosure balance numbers (rise, cooling W, fan m3/h) come from
`novo_layout::sizing` (PR #402, Enclosure sizing thread): join them into the strip after #402 merges.

Decisions: mean interior temperature only (no IEC 60890 curves); unknown loss is never 0; device property overrides part
for `power_loss_w` and `load_factor` (load_factor read by name, default 1); cells floor 5 mm, cap 40k cells.
Next: palette command "Toggle heat layer" (with UI integration thread session_01G52sVY5N34jPsrkqWukApR), result strip join, vertical gradient.
Reviewer B session_017hRHV6ik9P7Wcan8CARcGN; message head SHA after every push.
