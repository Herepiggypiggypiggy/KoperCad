# NovoElectric main handoff

For a fresh cloud session that takes over as **integration lead**: the role that coordinates the threads and merges PRs.
Last updated 2026-10-14 17:17 UTC (final from the first lead session) by the integration lead (session_011PfTJ1QaP1JQJPwXcFnxBE).

## 0e. State at 2026-10-15 16:40 UTC (integration lead)
- **Main** has about 1022 tests passing (lead's combined local run); product KoperCAD. **No build is planned** until Tadjiro asks; Release is idle and must not dispatch one. Windows hands-on checklist: `docs/WINDOWS-CHECK.md`.
- **Merged since 10:40 UTC (UI and engine):** wheel and table, wire corners and parts, Corners/Lines modes with plain drag moving only the component (Shift+M carries wires, Moving preference), terminals (strips, multi-pole drag, connect/disconnect, bridge bar per run), potentials and PE menus, live potentials (Shift+L, hover, timers with seconds left, theme tokens), Show where it goes (lights the potential), coil/contact coupling, Auto connections preference, pages navigator with groups/panels, contents list with panels, inline page rename (additive `Command::RenamePage`, accepted by the lead), feed numbering L1/L2/L3, Tab/Q while wiring, revisions (Issue window, title block fields, Changes mode), cabinet layout design and `novo-layout` geometry engine, data-folder migration to KoperCAD names, `kopercad.exe` with compat copy, Python module `kopercad` with `novoelectric` shim.
- **Open at 16:40:** #311 (cable tool, WS-08 reopens from main), #300 (inspector Time field; author merges main).
- **Threads busy:** Schematic editor, Canvas engine, App shell, Fast keyboard, Workflow, Layout, Pages, Terminals, Potentials. Release idle.
- **Next on the 26-point list:** points 8 (wheel Other), 9 (rotate selection), 18 (search, Ctrl+K), 20 (cable), 22/23 follow-ups; hands-on checks for 14/16. Follow-ups noted on merged PRs: #266 PE state in menu row, #275 chips, #278 timer in Shift+L, highlight render path for Show where it goes.
- **Rules unchanged;** new: put new tests in a new file, not appended to `tests/editing.rs` (it caused repeated conflicts). `claude/local-builds` branch could not be deleted by the lead (gate denied); content is on main.
- The PROGRESS clock labels in `handoff/integration-lead/PROGRESS.md` before this entry are mixed local/UTC; entries from here on are UTC.

## 0d. State at 2026-10-15 10:40 UTC (integration lead)
- **Main 2d5fff7**, about 919 tests passing (design reviewer's merged run), no open PRs. Product is **KoperCAD** by **KoperVonk** (#201); repo is `Herepiggypiggypiggy/kopercad` (old slug redirects; old-name API calls fail). Code, crates and data folders keep `novo-*` / NovoElectric for now.
- **Builds:** experimental build 2 published 10:07Z on main 424234a. Build 3 waits for double-click text / wire-number rename (WS-01, on the label_hit query from #232). Release dispatches builds by manual dispatch (5-14 min).
- **Shipped today (Tadjiro's UI list):** drag-move without M, pin-click wire, double-click open end / free wire, drop contact on coil to link, row placement and strip pitch, right-click menu + place wheel (Q for Place), grid dots, box-select highlight and N selected, device properties inspector, contrast and joints palette, wire.open check, Endpoint::Free.
- **Open:** text/wire-number rename (WS-01), end-cap drag/resume and 4 px cap (WS-03/WS-06), inspector pin/slide/override arrows, DESIGN nits (grab radius, junction dot radius, +/- spacing wording, Link-to hint wording), repo-slug PR (Release, scripts/installer/Cargo.toml/workflows/docs), README Windows line is stale, GitHub repo description and topics (only Tadjiro).
- **Rules:** token-saving mode (no polling, short messages, one reviewer pass per head); main reviewer session_016C9nTT8cN7voW8t1NQUG4W and design reviewer session_0114emQ5fSFrzcALbR4S18TU; a head that moves after READY goes back to its reviewer; squash with the full SHA and an overriding commit message; one line per merge batch in `handoff/integration-lead/PROGRESS.md`.

## 0c. State at 2026-10-15 04:00 UTC (integration lead)
- **Main d563971**, 713 tests passing locally (fmt, clippy -D warnings clean, Linux). Merged 02:50-03:55: #87, #111, #115-#125, #127-#132, #134, #135, #137 (#116 DESIGN page names; #119 page freeze; #124 shared IEC 81346-2:2019 letter_code table in novo-symbols; #125 blocks; #134 Inter subset for Unicode PDF text).
- **Open:** #126 WS-03 drawing (main + B READY on 1bf0920, waits design), #133 WS-06 page-name letters (A + design), #136 WS-08 coil.contactor + DESIGN §6 (B READY, waits design), #138 WS-01 drawers (A + design), #139 WS-11 optional terminal pins (B).
- **Phase 3 started:** the Data model thread also owns WS-18 Revisions (DESIGN first). A WS-17 PLC thread was requested from the coordinator. The tester is assessing Gate 2.
- **Rules applied tonight:** a head that moves after READY goes back to its reviewer (#122, #124, #126); a third reviewer's pending blocking finding holds a merge (#126).

## 0b. State at 2026-10-15 02:55 UTC (integration lead cse_01FiLvRBKEcNNHbQEGhYyL1S)
- **Main 89971f4.** main + #87 + #111 + #117 + #115 tested together: fmt and clippy -D warnings clean, `cargo test --workspace` 679 passed, 0 failed, 2 ignored (Linux). Since 21:20 this lead merged 35 PRs (#80 through #117); see `handoff/integration-lead/PROGRESS.md`.
- **CI still down** (Actions billing); CI fails within seconds on every push. Local checks remain the signal; Windows unverified.
- **Decisions since §0a:** contactor K by default, project rule for Q; MCB F, MPCB Q, overload F (DESIGN §6, #92). Generated sheets are 20a, 20b … skipping i, l and o, never dotted (#108, #110, #115). Pending #116: page names never contain a dot, and inserted pages take letters (3a). The terminal article comes from `Terminal.part`, else strip `terminal_part.<kind>`, through one resolver (#99). Structure nodes and Dismissal records are in core (#117). WS-08 owns one shared `letter_code(function)` table in novo-symbols.
- **Open PRs:** #105 WS-01 focus (needs A's delta READY on 2a83164), #112 WS-08 (A), #113 WS-06 (A + design), #116 DESIGN page names (design reviewer).
- **Queued next:** WS-01 bundles (accept-all/Delete+Untitled, drawers, Issues outline, pan, dock+inspector); WS-03 perf then drawing and the F5 gap; WS-02 page freeze then blocks; WS-06 next_page_name letters after #116; WS-11 terminal suggestions onto Dismissal records and per-terminal `part` retired.
- Threads that stopped around 22:30 (app shell, canvas, editor, keyboard workflow, design reviewer) were nudged at 02:50.

## 0a. Lead change (2026-10-14 21:20 UTC)
- **Integration lead from 21:20 UTC:** the project thread "Catch up and resume the build" (cse_01FiLvRBKEcNNHbQEGhYyL1S, branch `claude/project-thread-6lmg5c`). The old lead thread (cse_011PfTJ1QaP1JQJPwXcFnxBE) handed over to it; the cloud coordinator (session_01DZ68tTULUv7Uhe8ecKbC47) is stopped. There is one lead. Send verdicts and merge requests to it.
- **CI still down:** Actions jobs fail with "recent account payments have failed or your spending limit needs to be increased". Owner told. Local fmt + clippy + tests remain the CI signal.
- #73 is closed. #72 got CHANGES NEEDED from the main reviewer and its fixer has stopped, so it was closed; WS-07 redoes it from main following WS-02's decision (a terminal's own article is `Terminal.part` only, the strip default is `terminal_part.<kind>`, `novo_parts::terminal_part` is the single resolver with no strip-device fallback).
- Merged by this lead: #80 and #88 (DESIGN.md, design READY, docs only), #85 (WS-11: cable 3G and shield fixes, reviewer B READY).
- Reviewers now: main reviewer (cse_016C9nTT8cN7voW8t1NQUG4W) and reviewer B (session_01QSnXFBDRyvwwzhT1S1gFGN) share correctness reviews; one correctness READY is enough for fixes to existing crates. UI PRs also need the design reviewer (cse_0114emQ5fSFrzcALbR4S18TU).
- New project thread "Outputs: PDF, reports and checks" (cse_01GNLWH7SKcFppZwCxZoC1rw) owns novo-pdf, novo-reports and novo-checks. WS-11 takes cable (WS-12) fixes. WS-28 is parked.
- Main 9c9f522 + lopdf move: fmt and clippy -D warnings clean, cargo test --workspace 626 passed, 0 failed, 3 ignored (Linux). Release thread: `cargo clippy --target x86_64-pc-windows-gnu` clean on 15c9992; an untested Windows zip is at /mnt/project-files/builds/.
- `lopdf` moved into `[workspace.dependencies]` (lead PR).
- Several project threads restarted at 21:20, not 23:20: read `handoff/RESUME-2320.md` anyway.

## 0. Current state (2026-10-14 20:30 UTC, wrap-up by cloud coordinator session_01DZ68tTULUv7Uhe8ecKbC47)
Sections 6 and 11 below are the 17:17 snapshot; this section supersedes them. **Project threads resuming at 23:20 UTC: read `handoff/RESUME-2320.md` next.**
- **Who runs the project:** from 17:30 to 20:30 UTC a cloud coordinator ran it (see `handoff/cloud-coordinator/`). It has stopped, as have all cloud reviewers and workers (`handoff/coordinator/cloud-roster.md`); the owner is near their credit limit. Coordination returns to the project's integration lead thread.
- **Review policy:**
  - Original threads' PRs: main reviewer (correctness); UI PRs also need the design reviewer.
  - New-crate PRs: two correctness reviewers where available.
  - Any CHANGES NEEDED blocks a merge. Merge only on the *named* reviewer's READY on the *current* head; #64 broke this rule and shipped a regression. The lead never approves work it or its helpers wrote.
  - After READY, a head that moved only by mechanical main merges keeps READY if `scripts/check-mechanical-merge.sh <ready> <head>` prints OK and `cargo test --workspace` passes on the new head; paste the output on the PR. Anything else goes back to the reviewer.
- **CI is down:** GitHub Actions starts no jobs on this account (billing / spending limit; owner to check, or making the repo public gives free minutes). The reviewers' local fmt + clippy -D warnings + tests are the CI signal. **Windows is unverified** for everything merged today.
- **Merged today since 17:20 UTC: 44 PRs (34 code/process, 10 handoff, counting this #69):**
  - Code: #20 theme, #41 CI/experimental, #34 app storage+canvas, #28 canvas, #29 workflow, #42 Gate 1 tests, #45 core SetText/Page.structure, #36 parts, #30 terminals, #48 tester gaps, #47 marks+page frame, #50 design checks, #31 xref, #57 tester gaps + F3, #51 fixed crate list, #52 pre-public scrub, #38 schematic editor, #54 cables, #58 + #61 merge-main.sh, #59 example rename, #49 PDF export, #46 Workbench window, #56 CLI + Python, #64 accept-all order (regression, fixed by #71), #55 reports, #67 parts follow-ups, #62 coil K1, #63 accept all, #65 WS-28 report exports, #70 editor `art` feature, #71 accept-all axis order, #68 terminal/cable diagram reports, **#53 Gate 1 in the app** (editor, command field, terminals drawer, xref, checks, PDF hosted in novo-app).
  - Handoffs: #44, #60, #66, #69 (this wrap-up, which also carries the reviewer-a, reviewer-c and ws13 handoffs), #74 fixer, #75 ws12, #76 gate1-integration, #77 ws28, #78 reviewer-b, #79 design-reviewer.
- **Last verified main:** main + #53 gave fmt clean, clippy -D warnings clean and `cargo test --workspace` 625 passed, 0 failed (Linux). Windows has not been verified.
- **Open PRs at wrap-up:**
  | PR | What | Reviewer | State |
  | --- | --- | --- | --- |
  | #72 | One terminal article source: strip row, BOM and missing-parts agree (fixer) | main reviewer | **not reviewed yet**; checklist in `handoff/reviewer-a/ROADMAP.md`. It touches novo-reports/Cargo.toml, which #68 also changed, so expect a trivial conflict: keep one `novo-terminals` line. |
  | #73 | ws13 handoff | | superseded: its files went into #69. Close it. |
- **Follow-ups owed (not in any open PR).** The full lists are in `handoff/reviewer-a/ROADMAP.md`, `handoff/reviewer-c/ROADMAP.md`, `handoff/design-reviewer/HANDOFF.md` (10 UI notes) and `handoff/gate1-integration/HANDOFF.md`. Main items:
  - WS-01:
    - Don't overwrite a user's own PDF next to the .novo (`/Producer` check, or temp file + rename).
    - Focus::Canvas applies while a panel button has focus.
    - Remove the app's accept-all workaround that #63 made redundant.
    - Drawers, Issues outline mode, inspector, dock, Space-held pan.
  - WS-02, WS-11 and WS-07: pick one terminal-article store (core `Terminal.part` vs novo-terminals properties).
  - WS-03:
    - Make wgpu optional in novo-canvas.
    - Draw multi-sheet generated pages, with WS-09.
    - Put xref labels into the scene.
  - WS-13:
    - "20.1" sheet names can collide with real sub-pages.
    - The cable diagram builds cable screens twice.
    - Pinned flow vs growing generated pages.
  - WS-12: the `choose_type` break bug (L+PE → numbered 3G); shield connected to a deleted terminal.
  - WS-28:
    - `execute_one` rollback on NothingCreated; exit-2 help text.
    - Switch novo-cli to novo-editor's `art` feature (#70).
    - Python `place(main=...)` default; check()/export_pdf()/batch(); then the Workbench console.
  - Lead: move `lopdf` (dev-dep in novo-app and novo-pdf) into `[workspace.dependencies]`.
  - Lead: close #73.
  - Tester: with #53 merged, drive the Gate 1 scenario through `NovoApp::execute` and flip the ignored tests.
  - Windows: single click doesn't select a symbol under Xvfb (box select works); check on Windows.
- **Going public:** no secrets anywhere in history; personal paths/ids scrubbed from current files (#52); the owner's Windows user name and an EPLAN folder name remain in older commits. Owner decides (accept, or fresh repo) and switches visibility in Settings.
- **Crate list:** fixed in CLAUDE.md and `[workspace.dependencies]` since #51; PRs never edit those lines. Merge main with `scripts/merge-main.sh` (#58 makes it keep main's list exactly). Since #61 it also keeps a branch's external dependency lines and rejects duplicate keys, so nothing is re-added by hand. It still aborts on any other conflict (code, or handoff/*/PROGRESS.md as on #70). Resolve those by hand, keep both log lines, and send the new SHA to the reviewer, because check-mechanical-merge.sh will not pass a hand resolution.
- **Gate 1:** #53 put the first real flow in the app. The tester now drives the Gate 1 scenario through `NovoApp::execute`, and Tadjiro tries it on Windows from the experimental build (once CI runs).
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
