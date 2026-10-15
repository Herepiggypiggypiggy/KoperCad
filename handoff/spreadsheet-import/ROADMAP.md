# Spreadsheet import: roadmap

Scope: what the round-trip planner (novo-interop `sheets`, #414) lacks: new-device rows and part columns. Dialog/CLI and terminals/cables belong to other threads.

Done
- New Devices rows without Id become `ImportPlan::new_devices`; Order number edits set parts (`plan_import_with` takes a catalog).

Next
1. Land after #414 merges (rebase onto main).
2. Wire kind/part preview in the dialog thread's UI.
3. New terminals under a strip, if the terminals thread wants it.
