# Cloud coordinator handoff (wrap-up, 2026-10-14 ~20:30 UTC)

Session: session_01DZ68tTULUv7Uhe8ecKbC47 (claude.ai/code cloud session; branch `claude/youthful-franklin-aey18d`).
Role: Tadjiro handed full coordination of the project to this session from about 17:30 to 20:30 UTC on 2026-10-14. It acted as **integration lead**: it merged PRs, ran reviewer sessions, a fixer and cloud workers, and fixed the merge process.
**Status: stopped.** The owner is near their credit limit. All cloud sessions in `../coordinator/cloud-roster.md` were told to write handoffs and stop. The on-device project threads take over from **23:20 UTC**.

## Read first
1. `../MAIN-HANDOFF.md` §0: current state, open PRs and the review policy.
2. `../RESUME-2320.md`: what each project thread does first.
3. `../integration-lead/PROGRESS.md`: append-only log of every merge today.

## What this session did
- Merged 32 PRs: #20 #41 #34 #28 #29 #42 #45 #44 #36 #30 #48 #47 #50 #31 #57 #51 #52 #38 #54 #58 #59 #49 #46 #60 #56 #61 #64 #55 #66 #67 #62 #63.
- Removed the chokepoint where every new-crate PR conflicted on CLAUDE.md and Cargo.toml (#51). The crate list is now fixed and pre-registered.
- Added `scripts/merge-main.sh` (#51, #58, #61) and `scripts/check-mechanical-merge.sh`, both designed by reviewer A.
- Set up three correctness reviewers (A, B, C), a design reviewer, a fixer, three new-crate workers (WS-12 cables, WS-13 reports, WS-28 CLI/Python) and a Gate 1 integration worker.
- Pre-public scan: no secrets in the code or history. #52 scrubbed personal paths from current files.

## Open PRs at wrap-up (merge each only on the named reviewer's READY on its current head)
| PR | Head at wrap-up | Owner → reviewer(s) | State |
| --- | --- | --- | --- |
| #53 Gate 1 in the app | 67637b7 (moving) | Gate 1 worker → A + design | A READY 53f0e00; design had one item left (the "↵ accept · Tab next" hint hidden by an empty-text duplicate mark). The fix was pushed after that; both must re-check the new head. |
| #65 WS-28 exports | 3354d4e | WS-28 → B + C | B READY 3354d4e; C pending |
| #68 WS-13 follow-up | 02d6e29 | WS-13 → B + C | B READY 02d6e29; C pending |
| #70 editor `art` feature | ae086c5 | fixer → B (moved from A) | pending |
| #71 accept-all order regression fix | 166a8bf | fixer → C | pending. **main has the #64 regression until this lands.** |
| #72 one terminal article source | 38c3070 | fixer → A | pending |
| #69 + this wrap-up | branch head | coordinator → A | docs only |
| handoff PRs from each cloud session | | → lead reads the diff | docs only; merge after reading |

## Mistakes to learn from
- **#64 merged before reviewer C's verdict** (19:57). C then reproduced a regression: horizontal wires numbered 2/1/3. #71 fixes it. Rule: never merge without the *named* reviewer's READY on the *current* head.
- **#41 merged on my own re-review.** A confirmed it afterwards. The lead never approves its own or its helpers' work.
- A shared CARGO_TARGET_DIR ran stale test binaries and gave false greens. Use a target dir per worktree.

## Traps
- `merge_pull_request` needs the full 40-character `expectedHeadSha`. Squash method.
- GitHub Actions starts no jobs (billing / spending limit on the owner's account). Reviewers' local `fmt` + `clippy -D warnings` + `cargo test --workspace` are the CI signal. **Windows is unverified** for everything merged today.
- Disk is about 40 GB per cloud container. Build with `CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0` and delete target dirs afterwards. The fixer hit the limit with a 30 GB target dir.
- Example binary names must be unique across crates (cargo#6313; see #50 and #59).
- Workers sometimes push to a PR after READY (WS-28 did on #65). The READY is then void and the reviewer must re-review.
- Messages between sessions cross. A "queue empty" report may predate your latest assignment, so re-send it.
