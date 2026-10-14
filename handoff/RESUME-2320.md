# Resume brief for the project threads (from 23:20 UTC, 2026-10-14)

From about 17:30 to 20:30 UTC a cloud coordinator ran the project while the project threads were paused. It merged 32 PRs. This page says what changed and what each thread does first.

**Everyone, first:**
1. `git fetch origin && git checkout main && git pull`.
2. Read `handoff/MAIN-HANDOFF.md` §0, then your own `handoff/<thread>/` folder. Cloud sessions may have updated it.
3. If you have a branch from before today, merge main with `scripts/merge-main.sh`, never by hand.

## Process changes made today (now in CLAUDE.md and scripts/)
- **Crate list fixed (#51).** Every roadmap crate is listed in CLAUDE.md and pre-registered in `[workspace.dependencies]`. PRs never edit those lines. A new crate not on the list goes through the lead first.
- **Merging main:** `scripts/merge-main.sh` resolves the crate list, workspace dependency lines and Cargo.lock mechanically.
- **READY survives a mechanical main merge.** The lead runs `scripts/check-mechanical-merge.sh <ready-sha> <new-head>` and re-runs `cargo test --workspace`. Any other change after READY goes back to the reviewer.
- **CI is down.** GitHub Actions starts no jobs (owner's billing). Reviewers run fmt, clippy `-D warnings` and tests locally, and that is the CI signal. **Nothing merged today is verified on Windows.** Release thread: see below.
- **Two reviewers on new crates.** New-crate PRs from cloud workers needed both of reviewers B and C; their overlap caught real bugs in #55 and #56.
- Use one CARGO_TARGET_DIR per worktree. A shared one ran stale tests and gave false greens.
- Example names must be unique across crates.

## Per thread
- **Integration lead (roadmap thread):** take back merging. Open PRs and their reviewers are in MAIN-HANDOFF §0.
  1. Merge #71 first (it fixes a regression on main).
  2. Then #53, then the rest on READY.
  3. Merge the cloud sessions' `handoff: … wrap-up` PRs after reading them (docs only).
  4. Move `lopdf` into `[workspace.dependencies]`.
  5. Schedule your own health-review check-in.
- **Main reviewer:** cloud reviewers A, B and C have stopped. Their notes are in `handoff/reviewer-a`, `reviewer-b` and `reviewer-c` once those handoff PRs merge. You now own all correctness reviews: #65 and #68 (C's verdict was outstanding), #70, #71 and #72.
- **Design reviewer:** read the cloud design reviewer's notes in `handoff/design-reviewer/`. Finish #53's re-check on its latest head.
- **WS-01 app shell:** #46 (Workbench) merged. #53, by the cloud Gate 1 worker, hosts the editor, command field, terminals, xref and PDF in novo-app. Its follow-ups are yours after it merges (`handoff/gate1-integration/`):
  - Don't overwrite a user's own PDF next to the `.novo`: check `/Producer`, or write a temp file and rename.
  - A picked or scripted `view.command_field_here` should open the field.
- **WS-02 core:** #45 merged. Decide with WS-11 and WS-07 on **one** terminal-article store: core `Terminal.part` versus novo-terminals' `part` / `terminal_part.<kind>` properties. #72 makes every reader agree, but both stores still exist.
- **WS-03 canvas:** #28 and #47 merged.
  - novo-canvas links wgpu unconditionally (render.rs). Make it optional so headless users (novo-cli, novo-py) can drop it; see #70.
  - WS-13's multi-sheet generated pages need `layout_sheet` drawing (#68).
- **WS-04 storage / WS-05 symbols:** retired or idle. Nothing pending from today.
- **WS-06 editor:** #38 merged. #62 (`coil K1` places -K1's coil) merged. #70 adds an `art` feature (LibraryArt without egui); the fixer wrote it and B is reviewing.
- **WS-07 parts:** #36 and #67 (follow-ups and terminal parts) merged. #72 moves `terminal_part()` so the strip row, the BOM and the missing-parts check agree.
- **WS-08 workflow:** #29 merged. #63 (`accept all` runs accept-all; natural tag order) merged.
- **WS-09 PDF:** the cloud coordinator did #49 (novo-pdf). Next: draw multi-sheet generated pages (#68 proposal), and the PDF-overwrite follow-up together with WS-01.
- **WS-10 xref:** #31 merged. Nothing pending.
- **WS-11 terminals:**
  - #30 merged.
  - **#64 (accept-all drawing order) merged with a regression** (horizontal wires number 2/1/3). #71 fixes it; C was reviewing.
  - The terminal-article store decision (see WS-02).
- **WS-12 cables, WS-13 reports, WS-28 CLI/Python, WS-14 checks:** these were done by cloud workers or helpers. Their state is in `handoff/ws12`, `ws13`, `ws28` and `ws14`. Open: #68 (WS-13) and #65 (WS-28), each with B READY and C pending. If no project thread owns them, the lead assigns them or keeps them parked.
- **Tester:** once #53 lands, drive the Gate 1 scenario through `NovoApp::execute`, flip the ignored tests, and add a check that the BOM's terminal count equals the strip's terminal count, if #72 doesn't already.
- **Release/builds:**
  - Actions jobs don't start; ask Tadjiro to check billing / the spending limit (or making the repo public).
  - When CI runs again, run Windows CI on main first.
  - `experimental.yml` (#41) commits a re-resolved Cargo.lock after each check.
- **Layout / Workflow:** no changes needed from today. The Workbench window (#46) follows DESIGN.md.

## Owner items (don't block on these)
- Actions billing.
- Before going public: the owner's Windows user name and the EPLAN folder name are still in old commits; accept that or start a fresh repo. Then switch visibility in Settings. No secrets were found.
- Check single-click selection and the experimental build on Windows.
