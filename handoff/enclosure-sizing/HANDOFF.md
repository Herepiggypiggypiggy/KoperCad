# Enclosure sizing helper: handoff

Branch `claude/project-thread-4m13wx`. #402 merged: engine `novo-layout/src/sizing.rs`, bridge `sizing_model.rs`, tests in `crates/novo-layout/tests/`. No `novo-core` change, no build run.

API: `EnclosureCatalog` (user list; `from_csv` accepts `;`, `,` or tab and decimal comma, `to_csv`), `recommend(devices, catalog, request)`, `thermal(enclosure, loss, request)`, `effective_surface`, `sizing_model::sizing_devices(project, structure)`. Part keys read: `width_mm`, `height_mm`, `depth_mm`, `power_loss_w`, `clearance_*_mm`, `load_factor`; device property overrides part.

## How to reach it (user-visible entries; none placed yet)
Owned by the "UI integration: cabinet, 3D, enclosure" thread (session_01G52sVY5N34jPsrkqWukApR), who got the API and the proposal:
- Cabinet side panel "Size the enclosure", command `cabinet.size-enclosure`.
- "Enclosures..." catalog editor with CSV import and export, command `cabinet.enclosures`, file `enclosures.csv` under `%APPDATA%`.
- Issues-list problem "needs about N W of cooling" (open: who adds it; the catalog lives outside the project, so the check needs the enclosure's own dimensions from the Enclosure thread's #409 fields).

## Traps
- Constants (K_*, surface factors) are planning values, unverified against a maker handbook; the UI must say so.
- The area rule is an estimate, not the `place all` planner.
- Heat model thread (#406) owns per-device heat contribution and the hot-spot map in `thermal.rs`; do not duplicate the balance.
- `depth_reserve` is a flat number until `novo_layout::enclosure` (#409) interior fields are used.
