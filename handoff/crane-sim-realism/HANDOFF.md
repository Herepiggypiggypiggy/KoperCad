# Crane sim realism: handoff

Branch `claude/project-thread-igbbvo`. Thread owner: "Make crane sim shapes realistic". Coordinator relays PRs by parity.

State: S1 written (`crates/novo-view3d/src/crane_parts.rs`, `crane.rs` uses it, `hoist_3d.rs` calls `crane_parts::ensure`). Scenario `qa/scenarios/9600-crane-parts-shapes.qa`. Evidence goes to `/mnt/project-files/qa/pr<N>-<sha8>/`.

Resume: `CARGO_TARGET_DIR=/tmp/kc-target cargo test -p novo-view3d`, then `scripts/qa.sh --only crane-parts --out /mnt/project-files/qa/<name>`. The qa box needs openbox, mesa-vulkan-drivers, libxkbcommon-x11-0.

Traps: parts are drawn at the item's local box (x, y centred, z from 0); `crane_parts::item` hides that. The reference STEP file is customer-derived (private): never commit it or its names/numbers.
