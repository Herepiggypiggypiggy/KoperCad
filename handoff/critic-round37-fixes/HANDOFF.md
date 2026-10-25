# Critic round 37 fixes: handoff
Branch claude/project-thread-wsjnf0, one PR. QA scenarios 410, 411, 412 (pictures in /mnt/project-files/qa/c37-fixes/).
- C-67: the last build-order step forces the wires layer on and its text says how many wires are drawn, or why none can be (view3d.rs, view3d_build_order.rs).
- C-68: step 0 lights the plate (novo-view3d assembly::added).
- C-65: unavailable EMC chips read "n/a" (emc_view.rs layer_chips).
- C-66: hot-spot badges keep off other spots' points; ring size fixed (emc_plate.rs).
- C-69: device tags keep 1.5 mm plus 0.8 of their width from the frame, because drawn text is wider than its mm box at fit zoom in small windows (novo-canvas model.rs place_tags). The tag did touch the frame at 1366x768 before.
Trap: cloud disk fills; use CARGO_INCREMENTAL=0 and scripts/free-disk.sh.
