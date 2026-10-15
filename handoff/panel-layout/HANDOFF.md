# Panel layout intelligence: handoff

Branch `claude/project-thread-o2407u` (restart from main after each merge). Engine code in `crates/novo-layout`; design in `docs/design/cabinet-layout.md` section 5b.1.

Resume: `cargo test -p novo-layout && cargo clippy --workspace --all-targets -- -D warnings`.

Decisions
- No `novo-core` change: duct class is the rail property `duct_class`; surfaces will be placement properties.
- The check judges relative zone order, not absolute bands, so a plate of only top-zone devices is not "wrong".
- Separation is given up before a device is left off (never silent: `Explanation::separation_relaxed`).
- Defaults (80 mm drive room, 40 mm duct separation, 25 W hot limit, 2000 mm stock, 2 mm kerf) are practice values, not standards.

Coordinate with: Enclosure depth and door (#409, `enclosure.rs`), Cabinet UI thread (cabinet.rs), Heat (#406).
