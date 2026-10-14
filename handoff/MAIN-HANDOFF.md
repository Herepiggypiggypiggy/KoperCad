# NovoElectric main handoff

For a fresh cloud session that takes over as **integration lead**: the role that coordinates the threads and merges PRs.
Last updated 2026-10-14 17:17 UTC (final from the first lead session) by the integration lead (session_011PfTJ1QaP1JQJPwXcFnxBE).

## 0. Current state (2026-10-14 19:35 UTC, coordinator session_01DZ68tTULUv7Uhe8ecKbC47)
Sections 6 and 11 below are the 17:17 snapshot; this section supersedes them.
- **Who runs the project:** Tadjiro handed full coordination to the cloud coordinator session above. Project threads resume at **23:20 UTC**; each was sent a briefing and must `git fetch` and read its own `handoff/<thread>/` on main first.
- **Sessions:** see `handoff/coordinator/cloud-roster.md` (reviewers A, B, C, design reviewer, fixer, workers WS-12/13/28, Gate 1 integration).
- **Review policy:**
  - Original threads' PRs: main reviewer A (correctness); UI PRs also need the design reviewer.
  - Cloud workers' new-crate PRs: BOTH reviewers B and C (their overlap caught two real bugs the other missed: CSV formula injection in #55, recovery-file deletion in #56).
  - Any CHANGES NEEDED blocks a merge, whoever else said READY. The coordinator never approves work it or its helpers wrote.
  - After READY, a head that moved only by mechanical main merges keeps READY if `scripts/check-mechanical-merge.sh <ready> <head>` prints OK and `cargo test --workspace` passes on the new head; paste the output on the PR. Anything else goes back to the reviewer.
- **CI is down:** GitHub Actions starts no jobs on this account (billing / spending limit; owner to check, or making the repo public gives free minutes). The reviewers' local fmt + clippy -D warnings + tests are the CI signal; Windows is unverified.
- **Merged today (19):** #20 theme, #41 CI/experimental, #34 app storage+canvas, #28 canvas, #29 workflow, #42 Gate 1 tests, #45 core SetText/Page.structure, #44 handoffs, #36 parts, #30 terminals, #48 tester gaps, #47 marks+page frame, #50 design checks, #31 xref, #57 tester gaps + F3, #51 fixed crate list, #52 pre-public scrub, #38 schematic editor, #54 cables.
- **Open PRs:**
  | PR | What | State |
  | --- | --- | --- |
  | #46 | WS-01 Workbench window | design READY at 1fb64af; reviewer A pending |
  | #49 | WS-09 PDF export | READY 95d4749; main merge ad4fd1e re-placed pdf-writer dep → B confirming |
  | #53 | Gate 1 integration (draft, stacked) | worker hosting editor/command field/terminals/xref/PDF in novo-app |
  | #55 | WS-13 reports | B READY 37f684f (CSV injection fixed); C pending; main merge must re-add rust_xlsxwriter + calamine deps |
  | #56 | WS-28 CLI + Python | C CHANGES NEEDED (create/save_as deletes recovery file); worker fixing on 0089b7c |
  | #58 | merge-main.sh keeps main's crate list | C re-checking 42e9b02 |
  | #59 | rename novo-xref scale example | B pending |
- **Crate list:** fixed in CLAUDE.md and `[workspace.dependencies]` since #51; PRs never edit those lines. Merge main with `scripts/merge-main.sh` (#58 makes it keep main's list exactly). Branches older than #51 that add external deps need those dep lines re-added by hand after taking main's Cargo.toml.
- **Going public:** no secrets anywhere in history; personal paths/ids scrubbed from current files (#52); the owner's Windows user name and an EPLAN folder name remain in older commits. Owner decides (accept, or fresh repo) and flips visibility in Settings.
- **Next for Gate 1:** #46 and #49 merge → #53 rebases and lands → tester drives the Gate 1 scenario through `NovoApp::execute`.
- **Traps:** worktrees sharing one CARGO_TARGET_DIR run stale test binaries; give each its own and delete when done (disk ~40 GB). Example names must be unique across crates (cargo output collision, #50/#59).

## 1. What this is
NovoElectric is a local-first electrical CAD program in Rust: egui/eframe 0.36, wgpu, SQLite, with PyO3 as the scripting layer.
- Repository: `Herepiggypiggypiggy/NovoElectric`, default branch `main`.
- Roadmap: the Claude Doc https://claude.ai/code/artifact/8ff48512-c76a-4566-8937-166b4b128d2e, summarised in `docs/ROADMAP.md`.
- Owner: Tadjiro. They work in the project thread "coordination" and is often away.

## 2. Owner's standing instructions (hard rules)
1. **No admin rights, ever.** Everything must install, run, update and be scripted as a standard Windows user: per-user install, HKCU and %LOCALAPPDATA% only.
2. **Never modify** the owner's existing EPLAN project folder on their PC (path kept on the project mount, not in the repo).
3. **It is its own program, not an EPLAN copy.** Be critical, design creatively, and let the layout, workflow and review threads critique each other. Fix the EPLAN pain points without copying EPLAN. The pain points are in the roadmap doc under "Lessons from EPLAN".
4. **Don't wait on him.** Pick reasonable defaults and keep moving. He trusts the lead to run the project efficiently and to re-examine the setup routinely.
5. The integration lead merges PRs itself. Never ask Tadjiro to review or merge.
6. Every thread keeps `handoff/<thread>/ROADMAP.md`, `HANDOFF.md` and `PROGRESS.md` current. The convention is in `handoff/README.md`.

## 3. Merge procedure (REST only, GraphQL is blocked)
A PR merges when the right reviewer says **READY on the current head SHA** and CI is green on that head (ubuntu-latest and windows-latest, plus the installer if it ran).
```
R=repos/Herepiggypiggypiggy/NovoElectric
gh api $R/pulls/N --jq .head.sha
gh api $R/commits/<sha>/check-runs --jq '[.check_runs[]|"\(.name)=\(.conclusion // .status)"]'
gh api -X PUT $R/pulls/N/merge -f merge_method=squash -f sha=<sha>
```
- Mark a draft PR ready: `gh api -X POST $R/pulls/N/ccr/ready_for_review`.
- **Mechanical main merges keep READY (from #51):** when a PR's head moved after READY only by merging main (use `scripts/merge-main.sh`), run `scripts/check-mechanical-merge.sh <ready-sha> <new-head>`. If it prints `OK`, the READY carries over: re-run `cargo test --workspace` on the new head (a lock-only difference passes the script), paste the script output into a PR comment, then merge with the new head's full sha. Any other change after READY goes back to the reviewer.
- If the head moved after READY: docs-only or trivial changes can merge after you read the diff yourself; anything else goes back to the reviewer.
- After a core (#35-style) merge, check open PRs for build breaks: exhaustive matches, struct literals.
- The repo is private on a free plan, so there is no branch protection and Actions capacity is limited. Keep CI load low.

## 4. Threads (roster with session ids: `coordination/threads.md`)
- **Reviewers.** The main reviewer covers core, engine, storage and test PRs. The design and workflow reviewer covers UI, visual and DESIGN.md PRs.
- **Project coordinator** (session_018btnHEHYdedX4jueCivWjT). It can start and resolve threads (start_thread_session). Ask it when a new thread is needed.
- **Workstreams.** WS-01 app shell, WS-02 data model (the only thread that may change the novo-core API), WS-03 canvas, WS-04 storage (retiring), WS-05 symbols (retires after #37), WS-06 editor, WS-07 parts, WS-08 keyboard and command field, WS-10 cross references, WS-11 terminals.
- **Other threads.** Layout/branding, Workflow (owns DESIGN.md), Tester.
- **Release/build thread** (cse_01UuGC6jT84yVrbcwGaSJ5Df) owns `.github/workflows`, the installer, run-latest.ps1, the launcher and the GPU check. It has #41.
- **Design and workflow reviewer** (cse_0114emQ5fSFrzcALbR4S18TU).
- **Remote Control session on Tadjiro's PC** (id on the project mount: coordination/threads.md). It only takes his own typed messages; you cannot message it.

Working rules, also in CLAUDE.md via #39:
- A PR is frozen once READY; follow-ups go in a new PR from main.
- Threads ping the reviewer themselves after each push.
- Check that your PR is still open before pushing.
- Keep novo-core PRs small.
- Build no UI beyond a thin layer until DESIGN.md merges.
- Work on events, with no hourly polling.

## 5. Design decisions already made
- Wiring is pin to pin. Corners, T-junctions and crossings are **derived from topology** (canvas `JunctionGlyph::from_directions`). No junction style is stored, and there are no manual corner or T pieces.
- **One command field** replaces menus and EPLAN's insert centre. `I` opens it in place mode; Ctrl+K or a tapped Space opens it for everything. Grammar: a noun jumps, verb+noun runs, mnemonic+device places a child. An empty query shows a grouped menu.
- **Tab = next proposal, Enter = accept. R and X orient while placing.** WS-08's placement tool owns orientation, and the canvas binds no orientation keys.
- Ctrl+1/2 switch lenses. Ctrl+3/4/5 open the one drawer on Terminals, Cables or Devices. There is no Ctrl+J.
- Terminal rule: `ProjectInfo.rules.terminals { same_name: Off|Join|Refuse, join_link: Connector|Jumper }`. New projects get Join+Connector; old files load Off. novo-terminals reads this rule and does no name recognition of its own.
- Potentials from terminal names (-X4:PE) sit behind `rules.potentials.from_terminal_names`: on for new projects, off when absent.
- The theme is graphite dark (novo-theme) and the logo is "Nova". Tadjiro likes the current symbols.

## 6. State right now
Merged recently: #37 symbols follow-ups, #43 DESIGN follow-ups, #24 DESIGN.md, #39 thread rules, #33 recovery fix, #3 installer, #25 aliases and favourites, #32 Gate 0 summary, and earlier #4, #5, #10-#19, #21-#23, #26, #27.

**Gate 0** is complete: the GPU check passed on the work PC (Quadro P1000; 10k objects 1135 fps avg, 127 min; 100k 113 avg with stutter). Second-PC shortcut setup: launcher/setup-novoelectric-shortcut.ps1.

**Gate 1 (current milestone, "first real flow"):** in the real app, open or create a project, place a motor starter, wire it, see its terminals and potentials, then save and reload. The tester writes the end-to-end test listing the gaps; WS-01 makes it work in novo-app.

Open PRs (head SHA, then what to do):
| PR | Head | State / next |
| --- | --- | --- |
| #20 theme | 2d60fdd | READY (design reviewer); auto-merge job on green CI. |
| #42 Gate 1 scenario | 3a98343 | Tester's scoreboard: ignored tests name the PR that closes each gap. Main reviewer. |
| #28 canvas | 924be9b | CHANGES NEEDED: junction glyph must count wires, not sides (3+ wires = dot). WS-03 told. After #35: draw Text records, avoid drawing tags twice, refresh Potential labels. |
| #29 command field | 01d8023 | CHANGES NEEDED (undo must yield to text fields; duplicate leaves poles on -K1). WS-08 told. After #35: smart duplicate must copy texts. |
| #30 terminals | 800680b | CHANGES NEEDED (pin refit on style swap; refuse empty/duplicate names; mark temp keys). WS-11 told. After #35: map PotentialKind::Pen, drop own PE/N name recognition. Then the tester flips the X1:PE fixture. |
| #31 xref | 37b3247 | Main reviewer; engine only for now. |
| #34 app shell | baf6e72 | Main reviewer. Wires storage and canvas into the app. |
| #35 core | 7677b8c | READY; auto-merge job runs on green CI. Merge first among the code PRs, then route the follow-ups above. |
| #36 parts | ea67da2 | CHANGES NEEDED: 3 data-loss bugs in the bulk-table round trip. WS-07 told. |

| #38 editor | 81c0caa | CHANGES NEEDED from the design reviewer (5 bugs + preview ghost wires). WS-06 told. Check overlap with #28 and #29 and against DESIGN.md (right-click potentials, sticky tools). |
| #41 CI and experimental build | 618b36f | Also adds scripts/setup-new-pc.ps1. Release thread's PR. Folds in #40 (now closed): cancel stale PR runs, skip docs-only, narrow installer, per-PR drop on experimental failure. Merge on the main reviewer's READY without waiting for CI (workflow-only, CI backlog of about 38 runs). |

Suggested merge order: #41, #35, #37, #20, then #34, #28, #29, #30, #36, #38 and #31, as each becomes READY.

## 7. Routines
- A project health review runs every 3 hours: a send_later scheduled task in the lead's session (trig_01SMuaHZRY73L2au36F4CQ3P, first run 19:59 UTC). A new lead should schedule its own. It checks open PRs and review state, idle or stuck threads, rework, DESIGN.md and Gate 1 progress, then fixes the process and updates `coordination/threads.md`.
- Keep the roadmap Claude Doc current; it's at rev 22, and "Lessons from EPLAN" is the section to touch.

## 8. Experimental build and launcher
- Every push to main and every PR update rebuilds the `experimental` pre-release: main plus all open PRs, merged in PR order. Lock-only conflicts are re-resolved; conflicting PRs are skipped.
- Tadjiro's desktop shortcut `NovoElectric (latest).lnk` runs `scripts/run-latest.ps1`, which downloads the newest build into %LOCALAPPDATA%\NovoElectric\experimental with `gh release download`.

## 9. Traps learned
- `gh pr view` fails because GraphQL is blocked (403). Use `gh api` REST.
- Threads push to branches whose PRs were already merged. Tell them to restart from main and open a new PR.
- A head moving after READY made background merges fail. Always merge with `-f sha=`.
- Core enum additions can break other PRs' exhaustive matches; most use `_` arms, but check anyway.
- send_message to the RC session is rejected; only Tadjiro's own messages reach it.

## 10. Exceptions to the CI rule
- Docs-only and workflow-only PRs may merge on a reviewer's READY without CI. CI cannot validate them meaningfully, and the queue was 38 runs deep.
- DESIGN.md is on main. UI PRs are reviewed against it by the design reviewer. Its follow-up PR from the workflow thread covers the page frame/title block, Dutch command aliases, the first-five-minutes journey, post-Gate-1 marks, auto-connect on alignment, pin vs symbol drag, and the Documents\NovoElectric default.

## 11. First things for the next lead
1. The background merge jobs for #20 (READY at 2d60fdd) and #35 (READY at 7677b8c) die with the old session. Merge both by hand once CI is green on those heads.
2. Merge #41 on the main reviewer's READY. It's workflow-only, so CI isn't required.
3. After #35 merges, send the follow-ups: #28 Text records and labels, #29 duplicate copies texts, #30 PEN and drop own name recognition.
4. The 3-hourly health-review scheduled task belonged to the old session. Schedule your own with send_later.
5. A decision is already sent to WS-01: no save prompts. A saved project writes into its .novo file continuously, and an untitled one is kept and offered back.
6. Tadjiro's second PC: he has launcher/setup-novoelectric-shortcut.ps1, the fixed version. Ask whether it worked.
