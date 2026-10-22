# Text collision stress test: handoff

Branch claude/project-thread-50il08 (kopercad). Kit: crates/novo-tests/src/{text_conflicts,text_stress}.rs, tests/text_conflicts.rs with tests/golden/text_conflicts_baseline.txt, examples/text_conflicts.rs, qa/fixtures/text-stress-*.novo, qa/scenarios/60,61,62, tools/text-conflicts/*.py.
Resume: `cargo run -p novo-tests --example text_conflicts -- /tmp/out` (rewrites fixtures, summary, conflicts.json); copy the .novo files into qa/fixtures when the matrix changes. Run pictures with scripts/qa.sh.
Traps: scenario page switching types the page name into the Pages filter (1366: click_text Pages first; 1920 panel is pinned) then clicks the first row; `key ctrl+a` clears the filter. Fixture must be regenerated when matrix() changes. Boxes are estimates (advance 0.6).
Talk to: Text placement plan thread (owns docs/TEXT-PLACEMENT.md, PR #1351).
Two ratchets: this kit (tests/text_conflicts.rs, per cause, 90 stress pages, independent oracle) and the engine's text_audit (tests/text_baseline.rs, Gate 1 motor starter). Since stage 0b both count a tag over its own symbol; on the motor starter they must agree (test kit_and_engine_audit_agree_on_the_motor_starter). A stage that lowers one re-checks the other. The kit baseline moves with symbol art too: look at which causes moved before updating it.
