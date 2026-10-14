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
1. Get #72 (one terminal article source) reviewed by the main reviewer and merged. Expect a trivial novo-reports/Cargo.toml conflict with #68: keep one `novo-terminals` line.
2. Close #73 (superseded by #69).
3. Tester: drive the Gate 1 scenario through `NovoApp::execute` now that #53 is in, and flip the ignored tests.
4. Move `lopdf` (a dev-dependency in novo-app and novo-pdf) into `[workspace.dependencies]`.
5. Route the follow-ups in MAIN-HANDOFF §0 to their owning threads.
6. Owner items: Actions billing; repo history decision and visibility; a Windows check of single-click selection and of the experimental build.
