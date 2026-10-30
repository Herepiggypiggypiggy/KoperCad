# Controller panel kinds and signal filter: handoff

Branch `claude/controller-panel-kinds-filter`, from main f976c8e. PR number and head SHA: see the last line of PROGRESS.md.

## State

- Add-on API 2 panel kinds (`rocker`, `joystick`, `selector`, `keymap`, `table`, `tabs`, `bar`) in
  `crates/novo-addons/src/layout.rs`; drawn by `crates/novo-app/src/app/addons_widgets.rs`.
  `API_VERSION = 2`, `MIN_API_VERSION = 1`; a kind needs the API it came with.
- Controllers add-on 0.2.0 (`addons/controllers/pack`) uses the new kinds (tabs Motions, Keys, Contacts).
- Signal filter: `crates/novo-controller/src/monitor.rs` (pure, tested); UI in `controller_card.rs`
  (`filter_bar`, `signals_block`, `bindings_block`); pop-out panels come from main (`popped_panels`, `ctl.popped`), not from this PR.
- Docs: `docs/design/addons.md`, `docs/design/controller-live.md`, `addons/controllers/README.md`.
- QA scenarios 5630 (add-on panel tabs), 5631 (signal filter), 5632 (all controllers).

## Resume

```
export CARGO_TARGET_DIR=/tmp/kc-target-panel CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0
cargo test -p novo-controller -p novo-addons
cargo test -p novo-app --lib -- addons_widgets controller_card
scripts/build-controllers-addon.sh && cargo test -p novo-tests --test controllers_addon
scripts/qa.sh --only 563
```

## Traps

- Set `CARGO_TARGET_DIR` on every cargo call (shell state does not persist); a `target/` in the worktree
  filled the disk once. `scripts/free-disk.sh` if tight.
- The `tabs` headless test only proves the second tab is reachable (the sweep clicks its header).
- Main landed its own pop-out panels while this PR was open; this PR dropped its own version in the merge.
- Once the PR is marked READY, do not push again; follow-ups go in a new PR from main.
