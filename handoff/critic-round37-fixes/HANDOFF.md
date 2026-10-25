# Critic round 37 fixes: handoff
Branch claude/project-thread-wsjnf0, one PR. QA scenarios 410, 411, 412 (pictures in /mnt/project-files/qa/c37-fixes/).
- C-67: the last build-order step forces the wires layer on and its text says how many wires are drawn, or why none can be (view3d.rs, view3d_build_order.rs).
- C-68: step 0 lights the plate (novo-view3d assembly::added).
- C-65: unavailable EMC chips read "n/a" (emc_view.rs layer_chips).
- C-66: hot-spot badges keep off other spots' points; ring size fixed (emc_plate.rs).
- C-69: device tags go through two passes over the same places (novo-canvas model.rs place_tags): first a place with room to the frame (1.5 mm plus 0.8 of the tag width, because drawn text is wider than its mm box at fit zoom in small windows) and no obstacle, then the old bare-minimum rule. A tag keeps the room only when a roomier free place exists. A single wide rule traded tags for wires (motor starter ratchet 0,1,1 became 0,0,4), hence two passes. Text stress matrix moved: tag/symbol-line 224 to 229, tag/tag tight 2 to 3 (tags near the frame now face a neighbour); baseline text_conflicts_baseline.txt updated. Motor starter ratchet stays 0,1,1.
- QA evidence: /mnt/project-files/qa/c37-fixes/ (HEAD.txt names the build commit).
Trap: cloud disk fills; use CARGO_INCREMENTAL=0 and scripts/free-disk.sh.
