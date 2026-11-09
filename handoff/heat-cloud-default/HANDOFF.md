# Heat cloud default: handoff
Cause: in the crane sim example (and every project) the 3D heat view solved the cabinet but Cloud and Flow started Off, so only a flat slice showed. Fix in `crates/novo-app/src/app/view3d.rs` (`heat_looked`). Scenarios 1990 and 1990b (crane sim example, 1366x768 and 1920x1080) expect Flow "Streamlines" after the first result; they fail without the fix.
Where to find it: Cabinet > Simulate heat (or 3D > Heat), then the 3D view.
