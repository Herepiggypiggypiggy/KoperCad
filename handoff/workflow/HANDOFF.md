# Workflow thread handoff

Updated 2026-10-14 21:26 UTC.

## State
- Repo herepiggypiggypiggy/novoelectric; my only branch: `claude/project-thread-o9tc55`.
- DESIGN.md on main (#24, #43). All feature PRs I reviewed are merged (#30, #31, #36,
  #38, #46, #47, #53).
- #80 MERGED 21:26 (saving, dots, letter codes, Enter target).
- #88 MERGED 21:37 (generated pages reflow in own sections; Unsaved location; save a copy).
- #92 MERGED 21:43 (contactors K default with K|Q rule; highlighted proposal dashed
  2 px, never solid).
- #100 MERGED 21:55 (blocks live in project, libraries are copy sources; tags allocated,
  properties/texts substituted, parts proposed).
- OPEN: #106 (n of m per section), head 9f5e0fb, with design reviewer. Subscribed.
- Criteria for #80 sent to WS-01, WS-03, WS-08, WS-05, WS-11; recheck their heads.
- Integration lead is now session_01FiLvRBKEcNNHbQEGhYyL1S ("Catch up and resume the
  build"). Design PRs go to the design reviewer session_01NLERJkVHpsVcABrarTR3CF.
- Design-reviewer notes owed to WS-01/WS-03 are in handoff/design-reviewer/HANDOFF.md
  (the design reviewer routes them; note 8 is settled by #80).

## Resume
```
cd <repo>; git fetch origin claude/project-thread-o9tc55 main
git checkout claude/project-thread-o9tc55   # then reset to main if no PR open
# if #80 has merged: git checkout -B claude/project-thread-o9tc55 origin/main
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
