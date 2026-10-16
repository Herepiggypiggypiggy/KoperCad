# PANEL-B2 roadmap: door swing, openings, per-surface place-all

1. Slice 1 (PR #752, READY): door swing sweep, `swing.rs`.
2. Slice 2 (this branch): wiring openings, `openings.rs`: `opening.<name>` structure property, keep-out rule `opening.keepout` in the layout report and device rows, `best_passage` for routing.
3. Next: routing uses `best_passage` between plates (novo-routing model.rs belongs to PINS-UI, ask first); openings in the drill table.
4. Next: Place all per surface with rule sections (surface door), door devices with hole and cut-out work (R5).
5. Next: fill `door_devices` from placements whose `surface` property is `door`.
