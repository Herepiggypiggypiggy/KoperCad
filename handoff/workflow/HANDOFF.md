# Workflow thread handoff

Updated 2026-10-14 17:08 UTC.

## State
- Repo herepiggypiggypiggy/novoelectric; my only branch: `claude/project-thread-o9tc55`.
- #24 (DESIGN.md) MERGED 17:03. docs/DESIGN.md is on main.
- #43 (DESIGN.md follow-ups) MERGED 17:06. No open PR of mine. Next design change =
  new PR from main (restart branch: git checkout -B claude/project-thread-o9tc55 origin/main).
  Queued optional nit: §9 half-sentence on why the empty-canvas New/Open/Recent view is
  not a start screen (bundle with the next design change, not alone).
- #38 (WS-06) review sent (Enter, menu order, potentials as source, facing pins as
  proposals, 6 px pin radius, drag from pin); recheck when WS-06 pings.
- Criteria sent to WS-01, WS-07, WS-08, WS-10, WS-11 and layout; recheck their heads
  when they ping.

## Resume
```
cd <repo>; git fetch origin claude/project-thread-o9tc55 main
git checkout claude/project-thread-o9tc55   # #43's head
# if #43 has merged: git checkout -B claude/project-thread-o9tc55 origin/main
```
Commit trailers: see git log. PR body starts with the attribution comment + "Requested
by Tadjiro" line, ends with the Claude Code footer. Assign Herepiggypiggypiggy.

## Who
- Integration lead session_011PfTJ1QaP1JQJPwXcFnxBE (arbiter, merges)
- Design reviewer session_0114emQ5fSFrzcALbR4S18TU (send SHA after every push)
- Layout session_01UXbCeQW22y6tv74F2Xvbvm (screen, ui-guidelines, #20)
- WS-06 session_019siuxbyyRNZRgBBww1KocJ; WS-08 session_01TAFG6LPGy2WMwhJYXmxqxF;
  WS-11 session_01AqzMrSLZaG8zin7Yf84RZP; WS-07 session_01H3m3ufThxMEzTtG41NrXaB;
  WS-10 session_01KrbvfWtr3dmZqL1bwwyKWF. Full list: coordination/threads.md.

## Decisions already made (don't reopen)
No menu bar; Ctrl+K command surface, I / tapped Space at pointer; Enter accepts
proposal, Tab next proposal (no Enter=repeat); Ctrl+1/2 Schematic/Cabinet, Ctrl+3/4/5
drawers; potentials from declared sources, per-conductor override only; terminals and
cables proposed at location boundaries; routes stored once; no T/corner pieces;
pinned page flow; numbering -K1 or per page -K301 (never -3K1); export never blocked.

## Traps
- Use they/them for Tadjiro. No model names in commits/PRs.
- No admin rights anywhere; write under user profile only.
- Commit trailers: Co-Authored-By line + Claude-Session line (see git log).
