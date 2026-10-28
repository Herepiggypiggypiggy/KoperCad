# Edge test handoff
Branch claude/project-thread-laudu8. New tests: crates/novo-tests/tests/hostile_files.rs, hostile_cabinets.rs; scenarios qa/scenarios/600-608-hostile-*.qa.
Fix: novo-thermal validate refuses negative wall or sizes over 20 m (was a hang).
Open: E-2 empty cabinet shows a preview temperature map with hot-spot badges (S3, Cabinet heat owner).
Resume: `scripts/qa.sh --only hostile`; full list in /mnt/project-files/qa/pre-work/edge.md.
Trap: scenario files may only name files committed in the repo (a unit test checks); big local inputs go in /tmp scenarios.
Trap: the harness `key` step does not know pgup/pgdn.
