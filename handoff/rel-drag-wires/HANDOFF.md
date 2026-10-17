# rel-drag-wires handoff

Branch claude/project-thread-9bwxxm, one PR from main.
Cause: not a bug in the carry path. A plain drag deliberately left wires behind as open ends (Preferences "Moving a symbol" defaulted to "leaves", DESIGN.md section 4); testers read that as data loss.
Fix: default is now "takes its wires along" (Shift leaves them). Setting renamed `move_leaves_wires` (old `move_carries` key ignored, so nobody keeps an unchosen default). Cabinet plate moves use other code (novo-app view3d_move) and are untouched.
Tests: crates/novo-editor/tests/drag_keeps_wires.rs (single, multi-select, one undo step, view default); gate1_tests preference test flipped.
