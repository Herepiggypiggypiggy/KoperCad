# Lead fixes for PR #28 (integration lead, 2026-10-14 ~18:20 UTC)

The thread was at its session limit, so the integration lead (cloud session
session_01DZ68tTULUv7Uhe8ecKbC47) did this work locally. That session may push
only to its own branch, so nothing was pushed to `claude/canvas-engine-ymvwu2`.

Result: branch `fix-pr28`, head `a3302fc` = PR head `924be9b` + merge of main 6d10ab4
(#35 core texts and potentials, #20 theme; no conflicts) + one fix commit (0001 here).

Fixes:
- Junction glyphs count wires, not sides: 3+ wires always get a dot (T on exactly
  three sides, plain dot otherwise); a corner only for exactly two wires at a right
  angle. A wire ending exactly at the junction point now counts. Tests for
  [x,y,y], [x,x,y,y], [x,y,zero], [x,-x,y,y] and a scene test.
- #35 follow-ups in PageScene: Text records drawn on a new `texts` layer
  (ModelLayers gains a public `texts` field); redrawn on add/change/remove and when
  their host symbol, wire, junction, device, terminal or potential changes; a symbol
  with a DeviceTag text no longer draws its own tag; wire potential labels refresh on
  set, clear and rename. Headless test `text_records_are_drawn_and_follow_their_hosts`.
- Hit::distance doc comment corrected.

Open, non-blocking: pixel snap uses view.size*ppp; symbol drawing cache cleared only
on rebuild/style change; text vs own-symbol drag priority; no hover handle on a
two-wire corner; hit.label on symbols indexes labels, not text records; a DeviceTag
text on a terminal shows the full designation (-X1:3) per novo-core text_string.

Your queued next-PR patches (../0001-Canvas-marks..., ../0002-Canvas-page-frame...)
were made on 924be9b; rebase them after this lands.

## Apply (thread owner, on resume)
```
git fetch origin && git checkout claude/canvas-engine-ymvwu2
git status                     # PR still open? head still 924be9b?
git fetch <path>/handoff/ws3/lead-fixes/pr28.bundle fix-pr28:fix-pr28
git merge --ff-only fix-pr28   # fast-forwards to a3302fc
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
git push origin claude/canvas-engine-ymvwu2
```
Then send the reviewer the new head SHA and update the PR body. Review it like
your own work before pushing: you own the crate.
