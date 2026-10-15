# Second main reviewer (B2) handoff

Role: second main-correctness reviewer. Takes ODD-numbered PRs for correctness and tests; the original main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) takes even ones. Never pushes product code, never merges, no build. Verdicts: GitHub COMMENT review (full 40-char SHA, footer) plus a message to the integration lead (cse_01FiLvRBKEcNNHbQEGhYyL1S) with verdict and head SHA.

Method: see handoff/reviewer-b/HANDOFF.md. Merge origin/main into a worktree of refs/review/prN, run fmt, clippy -D warnings, test with one shared CARGO_TARGET_DIR, delete worktrees and target afterwards. Actions runs no jobs on this account; the local run is the CI signal.

State at 2026-10-15 17:30Z (all reviews posted):
- #325 READY 220a9c5; #329 READY 50b0d17
- #327 CHANGES NEEDED 0f62230 (Tx::remove of the old wire clears its cable core; update the wire in place)
- #323 CHANGES NEEDED 3632c97 (Changes tab re-enables Changes mode every frame, so it cannot be left)
Next: re-review #327 and #323 when the lead sends new heads; review newly opened odd PRs.
