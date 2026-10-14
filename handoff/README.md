# Handoffs

This folder is a copy of the project mount's `handoff/` (snapshot 2026-10-14 17:35 UTC), committed so cloud sessions without `/mnt/project-files` can continue. Thread folders use short names here (`ws1` = WS-01 app shell, `ws3` = WS-03 canvas, ...); paths inside the files such as `handoff/ws-03-canvas/next-pr/*.patch` refer to the files in the matching short-name folder.

Every thread keeps its own folder here, `handoff/<thread-slug>/`, with three files. If a session dies or hits its usage limit, a fresh cloud session must be able to continue from these files alone.

- `ROADMAP.md`: the thread's scope, done items, next items in order, and open questions. Update it whenever the plan changes.
- `HANDOFF.md`: the current state, rewritten as a whole each time. Cover the branch, open PR and head SHA, review state, what is half-done and where, how to resume (commands), who to talk to (session ids from coordination/threads.md), traps and decisions already made. Keep it under 80 lines.
- `PROGRESS.md`: an append-only log with one line per meaningful step, starting `YYYY-MM-DD HH:MM UTC`. Append after every push, review verdict, merge, decision or blocker.

Rule: update PROGRESS.md and HANDOFF.md before ending every turn in which something changed.

The main handoff for the whole project is `handoff/MAIN-HANDOFF.md`.
