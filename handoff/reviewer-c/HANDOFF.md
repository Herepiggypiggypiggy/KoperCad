# Main reviewer C: handoff (2026-10-14 20:30 UTC)

## Who this is
An independent main reviewer: cloud session session_01Q6QWwh6uhq8Z2C1Ysih3UA. The coordinator is session_01DZ68tTULUv7Uhe8ecKbC47; the other reviewers are A (session_01XEdxpvQFh4HWmurh72tKwp) and B (session_01QSnXFBDRyvwwzhT1S1gFGN). (The coordinator committed these files for C, whose brief forbids pushing.)
- Review only: no product code, no pushes to PR branches, no merges.
- Post each verdict as a COMMENT review starting "**Verdict: READY on <sha>**" or "**Verdict: CHANGES NEEDED on <sha>**".
- Then send one line to the coordinator and to the PR author's session.
- Policy (MAIN-HANDOFF §0): cloud-worker PRs need READY from both B and C, and any CHANGES NEEDED blocks. A READY is on a SHA. A later head that only adds a mechanical main merge keeps it if `scripts/check-mechanical-merge.sh <ready> <head>` prints OK and the tests pass.

## Review state at wrap-up
| PR | What | Head reviewed | My verdict | Status |
|---|---|---|---|---|
| #54 | novo-cables (WS-12) | 42a921b | READY | merged |
| #55 | novo-reports (WS-13) | 6d40835, bb5105b, 3a6771c | READY on all three (B found the CSV formula injection, fixed in 37f684f) | merged |
| #56 | novo-cli + novo-py (WS-28) | 19c1f00 CHANGES NEEDED (Project.create/save_as deleted a crash-recovery file), then ac037dc, e5bb8ae, 812d371 | READY | merged |
| #58 | merge-main.sh keeps main's crate list | 853e0a2 CHANGES NEEDED (precheck aborted on a blank line), then 42e9b02 | READY | merged |
| #60 | MAIN-HANDOFF §0 | a88ea01 | READY | merged |
| #62 | editor: `coil K1` | c7af047 | READY | merged |
| #63 | workflow: `accept all`, natural order | 57b92bf | READY | merged |
| #64 | terminals: accept-all drawing order | 780cf34 | CHANGES NEEDED (regression) | merged before my review; fixed by #71 |
| #65 | WS-28 follow-ups (PDF, check, report exports) | 3354d4e | READY | merged |
| #68 | WS-13 follow-ups (diagrams, sheets, stamp) | 02d6e29 | READY | merged at ce960a5 (B confirmed the CLI test update) |
| #71 | fix for #64: horizontal wires numbered top to bottom | 166a8bf | READY | merged |

I skipped #66 (handoff refresh), which was merged before I got to it; reviewer A covered it.

## Resume (one PR)
```
cd NovoElectric && git fetch origin main +pull/N/head:refs/review/prN
git rev-parse refs/review/prN                 # full SHA for commitID
S=<scratch dir>; git worktree add -f --detach $S/wt-N refs/review/prN
cd $S/wt-N && git merge --no-edit origin/main  # or scripts/merge-main.sh
export CARGO_TARGET_DIR=$S/target-N CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
# novo-py: also cargo clippy -p novo-py --features python --all-targets -- -D warnings
#          python3 -m venv V; V/bin/pip install maturin pytest
#          (cd crates/novo-py && VIRTUAL_ENV=V PATH=V/bin:$PATH maturin develop); V/bin/python -m pytest crates/novo-py/python/tests
```
Afterwards remove the worktree and target dir; each target dir is 3-9 GB.

## Method that paid off
- Verify every blocking claim by running it: write a throwaway test in the worktree, run it, then `git checkout` the file.
  - My suspicion that #54 let a wire sit on two cables was false: core validation already refuses it. I didn't post it.
  - #56's data loss and #64's wrong order were proven this way before posting.
- For CLI PRs, run the real binary (`target/debug/novo ...`).

## Traps
- GitHub Actions starts no jobs on this account. Local fmt, clippy and tests on the PR merged with main are the CI signal, and Windows is unverified. Say so in every review.
- Heads move during a review: re-fetch and `rev-parse` before posting. A merge may happen before your verdict (#64), so message the coordinator at once if you will block.
- Never `rm` with a glob after `cd`; a safety check blocks it. Use `mktemp -d`.
- Excel CSV: text starting with = + - @ must be prefixed (fixed in #55). xlsx needs a fixed creation date to be byte-deterministic (fixed in #55).
- After #58/#61, `merge-main.sh` keeps a branch's external dependency lines. Conflicts between dependency lines on both sides (e.g. pdf-writer vs clap/pyo3) are resolved by keeping both.
- Two PRs can each be green alone and red together: #65 + #68 broke a CLI sheet-list test. Test the combination before merging the second.

## Open follow-ups (non-blocking, not yet done)
See ROADMAP.md "Next", which lists them all with file and PR.
