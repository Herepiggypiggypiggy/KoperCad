# Enclosure sizing helper: handoff

Branch `claude/project-thread-4m13wx`. Engine only so far, in `crates/novo-layout/src/sizing.rs` with tests in `crates/novo-layout/tests/sizing.rs`. No `novo-core` change, no UI, no build run.

- `EnclosureCatalog` (user list, `from_csv` accepts `;`, `,` or tab and decimal comma; `to_csv` round trips), `recommend(devices, catalog, request)`, `thermal(enclosure, loss, request)`, `effective_surface`.
- Area rule: sum of (w+gap)(h+gap), divided by (1 - duct) and (1 - free space), against usable plate area. Also checks every device fits the plate (either rotation) and depth + reserve fits.
- Heat: IEC 60890 effective surface, k per material, allowed difference inside max minus ambient max, fan flow 3.1 * P / dT.
- Traps: constants unverified (planning values); area rule is not the planner, so the real fit is `place all`.
- Coordination: asked the integration lead to relay to the Part power loss and clearances and Enclosure depth and door swing threads.
