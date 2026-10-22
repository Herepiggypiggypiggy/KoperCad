# Real machine build: handoff

Branch `claude/project-thread-0f2s1l`. Findings, shots and outputs: `/mnt/project-files/audit/real-machine/` (findings.md, shots/, outputs/cv200.novo).
Code changes: `novo-workflow/src/generate.rs` (snap generated block origin to the 4 mm grid), `novo-app/src/control.rs` (scroll unit line/page for zoom), `novo-app/src/app/drawers.rs` (Cables drawer scrolls when only proposals exist).
Resume: `cargo test -p novo-workflow -p novo-app`. The harness helpers lived in /home/claude/rm (not committed).
Trap: the test generated_blocks_sit_on_the_snap_grid also passes without the fix; the fixture needs a wire member.
Verdict: machine not shipped as an example (standards report 5 fails, 28 errors).
