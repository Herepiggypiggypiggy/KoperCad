# Rail and duct ends: handoff

Rules and geometry: `crates/novo-layout/src/railedit.rs` (pure, tested). UI: `crates/novo-app/src/app/cabinet_rail_ends.rs` (handles, preview, card, apply) and the canvas block in `cabinet.rs`. Entry: Cabinet, select a rail or duct, drag its handles; Rails and ducts window (palette: rails) has the card under the list. Applies only the rail (devices stay), one undo step. Compatible with PR 1604 (touching is fine, overlap refused).
