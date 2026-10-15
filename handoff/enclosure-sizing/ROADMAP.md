# Enclosure sizing helper: roadmap

Scope: recommend plate and enclosure size from devices, gaps, duct allowance and free space, and a heat estimate (natural cooling enough? cooling power needed). Local only: the enclosure list is the user's (typed or CSV import), nothing bundled from makers. Engine in `novo-layout/src/sizing.rs`.

## Done
- PR 1: engine, CSV catalog, tests.

## Next
1. Bridge: build `SizingDevice` list from an enclosure's devices (`device_size` plus part keys `depth_mm`, `power_loss_w`, `load_factor`; keys come from the Parts library thread, R0).
2. Per-part clearances once the layout rules thread (R1) lands; replace `SizingDevice.clearance`.
3. Enclosure depth and door swing (R2 thread): use its interior and setback fields instead of `depth_reserve`.
4. Problem in `novo-checks` ("needs about N W of cooling").
5. Thin UI: catalog editor and import, result panel. Persist the catalog under `%APPDATA%`.

## Open questions
- Thermal constants (K_* and surface factors) must be checked against IEC 60890 and a maker handbook before numbers are shown in the UI.
