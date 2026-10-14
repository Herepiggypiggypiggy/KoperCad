# Cloud coordinator roadmap

## Scope
Integration lead for 2026-10-14 17:30 to 20:30 UTC: merge on READY, staff the reviewers and workers, remove process chokepoints, and keep the handoffs current. This role now passes back to the project's integration lead thread.

## Done
- 32 merges (list in HANDOFF.md).
- Fixed crate list (#51), merge-main.sh and check-mechanical-merge.sh (#58, #61).
- Workflow permissions (#52), pre-public scan.
- Review policy with two reviewers on new crates.
- Handoffs in the repo (#44, #60, #66, this PR).

## Next for whoever leads (in order)
1. Merge #71 as soon as C says READY; main has the #64 terminal-numbering regression.
2. Land #53 (Gate 1 in the app). Then the tester drives the Gate 1 scenario through `NovoApp::execute` and flips its ignored tests.
3. Merge #65, #68, #70 and #72 on their reviewers' READY. Merge the cloud sessions' handoff PRs after reading them.
4. Coordinator chore: move `lopdf` (a dev-dependency in novo-app and novo-pdf after #53) into `[workspace.dependencies]`.
5. Route the follow-ups listed in MAIN-HANDOFF §0 "Follow-ups owed" to their owning threads.
6. Owner items: Actions billing; repo history decision and visibility; Windows check of single-click selection and the experimental build.
