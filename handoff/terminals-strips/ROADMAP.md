# Terminals and strips: roadmap

Owner: thread "Terminals and strips" (Tadjiro's list, points 15, 24, 25, 26).

Done in PR 1: multi-pole drag (15), connect terminals (24), one-ended standard terminal (25), strip naming by adjacency and Up / Ctrl+Up (26).

Next:
- Fast keyboard thread: confirm Up / Ctrl+Up (terminal.strip_up / terminal.strip_lowest) do not clash with nudge.
- Gate 1 fixture still uses the two-ended alias `term`; tester may move it to `x`.
- A terminal placed directly left of a strip sorts first but keeps next number; consider renumbering.
- Connected symbol has bars on both sides for every member; end members could omit the outer bar.
